//! Git repository helpers for integration tests.

use std::{path::Path, process::Command};

/// Initialize a git repository with test user config.
pub fn init_git_repo(dir: &Path) {
    assert!(Command::new("git").args(["init"]).current_dir(dir).status().unwrap().success());
    assert!(
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success()
    );
}

/// Stage paths relative to `dir`.
pub fn stage_paths(dir: &Path, paths: &[&str]) {
    for path in paths {
        assert!(
            Command::new("git").args(["add", path]).current_dir(dir).status().unwrap().success()
        );
    }
}

/// Stage all changes and create a commit.
pub fn commit_all(dir: &Path, message: &str) {
    stage_paths(dir, &["-A"]);
    assert!(
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(dir)
            .status()
            .unwrap()
            .success()
    );
}
