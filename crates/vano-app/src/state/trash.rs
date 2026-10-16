//! Moving files to the system Trash and putting them back (the toast's Undo).
//!
//! Blocking calls, run on the background executor. Vano never deletes anything itself: the
//! Trash keeps every item until the user empties it.

use std::collections::HashSet;
use std::path::PathBuf;

/// Moves `paths` to the Trash.
///
/// # Errors
///
/// The OS error text when the move fails (the parts moved before the failure stay in the
/// Trash).
pub fn move_to_trash(paths: &[PathBuf]) -> Result<(), String> {
    trash::delete_all(paths).map_err(|error| error.to_string())
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
