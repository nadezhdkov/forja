/// The commented `forja.toml` scaffold written by `forja init`.
///
/// Only `version` is active — everything else is commented out. `init`
/// never invents a name, email, or preference on the user's behalf; it just
/// shows the shape of the file and the MVP defaults (PRD §8.2, §8.4).
pub const DEFAULT_TEMPLATE: &str = r#"# forja.toml
version = "1"

# [git]
# user_name      = "Your Name"
# user_email     = "you@example.com"
# default_branch = "main"
# editor         = "nvim"
# pull_rebase    = true

# [git.aliases]
# st = "status -sb"
# lg = "log --oneline --graph --decorate --all"

# [flow]
# strategy           = "rebase"        # "rebase" or "merge"
# auto_push          = true
# protected_branches = ["main", "master"]
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use std::io::Write;

    #[test]
    fn template_alone_loads_with_only_version_set() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(DEFAULT_TEMPLATE.as_bytes())
            .expect("write template");

        let outcome = load_config(file.path())
            .expect("template must be valid TOML with no required fields missing");
        assert_eq!(outcome.config.version, "1");
        assert!(outcome.config.git.is_none());
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn template_with_git_section_uncommented_is_valid() {
        let filled = DEFAULT_TEMPLATE
            .lines()
            .map(|line| {
                if line.starts_with("# [git]")
                    || line.starts_with("# user_name")
                    || line.starts_with("# user_email")
                {
                    line.trim_start_matches("# ").to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(filled.as_bytes())
            .expect("write filled template");

        let outcome = load_config(file.path()).expect("filled-in template must be valid");
        let git = outcome.config.git.expect("git section should be present");
        assert_eq!(git.user_name, "Your Name");
        assert_eq!(git.user_email, "you@example.com");
    }
}
