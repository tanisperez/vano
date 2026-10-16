//! `cargo run --release -p vano-scan --example bench -- <path> [threads] [--cross]`

use std::path::PathBuf;
use std::time::Instant;

use vano_scan::{ScanOptions, start};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: bench <path> [threads] [--cross]");
        return;
    };
    let mut threads = 0usize;
    let mut one_fs = true;
    for arg in args {
        if arg == "--cross" {
            one_fs = false;
        } else if let Ok(n) = arg.parse() {
            threads = n;
        }
    }
    let mut options = ScanOptions::new(PathBuf::from(path));
    options.threads = threads;
    options.one_file_system = one_fs;
    let begin = Instant::now();
    let handle = start(options);
    handle.wait();
    let progress = handle.progress();
    let secs = begin.elapsed().as_secs_f64();
    let (nodes, heap) = handle.read_tree(|t| (t.len(), t.heap_bytes()));
    println!(
        "threads={threads} files={} dirs={} bytes={} errors={} nodes={nodes} time={secs:.3}s \
         files/s={:.0} tree_mem={:.1}MiB cancelled={}",
        progress.files,
        progress.dirs,
        progress.bytes,
        progress.errors,
        (progress.files + progress.dirs) as f64 / secs,
        heap as f64 / 1048576.0,
        progress.cancelled,
    );
    if let Some(tree) = handle.take_tree() {
        let t = Instant::now();
        let bytes = vano_core::persist::encode(&tree);
        let enc = t.elapsed();
        let t = Instant::now();
        let back = vano_core::persist::decode(&bytes);
        println!(
            "persist: {:.1}MiB encode={enc:?} decode={:?} ok={}",
            bytes.len() as f64 / 1048576.0,
            t.elapsed(),
            back.is_ok()
        );
    }
}
