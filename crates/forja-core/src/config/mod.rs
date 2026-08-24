mod schema;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::Serialize;

use crate::error::{ForjaError, ValidationError, ValidationErrors};
use schema::{RawConfig, RawFlowConfig, RawGitConfig};

const SUPPORTED_VERSION: &str = "1";
const ALIAS_NAME_PATTERN: &str = r"^[a-zA-Z0-9_-]+$";

/// The strategy `forja sync` uses to integrate the base branch (PRD §8.2,
/// `[flow].strategy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    #[default]
    Rebase,
    Merge,
}

/// Validated, defaulted `[git]` section. Only present when the config file
/// declares a `[git]` table — flows never require it (PRD §8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GitConfig {
    pub user_name: String,
    pub user_email: String,
    pub default_branch: String,
    pub editor: Option<String>,
    pub pull_rebase: Option<bool>,
    pub aliases: BTreeMap<String, String>,
}

/// Validated, defaulted `[flow]` section. Always present with defaults
/// applied, since `sync`/`cleanup` must work with zero config (PRD §8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowConfig {
    pub base_branch: Option<String>,
    pub strategy: Strategy,
    pub auto_push: bool,
    pub protected_branches: Vec<String>,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            base_branch: None,
            strategy: Strategy::default(),
            auto_push: true,
            protected_branches: vec!["main".to_string(), "master".to_string()],
        }
    }
}

/// A fully loaded, validated, and defaulted `forja.toml` (PRD §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForjaConfig {
    pub version: String,
    pub git: Option<GitConfig>,
    pub flow: FlowConfig,
}

/// The result of a successful [`load_config`] call: the validated config,
/// plus any non-fatal warnings (unknown keys, DD-06) gathered along the way.
#[derive(Debug, Clone)]
pub struct LoadOutcome {
    pub config: ForjaConfig,
    pub warnings: Vec<String>,
}

/// Loads and validates a `forja.toml` from `path`.
///
/// A missing file is always an error here — callers that must tolerate a
/// missing file (flows, per RF-01) should check [`Path::exists`] themselves
/// before calling this.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ForjaError> {
    if !path.exists() {
        return Err(ForjaError::ConfigNotFound {
            path: path.to_path_buf(),
        });
    }

    let contents = fs::read_to_string(path).map_err(|source| ForjaError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let raw: RawConfig = toml::from_str(&contents).map_err(|source| ForjaError::TomlParse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    collect_unknown_keys("", &raw.extra, &mut warnings);

    let version = validate_version(&raw, &mut errors);
    let git = raw
        .git
        .as_ref()
        .map(|g| validate_git(g, &mut errors, &mut warnings));
    let flow = validate_flow(raw.flow.as_ref(), &mut errors, &mut warnings);

    if !errors.is_empty() {
        return Err(ForjaError::Validation {
            path: path.to_path_buf(),
            errors: ValidationErrors(errors),
        });
    }

    Ok(LoadOutcome {
        config: ForjaConfig {
            version: version.expect("version must be Some when there are no errors"),
            git,
            flow,
        },
        warnings,
    })
}

/// Loads just the `[flow]` section for `sync`/`cleanup`, tolerating a
/// missing file (RF-01: flows work with zero config). A file that exists
/// but is invalid is still reported — even if the problem is in `[git]`,
/// which flows never read — because a broken `forja.toml` should never be
/// silently ignored.
pub fn load_flow_config(path: &Path) -> Result<(FlowConfig, Vec<String>), ForjaError> {
    if !path.exists() {
        return Ok((FlowConfig::default(), Vec::new()));
    }

    let outcome = load_config(path)?;
    Ok((outcome.config.flow, outcome.warnings))
}

fn validate_version(raw: &RawConfig, errors: &mut Vec<ValidationError>) -> Option<String> {
    match &raw.version {
        None => {
            errors.push(ValidationError::new(
                "version",
                "is required — add `version = \"1\"` at the top of the file",
            ));
            None
        }
        Some(value) => match value.as_str() {
            Some(SUPPORTED_VERSION) => Some(SUPPORTED_VERSION.to_string()),
            Some(other) => {
                errors.push(ValidationError::new(
                    "version",
                    format!("must be \"{SUPPORTED_VERSION}\", found \"{other}\""),
                ));
                None
            }
            None => {
                errors.push(ValidationError::new(
                    "version",
                    format!("must be a string, e.g. \"{SUPPORTED_VERSION}\""),
                ));
                None
            }
        },
    }
}

fn validate_git(
    raw: &RawGitConfig,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<String>,
) -> GitConfig {
    collect_unknown_keys("git.", &raw.extra, warnings);

    let user_name = required_non_empty_string(&raw.user_name, "git.user_name", errors);
    let user_email = required_email(&raw.user_email, "git.user_email", errors);
    let default_branch =
        optional_non_empty_string(&raw.default_branch, "git.default_branch", errors)
            .unwrap_or_else(|| "main".to_string());
    let editor = optional_non_empty_string(&raw.editor, "git.editor", errors);
    let pull_rebase = optional_bool(&raw.pull_rebase, "git.pull_rebase", errors);
    let aliases = validate_aliases(raw, errors);

    GitConfig {
        user_name: user_name.unwrap_or_default(),
        user_email: user_email.unwrap_or_default(),
        default_branch,
        editor,
        pull_rebase,
        aliases,
    }
}

fn validate_aliases(
    raw: &RawGitConfig,
    errors: &mut Vec<ValidationError>,
) -> BTreeMap<String, String> {
    let Some(aliases) = &raw.aliases else {
        return BTreeMap::new();
    };

    let name_pattern =
        Regex::new(ALIAS_NAME_PATTERN).expect("alias name pattern is a valid, fixed regex");
    let mut result = BTreeMap::new();

    for (name, value) in &aliases.entries {
        let field = format!("git.aliases.{name}");

        if !name_pattern.is_match(name) {
            errors.push(ValidationError::new(
                &field,
                format!("alias name \"{name}\" must match {ALIAS_NAME_PATTERN}"),
            ));
            continue;
        }

        match value.as_str() {
            Some(s) if !s.is_empty() => {
                result.insert(name.clone(), s.to_string());
            }
            Some(_) => errors.push(ValidationError::new(
                &field,
                "alias value must not be empty",
            )),
            None => errors.push(ValidationError::new(&field, "alias value must be a string")),
        }
    }

    result
}

fn validate_flow(
    raw: Option<&RawFlowConfig>,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<String>,
) -> FlowConfig {
    let defaults = FlowConfig::default();
    let Some(raw) = raw else {
        return defaults;
    };

    collect_unknown_keys("flow.", &raw.extra, warnings);

    let base_branch = optional_non_empty_string(&raw.base_branch, "flow.base_branch", errors);
    let strategy = validate_strategy(&raw.strategy, errors).unwrap_or(defaults.strategy);
    let auto_push =
        optional_bool(&raw.auto_push, "flow.auto_push", errors).unwrap_or(defaults.auto_push);
    let protected_branches = validate_protected_branches(&raw.protected_branches, errors)
        .unwrap_or(defaults.protected_branches);

    FlowConfig {
        base_branch,
        strategy,
        auto_push,
        protected_branches,
    }
}

fn validate_strategy(
    value: &Option<toml::Value>,
    errors: &mut Vec<ValidationError>,
) -> Option<Strategy> {
    let value = value.as_ref()?;
    match value.as_str() {
        Some("rebase") => Some(Strategy::Rebase),
        Some("merge") => Some(Strategy::Merge),
        Some(other) => {
            errors.push(ValidationError::new(
                "flow.strategy",
                format!("must be \"rebase\" or \"merge\", found \"{other}\""),
            ));
            None
        }
        None => {
            errors.push(ValidationError::new("flow.strategy", "must be a string"));
            None
        }
    }
}

fn validate_protected_branches(
    value: &Option<toml::Value>,
    errors: &mut Vec<ValidationError>,
) -> Option<Vec<String>> {
    let value = value.as_ref()?;
    let Some(array) = value.as_array() else {
        errors.push(ValidationError::new(
            "flow.protected_branches",
            "must be an array of strings",
        ));
        return None;
    };

    let mut branches = Vec::with_capacity(array.len());
    for (i, item) in array.iter().enumerate() {
        match item.as_str() {
            Some(s) if !s.is_empty() => branches.push(s.to_string()),
            _ => errors.push(ValidationError::new(
                format!("flow.protected_branches[{i}]"),
                "must be a non-empty string",
            )),
        }
    }
    Some(branches)
}

fn required_non_empty_string(
    value: &Option<toml::Value>,
    field: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    match value {
        None => {
            errors.push(ValidationError::new(field, "is required"));
            None
        }
        Some(v) => match v.as_str() {
            Some(s) if !s.is_empty() => Some(s.to_string()),
            Some(_) => {
                errors.push(ValidationError::new(field, "must not be empty"));
                None
            }
            None => {
                errors.push(ValidationError::new(field, "must be a string"));
                None
            }
        },
    }
}

fn required_email(
    value: &Option<toml::Value>,
    field: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    let email = required_non_empty_string(value, field, errors)?;
    if email.contains('@') {
        Some(email)
    } else {
        errors.push(ValidationError::new(field, "must contain \"@\""));
        None
    }
}

fn optional_non_empty_string(
    value: &Option<toml::Value>,
    field: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<String> {
    let value = value.as_ref()?;
    match value.as_str() {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        Some(_) => {
            errors.push(ValidationError::new(field, "must not be empty if present"));
            None
        }
        None => {
            errors.push(ValidationError::new(field, "must be a string"));
            None
        }
    }
}

fn optional_bool(
    value: &Option<toml::Value>,
    field: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<bool> {
    let value = value.as_ref()?;
    match value.as_bool() {
        Some(b) => Some(b),
        None => {
            errors.push(ValidationError::new(field, "must be a boolean"));
            None
        }
    }
}

fn collect_unknown_keys(prefix: &str, extra: &toml::Table, warnings: &mut Vec<String>) {
    for key in extra.keys() {
        warnings.push(format!(
            "unknown config key \"{prefix}{key}\" is ignored (it may belong to a newer forja schema)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn load(contents: &str) -> Result<LoadOutcome, ForjaError> {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(contents.as_bytes())
            .expect("write temp file");
        load_config(file.path())
    }

    #[test]
    fn missing_file_is_config_not_found() {
        let err = load_config(Path::new("/nonexistent/forja.toml")).unwrap_err();
        assert!(matches!(err, ForjaError::ConfigNotFound { .. }));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn malformed_toml_is_reported() {
        let err = load("this is not [ valid toml").unwrap_err();
        assert!(matches!(err, ForjaError::TomlParse { .. }));
    }

    #[test]
    fn missing_version_is_a_validation_error() {
        let err = load("[git]\nuser_name = \"a\"\nuser_email = \"a@b.com\"\n").unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert!(errors.0.iter().any(|e| e.field == "version"));
    }

    #[test]
    fn wrong_version_is_a_validation_error() {
        let err = load("version = \"2\"\n").unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert!(errors.0.iter().any(|e| e.field == "version"));
    }

    #[test]
    fn git_without_user_email_reports_missing_field() {
        let err = load("version = \"1\"\n[git]\nuser_name = \"Ada\"\n").unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert!(errors.0.iter().any(|e| e.field == "git.user_email"));
    }

    #[test]
    fn user_email_without_at_sign_is_rejected() {
        let err =
            load("version = \"1\"\n[git]\nuser_name = \"Ada\"\nuser_email = \"not-an-email\"\n")
                .unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert!(errors.0.iter().any(|e| e.field == "git.user_email"));
    }

    #[test]
    fn all_errors_are_reported_together_not_fail_fast() {
        let err = load("[git]\nuser_name = \"\"\n").unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        // version missing, git.user_name empty, git.user_email missing — all three at once.
        assert_eq!(errors.0.len(), 3);
    }

    #[test]
    fn unknown_root_key_is_a_warning_not_an_error() {
        let outcome =
            load("version = \"1\"\nfrobnicate = true\n").expect("should load despite unknown key");
        assert!(outcome.warnings.iter().any(|w| w.contains("frobnicate")));
    }

    #[test]
    fn invalid_alias_name_is_rejected() {
        let err = load("version = \"1\"\n[git.aliases]\n\"bad name\" = \"status\"\n").unwrap_err();
        let ForjaError::Validation { errors, .. } = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert!(errors.0.iter().any(|e| e.field.contains("aliases")));
    }

    #[test]
    fn minimal_config_with_only_version_uses_defaults() {
        let outcome = load("version = \"1\"\n").expect("minimal config should be valid");
        assert_eq!(outcome.config.version, "1");
        assert!(outcome.config.git.is_none());
        assert_eq!(outcome.config.flow, FlowConfig::default());
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn full_example_from_prd_loads_cleanly() {
        let toml = r#"
version = "1"

[git]
user_name      = "Fulano de Tal"
user_email     = "fulano@example.com"
default_branch = "main"
editor         = "nvim"
pull_rebase    = true

[git.aliases]
st   = "status -sb"
lg   = "log --oneline --graph --decorate --all"
undo = "reset --soft HEAD~1"

[flow]
strategy            = "rebase"
auto_push           = true
protected_branches  = ["main", "develop"]
"#;
        let outcome = load(toml).expect("PRD example config must be valid");
        assert!(outcome.warnings.is_empty());

        let git = outcome.config.git.expect("git section expected");
        assert_eq!(git.user_name, "Fulano de Tal");
        assert_eq!(git.user_email, "fulano@example.com");
        assert_eq!(git.aliases.get("st"), Some(&"status -sb".to_string()));

        assert_eq!(outcome.config.flow.strategy, Strategy::Rebase);
        assert_eq!(
            outcome.config.flow.protected_branches,
            vec!["main".to_string(), "develop".to_string()]
        );
    }
}
