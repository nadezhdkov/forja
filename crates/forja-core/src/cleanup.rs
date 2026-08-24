use serde::Serialize;

use crate::config::FlowConfig;
use crate::error::ForjaError;
use crate::exec::{CommandRequest, CommandRunner};
use crate::sync::{current_branch, detect_base_branch, ensure_git_repo};

/// Local branches eligible for deletion: merged into the base branch *and*
/// with an upstream that's confirmed gone (PRD §2's "already merged,
/// already deleted on the remote" scenario) — never the current branch,
/// the base branch itself, or anything in `protected_branches`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupPlan {
    pub candidates: Vec<String>,
}

/// Result of running [`delete_branches`]: what was deleted, and — if
/// deletion stopped early — the error that stopped it (same shape as
/// `setup::ApplyOutcome`, for the same reason: report partial progress).
#[derive(Debug)]
pub struct DeleteOutcome {
    pub deleted: Vec<String>,
    pub error: Option<ForjaError>,
}

fn prune(runner: &dyn CommandRunner) -> Result<(), ForjaError> {
    let outcome = runner.run(&CommandRequest::new("git", ["fetch", "--prune", "origin"]))?;
    if !outcome.success() {
        return Err(ForjaError::CommandFailed {
            command: "git fetch --prune origin".to_string(),
            stderr: outcome.stderr,
            exit_code: outcome.status_code,
        });
    }
    Ok(())
}

fn list_merged_branches(
    base_ref: &str,
    runner: &dyn CommandRunner,
) -> Result<Vec<String>, ForjaError> {
    let outcome = runner.run(&CommandRequest::new(
        "git",
        ["branch", "--merged", base_ref, "--format=%(refname:short)"],
    ))?;
    if !outcome.success() {
        return Err(ForjaError::CommandFailed {
            command: format!("git branch --merged {base_ref}"),
            stderr: outcome.stderr,
            exit_code: outcome.status_code,
        });
    }

    Ok(outcome
        .stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

/// Local branches whose upstream `git` has confirmed is gone (shows as
/// `[gone]` in `%(upstream:track)`) — i.e. branches that *had* a remote
/// counterpart which was since deleted, not branches that were simply
/// never pushed.
fn list_gone_branches(runner: &dyn CommandRunner) -> Result<Vec<String>, ForjaError> {
    let outcome = runner.run(&CommandRequest::new(
        "git",
        [
            "for-each-ref",
            "--format=%(refname:short)\t%(upstream:track)",
            "refs/heads",
        ],
    ))?;
    if !outcome.success() {
        return Err(ForjaError::CommandFailed {
            command: "git for-each-ref refs/heads".to_string(),
            stderr: outcome.stderr,
            exit_code: outcome.status_code,
        });
    }

    Ok(outcome
        .stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let name = parts.next()?.trim();
            let track = parts.next().unwrap_or("");
            track.contains("[gone]").then(|| name.to_string())
        })
        .collect())
}

/// Computes the cleanup candidates (RF-10). Runs `git fetch --prune`
/// itself so the "gone" check reflects the remote's current state.
pub fn plan_cleanup(
    flow: &FlowConfig,
    runner: &dyn CommandRunner,
) -> Result<CleanupPlan, ForjaError> {
    ensure_git_repo(runner)?;
    let current = current_branch(runner)?;
    let base = detect_base_branch(flow, runner)?;
    let base_ref = format!("origin/{base}");

    prune(runner)?;

    let merged = list_merged_branches(&base_ref, runner)?;
    let gone = list_gone_branches(runner)?;

    let candidates = merged
        .into_iter()
        .filter(|branch| gone.contains(branch))
        .filter(|branch| branch != &current)
        .filter(|branch| branch != &base)
        .filter(|branch| !flow.protected_branches.contains(branch))
        .collect();

    Ok(CleanupPlan { candidates })
}

/// Deletes each candidate with `git branch -d` (never `-D`) — a safe
/// delete refuses on an unmerged branch, which is a second, independent
/// enforcement of DD-08's "never delete an unmerged branch" beyond our own
/// `--merged` filter above. Stops at the first failure (DD-02 pattern).
pub fn delete_branches(candidates: &[String], runner: &dyn CommandRunner) -> DeleteOutcome {
    let mut deleted = Vec::new();

    for branch in candidates {
        match runner.run(&CommandRequest::new(
            "git",
            ["branch", "-d", branch.as_str()],
        )) {
            Ok(outcome) if outcome.success() => deleted.push(branch.clone()),
            Ok(outcome) => {
                return DeleteOutcome {
                    deleted,
                    error: Some(ForjaError::CommandFailed {
                        command: format!("git branch -d {branch}"),
                        stderr: outcome.stderr,
                        exit_code: outcome.status_code,
                    }),
                };
            }
            Err(err) => {
                return DeleteOutcome {
                    deleted,
                    error: Some(err),
                }
            }
        }
    }

    DeleteOutcome {
        deleted,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::CommandOutcome;
    use std::collections::HashMap;

    type FakeResponse = (bool, String, String);

    #[derive(Default)]
    struct FakeRunner {
        responses: HashMap<(String, Vec<String>), FakeResponse>,
    }

    impl FakeRunner {
        fn with(
            mut self,
            program: &str,
            args: &[&str],
            success: bool,
            stdout: &str,
            stderr: &str,
        ) -> Self {
            self.responses.insert(
                (
                    program.to_string(),
                    args.iter().map(|s| s.to_string()).collect(),
                ),
                (success, stdout.to_string(), stderr.to_string()),
            );
            self
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, request: &CommandRequest) -> Result<CommandOutcome, ForjaError> {
            match self
                .responses
                .get(&(request.program.clone(), request.args.clone()))
            {
                Some((success, stdout, stderr)) => Ok(CommandOutcome {
                    status_code: Some(if *success { 0 } else { 1 }),
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                }),
                None => Ok(CommandOutcome {
                    status_code: Some(1),
                    stdout: String::new(),
                    stderr: String::new(),
                }),
            }
        }
    }

    fn base_runner() -> FakeRunner {
        FakeRunner::default()
            .with(
                "git",
                &["rev-parse", "--is-inside-work-tree"],
                true,
                "true\n",
                "",
            )
            .with(
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
                true,
                "main\n",
                "",
            )
            .with(
                "git",
                &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
                true,
                "origin/main\n",
                "",
            )
            .with("git", &["fetch", "--prune", "origin"], true, "", "")
    }

    fn default_flow() -> FlowConfig {
        FlowConfig::default()
    }

    #[test]
    fn merged_and_gone_branch_not_protected_is_a_candidate() {
        let runner = base_runner()
            .with(
                "git",
                &[
                    "branch",
                    "--merged",
                    "origin/main",
                    "--format=%(refname:short)",
                ],
                true,
                "main\nfeature/done\n",
                "",
            )
            .with(
                "git",
                &[
                    "for-each-ref",
                    "--format=%(refname:short)\t%(upstream:track)",
                    "refs/heads",
                ],
                true,
                "main\t\nfeature/done\t[gone]\n",
                "",
            );

        let plan = plan_cleanup(&default_flow(), &runner).expect("plan_cleanup should succeed");
        assert_eq!(plan.candidates, vec!["feature/done".to_string()]);
    }

    #[test]
    fn merged_but_still_on_remote_is_excluded() {
        let runner = base_runner()
            .with(
                "git",
                &[
                    "branch",
                    "--merged",
                    "origin/main",
                    "--format=%(refname:short)",
                ],
                true,
                "main\nfeature/still-remote\n",
                "",
            )
            .with(
                "git",
                &[
                    "for-each-ref",
                    "--format=%(refname:short)\t%(upstream:track)",
                    "refs/heads",
                ],
                true,
                "main\t\nfeature/still-remote\t\n",
                "",
            );

        let plan = plan_cleanup(&default_flow(), &runner).expect("plan_cleanup should succeed");
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn gone_but_not_merged_is_excluded() {
        let runner = base_runner()
            .with(
                "git",
                &[
                    "branch",
                    "--merged",
                    "origin/main",
                    "--format=%(refname:short)",
                ],
                true,
                "main\n",
                "",
            )
            .with(
                "git",
                &[
                    "for-each-ref",
                    "--format=%(refname:short)\t%(upstream:track)",
                    "refs/heads",
                ],
                true,
                "main\t\nfeature/unmerged\t[gone]\n",
                "",
            );

        let plan = plan_cleanup(&default_flow(), &runner).expect("plan_cleanup should succeed");
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn protected_branch_is_never_a_candidate_even_if_merged_and_gone() {
        let mut flow = default_flow();
        flow.protected_branches = vec!["main".to_string(), "develop".to_string()];

        let runner = base_runner()
            .with(
                "git",
                &[
                    "branch",
                    "--merged",
                    "origin/main",
                    "--format=%(refname:short)",
                ],
                true,
                "main\ndevelop\n",
                "",
            )
            .with(
                "git",
                &[
                    "for-each-ref",
                    "--format=%(refname:short)\t%(upstream:track)",
                    "refs/heads",
                ],
                true,
                "main\t\ndevelop\t[gone]\n",
                "",
            );

        let plan = plan_cleanup(&flow, &runner).expect("plan_cleanup should succeed");
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn current_branch_is_never_a_candidate() {
        let runner = base_runner()
            .with(
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
                true,
                "feature/done\n",
                "",
            )
            .with(
                "git",
                &[
                    "branch",
                    "--merged",
                    "origin/main",
                    "--format=%(refname:short)",
                ],
                true,
                "main\nfeature/done\n",
                "",
            )
            .with(
                "git",
                &[
                    "for-each-ref",
                    "--format=%(refname:short)\t%(upstream:track)",
                    "refs/heads",
                ],
                true,
                "main\t\nfeature/done\t[gone]\n",
                "",
            );

        let plan = plan_cleanup(&default_flow(), &runner).expect("plan_cleanup should succeed");
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn delete_stops_at_first_failure_and_reports_progress() {
        let runner = FakeRunner::default()
            .with("git", &["branch", "-d", "feature/a"], true, "", "")
            .with(
                "git",
                &["branch", "-d", "feature/b"],
                false,
                "",
                "error: the branch is not fully merged\n",
            );

        let outcome = delete_branches(
            &[
                "feature/a".to_string(),
                "feature/b".to_string(),
                "feature/c".to_string(),
            ],
            &runner,
        );
        assert_eq!(outcome.deleted, vec!["feature/a".to_string()]);
        assert!(matches!(
            outcome.error,
            Some(ForjaError::CommandFailed { .. })
        ));
    }
}
