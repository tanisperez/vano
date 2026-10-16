//! What a worker produces for one directory, and the shared settings it reads with.

use std::path::Path;

use vano_core::tree::EntryData;

use crate::hardlinks::Hardlinks;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;

#[cfg(not(unix))]
mod portable;
#[cfg(not(unix))]
use portable as platform;

/// Settings shared by every worker.
pub(crate) struct Ctx {
    /// Stay on the file system of the root.
    pub(crate) one_file_system: bool,
    /// Multiply linked files already counted.
    pub(crate) hardlinks: Hardlinks,
}

/// The result of reading one directory.
#[derive(Default)]
pub(crate) struct Listing {
    /// The entries, in reading order.
    pub(crate) entries: Vec<EntryData>,
    /// Index into `entries` and device of each subdirectory that must be read next.
    pub(crate) descend: Vec<(u32, u64)>,
    /// The directory (or part of it) could not be read.
    pub(crate) unreadable: bool,
    /// Errors met while reading.
    pub(crate) errors: u32,
}

/// What a directory job knows about the directory it reads.
#[derive(Clone, Copy)]
pub(crate) struct DirInfo {
    /// Device of the directory itself (0 where devices do not exist).
    pub(crate) dev: u64,
    /// Whether the path may be a symlink to follow (only the scan root).
    pub(crate) follow: bool,
}

/// Reads `path` and describes its entries.
pub(crate) fn read_directory(path: &Path, info: DirInfo, ctx: &Ctx) -> Listing {
    platform::read_directory(path, info, ctx)
}

/// The device and modification time of the scan root, `None` if it cannot be read.
pub(crate) fn stat_root(path: &Path) -> Option<(u64, u32)> {
    platform::stat_root(path)
}

/// A descendant directory that must not be entered, with the flag that explains why.
pub(crate) const SKIPPED: u8 = vano_core::tree::FLAG_OTHER_FS | vano_core::tree::FLAG_COMPLETE;
