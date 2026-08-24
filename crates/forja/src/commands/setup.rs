use std::path::{Path, PathBuf};

use forja_core::{
    apply_plan, compute_plan, load_config, ForjaError, SetupPlan, SystemCommandRunner,
};

/// `forja setup` — apply `[git]`/`[git.aliases]` via `git config --global`
/// (RF-06, RF-07, RF-12).
pub fn run(config_path: &Path, dry_run: bool, json: bool) -> Result<(), ForjaError> {
    let outcome = load_config(config_path)?;
    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }

    let Some(git) = &outcome.config.git else {
        println!("no [git] section in config — nothing to apply");
        return Ok(());
    };

    let runner = SystemCommandRunner;
    let plan = compute_plan(git, &runner)?;

    if plan.changes.is_empty() {
        println!("git config already matches forja.toml — no changes needed");
        return Ok(());
    }

    print_plan(&plan, json);

    if dry_run {
        return Ok(());
    }

    backup_gitconfig()?;

    let result = apply_plan(&plan, &runner);
    println!(
        "applied {} of {} change(s)",
        result.applied.len(),
        plan.changes.len()
    );

    match result.error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn print_plan(plan: &SetupPlan, json: bool) {
    if json {
        let rendered =
            serde_json::to_string_pretty(plan).expect("SetupPlan serialization cannot fail");
        println!("{rendered}");
        return;
    }

    for change in &plan.changes {
        let old = change.old_value.as_deref().unwrap_or("(unset)");
        println!("  {} : {} -> {}", change.key, old, change.new_value);
    }
}

/// Resolves the global gitconfig file that `git config --global` will
/// actually write to — honoring `GIT_CONFIG_GLOBAL` (used by tests, per PRD
/// §13) before falling back to `~/.gitconfig`, so the backup always targets
/// the file that's really about to change.
fn global_gitconfig_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GIT_CONFIG_GLOBAL") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".gitconfig"))
}

/// Backs up the global gitconfig before the first write of this execution
/// (PRD §15). A missing gitconfig (nothing configured yet) is not an error.
fn backup_gitconfig() -> Result<(), ForjaError> {
    let Some(path) = global_gitconfig_path() else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }

    let mut backup_name = path.file_name().unwrap_or_default().to_os_string();
    backup_name.push(".forja.bak");
    let backup_path = path.with_file_name(backup_name);

    std::fs::copy(&path, &backup_path).map_err(|source| ForjaError::Io { path, source })?;
    Ok(())
}
