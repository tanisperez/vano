//! What a scan reads: the home folder, the whole system disk, or a folder the user picks.

use std::path::PathBuf;

use rust_i18n::t;
use serde::{Deserialize, Serialize};

/// The kind of place a scan starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    #[default]
    Home,
    Disk,
    Folder,
}

/// A place to scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub kind: TargetKind,
    pub path: PathBuf,
}

impl Target {
    /// The user's home folder, `None` when the OS cannot tell.
    pub fn home() -> Option<Self> {
        dirs::home_dir().map(|path| Target {
            kind: TargetKind::Home,
            path,
        })
    }

    /// The disk the system runs from: `/` on Unix, the system drive on Windows.
    pub fn disk() -> Self {
        Target {
            kind: TargetKind::Disk,
            path: system_root(),
        }
    }

    /// A folder the user picked. Picking the system root is the same as picking the disk.
    pub fn folder(path: PathBuf) -> Self {
        if path == system_root() {
            return Self::disk();
        }
        Target {
            kind: TargetKind::Folder,
            path,
        }
    }

    /// The name shown in the title bar, the scan button and the recent list.
    pub fn label(&self) -> String {
        match self.kind {
            TargetKind::Home => t!("target.home").into_owned(),
            TargetKind::Disk => disk_name(),
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
