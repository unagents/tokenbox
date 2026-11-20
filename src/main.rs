mod cli;
mod crypto;
mod error;
mod input;
mod store;

use std::io::{IsTerminal, Write};

use clap::Parser;
use secrecy::ExposeSecret;

use cli::{Cli, Command};
use error::{Failure, Result};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // --help and --version are not errors; everything else is a usage error.
            let code = if e.use_stderr() { 1 } else { 0 };
            let _ = e.print();
            std::process::exit(code);
        }
    };
    if let Err(failure) = run(&cli) {
        eprintln!("tokenbox: {failure}");
        std::process::exit(failure.exit_code());
    }
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Register { key_name, clipboard, stdin } => {
            let source = if *clipboard {
                input::Source::Clipboard
            } else if *stdin {
                input::Source::Stdin
            } else {
                input::Source::Hidden
            };
            register(cli, key_name, source)
        }
        Command::Get { key_name, force_tty } => get(cli, key_name, *force_tty),
    }
}

fn store_path(file_id: &str) -> String {
    format!("{}/{}", crypto::FORMAT_DIR, file_id)
}

fn register(cli: &Cli, key_name: &str, source: input::Source) -> Result<()> {
    let value = input::read_value(source)?;
    let derived = crypto::derive(key_name)?;
    let data = derived.encrypt(&value)?;
    let commit = store::write_file(&cli.git_url, &store_path(&derived.file_id), &data)?;
    if cli.json {
        println!("{{\"status\":\"registered\",\"commit\":\"{commit}\"}}");
    } else if !cli.quiet {
        eprintln!("registered (commit {})", &commit[..12.min(commit.len())]);
    }
    Ok(())
}

fn get(cli: &Cli, key_name: &str, force_tty: bool) -> Result<()> {
    if std::io::stdout().is_terminal() && !force_tty {
        return Err(Failure::Usage(
            "refusing to print a secret to a terminal; pipe the output or pass --force-tty".into(),
        ));
    }
    let derived = crypto::derive(key_name)?;
    let data = store::read_file(&cli.git_url, &store_path(&derived.file_id))?.ok_or_else(|| {
        Failure::Decrypt("no secret for this key name: the name is wrong or nothing is registered".into())
    })?;
    let value = derived.decrypt(&data)?;
    let mut out = std::io::stdout().lock();
    let written = if cli.json {
        writeln!(out, "{{\"value\":{}}}", json_string(value.expose_secret()))
    } else {
        writeln!(out, "{}", value.expose_secret())
    };
    written.map_err(|e| Failure::Usage(format!("cannot write the value: {e}")))
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::json_string;

    #[test]
    fn json_escapes_special_characters() {
        assert_eq!(json_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
    }
}
