use std::collections::HashMap;

use serde::Deserialize;

/// Mirrors the on-disk shape of `forja.toml` exactly (PRD §8.2) with every
/// field optional, so a missing or wrong-typed field becomes a validation
/// problem we can report with the others, not a hard `serde` parse failure.
///
/// Each level captures its own unrecognized keys via `#[serde(flatten)]`
/// into a `toml::Table`, which is how unknown-key warnings (DD-06) are
/// implemented without `deny_unknown_fields` turning them into errors.
#[derive(Debug, Clone, Deserialize)]
pub struct RawConfig {
    pub version: Option<toml::Value>,
    pub git: Option<RawGitConfig>,
    pub flow: Option<RawFlowConfig>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawGitConfig {
    pub user_name: Option<toml::Value>,
    pub user_email: Option<toml::Value>,
    pub default_branch: Option<toml::Value>,
    pub editor: Option<toml::Value>,
    pub pull_rebase: Option<toml::Value>,
    pub aliases: Option<RawGitAliases>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawGitAliases {
    #[serde(flatten)]
    pub entries: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawFlowConfig {
    pub base_branch: Option<toml::Value>,
    pub strategy: Option<toml::Value>,
    pub auto_push: Option<toml::Value>,
    pub protected_branches: Option<toml::Value>,
    #[serde(flatten)]
    pub extra: toml::Table,
}
