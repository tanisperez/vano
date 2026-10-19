//! Moving files to the system Trash and putting them back (the toast's Undo).
//!
//! Blocking calls, run on the background executor. Vano never deletes anything itself: the
//! Trash keeps every item until the user empties it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

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
    /// Indices into the batch of the items now in the Trash.
    pub moved: Vec<usize>,
    /// Indices of the items left in place, with the reason.
    pub failed: Vec<(usize, TrashError)>,
    /// Whether the batch was cancelled before its end.
    pub cancelled: bool,
}

/// A batch in progress, shared between the background job and the status bar.
#[derive(Debug, Default)]
pub struct TrashProgress {
    pub total: usize,
    pub done: AtomicUsize,
    pub bytes: AtomicU64,
    /// Set by the user to stop before the next item.
    pub cancel: AtomicBool,
    /// Set by the job when it returns.
    pub finished: AtomicBool,
}

impl TrashProgress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            ..Self::default()
        }
    }
}

/// Moves `paths` to the Trash one at a time, so a failure only affects its own item and
/// `progress` can report each one done (its index). Stops before the next item once `cancel`
/// is set.
pub fn move_each(paths: &[PathBuf], cancel: &AtomicBool, progress: impl FnMut(usize)) -> Outcome {
    move_each_with(paths, cancel, move_one, progress)
}

fn move_one(path: &Path) -> Result<(), TrashError> {
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
