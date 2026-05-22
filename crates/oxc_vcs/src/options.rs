/// Options for [`crate::VcsProvider::find_changed_files`].
#[derive(Debug, Clone, Default)]
pub struct FindChangedFilesOptions {
    /// Working directory used to locate the git repository.
    pub cwd: std::path::PathBuf,
    /// When set, include committed changes since this ref (plus staged and unstaged).
    /// When unset, only staged and unstaged changes are returned.
    pub changed_since: Option<String>,
    /// When `true`, return only staged files.
    pub staged_only: bool,
}

/// Glob pattern that forces a full lint run when a changed file matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForceRerunTrigger(pub &'static str);

impl ForceRerunTrigger {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0
    }
}

/// Default config-change triggers (Vitest `forceRerunTriggers` parity).
pub const DEFAULT_FORCE_RERUN_TRIGGERS: &[ForceRerunTrigger] = &[
    ForceRerunTrigger("**/oxlint.config.*"),
    ForceRerunTrigger("**/.oxlintrc.json"),
    ForceRerunTrigger("**/.oxlintrc.jsonc"),
    ForceRerunTrigger("**/package.json"),
    ForceRerunTrigger("**/tsconfig*.json"),
];
