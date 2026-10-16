#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use vano_core::Category;
use vano_rules::{LiveRules, Os, Status, categorize, category_of, evaluate, rule_ids};

const OLD: u32 = NOW - 200 * DAY;
const NEW: u32 = NOW - 2 * DAY;

#[test]
fn npm_cache_is_safe() {
    let t = tree(
        "/home/u",
        &[
            (".npm/_cacache/a", 100, NEW),
            (".npm/_logs/b", 20, NEW),
            (".config/x", 5, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    assert_eq!(r.rule, "npm");
    assert_eq!(r.status, Status::Safe);
    assert_eq!((r.size, r.files, r.newest_mtime), (120, 2, NEW));
    assert_eq!(r.display_path, "~/.npm");
    assert!(!r.approx);
}

#[test]
fn cargo_caches_but_never_bin() {
    let t = tree(
        "/home/u",
        &[
            (".cargo/bin/cargo", 50, NEW),
            (".cargo/registry/cache/x.crate", 10, NEW),
            (".cargo/registry/src/y/lib.rs", 20, NEW),
            (".cargo/registry/index/z", 5, NEW),
            (".cargo/git/checkouts/c/f", 30, NEW),
            (".cargo/git/db/d", 40, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    let r = rec(&recs, "cargo_cache").unwrap();
    assert_eq!(r.size, 60);
    assert_eq!(r.nodes.len(), 3);
    assert!(paths(&t, r).iter().all(|p| !p.contains("bin")));
    let cats = categorize(&t, &linux());
    let bin = t
        .find(std::path::Path::new("/home/u/.cargo/bin/cargo"))
        .unwrap();
    assert_ne!(cats[bin as usize], Category::Caches);
    let idx = t
        .find(std::path::Path::new("/home/u/.cargo/registry/index"))
        .unwrap();
    assert_eq!(cats[idx as usize], Category::Caches);
}

#[test]
fn rust_target_needs_proof() {
    let t = tree(
        "/home/u",
        &[
            ("p/with/Cargo.toml", 1, NEW),
            ("p/with/target/debug/x", 500, NEW),
            ("p/without/target/x", 300, NEW),
            ("p/tagged/target/CACHEDIR.TAG", 1, NEW),
            ("p/tagged/target/x", 200, NEW),
            ("p/maven/pom.xml", 1, NEW),
            ("p/maven/target/classes/x", 100, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    let r = rec(&recs, "rust_target").unwrap();
    assert_eq!(
        paths(&t, r),
        vec!["/home/u/p/tagged/target", "/home/u/p/with/target"]
    );
    assert_eq!(rec(&recs, "maven_target").unwrap().size, 100);
}

#[test]
fn node_modules_only_when_idle() {
    let t = tree(
        "/home/u",
        &[
            ("old/package.json", 1, OLD),
            ("old/src/a.js", 1, OLD),
            ("old/node_modules/x/i.js", 900, NEW),
            ("busy/package.json", 1, NEW),
            ("busy/node_modules/x/i.js", 800, NEW),
            ("lonely/node_modules/x/i.js", 700, OLD),
        ],
    );
    let recs = evaluate(&t, &linux());
    let r = rec(&recs, "node_modules").unwrap();
    assert_eq!(paths(&t, r), vec!["/home/u/old/node_modules"]);
    assert_eq!(r.status, Status::Review);
    // Active and unprovable ones are still colored as caches.
    let cats = categorize(&t, &linux());
    let busy = t
        .find(std::path::Path::new("/home/u/busy/node_modules/x"))
        .unwrap();
    assert_eq!(cats[busy as usize], Category::Caches);
}

#[test]
fn js_build_needs_package_json() {
    let t = tree(
        "/home/u",
        &[
            ("a/package.json", 1, NEW),
            ("a/.next/x", 60, NEW),
            ("a/.turbo/y", 40, NEW),
            ("b/.next/x", 50, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    let r = rec(&recs, "js_build").unwrap();
    assert_eq!(r.size, 100);
    assert_eq!(r.nodes.len(), 2);
}

#[test]
fn gradle_dart_python_pods() {
    let t = tree(
        "/home/u",
        &[
            ("g/build.gradle", 1, NEW),
            ("g/build/o", 10, NEW),
            ("g/.gradle/c", 10, NEW),
            ("n/build/o", 10, NEW),
            ("d/pubspec.yaml", 1, NEW),
            ("d/.dart_tool/o", 10, NEW),
            ("d/build/o", 10, NEW),
            ("py/pyproject.toml", 1, NEW),
            ("py/.venv/lib", 10, NEW),
            ("py/__pycache__/a.pyc", 3, NEW),
            ("py2/venv/lib", 10, NEW),
            ("ios/Podfile", 1, NEW),
            ("ios/Pods/x", 10, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(rec(&recs, "gradle_build").unwrap().size, 20);
    assert_eq!(rec(&recs, "dart_build").unwrap().size, 20);
    assert_eq!(rec(&recs, "python_venv").unwrap().nodes.len(), 1);
    assert_eq!(rec(&recs, "python_bytecode").unwrap().size, 3);
    assert_eq!(rec(&recs, "cocoapods").unwrap().status, Status::Review);
}

#[test]
fn worktrees_win_over_what_is_inside() {
    let t = tree(
        "/home/u",
        &[
            ("atlas/.claude/worktrees/feat/Cargo.toml", 1, NEW),
            ("atlas/.claude/worktrees/feat/target/x", 500, NEW),
            ("atlas/.claude/worktrees/feat/node_modules/x", 400, NEW),
            ("atlas/.claude/worktrees/feat/src/a.rs", 100, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(rule_names(&recs), vec!["worktrees"]);
    let r = &recs[0];
    assert!(r.approx);
    assert_eq!(r.size, 1001);
    assert_eq!(r.nodes.len(), 1);
}

#[test]
fn ai_and_sdk_review() {
    let t = tree(
        "/home/u",
        &[
            (".ollama/models/blobs/b", 1000, OLD),
            (".cache/huggingface/hub/m", 800, OLD),
            (".lmstudio/models/m", 700, OLD),
            ("Android/Sdk/platforms/p", 600, OLD),
            (".android/avd/a/x", 500, OLD),
            (".android/adbkey", 1, OLD),
            (".local/share/docker/v", 400, OLD),
            (".local/share/Trash/files/f", 300, OLD),
            (".m2/repository/x", 200, OLD),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(
        rule_names(&recs),
        vec![
            "android_sdk",
            "docker",
            "huggingface",
            "lm_studio",
            "maven_repo",
            "ollama",
            "trash"
        ]
    );
    assert!(recs.iter().all(|r| r.status == Status::Review));
    assert_eq!(rec(&recs, "android_sdk").unwrap().nodes.len(), 2);
    let cats = categorize(&t, &linux());
    let at = |p: &str| cats[t.find(std::path::Path::new(p)).unwrap() as usize];
    assert_eq!(at("/home/u/.ollama/models/blobs/b"), Category::Ai);
    assert_eq!(at("/home/u/.android/adbkey"), Category::Sdk);
    assert_eq!(at("/home/u/.cache/huggingface/hub"), Category::Ai);
}

#[test]
fn downloads_and_desktop_files() {
    let t = tree(
        "/home/u",
        &[
            ("Downloads/old.deb", 10, OLD),
            ("Downloads/old.AppImage", 20, OLD),
            ("Downloads/new.deb", 30, NEW),
            ("Downloads/old.pdf", 40, OLD),
            ("Downloads/sub/old.iso", 50, OLD),
            ("Desktop/rec.mov", 150 * MB, NEW),
            ("Desktop/small.mp4", 5 * MB, NEW),
            ("Desktop/big.txt", 500 * MB, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    let inst = rec(&recs, "installers").unwrap();
    assert_eq!(inst.size, 30);
    assert_eq!(inst.nodes.len(), 2);
    assert_eq!(inst.display_path, "~/Downloads/*.appimage, *.deb");
    let rc = rec(&recs, "recordings").unwrap();
    assert_eq!(paths(&t, rc), vec!["/home/u/Desktop/rec.mov"]);
    assert_eq!(rc.display_path, "~/Desktop/*.mov");
}

#[test]
fn display_paths_summarize() {
    let t = tree(
        "/home/u",
        &[
            ("Desktop/orbit/package.json", 1, OLD),
            ("Desktop/orbit/node_modules/x", 30, NEW),
            ("Desktop/sandbox/package.json", 1, OLD),
            ("Desktop/sandbox/node_modules/x", 20, NEW),
            ("Desktop/side/package.json", 1, OLD),
            ("Desktop/side/node_modules/x", 10, NEW),
            ("Desktop/side/.next/x", 5, NEW),
            (".cache/uv/x", 9, NEW),
            (".cache/pip/x", 8, NEW),
            (".cache/pypoetry/x", 7, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(
        rec(&recs, "node_modules").unwrap().display_path,
        "~/Desktop/{orbit,sandbox,side}"
    );
    assert_eq!(
        rec(&recs, "python_caches").unwrap().display_path,
        "~/.cache/{uv,pip,pypoetry}"
    );
}

#[test]
fn system_rules_need_the_whole_disk() {
    let t = tree(
        "/",
        &[
            ("var/cache/pacman/pkg/a.zst", 100, NEW),
            ("var/log/journal/j", 50, NEW),
            ("var/lib/docker/overlay/x", 400, NEW),
            ("usr/lib/python3/__pycache__/a.pyc", 7, NEW),
            ("usr/share/ollama/.ollama/models/m", 300, NEW),
            ("home/u/.npm/x", 9, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(
        rule_names(&recs),
        vec![
            "docker",
            "journal_logs",
            "npm",
            "ollama",
            "package_manager_cache"
        ]
    );
    // Scanning only the home never sees them.
    let t2 = tree("/home/u", &[(".npm/x", 9, NEW)]);
    assert_eq!(evaluate(&t2, &linux()).len(), 1);
}

#[test]
fn os_gating() {
    let t = tree(
        "/home/u",
        &[
            ("Library/Developer/Xcode/DerivedData/x", 100, NEW),
            ("Library/Developer/CoreSimulator/Devices/d", 50, NEW),
            ("Library/Caches/Homebrew/x", 20, NEW),
            ("Library/Caches/Other/x", 20, NEW),
        ],
    );
    let mac = evaluate(&t, &ctx(Os::MacOs));
    assert_eq!(
        rule_names(&mac),
        vec!["ios_simulators", "mac_caches", "xcode_derived"]
    );
    assert!(evaluate(&t, &linux()).is_empty());
    let w = tree("C:\\Users\\u", &[("AppData/Local/npm-cache/x", 9, NEW)]);
    assert_eq!(rule_names(&evaluate(&w, &ctx(Os::Windows))), vec!["npm"]);
    assert!(evaluate(&w, &linux()).is_empty());
}

#[test]
fn browser_caches_use_wildcards() {
    let t = tree(
        "/home/u",
        &[
            (".cache/google-chrome/Default/Cache/a", 10, NEW),
            (".cache/google-chrome/Profile 1/GPUCache/b", 5, NEW),
            (".cache/google-chrome/Default/History", 99, NEW),
            (".cache/mozilla/firefox/p/cache2/c", 7, NEW),
            (".var/app/org.x.App/cache/z", 3, NEW),
            (".var/app/org.x.App/data/z", 30, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(rec(&recs, "browser_cache").unwrap().size, 22);
    assert_eq!(rec(&recs, "flatpak_cache").unwrap().size, 3);
}

#[test]
fn removed_and_empty_nodes_are_ignored() {
    let mut t = tree(
        "/home/u",
        &[
            (".npm/x", 10, NEW),
            (".cache/pip/y", 0, NEW),
            (".cache/uv/z", 5, NEW),
        ],
    );
    let npm = t.find(std::path::Path::new("/home/u/.npm")).unwrap();
    t.mark_removed(npm);
    let recs = evaluate(&t, &linux());
    assert_eq!(rule_names(&recs), vec!["python_caches"]);
    assert_eq!(recs[0].size, 5);
}

#[test]
fn sorted_safe_first_then_by_size() {
    let t = tree(
        "/home/u",
        &[
            (".ollama/models/m", 1000, OLD),
            (".npm/x", 10, NEW),
            (".cache/pip/x", 20, NEW),
        ],
    );
    let recs = evaluate(&t, &linux());
    assert_eq!(
        recs.iter().map(|r| r.rule).collect::<Vec<_>>(),
        vec!["python_caches", "npm", "ollama"]
    );
}

fn mixed() -> vano_core::Tree {
    tree(
        "/home/u",
        &[
            (".npm/_cacache/a", 100, NEW),
            (".cache/pip/a", 40, NEW),
            (".cache/huggingface/m", 400, OLD),
            (".gradle/caches/c", 70, NEW),
            ("Downloads/old.deb", 10, OLD),
            ("Downloads/new.deb", 10, NEW),
            ("Desktop/rec.mkv", 120 * MB, NEW),
            ("Desktop/notes.txt", 1, NEW),
            ("proj/Cargo.toml", 1, OLD),
            ("proj/src/main.rs", 5, OLD),
            ("proj/target/debug/x", 600, OLD),
            ("proj/node_modules/a/b", 90, OLD),
            ("proj/.git/HEAD", 1, OLD),
            ("proj2/package.json", 1, NEW),
            ("proj2/node_modules/a/b", 90, NEW),
            ("proj2/.next/c", 30, NEW),
            ("a/.claude/worktrees/w/target/x", 33, NEW),
            ("Documents/a.pdf", 12, OLD),
            ("Pictures/a.jpg", 12, OLD),
            ("misc/x.png", 12, OLD),
            ("misc/y.bin", 12, OLD),
        ],
    )
}

#[test]
fn live_equals_evaluate_at_the_end() {
    let t = mixed();
    let ctx = linux();
    let mut live = LiveRules::new(ctx.clone());
    // One folder at a time, children before parents, then everything again in one call.
    for d in post_order(&t) {
        live.on_completed(&t, &[d]);
    }
    let mut a = live.found();
    let mut b = evaluate(&t, &ctx);
    a.sort_by_key(|r| r.rule);
    b.sort_by_key(|r| r.rule);
    assert_eq!(a, b);
    assert!(!a.is_empty());

    let mut again = LiveRules::new(ctx.clone());
    again.on_completed(&t, &post_order(&t));
    let mut c = again.found();
    c.sort_by_key(|r| r.rule);
    assert_eq!(c, b);
}

#[test]
fn live_reports_as_folders_finish() {
    let t = mixed();
    let mut live = LiveRules::new(linux());
    let npm = t.find(std::path::Path::new("/home/u/.npm")).unwrap();
    live.on_completed(&t, &[npm]);
    assert_eq!(live.found().len(), 1);
    assert_eq!(live.found()[0].rule, "npm");
    // A folder inside a match reports nothing new.
    let inner = t
        .find(std::path::Path::new("/home/u/.npm/_cacache"))
        .unwrap();
    live.on_completed(&t, &[inner, npm]);
    assert_eq!(live.found().len(), 1);
}

#[test]
fn categories() {
    let t = mixed();
    let ctx = linux();
    let cats = categorize(&t, &ctx);
    assert_eq!(cats.len(), t.len());
    let at = |p: &str| cats[t.find(std::path::Path::new(p)).unwrap() as usize];
    assert_eq!(at("/home/u"), Category::Other);
    assert_eq!(at("/home/u/proj"), Category::Code);
    assert_eq!(at("/home/u/proj/src/main.rs"), Category::Code);
    assert_eq!(at("/home/u/proj/target/debug/x"), Category::Caches);
    assert_eq!(at("/home/u/proj/node_modules/a/b"), Category::Caches);
    assert_eq!(at("/home/u/proj2/node_modules"), Category::Caches);
    assert_eq!(at("/home/u/Documents/a.pdf"), Category::Media);
    assert_eq!(at("/home/u/Desktop/rec.mkv"), Category::Media);
    assert_eq!(at("/home/u/Desktop/notes.txt"), Category::Other);
    assert_eq!(at("/home/u/misc/x.png"), Category::Media);
    assert_eq!(at("/home/u/misc/y.bin"), Category::Other);
    assert_eq!(at("/home/u/.cache/huggingface/m"), Category::Ai);
    assert_eq!(at("/home/u/.gradle/caches/c"), Category::Caches);
    assert_eq!(at("/home/u/Downloads/old.deb"), Category::Other);
}

#[test]
fn home_with_a_dotfiles_repo_is_not_code() {
    let t = tree("/home/u", &[(".git/HEAD", 1, NEW), ("x/y", 1, NEW)]);
    let cats = categorize(&t, &linux());
    assert!(cats.iter().all(|c| *c == Category::Other));
}

#[test]
fn system_folders_are_other() {
    let t = tree(
        "/",
        &[
            ("usr/src/linux/Makefile", 1, NEW),
            ("usr/src/linux/src/a.c", 1, NEW),
            ("opt/tool/package.json", 1, NEW),
            ("home/u/p/package.json", 1, NEW),
        ],
    );
    let cats = categorize(&t, &linux());
    let at = |p: &str| cats[t.find(std::path::Path::new(p)).unwrap() as usize];
    assert_eq!(at("/usr/src/linux/src/a.c"), Category::Other);
    assert_eq!(at("/opt/tool"), Category::Other);
    assert_eq!(at("/home/u/p/package.json"), Category::Code);
}

#[test]
fn scanning_inside_a_known_folder() {
    let t = tree("/home/u/.cache", &[("pip/x", 5, NEW), ("other/y", 3, NEW)]);
    let recs = evaluate(&t, &linux());
    assert_eq!(rule_names(&recs), vec!["python_caches"]);
    let cats = categorize(&t, &linux());
    assert!(cats.iter().all(|c| *c == Category::Caches));
    // Scanning inside a match recommends nothing.
    let inside = tree("/home/u/.npm", &[("_cacache/x", 5, NEW)]);
    assert!(evaluate(&inside, &linux()).is_empty());
}

#[test]
fn category_of_agrees_with_categorize() {
    for t in [
        mixed(),
        tree("/", &[("usr/a/b", 1, NEW), ("home/u/.npm/x", 2, NEW)]),
    ] {
        let ctx = linux();
        let cats = categorize(&t, &ctx);
        for id in 0..t.len() as u32 {
            assert_eq!(
                category_of(&t, &ctx, id),
                cats[id as usize],
                "{}",
                t.path(id).display()
            );
        }
    }
}

#[test]
fn rule_ids_are_unique() {
    let mut ids = rule_ids().to_vec();
    ids.sort_unstable();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert!(n > 30);
}
