mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    if let Err(message) = cli.validate_flag_combinations() {
        eprintln!("error: {message}");
        std::process::exit(2);
    }

    let result = match cli.command {
        Command::Show => commands::show::run(&cli.config, cli.json),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(err.exit_code());
    }
}
