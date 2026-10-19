//! A Linux root reached from Windows, such as a WSL2 distribution at `\\wsl.localhost\Ubuntu` or
//! a network drive mapped to it. Windows sees one file system there, so without help the scan
//! would enter `/proc` (whose `kcore` claims terabytes), `/sys`, `/dev`, `/run`, and `/mnt/c`,
//! which is the Windows disk itself again. On Linux the scanner tells file systems apart by device;
//! here the distribution's own mount table says where the other file systems are.
#![cfg_attr(unix, allow(dead_code))]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The mount tables to try, in order. `proc/mounts` and `proc/self/mounts` are symlinks, which a
/// network share may not follow; `proc/1/mounts` is init's own table, a plain file.
const MOUNT_TABLES: [&str; 3] = ["proc/1/mounts", "proc/mounts", "proc/self/mounts"];

/// Directories never worth entering when the mount table cannot be read.
const FALLBACK: [&str; 6] = ["proc", "sys", "dev", "run", "mnt", "usr/lib/wsl"];

/// The directories under `root` not to enter, when `root` is the root of a Linux system: every
/// mount point but `/` itself. `None` when `root` does not look like one.
pub(crate) fn foreign_mounts(root: &Path) -> Option<HashSet<PathBuf>> {
    if !looks_like_root(root) {
        return None;
    }
    let points = MOUNT_TABLES
        .iter()
        .filter_map(|table| std::fs::read_to_string(root.join(table)).ok())
        .map(|text| mount_points(&text))
        .find(|points| !points.is_empty())
        .unwrap_or_else(|| FALLBACK.iter().map(|point| format!("/{point}")).collect());
    log::info!(
        "{} is a Linux root: {} mount points are not entered",
        root.display(),
        points.len()
    );
    Some(points.iter().map(|point| under(root, point)).collect())
}

/// Whether `path` has the `proc` and `etc` folders of a Linux root.
fn looks_like_root(path: &Path) -> bool {
    path.join("proc").is_dir() && path.join("etc").is_dir()
}

/// The closest ancestor of `path` (or `path` itself) that is the root of a Linux system, such as
/// the WSL2 distribution holding it.
pub fn linux_root_of(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| looks_like_root(ancestor))
        .map(Path::to_path_buf)
}

/// The mount points of a `/proc/mounts` table, except `/`.
fn mount_points(table: &str) -> Vec<String> {
    table
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(unescape)
        .filter(|point| point.starts_with('/') && point != "/")
        .collect()
}

/// Undoes the octal escapes of the mount table (`\040` for a space, `\134` for a backslash).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && let Some(code) = bytes
                .get(i + 1..i + 4)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 8).ok())
        {
            out.push(code);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `point` (an absolute Linux path) under `root`, one component at a time, so the separators
/// match the paths the scan builds.
fn under(root: &Path, point: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for component in point.split('/').filter(|component| !component.is_empty()) {
        path.push(component);
    }
    path
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The start of a WSL2 Ubuntu 24.04 table.
    const WSL: &str = r"none /usr/lib/modules/6.6.36.6-microsoft-standard-WSL2 overlay rw,nosuid 0 0
none /mnt/wsl tmpfs rw,relatime 0 0
drivers /usr/lib/wsl/drivers 9p ro,nosuid,nodev,aname=drivers 0 0
/dev/sdc / ext4 rw,relatime,discard,errors=remount-ro 0 0
none /mnt/wslg tmpfs rw,relatime 0 0
/dev/sdc /mnt/wslg/distro ext4 ro,relatime,discard 0 0
none /dev devtmpfs rw,nosuid,relatime,mode=755 0 0
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
none /run tmpfs rw,nosuid,nodev,mode=755 0 0
C:\134 /mnt/c 9p rw,noatime,dirsync,aname=drvfs;path=C:\;uid=1000 0 0
/dev/sdd /mnt/my\040disk ext4 rw 0 0
";

    #[test]
    fn every_mount_point_but_the_root_is_listed() {
        let points = mount_points(WSL);
        assert!(points.contains(&"/proc".to_string()));
        assert!(points.contains(&"/mnt/c".to_string()));
        assert!(points.contains(&"/mnt/wslg/distro".to_string()));
        assert!(points.contains(&"/mnt/my disk".to_string()));
        assert!(!points.contains(&"/".to_string()));
        assert_eq!(points.len(), 11);
    }

    #[test]
    fn a_linux_root_skips_its_mounts_and_another_folder_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("proc/1")).unwrap();
        std::fs::create_dir(root.join("etc")).unwrap();
        std::fs::write(root.join("proc/1/mounts"), WSL).unwrap();
        let skip = foreign_mounts(root).unwrap();
        assert!(skip.contains(&root.join("proc")));
        assert!(skip.contains(&root.join("mnt").join("c")));
        assert!(!skip.contains(&root.join("home")));

        let other = tempfile::tempdir().unwrap();
        assert!(foreign_mounts(other.path()).is_none());
    }

    #[test]
    fn the_root_of_a_path_is_its_closest_linux_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("proc")).unwrap();
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::create_dir_all(root.join("home/user/app")).unwrap();
        assert_eq!(
            linux_root_of(&root.join("home/user/app")),
            Some(root.to_path_buf())
        );
        assert_eq!(linux_root_of(root), Some(root.to_path_buf()));
    }

    #[test]
    fn without_a_readable_table_the_usual_suspects_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("proc")).unwrap();
        std::fs::create_dir(root.join("etc")).unwrap();
        let skip = foreign_mounts(root).unwrap();
        assert!(skip.contains(&root.join("proc")));
        assert!(skip.contains(&root.join("mnt")));
        assert!(skip.contains(&root.join("usr").join("lib").join("wsl")));
    }
}
