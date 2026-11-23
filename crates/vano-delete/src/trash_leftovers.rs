//! What a failed move to the freedesktop Trash leaves behind (#34).
//!
//! Before it renames an item into `Trash/files`, the `trash` crate (5.4) creates an empty
//! placeholder with the same name, so two moves never take the same one. When the rename then
//! fails (a folder without write permission cannot be renamed to another place, since its `..`
//! entry must change) it removes the `.trashinfo` it wrote, but not the placeholder: an empty
//! entry stays in `Trash/files` that nobody can restore. [`remove_placeholders`] removes it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Removes the placeholders a failed move of `item` left in the Trash folders that could hold it.
/// The item is still in place. Only entries that are empty, have no `.trashinfo` and appeared
/// after `since` go: anything the user really trashed has its info file.
pub fn remove_placeholders(item: &Path, since: SystemTime) {
    let Some(name) = item
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return;
    };
    for trash in candidate_trashes(item) {
        remove_in(&trash, &name, since);
    }
}

/// The Trash folders the `trash` crate may have used for `item`: the home Trash, and the
/// `.Trash/<uid>` and `.Trash-<uid>` of the folders above it (a top directory Trash, for other
/// file systems).
fn candidate_trashes(item: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")));
    if let Some(data) = data {
        found.push(data.join("Trash"));
    }
    let uid = rustix::process::getuid().as_raw();
    for folder in item.ancestors().skip(1) {
        found.push(folder.join(".Trash").join(uid.to_string()));
        found.push(folder.join(format!(".Trash-{uid}")));
    }
    found.retain(|trash| trash.join("files").is_dir());
    found
}

/// Removes, in one Trash, the empty entries named `name` or `name.N` that have no info file and
/// were created after `since` (with two seconds of leeway for coarse clocks).
fn remove_in(trash: &Path, name: &str, since: SystemTime) {
    let files = trash.join("files");
    let info = trash.join("info");
    let Ok(entries) = std::fs::read_dir(&files) else {
        return;
    };
    let since = since
        .checked_sub(std::time::Duration::from_secs(2))
        .unwrap_or(since);
    for entry in entries.flatten() {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if !is_placeholder_name(&entry_name, name) {
            continue;
        }
        if info.join(format!("{entry_name}.trashinfo")).exists() {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = path.symlink_metadata() else {
            continue;
        };
        let recent = metadata.modified().is_ok_and(|modified| modified >= since);
        if !recent {
            continue;
        }
        let removed = if metadata.is_dir() {
            // Fails unless empty, which is what a placeholder is.
            std::fs::remove_dir(&path)
        } else if metadata.is_file() && metadata.len() == 0 {
            std::fs::remove_file(&path)
        } else {
            continue;
        };
        if removed.is_ok() {
            log::info!("removed the leftover {} of a failed move", path.display());
        }
    }
}

/// `mod` and `mod.2`, `mod.3`... (the names the crate tries in turn) for `name` = `mod`.
fn is_placeholder_name(entry: &str, name: &str) -> bool {
    match entry.strip_prefix(name) {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix('.')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        None => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn trash() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("files")).unwrap();
        fs::create_dir_all(dir.path().join("info")).unwrap();
        dir
    }

    #[test]
    fn an_empty_placeholder_without_info_goes() {
        let dir = trash();
        let start = SystemTime::now() - std::time::Duration::from_secs(1);
        fs::create_dir(dir.path().join("files/mod")).unwrap();
        fs::write(dir.path().join("files/mod.2"), b"").unwrap();
        remove_in(dir.path(), "mod", start);
        assert!(!dir.path().join("files/mod").exists());
        assert!(!dir.path().join("files/mod.2").exists());
    }

    #[test]
    fn what_the_user_trashed_stays() {
        let dir = trash();
        let start = SystemTime::now() - std::time::Duration::from_secs(1);
        // Trashed for real: it has its info file.
        fs::create_dir(dir.path().join("files/mod")).unwrap();
        fs::write(dir.path().join("info/mod.trashinfo"), b"[Trash Info]\n").unwrap();
        // Not empty.
        fs::create_dir_all(dir.path().join("files/mod.2/a")).unwrap();
        // Another name.
        fs::create_dir(dir.path().join("files/model")).unwrap();
        remove_in(dir.path(), "mod", start);
        assert!(dir.path().join("files/mod").exists());
        assert!(dir.path().join("files/mod.2/a").exists());
        assert!(dir.path().join("files/model").exists());
    }

    #[test]
    fn an_old_empty_entry_stays() {
        let dir = trash();
        fs::create_dir(dir.path().join("files/mod")).unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(60);
        remove_in(dir.path(), "mod", later);
        assert!(dir.path().join("files/mod").exists());
    }

    #[test]
    fn placeholder_names_follow_the_crate() {
        assert!(is_placeholder_name("mod", "mod"));
        assert!(is_placeholder_name("mod.12", "mod"));
        assert!(!is_placeholder_name("mod.", "mod"));
        assert!(!is_placeholder_name("mod.bak", "mod"));
        assert!(!is_placeholder_name("modules", "mod"));
    }
}
