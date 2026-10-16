//! Saving and loading finished scans, so results survive a restart.

use std::io;
use std::path::{Path, PathBuf};

use vano_core::Tree;
use vano_core::persist;

/// Writes `tree` to `path`. The data goes to a temporary file next to it first and is renamed
/// into place, so a crash never leaves a half written cache.
pub fn save(tree: &Tree, path: &Path) -> io::Result<()> {
    let bytes = persist::encode(tree);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp: PathBuf = path.to_path_buf();
    let mut name = tmp
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    tmp.set_file_name(name);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Reads a tree saved by [`save`]. Corrupt or foreign files give an `InvalidData` error.
pub fn load(path: &Path) -> io::Result<Tree> {
    let bytes = std::fs::read(path)?;
    persist::decode(&bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}
