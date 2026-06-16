# kwintool

A small command-line tool for manipulating KDE windows from shell scripts on
**KDE Plasma 6 / Wayland**.

Its headline feature is **focus-or-start**: bring an application's window to the
front if it is already open, otherwise launch it — the kind of one-keystroke
behaviour that is trivial to bind to a global shortcut but surprisingly hard to
implement on modern KDE.

## Motivation

On X11 you could lean on tools like `wmctrl` or `xdotool` to find a window by
class and activate it. Under Wayland the compositor no longer exposes that
information to arbitrary clients, so window management has to go *through* the
compositor. On KWin that means **KWin scripting**.

[`kdotool`](https://github.com/jinliu/kdotool) bridges this gap, and it served
me well for a long time. Its model, however, is to generate an *ad-hoc* KWin
script for every invocation: it writes the script to disk, registers it, runs
it, unregisters it, and cleans up. In practice this is:

- **Slow** — a register/run/unregister round-trip per command.
- **Fragile** — interrupted runs routinely leave orphaned scripts registered and
  stray files behind in the KWin script directories.

`kwintool` takes the opposite approach: it ships **one long-lived KWin script**
that is installed and started once. The CLI then talks to that resident script
over D-Bus, so each command is just a message round-trip — no per-call
installation, no cleanup, nothing left behind.

## How it works

```
+ kwintool
  |- src/                     Rust CLI
  |- kwin/
  |  |- contents/code/main.js KWin script (resident command interpreter)
  |  |- metadata.json         KWin script package metadata
```

1. The CLI parses your arguments into a compact command string of the form
   `search && search && action;action`.
2. It owns a well-known D-Bus name, then triggers the resident KWin script via a
   global shortcut.
3. The script calls back to fetch the command, finds the first matching window
   in the stacking order, applies the actions, and replies with the window id.

Search criteria are matched against window properties (resource class, resource
name, caption, desktop). Actions move, resize/maximize, and activate the matched
window.

## Building

```shell
cargo build --release
# binary at target/release/kwintool
```

### Installing the KWin script

The CLI drives a KWin script that must be installed and running. Install the
bundled package and enable it:

```shell
kpackagetool6 --type KWin/Script --install kwin
# then enable "KDE KWin Tool" in System Settings → Window Management → KWin Scripts
```

Once enabled the script registers two global shortcuts (`KWinToolAction` and a
debug toggle) and stays loaded for the session.

> **Status:** early development (`0.1.0`). Automatic installation/registration of
> the KWin script from the CLI is on the roadmap; for now install it manually as
> above.

## Usage

```
kwintool [OPTIONS] [PROGRAM] [ARGS...]
```

### Arguments

| Argument     | Description                                                                 |
|--------------|-----------------------------------------------------------------------------|
| `PROGRAM`    | Executable to focus or launch. When no `--class`/`--name`/`--title` pattern is given, its name is matched against the window resource class (e.g. `dolphin` → `^dolphin$`). |
| `ARGS...`    | Arguments forwarded to `PROGRAM` when it has to be launched.                 |

### Search criteria

These narrow down which window is targeted. Regex values are matched
case-insensitively. Prefix any regex with `!` to **negate** it.

| Option                 | Description                                              |
|------------------------|----------------------------------------------------------|
| `-c, --class <REGEX>`  | Match the window **resource class**.                     |
| `-n, --name <REGEX>`   | Match the window **resource name**.                      |
| `-t, --title <REGEX>`  | Match the window **caption/title**.                      |
| `-d, --desktop <N>`    | Restrict the search to virtual desktop index `N`.        |

### Actions

These are applied to the matched window. `activate` is appended automatically
**unless `--id` is given**.

| Option                    | Description                                                        |
|---------------------------|-------------------------------------------------------------------|
| `-D, --to-desktop <N>`    | Move the window to desktop `N`. Use `-1` for *all* desktops.       |
| `-S, --to-screen <REGEX>` | Move the window to the first screen whose model matches `REGEX`.   |
| `-g, --geometry <GEO>`    | Set the window position, size and maximize state (see below).      |
| `-i, --id`                | Print the matched window id to stdout instead of activating it.    |

### Other options

| Option            | Description                                  |
|-------------------|----------------------------------------------|
| `-v, --verbose`   | Print diagnostic messages to standard error. |
| `-V, --version`   | Print version information.                    |
| `-h, --help`      | Print help information.                       |

### Geometry mini-language

The `--geometry` value is a concatenation of tokens; order does not matter and
the last occurrence of a token wins. Coordinates may be given in pixels or, with
a trailing `%`, as a proportion of the window's available screen area. Invalid
tokens (e.g. an undefined coordinate like `a3`) are rejected with an error.

| Token  | Meaning                          |
|--------|----------------------------------|
| `wNNN` | Width                            |
| `hNNN` | Height                           |
| `xNNN` | Horizontal (left) position       |
| `yNNN` | Vertical (top) position          |
| `v`    | Maximize vertically              |
| `m`    | Maximize (both directions)       |

Examples: `w60%x20%v` (60 % wide, 20 % from the left, vertically maximized),
`m` (fully maximized), `w1280h720x0y0` (a 1280×720 window in the top-left
corner).

### Behaviour & exit codes

- If the **currently active** window matches the search criteria, focus moves to
  the *next* matching window (so repeated invocations cycle through them).
- If **no** window matches and a `PROGRAM` was given, it is launched detached.
- If a matching process is already running but has no window, a warning is
  printed and the tool exits **127**.
- Unrecoverable errors (e.g. executable not found, KWin script not loaded) exit
  **1**. Success exits **0**.

## Examples

### Focus or start Dolphin

Bind this to a shortcut to toggle Dolphin into focus, launching it the first
time:

```shell
kwintool dolphin
```

- If a Dolphin window exists, it is activated (and repeated presses cycle
  through multiple Dolphin windows).
- If none exists, `dolphin` is launched.

Equivalent explicit form, plus passing an argument to the launched program:

```shell
kwintool --class '^org.kde.dolphin$' dolphin ~/Documents
```

### Move a specific window to another desktop (only if it exists)

With no `PROGRAM` argument nothing is ever launched — the command is a no-op
unless a matching window is found. This moves an existing window titled
"Meeting Notes" to desktop 3 and focuses it:

```shell
kwintool --title '^Meeting Notes' --to-desktop 3
```

Target by class instead, and send it to *all* desktops:

```shell
kwintool --class '^konsole$' --to-desktop -1
```

### Other handy combinations

```shell
# Focus the first JetBrains window that is NOT Fleet
kwintool --class '!fleet' --name jetbrains

# Tile Konsole to the left half of the external display, without stealing focus
kwintool --class '^konsole$' --to-screen 'DP-' --geometry 'w50%x0v' --id

# Just print the id of the focused-or-matched Dolphin window
kwintool --id dolphin
```

## License

MIT — see [LICENSE](LICENSE).
