//! Command line definition.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "tokenbox",
    version,
    about = "Store secrets encrypted in a git repo and retrieve them by git URL and key name"
)]
pub struct Cli {
    /// Git URL of the store. Required on every run; nothing is remembered.
    pub git_url: String,

    /// Print the result as JSON.
    #[arg(long, global = true)]
    pub json: bool,

    /// Print nothing except the requested value and errors.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Encrypt a value and put it in the store (replaces an existing value).
    Register {
        /// Name that identifies the secret. Whoever knows it can read the secret.
        key_name: String,

        /// Take the value from the clipboard.
        #[arg(long, conflicts_with = "stdin")]
        clipboard: bool,

        /// Take the value from standard input.
        #[arg(long)]
        stdin: bool,
    },
    /// Fetch a value from the store and print it to standard output.
    Get {
        /// Name that identifies the secret.
        key_name: String,

        /// Allow printing the value to a terminal.
        #[arg(long)]
        force_tty: bool,
    },
}
