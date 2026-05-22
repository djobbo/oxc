use std::{
    io::Write,
    path::{Path, PathBuf},
};

use oxc_linter::{LintService, LintServiceOptions, Linter, LintOptions, OsFileSystem, ConfigStore};
use oxc_vcs::{
    DEFAULT_FORCE_RERUN_TRIGGERS, FindChangedFilesOptions, GitVcsError, GitVcsProvider,
    VcsProvider, matches_force_rerun_trigger, normalize_path,
};
use rustc_hash::FxHashSet;

use crate::{
    cli::ChangedOptions,
    lint::print_and_flush_stdout,
};

pub struct ChangedFilterResult {
    pub changed_paths: FxHashSet<PathBuf>,
    pub force_full_run: bool,
}

/// Resolve the set of changed file paths from CLI options.
pub fn resolve_changed_paths(
    cwd: &Path,
    options: &ChangedOptions,
) -> Result<ChangedFilterResult, GitVcsError> {
    let mut changed_paths = FxHashSet::default();

    if !options.related.is_empty() {
        for path in &options.related {
            let absolute = if path.is_absolute() { path.clone() } else { cwd.join(path) };
            if absolute.is_file() {
                changed_paths.insert(normalize_path(&absolute, cwd));
            }
        }
    } else if options.staged || options.changed || options.since.is_some() {
        let provider = GitVcsProvider;
        let paths = provider.find_changed_files(&FindChangedFilesOptions {
            cwd: cwd.to_path_buf(),
            changed_since: options.changed_since().map(str::to_string),
            staged_only: options.staged,
        })?;
        for path in paths {
            changed_paths.insert(normalize_path(&path, cwd));
        }
    }

    let force_full_run = matches_force_rerun_trigger(
        &changed_paths.iter().cloned().collect::<Vec<_>>(),
        DEFAULT_FORCE_RERUN_TRIGGERS,
    );

    Ok(ChangedFilterResult { changed_paths, force_full_run })
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

    if !use_cross_module && !changed.changed_paths.is_empty() {
        print_and_flush_stdout(
            stdout,
            "warning: --changed without --import-plugin only lints files in git diff, not their importers\n",
        );
    }

    let normalized_changed: FxHashSet<PathBuf> = changed
        .changed_paths
        .iter()
        .map(|path| normalize_path(path, cwd))
        .collect();

    if use_cross_module {
        let mut lint_options = LintServiceOptions::new(cwd).with_cross_module(true);
        if let Some(tsconfig) = tsconfig.filter(|path| path.is_file()) {
            lint_options = lint_options.with_tsconfig(tsconfig);
        }
        let lint_service = LintService::new(
            Linter::new(LintOptions::default(), config_store.clone(), external_linter.cloned()),
            lint_options,
        );
        lint_service.filter_paths_by_changed(&OsFileSystem, candidates, &normalized_changed)
    } else {
        candidates
            .into_iter()
            .filter(|path| {
                normalized_changed.contains(&normalize_path(Path::new(path.as_ref()), cwd))
            })
            .collect()
    }
}
