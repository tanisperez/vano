//! What `vano scan` prints, as text for people or JSON for scripts. The JSON field names are a
//! stable interface: add fields, never rename or remove them.

use std::path::Path;

use serde::Serialize;
use vano_core::size::format_plain;
use vano_core::{NodeId, ROOT, Tree};
use vano_rules::{Recommendation, Status};

use crate::scan::Scanned;

/// One of the biggest entries of the scanned folder.
#[derive(Debug, Serialize)]
pub struct Entry {
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    pub dir: bool,
}

/// One recommendation, with the English name of its rule.
#[derive(Debug, Serialize)]
pub struct Rec {
    pub rule: String,
    pub name: String,
    /// `safe` or `review`.
    pub status: &'static str,
    pub bytes: u64,
    pub files: u64,
    /// Only shown for information: it says how to get the space back, it is never removed.
    pub info: bool,
    pub paths: Vec<String>,
}

impl Rec {
    pub fn new(tree: &Tree, rec: &Recommendation) -> Self {
        Rec {
            rule: rec.rule.to_string(),
            name: rule_name(rec.rule),
            status: status_name(rec.status),
            bytes: rec.size,
            files: rec.files,
            info: rec.info,
            paths: rec
                .nodes
                .iter()
                .map(|id| tree.path(*id).display().to_string())
                .collect(),
        }
    }
}

/// The result of `vano scan`.
#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub root: String,
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    /// Folders or entries that could not be read.
    pub unreadable: u64,
    pub seconds: f64,
    pub biggest: Vec<Entry>,
    pub recommendations: Vec<Rec>,
}

impl ScanReport {
    pub fn new(scanned: &Scanned, recommendations: &[Recommendation], top: usize) -> Self {
        let tree = &scanned.tree;
        let biggest = tree
            .sorted_children(ROOT)
            .into_iter()
            .take(top)
            .map(|id| entry(tree, id))
            .collect();
        ScanReport {
            root: tree.root_path().display().to_string(),
            bytes: tree.node(ROOT).size,
            files: scanned.progress.files,
            dirs: scanned.progress.dirs,
            unreadable: scanned.progress.errors,
            seconds: scanned.progress.elapsed.as_secs_f64(),
            biggest,
            recommendations: recommendations
                .iter()
                .map(|rec| Rec::new(tree, rec))
                .collect(),
        }
    }

    /// Pretty JSON, ending in a newline.
    pub fn json(&self) -> String {
        to_json(self)
    }

    /// The text for a terminal.
    pub fn text(&self) -> String {
        let mut out = format!(
            "{}: {} in {} files, scanned in {:.1} s\n",
            show_path(Path::new(&self.root)),
            format_plain(self.bytes),
            count(self.files),
            self.seconds,
        );
        if self.unreadable > 0 {
            out.push_str(&format!(
                "{} could not be read, not counted.\n",
                plural(self.unreadable, "entry", "entries")
            ));
        }
        if !self.biggest.is_empty() {
            out.push_str("\nBiggest\n");
            for entry in &self.biggest {
                let name = Path::new(&entry.path)
                    .file_name()
                    .map_or_else(|| entry.path.clone(), |n| n.to_string_lossy().into_owned());
                let slash = if entry.dir { "/" } else { "" };
                out.push_str(&format!(
                    "  {:>9}  {name}{slash}\n",
                    format_plain(entry.bytes)
                ));
            }
        }
        if self.recommendations.is_empty() {
            out.push_str("\nNo recommendations.\n");
        } else {
            out.push_str("\nRecommendations\n");
            out.push_str(&recommendation_lines(&self.recommendations));
        }
        out
    }
}

/// One line per recommendation: status, size, name and where.
pub fn recommendation_lines(recs: &[Rec]) -> String {
    let width = recs
        .iter()
        .map(|rec| rec.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for rec in recs {
        let status = if rec.status == "safe" {
            "Safe"
        } else {
            "Review"
        };
        let place = match rec.paths.as_slice() {
            [one] => show_path(Path::new(one)),
            many => plural(many.len() as u64, "place", "places"),
        };
        out.push_str(&format!(
            "  {status:<6}  {:>9}  {:<width$}  {place}\n",
            format_plain(rec.bytes),
            rec.name,
        ));
    }
    out
}

fn entry(tree: &Tree, id: NodeId) -> Entry {
    let node = tree.node(id);
    Entry {
        path: tree.path(id).display().to_string(),
        bytes: node.size,
        files: u64::from(node.files),
        dir: node.is_dir(),
    }
}

/// The English name of a rule, from the rules' own texts.
pub fn rule_name(rule: &str) -> String {
    let key = format!("rules.{rule}.name");
    let name = rust_i18n::t!(&key, locale = "en").into_owned();
    if name == key { rule.to_string() } else { name }
}

pub fn status_name(status: Status) -> &'static str {
    match status {
        Status::Safe => "safe",
        Status::Review => "review",
    }
}

/// Pretty JSON of any report, ending in a newline.
pub fn to_json(value: &impl Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string());
    text.push('\n');
    text
}

/// A path with the home folder shown as `~`.
pub fn show_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".to_string();
        }
        return Path::new("~").join(rest).display().to_string();
    }
    path.display().to_string()
}

/// `12,345`.
pub fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `1 item`, `3 items`.
pub fn plural(value: u64, one: &str, many: &str) -> String {
    if value == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", count(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1000), "1,000");
        assert_eq!(count(1_234_567), "1,234,567");
    }

    #[test]
    fn plurals_say_one_or_many() {
        assert_eq!(plural(1, "file", "files"), "1 file");
        assert_eq!(plural(2000, "file", "files"), "2,000 files");
    }

    #[test]
    fn rules_get_their_english_name() {
        assert_eq!(rule_name("npm"), "npm cache");
        assert_eq!(rule_name("no_such_rule"), "no_such_rule");
    }

    #[test]
    fn recommendation_lines_align_and_name_the_place() {
        let recs = vec![
            Rec {
                rule: "npm".into(),
                name: "npm cache".into(),
                status: "safe",
                bytes: 2_000_000_000,
                files: 10,
                info: false,
                paths: vec!["/srv/npm".into()],
            },
            Rec {
                rule: "x".into(),
                name: "Old installers".into(),
                status: "review",
                bytes: 500_000,
                files: 2,
                info: false,
                paths: vec!["/a".into(), "/b".into()],
            },
        ];
        assert_eq!(
            recommendation_lines(&recs),
            "  Safe       2.0 GB  npm cache       /srv/npm\n  Review     500 KB  Old installers  2 places\n"
        );
    }
}
