//! A trie of path components for the fixed locations of the catalog.
//!
//! The scan walks the tree top down carrying one trie node per folder, so a folder outside the
//! few known locations costs nothing, and the children of a known folder are matched by looking
//! at a handful of keys.

use vano_core::Category;

use crate::defs::R;

/// "No trie node": the folder is not on the way to any known location.
pub(crate) const NONE: u32 = u32::MAX;

/// What the catalog says about one location.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Spec {
    /// The folder is a recommendation of this rule.
    pub rule: Option<R>,
    /// The folder (and what is inside) takes this category.
    pub cat: Option<Category>,
    /// Name based rules and project detection are off inside.
    pub no_names: bool,
    /// The folder is never a code project itself (the home).
    pub not_project: bool,
    /// Files directly inside may match this file rule.
    pub files: Option<R>,
}

#[derive(Default)]
struct TNode {
    children: Vec<(Vec<u8>, u32)>,
    wild: Option<u32>,
    spec: Spec,
}

pub(crate) struct Trie {
    nodes: Vec<TNode>,
    /// Compare names ignoring ASCII case (macOS and Windows file systems).
    ci: bool,
}

impl Trie {
    pub fn new(ci: bool) -> Self {
        Self {
            nodes: vec![TNode::default()],
            ci,
        }
    }

    /// Adds the location `comps` (`*` is a wildcard) and lets `f` set what it means.
    pub fn insert(&mut self, comps: &[Vec<u8>], f: impl FnOnce(&mut Spec)) {
        let mut cur = 0usize;
        for comp in comps {
            let key = if self.ci {
                comp.to_ascii_lowercase()
            } else {
                comp.clone()
            };
            let existing = if key == b"*" {
                self.nodes[cur].wild
            } else {
                self.nodes[cur]
                    .children
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, i)| *i)
            };
            cur = match existing {
                Some(i) => i as usize,
                None => {
                    let idx = self.nodes.len() as u32;
                    self.nodes.push(TNode::default());
                    if key == b"*" {
                        self.nodes[cur].wild = Some(idx);
                    } else {
                        self.nodes[cur].children.push((key, idx));
                    }
                    idx as usize
                }
            };
        }
        f(&mut self.nodes[cur].spec);
    }

    /// The child of `node` called `name`, or [`NONE`].
    pub fn child(&self, node: u32, name: &[u8]) -> u32 {
        let n = &self.nodes[node as usize];
        for (key, idx) in &n.children {
            let same = if self.ci {
                key.eq_ignore_ascii_case(name)
            } else {
                key == name
            };
            if same {
                return *idx;
            }
        }
        n.wild.unwrap_or(NONE)
    }

    pub fn spec(&self, node: u32) -> &Spec {
        &self.nodes[node as usize].spec
    }
}
