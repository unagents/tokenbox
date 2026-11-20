//! Reads the value for `register` (SPEC §3): hidden prompt, clipboard or stdin.

use std::io::{IsTerminal, Read};
use std::process::Command;

use secrecy::SecretString;

use crate::error::{Failure, Result};

pub enum Source {
    Hidden,
    Clipboard,
    Stdin,
}

pub fn read_value(source: Source) -> Result<SecretString> {
    let raw = match source {
        Source::Hidden => read_hidden()?,
        Source::Clipboard => read_clipboard()?,
        Source::Stdin => read_stdin()?,
    };
    let value = strip_one_newline(raw);
    if value.is_empty() {
        return Err(Failure::Usage("the value is empty".into()));
    }
    Ok(SecretString::from(value))
}

fn read_hidden() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return Err(Failure::Usage(
            "no terminal for hidden input; use --stdin or --clipboard".into(),
        ));
    }
    rpassword::prompt_password("value (hidden): ")
        .map_err(|e| Failure::Usage(format!("cannot read hidden input: {e}")))
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| Failure::Usage(format!("cannot read stdin: {e}")))?;
    Ok(buf)
}

/// Clipboard readers tried in order. The first one that runs successfully wins.
#[cfg(target_os = "macos")]
const CLIPBOARD_COMMANDS: &[&[&str]] = &[&["pbpaste"]];
#[cfg(target_os = "windows")]
const CLIPBOARD_COMMANDS: &[&[&str]] = &[&["powershell", "-NoProfile", "-Command", "Get-Clipboard"]];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const CLIPBOARD_COMMANDS: &[&[&str]] = &[
    &["wl-paste", "--no-newline"],
    &["xclip", "-o", "-selection", "clipboard"],
    &["xsel", "--clipboard", "--output"],
];

fn read_clipboard() -> Result<String> {
    for cmd in CLIPBOARD_COMMANDS {
        let Ok(out) = Command::new(cmd[0]).args(&cmd[1..]).output() else {
            continue;
        };
        if out.status.success() {
            return String::from_utf8(out.stdout)
                .map_err(|_| Failure::Usage("the clipboard does not hold text".into()));
        }
    }
    let names: Vec<&str> = CLIPBOARD_COMMANDS.iter().map(|c| c[0]).collect();
    Err(Failure::Usage(format!(
        "cannot read the clipboard (tried: {}); use --stdin instead",
        names.join(", ")
    )))
}

fn strip_one_newline(mut s: String) -> String {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::strip_one_newline;

    #[test]
    fn strips_exactly_one_trailing_newline() {
        assert_eq!(strip_one_newline("abc\n".into()), "abc");
        assert_eq!(strip_one_newline("abc\r\n".into()), "abc");
        assert_eq!(strip_one_newline("abc\n\n".into()), "abc\n");
        assert_eq!(strip_one_newline("abc".into()), "abc");
    }
}
