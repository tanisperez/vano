//! What Vano recommends removing, and what color each node has in the treemap.
//!
//! Vano scans a folder into a [`Tree`]. This crate reads that tree, with no IO, and answers:
//!
//! * [`evaluate`] and [`LiveRules`]: the recommendations. **Safe** ones are caches and build
//!   outputs their tool recreates by itself; **Review** ones are things the user may still need
//!   (SDKs, models, idle `node_modules`, old installers...).
//! * [`categorize`] and [`category_of`]: the [`Category`] of every node.
//!
//! The rules are conservative: a match is either a well-known location (see `defs.rs`) or a
//! folder with a given name next to the file that proves what it is (`target` next to
//! `Cargo.toml`). The outermost match wins, so recommendations never overlap, and nothing is
//! examined inside a match.

mod acc;
mod defs;
mod engine;
mod live;
mod trie;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use vano_core::{Category, NodeId, Tree};

pub use live::LiveRules;

use acc::Acc;
use defs::RULES;
use engine::Engine;

/// Whether the owning tool recreates the data without help.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Caches and build outputs: the tool that owns them recreates them.
    Safe,
    /// The user might still need it.
    Review,
}

/// The operating system whose well-known locations are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

impl Os {
    /// The OS this build runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// What the rules need to know besides the tree.
#[derive(Debug, Clone)]
pub struct Context {
    /// The user's home folder, if known: rules for `~/...` need it.
    pub home: Option<PathBuf>,
    /// The current time, in seconds since the Unix epoch.
    pub now: u32,
    pub os: Os,
}

impl Context {
    /// The real home, time and OS.
    pub fn current() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX));
        Self {
            home: dirs::home_dir(),
            now,
            os: Os::current(),
        }
    }
}

/// One group of folders or files the user can move to the Trash.
#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    /// Stable rule id; the texts are `rules.<id>.name`, `.desc` and, for Review, `.reason`.
    pub rule: &'static str,
    pub status: Status,
    /// The matched nodes, none inside another, biggest first.
    pub nodes: Vec<NodeId>,
    /// Sum of the sizes of `nodes`.
    pub size: u64,
    /// Sum of the file counts of `nodes`.
    pub files: u64,
    /// The newest modification time among `nodes`.
    pub newest_mtime: u32,
    /// The size is an estimate.
    pub approx: bool,
    /// A short path pattern for the UI, like `~/.npm` or `~/projects/{orbit,sandbox}`.
    pub display_path: String,
}

/// The recommendations of a finished tree: Safe first, then Review, each biggest first.
pub fn evaluate(tree: &Tree, ctx: &Context) -> Vec<Recommendation> {
    let engine = Engine::new(ctx);
    let (_, hits) = engine.walk(tree, false);
    let mut accs: Vec<Option<Acc>> = (0..RULES.len()).map(|_| None).collect();
    for (id, rule) in hits {
        if tree.node(id).size == 0 {
            continue;
        }
        accs[rule as usize]
            .get_or_insert_with(|| Acc::new(rule))
            .add(tree, ctx.home.as_deref(), id);
    }
    let mut recs: Vec<(usize, Recommendation)> = accs
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.as_ref().map(|a| (i, a.recommendation())))
        .collect();
    recs.sort_by(|(ia, a), (ib, b)| {
        let status = |r: &Recommendation| u8::from(r.status == Status::Review);
        status(a)
            .cmp(&status(b))
            .then(b.size.cmp(&a.size))
            .then(ia.cmp(ib))
    });
    recs.into_iter().map(|(_, r)| r).collect()
}

/// The category of every node (index = node id), in one pass over the tree.
pub fn categorize(tree: &Tree, ctx: &Context) -> Vec<Category> {
    let engine = Engine::new(ctx);
    let (states, _) = engine.walk(tree, true);
    states.into_iter().map(|s| s.cat).collect()
}

/// The category of one node, the same as `categorize(tree, ctx)[id]` on a finished tree, but
/// walking only its ancestry. Meant for coloring the few visible tiles while a scan runs; a
/// folder whose children are not read yet can still change color when they arrive.
pub fn category_of(tree: &Tree, ctx: &Context, id: NodeId) -> Category {
    type Cached = Option<(Option<PathBuf>, Os, Rc<Engine>)>;
    thread_local! {
        /// Building the engine costs more than one lookup, and the UI asks hundreds of times.
        static CACHE: RefCell<Cached> =
            const { RefCell::new(None) };
    }
    let engine = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        match &*cache {
            Some((home, os, engine)) if *home == ctx.home && *os == ctx.os => engine.clone(),
            _ => {
                let engine = Rc::new(Engine::new(ctx));
                *cache = Some((ctx.home.clone(), ctx.os, engine.clone()));
                engine
            }
        }
    });
    if id as usize >= tree.len() {
        return Category::Other;
    }
    engine.state_of(tree, id, true).cat
}

/// Every rule id, for checking that each one has its texts.
pub fn rule_ids() -> &'static [&'static str] {
    static IDS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| RULES.iter().map(|r| r.id).collect())
}

/// The status of rule `id`, if it exists.
pub fn rule_status(id: &str) -> Option<Status> {
    RULES.iter().find(|r| r.id == id).map(|r| r.status)
}
