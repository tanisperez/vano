//! Unix directory reading: `getdents` through rustix, then `fstatat` relative to the directory
//! descriptor so the kernel never resolves a long path for each entry.

use std::ffi::CStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, Stat, statat};
use rustix::io::Errno;
use vano_core::tree::{EntryData, FLAG_COMPLETE, FLAG_HARDLINK_DUP, FLAG_UNREADABLE, Kind};

use super::{Ctx, DirInfo, Listing, SKIPPED};

/// Top level directories of `/` that are virtual or volatile, skipped even when the scan may
/// cross file systems.
const ROOT_SKIPS: [&[u8]; 4] = [b"proc", b"sys", b"dev", b"run"];

pub(super) fn stat_root(path: &Path) -> Option<(u64, u32)> {
    let stat = statat(CWD, path, AtFlags::empty()).ok()?;
    Some((stat.st_dev as u64, mtime_of(&stat)))
}

fn mtime_of(stat: &Stat) -> u32 {
    stat.st_mtime.clamp(0, i64::from(u32::MAX)) as u32
}

fn kind_of_dtype(file_type: FileType) -> Kind {
    match file_type {
        FileType::Directory => Kind::Dir,
        FileType::RegularFile => Kind::File,
        FileType::Symlink => Kind::Symlink,
        _ => Kind::Other,
    }
}

pub(super) fn read_directory(path: &Path, info: DirInfo, ctx: &Ctx) -> Listing {
    let mut listing = Listing::default();
    let mut flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    if !info.follow {
        flags |= OFlags::NOFOLLOW;
    }
    let opened = rustix::fs::openat(CWD, path, flags, Mode::empty()).and_then(Dir::new);
    let mut dir = match opened {
        Ok(dir) => dir,
        Err(err) => {
            log::debug!("cannot open {}: {err}", path.display());
            listing.unreadable = true;
            listing.errors = 1;
            return listing;
        }
    };

    // First pass: names and d_type only, so the stat calls below can borrow the descriptor.
    // Names go in one NUL separated arena to avoid an allocation per entry here.
    let mut arena: Vec<u8> = Vec::with_capacity(4096);
    let mut raw: Vec<(u32, FileType)> = Vec::new();
    while let Some(item) = dir.read() {
        match item {
            Ok(entry) => {
                let bytes = entry.file_name().to_bytes();
                if bytes == b"." || bytes == b".." {
                    continue;
                }
                raw.push((arena.len() as u32, entry.file_type()));
                arena.extend_from_slice(bytes);
                arena.push(0);
            }
            Err(err) => {
                log::debug!("read error in {}: {err}", path.display());
                listing.unreadable = true;
                listing.errors += 1;
                break;
            }
        }
    }
    let Ok(dirfd) = dir.fd() else {
        listing.unreadable = true;
        listing.errors += 1;
        return listing;
    };

    let is_fs_root = path.as_os_str().as_bytes() == b"/";
    listing.entries.reserve(raw.len());
    for (start, d_type) in raw {
        let Ok(name) = CStr::from_bytes_until_nul(&arena[start as usize..]) else {
            continue;
        };
        let stat = match statat(dirfd, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            // Vanished between listing and stat: not an error worth reporting.
            Err(Errno::NOENT) => continue,
            Err(err) => {
                log::debug!("cannot stat {name:?}: {err}");
                listing.unreadable = true;
                listing.errors += 1;
                let kind = kind_of_dtype(d_type);
                listing.entries.push(EntryData {
                    name: name.to_bytes().to_vec(),
                    kind,
                    size: 0,
                    mtime: 0,
                    flags: FLAG_UNREADABLE | if kind == Kind::Dir { FLAG_COMPLETE } else { 0 },
                });
                continue;
            }
        };
        let kind = kind_of_dtype(FileType::from_raw_mode(stat.st_mode as _));
        let dev = stat.st_dev as u64;
        let mut entry = EntryData {
            name: name.to_bytes().to_vec(),
            kind,
            size: 0,
            mtime: mtime_of(&stat),
            flags: 0,
        };
        if kind == Kind::Dir {
            let skip = (is_fs_root && ROOT_SKIPS.contains(&name.to_bytes()))
                || (dev != info.dev && (ctx.one_file_system || pseudo_fs(path, name)));
            if skip {
                entry.flags = SKIPPED;
            } else {
                listing.descend.push((listing.entries.len() as u32, dev));
            }
        } else {
            entry.size = (stat.st_blocks as u64).saturating_mul(512);
            if stat.st_nlink > 1 && !ctx.hardlinks.first_seen(dev, stat.st_ino as u64) {
                entry.size = 0;
                entry.flags = FLAG_HARDLINK_DUP;
            }
        }
        listing.entries.push(entry);
    }
    listing
}

/// Whether the mount point `path/name` is a virtual file system (proc, sysfs, cgroups...).
#[cfg(target_os = "linux")]
fn pseudo_fs(path: &Path, name: &CStr) -> bool {
    use std::ffi::OsStr;

    // Magic numbers from `linux/magic.h`.
    const PSEUDO: [u64; 16] = [
        0x0000_9fa0, // proc
        0x6265_6572, // sysfs
        0x0027_e0eb, // cgroup
        0x6367_7270, // cgroup2
        0x6462_6720, // debugfs
        0x7472_6163, // tracefs
        0x7363_6673, // securityfs
        0xf97c_ff8c, // selinuxfs
        0x6165_676c, // pstore
        0xcafe_4a11, // bpf
        0x0000_1cd1, // devpts
        0x6573_5543, // fusectl
        0x4249_4e4d, // binfmt_misc
        0x6265_6570, // configfs
        0x1980_0202, // mqueue
        0x6e73_6673, // nsfs
    ];
    let full = path.join(OsStr::from_bytes(name.to_bytes()));
    match rustix::fs::statfs(&full) {
        Ok(fs) => PSEUDO.contains(&(fs.f_type as u64)),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
fn pseudo_fs(_path: &Path, _name: &CStr) -> bool {
    false
}
