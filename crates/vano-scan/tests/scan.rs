#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use vano_core::tree::{
    FLAG_COMPLETE, FLAG_HARDLINK_DUP, FLAG_UNREADABLE, Kind, NodeId, ROOT, Tree,
};
use vano_scan::{ScanHandle, ScanOptions, cache, start};

fn write(path: &Path, len: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; len]).unwrap();
}

fn scan(root: &Path) -> (ScanHandle, Tree) {
    let handle = start(ScanOptions::new(root));
    handle.wait();
    assert!(handle.is_finished());
    let tree = handle.take_tree().unwrap();
    (handle, tree)
}

/// Allocated size of a file on this file system, which is what the scanner reports.
#[cfg(unix)]
fn allocated(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    fs::symlink_metadata(path).unwrap().blocks() * 512
}

#[cfg(not(unix))]
fn allocated(path: &Path) -> u64 {
    fs::symlink_metadata(path).unwrap().len()
}

fn build_sample(root: &Path) {
    write(&root.join("a.bin"), 10_000);
    write(&root.join("d1/b.bin"), 5_000);
    write(&root.join("d1/d2/c.bin"), 123);
    write(&root.join("d1/d2/d3/e.bin"), 70_000);
    fs::create_dir_all(root.join("empty")).unwrap();
}

#[test]
fn sizes_and_counts_match() {
    let tmp = tempfile::tempdir().unwrap();
    build_sample(tmp.path());
    let (handle, tree) = scan(tmp.path());
    let expected: u64 = ["a.bin", "d1/b.bin", "d1/d2/c.bin", "d1/d2/d3/e.bin"]
        .iter()
        .map(|p| allocated(&tmp.path().join(p)))
        .sum();
    assert_eq!(tree.node(ROOT).size, expected);
    assert_eq!(tree.node(ROOT).files, 4);
    let d1 = tree.child_named(ROOT, "d1").unwrap();
    assert_eq!(tree.node(d1).files, 3);
    let progress = handle.progress();
    assert_eq!(progress.files, 4);
    assert_eq!(progress.dirs, 5);
    assert_eq!(progress.bytes, expected);
    assert_eq!(progress.errors, 0);
    assert!(progress.finished && !progress.cancelled);
    assert!(tree.node(ROOT).mtime > 0);
    assert_eq!(
        tree.find(&tmp.path().join("d1/d2/c.bin"))
            .map(|id| tree.node(id).kind),
        Some(Kind::File)
    );
}

#[test]
fn children_ids_are_contiguous_and_everything_completes() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..20 {
        for j in 0..5 {
            write(
                &tmp.path().join(format!("d{i}/s{j}/f{i}_{j}.bin")),
                100 + i * j,
            );
        }
    }
    let (_handle, tree) = scan(tmp.path());
    let mut seen = vec![false; tree.len()];
    seen[0] = true;
    for id in 0..tree.len() as NodeId {
        let node = tree.node(id);
        if node.is_dir() {
            assert!(node.has(FLAG_COMPLETE), "dir {id} not complete");
            let mut sum = 0;
            for child in tree.children(id) {
                assert_eq!(tree.node(child).parent, id);
                assert!(!seen[child as usize], "node {child} listed twice");
                seen[child as usize] = true;
                sum += tree.node(child).size;
            }
            assert_eq!(tree.node(id).size, sum);
        }
    }
    assert!(seen.iter().all(|s| *s), "every slot belongs to a directory");
}

#[test]
fn drain_completed_returns_every_dir_once() {
    let tmp = tempfile::tempdir().unwrap();
    build_sample(tmp.path());
    let handle = start(ScanOptions::new(tmp.path()));
    let mut completed = Vec::new();
    while !handle.is_finished() {
        completed.extend(handle.drain_completed());
        std::thread::sleep(Duration::from_millis(1));
    }
    completed.extend(handle.drain_completed());
    let tree = handle.take_tree().unwrap();
    let dirs: Vec<NodeId> = (0..tree.len() as NodeId)
        .filter(|id| tree.node(*id).is_dir())
        .collect();
    let mut sorted = completed.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, dirs);
    // Children complete before their parents, the root last.
    assert_eq!(completed.last(), Some(&ROOT));
    let pos = |id: NodeId| completed.iter().position(|c| *c == id).unwrap();
    for id in dirs.iter().filter(|id| **id != ROOT) {
        assert!(pos(*id) < pos(tree.node(*id).parent));
    }
}

#[cfg(unix)]
#[test]
fn hard_links_count_once_and_symlinks_are_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("orig.bin"), 50_000);
    fs::hard_link(root.join("orig.bin"), root.join("link.bin")).unwrap();
    fs::create_dir_all(root.join("real")).unwrap();
    write(&root.join("real/big.bin"), 80_000);
    std::os::unix::fs::symlink(root.join("real"), root.join("dirlink")).unwrap();
    let (_handle, tree) = scan(root);

    let a = tree.child_named(ROOT, "orig.bin").unwrap();
    let b = tree.child_named(ROOT, "link.bin").unwrap();
    let (first, dup) = if tree.node(a).has(FLAG_HARDLINK_DUP) {
        (b, a)
    } else {
        (a, b)
    };
    assert!(tree.node(first).size > 0);
    assert_eq!(tree.node(dup).size, 0);
    assert!(tree.node(dup).has(FLAG_HARDLINK_DUP));

    let link = tree.child_named(ROOT, "dirlink").unwrap();
    assert_eq!(tree.node(link).kind, Kind::Symlink);
    assert!(tree.children(link).is_empty());
    let expected = allocated(&root.join("orig.bin"))
        + allocated(&root.join("real/big.bin"))
        + allocated(&root.join("dirlink"));
    assert_eq!(tree.node(ROOT).size, expected);
}

#[cfg(unix)]
#[test]
fn unreadable_dir_is_flagged() {
    use std::os::unix::fs::PermissionsExt as _;
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("ok/f.bin"), 1000);
    write(&tmp.path().join("locked/f.bin"), 1000);
    let locked = tmp.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    // Root ignores permissions: nothing to check then.
    let can_read = fs::read_dir(&locked).is_ok();
    let (handle, tree) = scan(tmp.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if can_read {
        return;
    }
    let id = tree.child_named(ROOT, "locked").unwrap();
    assert!(tree.node(id).has(FLAG_UNREADABLE));
    assert!(tree.node(id).has(FLAG_COMPLETE));
    assert_eq!(handle.progress().errors, 1);
    assert_eq!(tree.node(id).size, 0);
    let ok = tree.child_named(ROOT, "ok").unwrap();
    assert!(tree.node(ok).size > 0);
}

#[test]
fn cancel_returns_quickly() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..60 {
        for j in 0..30 {
            fs::create_dir_all(tmp.path().join(format!("d{i}/s{j}"))).unwrap();
        }
    }
    let handle = start(ScanOptions::new(tmp.path()));
    handle.cancel();
    let begin = Instant::now();
    handle.wait();
    assert!(
        begin.elapsed() < Duration::from_millis(100),
        "{:?}",
        begin.elapsed()
    );
    assert!(handle.is_finished());
    // A finished scan can still be taken and is consistent.
    let tree = handle.take_tree().unwrap();
    assert!(tree.node(ROOT).is_dir());
}

#[test]
fn live_reads_never_see_a_parent_smaller_than_its_children() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..40 {
        for j in 0..40 {
            write(&tmp.path().join(format!("a{i}/b{j}/c/f.bin")), 1 + i * j);
        }
    }
    let handle = start(ScanOptions::new(tmp.path()));
    let bad = AtomicBool::new(false);
    let mut reads = 0;
    while !handle.is_finished() {
        handle.read_tree(|tree| {
            for id in 0..tree.len() as NodeId {
                let node = tree.node(id);
                if node.is_dir() {
                    let sum: u64 = tree.children(id).map(|c| tree.node(c).size).sum();
                    if node.size < sum {
                        bad.store(true, Ordering::Relaxed);
                    }
                }
            }
        });
        reads += 1;
    }
    assert!(!bad.load(Ordering::Relaxed));
    assert!(reads > 0);
}

#[test]
fn nonexistent_root_finishes_with_an_error() {
    let handle = start(ScanOptions::new("/definitely/not/here"));
    handle.wait();
    let progress = handle.progress();
    assert_eq!(progress.errors, 1);
    assert!(!progress.cancelled);
    let tree = handle.take_tree().unwrap();
    assert!(tree.node(ROOT).has(FLAG_UNREADABLE));
    assert!(handle.take_tree().is_none());
}

#[test]
fn cache_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    build_sample(tmp.path());
    let (_handle, tree) = scan(tmp.path());
    let file = tmp.path().join("cache/out.vano");
    cache::save(&tree, &file).unwrap();
    assert!(!tmp.path().join("cache/out.vano.tmp").exists());
    let back = cache::load(&file).unwrap();
    assert_eq!(back.node(ROOT).size, tree.node(ROOT).size);
    assert_eq!(back.len(), tree.len());
    fs::write(&file, b"garbage").unwrap();
    assert!(cache::load(&file).is_err());
}

#[test]
fn filesystem_usage_is_sane() {
    let tmp = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        let usage = vano_scan::root_filesystem_usage(tmp.path()).unwrap();
        assert!(usage.total > 0 && usage.available <= usage.free && usage.free <= usage.total);
    }
    #[cfg(not(unix))]
    let _ = tmp;
}

#[cfg(unix)]
#[test]
fn cancel_during_a_big_scan_returns_quickly() {
    let handle = start(ScanOptions::new("/usr"));
    std::thread::sleep(Duration::from_millis(5));
    let begin = Instant::now();
    handle.cancel();
    handle.wait();
    assert!(
        begin.elapsed() < Duration::from_millis(100),
        "{:?}",
        begin.elapsed()
    );
}
