use std::path::Path;

use forja_core::{
    execute_sync, load_flow_config, plan_sync, ForjaError, Strategy, SyncPlan, SystemCommandRunner,
};

/// `forja sync` — fetch, integrate onto the base branch, and push (RF-09).
pub fn run(config_path: &Path, dry_run: bool, json: bool) -> Result<(), ForjaError> {
    let (flow, warnings) = load_flow_config(config_path)?;
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }

    let runner = SystemCommandRunner;
    let plan = plan_sync(&flow, &runner)?;
    print_plan(&plan, json);

    if dry_run {
        return Ok(());
    }

    let outcome = execute_sync(&plan, &runner)?;

    if outcome.pushed {
        println!(
            "{} synced with origin/{} and pushed.",
            outcome.current_branch, outcome.base_branch
        );
    } else {
        println!(
            "{} synced with origin/{} (not pushed — auto_push is disabled).",
            outcome.current_branch, outcome.base_branch
        );
    }

    Ok(())
}

fn print_plan(plan: &SyncPlan, json: bool) {
    if json {
        let rendered =
            serde_json::to_string_pretty(plan).expect("SyncPlan serialization cannot fail");
        println!("{rendered}");
        return;
    }

    println!("Current branch: {}", plan.current_branch);
    println!("Base:           origin/{}", plan.base_branch);
    println!();
    println!("  ✓ working tree is clean");
    println!("  ✓ branch is not protected");

    let integrate_verb = match plan.strategy {
        Strategy::Rebase => "rebase",
        Strategy::Merge => "merge",
    };
    println!("  → git fetch origin");
    println!("  → git {integrate_verb} origin/{}", plan.base_branch);
    if plan.will_push {
        println!(
            "  → git push --force-with-lease origin {}",
            plan.current_branch
        );
    }
}
