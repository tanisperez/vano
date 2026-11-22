//! `vano-wsl scan <ROOT>`.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use vano_scan::{Progress, ScanOptions};
use vano_system::wsl::ScanLine;

use crate::FAILED;

pub fn command(root: &Path) -> u8 {
    if let Some(home) = dirs::home_dir() {
        report(&ScanLine::Home(home));
    }
    let handle = vano_scan::start(ScanOptions::new(root));
    while !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(100));
        report(&progress_line(&handle.progress()));
    }
    handle.wait();
    report(&progress_line(&handle.progress()));
    let Some(tree) = handle.take_tree() else {
        eprintln!("vano-wsl: the scan ended without a tree");
        return FAILED;
    };
    let bytes = vano_core::persist::encode(&tree);
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("vano-wsl: {error}");
            FAILED
        }
    }
}

fn progress_line(progress: &Progress) -> ScanLine {
    ScanLine::Progress {
        files: progress.files,
        dirs: progress.dirs,
        bytes: progress.bytes,
        errors: progress.errors,
    }
}

fn report(line: &ScanLine) {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr
        .write_all(line.to_line().as_bytes())
        .and_then(|()| stderr.flush());
}
