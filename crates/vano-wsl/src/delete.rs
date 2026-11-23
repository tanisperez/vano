//! `vano-wsl delete`: the app runs it inside a WSL 2 distribution, where deleting through
//! `\\wsl$` would be slow and the Recycle Bin cannot take the files. Same walk as
//! the app's permanent deletion (`vano_delete::delete`): never follows a symbolic link, never
//! enters another file system.
//!
//! Standard input holds the paths, each ended by a NUL, then an empty one. A `c` written after
//! them stops before the next file. Standard output reports `vano_system::wsl::DeleteLine`s.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use vano_delete::trash::{DeleteMode, TrashError, TrashProgress};
use vano_system::wsl::DeleteLine;

use crate::{FAILED, USAGE};

/// Splits the NUL-ended paths up to the first empty one. `None` when the list never ends.
fn split_paths(bytes: &[u8]) -> Option<(Vec<PathBuf>, usize)> {
    let mut paths = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != 0 {
            continue;
        }
        if index == start {
            return Some((paths, index + 1));
        }
        paths.push(path_from_bytes(&bytes[start..index]));
        start = index + 1;
    }
    None
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// `vano-wsl delete`.
pub fn command() -> u8 {
    let mut stdin = std::io::stdin();
    let mut read = Vec::new();
    let mut chunk = [0u8; 8192];
    let (paths, used) = loop {
        if let Some(found) = split_paths(&read) {
            break found;
        }
        match stdin.read(&mut chunk) {
            Ok(0) | Err(_) => {
                eprintln!("vano-wsl delete: the list of paths did not end with an empty one");
                return USAGE;
            }
            Ok(count) => read.extend_from_slice(&chunk[..count]),
        }
    };
    let progress = Arc::new(TrashProgress::new(DeleteMode::Permanent, paths.len(), 0, 0));
    if read[used..].contains(&b'c') {
        progress.cancel.store(true, Ordering::Relaxed);
    }
    // Anything written later is a request to stop.
    {
        let progress = Arc::clone(&progress);
        std::thread::spawn(move || {
            let mut byte = [0u8; 1];
            while let Ok(1) = std::io::stdin().read(&mut byte) {
                if byte[0] == b'c' {
                    progress.cancel.store(true, Ordering::Relaxed);
                }
            }
        });
    }
    let reporter = {
        let progress = Arc::clone(&progress);
        std::thread::spawn(move || {
            let mut last = (0, 0);
            loop {
                let finished = progress.finished.load(Ordering::Acquire);
                let now = (
                    progress.files.load(Ordering::Relaxed),
                    progress.bytes.load(Ordering::Relaxed),
                );
                if now != last {
                    print_line(&DeleteLine::Progress(now.0, now.1));
                    last = now;
                }
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    let outcome = vano_delete::delete::delete_each(&paths, &progress);
    progress.finished.store(true, Ordering::Release);
    let _ = reporter.join();
    for index in &outcome.moved {
        print_line(&DeleteLine::Gone(*index));
    }
    for (index, error) in &outcome.failed {
        let reason = match error {
            TrashError::Refused => "refused".to_string(),
            TrashError::OtherFileSystem => "other file system".to_string(),
            TrashError::ReadOnlyFolder => "read-only folder".to_string(),
            TrashError::Os(text) => text.clone(),
        };
        print_line(&DeleteLine::Failed(*index, reason));
    }
    for index in &outcome.partial {
        print_line(&DeleteLine::Stopped(*index));
    }
    if outcome.failed.is_empty() { 0 } else { FAILED }
}

fn print_line(line: &DeleteLine) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout
        .write_all(line.to_line().as_bytes())
        .and_then(|()| stdout.flush());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_end_with_an_empty_one() {
        let (paths, used) = split_paths(b"/a\0/b c\0\0c").unwrap_or_default();
        assert_eq!(paths, vec![PathBuf::from("/a"), PathBuf::from("/b c")]);
        assert_eq!(used, 9);
    }

    #[test]
    fn an_unfinished_list_waits_for_more() {
        assert_eq!(split_paths(b"/a\0/b"), None);
        assert_eq!(split_paths(b""), None);
    }

    #[test]
    fn an_empty_list_has_no_paths() {
        assert_eq!(split_paths(b"\0"), Some((Vec::new(), 1)));
    }
}
