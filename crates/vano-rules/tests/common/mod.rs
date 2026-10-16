#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use vano_core::{EntryData, Kind, NodeId, Tree};
use vano_rules::{Context, Os, Recommendation};

pub const DAY: u32 = 86_400;
pub const NOW: u32 = 1_800_000_000;
pub const MB: u64 = 1024 * 1024;

#[derive(Default)]
struct Dir {
    dirs: BTreeMap<String, Dir>,
    files: BTreeMap<String, (u64, u32)>,
}

/// Builds a tree from `(relative path, size, mtime)` files. A path ending in `/` is an empty
/// directory. Ids are assigned breadth first, like the scanner does.
pub fn tree(root: &str, items: &[(&str, u64, u32)]) -> Tree {
    let mut top = Dir::default();
    for (path, size, mtime) in items {
        let (path, is_dir) = match path.strip_suffix('/') {
            Some(p) => (p, true),
            None => (*path, false),
        };
        let mut cur = &mut top;
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if i + 1 == parts.len() && !is_dir {
                cur.files.insert((*part).to_string(), (*size, *mtime));
            } else {
                cur = cur.dirs.entry((*part).to_string()).or_default();
            }
        }
    }
    let mut tree = Tree::new(PathBuf::from(root));
    let mut next: NodeId = 1;
    let mut queue: VecDeque<(NodeId, &Dir)> = VecDeque::new();
    queue.push_back((0, &top));
    while let Some((id, dir)) = queue.pop_front() {
        let mut entries = Vec::new();
        let mut subs = Vec::new();
        for (name, sub) in &dir.dirs {
            entries.push(EntryData {
                name: name.as_bytes().to_vec(),
                kind: Kind::Dir,
                size: 0,
                mtime: 0,
                flags: 0,
            });
            subs.push(sub);
        }
        for (name, (size, mtime)) in &dir.files {
            entries.push(EntryData {
                name: name.as_bytes().to_vec(),
                kind: Kind::File,
                size: *size,
                mtime: *mtime,
                flags: 0,
            });
        }
        if entries.is_empty() {
            continue;
        }
        let first = next;
        next += entries.len() as NodeId;
        tree.set_children(id, first, &entries);
        for (i, sub) in subs.into_iter().enumerate() {
            queue.push_back((first + i as NodeId, sub));
        }
    }
    tree
}

pub fn ctx(os: Os) -> Context {
    let home = match os {
        Os::Windows => "C:\\Users\\u",
        _ => "/home/u",
    };
    Context {
        home: Some(PathBuf::from(home)),
        now: NOW,
        os,
    }
}

pub fn linux() -> Context {
    ctx(Os::Linux)
}

/// The recommendation of `rule`, if present.
pub fn rec<'a>(recs: &'a [Recommendation], rule: &str) -> Option<&'a Recommendation> {
    recs.iter().find(|r| r.rule == rule)
}

/// The paths of the nodes of a recommendation, sorted.
pub fn paths(tree: &Tree, r: &Recommendation) -> Vec<String> {
    let mut v: Vec<String> = r
        .nodes
        .iter()
        .map(|n| tree.path(*n).to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

pub fn rule_names(recs: &[Recommendation]) -> Vec<&'static str> {
    let mut v: Vec<&str> = recs.iter().map(|r| r.rule).collect();
    v.sort_unstable();
    v
}

/// Every directory id, children before parents, like a scan completes them.
pub fn post_order(tree: &Tree) -> Vec<NodeId> {
    fn go(tree: &Tree, id: NodeId, out: &mut Vec<NodeId>) {
        for c in tree.children(id) {
            if tree.node(c).is_dir() {
                go(tree, c, out);
            }
        }
        out.push(id);
    }
    let mut out = Vec::new();
    go(tree, 0, &mut out);
    out
}
