mod common;

use assert_cmd::Command;
use common::TestRepo;
use predicates::str::contains;

fn merge_and_delete_remote(repo: &TestRepo, branch: &str) {
    repo.checkout_new_branch(branch);
    let file_name = format!("{}.txt", branch.replace('/', "-"));
    repo.write_file(&file_name, "content\n");
    repo.commit_all(&format!("add {branch}"));
    repo.push_new(branch);

    repo.checkout("main");
    repo.git(&["merge", "--no-ff", branch, "-m", &format!("merge {branch}")]);
    repo.push("main");
    repo.git(&["push", "origin", "--delete", branch]);
}

fn branch_exists(repo: &TestRepo, branch: &str) -> bool {
    let output = repo.git(&["branch", "--list", branch]);
    !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}

#[test]
fn deletes_merged_branch_whose_remote_was_deleted() {
    let repo = TestRepo::new();
    merge_and_delete_remote(&repo, "feature/done");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .args(["cleanup", "--yes"])
        .assert()
        .success()
        .stdout(contains("deleted 1 of 1"));

    assert!(!branch_exists(&repo, "feature/done"));
}

#[test]
fn preserves_merged_branch_still_present_on_remote() {
    let repo = TestRepo::new();
    repo.checkout_new_branch("feature/keep");
    repo.write_file("feature.txt", "keep\n");
    repo.commit_all("add feature");
    repo.push_new("feature/keep");

    repo.checkout("main");
    repo.git(&["merge", "--no-ff", "feature/keep", "-m", "merge feature"]);
    repo.push("main");
    // deliberately not deleted on the remote

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .args(["cleanup", "--yes"])
        .assert()
        .success()
        .stdout(contains("no branches to clean up"));

    assert!(branch_exists(&repo, "feature/keep"));
}

#[test]
fn protected_branch_is_preserved_even_if_merged_and_gone() {
    let repo = TestRepo::new();
    merge_and_delete_remote(&repo, "develop");
    repo.write_config("version = \"1\"\n[flow]\nprotected_branches = [\"main\", \"develop\"]\n");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .args(["cleanup", "--yes"])
        .assert()
        .success()
        .stdout(contains("no branches to clean up"));

    assert!(branch_exists(&repo, "develop"));
}

#[test]
fn declines_without_yes_when_stdin_says_no() {
    let repo = TestRepo::new();
    merge_and_delete_remote(&repo, "feature/done");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("cleanup")
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(contains("aborted"));

    assert!(branch_exists(&repo, "feature/done"));
}

#[test]
fn confirms_without_yes_when_stdin_says_yes() {
    let repo = TestRepo::new();
    merge_and_delete_remote(&repo, "feature/done");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .arg("cleanup")
        .write_stdin("y\n")
        .assert()
        .success();

    assert!(!branch_exists(&repo, "feature/done"));
}

#[test]
fn dry_run_lists_candidates_without_deleting() {
    let repo = TestRepo::new();
    merge_and_delete_remote(&repo, "feature/done");

    Command::cargo_bin("forja")
        .unwrap()
        .current_dir(repo.path())
        .args(["--dry-run", "cleanup"])
        .assert()
        .success()
        .stdout(contains("feature/done"));

    assert!(branch_exists(&repo, "feature/done"));
}
