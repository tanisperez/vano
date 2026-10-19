//! The engine: worker threads that read directories, one aggregator that owns the tree writes.
//!
//! Workers never touch the tree. Each directory they read becomes one [`Msg`] on a channel, and
//! the aggregator applies the messages in batches under a single write lock. A worker always
//! sends the listing of a directory before it queues that directory's subdirectories, so the
//! channel guarantees a child's listing never overtakes its parent's.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Mutex, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use crossbeam_utils::Backoff;
use vano_core::tree::{EntryData, FLAG_COMPLETE, FLAG_UNREADABLE, NO_NODE, NodeId, ROOT, Tree};

use crate::listing::{Ctx, DirInfo, read_directory};

/// A directory waiting to be read.
struct Job {
    id: NodeId,
    path: PathBuf,
    info: DirInfo,
}

/// The listing of one directory, ready for the aggregator.
struct Msg {
    dir: NodeId,
    first: NodeId,
    entries: Vec<EntryData>,
    /// Number of subdirectory jobs that follow this message.
    subdirs: u32,
    unreadable: bool,
}

/// State shared between the workers, the aggregator and the [`crate::ScanHandle`].
pub(crate) struct Shared {
    pub(crate) tree: RwLock<Tree>,
    pub(crate) completed: Mutex<Vec<NodeId>>,
    pub(crate) current: Mutex<Option<PathBuf>>,
    pub(crate) cancel: AtomicBool,
    /// Set by the aggregator as its very last action, after every worker has stopped.
    pub(crate) done: AtomicBool,
    /// Whether the root completed, i.e. the scan was not cut short.
    pub(crate) complete: AtomicBool,
    pub(crate) files: AtomicU64,
    pub(crate) dirs: AtomicU64,
    pub(crate) bytes: AtomicU64,
    pub(crate) errors: AtomicU64,
    /// Frozen duration of the scan, in nanoseconds, 0 while running.
    pub(crate) elapsed_ns: AtomicU64,
    pub(crate) started: Instant,
    pub(crate) taken: AtomicBool,
}

impl Shared {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            tree: RwLock::new(Tree::new(root)),
            completed: Mutex::new(Vec::new()),
            current: Mutex::new(None),
            cancel: AtomicBool::new(false),
            done: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            elapsed_ns: AtomicU64::new(0),
            started: Instant::now(),
            taken: AtomicBool::new(false),
        }
    }
}

/// Everything the workers share besides [`Shared`].
struct Engine<'a> {
    shared: &'a Shared,
    ctx: Ctx,
    injector: Injector<Job>,
    /// Ids handed out so far; a directory with `k` entries reserves `k` consecutive ids.
    next_id: AtomicU32,
    /// Jobs queued or running. Zero means every directory has been read.
    outstanding: AtomicUsize,
}

/// Runs a whole scan on the calling thread plus `threads` workers, and returns when it ends.
pub(crate) fn run(shared: &Shared, root: PathBuf, one_file_system: bool, threads: usize) {
    let engine = Engine {
        shared,
        ctx: Ctx {
            one_file_system,
            hardlinks: crate::hardlinks::Hardlinks::new(),
            skip: foreign_mounts(&root, one_file_system),
        },
        injector: Injector::new(),
        next_id: AtomicU32::new(1),
        outstanding: AtomicUsize::new(1),
    };
    let (dev, root_mtime) = crate::listing::stat_root(&root).unwrap_or((0, 0));
    engine.injector.push(Job {
        id: ROOT,
        path: root,
        info: DirInfo { dev, follow: true },
    });

    let workers: Vec<Worker<Job>> = (0..threads).map(|_| Worker::new_lifo()).collect();
    let stealers: Vec<Stealer<Job>> = workers.iter().map(Worker::stealer).collect();
    let (tx, rx) = mpsc::channel::<Msg>();
    thread::scope(|scope| {
        for (index, local) in workers.into_iter().enumerate() {
            let tx = tx.clone();
            let (engine, stealers) = (&engine, &stealers);
            let spawned = thread::Builder::new()
                .name(format!("vano-scan-{index}"))
                .spawn_scoped(scope, move || {
                    worker_loop(index, local, engine, stealers, tx)
                });
            if let Err(err) = spawned {
                log::warn!("cannot start scan worker {index}: {err}");
            }
        }
        // Only the workers' clones keep the channel open: it disconnects when they all exit.
        drop(tx);
        aggregate(shared, &rx, root_mtime);
    });
    let elapsed = shared
        .started
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    shared.elapsed_ns.store(elapsed.max(1), Ordering::Release);
    shared.done.store(true, Ordering::Release);
}

fn worker_loop(
    index: usize,
    local: Worker<Job>,
    engine: &Engine<'_>,
    stealers: &[Stealer<Job>],
    tx: Sender<Msg>,
) {
    let shared = engine.shared;
    let backoff = Backoff::new();
    let mut handled = 0u32;
    loop {
        if shared.cancel.load(Ordering::Relaxed) {
            return;
        }
        let Some(job) = find_job(&local, &engine.injector, stealers) else {
            if engine.outstanding.load(Ordering::Acquire) == 0 {
                return;
            }
            if backoff.is_completed() {
                thread::sleep(Duration::from_micros(50));
            } else {
                backoff.snooze();
            }
            continue;
        };
        backoff.reset();
        handled = handled.wrapping_add(1);
        if handled % 8 == 1 || index == 0 {
            publish_current(shared, &job);
        }
        process(job, engine, &local, &tx);
        engine.outstanding.fetch_sub(1, Ordering::AcqRel);
    }
}

fn publish_current(shared: &Shared, job: &Job) {
    // Skipping when contended is fine: the field is only a hint for the status line.
    if let Ok(mut current) = shared.current.try_lock() {
        match current.as_mut() {
            Some(path) => path.clone_from(&job.path),
            None => *current = Some(job.path.clone()),
        }
    }
}

fn find_job(
    local: &Worker<Job>,
    injector: &Injector<Job>,
    stealers: &[Stealer<Job>],
) -> Option<Job> {
    local.pop().or_else(|| {
        loop {
            let steal = injector
                .steal_batch_and_pop(local)
                .or_else(|| stealers.iter().map(Stealer::steal).collect());
            match steal {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
    })
}

fn process(job: Job, engine: &Engine<'_>, local: &Worker<Job>, tx: &Sender<Msg>) {
    let shared = engine.shared;
    let listing = read_directory(&job.path, job.info, &engine.ctx);
    let count = listing.entries.len() as u32;
    let first = if count == 0 {
        NO_NODE
    } else {
        engine.next_id.fetch_add(count, Ordering::Relaxed)
    };

    let mut bytes = 0u64;
    let mut files = 0u64;
    for entry in &listing.entries {
        bytes += entry.size;
        files += u64::from(entry.kind != vano_core::tree::Kind::Dir);
    }
    shared.files.fetch_add(files, Ordering::Relaxed);
    shared.bytes.fetch_add(bytes, Ordering::Relaxed);
    shared.dirs.fetch_add(1, Ordering::Relaxed);
    if listing.errors > 0 {
        shared
            .errors
            .fetch_add(u64::from(listing.errors), Ordering::Relaxed);
    }

    // Build the child jobs before the entries move into the message, but queue them only after
    // the message is sent: the aggregator needs the parent listing first.
    let mut children = Vec::with_capacity(listing.descend.len());
    for &(index, dev) in &listing.descend {
        let entry = &listing.entries[index as usize];
        children.push(Job {
            id: first + index,
            path: join_name(&job.path, &entry.name),
            info: DirInfo { dev, follow: false },
        });
    }
    let subdirs = children.len() as u32;
    let msg = Msg {
        dir: job.id,
        first,
        entries: listing.entries,
        subdirs,
        unreadable: listing.unreadable,
    };
    if tx.send(msg).is_err() {
        return;
    }
    engine
        .outstanding
        .fetch_add(children.len(), Ordering::AcqRel);
    for child in children {
        local.push(child);
    }
}

#[cfg(unix)]
fn join_name(parent: &std::path::Path, name: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    parent.join(std::ffi::OsStr::from_bytes(name))
}

#[cfg(not(unix))]
fn join_name(parent: &std::path::Path, name: &[u8]) -> PathBuf {
    parent.join(String::from_utf8_lossy(name).as_ref())
}

/// How many messages the aggregator applies under one write lock.
const BATCH: usize = 512;

/// Applies the workers' messages to the tree until every worker is gone.
fn aggregate(shared: &Shared, rx: &Receiver<Msg>, root_mtime: u32) {
    // Directories still waiting for their own listing or for subdirectories, by node id.
    let mut pending: Vec<u32> = vec![0];
    let mut batch: Vec<Msg> = Vec::with_capacity(BATCH);
    let mut done_dirs: Vec<NodeId> = Vec::new();
    let mut first_batch = true;
    while let Ok(msg) = rx.recv() {
        batch.push(msg);
        while batch.len() < BATCH {
            match rx.try_recv() {
                Ok(msg) => batch.push(msg),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        {
            let mut tree = shared.tree.write().unwrap_or_else(PoisonError::into_inner);
            if first_batch {
                tree.touch(ROOT, root_mtime);
                first_batch = false;
            }
            for msg in batch.drain(..) {
                apply(&mut tree, &mut pending, &mut done_dirs, msg);
            }
        }
        if !done_dirs.is_empty() {
            if done_dirs.contains(&ROOT) {
                shared.complete.store(true, Ordering::Release);
            }
            shared
                .completed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .append(&mut done_dirs);
        }
        // Let a waiting reader (the UI) in before the next batch.
        thread::yield_now();
    }
}

fn apply(tree: &mut Tree, pending: &mut Vec<u32>, done: &mut Vec<NodeId>, msg: Msg) {
    if !msg.entries.is_empty() {
        tree.set_children(msg.dir, msg.first, &msg.entries);
    }
    if msg.unreadable {
        tree.set_flag(msg.dir, FLAG_UNREADABLE);
    }
    let slot = msg.dir as usize;
    if pending.len() <= slot {
        pending.resize((slot + 1).max(pending.len() * 2), 0);
    }
    // One for the directory's own listing (consumed right below) plus one per subdirectory.
    pending[slot] = msg.subdirs + 1;
    let mut cur = msg.dir;
    while cur != NO_NODE {
        let left = &mut pending[cur as usize];
        *left -= 1;
        if *left > 0 {
            break;
        }
        tree.set_flag(cur, FLAG_COMPLETE);
        done.push(cur);
        cur = tree.node(cur).parent;
    }
}

/// The mount points of a Linux root scanned from Windows, not to enter (see `linux_root`).
#[cfg(not(unix))]
fn foreign_mounts(
    root: &std::path::Path,
    one_file_system: bool,
) -> std::collections::HashSet<PathBuf> {
    if !one_file_system {
        return Default::default();
    }
    crate::linux_root::foreign_mounts(root).unwrap_or_default()
}

/// Unix tells file systems apart by device, in `listing::unix`.
#[cfg(unix)]
fn foreign_mounts(
    _root: &std::path::Path,
    _one_file_system: bool,
) -> std::collections::HashSet<PathBuf> {
    Default::default()
}
