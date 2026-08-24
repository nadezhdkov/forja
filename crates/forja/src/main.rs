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

    if let Command::Doctor = cli.command {
        let report = commands::doctor::run(cli.json);
        std::process::exit(if report.has_failure() { 3 } else { 0 });
    }

    let result = match cli.command {
        Command::Show => commands::show::run(&cli.config, cli.json),
        Command::Init { force } => commands::init::run(&cli.config, force),
        Command::Setup => commands::setup::run(&cli.config, cli.dry_run, cli.json),
        Command::Doctor => unreachable!("handled above"),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(err.exit_code());
    }
}
