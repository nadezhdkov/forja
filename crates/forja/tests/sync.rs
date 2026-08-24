mod common;

use assert_cmd::Command;
use common::TestRepo;
use predicates::str::contains;

#[test]
fn happy_path_rebases_and_pushes() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/login");
    repo.write_file("feature.txt", "wip\n");
    repo.commit_all("add feature");
    repo.push_new("feature/login");

    // Advance origin/main independently, so there's something to rebase onto.
    repo.checkout("main");
    repo.write_file("other.txt", "server-side change\n");
    repo.commit_all("advance main");
    repo.push("main");
    repo.checkout("feature/login");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("synced with origin/main"));

    let local = repo.current_commit("feature/login");
    let remote = repo.current_commit("origin/feature/login");
    assert_eq!(
        local, remote,
        "feature branch should have been pushed after rebase"
    );
}

#[test]
fn dirty_tree_aborts_with_exit_4_and_changes_nothing() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/x");
    repo.write_file("scratch.txt", "uncommitted\n");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("sync")
        .assert()
        .failure()
        .code(4)
        .stderr(contains("dirty"));

    assert!(repo.status_porcelain().contains("scratch.txt"));
}

#[test]
fn protected_branch_aborts_with_exit_4() {
    // TestRepo::new() leaves us checked out on "main", protected by default.
    let repo = TestRepo::new();

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("sync")
        .assert()
        .failure()
        .code(4)
        .stderr(contains("protected"));
}

#[test]
fn rebase_conflict_aborts_and_leaves_repo_mid_rebase() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/conflict");
    repo.write_file("shared.txt", "feature version\n");
    repo.commit_all("feature change");
    repo.push_new("feature/conflict");

    repo.checkout("main");
    repo.write_file("shared.txt", "main version\n");
    repo.commit_all("main change");
    repo.push("main");
    repo.checkout("feature/conflict");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("sync")
        .assert()
        .failure()
        .code(4)
        .stderr(contains("conflict"));

    assert!(
        repo.is_rebase_in_progress(),
        "forja must leave the repo mid-rebase, never auto-resolve (DD-08)"
    );
}

#[test]
fn auto_push_false_rebases_locally_but_does_not_push() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/local-only");
    repo.write_file("feature.txt", "wip\n");
    repo.commit_all("add feature");
    repo.push_new("feature/local-only");

    repo.checkout("main");
    repo.write_file("other.txt", "server change\n");
    repo.commit_all("advance main");
    repo.push("main");
    repo.checkout("feature/local-only");

    repo.write_config("version = \"1\"\n[flow]\nauto_push = false\n");
    repo.commit_all("add forja.toml");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("not pushed"));

    let local = repo.current_commit("feature/local-only");
    let remote = repo.current_commit("origin/feature/local-only");
    assert_ne!(
        local, remote,
        "local branch should be rebased but not pushed"
    );
}

#[test]
fn dry_run_changes_nothing() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/dry");
    repo.write_file("feature.txt", "wip\n");
    repo.commit_all("add feature");
    let before = repo.current_commit("feature/dry");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .args(["--dry-run", "sync"])
        .assert()
        .success();

    let after = repo.current_commit("feature/dry");
    assert_eq!(before, after);
}
