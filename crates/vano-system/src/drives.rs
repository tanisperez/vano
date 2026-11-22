//! The disks worth scanning: internal and external ones, without pseudo file systems, boot
//! partitions, snaps, container layers or the same disk seen twice.
//!
//! `sysinfo` lists the mounted file systems (on Linux it already leaves out `/proc`, `/sys`,
//! `tmpfs`, squashfs, and network file systems that could hang). [`select`] then keeps what a
//! person calls a disk, so the filtering is unit tested without the real mounts. Network drives
//! are not listed on any system: a scan over the network is slow, and `sysinfo` skips them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The kind of storage behind a disk, when the system tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// A solid state disk on NVMe (only told apart on Linux, by the device name).
    Nvme,
    Ssd,
    Hdd,
    Unknown,
}

impl Media {
    /// A short English name for the command line: `NVMe SSD`, `SSD`, `HDD` or `Disk`.
    pub fn name(self) -> &'static str {
        match self {
            Media::Nvme => "NVMe SSD",
            Media::Ssd => "SSD",
            Media::Hdd => "HDD",
            Media::Unknown => "Disk",
        }
    }
}

/// One disk, as the home screen and `vano drives` show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drive {
    /// Where its files are: `/`, `/run/media/ana/Backup`, `D:\`.
    pub mount_point: PathBuf,
    /// The volume label, when it has one: `Backup`, `Windows`.
    pub label: Option<String>,
    /// The device (`/dev/nvme0n1p2`) on Linux and macOS; on Windows the label, as `sysinfo` gives
    /// nothing else. Only used to tell two mounts of the same disk apart.
    pub device: String,
    /// `ext4`, `btrfs`, `APFS`, `NTFS`, `exfat`...
    pub file_system: String,
    pub media: Media,
    /// External: USB disks and memory cards.
    pub removable: bool,
    /// The disk the operating system runs from (`/`, `C:\`).
    pub system: bool,
    pub read_only: bool,
    /// Capacity in bytes.
    pub total: u64,
    /// Free bytes the user can still write.
    pub available: u64,
    /// Bytes no file uses. More than `available` when the file system keeps a reserve for the
    /// administrator (ext4 keeps 5%), which is neither free for the user nor taken by files.
    pub free: u64,
}

impl Drive {
    /// Bytes files take: everything that is not free. The administrator's reserve is not
    /// counted, as on the rest of Vano's screens.
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free.max(self.available))
    }

    /// The name to show: the label, or the mount point when there is none.
    pub fn display_name(&self) -> String {
        match &self.label {
            Some(label) => label.clone(),
            None => self.mount_point.display().to_string(),
        }
    }
}

/// The system whose rules [`select`] applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    /// The system this build runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// The connected disks, the system one first. Takes a few milliseconds: call it when the home
/// screen shows or the window gets the focus, never on a timer.
pub fn list_drives() -> Vec<Drive> {
    let platform = Platform::current();
    let disks = match sysinfo::Disks::new_with_refreshed_list() {
        Ok(disks) => disks,
        Err(error) => {
            log::warn!("cannot list the disks: {error}");
            return Vec::new();
        }
    };
    let labels = linux_labels();
    let system_drive = std::env::var("SystemDrive").ok();
    let raw = disks
        .iter()
        .map(|disk| {
            let mount_point = disk.mount_point().to_path_buf();
            let device = disk.name().to_string_lossy().into_owned();
            let label = match platform {
                Platform::Linux => labels.get(&device).cloned(),
                // macOS and Windows name the volume itself.
                _ => Some(device.clone()).filter(|name| !name.is_empty()),
            };
            let media = match disk.kind() {
                sysinfo::DiskKind::SSD if device.starts_with("/dev/nvme") => Media::Nvme,
                sysinfo::DiskKind::SSD => Media::Ssd,
                sysinfo::DiskKind::HDD => Media::Hdd,
                sysinfo::DiskKind::Unknown(_) => Media::Unknown,
            };
            let system = match platform {
                Platform::Windows => system_drive
                    .as_deref()
                    .is_some_and(|drive| is_windows_system(&mount_point, drive)),
                _ => mount_point == Path::new("/"),
            };
            Drive {
                label,
                device,
                file_system: disk.file_system().to_string_lossy().into_owned(),
                media,
                removable: disk.is_removable(),
                system,
                read_only: disk.is_read_only(),
                total: disk.total_space(),
                available: disk.available_space(),
                free: disk.available_space(),
                mount_point,
            }
        })
        .collect();
    let mut drives = select(raw, platform);
    // The same measure as the rest of the app, which tells the reserve apart.
    for drive in &mut drives {
        if let Some(usage) = vano_scan::root_filesystem_usage(&drive.mount_point) {
            drive.total = usage.total;
            drive.available = usage.available;
            drive.free = usage.free;
        }
    }
    drives
}

/// Whether `mount_point` is the root of the Windows system drive (`SystemDrive`, as `C:`).
fn is_windows_system(mount_point: &Path, system_drive: &str) -> bool {
    let mount = mount_point.to_string_lossy();
    mount
        .trim_end_matches(['\\', '/'])
        .eq_ignore_ascii_case(system_drive)
}

/// File systems that hold no files of their own: container layers, kernel interfaces, desktop
/// portals and FUSE mounts of remote or virtual files (`fuseblk`, the FUSE drivers of NTFS and
/// exFAT disks, is kept).
fn is_pseudo_linux_fs(fs: &str) -> bool {
    matches!(
        fs,
        "overlay"
            | "ramfs"
            | "tmpfs"
            | "tracefs"
            | "debugfs"
            | "securityfs"
            | "efivarfs"
            | "bpf"
            | "configfs"
            | "fusectl"
            | "binfmt_misc"
            | "nsfs"
            | "autofs"
            | "zram"
    ) || fs.starts_with("fuse.")
}

/// Linux mount points that are not a disk to clean up: boot partitions, snaps and container
/// storage.
fn is_hidden_linux_mount(mount: &Path) -> bool {
    const HIDDEN: [&str; 6] = [
        "/boot",
        "/efi",
        "/snap",
        "/var/snap",
        "/var/lib/docker",
        "/var/lib/containers",
    ];
    HIDDEN.iter().any(|hidden| mount.starts_with(hidden))
}

/// Keeps the disks a person would pick, from everything `sysinfo` lists.
pub fn select(raw: Vec<Drive>, platform: Platform) -> Vec<Drive> {
    let mut kept: Vec<Drive> = raw
        .into_iter()
        .filter(|drive| drive.total > 0)
        .filter(|drive| match platform {
            Platform::Linux => {
                !is_pseudo_linux_fs(&drive.file_system)
                    && !is_hidden_linux_mount(&drive.mount_point)
            }
            // APFS shows one container as several volumes (Data, VM, Preboot...). The root and
            // what is under /Volumes are the ones people know.
            Platform::MacOs => {
                drive.mount_point == Path::new("/") || drive.mount_point.starts_with("/Volumes")
            }
            Platform::Windows => true,
        })
        .collect();

    // The same disk seen twice (btrfs subvolumes, bind mounts): keep its shortest mount point.
    if platform != Platform::Windows {
        let mut best: HashMap<String, usize> = HashMap::new();
        for (index, drive) in kept.iter().enumerate() {
            let entry = best.entry(drive.device.clone()).or_insert(index);
            let current = &kept[*entry];
            if mount_depth(&drive.mount_point) < mount_depth(&current.mount_point) {
                *entry = index;
            }
        }
        let keep: Vec<usize> = best.into_values().collect();
        kept = kept
            .into_iter()
            .enumerate()
            .filter(|(index, _)| keep.contains(index))
            .map(|(_, drive)| drive)
            .collect();
    }

    kept.sort_by(|a, b| {
        b.system
            .cmp(&a.system)
            .then(a.removable.cmp(&b.removable))
            .then(a.mount_point.cmp(&b.mount_point))
    });
    kept
}

fn mount_depth(path: &Path) -> usize {
    path.components().count()
}

/// Device path to volume label, from `/dev/disk/by-label` (empty elsewhere or when missing).
fn linux_labels() -> HashMap<String, String> {
    let mut labels = HashMap::new();
    if !cfg!(target_os = "linux") {
        return labels;
    }
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-label") else {
        return labels;
    };
    for entry in entries.flatten() {
        let Ok(device) = std::fs::canonicalize(entry.path()) else {
            continue;
        };
        let label = decode_udev(&entry.file_name().to_string_lossy());
        labels.insert(device.to_string_lossy().into_owned(), label);
    }
    labels
}

/// Undoes udev's escaping of a label in a link name: `My\x20Disk` is `My Disk`.
fn decode_udev(name: &str) -> String {
    let mut bytes = Vec::with_capacity(name.len());
    let raw = name.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'\\'
            && raw.get(index + 1) == Some(&b'x')
            && let Some(hex) = name.get(index + 2..index + 4)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            bytes.push(byte);
            index += 4;
            continue;
        }
        bytes.push(raw[index]);
        index += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn drive(mount: &str, device: &str, fs: &str) -> Drive {
        Drive {
            mount_point: PathBuf::from(mount),
            label: None,
            device: device.to_string(),
            file_system: fs.to_string(),
            media: Media::Ssd,
            removable: false,
            system: mount == "/",
            read_only: false,
            total: 100,
            available: 40,
            free: 40,
        }
    }

    fn mounts(drives: &[Drive]) -> Vec<String> {
        drives
            .iter()
            .map(|drive| drive.mount_point.display().to_string())
            .collect()
    }

    #[test]
    fn linux_leaves_out_boot_partitions_snaps_and_container_layers() {
        let raw = vec![
            drive("/", "/dev/nvme0n1p2", "ext4"),
            drive("/boot", "/dev/nvme0n1p1", "vfat"),
            drive("/boot/efi", "/dev/nvme0n1p1", "vfat"),
            drive("/snap/core/1", "/dev/loop0", "ext4"),
            drive("/var/lib/docker/overlay2/x", "overlay", "overlay"),
            drive("/run/user/1000/doc", "portal", "fuse.portal"),
            drive("/mnt/data", "/dev/sda1", "ext4"),
        ];
        assert_eq!(
            mounts(&select(raw, Platform::Linux)),
            vec!["/", "/mnt/data"]
        );
    }

    #[test]
    fn btrfs_subvolumes_of_one_disk_show_once_at_the_shortest_mount() {
        let raw = vec![
            drive("/home", "/dev/nvme0n1p2", "btrfs"),
            drive("/", "/dev/nvme0n1p2", "btrfs"),
            drive("/var/log", "/dev/nvme0n1p2", "btrfs"),
        ];
        assert_eq!(mounts(&select(raw, Platform::Linux)), vec!["/"]);
    }

    #[test]
    fn ntfs_and_exfat_through_fuse_are_still_disks() {
        let raw = vec![drive("/run/media/ana/Backup", "/dev/sdb1", "fuseblk")];
        assert_eq!(
            mounts(&select(raw, Platform::Linux)),
            vec!["/run/media/ana/Backup"]
        );
    }

    #[test]
    fn macos_keeps_the_root_and_the_volumes_only() {
        let raw = vec![
            drive("/", "/dev/disk3s1s1", "apfs"),
            drive("/System/Volumes/Data", "/dev/disk3s5", "apfs"),
            drive("/System/Volumes/VM", "/dev/disk3s6", "apfs"),
            drive("/Volumes/T7 Shield", "/dev/disk5s1", "apfs"),
        ];
        assert_eq!(
            mounts(&select(raw, Platform::MacOs)),
            vec!["/", "/Volumes/T7 Shield"]
        );
    }

    #[test]
    fn the_system_disk_comes_first_then_internal_then_external() {
        let mut usb = drive("/a-usb", "/dev/sdc1", "exfat");
        usb.removable = true;
        let raw = vec![
            usb,
            drive("/mnt/data", "/dev/sda1", "ext4"),
            drive("/", "/dev/nvme0n1p2", "ext4"),
        ];
        assert_eq!(
            mounts(&select(raw, Platform::Linux)),
            vec!["/", "/mnt/data", "/a-usb"]
        );
    }

    #[test]
    fn an_empty_disk_reader_is_not_listed() {
        let mut empty = drive("/run/media/ana/CARD", "/dev/mmcblk0p1", "vfat");
        empty.total = 0;
        assert!(select(vec![empty], Platform::Linux).is_empty());
    }

    #[test]
    fn the_windows_system_drive_matches_with_or_without_its_backslash() {
        assert!(is_windows_system(Path::new("C:\\"), "C:"));
        assert!(is_windows_system(Path::new("c:"), "C:"));
        assert!(!is_windows_system(Path::new("D:\\"), "C:"));
    }

    #[test]
    fn udev_labels_are_decoded() {
        assert_eq!(decode_udev("My\\x20Disk"), "My Disk");
        assert_eq!(decode_udev("Ventoy"), "Ventoy");
        assert_eq!(decode_udev("a\\x2"), "a\\x2");
    }

    #[test]
    fn used_space_never_goes_below_zero() {
        let mut odd = drive("/", "/dev/sda1", "ext4");
        odd.available = 200;
        assert_eq!(odd.used(), 0);
    }

    #[test]
    fn the_reserve_of_the_administrator_is_not_used_space() {
        let mut ext4 = drive("/", "/dev/sda1", "ext4");
        ext4.available = 40;
        ext4.free = 45;
        assert_eq!(ext4.used(), 55);
    }
}
