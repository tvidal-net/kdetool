/// A parsed HOCON value.
///
/// Object entries keep their insertion order and may contain duplicate keys;
/// [`Value::get`] returns the last occurrence, matching HOCON's rule that a
/// later assignment overrides an earlier one.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// Looks up a direct child by key, returning the last match (override wins).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v),
            _ => None,
        }
    }

    /// A short human-readable name for the variant, used in error messages.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "a boolean",
            Value::Integer(_) => "an integer",
            Value::Float(_) => "a float",
            Value::String(_) => "a string",
            Value::Array(_) => "an array",
            Value::Object(_) => "an object",
        }
    }
}
