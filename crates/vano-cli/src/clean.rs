//! `vano clean [PATH]`: scans a folder and removes its Safe recommendations, the caches and build
//! outputs their tools recreate. Review recommendations are never removed from the terminal:
//! they need someone to look at them, in the window.
//!
//! It lists what it would remove first. Then it asks on a terminal, does nothing more with
//! `--dry-run` or without a terminal to ask, and removes without asking with `--yes`. Items go to
//! the Trash unless `--permanent`.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use vano_core::size::format_plain;
use vano_delete::trash::{self, DeleteMode, Outcome, TrashError, TrashProgress};
use vano_rules::Status;

use crate::args::CleanArgs;
use crate::exit;
use crate::report::{Rec, plural, recommendation_lines, show_path, to_json};
use crate::scan;

/// What `vano clean` would do next, once it has listed the items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    Remove,
    Ask,
    /// List only: `--dry-run`, or nobody at a terminal to ask.
    Stop,
}

fn next(args: &CleanArgs, interactive: bool) -> Next {
    if args.yes {
        Next::Remove
    } else if args.dry_run || !interactive || args.json {
        Next::Stop
    } else {
        Next::Ask
    }
}

/// The outcome in the JSON output.
#[derive(Debug, Serialize)]
struct CleanJson {
    root: String,
    /// `trash` or `permanent`.
    mode: &'static str,
    /// The items were removed (`--yes`), or only listed.
    removed: bool,
    bytes: u64,
    recommendations: Vec<Rec>,
    /// Items left in place, with the reason.
    failed: Vec<Failure>,
}

#[derive(Debug, Serialize)]
struct Failure {
    path: String,
    reason: String,
}

/// `vano clean`.
pub fn command(args: &CleanArgs) -> u8 {
    let root = match scan::target(args.path.as_deref()) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("vano clean: {error}");
            return exit::FAILED;
        }
    };
    let live = if args.json {
        scan::Live::Quiet
    } else {
        scan::Live::Terminal
    };
    let scanned = scan::run(&root, true, live);
    let rules = vano_rules::Context::current();
    let recs: Vec<Rec> = vano_rules::evaluate(&scanned.tree, &rules)
        .iter()
        .filter(|rec| rec.status == Status::Safe && !rec.info)
        .map(|rec| Rec::new(&scanned.tree, rec))
        .collect();
    let mode = if args.permanent {
        DeleteMode::Permanent
    } else {
        DeleteMode::Trash
    };
    let paths: Vec<PathBuf> = recs
        .iter()
        .flat_map(|rec| rec.paths.iter().map(PathBuf::from))
        .collect();
    let bytes: u64 = recs.iter().map(|rec| rec.bytes).sum();
    let files: u64 = recs.iter().map(|rec| rec.files).sum();
    let mut out = std::io::stdout();

    let next = next(args, std::io::stdin().is_terminal());
    let mut report = CleanJson {
        root: root.display().to_string(),
        mode: mode_name(mode),
        removed: false,
        bytes,
        recommendations: recs,
        failed: Vec::new(),
    };
    if report.recommendations.is_empty() {
        if args.json {
            let _ = out.write_all(to_json(&report).as_bytes());
        } else {
            let _ = writeln!(out, "Nothing to clean in {}.", show_path(&root));
        }
        return exit::NOTHING_TO_CLEAN;
    }
    if !args.json {
        let _ = writeln!(
            out,
            "{} to clean in {}:\n{}",
            format_plain(bytes),
            show_path(&root),
            recommendation_lines(&report.recommendations)
        );
    }
    match next {
        Next::Stop => {
            if args.json {
                let _ = out.write_all(to_json(&report).as_bytes());
            } else {
                let _ = writeln!(
                    out,
                    "Nothing was removed. Run vano clean --yes to {}.",
                    action(mode)
                );
            }
            return exit::SUCCESS;
        }
        Next::Ask => {
            let _ = write!(
                out,
                "{} ({})? [y/N] ",
                capitalize(&action(mode)),
                plural(paths.len() as u64, "item", "items")
            );
            let _ = out.flush();
            let mut answer = String::new();
            let _ = std::io::stdin().lock().read_line(&mut answer);
            if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
                let _ = writeln!(out, "Nothing was removed.");
                return exit::SUCCESS;
            }
        }
        Next::Remove => {}
    }

    let outcome = remove(&paths, mode, files, bytes);
    report.removed = true;
    report.failed = outcome
        .failed
        .iter()
        .map(|(index, error)| Failure {
            path: paths[*index].display().to_string(),
            reason: reason(error),
        })
        .collect();
    if args.json {
        let _ = out.write_all(to_json(&report).as_bytes());
    } else {
        let removed: u64 = outcome.moved.len() as u64;
        let _ = writeln!(out, "{}", done_line(mode, removed));
        for failure in &report.failed {
            eprintln!("Not removed: {}: {}", failure.path, failure.reason);
        }
    }
    if report.failed.is_empty() {
        exit::SUCCESS
    } else {
        exit::FAILED
    }
}

fn remove(paths: &[PathBuf], mode: DeleteMode, files: u64, bytes: u64) -> Outcome {
    match mode {
        DeleteMode::Trash => trash::move_each(paths, &AtomicBool::new(false), |_| {}),
        DeleteMode::Permanent => {
            let progress = TrashProgress::new(mode, paths.len(), files, bytes);
            vano_delete::delete::delete_each(paths, &progress)
        }
    }
}

/// The Trash is the Recycle Bin on Windows.
fn trash_name() -> &'static str {
    if cfg!(windows) {
        "the Recycle Bin"
    } else {
        "the Trash"
    }
}

fn mode_name(mode: DeleteMode) -> &'static str {
    match mode {
        DeleteMode::Trash => "trash",
        DeleteMode::Permanent => "permanent",
    }
}

/// `move them to the Trash`, `delete them for good`.
fn action(mode: DeleteMode) -> String {
    match mode {
        DeleteMode::Trash => format!("move them to {}", trash_name()),
        DeleteMode::Permanent => "delete them for good".to_string(),
    }
}

fn done_line(mode: DeleteMode, removed: u64) -> String {
    let items = plural(removed, "item", "items");
    match mode {
        DeleteMode::Trash => format!("Moved {items} to {}.", trash_name()),
        DeleteMode::Permanent => format!("Deleted {items} for good."),
    }
}

fn reason(error: &TrashError) -> String {
    match error {
        TrashError::Refused => format!("{} refused it", trash_name()),
        TrashError::OtherFileSystem => {
            "it holds another file system, which was left in place".to_string()
        }
        TrashError::Os(text) => text.clone(),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(dry_run: bool, yes: bool, json: bool) -> CleanArgs {
        CleanArgs {
            path: None,
            dry_run,
            permanent: false,
            yes,
            json,
        }
    }

    #[test]
    fn it_asks_on_a_terminal_and_only_lists_otherwise() {
        assert_eq!(next(&args(false, false, false), true), Next::Ask);
        assert_eq!(next(&args(false, false, false), false), Next::Stop);
        assert_eq!(next(&args(true, false, false), true), Next::Stop);
        assert_eq!(next(&args(false, false, true), true), Next::Stop);
    }

    #[test]
    fn yes_removes_without_asking() {
        assert_eq!(next(&args(false, true, false), false), Next::Remove);
        assert_eq!(next(&args(false, true, true), true), Next::Remove);
    }

    #[test]
    fn the_question_starts_with_a_capital() {
        assert_eq!(capitalize("delete them for good"), "Delete them for good");
    }

    #[test]
    fn the_done_line_says_where_the_items_went() {
        assert_eq!(
            done_line(DeleteMode::Permanent, 1),
            "Deleted 1 item for good."
        );
        assert!(done_line(DeleteMode::Trash, 3).starts_with("Moved 3 items to the "));
    }
}
