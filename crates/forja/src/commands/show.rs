use std::path::Path;

use forja_core::{load_config, ForjaConfig, ForjaError, Strategy};

/// `forja show` — load and display the normalized config. Read-only: it
/// never mutates anything and never invokes an external command (RF-04).
pub fn run(config_path: &Path, json: bool) -> Result<(), ForjaError> {
    let outcome = load_config(config_path)?;

    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }

    if json {
        let rendered =
            serde_json::to_string_pretty(&outcome.config).expect("ForjaConfig serialization cannot fail");
        println!("{rendered}");
    } else {
        print_human(&outcome.config);
    }

    Ok(())
}

fn print_human(config: &ForjaConfig) {
    println!("version: {}", config.version);
    println!();

    match &config.git {
        Some(git) => {
            println!("[git]");
            println!("  user_name      = {}", git.user_name);
            println!("  user_email     = {}", git.user_email);
            println!("  default_branch = {}", git.default_branch);
            if let Some(editor) = &git.editor {
                println!("  editor         = {editor}");
            }
            if let Some(pull_rebase) = git.pull_rebase {
                println!("  pull_rebase    = {pull_rebase}");
            }

            if !git.aliases.is_empty() {
                println!();
                println!("  [git.aliases]");
                for (name, value) in &git.aliases {
                    println!("    {name} = \"{value}\"");
                }
            }
        }
        None => println!("[git]  (not set)"),
    }

    println!();
    println!("[flow]");
    let strategy = match config.flow.strategy {
        Strategy::Rebase => "rebase",
        Strategy::Merge => "merge",
    };
    println!("  strategy           = {strategy}");
    println!("  auto_push          = {}", config.flow.auto_push);
    println!(
        "  base_branch        = {}",
        config.flow.base_branch.as_deref().unwrap_or("(detected from remote)")
    );
    println!(
        "  protected_branches = [{}]",
        config
            .flow
            .protected_branches
            .iter()
            .map(|b| format!("\"{b}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
