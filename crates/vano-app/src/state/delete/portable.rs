//! Deletion through the standard library, for Windows. `symlink_metadata` calls a junction or a
//! mount point a link too, so the walk never enters one: it only removes the link itself.

use std::fs;
use std::io;
use std::path::Path;

use super::Walk;

pub(super) fn delete_item(path: &Path, walk: &mut Walk) {
    remove_entry(path, walk);
}

fn remove_entry(path: &Path, walk: &mut Walk) {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    let kind = metadata.file_type();
    if kind.is_symlink() {
        // A link to a folder is removed as a folder on Windows, as a file elsewhere.
        match fs::remove_file(path).or_else(|_| fs::remove_dir(path)) {
            Ok(()) => walk.file_removed(0),
            Err(error) => walk.fail(error),
        }
    } else if kind.is_dir() {
        remove_folder(path, walk);
    } else {
        match retrying_read_only(path, |path| fs::remove_file(path)) {
            Ok(()) => walk.file_removed(metadata.len()),
            Err(error) => walk.fail(error),
        }
    }
}

fn remove_folder(path: &Path, walk: &mut Walk) {
    // Read in full first: removing entries while reading a folder may skip some.
    let entries: io::Result<Vec<_>> = match fs::read_dir(path) {
        Ok(entries) => entries.collect(),
        Err(error) => Err(error),
    };
    let entries = match entries {
        Ok(entries) => entries,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    for entry in entries {
        if walk.should_stop() {
            return;
        }
        remove_entry(&entry.path(), walk);
    }
    if walk.should_stop() {
        return;
    }
    if let Err(error) = retrying_read_only(path, |path| fs::remove_dir(path)) {
        walk.fail(error);
    }
}

/// Windows refuses to remove an entry with the read only attribute: clears it and tries again.
fn retrying_read_only(path: &Path, remove: impl Fn(&Path) -> io::Result<()>) -> io::Result<()> {
    match remove(path) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let mut permissions = path.symlink_metadata()?.permissions();
            if !permissions.readonly() {
                return Err(error);
            }
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions)?;
            remove(path)
        }
        other => other,
    }
}
