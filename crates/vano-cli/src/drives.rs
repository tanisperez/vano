//! `vano drives`: the connected disks, as the home screen lists them.

use std::io::Write;

use serde::Serialize;
use vano_core::size::format_plain;
use vano_system::Drive;

use crate::args::DrivesArgs;
use crate::exit;
use crate::report::to_json;

/// One disk in the JSON output.
#[derive(Debug, Serialize)]
struct DriveJson {
    name: String,
    mount_point: String,
    label: Option<String>,
    file_system: String,
    /// `NVMe SSD`, `SSD`, `HDD` or `Disk`.
    media: &'static str,
    removable: bool,
    system: bool,
    read_only: bool,
    total: u64,
    used: u64,
    available: u64,
}

impl DriveJson {
    fn new(drive: &Drive) -> Self {
        DriveJson {
            name: drive.display_name(),
            mount_point: drive.mount_point.display().to_string(),
            label: drive.label.clone(),
            file_system: drive.file_system.clone(),
            media: drive.media.name(),
            removable: drive.removable,
            system: drive.system,
            read_only: drive.read_only,
            total: drive.total,
            used: drive.used(),
            available: drive.available,
        }
    }
}

/// `vano drives`.
pub fn command(args: &DrivesArgs) -> u8 {
    let drives = vano_system::list_drives();
    let text = if args.json {
        to_json(&drives.iter().map(DriveJson::new).collect::<Vec<_>>())
    } else {
        table(&drives)
    };
    let _ = std::io::stdout().write_all(text.as_bytes());
    exit::SUCCESS
}

/// The kind of a disk in the table: `NVMe SSD`, `External SSD`, `HDD, system`.
fn kind(drive: &Drive) -> String {
    let mut kind = drive.media.name().to_string();
    if drive.removable {
        kind = format!("External {kind}");
    }
    if drive.system {
        kind.push_str(", system");
    }
    kind
}

fn table(drives: &[Drive]) -> String {
    if drives.is_empty() {
        return "No disks found.\n".to_string();
    }
    let rows: Vec<[String; 6]> = drives
        .iter()
        .map(|drive| {
            let percent = (drive.used() as f64 / drive.total.max(1) as f64 * 100.0).round();
            [
                drive.mount_point.display().to_string(),
                drive.label.clone().unwrap_or_default(),
                kind(drive),
                drive.file_system.clone(),
                format!(
                    "{} of {}",
                    format_plain(drive.used()),
                    format_plain(drive.total)
                ),
                format!("{percent}%"),
            ]
        })
        .collect();
    let header = ["MOUNT", "LABEL", "KIND", "FS", "USED", ""];
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: [&str; 6]| {
        let mut out = String::new();
        for (index, (cell, width)) in cells.iter().zip(widths).enumerate() {
            match index {
                0 => out.push_str(&format!("{cell:<width$}")),
                // Sizes and percentages align to the right.
                4 | 5 => out.push_str(&format!("  {cell:>width$}")),
                _ => out.push_str(&format!("  {cell:<width$}")),
            }
        }
        out.trim_end().to_string() + "\n"
    };
    let mut out = line(header);
    for row in &rows {
        out.push_str(&line(row.each_ref().map(String::as_str)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use vano_system::Media;

    fn drive(mount: &str, label: Option<&str>, removable: bool, system: bool) -> Drive {
        Drive {
            mount_point: PathBuf::from(mount),
            label: label.map(str::to_string),
            device: String::new(),
            file_system: "ext4".to_string(),
            media: Media::Ssd,
            removable,
            system,
            read_only: false,
            total: 1_000_000_000_000,
            available: 250_000_000_000,
        }
    }

    #[test]
    fn the_table_aligns_its_columns() {
        let drives = vec![
            drive("/", None, false, true),
            drive("/run/media/ana/Backup", Some("Backup"), true, false),
        ];
        assert_eq!(
            table(&drives),
            "MOUNT                  LABEL   KIND          FS                USED\n\
             /                              SSD, system   ext4  750 GB of 1.0 TB  75%\n\
             /run/media/ana/Backup  Backup  External SSD  ext4  750 GB of 1.0 TB  75%\n"
        );
    }

    #[test]
    fn no_disks_says_so() {
        assert_eq!(table(&[]), "No disks found.\n");
    }
}
