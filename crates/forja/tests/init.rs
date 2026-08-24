use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn init_creates_a_config_file_that_did_not_exist() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config_path = dir.path().join("forja.toml");

    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", config_path.to_str().unwrap(), "init"])
        .assert()
        .success();

    assert!(config_path.exists());
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("version = \"1\""));
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config_path = dir.path().join("forja.toml");
    std::fs::write(&config_path, "version = \"1\"\n").unwrap();

    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", config_path.to_str().unwrap(), "init"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("already exists"));
}

#[test]
fn init_overwrites_with_force() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config_path = dir.path().join("forja.toml");
    std::fs::write(&config_path, "stale content").unwrap();

    Command::cargo_bin("forja")
        .unwrap()
        .args(["--config", config_path.to_str().unwrap(), "init", "--force"])
        .assert()
        .success();

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("version = \"1\""));
}
