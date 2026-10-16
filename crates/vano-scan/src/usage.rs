//! Capacity of the file system that holds a path.

use std::path::Path;

/// Total and free space of a file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskUsage {
    /// Size of the file system in bytes.
    pub total: u64,
    /// Bytes available to unprivileged users.
    pub available: u64,
    /// Bytes not used by any file. More than `available` when the file system keeps a reserve
    /// for the administrator (ext4 keeps 5% by default).
    pub free: u64,
}

impl DiskUsage {
    /// Bytes files take: everything that is not free.
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    /// Bytes reserved for the administrator: free, but not available to the user.
    pub fn reserved(&self) -> u64 {
        self.free.saturating_sub(self.available)
    }
}

/// Total, available and free bytes of the file system holding `path`, `None` if it cannot be asked.
#[cfg(unix)]
pub fn root_filesystem_usage(path: &Path) -> Option<DiskUsage> {
    let stat = rustix::fs::statvfs(path).ok()?;
    // Block counts are in units of the fragment size.
    let unit = if stat.f_frsize > 0 {
        stat.f_frsize
    } else {
        stat.f_bsize
    };
    Some(DiskUsage {
        total: stat.f_blocks.saturating_mul(unit),
        available: stat.f_bavail.saturating_mul(unit),
        free: stat.f_bfree.saturating_mul(unit),
    })
}

/// Not implemented off Unix: there is no safe API in the dependency set to ask.
#[cfg(not(unix))]
pub fn root_filesystem_usage(_path: &Path) -> Option<DiskUsage> {
    None
}
