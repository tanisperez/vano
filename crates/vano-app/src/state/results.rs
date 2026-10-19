//! A finished scan: the tree, the category of every node, the recommendations and which of
//! them the user selected or already moved to the Trash. Plain Rust, unit tested.

use std::collections::{HashMap, HashSet};

use vano_core::{Category, NodeId, ROOT, Tree};
use vano_rules::{Recommendation, Status};

use super::target::Target;

/// Recommendations smaller than this are not worth the user's attention.
pub const MIN_REC_SIZE: u64 = 1_000_000;

/// The results screen's data.
pub struct Results {
    pub target: Target,
    pub tree: Tree,
    pub categories: Vec<Category>,
    pub recs: Vec<Recommendation>,
    /// Rules whose nodes were all moved to the Trash.
    pub removed: HashSet<&'static str>,
    /// Rules ticked in the list.
    pub selected: HashSet<&'static str>,
    /// Every recommended node and the index of its recommendation in `recs`.
    covered: HashMap<NodeId, usize>,
    /// Size and file count of each of `recs` as scanned, while part of it is in the Trash.
    scanned: Vec<(u64, u64)>,
    /// How long the scan took, in seconds.
    pub seconds: f64,
    /// Size and free space of the file system the scanned folder is on.
    pub fs: Option<vano_scan::DiskUsage>,
    /// For every node, the category that takes the most bytes inside it (a file's own one).
    /// Colors the treemap: a folder full of code projects reads as code even if its own path
    /// says nothing.
    dominant: Vec<Category>,
    /// Bytes per category ([`Category::ALL`] order) inside each folder, by the folder's slot.
    category_sums: CategorySums,
    /// Bytes of active Safe and Review recommendations inside each folder that holds some,
    /// for the Cleanup coloring.
    reclaim: HashMap<NodeId, (u64, u64)>,
}

impl Results {
    /// Builds results from the categories and recommendations of a finished `tree`. Safe
    /// recommendations start selected, like the design.
    pub fn with(
        target: Target,
        tree: Tree,
        categories: Vec<Category>,
        recs: Vec<Recommendation>,
        seconds: f64,
    ) -> Self {
        let target_path = target.path.clone();
        let recs: Vec<Recommendation> = recs
            .into_iter()
            .filter(|rec| rec.size >= MIN_REC_SIZE)
            .collect();
        let mut covered = HashMap::new();
        for (index, rec) in recs.iter().enumerate() {
            for node in &rec.nodes {
                covered.insert(*node, index);
            }
        }
        let selected = recs
            .iter()
            .filter(|rec| rec.status == Status::Safe)
            .map(|rec| rec.rule)
            .collect();
        let scanned = recs.iter().map(|rec| (rec.size, rec.files)).collect();
        let (dominant, category_sums) = dominant_categories(&tree, &categories);
        let mut results = Self {
            target,
            tree,
            categories,
            recs,
            removed: HashSet::new(),
            selected,
            covered,
            scanned,
            fs: vano_scan::root_filesystem_usage(&target_path),
            dominant,
            category_sums,
            reclaim: HashMap::new(),
            seconds,
        };
        results.refresh_reclaim();
        results
    }

    /// The category that takes the most bytes inside `id`.
    pub fn dominant(&self, id: NodeId) -> Category {
        self.dominant
            .get(id as usize)
            .copied()
            .unwrap_or_else(|| self.category(id))
    }

    /// Bytes per category inside `id`, in [`Category::ALL`] order.
    pub fn category_bytes(&self, id: NodeId) -> [u64; 6] {
        self.category_sums.of(&self.tree, &self.categories, id)
    }

    /// Bytes of active Safe and Review recommendations inside `id` (the node included).
    pub fn reclaimable(&self, id: NodeId) -> (u64, u64) {
        self.reclaim.get(&id).copied().unwrap_or_default()
    }

    fn refresh_reclaim(&mut self) {
        let mut reclaim: HashMap<NodeId, (u64, u64)> = HashMap::new();
        for rec in self.active() {
            for node in &rec.nodes {
                if self.tree.is_removed(*node) {
                    continue;
                }
                let size = self.tree.node(*node).size;
                for id in self.tree.ancestry(*node) {
                    let entry = reclaim.entry(id).or_default();
                    match rec.status {
                        Status::Safe => entry.0 += size,
                        Status::Review => entry.1 += size,
                    }
                }
            }
        }
        self.reclaim = reclaim;
    }

    /// The recommendations still shown, in order.
    pub fn active(&self) -> impl Iterator<Item = &Recommendation> {
        self.recs
            .iter()
            .filter(|rec| !self.removed.contains(rec.rule))
    }

    /// The recommendation with this rule id.
    pub fn rec(&self, rule: &str) -> Option<&Recommendation> {
        self.recs.iter().find(|rec| rec.rule == rule)
    }

    /// Size and count of the active recommendations of `status`.
    pub fn total(&self, status: Status) -> (u64, usize) {
        self.active()
            .filter(|rec| rec.status == status)
            .fold((0, 0), |(size, count), rec| (size + rec.size, count + 1))
    }

    /// The active, selected recommendations.
    pub fn selection(&self) -> Vec<&Recommendation> {
        self.active()
            .filter(|rec| self.selected.contains(rec.rule))
            .collect()
    }

    /// Size of the selection.
    pub fn selected_size(&self) -> u64 {
        self.selection().iter().map(|rec| rec.size).sum()
    }

    /// Ticks or unticks one recommendation.
    pub fn toggle(&mut self, rule: &'static str) {
        if !self.selected.remove(rule) {
            self.selected.insert(rule);
        }
    }

    /// How many of the active recommendations of `status` are ticked, and how many there are.
    pub fn group_selection(&self, status: Status) -> (usize, usize) {
        self.active()
            .filter(|rec| rec.status == status)
            .fold((0, 0), |(on, all), rec| {
                (on + usize::from(self.selected.contains(rec.rule)), all + 1)
            })
    }

    /// The group checkbox: clears the group when anything in it is ticked, ticks all of it
    /// otherwise.
    pub fn toggle_group(&mut self, status: Status) {
        let rules: Vec<&'static str> = self
            .active()
            .filter(|rec| rec.status == status)
            .map(|rec| rec.rule)
            .collect();
        if self.group_selection(status).0 > 0 {
            for rule in rules {
                self.selected.remove(rule);
            }
        } else {
            self.selected.extend(rules);
        }
    }

    /// The status of the active recommendation covering `id` (the node itself or an ancestor).
    pub fn status_of(&self, id: NodeId) -> Option<Status> {
        self.covering(id).map(|rec| rec.status)
    }

    /// The active recommendation covering `id` or an ancestor of it.
    pub fn covering(&self, id: NodeId) -> Option<&Recommendation> {
        let mut cur = id;
        loop {
            if let Some(index) = self.covered.get(&cur) {
                let rec = &self.recs[*index];
                if !self.removed.contains(rec.rule) {
                    return Some(rec);
                }
            }
            if cur == ROOT {
                return None;
            }
            cur = self.tree.node(cur).parent;
        }
    }

    /// The active recommendations with nodes inside `id`, and how many bytes of each are there.
    pub fn recs_inside(&self, id: NodeId) -> Vec<(&Recommendation, u64)> {
        self.active()
            .filter_map(|rec| {
                let size: u64 = rec
                    .nodes
                    .iter()
                    .filter(|node| self.tree.is_within(**node, id) && !self.tree.is_removed(**node))
                    .map(|node| self.tree.node(*node).size)
                    .sum();
                (size > 0).then_some((rec, size))
            })
            .collect()
    }

    /// The category of `id`.
    pub fn category(&self, id: NodeId) -> Category {
        self.categories
            .get(id as usize)
            .copied()
            .unwrap_or_default()
    }

    /// Records that `nodes` went to the Trash: they stop counting, and a recommendation whose
    /// nodes are all gone is removed from the list.
    pub fn mark_trashed(&mut self, nodes: &[NodeId]) {
        for node in nodes {
            if !self.tree.is_removed(*node) {
                self.category_sums
                    .shift(&self.tree, &self.categories, *node, false);
            }
            self.tree.mark_removed(*node);
        }
        self.refresh_removed();
    }

    /// Undoes [`Self::mark_trashed`] for `nodes`, after they were put back.
    pub fn mark_restored(&mut self, nodes: &[NodeId]) {
        for node in nodes {
            self.tree.unmark_removed(*node);
            if !self.tree.is_removed(*node) {
                self.category_sums
                    .shift(&self.tree, &self.categories, *node, true);
            }
        }
        self.refresh_removed();
    }

    fn refresh_removed(&mut self) {
        let tree = &self.tree;
        // A recommendation partly moved (some items failed) counts only what is left.
        for (rec, (size, files)) in self.recs.iter_mut().zip(&self.scanned) {
            let left = rec.nodes.iter().filter(|node| !tree.is_removed(**node));
            if left.clone().count() == rec.nodes.len() {
                (rec.size, rec.files) = (*size, *files);
            } else {
                rec.size = left.clone().map(|node| tree.node(*node).size).sum();
                rec.files = left.map(|node| u64::from(tree.node(*node).files)).sum();
            }
        }
        self.removed = self
            .recs
            .iter()
            .filter(|rec| rec.nodes.iter().all(|node| tree.is_removed(*node)))
            .map(|rec| rec.rule)
            .collect();
        let removed = &self.removed;
        self.selected.retain(|rule| !removed.contains(rule));
        self.refresh_reclaim();
    }
}

/// [`Results::dominant`] for every node, in one pass. A child always has a higher id than its
/// parent (the scanner reserves a folder's children after reading the folder), so walking the
/// ids backwards sees every folder after everything inside it.
fn dominant_categories(tree: &Tree, categories: &[Category]) -> (Vec<Category>, CategorySums) {
    let len = tree.len();
    let own = |id: usize| categories.get(id).copied().unwrap_or_default();
    // One row of per-category bytes for each folder only: a few MB even for a whole disk.
    let mut slot = vec![u32::MAX; len];
    let mut sums: Vec<[u64; 6]> = Vec::new();
    for (id, slot) in slot.iter_mut().enumerate() {
        if tree.node(id as NodeId).is_dir() {
            *slot = sums.len() as u32;
            sums.push([0; 6]);
        }
    }
    let mut dominant = vec![Category::Other; len];
    for id in (0..len).rev() {
        let node = tree.node(id as NodeId);
        let row = if node.is_dir() {
            let row = sums[slot[id] as usize];
            dominant[id] = match row.iter().enumerate().max_by_key(|(_, bytes)| **bytes) {
                Some((best, bytes)) if *bytes > 0 => Category::ALL[best],
                _ => own(id),
            };
            row
        } else {
            dominant[id] = own(id);
            let mut row = [0; 6];
            row[category_index(own(id))] = node.size;
            row
        };
        if id == 0 || tree.is_removed(id as NodeId) {
            continue;
        }
        let parent = node.parent as usize;
        if let Some(target) = sums.get_mut(slot[parent] as usize) {
            for (sum, bytes) in target.iter_mut().zip(row) {
                *sum += bytes;
            }
        }
    }
    (dominant, CategorySums { slot, sums })
}

/// The position of `category` in [`Category::ALL`].
fn category_index(category: Category) -> usize {
    Category::ALL
        .iter()
        .position(|c| *c == category)
        .unwrap_or(1)
}

/// Bytes per category of every folder.
struct CategorySums {
    /// The row of each node in `sums`, `u32::MAX` for files.
    slot: Vec<u32>,
    sums: Vec<[u64; 6]>,
}

impl CategorySums {
    fn of(&self, tree: &Tree, categories: &[Category], id: NodeId) -> [u64; 6] {
        match self
            .slot
            .get(id as usize)
            .and_then(|slot| self.sums.get(*slot as usize))
        {
            Some(row) => *row,
            None => {
                let mut row = [0; 6];
                let own = categories.get(id as usize).copied().unwrap_or_default();
                row[category_index(own)] = tree.node(id).size;
                row
            }
        }
    }

    /// Adds (`add`) or subtracts the bytes of `id` to every ancestor of it.
    fn shift(&mut self, tree: &Tree, categories: &[Category], id: NodeId, add: bool) {
        let row = self.of(tree, categories, id);
        let mut cur = tree.node(id).parent;
        while cur != vano_core::NO_NODE {
            if let Some(target) = self
                .slot
                .get(cur as usize)
                .and_then(|slot| self.sums.get_mut(*slot as usize))
            {
                for (sum, bytes) in target.iter_mut().zip(row) {
                    *sum = if add {
                        *sum + bytes
                    } else {
                        sum.saturating_sub(bytes)
                    };
                }
            }
            cur = tree.node(cur).parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use vano_core::{EntryData, Kind};

    const MB: u64 = 1_000_000;

    fn entry(name: &str, kind: Kind, size: u64) -> EntryData {
        EntryData {
            name: name.as_bytes().to_vec(),
            kind,
            size,
            mtime: 0,
            flags: 0,
        }
    }

    fn rec(rule: &'static str, status: Status, nodes: Vec<NodeId>, size: u64) -> Recommendation {
        Recommendation {
            rule,
            status,
            nodes,
            size,
            files: 1,
            newest_mtime: 0,
            approx: false,
            display_path: String::new(),
        }
    }

    fn sample() -> Results {
        let mut tree = Tree::new("/h");
        tree.set_children(
            ROOT,
            1,
            &[entry(".npm", Kind::Dir, 0), entry("models", Kind::Dir, 0)],
        );
        tree.set_children(1, 3, &[entry("a", Kind::File, 100 * MB)]);
        tree.set_children(2, 4, &[entry("b", Kind::File, 50 * MB)]);
        let len = tree.len();
        Results::with(
            Target::folder(PathBuf::from("/h")),
            tree,
            vec![Category::Other; len],
            vec![
                rec("npm", Status::Safe, vec![1], 100 * MB),
                rec("models", Status::Review, vec![2], 50 * MB),
            ],
            1.0,
        )
    }

    #[test]
    fn tiny_recs_are_dropped() {
        let results = Results::with(
            Target::folder(PathBuf::from("/h")),
            Tree::new("/h"),
            vec![Category::Other],
            vec![rec("trash", Status::Review, vec![ROOT], 16_000)],
            1.0,
        );
        assert_eq!(results.active().count(), 0);
    }

    #[test]
    fn dominant_category_follows_the_bytes() {
        let mut tree = Tree::new("/h");
        tree.set_children(ROOT, 1, &[entry("mixed", Kind::Dir, 0)]);
        tree.set_children(
            1,
            2,
            &[
                entry("a.rs", Kind::File, 90),
                entry("b.jpg", Kind::File, 10),
            ],
        );
        let categories = vec![
            Category::Other,
            Category::Other,
            Category::Code,
            Category::Media,
        ];
        let (dominant, sums) = dominant_categories(&tree, &categories);
        assert_eq!(sums.of(&tree, &categories, 1)[2], 90);
        assert_eq!(dominant[1], Category::Code);
        assert_eq!(dominant[0], Category::Code);
        assert_eq!(dominant[3], Category::Media);
    }

    #[test]
    fn reclaimable_bytes_add_up_the_chain() {
        let mut results = sample();
        assert_eq!(results.reclaimable(ROOT), (100 * MB, 50 * MB));
        assert_eq!(results.reclaimable(1), (100 * MB, 0));
        results.mark_trashed(&[1]);
        assert_eq!(results.reclaimable(ROOT), (0, 50 * MB));
    }

    #[test]
    fn safe_recs_start_selected() {
        let results = sample();
        assert_eq!(results.selected_size(), 100 * MB);
        assert_eq!(results.total(Status::Review), (50 * MB, 1));
    }

    #[test]
    fn the_group_checkbox_clears_a_ticked_group_and_ticks_an_empty_one() {
        let mut results = sample();
        assert_eq!(results.group_selection(Status::Safe), (1, 1));
        assert_eq!(results.group_selection(Status::Review), (0, 1));
        results.toggle_group(Status::Safe);
        assert_eq!(results.group_selection(Status::Safe), (0, 1));
        results.toggle_group(Status::Review);
        assert_eq!(results.group_selection(Status::Review), (1, 1));
        // The other group is left alone.
        assert_eq!(results.group_selection(Status::Safe), (0, 1));
        results.toggle_group(Status::Safe);
        assert_eq!(results.selected_size(), 150 * MB);
    }

    #[test]
    fn status_follows_ancestors() {
        let results = sample();
        assert_eq!(results.status_of(3), Some(Status::Safe));
        assert_eq!(results.status_of(4), Some(Status::Review));
        assert_eq!(results.status_of(ROOT), None);
        assert_eq!(results.recs_inside(ROOT).len(), 2);
    }

    #[test]
    fn a_rec_partly_trashed_counts_what_is_left() {
        let mut tree = Tree::new("/h");
        tree.set_children(
            ROOT,
            1,
            &[entry("p1", Kind::Dir, 0), entry("p2", Kind::Dir, 0)],
        );
        tree.set_children(1, 3, &[entry("a", Kind::File, 100 * MB)]);
        tree.set_children(2, 4, &[entry("b", Kind::File, 50 * MB)]);
        let len = tree.len();
        let mut results = Results::with(
            Target::folder(PathBuf::from("/h")),
            tree,
            vec![Category::Other; len],
            vec![rec("nm", Status::Review, vec![1, 2], 150 * MB)],
            1.0,
        );
        results.mark_trashed(&[1]);
        assert_eq!(results.active().count(), 1);
        assert_eq!(results.rec("nm").map(|rec| rec.size), Some(50 * MB));
        results.mark_restored(&[1]);
        assert_eq!(results.rec("nm").map(|rec| rec.size), Some(150 * MB));
    }

    #[test]
    fn trashing_removes_the_rec_and_undo_brings_it_back() {
        let mut results = sample();
        results.mark_trashed(&[1]);
        assert_eq!(results.active().count(), 1);
        assert_eq!(results.tree.node(ROOT).size, 50 * MB);
        assert_eq!(results.selected_size(), 0);
        results.mark_restored(&[1]);
        assert_eq!(results.active().count(), 2);
        assert_eq!(results.tree.node(ROOT).size, 150 * MB);
    }
}
