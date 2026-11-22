//! `vano scan [PATH]`: scans a folder or a disk with the app's scanner, and prints the biggest
//! entries and the recommendations. The scan itself ([`run`]) is shared with `vano clean`.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use vano_core::Tree;
use vano_scan::{Progress, ScanOptions};

use crate::args::ScanArgs;
use crate::exit;
use crate::report::{self, ScanReport};

/// A finished scan.
pub(crate) struct Scanned {
    pub tree: Tree,
    pub progress: Progress,
}

/// The folder a command works on: the path given, or the home folder.
pub(crate) fn target(path: Option<&Path>) -> Result<PathBuf, String> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => dirs::home_dir().ok_or("the home folder is unknown: pass a path")?,
    };
    let absolute =
        std::path::absolute(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    match absolute.metadata() {
        Ok(metadata) if metadata.is_dir() => Ok(absolute),
        Ok(_) => Err(format!("{} is not a folder", absolute.display())),
        Err(error) => Err(format!("{}: {error}", absolute.display())),
    }
}

/// Scans `root` to the end. With `live`, a line on stderr counts the files as they are found
/// (only when stderr is a terminal, so pipes and files get clean output).
pub(crate) fn run(root: &Path, one_file_system: bool, live: bool) -> Scanned {
    let mut options = ScanOptions::new(root);
    options.one_file_system = one_file_system;
    let handle = vano_scan::start(options);
    let live = live && std::io::stderr().is_terminal();
    while !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(100));
        if live {
            let progress = handle.progress();
            eprint!(
                "\rScanning {}: {} files, {}\x1b[K",
                report::show_path(root),
                report::count(progress.files),
                vano_core::size::format_plain(progress.bytes),
            );
            let _ = std::io::stderr().flush();
        }
    }
    handle.wait();
    if live {
        eprint!("\r\x1b[K");
    }
    let progress = handle.progress();
    let tree = handle
        .take_tree()
        .unwrap_or_else(|| Tree::new(root.to_path_buf()));
    Scanned { tree, progress }
}

/// `vano scan`.
pub fn command(args: &ScanArgs) -> u8 {
    let root = match target(args.path.as_deref()) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("vano scan: {error}");
            return exit::FAILED;
        }
    };
    let scanned = run(&root, !args.cross_file_systems, !args.json);
    let rules = vano_rules::Context::current();
    let recommendations = vano_rules::evaluate(&scanned.tree, &rules);
    let report = ScanReport::new(&scanned, &recommendations, args.top);
    let text = if args.json {
        report.json()
    } else {
        report.text()
    };
    let _ = std::io::stdout().write_all(text.as_bytes());
    exit::SUCCESS
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(target(Some(&missing)).is_err());
    }

    #[test]
    fn a_file_is_not_a_folder_to_scan() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        let error = target(Some(&file)).unwrap_err();
        assert!(error.ends_with("is not a folder"), "{error}");
    }

    #[test]
    fn a_scan_finds_every_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/a"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.path().join("b"), b"x").unwrap();
        let scanned = run(dir.path(), true, false);
        assert_eq!(scanned.progress.files, 2);
        assert!(scanned.tree.node(vano_core::ROOT).size >= 5000);
    }
}
