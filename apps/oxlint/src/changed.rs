use std::{
    io::Write,
    path::{Path, PathBuf},
};

use oxc_linter::{ConfigStore, LintOptions, LintService, LintServiceOptions, Linter, OsFileSystem};
use oxc_vcs::{
    DEFAULT_FORCE_RERUN_TRIGGERS, FindChangedFilesOptions, GitVcsProvider, VcsError, VcsProvider,
    matches_force_rerun_trigger, normalize_path,
};
use rustc_hash::FxHashSet;

use crate::{cli::ChangedOptions, lint::print_and_flush_stdout};

/// Resolved changed-file sets from CLI options, ready for lint filtering.
pub struct ChangedFilterResult {
    /// Normalized paths of existing changed files.
    pub changed_paths: FxHashSet<PathBuf>,
    /// Normalized paths of deleted files (may no longer exist on disk).
    pub deleted_paths: FxHashSet<PathBuf>,
    /// When true, skip path filtering and lint all candidates (config file changed).
    pub force_full_run: bool,
}

/// Resolve the set of changed file paths from CLI options.
pub fn resolve_changed_paths(
    cwd: &Path,
    options: &ChangedOptions,
) -> Result<ChangedFilterResult, VcsError> {
    let mut changed_paths = FxHashSet::default();
    let mut deleted_paths = FxHashSet::default();

    if !options.related.is_empty() {
        for path in &options.related {
            let absolute = if path.is_absolute() { path.clone() } else { cwd.join(path) };
            if absolute.is_file() {
                changed_paths.insert(normalize_path(&absolute, cwd));
            } else {
                deleted_paths.insert(normalize_path(&absolute, cwd));
            }
        }
    } else if options.staged || options.changed || options.since.is_some() {
        let provider = GitVcsProvider;
        let paths = provider.find_changed_files(&FindChangedFilesOptions {
            cwd: cwd.to_path_buf(),
            changed_since: options.changed_since().map(str::to_string),
            staged_only: options.staged,
        })?;
        for path in paths.modified {
            changed_paths.insert(normalize_path(&path, cwd));
        }
        for path in paths.deleted {
            deleted_paths.insert(normalize_path(&path, cwd));
        }
    }

    let all_for_triggers =
        changed_paths.iter().chain(deleted_paths.iter()).cloned().collect::<Vec<_>>();
    let force_full_run =
        matches_force_rerun_trigger(&all_for_triggers, DEFAULT_FORCE_RERUN_TRIGGERS);

    Ok(ChangedFilterResult { changed_paths, deleted_paths, force_full_run })
}

/// Apply changed-file filtering to lint candidates.
pub fn filter_files_by_changed(
    cwd: &Path,
    candidates: Vec<std::sync::Arc<std::ffi::OsStr>>,
    changed: &ChangedFilterResult,
    use_cross_module: bool,
    tsconfig: Option<&Path>,
    config_store: &ConfigStore,
    external_linter: Option<&oxc_linter::ExternalLinter>,
    stdout: &mut dyn Write,
) -> Vec<std::sync::Arc<std::ffi::OsStr>> {
    if changed.force_full_run {
        return candidates;
    }

    let has_deleted = !changed.deleted_paths.is_empty();
    if !use_cross_module && (!changed.changed_paths.is_empty() || has_deleted) {
        print_and_flush_stdout(
            stdout,
            "warning: changed-file filtering without --import-plugin only lints changed files, not their importers\n",
        );
    }

    if use_cross_module {
        let mut lint_options = LintServiceOptions::new(cwd).with_cross_module(true);
        if let Some(tsconfig) = tsconfig.filter(|path| path.is_file()) {
            lint_options = lint_options.with_tsconfig(tsconfig);
        }
        let lint_service = LintService::new(
            Linter::new(LintOptions::default(), config_store.clone(), external_linter.cloned()),
            lint_options,
        );
        lint_service.filter_paths_by_changed(
            &OsFileSystem,
            candidates,
            &changed.changed_paths,
            &changed.deleted_paths,
        )
    } else {
        candidates
            .into_iter()
            .filter(|path| {
                changed.changed_paths.contains(&normalize_path(Path::new(path.as_ref()), cwd))
            })
            .collect()
    }
}
