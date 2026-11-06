//! Moving files to the system Trash and putting them back (the toast's Undo), and what a batch
//! shares with deleting for good ([`super::delete`]).
//!
//! Blocking calls, run on the background executor. The Trash keeps every item until the user
//! empties it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// What the confirmation does with the items. The first choice of a session is always the Trash,
/// and the choice is never saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DeleteMode {
    #[default]
    Trash,
    Permanent,
}

/// From this many files, the Trash can take a long time and the confirmation says so.
pub const SLOW_TRASH_FILES: u64 = 20_000;

/// Why one item could not be moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrashError {
    /// Windows refused to recycle it, usually because it is bigger than the Recycle Bin allows
    /// or a path inside is too long. It would only be deleted for good, so it is left in place.
    #[cfg_attr(not(windows), allow(dead_code))]
    Refused,
    /// Any other OS error, as its text.
    Os(String),
}

/// What happened to a batch moved one item at a time.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Indices into the batch of the items now in the Trash, or gone for good.
    pub moved: Vec<usize>,
    /// Indices of the items left in place, with the reason.
    pub failed: Vec<(usize, TrashError)>,
    /// Whether the batch was cancelled before its end.
    pub cancelled: bool,
    /// Index of the item that was being deleted for good when the batch was cancelled: part of
    /// it may be gone.
    pub partial: Vec<usize>,
}

/// A batch in progress, shared between the background job and the confirmation.
#[derive(Debug, Default)]
pub struct TrashProgress {
    pub mode: DeleteMode,
    pub total: usize,
    pub total_files: u64,
    pub total_bytes: u64,
    pub done: AtomicUsize,
    /// Files deleted so far. Only a permanent deletion counts them one by one.
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    /// Set by the user to stop before the next item, or the next file when deleting for good.
    pub cancel: AtomicBool,
    /// Set by the job when it returns.
    pub finished: AtomicBool,
}

impl TrashProgress {
    pub fn new(mode: DeleteMode, total: usize, total_files: u64, total_bytes: u64) -> Self {
        Self {
            mode,
            total,
            total_files,
            total_bytes,
            ..Self::default()
        }
    }

    /// Bytes handled so far. The scan's total is an estimate, so this never goes past it.
    pub fn bytes_done(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed).min(self.total_bytes)
    }

    /// Files deleted so far, never past the scan's estimate.
    pub fn files_done(&self) -> u64 {
        self.files.load(Ordering::Relaxed).min(self.total_files)
    }

    /// How far the batch is, from 0 to 1. A permanent deletion takes its time per file, whatever
    /// their size. The Trash moves whole items, so it goes by their bytes.
    pub fn fraction(&self) -> f32 {
        let (done, total) = if self.mode == DeleteMode::Permanent && self.total_files > 0 {
            (self.files_done(), self.total_files)
        } else if self.total_bytes > 0 {
            (self.bytes_done(), self.total_bytes)
        } else {
            (self.done.load(Ordering::Relaxed) as u64, self.total as u64)
        };
        (done as f64 / total.max(1) as f64).clamp(0.0, 1.0) as f32
    }
}

/// Moves `paths` to the Trash one at a time, so a failure only affects its own item and
/// `progress` can report each one done (its index). Stops before the next item once `cancel`
/// is set.
pub fn move_each(paths: &[PathBuf], cancel: &AtomicBool, progress: impl FnMut(usize)) -> Outcome {
    move_each_with(paths, cancel, move_one, progress)
}

fn move_one(path: &Path) -> Result<(), TrashError> {
    // The Recycle Bin refuses a WSL2 distribution's files; its own freedesktop Trash takes them.
    #[cfg(windows)]
    if let Some(root) = vano_scan::linux_root_of(path) {
        return super::linux_trash::trash(&root, path).map_err(TrashError::Os);
    }
    trash::delete(path).map_err(|error| match error {
        // The only error of the crate's Windows code that has no OS code: an aborted operation.
        #[cfg(windows)]
        trash::Error::Unknown { .. } => TrashError::Refused,
        // Just the OS message: the path is already named next to it.
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        trash::Error::FileSystem { source, .. } => TrashError::Os(source.to_string()),
        trash::Error::Os { description, .. } => TrashError::Os(description),
        other => TrashError::Os(other.to_string()),
    })
}

fn move_each_with(
    paths: &[PathBuf],
    cancel: &AtomicBool,
    mut mover: impl FnMut(&Path) -> Result<(), TrashError>,
    mut progress: impl FnMut(usize),
) -> Outcome {
    let mut outcome = Outcome::default();
    for (index, path) in paths.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            outcome.cancelled = true;
            break;
        }
        match mover(path) {
            Ok(()) => outcome.moved.push(index),
            // An error after the item left its place still means it is in the Trash.
            Err(_) if path.symlink_metadata().is_err() => outcome.moved.push(index),
            Err(error) => outcome.failed.push((index, error)),
        }
        progress(index);
    }
    outcome
}

/// Whether this OS lets Vano put items back from the Trash. macOS has no API for it; the user
/// puts them back from the Finder.
pub fn can_restore() -> bool {
    cfg!(any(
        windows,
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))
}

/// Puts back the items of `paths` that were moved to the Trash at `since` (Unix seconds) or
/// later.
///
/// # Errors
///
/// The OS error text, or a note that nothing matching was found.
#[cfg(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
pub fn restore(paths: &[PathBuf], since: i64) -> Result<(), String> {
    #[cfg(windows)]
    {
        // Items of a WSL2 distribution went to its own Trash, grouped here by distribution.
        let mut linux: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
            std::collections::HashMap::new();
        let mut system = Vec::new();
        for path in paths {
            match vano_scan::linux_root_of(path) {
                Some(root) => linux.entry(root).or_default().push(path.clone()),
                None => system.push(path.clone()),
            }
        }
        if !linux.is_empty() {
            let mut first_error = None;
            for (root, items) in &linux {
                if let Err(error) = super::linux_trash::restore(root, items, since) {
                    first_error.get_or_insert(error);
                }
            }
            if !system.is_empty()
                && let Err(error) = restore_system(&system, since)
            {
                first_error.get_or_insert(error);
            }
            return first_error.map_or(Ok(()), Err);
        }
    }
    restore_system(paths, since)
}

/// Puts items back from the system Trash.
#[cfg(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
fn restore_system(paths: &[PathBuf], since: i64) -> Result<(), String> {
    let wanted: HashSet<&PathBuf> = paths.iter().collect();
    let items = trash::os_limited::list().map_err(|error| error.to_string())?;
    let mut matching: Vec<trash::TrashItem> = items
        .into_iter()
        .filter(|item| item.time_deleted >= since - 2 && wanted.contains(&item.original_path()))
        .collect();
    // The newest copy of each path, if the same path was trashed twice.
    matching.sort_by_key(|item| std::cmp::Reverse(item.time_deleted));
    let mut seen = HashSet::new();
    matching.retain(|item| seen.insert(item.original_path()));
    if matching.is_empty() {
        return Err("the items are no longer in the Trash".to_string());
    }
    trash::os_limited::restore_all(matching).map_err(|error| error.to_string())
}

/// Stub for systems without a restore API.
///
/// # Errors
///
/// Always.
#[cfg(not(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
pub fn restore(_paths: &[PathBuf], _since: i64) -> Result<(), String> {
    Err("restoring from the Trash is not supported here".to_string())
}

/// The current time in Unix seconds, as the Trash records it.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn paths(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|name| {
                let path = dir.join(name);
                std::fs::write(&path, b"x").unwrap();
                path
            })
            .collect()
    }

    #[test]
    fn a_permanent_deletion_goes_by_files_and_never_passes_the_estimate() {
        let progress = TrashProgress::new(DeleteMode::Permanent, 1, 10, 100);
        assert_eq!(progress.fraction(), 0.0);
        progress.files.store(5, Ordering::Relaxed);
        progress.bytes.store(99, Ordering::Relaxed);
        assert_eq!(progress.fraction(), 0.5);
        progress.files.store(50, Ordering::Relaxed);
        progress.bytes.store(500, Ordering::Relaxed);
        assert_eq!(progress.fraction(), 1.0);
        assert_eq!(progress.bytes_done(), 100);
        assert_eq!(progress.files_done(), 10);
    }

    #[test]
    fn the_trash_goes_by_bytes_or_by_items_when_it_has_none() {
        let progress = TrashProgress::new(DeleteMode::Trash, 2, 4, 100);
        progress.bytes.store(25, Ordering::Relaxed);
        assert_eq!(progress.fraction(), 0.25);
        let progress = TrashProgress::new(DeleteMode::Trash, 2, 4, 0);
        progress.done.store(1, Ordering::Relaxed);
        assert_eq!(progress.fraction(), 0.5);
    }

    #[test]
    fn a_failure_only_affects_its_own_item() {
        let dir = tempfile::tempdir().unwrap();
        let batch = paths(dir.path(), &["a", "b", "c"]);
        let mut done = Vec::new();
        let outcome = move_each_with(
            &batch,
            &AtomicBool::new(false),
            |path| {
                if path.ends_with("b") {
                    Err(TrashError::Refused)
                } else {
                    std::fs::remove_file(path).map_err(|error| TrashError::Os(error.to_string()))
                }
            },
            |index| done.push(index),
        );
        assert_eq!(outcome.moved, vec![0, 2]);
        assert_eq!(outcome.failed, vec![(1, TrashError::Refused)]);
        assert!(!outcome.cancelled);
        assert_eq!(done, vec![0, 1, 2]);
    }

    #[test]
    fn an_error_after_the_item_left_counts_as_moved() {
        let dir = tempfile::tempdir().unwrap();
        let batch = paths(dir.path(), &["a"]);
        let outcome = move_each_with(
            &batch,
            &AtomicBool::new(false),
            |path| {
                std::fs::remove_file(path).unwrap();
                Err(TrashError::Os("late".into()))
            },
            |_| {},
        );
        assert_eq!(outcome.moved, vec![0]);
        assert!(outcome.failed.is_empty());
    }

    #[test]
    fn cancelling_stops_before_the_next_item() {
        let dir = tempfile::tempdir().unwrap();
        let batch = paths(dir.path(), &["a", "b", "c"]);
        let cancel = AtomicBool::new(false);
        let outcome = move_each_with(
            &batch,
            &cancel,
            |_| Ok(()),
            |index| {
                if index == 0 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert_eq!(outcome.moved, vec![0]);
        assert!(outcome.cancelled);
    }
}
