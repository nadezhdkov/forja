use std::fmt;
use std::path::PathBuf;

/// A single field-level validation problem found while checking a config.
///
/// Validation always runs to completion and collects every problem (RF-02)
/// instead of stopping at the first one, so callers can report all of them
/// at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationErrors(pub Vec<ValidationError>);

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} problem(s) found:", self.0.len())?;
        for (i, err) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "  - {err}")?;
        }
        Ok(())
    }
}

/// Errors that can occur while locating, reading, parsing, or validating a
/// `forja.toml`, or while running an external command.
///
/// Every variant is written to carry enough context to build a message that
/// states what failed, why, and what to do about it (RNF-05) — that
/// formatting happens here so every caller gets the same quality of error
/// for free.
#[derive(Debug, thiserror::Error)]
pub enum ForjaError {
    #[error(
        "config file not found: {path}\n\nrun `forja init` to create one, or pass --config <path> to point at an existing file"
    )]
    ConfigNotFound { path: PathBuf },

    #[error("could not read config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("config file {path} is not valid TOML:\n{source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("config file {path} has {errors}")]
    Validation {
        path: PathBuf,
        errors: ValidationErrors,
    },

    #[error("failed to run `{program}`: {source}\n\nis it installed and on your PATH?")]
    CommandSpawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("config file already exists: {path}\n\npass --force to overwrite it")]
    ConfigAlreadyExists { path: PathBuf },

    #[error("command failed: `{command}` (exit {exit_code:?})\n{stderr}")]
    CommandFailed {
        command: String,
        stderr: String,
        exit_code: Option<i32>,
    },
}

impl ForjaError {
    /// Maps this error onto the exit-code contract from PRD §9.2.
    ///
    /// Only the codes reachable by errors that already exist in M0 are
    /// mapped here (2: config/usage error, 3: missing dependency); the
    /// remaining codes belong to commands introduced in later milestones.
    pub fn exit_code(&self) -> i32 {
        match self {
            ForjaError::ConfigNotFound { .. }
            | ForjaError::Io { .. }
            | ForjaError::TomlParse { .. }
            | ForjaError::Validation { .. }
            | ForjaError::ConfigAlreadyExists { .. } => 2,
            ForjaError::CommandSpawn { .. } => 3,
            ForjaError::CommandFailed { .. } => 1,
        }
    }
}
