//! Times `categorize`, `evaluate` and `LiveRules` on a synthetic tree of about 2 million nodes.
//!
//! Run with `cargo run --release -p vano-rules --example perf`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use vano_core::{EntryData, Kind, NodeId, Tree};
use vano_rules::{Context, LiveRules, Os, categorize, category_of, evaluate};

const TARGET: usize = 2_000_000;
const NOW: u32 = 1_800_000_000;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn entry(name: &str, kind: Kind, size: u64, mtime: u32) -> EntryData {
    EntryData {
        name: name.as_bytes().to_vec(),
        kind,
        size,
        mtime,
        flags: 0,
    }
}

/// Random content: `breadth` entries, a third of them folders.
fn filler(rng: &mut Rng, breadth: u64) -> Vec<EntryData> {
    (0..breadth)
        .map(|i| {
            let mtime = NOW - rng.below(400 * 86_400) as u32;
            if rng.below(3) == 0 {
                entry(&format!("dir{i}"), Kind::Dir, 0, mtime)
            } else {
                let ext = ["rs", "js", "png", "txt", "mp4", "bin"][rng.below(6) as usize];
                entry(
                    &format!("file{i}.{ext}"),
                    Kind::File,
                    4096 * (1 + rng.below(50)),
                    mtime,
                )
            }
        })
        .collect()
}

fn main() {
    let mut rng = Rng(42);
    let mut tree = Tree::new(PathBuf::from("/home/u"));
    // (id, depth, special project)
    let mut queue: VecDeque<(NodeId, u32, u8)> = VecDeque::new();
    queue.push_back((0, 0, 0));
    let mut next: NodeId = 1;
    let t0 = Instant::now();
    while let Some((id, depth, kind)) = queue.pop_front() {
        let full = (next as usize) >= TARGET;
        let entries: Vec<EntryData> = match (kind, depth) {
            _ if full => Vec::new(),
            (0, 0) => {
                let mut v = vec![
                    entry(".npm", Kind::Dir, 0, NOW),
                    entry(".cache", Kind::Dir, 0, NOW),
                    entry("Documents", Kind::Dir, 0, NOW),
                    entry("Downloads", Kind::Dir, 0, NOW),
                ];
                for i in 0..2000 {
                    v.push(entry(
                        &format!("project{i}"),
                        Kind::Dir,
                        0,
                        NOW - 90 * 86_400,
                    ));
                }
                v
            }
            (0, 1) => filler(&mut rng, 40),
            (1, 1) => {
                // A project: manifest, sources, dependencies, build output.
                vec![
                    entry("Cargo.toml", Kind::File, 4096, NOW - 90 * 86_400),
                    entry("package.json", Kind::File, 4096, NOW - 90 * 86_400),
                    entry("src", Kind::Dir, 0, NOW - 90 * 86_400),
                    entry("target", Kind::Dir, 0, NOW),
                    entry("node_modules", Kind::Dir, 0, NOW),
                    entry(".git", Kind::Dir, 0, NOW),
                ]
            }
            (_, d) if d < 7 => {
                let breadth = 6 + rng.below(8);
                filler(&mut rng, breadth)
            }
            _ => filler(&mut rng, 3),
        };
        if entries.is_empty() {
            continue;
        }
        let first = next;
        next += entries.len() as NodeId;
        tree.set_children(id, first, &entries);
        for (i, e) in entries.iter().enumerate() {
            if e.kind != Kind::Dir {
                continue;
            }
            let child = first + i as NodeId;
            let special = if depth == 0 && e.name.starts_with(b"project") {
                1
            } else {
                0
            };
            queue.push_back((child, depth + 1, special));
        }
    }
    println!("built {} nodes in {:?}", tree.len(), t0.elapsed());

    let ctx = Context {
        home: Some(PathBuf::from("/home/u")),
        now: NOW,
        os: Os::Linux,
    };
    let t = Instant::now();
    let cats = categorize(&tree, &ctx);
    println!("categorize: {:?} ({} nodes)", t.elapsed(), cats.len());

    let t = Instant::now();
    let recs = evaluate(&tree, &ctx);
    println!(
        "evaluate:   {:?} ({} recommendations)",
        t.elapsed(),
        recs.len()
    );
    for r in &recs {
        println!(
            "  {:<14} {:>6} nodes {:>14} bytes  {}",
            r.rule,
            r.nodes.len(),
            r.size,
            r.display_path
        );
    }

    let dirs: Vec<NodeId> = (0..tree.len() as NodeId)
        .filter(|id| tree.node(*id).is_dir())
        .rev()
        .collect();
    let t = Instant::now();
    let mut live = LiveRules::new(ctx.clone());
    live.on_completed(&tree, &dirs);
    println!(
        "live (all {} dirs): {:?}, {} found",
        dirs.len(),
        t.elapsed(),
        live.found().len()
    );

    let t = Instant::now();
    let mut acc = 0usize;
    for i in 0..200u32 {
        acc += category_of(&tree, &ctx, (i * 9973) % tree.len() as u32) as usize;
    }
    println!("category_of x200: {:?} (checksum {acc})", t.elapsed());
}
