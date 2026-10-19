//! Directory reading where raw descriptors are not available (Windows and others): plain
//! `std::fs::read_dir`. On Windows `DirEntry::metadata` comes from the directory enumeration
//! itself, so it costs no extra system call.

use std::path::Path;
use std::time::UNIX_EPOCH;

use vano_core::tree::{EntryData, Kind};

use super::{Ctx, DirInfo, Listing, SKIPPED};

pub(super) fn stat_root(path: &Path) -> Option<(u64, u32)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((0, mtime_of(&meta)))
}

fn mtime_of(meta: &std::fs::Metadata) -> u32 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs().min(u64::from(u32::MAX)) as u32)
}

pub(super) fn read_directory(path: &Path, _info: DirInfo, ctx: &Ctx) -> Listing {
    let mut listing = Listing::default();
    let read = match std::fs::read_dir(path) {
        Ok(read) => read,
        Err(err) => {
            log::debug!("cannot open {}: {err}", path.display());
            listing.unreadable = true;
            listing.errors = 1;
            return listing;
        }
    };
    for item in read {
        let Ok(item) = item else {
            listing.unreadable = true;
            listing.errors += 1;
            continue;
        };
        let name = item.file_name().to_string_lossy().into_owned().into_bytes();
        let Ok(meta) = item.metadata() else {
            listing.unreadable = true;
            listing.errors += 1;
            continue;
        };
        let file_type = meta.file_type();
        // `metadata` does not follow links; reparse points are treated as links, never entered.
        let kind = if file_type.is_symlink() {
            Kind::Symlink
        } else if file_type.is_dir() {
            Kind::Dir
        } else if file_type.is_file() {
            Kind::File
        } else {
            Kind::Other
        };
        let mut flags = 0;
        if kind == Kind::Dir {
            if ctx.skip.contains(&item.path()) {
                flags = SKIPPED;
            } else {
                listing.descend.push((listing.entries.len() as u32, 0));
            }
        }
        listing.entries.push(EntryData {
            name,
            kind,
            size: if kind == Kind::Dir { 0 } else { meta.len() },
            mtime: mtime_of(&meta),
            flags,
        });
    }
    listing
}
