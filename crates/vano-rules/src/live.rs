//! Recommendations that appear while the scan runs.

use std::collections::HashSet;
use std::path::PathBuf;

use vano_core::{NodeId, ROOT, Tree};

use crate::acc::Acc;
use crate::defs::{Fire, R, RULES};
use crate::engine::Engine;
use crate::{Context, Recommendation};

/// Finds recommendations folder by folder, as the scanner completes them.
///
/// A match is reported when the matched folder completes, or, for matches that need their
/// siblings (idle `node_modules`) or are plain files, when the folder that holds them completes.
/// At the end of a scan the result equals [`crate::evaluate`] (same nodes, sizes and paths); only
/// the order differs: here it is the order of discovery.
pub struct LiveRules {
    engine: Engine,
    home: Option<PathBuf>,
    accs: Vec<Option<Acc>>,
    order: Vec<usize>,
    seen: HashSet<NodeId>,
}

impl LiveRules {
    /// A finder for one scan.
    pub fn new(ctx: Context) -> Self {
        Self {
            engine: Engine::new(&ctx),
            home: ctx.home,
            accs: (0..RULES.len()).map(|_| None).collect(),
            order: Vec::new(),
            seen: HashSet::new(),
        }
    }

    /// `dirs` just completed: all their subtree is read.
    pub fn on_completed(&mut self, tree: &Tree, dirs: &[NodeId]) {
        for &dir in dirs {
            if dir >= tree.len() as NodeId {
                continue;
            }
            self.completed_self(tree, dir);
            self.completed_children(tree, dir);
        }
    }

    fn completed_self(&mut self, tree: &Tree, dir: NodeId) {
        if dir == ROOT || !self.engine.self_candidate(tree.name_bytes(dir)) {
            return;
        }
        let parent = tree.node(dir).parent;
        let ps = self.engine.state_of(tree, parent, false);
        if let (_, Some(rule)) = self.engine.child(tree, parent, ps, dir, false)
            && RULES[rule as usize].fire == Fire::SelfDone
        {
            self.record(tree, dir, rule);
        }
    }

    fn completed_children(&mut self, tree: &Tree, dir: NodeId) {
        let has_nm = tree
            .children(dir)
            .any(|c| tree.node(c).is_dir() && tree.name_bytes(c) == b"node_modules");
        // The root can be a folder of files (a scan of ~/Downloads), so it is always checked.
        if dir != ROOT && !has_nm && !self.engine.file_parent(tree.name_bytes(dir)) {
            return;
        }
        let state = self.engine.state_of(tree, dir, false);
        for child in tree.children(dir) {
            if let (_, Some(rule)) = self.engine.child(tree, dir, state, child, false)
                && RULES[rule as usize].fire == Fire::ParentDone
            {
                self.record(tree, child, rule);
            }
        }
    }

    fn record(&mut self, tree: &Tree, id: NodeId, rule: R) {
        if tree.node(id).size == 0 || !self.seen.insert(id) {
            return;
        }
        let slot = &mut self.accs[rule as usize];
        if slot.is_none() {
            self.order.push(rule as usize);
        }
        slot.get_or_insert_with(|| Acc::new(rule))
            .add(tree, self.home.as_deref(), id);
    }

    /// The recommendations found so far, in the order they were first found.
    pub fn found(&self) -> Vec<Recommendation> {
        self.order
            .iter()
            .filter_map(|i| self.accs[*i].as_ref())
            .map(Acc::recommendation)
            .collect()
    }
}
