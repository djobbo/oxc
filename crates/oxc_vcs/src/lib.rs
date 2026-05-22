mod git;
mod options;

pub use git::{GitVcsError, GitVcsProvider};
pub use options::{FindChangedFilesOptions, ForceRerunTrigger, DEFAULT_FORCE_RERUN_TRIGGERS};

use std::path::{Path, PathBuf};

/// Finds files changed according to version control.
pub trait VcsProvider {
    /// Returns absolute paths of changed files.
    fn find_changed_files(
        &self,
        options: &FindChangedFilesOptions,
    ) -> Result<Vec<PathBuf>, GitVcsError>;
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
        let path_str = path.to_string_lossy().replace('\\', "/");
        triggers.iter().any(|trigger| fast_glob::glob_match(trigger.as_str(), &path_str))
    })
}
