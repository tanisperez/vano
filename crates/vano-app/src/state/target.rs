//! What a scan reads: the home folder, the whole system disk, another disk, a WSL 2 distribution,
//! or a folder the user picks.

use std::path::{Path, PathBuf};

use rust_i18n::t;
use serde::{Deserialize, Serialize};
use vano_system::Drive;

/// The kind of place a scan starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    #[default]
    Home,
    /// The disk the system runs from.
    Disk,
    /// Any other disk, internal or external.
    Drive,
    /// A WSL 2 distribution, scanned from inside (Windows only).
    Wsl,
    Folder,
}

/// A place to scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub kind: TargetKind,
    /// Where it is. For a WSL distribution, `\\wsl.localhost\<name>`: only an id, the scan runs
    /// inside the distribution from `/`.
    pub path: PathBuf,
    /// The name of a disk (its label) or of a WSL distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The home folder inside a WSL distribution, for the rules that look under `~`. Known once
    /// it has been scanned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,
}

impl Target {
    fn new(kind: TargetKind, path: PathBuf) -> Self {
        Target {
            kind,
            path,
            name: None,
            home: None,
        }
    }

    /// The user's home folder, `None` when the OS cannot tell.
    pub fn home() -> Option<Self> {
        dirs::home_dir().map(|path| Self::new(TargetKind::Home, path))
    }

    /// The disk the system runs from: `/` on Unix, the system drive on Windows.
    pub fn disk() -> Self {
        Self::new(TargetKind::Disk, system_root())
    }

    /// A disk the system lists: the system one is [`Target::disk`].
    pub fn drive(drive: &Drive) -> Self {
        if drive.system || drive.mount_point == system_root() {
            return Self::disk();
        }
        Target {
            name: drive.label.clone(),
            ..Self::new(TargetKind::Drive, drive.mount_point.clone())
        }
    }

    /// A WSL 2 distribution.
    pub fn wsl(name: &str) -> Self {
        Target {
            name: Some(name.to_string()),
            ..Self::new(
                TargetKind::Wsl,
                PathBuf::from(format!(r"\\wsl.localhost\{name}")),
            )
        }
    }

    /// A folder the user picked. Picking the system root is the same as picking the disk.
    pub fn folder(path: PathBuf) -> Self {
        if path == system_root() {
            return Self::disk();
        }
        Self::new(TargetKind::Folder, path)
    }

    /// Whether `self` and `other` are the same place, whatever else they remember.
    pub fn same_place(&self, other: &Target) -> bool {
        self.kind == other.kind && self.path == other.path
    }

    /// The name shown in the title bar, the scan button and the recent list.
    pub fn label(&self) -> String {
        match self.kind {
            TargetKind::Home => t!("target.home").into_owned(),
            TargetKind::Disk => disk_name(),
            TargetKind::Drive => drive_label(self.name.as_deref(), &self.path),
            TargetKind::Wsl => self.name.clone().unwrap_or_default(),
            TargetKind::Folder => self
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.display().to_string()),
        }
    }

    /// The name of the root tile of the explorer: `~ Home`, the disk name, or the folder name.
    pub fn root_label(&self) -> String {
        match self.kind {
            TargetKind::Home => format!("~ {}", t!("target.home_short")),
            _ => self.label(),
        }
    }
}

/// The name of a disk that is not the system one: `Backup (F:)` on Windows, the label or the
/// mount point elsewhere.
pub fn drive_label(label: Option<&str>, mount: &Path) -> String {
    let mount_text = mount.display().to_string();
    if cfg!(windows) {
        let letter = mount_text.trim_end_matches('\\');
        return match label.filter(|label| !label.is_empty()) {
            Some(label) => format!("{label} ({letter})"),
            None => format!("{} ({letter})", t!("target.local_disk")),
        };
    }
    match label.filter(|label| !label.is_empty()) {
        Some(label) => label.to_string(),
        None => mount
            .file_name()
            .map_or(mount_text, |name| name.to_string_lossy().into_owned()),
    }
}

/// The name of the system disk, as file managers call it.
pub fn disk_name() -> String {
    if cfg!(target_os = "macos") {
        "Macintosh HD".to_string()
    } else if cfg!(windows) {
        format!("{} ({})", t!("target.local_disk"), system_root().display())
    } else {
        t!("target.file_system").into_owned()
    }
}

/// `/`, or `C:\` (the system drive) on Windows.
pub fn system_root() -> PathBuf {
    if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        PathBuf::from(format!("{drive}\\"))
    } else {
        PathBuf::from("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_recent_without_name_still_reads() {
        let target: Target = serde_json::from_str(r#"{"kind":"disk","path":"/"}"#)
            .unwrap_or_else(|_| Target::folder(PathBuf::from("/x")));
        assert_eq!(target, Target::disk());
    }

    #[test]
    fn a_wsl_target_is_named_after_its_distribution() {
        let target = Target::wsl("Ubuntu");
        assert_eq!(target.label(), "Ubuntu");
        assert_eq!(target.path, PathBuf::from(r"\\wsl.localhost\Ubuntu"));
    }

    #[test]
    fn a_drive_shows_its_label() {
        assert_eq!(
            drive_label(None, Path::new("/run/media/ana/Backup")),
            if cfg!(windows) {
                format!("{} (/run/media/ana/Backup)", t!("target.local_disk"))
            } else {
                "Backup".to_string()
            }
        );
        if !cfg!(windows) {
            assert_eq!(drive_label(Some("Data"), Path::new("/mnt/d")), "Data");
        }
    }

    #[test]
    fn the_same_place_ignores_what_it_remembers() {
        let mut scanned = Target::wsl("Ubuntu");
        scanned.home = Some(PathBuf::from("/home/ana"));
        assert!(scanned.same_place(&Target::wsl("Ubuntu")));
        assert!(!scanned.same_place(&Target::wsl("Debian")));
    }
}
