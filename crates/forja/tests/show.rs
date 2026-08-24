use std::io::Write;

use assert_cmd::Command;
use predicates::str::contains;

fn write_config(contents: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("create temp config file");
    file.write_all(contents.as_bytes()).expect("write temp config file");
    file
}

#[test]
fn show_prints_valid_config_and_exits_zero() {
    let config = write_config(
        "version = \"1\"\n[git]\nuser_name = \"Ada\"\nuser_email = \"ada@example.com\"\n",
    );

    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", config.path().to_str().unwrap(), "show"])
        .assert()
        .success()
        .stdout(contains("ada@example.com"));
}

#[test]
fn show_fails_with_exit_2_when_config_file_is_missing() {
    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", "/nonexistent/forja.toml", "show"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("config file not found"));
}

#[test]
fn show_fails_with_exit_2_and_lists_problems_for_invalid_config() {
    let config = write_config("[git]\nuser_name = \"\"\n");

    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", config.path().to_str().unwrap(), "show"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("version"))
        .stderr(contains("git.user_name"))
        .stderr(contains("git.user_email"));
}

#[test]
fn verbose_and_quiet_together_exit_2() {
    let config = write_config("version = \"1\"\n");

    Command::cargo_bin("forja")
        .unwrap()
        .args([
            "--config",
            config.path().to_str().unwrap(),
            "--verbose",
            "--quiet",
            "show",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("mutually exclusive"));
}
