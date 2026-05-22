use std::io;

use crate::git::GitVcsError;

/// Errors from version-control changed file detection.
#[derive(Debug)]
pub enum VcsError {
    /// No supported VCS repository was found.
    NotARepository,
    /// Git-specific failure.
    Git(GitVcsError),
}

impl std::fmt::Display for VcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotARepository => {
                write!(f, "not a version control repository (or any of the parent directories)")
            }
            Self::Git(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for VcsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotARepository => None,
            Self::Git(err) => Some(err),
        }
    }
}

impl From<GitVcsError> for VcsError {
    fn from(err: GitVcsError) -> Self {
        match err {
            GitVcsError::NotAGitRepository => Self::NotARepository,
            other => Self::Git(other),
        }
    }
}

impl From<io::Error> for VcsError {
    fn from(err: io::Error) -> Self {
        Self::Git(GitVcsError::Io(err))
    }
}
