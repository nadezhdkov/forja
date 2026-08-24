use serde::Serialize;

use crate::config::{FlowConfig, Strategy};
use crate::error::ForjaError;
use crate::exec::{CommandRequest, CommandRunner};

/// The result of the read-only checks RF-09 requires before `sync` touches
/// anything. `base_branch` is always a plain name (`"main"`, never
/// `"origin/main"`) — see [`detect_base_branch`] for why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncPlan {
    pub current_branch: String,
    pub base_branch: String,
    pub strategy: Strategy,
    pub will_push: bool,
}

/// What actually happened once [`execute_sync`] ran the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub base_branch: String,
    pub current_branch: String,
    pub pushed: bool,
}

pub(crate) fn ensure_git_repo(runner: &dyn CommandRunner) -> Result<(), ForjaError> {
    let outcome = runner.run(&CommandRequest::new(
        "git",
        ["rev-parse", "--is-inside-work-tree"],
    ))?;
    if !outcome.success() || outcome.stdout.trim() != "true" {
        return Err(ForjaError::NotAGitRepository);
    }
    Ok(())
}

fn ensure_clean_tree(runner: &dyn CommandRunner) -> Result<(), ForjaError> {
    let outcome = runner.run(&CommandRequest::new("git", ["status", "--porcelain"]))?;
    let files: Vec<String> = outcome
        .stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if !files.is_empty() {
        return Err(ForjaError::DirtyWorkingTree { files });
    }
    Ok(())
}

pub(crate) fn current_branch(runner: &dyn CommandRunner) -> Result<String, ForjaError> {
    let outcome = runner.run(&CommandRequest::new(
        "git",
        ["rev-parse", "--abbrev-ref", "HEAD"],
    ))?;
    if !outcome.success() {
        return Err(ForjaError::NotAGitRepository);
    }
    Ok(outcome.stdout.trim().to_string())
}

fn ensure_not_protected(branch: &str, protected: &[String]) -> Result<(), ForjaError> {
    if protected.iter().any(|p| p == branch) {
        return Err(ForjaError::ProtectedBranch {
            branch: branch.to_string(),
        });
    }
    Ok(())
}

/// Resolves the base branch as a plain name. An explicit `flow.base_branch`
/// always wins; otherwise this reads the remote's own default branch via
/// `origin/HEAD` (purely local — no `gh`, no network beyond what a prior
/// `fetch`/`clone` already set up) and strips the `origin/` prefix, so the
/// rest of the code never has to care which source a name came from.
pub(crate) fn detect_base_branch(
    flow: &FlowConfig,
    runner: &dyn CommandRunner,
) -> Result<String, ForjaError> {
    if let Some(explicit) = &flow.base_branch {
        return Ok(explicit.clone());
    }

    let outcome = runner.run(&CommandRequest::new(
        "git",
        ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ))?;
    if !outcome.success() {
        return Err(ForjaError::BaseBranchNotDetected);
    }

    let short = outcome.stdout.trim();
    Ok(short.strip_prefix("origin/").unwrap_or(short).to_string())
}

/// Runs every RF-09 precondition, in order, without changing anything. Any
/// failure here means `sync` stops before touching the repository at all.
pub fn plan_sync(flow: &FlowConfig, runner: &dyn CommandRunner) -> Result<SyncPlan, ForjaError> {
    ensure_git_repo(runner)?;
    ensure_clean_tree(runner)?;

    let current_branch = current_branch(runner)?;
    ensure_not_protected(&current_branch, &flow.protected_branches)?;

    let base_branch = detect_base_branch(flow, runner)?;

    Ok(SyncPlan {
        current_branch,
        base_branch,
        strategy: flow.strategy,
        will_push: flow.auto_push,
    })
}

fn looks_like_a_conflict(stdout: &str, stderr: &str) -> bool {
    stdout.contains("CONFLICT") || stderr.contains("CONFLICT")
}

/// Fetches, integrates onto the base branch, and pushes if the plan calls
/// for it. Never resolves a conflict itself (DD-08): on conflict, this
/// stops and leaves the repository exactly where git left it.
pub fn execute_sync(
    plan: &SyncPlan,
    runner: &dyn CommandRunner,
) -> Result<SyncOutcome, ForjaError> {
    let fetch = runner.run(&CommandRequest::new("git", ["fetch", "origin"]))?;
    if !fetch.success() {
        return Err(ForjaError::CommandFailed {
            command: "git fetch origin".to_string(),
            stderr: fetch.stderr,
            exit_code: fetch.status_code,
        });
    }

    let base_ref = format!("origin/{}", plan.base_branch);
    let integrate_subcommand = match plan.strategy {
        Strategy::Rebase => "rebase",
        Strategy::Merge => "merge",
    };
    let integrate = runner.run(&CommandRequest::new(
        "git",
        [integrate_subcommand, base_ref.as_str()],
    ))?;
    if !integrate.success() {
        if looks_like_a_conflict(&integrate.stdout, &integrate.stderr) {
            return Err(ForjaError::RebaseConflict { base: base_ref });
        }
        return Err(ForjaError::CommandFailed {
            command: format!("git {integrate_subcommand} {base_ref}"),
            stderr: integrate.stderr,
            exit_code: integrate.status_code,
        });
    }

    if plan.will_push {
        let push = runner.run(&CommandRequest::new(
            "git",
            [
                "push",
                "--force-with-lease",
                "origin",
                plan.current_branch.as_str(),
            ],
        ))?;
        if !push.success() {
            return Err(ForjaError::CommandFailed {
                command: format!("git push --force-with-lease origin {}", plan.current_branch),
                stderr: push.stderr,
                exit_code: push.status_code,
            });
        }
    }

    Ok(SyncOutcome {
        base_branch: plan.base_branch.clone(),
        current_branch: plan.current_branch.clone(),
        pushed: plan.will_push,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::CommandOutcome;
    use std::cell::RefCell;
    use std::collections::HashMap;

    type FakeResponse = (bool, String, String);

    #[derive(Default)]
    struct FakeRunner {
        responses: HashMap<(String, Vec<String>), FakeResponse>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
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

        fn called(&self, program: &str, args: &[&str]) -> bool {
            self.calls.borrow().iter().any(|(p, a)| {
                p == program && a == &args.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            })
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, request: &CommandRequest) -> Result<CommandOutcome, ForjaError> {
            self.calls
                .borrow_mut()
                .push((request.program.clone(), request.args.clone()));

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

    fn clean_repo_runner() -> FakeRunner {
        FakeRunner::default()
            .with(
                "git",
                &["rev-parse", "--is-inside-work-tree"],
                true,
                "true\n",
                "",
            )
            .with("git", &["status", "--porcelain"], true, "", "")
            .with(
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
                true,
                "feature/login\n",
                "",
            )
            .with(
                "git",
                &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
                true,
                "origin/main\n",
                "",
            )
    }

    fn default_flow() -> FlowConfig {
        FlowConfig::default()
    }

    #[test]
    fn dirty_tree_aborts_before_anything_else() {
        let runner =
            clean_repo_runner().with("git", &["status", "--porcelain"], true, " M file.rs\n", "");
        let err = plan_sync(&default_flow(), &runner).unwrap_err();
        assert!(matches!(err, ForjaError::DirtyWorkingTree { .. }));
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn protected_branch_aborts() {
        let runner = clean_repo_runner().with(
            "git",
            &["rev-parse", "--abbrev-ref", "HEAD"],
            true,
            "main\n",
            "",
        );
        let err = plan_sync(&default_flow(), &runner).unwrap_err();
        assert!(matches!(err, ForjaError::ProtectedBranch { branch } if branch == "main"));
    }

    #[test]
    fn missing_origin_head_and_no_explicit_base_fails_detection() {
        let runner = clean_repo_runner().with(
            "git",
            &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
            false,
            "",
            "fatal: ref refs/remotes/origin/HEAD is not a symbolic ref\n",
        );
        let err = plan_sync(&default_flow(), &runner).unwrap_err();
        assert!(matches!(err, ForjaError::BaseBranchNotDetected));
    }

    #[test]
    fn explicit_base_branch_wins_over_detection() {
        let runner = clean_repo_runner();
        let mut flow = default_flow();
        flow.base_branch = Some("develop".to_string());

        let plan = plan_sync(&flow, &runner).expect("plan_sync should succeed");
        assert_eq!(plan.base_branch, "develop");
        assert!(!runner.called(
            "git",
            &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]
        ));
    }

    #[test]
    fn happy_path_plan_detects_base_from_origin_head() {
        let runner = clean_repo_runner();
        let plan = plan_sync(&default_flow(), &runner).expect("plan_sync should succeed");
        assert_eq!(plan.current_branch, "feature/login");
        assert_eq!(plan.base_branch, "main");
        assert!(plan.will_push);
    }

    fn sample_plan() -> SyncPlan {
        SyncPlan {
            current_branch: "feature/login".to_string(),
            base_branch: "main".to_string(),
            strategy: Strategy::Rebase,
            will_push: true,
        }
    }

    #[test]
    fn fetch_failure_is_a_command_failure_not_a_safety_abort() {
        let runner = FakeRunner::default().with(
            "git",
            &["fetch", "origin"],
            false,
            "",
            "network unreachable\n",
        );
        let err = execute_sync(&sample_plan(), &runner).unwrap_err();
        assert!(matches!(err, ForjaError::CommandFailed { .. }));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn rebase_conflict_is_detected_and_not_auto_resolved() {
        let runner = FakeRunner::default()
            .with("git", &["fetch", "origin"], true, "", "")
            .with(
                "git",
                &["rebase", "origin/main"],
                false,
                "CONFLICT (content): Merge conflict in file.rs\n",
                "",
            );
        let err = execute_sync(&sample_plan(), &runner).unwrap_err();
        assert!(matches!(err, ForjaError::RebaseConflict { .. }));
        assert_eq!(err.exit_code(), 4);
        assert!(!runner.called("git", &["rebase", "--abort"]));
    }

    #[test]
    fn auto_push_false_never_invokes_push() {
        let mut plan = sample_plan();
        plan.will_push = false;
        let runner = FakeRunner::default()
            .with("git", &["fetch", "origin"], true, "", "")
            .with("git", &["rebase", "origin/main"], true, "", "");

        let outcome = execute_sync(&plan, &runner).expect("execute_sync should succeed");
        assert!(!outcome.pushed);
        assert!(!runner.called(
            "git",
            &["push", "--force-with-lease", "origin", "feature/login"]
        ));
    }

    #[test]
    fn successful_sync_fetches_integrates_and_pushes() {
        let runner = FakeRunner::default()
            .with("git", &["fetch", "origin"], true, "", "")
            .with("git", &["rebase", "origin/main"], true, "", "")
            .with(
                "git",
                &["push", "--force-with-lease", "origin", "feature/login"],
                true,
                "",
                "",
            );

        let outcome = execute_sync(&sample_plan(), &runner).expect("execute_sync should succeed");
        assert!(outcome.pushed);
        assert_eq!(outcome.base_branch, "main");
        assert_eq!(outcome.current_branch, "feature/login");
    }
}
