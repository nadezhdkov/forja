use std::process::Command;

use crate::error::ForjaError;

/// A request to run an external command as an argument vector.
///
/// There is deliberately no way to build this from a single shell string —
/// every flow constructs `program` and `args` directly, so there is never a
/// string-interpolation point where config content could reach a shell
/// (PRD §15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandRequest {
    pub fn new(program: impl Into<String>, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

/// The result of running a [`CommandRequest`] to completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutcome {
    pub fn success(&self) -> bool {
        self.status_code == Some(0)
    }
}

/// Runs external commands. Abstracted behind a trait so flow logic can be
/// unit-tested against a fake runner without touching a real process or
/// repository.
pub trait CommandRunner {
    fn run(&self, request: &CommandRequest) -> Result<CommandOutcome, ForjaError>;
}

/// The real runner, backed by [`std::process::Command`].
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, request: &CommandRequest) -> Result<CommandOutcome, ForjaError> {
        let output = Command::new(&request.program)
            .args(&request.args)
            .output()
            .map_err(|source| ForjaError::CommandSpawn {
                program: request.program.clone(),
                source,
            })?;

        Ok(CommandOutcome {
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_trivial_command_and_captures_output() {
        let runner = SystemCommandRunner;
        let outcome = runner
            .run(&CommandRequest::new("echo", ["hello"]))
            .expect("echo should be available in the test environment");

        assert!(outcome.success());
        assert_eq!(outcome.stdout.trim(), "hello");
    }

    #[test]
    fn reports_spawn_failure_for_a_nonexistent_program() {
        let runner = SystemCommandRunner;
        let err = runner
            .run(&CommandRequest::new("forja-nonexistent-binary-xyz", Vec::<String>::new()))
            .expect_err("spawning a nonexistent program should fail");

        assert_eq!(err.exit_code(), 3);
    }
}
