//! Version-control helpers for discovering changed files (used by oxlint and other tools).

mod git;
mod options;

pub use git::{GitVcsError, GitVcsProvider};
pub use options::{FindChangedFilesOptions, ForceRerunTrigger, DEFAULT_FORCE_RERUN_TRIGGERS};

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers;

use std::path::{Path, PathBuf};

use cow_utils::CowUtils;

/// Changed paths discovered from version control.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedPaths {
    /// Existing files that were added, copied, modified, or renamed (new path).
    pub modified: Vec<PathBuf>,
    /// Paths that were deleted or renamed (old path). These files may no longer exist on disk.
    pub deleted: Vec<PathBuf>,
}

/// Finds files changed according to version control.
pub trait VcsProvider {
    /// Returns absolute paths of changed files, split into modified and deleted sets.
    ///
    /// # Errors
    ///
    /// Returns [`GitVcsError`] when the repository cannot be located, a git command fails,
    /// or an I/O error occurs while running git.
    fn find_changed_files(
        &self,
        options: &FindChangedFilesOptions,
    ) -> Result<ChangedPaths, GitVcsError>;
}

/// Normalize a path for set comparisons.
///
/// Uses [`Path::canonicalize`] when possible, otherwise absolute resolution from `cwd`.
#[must_use]
pub fn normalize_path(path: &Path, cwd: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        }
    })
}

/// Returns `true` when any changed path matches a force-rerun trigger glob.
#[must_use]
pub fn matches_force_rerun_trigger(
    changed_paths: &[PathBuf],
    triggers: &[ForceRerunTrigger],
) -> bool {
    changed_paths.iter().any(|path| {
        let path_lossy = path.to_string_lossy();
        let path_str = path_lossy.cow_replace('\\', "/");
        triggers.iter().any(|trigger| fast_glob::glob_match(trigger.as_str(), path_str.as_ref()))
    })
}
