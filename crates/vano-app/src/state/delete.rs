//! Deleting items for good, one entry at a time, so the batch reports progress and stops in the
//! middle of a big folder. Blocking calls, run on the background executor.
//!
//! The walk never follows a symbolic link and never enters another file system: it removes what
//! the scan counted and nothing it reaches by accident. Where the OS offers it, every step is
//! relative to the folder already open, so a folder swapped for a link in the meantime changes
//! nothing.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use vano_core::{NodeId, Tree};

use super::trash::{Outcome, TrashError, TrashProgress};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;

// Also built by the tests, so the Windows walk is exercised on every system.
#[cfg(any(not(unix), test))]
mod portable;
#[cfg(not(unix))]
use portable as platform;

/// One item being deleted: what stops it, what it reports, and how it went.
struct Walk<'a> {
    cancel: &'a AtomicBool,
    on_file: &'a mut dyn FnMut(u64),
    error: Option<String>,
    stopped: bool,
}

impl Walk<'_> {
    /// Whether to stop before the next entry.
    fn should_stop(&mut self) -> bool {
        self.stopped |= self.cancel.load(Ordering::Relaxed);
        self.stopped
    }

    /// One file gone, with the bytes it took.
    fn file_removed(&mut self, bytes: u64) {
        (self.on_file)(bytes);
    }

    /// Records a failure and goes on with the rest. The first one is what the report says.
    fn fail(&mut self, error: impl ToString) {
        self.error.get_or_insert_with(|| error.to_string());
    }
}

/// How deleting one item ended.
#[derive(Debug, PartialEq, Eq)]
enum ItemEnd {
    Gone,
    /// Stopped by the user: part of it may be gone already.
    Stopped,
    /// Something inside could not be deleted: the rest of it is gone.
    Failed(String),
}

fn delete_item(
    path: &Path,
    cancel: &AtomicBool,
    on_file: &mut dyn FnMut(u64),
    backend: fn(&Path, &mut Walk),
) -> ItemEnd {
    let mut walk = Walk {
        cancel,
        on_file,
        error: None,
        stopped: false,
    };
    backend(path, &mut walk);
    match (walk.error, walk.stopped) {
        (Some(error), _) => ItemEnd::Failed(error),
        (None, true) => ItemEnd::Stopped,
        (None, false) => ItemEnd::Gone,
    }
}

/// Deletes `paths` for good one at a time, counting files and bytes in `progress`. A failure only
/// affects its own item, and setting `progress.cancel` stops before the next entry.
pub fn delete_each(paths: &[std::path::PathBuf], progress: &TrashProgress) -> Outcome {
    let mut outcome = Outcome::default();
    let mut on_file = |bytes: u64| {
        progress.files.fetch_add(1, Ordering::Relaxed);
        progress.bytes.fetch_add(bytes, Ordering::Relaxed);
    };
    for (index, path) in paths.iter().enumerate() {
        if progress.cancel.load(Ordering::Relaxed) {
            outcome.cancelled = true;
            break;
        }
        match delete_item(path, &progress.cancel, &mut on_file, platform::delete_item) {
            ItemEnd::Gone => outcome.moved.push(index),
            ItemEnd::Stopped => {
                outcome.partial.push(index);
                outcome.cancelled = true;
                break;
            }
            // An error about something already gone means nothing is left to delete.
            ItemEnd::Failed(_) if path.symlink_metadata().is_err() => outcome.moved.push(index),
            ItemEnd::Failed(error) => outcome.failed.push((index, TrashError::Os(error))),
        }
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    outcome
}

/// The outermost entries under `node` that are no longer on disk: what a stopped or failed item
/// already lost, so the results show what is really left of it.
pub fn gone_under(tree: &Tree, node: NodeId) -> Vec<NodeId> {
    let mut gone = Vec::new();
    let mut pending = vec![node];
    while let Some(folder) = pending.pop() {
        for child in tree.children(folder) {
            if tree.node(child).has(vano_core::tree::FLAG_REMOVED) {
                continue;
            }
            match tree.path(child).symlink_metadata() {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => gone.push(child),
                Ok(_) if tree.node(child).is_dir() => pending.push(child),
                _ => {}
            }
        }
    }
    gone
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    type Backend = fn(&Path, &mut Walk);

    /// Every test runs on each walk this system can build.
    fn backends() -> Vec<Backend> {
        vec![
            portable::delete_item,
            #[cfg(unix)]
            unix::delete_item,
        ]
    }

    /// A folder with files in nested folders, returned with how many files it holds.
    fn tree_in(dir: &Path) -> (PathBuf, u64) {
        let item = dir.join("item");
        fs::create_dir_all(item.join("a/b")).unwrap();
        fs::create_dir_all(item.join("empty")).unwrap();
        for name in ["one", "a/two", "a/b/three", "a/b/four"] {
            fs::write(item.join(name), b"data").unwrap();
        }
        (item, 4)
    }

    fn run(path: &Path, backend: Backend) -> (ItemEnd, u64) {
        let mut files = 0;
        let end = delete_item(path, &AtomicBool::new(false), &mut |_| files += 1, backend);
        (end, files)
    }

    #[test]
    fn a_folder_goes_with_everything_inside() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let (item, count) = tree_in(dir.path());
            assert_eq!(run(&item, backend), (ItemEnd::Gone, count));
            assert!(item.symlink_metadata().is_err());
            assert!(dir.path().exists());
        }
    }

    #[test]
    fn a_single_file_goes() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("file");
            fs::write(&file, b"data").unwrap();
            assert_eq!(run(&file, backend), (ItemEnd::Gone, 1));
            assert!(!file.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_goes_but_what_it_points_to_stays() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let (item, _) = tree_in(dir.path());
            let outside = dir.path().join("outside");
            fs::create_dir(&outside).unwrap();
            fs::write(outside.join("precious"), b"data").unwrap();
            std::os::unix::fs::symlink(&outside, item.join("link")).unwrap();
            assert_eq!(run(&item, backend).0, ItemEnd::Gone);
            assert!(outside.join("precious").exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_chosen_itself_goes_but_not_its_target() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let outside = dir.path().join("outside");
            fs::create_dir(&outside).unwrap();
            fs::write(outside.join("precious"), b"data").unwrap();
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            assert_eq!(run(&link, backend).0, ItemEnd::Gone);
            assert!(link.symlink_metadata().is_err());
            assert!(outside.join("precious").exists());
        }
    }

    /// The Go module cache keeps its folders read only, and it is a Safe recommendation.
    #[cfg(unix)]
    #[test]
    fn read_only_folders_inside_are_deleted_on_unix() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let (item, _) = tree_in(dir.path());
        for folder in ["a/b", "a"] {
            fs::set_permissions(item.join(folder), fs::Permissions::from_mode(0o555)).unwrap();
        }
        assert_eq!(run(&item, unix::delete_item).0, ItemEnd::Gone);
        assert!(item.symlink_metadata().is_err());
    }

    /// The folder that holds the chosen item stays, so its permissions stay too.
    #[cfg(unix)]
    #[test]
    fn a_read_only_parent_of_the_item_is_left_as_it_was() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let (item, _) = tree_in(dir.path());
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let end = run(&item, unix::delete_item).0;
        assert_eq!(mode(dir.path()), 0o555);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(end, ItemEnd::Failed(_)), "{end:?}");
    }

    #[test]
    fn stopping_leaves_the_rest_in_place() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let (item, count) = tree_in(dir.path());
            let cancel = AtomicBool::new(false);
            let mut files = 0;
            let end = delete_item(
                &item,
                &cancel,
                &mut |_| {
                    files += 1;
                    cancel.store(true, Ordering::Relaxed);
                },
                backend,
            );
            assert_eq!(end, ItemEnd::Stopped);
            assert_eq!(files, 1);
            assert!(files < count);
            assert!(item.exists());
        }
    }

    #[test]
    fn a_missing_item_is_a_failure_of_its_own() {
        for backend in backends() {
            let dir = tempfile::tempdir().unwrap();
            let end = run(&dir.path().join("missing"), backend).0;
            assert!(matches!(end, ItemEnd::Failed(_)), "{end:?}");
        }
    }

    #[test]
    fn a_batch_counts_each_item_and_stops_between_them() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = ["a", "b", "c"]
            .iter()
            .map(|name| {
                let path = dir.path().join(name);
                fs::write(&path, b"data").unwrap();
                path
            })
            .collect();
        let progress = TrashProgress::default();
        let outcome = delete_each(&paths[..2], &progress);
        assert_eq!(outcome.moved, vec![0, 1]);
        assert!(!outcome.cancelled);
        assert_eq!(progress.files.load(Ordering::Relaxed), 2);
        assert_eq!(progress.done.load(Ordering::Relaxed), 2);

        progress.cancel.store(true, Ordering::Relaxed);
        let outcome = delete_each(&paths[2..], &progress);
        assert!(outcome.moved.is_empty());
        assert!(outcome.cancelled);
        assert!(paths[2].exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_batch_reports_what_it_could_not_delete_and_goes_on() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        let blocked = locked.join("file");
        fs::write(&blocked, b"data").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let other = dir.path().join("other");
        fs::write(&other, b"data").unwrap();
        let outcome = delete_each(&[blocked.clone(), other.clone()], &TrashProgress::default());
        let still_there = blocked.exists();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(outcome.moved, vec![1]);
        assert_eq!(outcome.failed.len(), 1);
        assert_eq!(outcome.failed[0].0, 0);
        assert!(still_there);
        assert!(!other.exists());
    }

    #[test]
    fn what_is_gone_under_a_node_is_found_at_its_outermost() {
        use vano_core::{EntryData, Kind, ROOT};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir_all(root.join("kept")).unwrap();
        fs::write(root.join("kept/file"), b"x").unwrap();
        let entry = |name: &str, kind| EntryData {
            name: name.into(),
            kind,
            size: 1,
            mtime: 0,
            flags: 0,
        };
        let mut tree = Tree::new(&root);
        tree.ensure_len(5);
        tree.set_children(
            ROOT,
            1,
            &[entry("kept", Kind::Dir), entry("lost", Kind::Dir)],
        );
        tree.set_children(
            1,
            3,
            &[entry("file", Kind::File), entry("missing", Kind::File)],
        );
        let gone = gone_under(&tree, ROOT);
        let paths: Vec<PathBuf> = gone.iter().map(|id| tree.path(*id)).collect();
        assert!(paths.contains(&root.join("lost")));
        assert!(paths.contains(&root.join("kept/missing")));
        assert_eq!(paths.len(), 2);
    }
}
