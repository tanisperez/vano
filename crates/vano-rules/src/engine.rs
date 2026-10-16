//! The matching engine: classifies one node given the state of its parent.
//!
//! The state flows top down. A folder that is a match (or a build output, or an SDK) is
//! "locked": nothing inside it is examined again, which makes the outermost match win and keeps
//! recommendations from overlapping. Both [`crate::evaluate`] and [`crate::LiveRules`] use
//! [`Engine::child`], so they cannot disagree.

use std::collections::HashSet;
use std::path::{Component, Path};

use vano_core::tree::{FLAG_REMOVED, NO_NODE, Node, NodeId, ROOT};
use vano_core::{Category, Kind, Tree};

use crate::defs::{
    Base, CHROMIUM_CACHES, Entry, NAME_RULE_NAMES, R, RULES, TABLE, chromium_bases, os_bit,
};
use crate::trie::{NONE, Trie};
use crate::{Context, Os};

/// Nothing inside is examined again.
pub(crate) const LOCKED: u8 = 1;
/// Name based rules and project detection are off.
pub(crate) const NO_NAMES: u8 = 2;

const DAY: u32 = 86_400;
/// A project whose files did not change for this long is idle (`node_modules` becomes a review).
const IDLE_DAYS: u32 = 60;
/// Installers older than this are recommended.
const OLD_INSTALLER_DAYS: u32 = 30;
/// Videos at least this big (allocated bytes) are recommended.
const BIG_VIDEO: u64 = 100 * 1024 * 1024;

/// What the scan carries from a folder to its children.
#[derive(Debug, Clone, Copy)]
pub(crate) struct State {
    pub trie: u32,
    pub cat: Category,
    pub flags: u8,
}

impl State {
    pub const START: State = State {
        trie: NONE,
        cat: Category::Other,
        flags: 0,
    };

    fn locked(cat: Category) -> State {
        State {
            trie: NONE,
            cat,
            flags: LOCKED,
        }
    }
}

enum NameMatch {
    None,
    /// A build output or dependency folder: colored as a cache, nothing recommended.
    Artifact,
    Hit(R),
}

pub(crate) struct Engine {
    trie: Trie,
    now: u32,
    /// Lowercase names a completed folder can have if it is itself a match.
    interesting: HashSet<Vec<u8>>,
    /// Lowercase names of folders whose files can match.
    file_parents: HashSet<Vec<u8>>,
}

fn path_comps(path: &Path) -> Vec<Vec<u8>> {
    path.components()
        .filter(|c| !matches!(c, Component::RootDir | Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().into_owned().into_bytes())
        .collect()
}

fn split(path: &str) -> Vec<Vec<u8>> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.as_bytes().to_vec())
        .collect()
}

impl Engine {
    pub fn new(ctx: &Context) -> Self {
        let mut trie = Trie::new(ctx.os != Os::Linux);
        let mut interesting = HashSet::new();
        let mut file_parents = HashSet::new();
        let home = ctx.home.as_deref().map(path_comps);
        let bit = os_bit(ctx.os);
        let lower = |c: &[u8]| c.to_ascii_lowercase();

        let mut add = |base: Base, path: &str, entry: Entry| {
            let mut comps = match base {
                Base::Abs => Vec::new(),
                Base::Home => match &home {
                    Some(h) => h.clone(),
                    None => return,
                },
            };
            comps.extend(split(path));
            if let Some(last) = comps.last() {
                match entry {
                    Entry::Hit(_) => {
                        interesting.insert(lower(last));
                    }
                    Entry::Files(_) => {
                        file_parents.insert(lower(last));
                    }
                    _ => {}
                }
            }
            trie.insert(&comps, |spec| match entry {
                Entry::Hit(r) => spec.rule = Some(r),
                Entry::Cat(c) => spec.cat = Some(c),
                Entry::CatNo(c) => {
                    spec.cat = Some(c);
                    spec.no_names = true;
                }
                Entry::NoNames => spec.no_names = true,
                Entry::Home => spec.not_project = true,
                Entry::Files(r) => spec.files = Some(r),
            });
        };
        for (mask, base, path, entry) in TABLE {
            if mask & bit != 0 {
                add(*base, path, *entry);
            }
        }
        for base in chromium_bases(ctx.os) {
            for cache in CHROMIUM_CACHES {
                add(
                    Base::Home,
                    &format!("{base}/*/{cache}"),
                    Entry::Hit(R::BrowserCache),
                );
            }
        }
        if ctx.os == Os::Windows {
            add(
                Base::Home,
                "AppData/Local/Mozilla/Firefox/Profiles/*/cache2",
                Entry::Hit(R::BrowserCache),
            );
        }
        for name in NAME_RULE_NAMES {
            interesting.insert(name.as_bytes().to_vec());
        }
        Self {
            trie,
            now: ctx.now,
            interesting,
            file_parents,
        }
    }

    /// The state of the tree's root: where it sits in the known locations.
    pub fn root_state(&self, tree: &Tree, deep: bool) -> State {
        let mut st = State {
            trie: 0,
            ..State::START
        };
        let comps = path_comps(tree.root_path());
        let mut not_project = false;
        for comp in &comps {
            let tn = self.trie.child(st.trie, comp);
            if tn == NONE {
                st.trie = NONE;
                not_project = false;
                break;
            }
            let spec = self.trie.spec(tn);
            if let Some(rule) = spec.rule {
                // The scan starts at or inside a match: all of it belongs to that rule.
                return State::locked(RULES[rule as usize].cat);
            }
            st.trie = tn;
            if let Some(c) = spec.cat {
                st.cat = c;
            }
            if spec.no_names {
                st.flags |= NO_NAMES;
            }
            not_project = spec.not_project;
        }
        if deep && self.may_be_project(&st, not_project) && is_project(tree, ROOT) {
            st.cat = Category::Code;
        }
        st
    }

    fn may_be_project(&self, st: &State, not_project: bool) -> bool {
        !not_project
            && st.flags & (LOCKED | NO_NAMES) == 0
            && matches!(st.cat, Category::Other | Category::Media | Category::Code)
    }

    /// The state of `id` and the rule it matches, given the state `ps` of its parent.
    /// `deep` adds what only the colors need: code projects and media files.
    pub fn child(
        &self,
        tree: &Tree,
        parent: NodeId,
        ps: State,
        id: NodeId,
        deep: bool,
    ) -> (State, Option<R>) {
        if ps.flags & LOCKED != 0 {
            return (ps, None);
        }
        let node = tree.node(id);
        if node.has(FLAG_REMOVED) {
            return (
                State {
                    trie: NONE,
                    flags: ps.flags | LOCKED,
                    ..ps
                },
                None,
            );
        }
        let name = tree.name_bytes(id);
        if node.kind != Kind::Dir {
            return self.file(ps, name, node, deep);
        }
        let mut st = State { trie: NONE, ..ps };
        let mut not_project = false;
        if ps.trie != NONE {
            let tn = self.trie.child(ps.trie, name);
            if tn != NONE {
                let spec = self.trie.spec(tn);
                if let Some(rule) = spec.rule {
                    return (State::locked(RULES[rule as usize].cat), Some(rule));
                }
                st.trie = tn;
                if let Some(c) = spec.cat {
                    st.cat = c;
                }
                if spec.no_names {
                    st.flags |= NO_NAMES;
                }
                not_project = spec.not_project;
            }
        }
        if st.flags & NO_NAMES != 0 {
            return (st, None);
        }
        match self.name_match(tree, parent, id, name) {
            NameMatch::Hit(rule) => {
                return (State::locked(RULES[rule as usize].cat), Some(rule));
            }
            NameMatch::Artifact => return (State::locked(Category::Caches), None),
            NameMatch::None => {}
        }
        if deep && self.may_be_project(&st, not_project) && is_project(tree, id) {
            st.cat = Category::Code;
        }
        (st, None)
    }

    fn file(&self, ps: State, name: &[u8], node: &Node, deep: bool) -> (State, Option<R>) {
        if ps.trie != NONE
            && node.kind == Kind::File
            && let Some(rule) = self.trie.spec(ps.trie).files
            && self.file_matches(rule, name, node)
        {
            return (State::locked(RULES[rule as usize].cat), Some(rule));
        }
        let mut st = State { trie: NONE, ..ps };
        if deep && st.cat == Category::Other && is_media_name(name) {
            st.cat = Category::Media;
        }
        (st, None)
    }

    fn file_matches(&self, rule: R, name: &[u8], node: &Node) -> bool {
        let Some(ext) = extension(name) else {
            return false;
        };
        match rule {
            R::Installers => {
                const EXTS: [&[u8]; 8] = [
                    b"deb",
                    b"rpm",
                    b"appimage",
                    b"iso",
                    b"dmg",
                    b"pkg",
                    b"exe",
                    b"msi",
                ];
                EXTS.contains(&ext.as_slice())
                    && self.now.saturating_sub(node.mtime) > OLD_INSTALLER_DAYS * DAY
            }
            R::Recordings => {
                const EXTS: [&[u8]; 4] = [b"mov", b"mp4", b"mkv", b"webm"];
                EXTS.contains(&ext.as_slice()) && node.size >= BIG_VIDEO
            }
            _ => false,
        }
    }

    /// Build outputs and dependency folders found by name, next to the file that proves them.
    fn name_match(&self, tree: &Tree, parent: NodeId, id: NodeId, name: &[u8]) -> NameMatch {
        let sib = |names: &[&str]| names.iter().any(|n| has_child(tree, parent, n));
        let hit = |r: R| NameMatch::Hit(r);
        match name {
            b"node_modules" => {
                if self.is_idle(tree, parent, id) {
                    hit(R::NodeModules)
                } else {
                    NameMatch::Artifact
                }
            }
            b"target" => {
                if sib(&["Cargo.toml"]) || has_child(tree, id, "CACHEDIR.TAG") {
                    hit(R::RustTarget)
                } else if sib(&["pom.xml"]) {
                    hit(R::MavenTarget)
                } else {
                    NameMatch::None
                }
            }
            b".next" | b".turbo" | b".parcel-cache" | b".nuxt" | b".svelte-kit" => {
                if sib(&["package.json"]) {
                    hit(R::JsBuild)
                } else {
                    NameMatch::None
                }
            }
            b"__pycache__" | b".pytest_cache" | b".mypy_cache" | b".ruff_cache" => {
                hit(R::PythonBytecode)
            }
            b".gradle" if sib(&GRADLE_FILES) => hit(R::GradleBuild),
            b"build" => {
                if sib(&GRADLE_FILES) {
                    hit(R::GradleBuild)
                } else if sib(&["pubspec.yaml"]) {
                    hit(R::DartBuild)
                } else {
                    NameMatch::None
                }
            }
            b".dart_tool" if sib(&["pubspec.yaml"]) => hit(R::DartBuild),
            b".venv" | b"venv" => {
                if sib(&["pyproject.toml", "requirements.txt", "setup.py", "Pipfile"]) {
                    hit(R::PythonVenv)
                } else if has_child(tree, id, "pyvenv.cfg") {
                    NameMatch::Artifact
                } else {
                    NameMatch::None
                }
            }
            b"Pods" if sib(&["Podfile"]) => hit(R::Cocoapods),
            b"worktrees" if tree.name_bytes(parent) == b".claude" => hit(R::Worktrees),
            _ => NameMatch::None,
        }
    }

    /// Whether nothing in `project`, apart from `node_modules` and other build outputs, changed
    /// for [`IDLE_DAYS`]. A project with no other content is not called idle: nothing proves it.
    fn is_idle(&self, tree: &Tree, project: NodeId, node_modules: NodeId) -> bool {
        let mut newest = 0u32;
        for c in tree.children(project) {
            let n = tree.node(c);
            if c == node_modules || n.has(FLAG_REMOVED) {
                continue;
            }
            if n.is_dir() && is_output_name(tree.name_bytes(c)) {
                continue;
            }
            newest = newest.max(n.mtime);
        }
        newest > 0 && self.now.saturating_sub(newest) > IDLE_DAYS * DAY
    }

    /// The state of `id`, computed from the root down its ancestry.
    pub fn state_of(&self, tree: &Tree, id: NodeId, deep: bool) -> State {
        let mut st = self.root_state(tree, deep);
        let chain = tree.ancestry(id);
        for pair in chain.windows(2) {
            if st.flags & LOCKED != 0 {
                break;
            }
            st = self.child(tree, pair[0], st, pair[1], deep).0;
        }
        st
    }

    /// Walks the whole tree. Returns the state of every node and every `(node, rule)` found.
    pub fn walk(&self, tree: &Tree, deep: bool) -> (Vec<State>, Vec<(NodeId, R)>) {
        let n = tree.len();
        let mut states = vec![State::START; n];
        let mut hits = Vec::new();
        states[0] = self.root_state(tree, deep);
        // The scanner reserves a directory's children after the directory itself, so a parent
        // always has a lower id than its children and one forward pass is top down.
        for id in 1..n as NodeId {
            let p = tree.node(id).parent;
            if p == NO_NODE || p >= id {
                continue;
            }
            let (st, hit) = self.child(tree, p, states[p as usize], id, deep);
            states[id as usize] = st;
            if let Some(r) = hit {
                hits.push((id, r));
            }
        }
        (states, hits)
    }

    /// Whether a folder with this name can be a match when it completes.
    pub fn self_candidate(&self, name: &[u8]) -> bool {
        self.in_set(&self.interesting, name)
    }

    /// Whether the files of a folder with this name can match a file rule.
    pub fn file_parent(&self, name: &[u8]) -> bool {
        self.in_set(&self.file_parents, name)
    }

    fn in_set(&self, set: &HashSet<Vec<u8>>, name: &[u8]) -> bool {
        let mut buf = [0u8; 64];
        if name.len() > buf.len() {
            return false;
        }
        for (b, c) in buf.iter_mut().zip(name) {
            *b = c.to_ascii_lowercase();
        }
        set.contains(&buf[..name.len()])
    }
}

const GRADLE_FILES: [&str; 4] = [
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
];

fn is_output_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"node_modules"
            | b".next"
            | b".turbo"
            | b".nuxt"
            | b".svelte-kit"
            | b".parcel-cache"
            | b"dist"
            | b"build"
            | b"target"
            | b".gradle"
            | b"__pycache__"
            | b".venv"
            | b"venv"
            | b"coverage"
    )
}

/// Whether directory `dir` has a direct child called `name`.
pub(crate) fn has_child(tree: &Tree, dir: NodeId, name: &str) -> bool {
    tree.children(dir)
        .any(|c| tree.name_bytes(c) == name.as_bytes())
}

/// Whether `dir` holds a version control folder or a build manifest.
fn is_project(tree: &Tree, dir: NodeId) -> bool {
    let (mut makefile, mut src) = (false, false);
    for c in tree.children(dir) {
        match tree.name_bytes(c) {
            b".git"
            | b"Cargo.toml"
            | b"package.json"
            | b"go.mod"
            | b"pyproject.toml"
            | b"pom.xml"
            | b"build.gradle"
            | b"build.gradle.kts"
            | b"settings.gradle"
            | b"settings.gradle.kts"
            | b"CMakeLists.txt"
            | b"pubspec.yaml"
            | b"composer.json"
            | b"Gemfile"
            | b"setup.py" => return true,
            b"Makefile" => makefile = true,
            b"src" => src = true,
            _ => {}
        }
    }
    makefile && src
}

/// The lowercase extension of `name` (up to 8 bytes), if any.
fn extension(name: &[u8]) -> Option<Vec<u8>> {
    let dot = name.iter().rposition(|b| *b == b'.')?;
    let ext = &name[dot + 1..];
    if dot == 0 || ext.is_empty() || ext.len() > 8 {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// Whether the file name has a photo, video, audio or document extension.
fn is_media_name(name: &[u8]) -> bool {
    let Some(dot) = name.iter().rposition(|b| *b == b'.') else {
        return false;
    };
    let ext = &name[dot + 1..];
    if ext.len() < 2 || ext.len() > 5 {
        return false;
    }
    let mut buf = [0u8; 5];
    for (b, c) in buf.iter_mut().zip(ext) {
        *b = c.to_ascii_lowercase();
    }
    matches!(
        &buf[..ext.len()],
        b"jpg"
            | b"jpeg"
            | b"png"
            | b"gif"
            | b"webp"
            | b"heic"
            | b"heif"
            | b"bmp"
            | b"tiff"
            | b"tif"
            | b"raw"
            | b"cr2"
            | b"nef"
            | b"arw"
            | b"dng"
            | b"psd"
            | b"mp4"
            | b"mov"
            | b"mkv"
            | b"avi"
            | b"webm"
            | b"m4v"
            | b"wmv"
            | b"flv"
            | b"mpg"
            | b"mpeg"
            | b"mp3"
            | b"flac"
            | b"wav"
            | b"aac"
            | b"ogg"
            | b"m4a"
            | b"opus"
            | b"wma"
            | b"aiff"
            | b"pdf"
            | b"doc"
            | b"docx"
            | b"odt"
            | b"xls"
            | b"xlsx"
            | b"ods"
            | b"ppt"
            | b"pptx"
            | b"odp"
            | b"epub"
            | b"rtf"
            | b"pages"
            | b"key"
    )
}
