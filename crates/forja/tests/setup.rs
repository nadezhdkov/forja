use std::io::Write;

use assert_cmd::Command;
use predicates::str::contains;

fn write_config(contents: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("create temp config file");
    file.write_all(contents.as_bytes())
        .expect("write temp config file");
    file
}

#[test]
fn setup_applies_git_config_and_is_idempotent() {
    let config = write_config(
        "version = \"1\"\n[git]\nuser_name = \"Ada\"\nuser_email = \"ada@example.com\"\n[git.aliases]\nst = \"status -sb\"\n",
    );
    let gitconfig = tempfile::NamedTempFile::new().expect("create temp gitconfig");

    Command::cargo_bin("forja")
        .unwrap()
        .env("GIT_CONFIG_GLOBAL", gitconfig.path())
        .args(["--config", config.path().to_str().unwrap(), "setup"])
        .assert()
        .success()
        .stdout(contains("user.name"));

    let applied = std::fs::read_to_string(gitconfig.path()).unwrap();
    assert!(applied.contains("Ada"));
    assert!(applied.contains("ada@example.com"));

    // Second run should find everything already conforming.
    Command::cargo_bin("forja")
        .unwrap()
        .env("GIT_CONFIG_GLOBAL", gitconfig.path())
        .args(["--config", config.path().to_str().unwrap(), "setup"])
        .assert()
        .success()
        .stdout(contains("no changes needed"));
}

#[test]
fn setup_dry_run_does_not_write_anything() {
    let config = write_config(
        "version = \"1\"\n[git]\nuser_name = \"Ada\"\nuser_email = \"ada@example.com\"\n",
    );
    let gitconfig = tempfile::NamedTempFile::new().expect("create temp gitconfig");

    Command::cargo_bin("forja")
        .unwrap()
        .env("GIT_CONFIG_GLOBAL", gitconfig.path())
        .args([
            "--config",
            config.path().to_str().unwrap(),
            "--dry-run",
            "setup",
        ])
        .assert()
        .success()
        .stdout(contains("user.name"));

    let contents = std::fs::read_to_string(gitconfig.path()).unwrap();
    assert!(
        contents.is_empty(),
        "dry-run must not write to the gitconfig file"
    );
}

#[test]
fn setup_with_no_git_section_does_nothing() {
    let config = write_config("version = \"1\"\n");
    let gitconfig = tempfile::NamedTempFile::new().expect("create temp gitconfig");

    Command::cargo_bin("forja")
        .unwrap()
        .env("GIT_CONFIG_GLOBAL", gitconfig.path())
        .args(["--config", config.path().to_str().unwrap(), "setup"])
        .assert()
        .success()
        .stdout(contains("nothing to apply"));

    let contents = std::fs::read_to_string(gitconfig.path()).unwrap();
    assert!(contents.is_empty());
}
