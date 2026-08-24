use std::io::{self, BufRead, Write};
use std::path::Path;

use forja_core::{
    delete_branches, load_flow_config, plan_cleanup, CleanupPlan, ForjaError, SystemCommandRunner,
};

/// `forja cleanup` — delete local branches already merged and deleted on
/// the remote (RF-10).
pub fn run(config_path: &Path, dry_run: bool, yes: bool, json: bool) -> Result<(), ForjaError> {
    let (flow, warnings) = load_flow_config(config_path)?;
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }

    let runner = SystemCommandRunner;
    let plan = plan_cleanup(&flow, &runner)?;

    if plan.candidates.is_empty() {
        println!("no branches to clean up");
        return Ok(());
    }

    print_plan(&plan, json);

    if dry_run {
        return Ok(());
    }

    if !yes && !confirm() {
        println!("aborted, nothing deleted");
        return Ok(());
    }

    let outcome = delete_branches(&plan.candidates, &runner);
    println!(
        "deleted {} of {} branch(es)",
        outcome.deleted.len(),
        plan.candidates.len()
    );

    match outcome.error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn print_plan(plan: &CleanupPlan, json: bool) {
    if json {
        let rendered =
            serde_json::to_string_pretty(plan).expect("CleanupPlan serialization cannot fail");
        println!("{rendered}");
        return;
    }

    println!("branches to delete (merged and removed on the remote):");
    for branch in &plan.candidates {
        println!("  - {branch}");
    }
}

fn confirm() -> bool {
    print!("delete these branches? [y/N] ");
    if io::stdout().flush().is_err() {
        return false;
    }

    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }

    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}
