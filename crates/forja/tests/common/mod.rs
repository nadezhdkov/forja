// This module is compiled separately into every integration test binary
// that includes it (`sync.rs`, `cleanup.rs`, ...); each one only exercises
// a subset of the helper methods, so the rest look unused from that binary's
// point of view.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A real, disposable git repository wired up with a bare "origin" (PRD
/// §13: no network, no GitHub, but real `git` end to end). Starts with one
/// commit on `main`, pushed, with `origin/HEAD` pointing at it — the same
/// baseline `git clone` would leave you with.
pub struct TestRepo {
    dir: tempfile::TempDir,
}

fn run(cwd: &Path, program: &str, args: &[&str]) -> Output {
    let output = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));
    assert!(
        output.status.success(),
        "{program} {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

impl TestRepo {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let origin_path = dir.path().join("origin.git");
        let work_path = dir.path().join("work");

        run(
            dir.path(),
            "git",
            &[
                "init",
                "--bare",
                "-b",
                "main",
                origin_path.to_str().unwrap(),
            ],
        );
        run(
            dir.path(),
            "git",
            &["init", "-b", "main", work_path.to_str().unwrap()],
        );

        let repo = Self { dir };
        repo.git(&["config", "user.name", "Test User"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["remote", "add", "origin", origin_path.to_str().unwrap()]);

        repo.write_file("README.md", "hello\n");
        repo.commit_all("initial commit");
        repo.push_new("main");
        repo.git(&["remote", "set-head", "origin", "main"]);

        repo
    }

    pub fn path(&self) -> PathBuf {
        self.dir.path().join("work")
    }

    pub fn git(&self, args: &[&str]) -> Output {
        run(&self.path(), "git", args)
    }

    pub fn write_file(&self, name: &str, content: &str) {
        std::fs::write(self.path().join(name), content).expect("write test file");
    }

    pub fn commit_all(&self, message: &str) {
        self.git(&["add", "."]);
        self.git(&["commit", "-m", message]);
    }

    /// Pushes a branch for the first time, setting up tracking — so a
    /// later `--force-with-lease` push has an unambiguous remote-tracking
    /// ref to compare against.
    pub fn push_new(&self, branch: &str) {
        self.git(&["push", "-u", "origin", branch]);
    }

    pub fn push(&self, branch: &str) {
        self.git(&["push", "origin", branch]);
    }

    pub fn checkout_new_branch(&self, name: &str) {
        self.git(&["checkout", "-b", name]);
    }

    pub fn checkout(&self, name: &str) {
        self.git(&["checkout", name]);
    }

    pub fn current_commit(&self, rev: &str) -> String {
        let output = self.git(&["rev-parse", rev]);
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    pub fn status_porcelain(&self) -> String {
        let output = Command::new("git")
            .current_dir(self.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("run git status");
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    pub fn is_rebase_in_progress(&self) -> bool {
        self.path().join(".git/rebase-merge").exists()
            || self.path().join(".git/rebase-apply").exists()
    }

    pub fn write_config(&self, contents: &str) {
        std::fs::write(self.path().join("forja.toml"), contents).expect("write forja.toml");
    }
}
