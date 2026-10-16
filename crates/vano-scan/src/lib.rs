//! Vano's disk scanner: many threads read directories, one aggregator builds the
//! [`vano_core::Tree`], and the tree can be read live while the scan is still running.
//!
//! Start a scan with [`start`], poll [`ScanHandle::progress`], draw from
//! [`ScanHandle::read_tree`], and move the finished tree out with [`ScanHandle::take_tree`].

pub mod cache;
mod hardlinks;
mod listing;
mod scan;
mod usage;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use vano_core::tree::{NodeId, Tree};

pub use usage::{DiskUsage, root_filesystem_usage};

use scan::Shared;

/// What to scan and how.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// The directory to scan.
    pub root: PathBuf,
    /// Do not enter directories that belong to another file system (mount points are still
    /// listed, flagged `FLAG_OTHER_FS`). Virtual file systems such as `/proc` are never entered.
    pub one_file_system: bool,
    /// Worker threads, 0 picks a good number for this machine.
    pub threads: usize,
}

impl ScanOptions {
    /// Options for scanning `root` on one file system with an automatic thread count.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            one_file_system: true,
            threads: 0,
        }
    }
}

/// A snapshot of how the scan is going.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Entries that are not directories found so far.
    pub files: u64,
    /// Directories read so far.
    pub dirs: u64,
    /// Allocated bytes found so far (hard links counted once).
    pub bytes: u64,
    /// Directories or entries that could not be read.
    pub errors: u64,
    /// Time since the scan started, frozen when it ends.
    pub elapsed: Duration,
    /// A directory being read right now, only meant for display.
    pub current: Option<PathBuf>,
    /// The scan ended, completed or cancelled.
    pub finished: bool,
    /// The scan ended because of [`ScanHandle::cancel`] before it could finish.
    pub cancelled: bool,
}

/// A running (or finished) scan. Cheap to share between threads.
pub struct ScanHandle {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/// Threads to use when [`ScanOptions::threads`] is 0.
///
/// One per core, between 4 and 16. On a warm cache that measured best (2x the cores was about 12%
/// slower, the extra threads only add contention); the floor of 4 keeps slow disks busy.
fn default_threads() -> usize {
    let cpus = std::thread::available_parallelism().map_or(4, usize::from);
    cpus.clamp(4, 16)
}

/// Starts scanning in the background and returns at once.
pub fn start(options: ScanOptions) -> ScanHandle {
    let threads = if options.threads == 0 {
        default_threads()
    } else {
        options.threads.clamp(1, 256)
    };
    let shared = Arc::new(Shared::new(options.root.clone()));
    let worker_shared = Arc::clone(&shared);
    let spawned = std::thread::Builder::new()
        .name("vano-scan".into())
        .spawn(move || {
            scan::run(
                &worker_shared,
                options.root,
                options.one_file_system,
                threads,
            );
        });
    let thread = match spawned {
        Ok(handle) => Some(handle),
        Err(err) => {
            log::error!("cannot start the scan thread: {err}");
            shared.errors.fetch_add(1, Ordering::Relaxed);
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

impl ScanHandle {
    /// Counters and timing. Cheap enough to call every frame.
    pub fn progress(&self) -> Progress {
        let shared = &self.shared;
        let finished = shared.done.load(Ordering::Acquire);
        let elapsed = if finished {
            Duration::from_nanos(shared.elapsed_ns.load(Ordering::Acquire))
        } else {
            shared.started.elapsed()
        };
        let current = if finished {
            None
        } else {
            shared
                .current
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        };
        Progress {
            files: shared.files.load(Ordering::Relaxed),
            dirs: shared.dirs.load(Ordering::Relaxed),
            bytes: shared.bytes.load(Ordering::Relaxed),
            errors: shared.errors.load(Ordering::Relaxed),
            elapsed,
            current,
            finished,
            cancelled: finished && !shared.complete.load(Ordering::Acquire),
        }
    }

    /// Runs `f` with a read lock on the live tree. The tree is consistent at any moment: a
    /// directory's size is always the sum of everything read below it so far. Keep `f` short, the
    /// scanner waits for writers.
    pub fn read_tree<R>(&self, f: impl FnOnce(&Tree) -> R) -> R {
        let tree = self
            .shared
            .tree
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        f(&tree)
    }

    /// The directories whose whole subtree has been read since the last call, in the order they
    /// completed (children before their parent).
    pub fn drain_completed(&self) -> Vec<NodeId> {
        std::mem::take(
            &mut *self
                .shared
                .completed
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Asks the scan to stop. Workers notice between directories, so it ends within milliseconds.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// Whether the scan ended (completed or cancelled) and all its threads are done.
    pub fn is_finished(&self) -> bool {
        self.shared.done.load(Ordering::Acquire)
    }

    /// Blocks until the scan ends.
    pub fn wait(&self) {
        let handle = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        match handle {
            Some(handle) => {
                if handle.join().is_err() {
                    log::error!("the scan thread panicked");
                    self.shared.done.store(true, Ordering::Release);
                }
            }
            // Another thread is already joining: poll until it is done.
            None => {
                while !self.is_finished() {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    /// Moves the finished tree out. `None` while the scan runs or after it was already taken.
    /// The handle keeps an empty tree afterwards.
    pub fn take_tree(&self) -> Option<Tree> {
        if !self.is_finished() || self.shared.taken.swap(true, Ordering::AcqRel) {
            return None;
        }
        let mut tree = self
            .shared
            .tree
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let root = tree.root_path().to_path_buf();
        Some(std::mem::replace(&mut *tree, Tree::new(root)))
    }
}

impl Drop for ScanHandle {
    fn drop(&mut self) {
        // A dropped handle must not leave threads reading the disk for nobody.
        self.shared.cancel.store(true, Ordering::Relaxed);
    }
}
