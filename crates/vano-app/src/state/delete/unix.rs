//! Unix deletion: every step is relative to a folder already open, with `O_NOFOLLOW`, so nothing
//! can redirect the walk through a link.

// The types of the `stat` fields change between Linux and macOS.
#![allow(clippy::unnecessary_cast)]

use std::ffi::{CStr, CString, OsStr};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

use rust_i18n::t;
use rustix::fs::{
    AtFlags, CWD, Dir, FileType, Mode, OFlags, Stat, fchmod, fstat, openat, statat, unlinkat,
};
use rustix::io::Errno;

use super::Walk;

/// Owner read, write and search: what emptying a folder takes.
const OWNER_ALL: u32 = 0o700;
/// `st_blocks` counts in units of 512 bytes, whatever the file system's block size.
const STAT_BLOCK: u64 = 512;

pub(super) fn delete_item(path: &Path, walk: &mut Walk) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        walk.fail(Errno::INVAL);
        return;
    };
    let Ok(name) = c_name(name) else {
        walk.fail(Errno::INVAL);
        return;
    };
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let parent = match openat(CWD, parent, flags, Mode::empty()) {
        Ok(parent) => parent,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    let stat = match statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    // The folder that holds the item stays, so it keeps its permissions.
    remove_entry(
        parent.as_fd(),
        &name,
        &stat,
        stat.st_dev as u64,
        false,
        walk,
    );
}

fn c_name(name: &OsStr) -> Result<CString, std::ffi::NulError> {
    CString::new(name.as_bytes())
}

/// Removes the entry `name` of `parent`, with everything inside when it is a folder.
fn remove_entry(
    parent: BorrowedFd,
    name: &CStr,
    stat: &Stat,
    device: u64,
    fix_parent: bool,
    walk: &mut Walk,
) {
    if FileType::from_raw_mode(stat.st_mode as _) != FileType::Directory {
        match unlink(parent, name, AtFlags::empty(), fix_parent) {
            Ok(()) => walk.file_removed((stat.st_blocks as u64).saturating_mul(STAT_BLOCK)),
            Err(error) => walk.fail(error),
        }
        return;
    }
    // A mount point: what is there belongs to another file system the scan did not count.
    if stat.st_dev as u64 != device {
        walk.fail(t!("report.other_file_system"));
        return;
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let folder = match openat(parent, name, flags, Mode::empty()) {
        Ok(folder) => folder,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    remove_children(&folder, device, walk);
    if walk.stopped {
        return;
    }
    drop(folder);
    if let Err(error) = unlink(parent, name, AtFlags::REMOVEDIR, fix_parent) {
        walk.fail(error);
    }
}

fn remove_children(folder: &OwnedFd, device: u64, walk: &mut Walk) {
    let names = match read_names(folder) {
        Ok(names) => names,
        Err(error) => {
            walk.fail(error);
            return;
        }
    };
    for name in names {
        if walk.should_stop() {
            return;
        }
        match statat(folder, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => remove_entry(folder.as_fd(), &name, &stat, device, true, walk),
            // Gone in the meantime: that is what was wanted.
            Err(Errno::NOENT) => {}
            Err(error) => walk.fail(error),
        }
    }
}

/// The folder's entries, read in full before any of them goes: removing entries while reading a
/// folder may skip some.
fn read_names(folder: &OwnedFd) -> Result<Vec<CString>, Errno> {
    let mut names = Vec::new();
    for entry in Dir::read_from(folder)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

/// `unlinkat`, opening up a read only folder when `fix_parent` allows it: the folder is about to
/// go too. Only through its open descriptor, which cannot be a link.
fn unlink(parent: BorrowedFd, name: &CStr, flags: AtFlags, fix_parent: bool) -> Result<(), Errno> {
    match unlinkat(parent, name, flags) {
        Err(Errno::ACCESS) if fix_parent => {
            let mode = fstat(parent)?.st_mode as u32;
            fchmod(
                parent,
                Mode::from_raw_mode((mode & 0o7777 | OWNER_ALL) as _),
            )?;
            unlinkat(parent, name, flags)
        }
        Err(Errno::NOENT) => Ok(()),
        other => other,
    }
}
