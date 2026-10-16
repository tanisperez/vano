//! The explorer's treemap layout: which tiles to draw for a focused folder, and where.
//!
//! Two levels are drawn, like the design: the focused folder's children as top tiles, and inside
//! each top tile big enough to hold them, its own children. Small entries are grouped into one
//! "N more" tile per level so a folder with a hundred thousand files still costs a few hundred
//! tiles. Everything here is plain Rust over a [`Tree`]; the view turns tiles into elements and
//! [`super::motion`] animates them.

use vano_core::treemap::{Rect, squarify};
use vano_core::{NodeId, Tree};

/// Gap between top tiles, per side (the design's `x + 2, w - 4`).
const TOP_INSET: f32 = 2.0;
/// Gap between nested tiles, per side.
const KID_INSET: f32 = 1.5;
/// Height of a parent tile's header (its name and size) above its nested tiles.
pub const PARENT_HEADER: f32 = 27.0;
/// Padding inside a parent tile around its nested tiles.
const PARENT_PADDING: f32 = 5.0;
/// A top tile shows its children only when it is at least this wide and tall.
const PARENT_MIN_W: f32 = 64.0;
const PARENT_MIN_H: f32 = 68.0;
/// Entries whose tile would be smaller than this many square pixels go into the "more" tile.
const MIN_TOP_AREA: f32 = 900.0;
const MIN_KID_AREA: f32 = 500.0;
/// Hard caps on tiles per level, whatever the sizes.
const MAX_TOP: usize = 90;
const MAX_KIDS: usize = 40;

/// What identifies a tile across layouts, so the animation can follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TileKey {
    /// The tile of one node.
    Node(NodeId),
    /// The "N more" tile grouping the small children of a folder.
    More(NodeId),
}

/// How a tile is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileRole {
    /// A top tile that shows its children inside, under a header.
    Parent,
    /// A top tile drawn as one block (a file, an empty or small folder).
    Leaf,
    /// A tile nested in a parent.
    Kid,
}

/// One tile to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub key: TileKey,
    /// The node, or the folder whose small children a `More` tile groups.
    pub node: NodeId,
    pub role: TileRole,
    pub rect: Rect,
    /// Bytes the tile stands for.
    pub size: u64,
    /// For a `More` tile, how many entries it groups.
    pub grouped: usize,
    /// Whether the node is a folder with something inside (clicking zooms into it).
    pub openable: bool,
}

/// Lays out the children of `focus` inside `area`, which is in the view's own pixel space.
/// Top tiles come first in the result (biggest first), each followed by its nested tiles, so
/// drawing in order paints parents under their children.
pub fn layout(tree: &Tree, focus: NodeId, area: Rect) -> Vec<Tile> {
    let mut tiles = Vec::new();
    if area.w <= 1.0 || area.h <= 1.0 {
        return tiles;
    }
    let top = pick(tree, focus, area.area(), MIN_TOP_AREA, MAX_TOP);
    let values: Vec<f64> = top.iter().map(|entry| entry.size as f64).collect();
    let rects = squarify(&values, area);
    for (entry, rect) in top.iter().zip(rects) {
        let rect = rect.inset(TOP_INSET);
        let node = entry.node;
        let openable = entry.grouped == 0 && has_children(tree, node);
        let fits = openable && rect.w >= PARENT_MIN_W && rect.h >= PARENT_MIN_H;
        tiles.push(Tile {
            key: entry.key,
            node,
            role: if fits {
                TileRole::Parent
            } else {
                TileRole::Leaf
            },
            rect,
            size: entry.size,
            grouped: entry.grouped,
            openable,
        });
        if fits {
            let inner = rect.inset_sides(
                PARENT_HEADER,
                PARENT_PADDING,
                PARENT_PADDING,
                PARENT_PADDING,
            );
            let kids = pick(tree, node, inner.area(), MIN_KID_AREA, MAX_KIDS);
            let values: Vec<f64> = kids.iter().map(|kid| kid.size as f64).collect();
            let rects = squarify(&values, inner);
            for (kid, rect) in kids.iter().zip(rects) {
                tiles.push(Tile {
                    key: kid.key,
                    node: kid.node,
                    role: TileRole::Kid,
                    rect: rect.inset(KID_INSET),
                    size: kid.size,
                    grouped: kid.grouped,
                    openable: kid.grouped == 0 && has_children(tree, kid.node),
                });
            }
        }
    }
    tiles
}

/// Where a node's tile sits in `tiles`, if it is drawn.
#[cfg(test)]
pub fn find(tiles: &[Tile], key: TileKey) -> Option<&Tile> {
    tiles.iter().find(|tile| tile.key == key)
}

/// The tile under `point`, preferring nested tiles over their parent.
#[cfg(test)]
pub fn hit(tiles: &[Tile], x: f32, y: f32) -> Option<&Tile> {
    let inside = |r: &Rect| x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h;
    tiles.iter().rev().find(|tile| inside(&tile.rect))
}

struct Entry {
    key: TileKey,
    node: NodeId,
    size: u64,
    grouped: usize,
}

/// The children of `dir` worth a tile of their own in `area` square pixels, biggest first, plus
/// one `More` entry for the rest when it is not empty.
fn pick(tree: &Tree, dir: NodeId, area: f32, min_area: f32, max: usize) -> Vec<Entry> {
    let children = tree.sorted_children(dir);
    let total: u64 = children.iter().map(|id| tree.node(*id).size).sum();
    let mut out = Vec::new();
    if total == 0 {
        return out;
    }
    let per_byte = f64::from(area) / total as f64;
    let mut rest = 0u64;
    let mut rest_count = 0usize;
    let mut rest_id = dir;
    for id in children {
        let size = tree.node(id).size;
        if size == 0 {
            continue;
        }
        let tile_area = size as f64 * per_byte;
        if out.len() < max && tile_area >= f64::from(min_area) {
            out.push(Entry {
                key: TileKey::Node(id),
                node: id,
                size,
                grouped: 0,
            });
        } else {
            rest += size;
            rest_count += 1;
            rest_id = id;
        }
    }
    match rest_count {
        0 => {}
        // A single leftover entry is shown as itself rather than as "1 more".
        1 => out.push(Entry {
            key: TileKey::Node(rest_id),
            node: rest_id,
            size: rest,
            grouped: 0,
        }),
        _ => out.push(Entry {
            key: TileKey::More(dir),
            node: dir,
            size: rest,
            grouped: rest_count,
        }),
    }
    out
}

fn has_children(tree: &Tree, id: NodeId) -> bool {
    let node = tree.node(id);
    node.is_dir() && !tree.children(id).is_empty() && node.size > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use vano_core::{EntryData, Kind, ROOT};

    fn entry(name: &str, kind: Kind, size: u64) -> EntryData {
        EntryData {
            name: name.as_bytes().to_vec(),
            kind,
            size,
            mtime: 0,
            flags: 0,
        }
    }

    /// root: big/ (a 600, b 300, 50 tiny files of 1), small.bin 100
    fn sample() -> Tree {
        let mut tree = Tree::new("/r");
        tree.set_children(
            ROOT,
            1,
            &[
                entry("big", Kind::Dir, 0),
                entry("small.bin", Kind::File, 100),
            ],
        );
        let mut kids = vec![entry("a", Kind::File, 600), entry("b", Kind::File, 300)];
        for i in 0..50 {
            kids.push(entry(&format!("t{i}"), Kind::File, 1));
        }
        tree.set_children(1, 3, &kids);
        tree
    }

    #[test]
    fn nests_children_and_groups_the_small_ones() {
        let tree = sample();
        let tiles = layout(&tree, ROOT, Rect::new(0.0, 0.0, 800.0, 500.0));
        let big = find(&tiles, TileKey::Node(1)).map(|t| t.role);
        assert_eq!(big, Some(TileRole::Parent));
        assert!(find(&tiles, TileKey::Node(3)).is_some());
        let more = find(&tiles, TileKey::More(1)).map(|t| (t.grouped, t.size));
        assert_eq!(more, Some((50, 50)));
        assert_eq!(
            find(&tiles, TileKey::Node(2)).map(|t| t.role),
            Some(TileRole::Leaf)
        );
    }

    #[test]
    fn hit_prefers_nested_tiles() {
        let tree = sample();
        let tiles = layout(&tree, ROOT, Rect::new(0.0, 0.0, 800.0, 500.0));
        let a = find(&tiles, TileKey::Node(3))
            .map(|t| t.rect)
            .unwrap_or_default();
        let hit = hit(&tiles, a.x + 1.0, a.y + 1.0).map(|t| t.key);
        assert_eq!(hit, Some(TileKey::Node(3)));
    }

    #[test]
    fn empty_area_gives_no_tiles() {
        let tree = sample();
        assert!(layout(&tree, ROOT, Rect::new(0.0, 0.0, 0.0, 0.0)).is_empty());
    }
}
