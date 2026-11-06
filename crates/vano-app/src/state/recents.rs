//! The recent scans listed on the home screen, and the saved trees that let "Open results" and
//! a restart show the last results without scanning again.
//!
//! The list is `<config dir>/vano/recents.json`; each tree is a file under
//! `<cache dir>/vano/scans/`. Losing either is harmless: the scan just runs again. A tree whose
//! scan left the list has no use and can be large, so [`prune_scans`] removes it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::target::Target;

/// How many scans are remembered.
const MAX_RECENTS: usize = 4;

/// One remembered scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recent {
    pub target: Target,
    /// Bytes counted.
    pub size: u64,
    /// Files counted.
    pub files: u64,
    /// When the scan finished, Unix seconds.
    pub scanned_at: u64,
    /// How long it took, in seconds.
    pub seconds: f64,
    /// The saved tree, a file name under the scans cache folder.
    pub cache: String,
}

impl Recent {
    /// The saved tree's path, `None` without a cache folder.
    pub fn cache_path(&self) -> Option<PathBuf> {
        scans_dir().map(|dir| dir.join(&self.cache))
    }
}

/// Where saved trees go.
pub fn scans_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("vano").join("scans"))
}

/// The cache file name of a target: stable per path, so a new scan replaces the old tree.
pub fn cache_name(target: &Target) -> String {
    // FNV-1a over the path bytes: stable across runs and versions, unlike `DefaultHasher`.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in target.path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}.vtree")
}

/// The remembered scans, newest first.
pub fn load() -> Vec<Recent> {
    dirs::config_dir()
        .map(|base| read(&base))
        .unwrap_or_default()
}

/// Puts `recent` first, replacing an older scan of the same target, and saves the list.
pub fn record(recent: Recent) -> Vec<Recent> {
    let Some(base) = dirs::config_dir() else {
        return vec![recent];
    };
    let list = push(read(&base), recent);
    write(&base, &list);
    list
}

/// Takes `target` out of the list and saves it. [`prune_scans`] then removes its saved tree.
pub fn forget(target: &Target) -> Vec<Recent> {
    let Some(base) = dirs::config_dir() else {
        return Vec::new();
    };
    let list = without(read(&base), target);
    write(&base, &list);
    list
}

fn without(mut list: Vec<Recent>, target: &Target) -> Vec<Recent> {
    list.retain(|old| old.target.path != target.path);
    list
}

fn push(mut list: Vec<Recent>, recent: Recent) -> Vec<Recent> {
    list.retain(|old| old.target.path != recent.target.path);
    list.insert(0, recent);
    list.truncate(MAX_RECENTS);
    list
}

/// Removes the saved trees that are not in `keep`, a list of cache file names. A blocking call,
/// for the background executor.
pub fn prune_scans(keep: &[String]) {
    if let Some(dir) = scans_dir() {
        prune(&dir, keep);
    }
}

fn prune(dir: &Path, keep: &[String]) {
    // No folder yet is the normal state before the first scan.
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kept = keep.iter().any(|name| entry.file_name() == name.as_str());
        if !kept
            && path.extension().is_some_and(|ext| ext == "vtree")
            && let Err(error) = fs::remove_file(&path)
        {
            log::warn!("could not remove {}: {error}", path.display());
        }
    }
}

fn file(base: &Path) -> PathBuf {
    base.join("vano").join("recents.json")
}

fn read(base: &Path) -> Vec<Recent> {
    let mut list: Vec<Recent> = fs::read_to_string(file(base))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // A list saved by a version that remembered more.
    list.truncate(MAX_RECENTS);
    list
}

fn write(base: &Path, list: &[Recent]) {
    let path = file(base);
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        log::warn!("could not create {}: {error}", parent.display());
        return;
    }
    match serde_json::to_string_pretty(list) {
        Ok(text) => {
            if let Err(error) = fs::write(&path, text) {
                log::warn!("could not save {}: {error}", path.display());
            }
        }
        Err(error) => log::warn!("could not serialize the recent scans: {error}"),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::state::target::TargetKind;

    fn recent(path: &str, size: u64) -> Recent {
        Recent {
            target: Target {
                kind: TargetKind::Folder,
                path: PathBuf::from(path),
            },
            size,
            files: 1,
            scanned_at: 0,
            seconds: 1.0,
            cache: "x".into(),
        }
    }

    #[test]
    fn newest_first_and_one_per_target() {
        let list = push(vec![recent("/a", 1), recent("/b", 2)], recent("/b", 3));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].size, 3);
        assert_eq!(list[1].target.path, PathBuf::from("/a"));
    }

    #[test]
    fn forgetting_leaves_the_other_targets() {
        let list = without(
            vec![recent("/a", 1), recent("/b", 2)],
            &Target::folder(PathBuf::from("/a")),
        );
        assert_eq!(list, vec![recent("/b", 2)]);
    }

    #[test]
    fn a_longer_saved_list_is_cut() {
        let dir = tempfile::tempdir().expect("tempdir");
        let long: Vec<Recent> = (0..MAX_RECENTS + 2)
            .map(|index| recent(&format!("/{index}"), 1))
            .collect();
        write(dir.path(), &long);
        assert_eq!(read(dir.path()), long[..MAX_RECENTS]);
    }

    #[test]
    fn pruning_only_removes_trees_out_of_the_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["kept.vtree", "old.vtree", "other.txt"] {
            fs::write(dir.path().join(name), "x").expect("write");
        }
        prune(dir.path(), &["kept.vtree".to_string()]);
        assert!(dir.path().join("kept.vtree").exists());
        assert!(!dir.path().join("old.vtree").exists());
        assert!(dir.path().join("other.txt").exists());
    }

    #[test]
    fn pruning_without_a_folder_does_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        prune(&dir.path().join("missing"), &[]);
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), &[recent("/a", 1)]);
        assert_eq!(read(dir.path()), vec![recent("/a", 1)]);
    }

    #[test]
    fn cache_name_is_stable() {
        let target = Target::folder(PathBuf::from("/home/u"));
        assert_eq!(cache_name(&target), cache_name(&target.clone()));
        assert!(cache_name(&target).ends_with(".vtree"));
    }
}
