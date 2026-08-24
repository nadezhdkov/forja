use serde::Serialize;

use crate::config::GitConfig;
use crate::error::ForjaError;
use crate::exec::{CommandRequest, CommandRunner};

/// One `git config --global` key whose current value differs from what
/// `forja.toml` declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitConfigChange {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: String,
}

/// The set of changes needed to bring the machine's global git config in
/// line with the declared `[git]` section. Keys already conforming never
/// appear here (RF-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupPlan {
    pub changes: Vec<GitConfigChange>,
}

/// Result of applying a [`SetupPlan`]: the changes that succeeded, and —
/// if execution stopped early — the error that stopped it. Carrying both in
/// one struct (rather than `Result<Vec<_>, ForjaError>`) is what lets a
/// caller report "here's what was already applied before this failed"
/// (RF-11, DD-02) without losing either piece of information.
#[derive(Debug)]
pub struct ApplyOutcome {
    pub applied: Vec<GitConfigChange>,
    pub error: Option<ForjaError>,
}

/// Maps a declared `[git]` section onto the `git config` keys it controls.
/// `default_branch` always has a value once the config is loaded (it
/// defaults to `"main"`), so it is always considered; `editor` and
/// `pull_rebase` stay `Option` end to end and are only considered when the
/// user actually set them — matching "campos ausentes não são aplicados"
/// (PRD §8.2).
fn desired_entries(git: &GitConfig) -> Vec<(String, String)> {
    let mut entries = vec![
        ("user.name".to_string(), git.user_name.clone()),
        ("user.email".to_string(), git.user_email.clone()),
        ("init.defaultBranch".to_string(), git.default_branch.clone()),
    ];

    if let Some(editor) = &git.editor {
        entries.push(("core.editor".to_string(), editor.clone()));
    }
    if let Some(pull_rebase) = git.pull_rebase {
        entries.push(("pull.rebase".to_string(), pull_rebase.to_string()));
    }
    for (name, value) in &git.aliases {
        entries.push((format!("alias.{name}"), value.clone()));
    }

    entries
}

/// Reads the current global value of `key`, if any. `git config --get`
/// exits non-zero when the key is unset — that is a normal "no value" case
/// here, not a failure; only a spawn failure (e.g. `git` missing) is
/// propagated as an error.
fn read_current(runner: &dyn CommandRunner, key: &str) -> Result<Option<String>, ForjaError> {
    let outcome = runner.run(&CommandRequest::new(
        "git",
        ["config", "--global", "--get", key],
    ))?;

    if !outcome.success() {
        return Ok(None);
    }

    let value = outcome.stdout.trim();
    Ok(if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    })
}

/// Computes the diff between the declared `[git]` section and the current
/// machine state (DD-03: always diff before writing, dry-run or not).
pub fn compute_plan(git: &GitConfig, runner: &dyn CommandRunner) -> Result<SetupPlan, ForjaError> {
    let mut changes = Vec::new();

    for (key, new_value) in desired_entries(git) {
        let old_value = read_current(runner, &key)?;
        if old_value.as_deref() != Some(new_value.as_str()) {
            changes.push(GitConfigChange {
                key,
                old_value,
                new_value,
            });
        }
    }

    Ok(SetupPlan { changes })
}

/// Applies each change in order via `git config --global <key> <value>`
/// (DD-04: one process per key), stopping at the first failure (DD-02: no
/// rollback — partial state is acceptable, silent partial state is not).
pub fn apply_plan(plan: &SetupPlan, runner: &dyn CommandRunner) -> ApplyOutcome {
    let mut applied = Vec::new();

    for change in &plan.changes {
        let request = CommandRequest::new(
            "git",
            [
                "config",
                "--global",
                change.key.as_str(),
                change.new_value.as_str(),
            ],
        );

        match runner.run(&request) {
            Ok(outcome) if outcome.success() => applied.push(change.clone()),
            Ok(outcome) => {
                return ApplyOutcome {
                    applied,
                    error: Some(ForjaError::CommandFailed {
                        command: format!("git config --global {} {}", change.key, change.new_value),
                        stderr: outcome.stderr,
                        exit_code: outcome.status_code,
                    }),
                };
            }
            Err(err) => {
                return ApplyOutcome {
                    applied,
                    error: Some(err),
                }
            }
        }
    }

    ApplyOutcome {
        applied,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::CommandOutcome;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, HashMap};

    struct FakeRunner {
        /// Maps a `git config --global --get <key>` lookup to its current value.
        current: HashMap<String, String>,
        /// Programmed failures for `git config --global <key> <value>`, by key.
        write_failures: HashMap<String, String>,
        writes: RefCell<Vec<(String, String)>>,
    }

    impl FakeRunner {
        fn new() -> Self {
            Self {
                current: HashMap::new(),
                write_failures: HashMap::new(),
                writes: RefCell::new(Vec::new()),
            }
        }

        fn with_current(mut self, key: &str, value: &str) -> Self {
            self.current.insert(key.to_string(), value.to_string());
            self
        }

        fn failing_write(mut self, key: &str, stderr: &str) -> Self {
            self.write_failures
                .insert(key.to_string(), stderr.to_string());
            self
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, request: &CommandRequest) -> Result<CommandOutcome, ForjaError> {
            match request.args.first().map(String::as_str) {
                Some("config") if request.args.get(2).map(String::as_str) == Some("--get") => {
                    let key = &request.args[3];
                    match self.current.get(key) {
                        Some(value) => Ok(CommandOutcome {
                            status_code: Some(0),
                            stdout: format!("{value}\n"),
                            stderr: String::new(),
                        }),
                        None => Ok(CommandOutcome {
                            status_code: Some(1),
                            stdout: String::new(),
                            stderr: String::new(),
                        }),
                    }
                }
                Some("config") => {
                    let key = &request.args[2];
                    let value = &request.args[3];
                    self.writes.borrow_mut().push((key.clone(), value.clone()));
                    match self.write_failures.get(key) {
                        Some(stderr) => Ok(CommandOutcome {
                            status_code: Some(1),
                            stdout: String::new(),
                            stderr: stderr.clone(),
                        }),
                        None => Ok(CommandOutcome {
                            status_code: Some(0),
                            stdout: String::new(),
                            stderr: String::new(),
                        }),
                    }
                }
                _ => panic!("unexpected command in test: {request:?}"),
            }
        }
    }

    fn sample_git() -> GitConfig {
        GitConfig {
            user_name: "Ada".to_string(),
            user_email: "ada@example.com".to_string(),
            default_branch: "main".to_string(),
            editor: None,
            pull_rebase: None,
            aliases: BTreeMap::new(),
        }
    }

    #[test]
    fn everything_already_conforming_produces_an_empty_plan() {
        let runner = FakeRunner::new()
            .with_current("user.name", "Ada")
            .with_current("user.email", "ada@example.com")
            .with_current("init.defaultBranch", "main");

        let plan = compute_plan(&sample_git(), &runner).expect("compute_plan should succeed");
        assert!(plan.changes.is_empty());
    }

    #[test]
    fn a_divergent_field_produces_one_change() {
        let runner = FakeRunner::new()
            .with_current("user.name", "Old Name")
            .with_current("user.email", "ada@example.com")
            .with_current("init.defaultBranch", "main");

        let plan = compute_plan(&sample_git(), &runner).expect("compute_plan should succeed");
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].key, "user.name");
        assert_eq!(plan.changes[0].old_value, Some("Old Name".to_string()));
        assert_eq!(plan.changes[0].new_value, "Ada");
    }

    #[test]
    fn absent_optional_fields_never_appear_in_the_plan() {
        let mut git = sample_git();
        git.editor = None;
        git.pull_rebase = None;

        let runner = FakeRunner::new()
            .with_current("user.name", "Ada")
            .with_current("user.email", "ada@example.com")
            .with_current("init.defaultBranch", "main");

        let plan = compute_plan(&git, &runner).expect("compute_plan should succeed");
        assert!(!plan.changes.iter().any(|c| c.key == "core.editor"));
        assert!(!plan.changes.iter().any(|c| c.key == "pull.rebase"));
    }

    #[test]
    fn apply_stops_at_first_failure_and_reports_what_succeeded() {
        let plan = SetupPlan {
            changes: vec![
                GitConfigChange {
                    key: "user.name".to_string(),
                    old_value: None,
                    new_value: "Ada".to_string(),
                },
                GitConfigChange {
                    key: "user.email".to_string(),
                    old_value: None,
                    new_value: "ada@example.com".to_string(),
                },
                GitConfigChange {
                    key: "init.defaultBranch".to_string(),
                    old_value: None,
                    new_value: "main".to_string(),
                },
            ],
        };
        let runner = FakeRunner::new().failing_write("user.email", "permission denied");

        let outcome = apply_plan(&plan, &runner);
        assert_eq!(outcome.applied.len(), 1);
        assert_eq!(outcome.applied[0].key, "user.name");
        assert!(matches!(
            outcome.error,
            Some(ForjaError::CommandFailed { .. })
        ));
    }

    #[test]
    fn apply_succeeds_when_every_write_succeeds() {
        let plan = SetupPlan {
            changes: vec![GitConfigChange {
                key: "user.name".to_string(),
                old_value: None,
                new_value: "Ada".to_string(),
            }],
        };
        let runner = FakeRunner::new();

        let outcome = apply_plan(&plan, &runner);
        assert_eq!(outcome.applied.len(), 1);
        assert!(outcome.error.is_none());
    }
}
