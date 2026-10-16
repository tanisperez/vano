//! A compact binary form of a [`Tree`], for saving finished scans. No IO here: the scanner crate
//! writes and reads the bytes.
//!
//! Layout, all little endian: the magic `VANOTREE`, a `u32` version, the root path (`u32` length
//! and bytes), the node count and names length (`u64` each), then the fixed size node records and
//! finally the names buffer. Nodes are copied field by field, so the format does not depend on how
//! the in-memory struct is laid out.

use std::fmt;
use std::path::PathBuf;

use crate::tree::{Kind, NO_NODE, Node, Tree};

const MAGIC: &[u8; 8] = b"VANOTREE";
const VERSION: u32 = 1;
const NODE_BYTES: usize = 36;

/// Why a byte buffer is not a valid saved tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The data does not start with the Vano magic.
    BadMagic,
    /// Written by a newer (or unknown) format version.
    UnsupportedVersion(u32),
    /// The buffer ends before the declared content.
    Truncated,
    /// The content is structurally wrong (a reference out of range, an unknown kind...).
    Corrupt(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a Vano tree file"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported tree file version {v}"),
            Self::Truncated => write!(f, "tree file is truncated"),
            Self::Corrupt(why) => write!(f, "tree file is corrupt: {why}"),
        }
    }
}

impl std::error::Error for DecodeError {}

fn kind_to_byte(kind: Kind) -> u8 {
    match kind {
        Kind::Dir => 0,
        Kind::File => 1,
        Kind::Symlink => 2,
        Kind::Other => 3,
    }
}

fn kind_from_byte(byte: u8) -> Option<Kind> {
    match byte {
        0 => Some(Kind::Dir),
        1 => Some(Kind::File),
        2 => Some(Kind::Symlink),
        3 => Some(Kind::Other),
        _ => None,
    }
}

#[cfg(unix)]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// Serializes `tree`.
pub fn encode(tree: &Tree) -> Vec<u8> {
    let (root, nodes, names) = tree.raw_parts();
    let root = path_bytes(root);
    let mut out = Vec::with_capacity(32 + root.len() + nodes.len() * NODE_BYTES + names.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(root.len() as u32).to_le_bytes());
    out.extend_from_slice(&root);
    out.extend_from_slice(&(nodes.len() as u64).to_le_bytes());
    out.extend_from_slice(&(names.len() as u64).to_le_bytes());
    for node in nodes {
        out.extend_from_slice(&node.parent.to_le_bytes());
        out.extend_from_slice(&node.name_start.to_le_bytes());
        out.extend_from_slice(&node.name_len.to_le_bytes());
        out.push(kind_to_byte(node.kind));
        out.push(node.flags);
        out.extend_from_slice(&node.size.to_le_bytes());
        out.extend_from_slice(&node.files.to_le_bytes());
        out.extend_from_slice(&node.mtime.to_le_bytes());
        out.extend_from_slice(&node.first_child.to_le_bytes());
        out.extend_from_slice(&node.child_count.to_le_bytes());
    }
    out.extend_from_slice(names);
    out
}

/// A cursor over the input that fails with [`DecodeError::Truncated`] instead of panicking.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        if self.bytes.len() < len {
            return Err(DecodeError::Truncated);
        }
        let (head, tail) = self.bytes.split_at(len);
        self.bytes = tail;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        let mut buf = [0; 4];
        buf.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(buf))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        let mut buf = [0; 8];
        buf.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(buf))
    }
}

fn le_u32(bytes: &[u8]) -> u32 {
    let mut buf = [0; 4];
    buf.copy_from_slice(&bytes[..4]);
    u32::from_le_bytes(buf)
}

/// Rebuilds a tree from [`encode`] output, checking every reference so that a damaged file can
/// never make later tree accessors panic.
pub fn decode(bytes: &[u8]) -> Result<Tree, DecodeError> {
    let mut reader = Reader { bytes };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = reader.u32()?;
    if version != VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let root_len = reader.u32()? as usize;
    let root = path_from_bytes(reader.take(root_len)?);
    let count = reader.u64()?;
    let names_len = reader.u64()?;
    let count = usize::try_from(count).map_err(|_| DecodeError::Corrupt("node count"))?;
    let names_len = usize::try_from(names_len).map_err(|_| DecodeError::Corrupt("names size"))?;
    if count == 0 || count > NO_NODE as usize {
        return Err(DecodeError::Corrupt("node count"));
    }
    let node_bytes = count
        .checked_mul(NODE_BYTES)
        .ok_or(DecodeError::Truncated)?;
    let raw = reader.take(node_bytes)?;
    let names = reader.take(names_len)?.to_vec();

    let mut nodes = Vec::with_capacity(count);
    for rec in raw.as_chunks::<NODE_BYTES>().0 {
        let parent = le_u32(&rec[0..4]);
        let name_start = le_u32(&rec[4..8]);
        let name_len = u16::from_le_bytes([rec[8], rec[9]]);
        let kind = kind_from_byte(rec[10]).ok_or(DecodeError::Corrupt("node kind"))?;
        let flags = rec[11];
        let mut size = [0; 8];
        size.copy_from_slice(&rec[12..20]);
        let files = le_u32(&rec[20..24]);
        let mtime = le_u32(&rec[24..28]);
        let first_child = le_u32(&rec[28..32]);
        let child_count = le_u32(&rec[32..36]);

        if parent != NO_NODE && parent as usize >= count {
            return Err(DecodeError::Corrupt("parent out of range"));
        }
        if name_start as usize + name_len as usize > names_len {
            return Err(DecodeError::Corrupt("name out of range"));
        }
        if first_child != NO_NODE && first_child as usize + child_count as usize > count {
            return Err(DecodeError::Corrupt("children out of range"));
        }
        nodes.push(Node {
            parent,
            name_start,
            name_len,
            kind,
            flags,
            size: u64::from_le_bytes(size),
            files,
            mtime,
            first_child,
            child_count,
        });
    }
    Ok(Tree::from_raw_parts(root, nodes, names))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tree::{EntryData, ROOT};

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
        tree.set_children(1, 3, &[entry("x.txt", Kind::File, 10, 9)]);
        tree
    }

    #[test]
    fn round_trip() {
        let tree = sample();
        let back = decode(&encode(&tree)).unwrap();
        assert_eq!(back.root_path(), tree.root_path());
        assert_eq!(back.len(), tree.len());
        for id in 0..tree.len() as u32 {
            assert_eq!(back.name_bytes(id), tree.name_bytes(id));
            let (a, b) = (back.node(id), tree.node(id));
            assert_eq!(
                (a.size, a.files, a.mtime, a.flags),
                (b.size, b.files, b.mtime, b.flags)
            );
            assert_eq!((a.parent, a.kind), (b.parent, b.kind));
            assert_eq!(back.children(id), tree.children(id));
        }
    }

    #[test]
    fn rejects_garbage() {
        let bytes = encode(&sample());
        assert_eq!(decode(b"nope").unwrap_err(), DecodeError::Truncated);
        assert_eq!(
            decode(b"NOTVANOTREE0000").unwrap_err(),
            DecodeError::BadMagic
        );
        assert_eq!(
            decode(&bytes[..bytes.len() - 3]).unwrap_err(),
            DecodeError::Truncated
        );
        let mut bad = bytes.clone();
        bad[8] = 9;
        assert_eq!(
            decode(&bad).unwrap_err(),
            DecodeError::UnsupportedVersion(9)
        );
    }

    #[test]
    fn rejects_out_of_range_references() {
        let mut bytes = encode(&sample());
        // Root record starts after magic(8) version(4) pathlen(4) path(7) counts(16).
        let root_rec = 8 + 4 + 4 + 7 + 16;
        // first_child of the root points past the end.
        bytes[root_rec + 28..root_rec + 32].copy_from_slice(&100u32.to_le_bytes());
        assert!(matches!(decode(&bytes), Err(DecodeError::Corrupt(_))));
    }
}
