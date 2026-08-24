use assert_cmd::Command;
use predicates::str::contains;

/// `doctor` doesn't need a config file at all — it inspects the machine,
/// not `forja.toml`.
#[test]
fn doctor_reports_git_and_exits_0_or_3() {
    let assert = Command::cargo_bin("forja").unwrap().arg("doctor").assert();

    let output = assert.get_output();
    let code = output.status.code().unwrap();
    assert!(
        code == 0 || code == 3,
        "expected exit 0 (all required checks pass) or 3 (a required check failed), got {code}"
    );

    assert.stdout(contains("git"));
}
