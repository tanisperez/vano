//! The scanned file tree: one flat arena of [`Node`]s, built for millions of entries.
//!
//! Every node lives in one `Vec`, addressed by a [`NodeId`]. The children of a directory always
//! have consecutive ids (the scanner reserves the whole range when it reads the directory), so a
//! directory only stores where its range starts and how long it is. Names live in one shared
//! byte buffer. A node costs 40 bytes plus its name, about a third of what a `PathBuf` per entry
//! would cost.
//!
//! Sizes are allocated sizes (the space the file really takes on disk), and a directory's size,
//! file count and modification time are the totals of its whole subtree.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The index of a node in its [`Tree`].
pub type NodeId = u32;

/// Marks "no node": the parent of the root, the children of a file.
pub const NO_NODE: NodeId = u32::MAX;

/// The id of the root node, always present.
pub const ROOT: NodeId = 0;

/// What kind of entry a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    #[default]
    Dir,
    File,
    /// A symbolic link. Never followed; its own (tiny) size is counted.
    Symlink,
    /// Sockets, pipes, devices.
    Other,
}

/// The directory could not be read (permissions, IO error): its size only covers what was read.
pub const FLAG_UNREADABLE: u8 = 1;
/// The directory is a mount point of another file system and was not entered.
pub const FLAG_OTHER_FS: u8 = 1 << 1;
/// Every directory below this one has been read. Set by the scanner as the scan advances.
pub const FLAG_COMPLETE: u8 = 1 << 2;
/// The node was moved to the Trash: its size no longer counts in its ancestors.
pub const FLAG_REMOVED: u8 = 1 << 3;
/// A hard link to a file already counted elsewhere: listed, but with no size of its own.
pub const FLAG_HARDLINK_DUP: u8 = 1 << 4;

/// One entry of the tree.
#[derive(Debug, Clone, Copy)]
pub struct Node {
    /// The parent directory, [`NO_NODE`] for the root.
    pub parent: NodeId,
    pub(crate) name_start: u32,
    pub(crate) name_len: u16,
    /// What the entry is.
    pub kind: Kind,
    /// `FLAG_*` bits.
    pub flags: u8,
    /// Allocated bytes: the file's own, or the whole subtree's for a directory.
    pub size: u64,
    /// Files in the subtree (1 for a file, 0 for an empty directory).
    pub files: u32,
    /// Newest modification time in the subtree, in seconds since the Unix epoch.
    pub mtime: u32,
    pub(crate) first_child: NodeId,
    pub(crate) child_count: u32,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            parent: NO_NODE,
            name_start: 0,
            name_len: 0,
            kind: Kind::Dir,
            flags: 0,
            size: 0,
            files: 0,
            mtime: 0,
            first_child: NO_NODE,
            child_count: 0,
        }
    }
}

impl Node {
    /// Whether this node is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }

    /// Whether `flag` (a `FLAG_*` constant) is set.
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// What the scanner knows about one entry when it reads a directory.
#[derive(Debug, Clone)]
pub struct EntryData {
    /// The file name, raw bytes on Unix (any byte but `/` and NUL), UTF-8 elsewhere.
    pub name: Vec<u8>,
    /// What the entry is.
    pub kind: Kind,
    /// Allocated bytes. Directories pass 0: their size comes from their own listing.
    pub size: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: u32,
    /// `FLAG_*` bits to start with (`FLAG_OTHER_FS`, `FLAG_HARDLINK_DUP`, ...).
    pub flags: u8,
}

/// The whole scanned tree.
#[derive(Debug, Clone)]
pub struct Tree {
    root_path: PathBuf,
    nodes: Vec<Node>,
    names: Vec<u8>,
}

impl Tree {
    /// A tree with only its root directory, `root_path`.
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        let mut tree = Self {
            root_path: root_path.into(),
            nodes: Vec::new(),
            names: Vec::new(),
        };
        tree.nodes.push(Node::default());
        tree
    }

    /// The path the scan started from.
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Number of node slots, including slots reserved but not written yet.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Never true: the root always exists.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The node `id`. Panics on an id that was never reserved, like slice indexing.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// The node `id`, or `None` past the end.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    /// The name of `id` as raw bytes. The root's name is empty, see [`Tree::display_name`].
    pub fn name_bytes(&self, id: NodeId) -> &[u8] {
        let node = self.node(id);
        let start = node.name_start as usize;
        &self.names[start..start + node.name_len as usize]
    }

    /// The name of `id` for display (lossy on invalid UTF-8). The root shows its full path.
    pub fn display_name(&self, id: NodeId) -> String {
        if id == ROOT {
            return self.root_path.display().to_string();
        }
        String::from_utf8_lossy(self.name_bytes(id)).into_owned()
    }

    /// The name of `id` as an `OsStr`, exact on Unix.
    #[cfg(unix)]
    pub fn os_name(&self, id: NodeId) -> &OsStr {
        use std::os::unix::ffi::OsStrExt as _;
        OsStr::from_bytes(self.name_bytes(id))
    }

    /// The name of `id` as an `OsStr`. Names are stored as UTF-8 off Unix.
    #[cfg(not(unix))]
    pub fn os_name(&self, id: NodeId) -> &OsStr {
        OsStr::new(std::str::from_utf8(self.name_bytes(id)).unwrap_or(""))
    }

    /// The full path of `id` on disk.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut parts = Vec::new();
        let mut cur = id;
        while cur != ROOT && cur != NO_NODE {
            parts.push(cur);
            cur = self.node(cur).parent;
        }
        let mut path = self.root_path.clone();
        for part in parts.iter().rev() {
            path.push(self.os_name(*part));
        }
        path
    }

    /// The ids of `id`'s children, in reading order (not sorted).
    pub fn children(&self, id: NodeId) -> std::ops::Range<NodeId> {
        let node = self.node(id);
        if node.first_child == NO_NODE {
            return 0..0;
        }
        node.first_child..node.first_child + node.child_count
    }

    /// The children of `id` that are still present, biggest first.
    pub fn sorted_children(&self, id: NodeId) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self
            .children(id)
            .filter(|child| !self.node(*child).has(FLAG_REMOVED))
            .collect();
        ids.sort_by(|a, b| {
            let (na, nb) = (self.node(*a), self.node(*b));
            nb.size.cmp(&na.size).then_with(|| a.cmp(b))
        });
        ids
    }

    /// The chain from the root down to `id`, both included.
    pub fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = vec![id];
        let mut cur = self.node(id).parent;
        while cur != NO_NODE {
            chain.push(cur);
            cur = self.node(cur).parent;
        }
        chain.reverse();
        chain
    }

    /// Depth of `id` below the root (the root is 0).
    pub fn depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut cur = self.node(id).parent;
        while cur != NO_NODE {
            depth += 1;
            cur = self.node(cur).parent;
        }
        depth
    }

    /// Whether `ancestor` is `id` or one of its ancestors.
    pub fn is_within(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut cur = id;
        while cur != NO_NODE {
            if cur == ancestor {
                return true;
            }
            cur = self.node(cur).parent;
        }
        false
    }

    /// Makes room for ids up to `len - 1`. Slots that are not written yet stay default nodes.
    pub fn ensure_len(&mut self, len: usize) {
        if self.nodes.len() < len {
            self.nodes.resize(len, Node::default());
        }
    }

    /// Writes the listing of directory `dir`: its children take ids `first..first + entries.len()`
    /// (reserved by the caller), and their sizes, file counts and newest time are added to `dir`
    /// and to every ancestor of it.
    pub fn set_children(&mut self, dir: NodeId, first: NodeId, entries: &[EntryData]) {
        let end = first as usize + entries.len();
        self.ensure_len(end);
        let mut size = 0u64;
        let mut files = 0u32;
        let mut mtime = 0u32;
        for (i, entry) in entries.iter().enumerate() {
            let name_start = self.names.len() as u32;
            let name_len = entry.name.len().min(u16::MAX as usize);
            self.names.extend_from_slice(&entry.name[..name_len]);
            let is_file = entry.kind != Kind::Dir;
            self.nodes[first as usize + i] = Node {
                parent: dir,
                name_start,
                name_len: name_len as u16,
                kind: entry.kind,
                flags: entry.flags | if is_file { FLAG_COMPLETE } else { 0 },
                size: entry.size,
                files: u32::from(is_file),
                mtime: entry.mtime,
                first_child: NO_NODE,
                child_count: 0,
            };
            size += entry.size;
            files += u32::from(is_file);
            mtime = mtime.max(entry.mtime);
        }
        let node = &mut self.nodes[dir as usize];
        node.first_child = if entries.is_empty() { NO_NODE } else { first };
        node.child_count = entries.len() as u32;
        self.add_to_chain(dir, size, files, mtime);
    }

    /// Adds `size`, `files` and `mtime` to `id` and every ancestor of it.
    pub fn add_to_chain(&mut self, id: NodeId, size: u64, files: u32, mtime: u32) {
        let mut cur = id;
        while cur != NO_NODE {
            let node = &mut self.nodes[cur as usize];
            node.size += size;
            node.files += files;
            node.mtime = node.mtime.max(mtime);
            cur = node.parent;
        }
    }

    /// Sets `flag` on `id`.
    pub fn set_flag(&mut self, id: NodeId, flag: u8) {
        self.nodes[id as usize].flags |= flag;
    }

    /// Sets the modification time of `id` itself (a directory's own mtime), keeping the newest.
    pub fn touch(&mut self, id: NodeId, mtime: u32) {
        let node = &mut self.nodes[id as usize];
        node.mtime = node.mtime.max(mtime);
    }

    /// Marks `id` as moved to the Trash and subtracts its size and files from its ancestors.
    /// Does nothing if it is already removed or is the root.
    pub fn mark_removed(&mut self, id: NodeId) {
        let node = *self.node(id);
        if id == ROOT || node.has(FLAG_REMOVED) || self.has_removed_ancestor(id) {
            return;
        }
        self.nodes[id as usize].flags |= FLAG_REMOVED;
        let mut cur = node.parent;
        while cur != NO_NODE {
            let parent = &mut self.nodes[cur as usize];
            parent.size = parent.size.saturating_sub(node.size);
            parent.files = parent.files.saturating_sub(node.files);
            cur = parent.parent;
        }
    }

    /// Undoes [`Tree::mark_removed`].
    pub fn unmark_removed(&mut self, id: NodeId) {
        let node = *self.node(id);
        if !node.has(FLAG_REMOVED) {
            return;
        }
        self.nodes[id as usize].flags &= !FLAG_REMOVED;
        if self.has_removed_ancestor(id) {
            return;
        }
        let mut cur = node.parent;
        while cur != NO_NODE {
            let parent = &mut self.nodes[cur as usize];
            parent.size += node.size;
            parent.files += node.files;
            cur = parent.parent;
        }
    }

    /// Whether `id` or an ancestor of it was moved to the Trash.
    pub fn is_removed(&self, id: NodeId) -> bool {
        self.node(id).has(FLAG_REMOVED) || self.has_removed_ancestor(id)
    }

    fn has_removed_ancestor(&self, id: NodeId) -> bool {
        let mut cur = self.node(id).parent;
        while cur != NO_NODE {
            if self.node(cur).has(FLAG_REMOVED) {
                return true;
            }
            cur = self.node(cur).parent;
        }
        false
    }

    /// Finds the node at `path`, if it is inside the tree.
    pub fn find(&self, path: &Path) -> Option<NodeId> {
        let rest = path.strip_prefix(&self.root_path).ok()?;
        let mut cur = ROOT;
        for part in rest.components() {
            let name = part.as_os_str();
            cur = self
                .children(cur)
                .find(|child| self.os_name(*child) == name)?;
        }
        Some(cur)
    }

    /// Finds a child of `dir` by name.
    pub fn child_named(&self, dir: NodeId, name: &str) -> Option<NodeId> {
        self.children(dir)
            .find(|child| self.name_bytes(*child) == name.as_bytes())
    }

    /// The raw storage, for the binary encoder: root path, node slots and the names buffer.
    pub(crate) fn raw_parts(&self) -> (&Path, &[Node], &[u8]) {
        (&self.root_path, &self.nodes, &self.names)
    }

    /// Rebuilds a tree from raw storage already checked by the decoder.
    pub(crate) fn from_raw_parts(root_path: PathBuf, nodes: Vec<Node>, names: Vec<u8>) -> Self {
        Self {
            root_path,
            nodes,
            names,
        }
    }

    /// Approximate heap bytes the tree uses.
    pub fn heap_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<Node>() + self.names.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: Kind, size: u64, mtime: u32) -> EntryData {
        EntryData {
            name: name.as_bytes().to_vec(),
            kind,
            size,
            mtime,
            flags: 0,
        }
    }

    fn sample() -> Tree {
        let mut tree = Tree::new("/home/u");
        tree.set_children(
            ROOT,
            1,
            &[
                entry("a", Kind::Dir, 0, 5),
                entry("big.iso", Kind::File, 1000, 7),
            ],
        );
        tree.set_children(
            1,
            3,
            &[
                entry("x.txt", Kind::File, 10, 9),
                entry("y.txt", Kind::File, 30, 3),
            ],
        );
        tree
    }

    #[test]
    fn sizes_add_up_the_chain() {
        let tree = sample();
        assert_eq!(tree.node(ROOT).size, 1040);
        assert_eq!(tree.node(ROOT).files, 3);
        assert_eq!(tree.node(ROOT).mtime, 9);
        assert_eq!(tree.node(1).size, 40);
        assert_eq!(tree.node(1).files, 2);
    }

    #[test]
    fn paths_and_lookup() {
        let tree = sample();
        assert_eq!(tree.path(4), PathBuf::from("/home/u/a/y.txt"));
        assert_eq!(tree.find(Path::new("/home/u/a/y.txt")), Some(4));
        assert_eq!(tree.find(Path::new("/home/u/zzz")), None);
        assert_eq!(tree.ancestry(4), vec![0, 1, 4]);
        assert_eq!(tree.depth(4), 2);
        assert!(tree.is_within(4, 1));
        assert!(!tree.is_within(2, 1));
    }

    #[test]
    fn sorted_children_biggest_first() {
        let tree = sample();
        assert_eq!(tree.sorted_children(ROOT), vec![2, 1]);
        assert_eq!(tree.sorted_children(1), vec![4, 3]);
        assert!(tree.sorted_children(2).is_empty());
    }

    #[test]
    fn removal_round_trip() {
        let mut tree = sample();
        tree.mark_removed(1);
        assert_eq!(tree.node(ROOT).size, 1000);
        assert_eq!(tree.node(ROOT).files, 1);
        assert!(tree.is_removed(3));
        assert_eq!(tree.sorted_children(ROOT), vec![2]);
        // A child of a removed node changes nothing more.
        tree.mark_removed(3);
        assert_eq!(tree.node(ROOT).size, 1000);
        tree.unmark_removed(3);
        tree.unmark_removed(1);
        assert_eq!(tree.node(ROOT).size, 1040);
        assert_eq!(tree.node(ROOT).files, 3);
    }
}
