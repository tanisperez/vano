//! A scan done by someone else: Vano inside a WSL 2 distribution scans it natively and sends the
//! finished tree back. [`start_external`] wraps that work in a [`ScanHandle`], so the app shows
//! its progress and opens its results exactly as for a local scan. The tree arrives whole at the
//! end: there is no live tree while it runs.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use vano_core::Tree;

use crate::ScanHandle;
use crate::scan::Shared;

/// What the work of an external scan reports while it runs.
pub struct ExternalScan {
    shared: Arc<Shared>,
}

impl ExternalScan {
    /// The counters so far, shown on the scanning screen.
    pub fn set_counts(&self, files: u64, dirs: u64, bytes: u64, errors: u64) {
        let shared = &self.shared;
        shared.files.store(files, Ordering::Relaxed);
        shared.dirs.store(dirs, Ordering::Relaxed);
        shared.bytes.store(bytes, Ordering::Relaxed);
        shared.errors.store(errors, Ordering::Relaxed);
    }

    /// The home folder of the scanned system, for the rules that look under `~`.
    pub fn set_home(&self, home: PathBuf) {
        *self
            .shared
            .home
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(home);
    }

    /// Whether the user asked to stop: the work should end as soon as it can.
    pub fn cancelled(&self) -> bool {
        self.shared.cancel.load(Ordering::Relaxed)
    }
}

/// Starts `work` on a thread of its own and returns at once. `work` returns the finished tree,
/// or why it could not scan ([`ScanHandle::failure`]). A failed or cancelled scan ends as
/// cancelled, without a tree.
pub fn start_external(
    root: PathBuf,
    work: impl FnOnce(&ExternalScan) -> Result<Tree, String> + Send + 'static,
) -> ScanHandle {
    let shared = Arc::new(Shared::new(root));
    let worker = ExternalScan {
        shared: Arc::clone(&shared),
    };
    let spawned = std::thread::Builder::new()
        .name("vano-scan-external".into())
        .spawn(move || {
            let outcome = work(&worker);
            let shared = &worker.shared;
            match outcome {
                Ok(tree) if !worker.cancelled() => {
                    *shared.tree.write().unwrap_or_else(PoisonError::into_inner) = tree;
                    shared.complete.store(true, Ordering::Release);
                }
                Ok(_) => {}
                Err(error) => {
                    log::warn!("external scan failed: {error}");
                    *shared
                        .failure
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = Some(error);
                }
            }
            let elapsed = shared
                .started
                .elapsed()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            shared.elapsed_ns.store(elapsed.max(1), Ordering::Release);
            shared.done.store(true, Ordering::Release);
        });
    let thread = match spawned {
        Ok(handle) => Some(handle),
        Err(error) => {
            *shared
                .failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner) =
                Some(format!("cannot start the scan thread: {error}"));
            shared.elapsed_ns.store(1, Ordering::Release);
            shared.done.store(true, Ordering::Release);
            None
        }
    };
    ScanHandle {
        shared,
        thread: Mutex::new(thread),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use vano_core::ROOT;

    #[test]
    fn the_tree_of_a_finished_external_scan_is_the_result() {
        let handle = start_external(PathBuf::from("/"), |scan| {
            scan.set_counts(3, 1, 300, 0);
            scan.set_home(PathBuf::from("/home/ana"));
            let mut tree = Tree::new("/");
            tree.add_to_chain(ROOT, 300, 3, 0);
            Ok(tree)
        });
        handle.wait();
        let progress = handle.progress();
        assert!(progress.finished && !progress.cancelled);
        assert_eq!(progress.files, 3);
        assert_eq!(handle.home(), Some(PathBuf::from("/home/ana")));
        assert_eq!(handle.take_tree().unwrap().node(ROOT).size, 300);
        assert_eq!(handle.failure(), None);
    }

    #[test]
    fn a_failed_external_scan_ends_cancelled_with_its_reason() {
        let handle = start_external(PathBuf::from("/"), |_| Err("no wsl.exe".to_string()));
        handle.wait();
        assert!(handle.progress().cancelled);
        assert_eq!(handle.failure().as_deref(), Some("no wsl.exe"));
    }

    #[test]
    fn a_cancelled_external_scan_drops_its_tree() {
        let handle = start_external(PathBuf::from("/"), |scan| {
            while !scan.cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Ok(Tree::new("/"))
        });
        handle.cancel();
        handle.wait();
        assert!(handle.progress().cancelled);
    }
}
