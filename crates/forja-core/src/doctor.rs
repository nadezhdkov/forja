use regex::Regex;
use serde::Serialize;

use crate::exec::{CommandRequest, CommandRunner};

const MIN_GIT_VERSION: (u32, u32, u32) = (2, 23, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    /// True if any *required* check failed — this is what `forja doctor`
    /// maps to exit code 3 (PRD §9.2). Warnings alone don't fail the run
    /// (RF-05: `gh` absence is a warning, not an error, in the MVP).
    pub fn has_failure(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Failed)
    }
}

/// Runs every diagnostic check (RF-05) via `runner`, never touching
/// `std::process` directly so this stays testable against a fake runner.
pub fn run_checks(runner: &dyn CommandRunner) -> DoctorReport {
    let mut checks = vec![check_git(runner)];

    if check_gh_presence(runner, &mut checks) {
        checks.push(check_gh_auth(runner));
    }

    DoctorReport { checks }
}

fn check_git(runner: &dyn CommandRunner) -> CheckResult {
    let name = "git".to_string();

    let Ok(outcome) = runner.run(&CommandRequest::new("git", ["--version"])) else {
        return CheckResult {
            name,
            status: CheckStatus::Failed,
            detail: "not found on PATH — install git".to_string(),
        };
    };

    if !outcome.success() {
        return CheckResult {
            name,
            status: CheckStatus::Failed,
            detail: format!("`git --version` failed: {}", outcome.stderr.trim()),
        };
    }

    match parse_version(&outcome.stdout) {
        Some(version) if version >= MIN_GIT_VERSION => CheckResult {
            name,
            status: CheckStatus::Ok,
            detail: format!("{}.{}.{}", version.0, version.1, version.2),
        },
        Some(version) => CheckResult {
            name,
            status: CheckStatus::Failed,
            detail: format!(
                "found {}.{}.{}, need >= {}.{}.{} — upgrade git",
                version.0,
                version.1,
                version.2,
                MIN_GIT_VERSION.0,
                MIN_GIT_VERSION.1,
                MIN_GIT_VERSION.2
            ),
        },
        None => CheckResult {
            name,
            status: CheckStatus::Failed,
            detail: format!("could not parse a version from: {}", outcome.stdout.trim()),
        },
    }
}

/// Checks for `gh` and pushes its result onto `checks`. Returns whether
/// `gh` was found, so the caller knows whether it's worth also checking
/// auth status.
fn check_gh_presence(runner: &dyn CommandRunner, checks: &mut Vec<CheckResult>) -> bool {
    let name = "gh".to_string();

    let found = match runner.run(&CommandRequest::new("gh", ["--version"])) {
        Ok(outcome) if outcome.success() => {
            checks.push(CheckResult {
                name,
                status: CheckStatus::Ok,
                detail: outcome
                    .stdout
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            });
            true
        }
        _ => {
            checks.push(CheckResult {
                name,
                status: CheckStatus::Warning,
                detail: "not found on PATH — needed for Phase 2 (pr, repo new); install from https://cli.github.com"
                    .to_string(),
            });
            false
        }
    };

    found
}

fn check_gh_auth(runner: &dyn CommandRunner) -> CheckResult {
    let name = "gh auth".to_string();

    match runner.run(&CommandRequest::new("gh", ["auth", "status"])) {
        Ok(outcome) if outcome.success() => CheckResult {
            name,
            status: CheckStatus::Ok,
            detail: "authenticated".to_string(),
        },
        Ok(_) => CheckResult {
            name,
            status: CheckStatus::Warning,
            detail: "not authenticated — run `gh auth login`".to_string(),
        },
        Err(_) => CheckResult {
            name,
            status: CheckStatus::Warning,
            detail: "could not determine authentication status".to_string(),
        },
    }
}

fn parse_version(output: &str) -> Option<(u32, u32, u32)> {
    let pattern =
        Regex::new(r"(\d+)\.(\d+)\.(\d+)").expect("version pattern is a valid, fixed regex");
    let caps = pattern.captures(output)?;
    Some((
        caps.get(1)?.as_str().parse().ok()?,
        caps.get(2)?.as_str().parse().ok()?,
        caps.get(3)?.as_str().parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ForjaError;
    use std::cell::RefCell;
    use std::collections::HashMap;

    type FakeResponse = Result<(bool, String, String), ()>;

    struct FakeRunner {
        responses: HashMap<(String, Vec<String>), FakeResponse>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl FakeRunner {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

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
                Ok((success, stdout.to_string(), stderr.to_string())),
            );
            self
        }

        fn missing(mut self, program: &str, args: &[&str]) -> Self {
            self.responses.insert(
                (
                    program.to_string(),
                    args.iter().map(|s| s.to_string()).collect(),
                ),
                Err(()),
            );
            self
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, request: &CommandRequest) -> Result<crate::exec::CommandOutcome, ForjaError> {
            self.calls
                .borrow_mut()
                .push((request.program.clone(), request.args.clone()));

            match self
                .responses
                .get(&(request.program.clone(), request.args.clone()))
            {
                Some(Ok((success, stdout, stderr))) => Ok(crate::exec::CommandOutcome {
                    status_code: Some(if *success { 0 } else { 1 }),
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                }),
                Some(Err(())) | None => Err(ForjaError::CommandSpawn {
                    program: request.program.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
                }),
            }
        }
    }

    #[test]
    fn recent_git_and_authenticated_gh_are_all_ok() {
        let runner = FakeRunner::new()
            .with("git", &["--version"], true, "git version 2.43.0\n", "")
            .with(
                "gh",
                &["--version"],
                true,
                "gh version 2.40.1 (2023-12-13)\n",
                "",
            )
            .with(
                "gh",
                &["auth", "status"],
                true,
                "Logged in to github.com\n",
                "",
            );

        let report = run_checks(&runner);
        assert!(!report.has_failure());
        assert!(report.checks.iter().all(|c| c.status == CheckStatus::Ok));
    }

    #[test]
    fn missing_git_is_a_failure() {
        let runner = FakeRunner::new()
            .missing("git", &["--version"])
            .missing("gh", &["--version"]);

        let report = run_checks(&runner);
        assert!(report.has_failure());
        let git_check = report.checks.iter().find(|c| c.name == "git").unwrap();
        assert_eq!(git_check.status, CheckStatus::Failed);
    }

    #[test]
    fn old_git_is_a_failure() {
        let runner = FakeRunner::new()
            .with("git", &["--version"], true, "git version 2.10.0\n", "")
            .missing("gh", &["--version"]);

        let report = run_checks(&runner);
        assert!(report.has_failure());
    }

    #[test]
    fn missing_gh_is_a_warning_not_a_failure() {
        let runner = FakeRunner::new()
            .with("git", &["--version"], true, "git version 2.43.0\n", "")
            .missing("gh", &["--version"]);

        let report = run_checks(&runner);
        assert!(!report.has_failure());
        let gh_check = report.checks.iter().find(|c| c.name == "gh").unwrap();
        assert_eq!(gh_check.status, CheckStatus::Warning);
        // gh auth status must not be checked when gh itself is missing.
        assert!(!report.checks.iter().any(|c| c.name == "gh auth"));
    }

    #[test]
    fn unauthenticated_gh_is_a_warning_not_a_failure() {
        let runner = FakeRunner::new()
            .with("git", &["--version"], true, "git version 2.43.0\n", "")
            .with("gh", &["--version"], true, "gh version 2.40.1\n", "")
            .with("gh", &["auth", "status"], false, "", "not logged in\n");

        let report = run_checks(&runner);
        assert!(!report.has_failure());
        let auth_check = report.checks.iter().find(|c| c.name == "gh auth").unwrap();
        assert_eq!(auth_check.status, CheckStatus::Warning);
    }
}
