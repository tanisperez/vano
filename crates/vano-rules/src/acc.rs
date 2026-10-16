//! Collects the nodes matched by one rule and turns them into a [`Recommendation`].

use std::cmp::Reverse;
use std::path::Path;

use vano_core::{NodeId, Tree};

use crate::Recommendation;
use crate::defs::{R, RULES, is_file_rule};

/// How many of the biggest matches are kept with their path, to write the display path.
const SAMPLE: usize = 64;

pub(crate) struct Acc {
    rule: R,
    nodes: Vec<(u64, NodeId)>,
    size: u64,
    files: u64,
    newest: u32,
    /// The biggest matches by `(size desc, id asc)`, with their display path. The order is total,
    /// so the sample does not depend on the order the matches arrive in.
    top: Vec<(u64, NodeId, String)>,
}

impl Acc {
    pub fn new(rule: R) -> Self {
        Self {
            rule,
            nodes: Vec::new(),
            size: 0,
            files: 0,
            newest: 0,
            top: Vec::new(),
        }
    }

    pub fn add(&mut self, tree: &Tree, home: Option<&Path>, id: NodeId) {
        let n = tree.node(id);
        self.nodes.push((n.size, id));
        self.size += n.size;
        self.files += u64::from(n.files);
        self.newest = self.newest.max(n.mtime);
        let key = (Reverse(n.size), id);
        let pos = self
            .top
            .partition_point(|(s, i, _)| (Reverse(*s), *i) < key);
        if pos < SAMPLE {
            self.top
                .insert(pos, (n.size, id, pretty(&tree.path(id), home)));
            self.top.truncate(SAMPLE);
        }
    }

    pub fn recommendation(&self) -> Recommendation {
        let def = &RULES[self.rule as usize];
        let mut nodes = self.nodes.clone();
        nodes.sort_by_key(|(size, id)| (Reverse(*size), *id));
        let paths: Vec<&str> = self.top.iter().map(|(_, _, p)| p.as_str()).collect();
        Recommendation {
            rule: def.id,
            status: def.status,
            nodes: nodes.into_iter().map(|(_, id)| id).collect(),
            size: self.size,
            files: self.files,
            newest_mtime: self.newest,
            approx: def.approx,
            display_path: if is_file_rule(self.rule) {
                file_summary(&paths)
            } else {
                dir_summary(&paths)
            },
        }
    }
}

/// `path` with the home folder written as `~` and `/` as the separator.
fn pretty(path: &Path, home: Option<&Path>) -> String {
    let text = match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.to_string_lossy()),
        None => path.to_string_lossy().into_owned(),
    };
    text.replace('\\', "/")
}

fn unique<'a>(items: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// A short pattern for folders: the paths themselves when there are one or two, otherwise
/// `prefix/{a,b,c}` (same name in a few projects shows the projects), `prefix/*/name` or
/// `prefix/*/name1, name2`.
fn dir_summary(paths: &[&str]) -> String {
    let uniq = unique(paths.iter().copied());
    if uniq.len() <= 2 {
        return uniq.join(", ");
    }
    let split: Vec<(&str, &str)> = uniq
        .iter()
        .map(|p| p.rsplit_once('/').unwrap_or(("", p)))
        .collect();
    let prefix = common_prefix(&split.iter().map(|s| s.0).collect::<Vec<_>>());
    let names = unique(split.iter().map(|s| s.1));
    let rels = unique(
        split
            .iter()
            .map(|s| s.0[prefix.len()..].trim_start_matches('/')),
    );
    if names.len() == 1 {
        if rels.len() <= 3 && rels.iter().all(|r| !r.is_empty()) {
            format!("{prefix}/{{{}}}", rels.join(","))
        } else {
            format!("{prefix}/*/{}", names[0])
        }
    } else if rels.len() == 1 {
        if names.len() <= 3 {
            format!("{prefix}/{{{}}}", names.join(","))
        } else {
            format!("{prefix}/*")
        }
    } else if names.len() <= 3 {
        format!("{prefix}/*/{}", names.join(", "))
    } else {
        format!("{prefix}/*")
    }
}

/// The longest leading run of whole path segments shared by all `dirs`.
fn common_prefix<'a>(dirs: &[&'a str]) -> &'a str {
    let Some(first) = dirs.first() else {
        return "";
    };
    let mut end = first.len();
    for d in &dirs[1..] {
        let mut common = 0;
        let mut pos = 0;
        for (a, b) in first.split('/').zip(d.split('/')) {
            if a != b {
                break;
            }
            pos += a.len();
            common = pos;
            pos += 1;
        }
        end = end.min(common);
    }
    &first[..end]
}

/// `dir/*.ext, *.ext` for each folder holding files (at most two folders).
fn file_summary(paths: &[&str]) -> String {
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for p in paths {
        let (dir, name) = p.rsplit_once('/').unwrap_or(("", p));
        let pattern = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => format!("*.{}", ext.to_lowercase()),
            _ => name.to_string(),
        };
        match groups.iter_mut().find(|(d, _)| *d == dir) {
            Some((_, pats)) => {
                if !pats.contains(&pattern) {
                    pats.push(pattern);
                }
            }
            None => groups.push((dir, vec![pattern])),
        }
    }
    groups
        .into_iter()
        .take(2)
        .map(|(dir, mut pats)| {
            pats.sort();
            format!("{dir}/{}", pats.join(", "))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
