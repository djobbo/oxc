use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use rustc_hash::FxHashSet;

use crate::{ChangedPaths, FindChangedFilesOptions, VcsProvider, normalize_path};

/// Errors from git-based changed file detection.
#[derive(Debug)]
pub enum GitVcsError {
    NotAGitRepository,
    GitCommandFailed { command: String, stderr: String },
    Io(io::Error),
}

impl std::fmt::Display for GitVcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAGitRepository => {
                write!(f, "not a git repository (or any of the parent directories)")
            }
            Self::GitCommandFailed { command, stderr } => {
                write!(f, "git command failed: {command}\n{stderr}")
            }
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for GitVcsError {}

impl From<io::Error> for GitVcsError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Git-based implementation of [`VcsProvider`].
///
/// Mirrors Vitest's `GitVCSProvider` with Biome's `--diff-filter=ACMR` and deleted-file filtering.
#[derive(Debug, Default, Clone, Copy)]
pub struct GitVcsProvider;

impl GitVcsProvider {
    fn git_root(cwd: &Path) -> Result<PathBuf, GitVcsError> {
        let output =
            Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(cwd).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.contains("not a git repository") {
                return Err(GitVcsError::NotAGitRepository);
            }
            return Err(GitVcsError::GitCommandFailed {
                command: "git rev-parse --show-toplevel".into(),
                stderr,
            });
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PathBuf::from(root))
    }

    fn run_git(root: &Path, args: &[&str]) -> Result<Vec<String>, GitVcsError> {
        let output = Command::new("git").args(args).current_dir(root).output()?;

        if !output.status.success() {
            return Err(GitVcsError::GitCommandFailed {
                command: format!("git {}", args.join(" ")),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn committed_since(root: &Path, base: &str) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(
            root,
            &["diff", "--name-only", "--relative", "--diff-filter=ACMR", &format!("{base}...HEAD")],
        )
    }

    fn committed_deleted_since(root: &Path, base: &str) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(
            root,
            &["diff", "--name-only", "--relative", "--diff-filter=D", &format!("{base}...HEAD")],
        )
    }

    fn staged_files(root: &Path) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(
            root,
            &["diff", "--name-only", "--relative", "--cached", "--diff-filter=ACMR"],
        )
    }

    fn staged_deleted(root: &Path) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(root, &["diff", "--name-only", "--relative", "--cached", "--diff-filter=D"])
    }

    fn unstaged_files(root: &Path) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(root, &["ls-files", "--other", "--modified", "--exclude-standard"])
    }

    fn unstaged_deleted(root: &Path) -> Result<Vec<String>, GitVcsError> {
        Self::run_git(root, &["diff", "--name-only", "--relative", "--diff-filter=D"])
    }

    /// Old paths from renames (`R*` status lines).
    fn rename_old_paths(root: &Path, cached: bool) -> Result<Vec<String>, GitVcsError> {
        let mut args = vec!["diff", "--name-status", "--relative", "--diff-filter=R"];
        if cached {
            args.push("--cached");
        }
        Self::run_git(root, &args).map(|lines| {
            lines
                .into_iter()
                .filter_map(|line| {
                    let mut parts = line.split_whitespace();
                    let status = parts.next()?;
                    if !status.starts_with('R') {
                        return None;
                    }
                    // Format: R100 old/path new/path
                    Some(parts.next()?.to_string())
                })
                .collect()
        })
    }

    fn resolve_modified_paths(
        root: &Path,
        relative_paths: impl IntoIterator<Item = String>,
    ) -> Vec<PathBuf> {
        relative_paths
            .into_iter()
            .map(|relative| root.join(relative))
            .filter(|path| path.is_file())
            .collect()
    }

    fn resolve_deleted_paths(
        root: &Path,
        relative_paths: impl IntoIterator<Item = String>,
    ) -> Vec<PathBuf> {
        relative_paths.into_iter().map(|relative| root.join(relative)).collect()
    }

    fn dedupe_paths(root: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut seen = FxHashSet::default();
        paths.into_iter().filter(|path| seen.insert(normalize_path(path, root))).collect()
    }

    fn collect_deleted(
        root: &Path,
        options: &FindChangedFilesOptions,
    ) -> Result<Vec<String>, GitVcsError> {
        let mut paths = if options.staged_only {
            Self::staged_deleted(root)?
        } else if let Some(base) = &options.changed_since {
            let mut paths = Self::committed_deleted_since(root, base)?;
            paths.extend(Self::staged_deleted(root)?);
            paths.extend(Self::unstaged_deleted(root)?);
            paths
        } else {
            let mut paths = Self::staged_deleted(root)?;
            paths.extend(Self::unstaged_deleted(root)?);
            paths
        };

        paths.extend(Self::rename_old_paths(root, true)?);
        if !options.staged_only {
            paths.extend(Self::rename_old_paths(root, false)?);
        }

        Ok(paths)
    }
}

impl VcsProvider for GitVcsProvider {
    fn find_changed_files(
        &self,
        options: &FindChangedFilesOptions,
    ) -> Result<ChangedPaths, crate::VcsError> {
        let root = Self::git_root(&options.cwd)?;

        let relative_modified = if options.staged_only {
            Self::staged_files(&root)?
        } else if let Some(base) = &options.changed_since {
            let mut paths = Self::committed_since(&root, base)?;
            paths.extend(Self::staged_files(&root)?);
            paths.extend(Self::unstaged_files(&root)?);
            paths
        } else {
            let mut paths = Self::staged_files(&root)?;
            paths.extend(Self::unstaged_files(&root)?);
            paths
        };

        let relative_deleted = Self::collect_deleted(&root, options)?;

        Ok(ChangedPaths {
            modified: Self::dedupe_paths(
                &root,
                Self::resolve_modified_paths(&root, relative_modified),
            ),
            deleted: Self::dedupe_paths(
                &root,
                Self::resolve_deleted_paths(&root, relative_deleted),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::{
        options::DEFAULT_FORCE_RERUN_TRIGGERS,
        test_helpers::{commit_all, init_git_repo, stage_paths},
    };

    #[test]
    fn uncommitted_changes_include_staged_and_unstaged() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        init_git_repo(root);

        fs::write(root.join("committed.js"), "console.log('committed');").unwrap();
        commit_all(root, "initial");

        fs::write(root.join("staged.js"), "console.log('staged');").unwrap();
        stage_paths(root, &["staged.js"]);

        fs::write(root.join("unstaged.js"), "console.log('unstaged');").unwrap();

        let provider = GitVcsProvider;
        let changed = provider
            .find_changed_files(&FindChangedFilesOptions {
                cwd: root.to_path_buf(),
                changed_since: None,
                staged_only: false,
            })
            .unwrap();

        let names: Vec<_> = changed
            .modified
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"staged.js".to_string()));
        assert!(names.contains(&"unstaged.js".to_string()));
        assert!(!names.contains(&"committed.js".to_string()));
    }

    #[test]
    fn staged_only_returns_staged_files() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        init_git_repo(root);

        fs::write(root.join("a.js"), "console.log('a');").unwrap();
        commit_all(root, "initial");

        fs::write(root.join("staged.js"), "console.log('staged');").unwrap();
        stage_paths(root, &["staged.js"]);
        fs::write(root.join("unstaged.js"), "console.log('unstaged');").unwrap();

        let provider = GitVcsProvider;
        let changed = provider
            .find_changed_files(&FindChangedFilesOptions {
                cwd: root.to_path_buf(),
                changed_since: None,
                staged_only: true,
            })
            .unwrap();

        let names: Vec<_> = changed
            .modified
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["staged.js".to_string()]);
    }

    #[test]
    fn deleted_files_are_tracked_separately() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        init_git_repo(root);

        fs::write(root.join("remove.js"), "console.log('remove');").unwrap();
        commit_all(root, "initial");

        fs::remove_file(root.join("remove.js")).unwrap();
        stage_paths(root, &["remove.js"]);

        let provider = GitVcsProvider;
        let changed = provider
            .find_changed_files(&FindChangedFilesOptions {
                cwd: root.to_path_buf(),
                changed_since: None,
                staged_only: true,
            })
            .unwrap();

        assert!(changed.modified.is_empty());
        assert_eq!(
            changed.deleted.iter().map(|p| p.file_name().unwrap()).collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("remove.js")]
        );
    }

    #[test]
    fn not_a_git_repository_returns_error() {
        let temp = TempDir::new().unwrap();
        let provider = GitVcsProvider;
        let err = provider
            .find_changed_files(&FindChangedFilesOptions {
                cwd: temp.path().to_path_buf(),
                ..FindChangedFilesOptions::default()
            })
            .unwrap_err();
        assert!(matches!(err, crate::VcsError::NotARepository));
    }

    #[test]
    fn force_rerun_triggers_match_config_files() {
        let changed = vec![PathBuf::from("/project/package.json")];
        assert!(crate::matches_force_rerun_trigger(&changed, DEFAULT_FORCE_RERUN_TRIGGERS));
        let changed = vec![PathBuf::from("/project/src/utils.ts")];
        assert!(!crate::matches_force_rerun_trigger(&changed, DEFAULT_FORCE_RERUN_TRIGGERS));
    }
}
