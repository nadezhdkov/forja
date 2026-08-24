use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// forja — a CLI that collapses repetitive git/GitHub sequences into single,
/// safe, predictable commands.
#[derive(Debug, Parser)]
#[command(name = "forja", version, about)]
pub struct Cli {
    /// Path to the config file.
    #[arg(long, global = true, default_value = "forja.toml")]
    pub config: PathBuf,

    /// Show the execution plan without running anything.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Skip interactive confirmations.
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    /// Log every external command executed, with its arguments.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Suppress non-essential output.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Emit structured JSON output instead of prose.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Display the loaded, normalized config. Read-only.
    Show,

    /// Generate a commented forja.toml scaffold.
    Init {
        /// Overwrite the file if it already exists.
        #[arg(long)]
        force: bool,
    },

    /// Check that git and gh are installed and ready to use.
    Doctor,

    /// Apply the [git] section of the config via `git config --global`.
    Setup,
}

impl Cli {
    /// Flags that don't make sense together, per PRD §9.1: `--quiet` and
    /// `--verbose` are mutually exclusive (exit 2).
    pub fn validate_flag_combinations(&self) -> Result<(), String> {
        if self.verbose && self.quiet {
            return Err("--verbose and --quiet are mutually exclusive".to_string());
        }
        Ok(())
    }
}
