//! Captures the short git commit of the build into `VANO_COMMIT`, which `vano-cli` reads
//! with `option_env!`. An explicit `VANO_COMMIT` in the environment wins (CI, packagers); with
//! neither it, git nor a repository (a source tarball) the variable is left unset and the build
//! simply has no commit.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=VANO_COMMIT");
    if let Some(commit) = explicit_commit().or_else(git_commit) {
        println!("cargo:rustc-env=VANO_COMMIT={commit}");
    }
    watch_git_head();
}

/// `VANO_COMMIT` from the environment, when set and not empty.
fn explicit_commit() -> Option<String> {
    let value = std::env::var("VANO_COMMIT").ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// `git rev-parse --short HEAD`, or `None` without git or outside a repository.
fn git_commit() -> Option<String> {
    git(&["rev-parse", "--short", "HEAD"])
}

/// Runs git in this crate's directory and returns its trimmed stdout, `None` on any failure.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Reruns the script when the commit changes: HEAD itself (a checkout), the ref it points at (a
/// commit) and `packed-refs` (a pack). The paths come from git, so they are right in a worktree.
/// A path that does not exist is skipped: cargo would treat it as changed on every build.
fn watch_git_head() {
    let mut paths = Vec::new();
    paths.extend(git(&["rev-parse", "--git-path", "HEAD"]));
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        paths.extend(git(&["rev-parse", "--git-path", &reference]));
    }
    paths.extend(git(&["rev-parse", "--git-path", "packed-refs"]));
    for path in paths.iter().filter(|path| Path::new(path).exists()) {
        println!("cargo:rerun-if-changed={path}");
    }
}
