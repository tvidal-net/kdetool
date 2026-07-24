use super::error::Error;
use super::value::Value;

/// Parses a HOCON document into a [`Value`].
///
/// This is a deliberately small subset, enough for configuration files and easy
/// to grow later:
///
/// * objects, with or without the outer braces at the document root;
/// * `key = value` and `key : value`, plus the `key { ... }` nested-object
///   shorthand;
/// * arrays;
/// * double-quoted strings (with the usual escapes) and unquoted strings;
/// * `true` / `false` / `null`, integers and floats;
/// * `#` and `//` line comments;
/// * commas and/or newlines as separators.
///
/// Not yet supported (rejected or treated literally): `${...}` substitutions,
/// triple-quoted strings, `include`, and dotted path keys. These are the natural
/// next extensions.
pub fn parse(input: &str) -> Result<Value, Error> {
    let mut parser = Parser::new(input);
    parser.skip_separators();
    let value = if parser.peek() == Some('{') {
        parser.parse_object()?
    } else {
        parser.parse_members(None)?
    };
    parser.skip_separators();
    match parser.peek() {
        None => Ok(value),
        Some(c) => Err(parser.error(format!("unexpected trailing input: {c:?}"))),
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(c) = c {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        c
    }

    fn error(&self, message: impl Into<String>) -> Error {
        Error::at(self.line, self.column, message)
    }

    /// Skips inline whitespace and line comments, but stops at a newline.
    fn skip_inline(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') => {
                    self.bump();
                }
                Some('#') => self.skip_line(),
                Some('/') if self.peek_at(1) == Some('/') => self.skip_line(),
                _ => break,
            }
        }
    }

    /// Skips whitespace, newlines, commas and line comments between tokens.
    fn skip_separators(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some(',') => {
                    self.bump();
                }
                Some('#') => self.skip_line(),
                Some('/') if self.peek_at(1) == Some('/') => self.skip_line(),
                _ => break,
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.bump();
        }
    }

    fn parse_object(&mut self) -> Result<Value, Error> {
        self.bump(); // consume '{'
        let object = self.parse_members(Some('}'))?;
        match self.peek() {
            Some('}') => {
                self.bump();
                Ok(object)
            }
            _ => Err(self.error("unterminated object: expected '}'")),
        }
    }

    /// Parses object members until `terminator` (or EOF when `None`).
    fn parse_members(&mut self, terminator: Option<char>) -> Result<Value, Error> {
        let mut entries = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                None => break,
                c if c == terminator => break,
                _ => {}
            }
            let key = self.parse_key()?;
            self.skip_inline();
            let value = match self.peek() {
                Some('{') => self.parse_object()?,
                Some('=') | Some(':') => {
                    self.bump();
                    self.skip_separators();
                    self.parse_value()?
                }
                Some(c) => return Err(self.error(format!("expected '=', ':' or '{{' after key, found {c:?}"))),
                None => return Err(self.error("expected a value after key, found end of input")),
            };
            entries.push((key, value));
        }
        Ok(Value::Object(entries))
    }

    fn parse_key(&mut self) -> Result<String, Error> {
        if self.peek() == Some('"') {
            return self.parse_quoted();
        }
        let mut key = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace() || matches!(c, '=' | ':' | '{' | '}' | '[' | ']' | ',') {
                break;
            }
            key.push(c);
            self.bump();
        }
        if key.is_empty() {
            return Err(self.error("expected a key"));
        }
        Ok(key)
    }

    fn parse_value(&mut self) -> Result<Value, Error> {
        self.skip_inline();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => Ok(Value::String(self.parse_quoted()?)),
            Some(_) => Ok(self.parse_bare_value()),
            None => Err(self.error("expected a value, found end of input")),
        }
    }

    fn parse_array(&mut self) -> Result<Value, Error> {
        self.bump(); // consume '['
        let mut items = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    break;
                }
                None => return Err(self.error("unterminated array: expected ']'")),
                _ => items.push(self.parse_value()?),
            }
        }
        Ok(Value::Array(items))
    }

    fn parse_quoted(&mut self) -> Result<String, Error> {
        self.bump(); // consume opening '"'
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.error("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('b') => out.push('\u{0008}'),
                    Some('f') => out.push('\u{000C}'),
                    Some(other) => return Err(self.error(format!("invalid escape: \\{other}"))),
                    None => return Err(self.error("unterminated escape sequence")),
                },
                Some(c) => out.push(c),
            }
        }
    }

    /// Reads an unquoted value up to the next newline, comma, comment or closing
    /// bracket, then interprets it as a bool, null, number or string.
    fn parse_bare_value(&mut self) -> Value {
        let mut raw = String::new();
        while let Some(c) = self.peek() {
            match c {
                '\n' | ',' | '}' | ']' => break,
                '#' => break,
                '/' if self.peek_at(1) == Some('/') => break,
                _ => {
                    raw.push(c);
                    self.bump();
                }
            }
        }
        let text = raw.trim();
        match text {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "null" => Value::Null,
            _ => {
                if let Ok(i) = text.parse::<i64>() {
                    Value::Integer(i)
                } else if let Ok(f) = text.parse::<f64>() {
                    Value::Float(f)
                } else {
                    Value::String(text.to_string())
                }
            }
        }
    }
}
