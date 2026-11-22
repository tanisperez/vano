//! The freedesktop Trash of a Linux system reached from Windows (a WSL2 distribution at
//! `\\wsl.localhost\<distro>`), where the Windows Recycle Bin refuses the path.
//!
//! Items are renamed into the user's `~/.local/share/Trash/files` and described by a
//! `.trashinfo` in `info`, so they can be put back by Vano, `trash-restore` or `gio trash`
//! inside the distribution. An item is only ever renamed, never copied or deleted.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};

/// The format of `DeletionDate` in a `.trashinfo`, in local time.
const DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// Moves `item`, which lives under the Linux root `root`, to the Trash of its user.
///
/// # Errors
///
/// The reason as text. The item stays in place and no `.trashinfo` is left behind.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn trash(root: &Path, item: &Path) -> Result<(), String> {
    let parts = linux_parts(root, item)?;
    let base = parts
        .last()
        .ok_or("the root cannot be moved to the Trash")?;
    let trash = trash_dir(root, &parts)?;
    let (files, info) = (trash.join("files"), trash.join("info"));
    for dir in [&files, &info] {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        encode(&format!("/{}", parts.join("/"))),
        Local::now().format(DATE_FORMAT)
    );
    for number in 1u32.. {
        let name = if number == 1 {
            base.clone()
        } else {
            format!("{base}.{number}")
        };
        let target = files.join(&name);
        if target.symlink_metadata().is_ok() {
            continue;
        }
        let info_file = info.join(format!("{name}.trashinfo"));
        let mut created = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info_file)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        };
        let moved = created
            .write_all(text.as_bytes())
            .and_then(|()| std::fs::rename(item, &target));
        drop(created);
        if let Err(error) = moved {
            let _ = std::fs::remove_file(&info_file);
            return Err(error.to_string());
        }
        return Ok(());
    }
    Err("no free name in the Trash".to_string())
}

/// Puts back the `items` (under the Linux root `root`) trashed at `since` (Unix seconds) or
/// later. An item whose place is taken again is not restored.
///
/// # Errors
///
/// The first failure as text, or a note that nothing matching was found.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn restore(root: &Path, items: &[PathBuf], since: i64) -> Result<(), String> {
    // The records of each Trash folder, read once.
    let mut records: HashMap<PathBuf, Vec<Record>> = HashMap::new();
    let mut restored = 0;
    let mut first_error = None;
    for item in items {
        let outcome = linux_parts(root, item).and_then(|parts| {
            let trash = trash_dir(root, &parts)?;
            let wanted = format!("/{}", parts.join("/"));
            let list = records
                .entry(trash.clone())
                .or_insert_with(|| read_records(&trash.join("info")));
            let found = list
                .iter()
                .filter(|record| record.path == wanted && record.time >= since - 2)
                .max_by_key(|record| record.time)
                .ok_or("the item is no longer in the Trash")?;
            put_back(&trash, &found.name, item)
        });
        match outcome {
            Ok(()) => restored += 1,
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None if restored == 0 => Err("the items are no longer in the Trash".to_string()),
        None => Ok(()),
    }
}

/// Renames `files/<name>` back to `item` and forgets its record.
fn put_back(trash: &Path, name: &str, item: &Path) -> Result<(), String> {
    if item.symlink_metadata().is_ok() {
        return Err(format!("{} exists again", item.display()));
    }
    std::fs::rename(trash.join("files").join(name), item).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(trash.join("info").join(format!("{name}.trashinfo")));
    Ok(())
}

/// One `.trashinfo`: the name in `files`, the original Linux path and the deletion time.
struct Record {
    name: String,
    path: String,
    time: i64,
}

/// The readable records of an `info` folder.
fn read_records(info: &Path) -> Vec<Record> {
    let Ok(entries) = std::fs::read_dir(info) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let name = file.strip_suffix(".trashinfo")?.to_string();
            let text = std::fs::read_to_string(entry.path()).ok()?;
            let (path, time) = parse_info(&text)?;
            Some(Record { name, path, time })
        })
        .collect()
}

/// The decoded `Path` and the `DeletionDate` (Unix seconds) of a `.trashinfo`.
fn parse_info(text: &str) -> Option<(String, i64)> {
    let mut path = None;
    let mut time = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Path=") {
            path = decode(value.trim());
        } else if let Some(value) = line.strip_prefix("DeletionDate=") {
            time = NaiveDateTime::parse_from_str(value.trim(), DATE_FORMAT)
                .ok()
                .and_then(|date| Local.from_local_datetime(&date).earliest())
                .map(|date| date.timestamp());
        }
    }
    Some((path?, time?))
}

/// The components of `item` below `root`, which are the Linux path split at `/`.
fn linux_parts(root: &Path, item: &Path) -> Result<Vec<String>, String> {
    let relative = item
        .strip_prefix(root)
        .map_err(|_| format!("{} is not inside {}", item.display(), root.display()))?;
    relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{} is not valid text", item.display()))
        })
        .collect()
}

/// The `Trash` folder for an item with the Linux path `parts`: its own user's if it is inside a
/// home, else the default user of `/etc/wsl.conf`, else the only user of `/home`.
fn trash_dir(root: &Path, parts: &[String]) -> Result<PathBuf, String> {
    let home = root.join("home");
    let user = match parts {
        [first, user, _, ..] if first == "home" => user.clone(),
        _ => default_user(root)
            .filter(|user| home.join(user).is_dir())
            .or_else(|| only_user(&home))
            .ok_or("cannot tell which user's Trash to use")?,
    };
    Ok(home.join(user).join(".local/share/Trash"))
}

/// `default=` of the `[user]` section of `/etc/wsl.conf`.
fn default_user(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("etc/wsl.conf")).ok()?;
    let mut in_user = false;
    for line in text.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_user = section.trim().eq_ignore_ascii_case("user");
        } else if in_user
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "default"
        {
            let value = value.trim().trim_matches('"');
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// The only folder in `home`, if there is exactly one.
fn only_user(home: &Path) -> Option<String> {
    let mut users = std::fs::read_dir(home)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok());
    let user = users.next()?;
    users.next().is_none().then_some(user)
}

/// Percent-encodes a path as the Trash spec asks: unreserved characters and `/` stay.
fn encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Undoes [`encode`]; `None` for a malformed escape or text that is not UTF-8.
fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A fake Linux root with the given users in `home`.
    fn root(users: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("proc")).unwrap();
        std::fs::create_dir(dir.path().join("etc")).unwrap();
        for user in users {
            std::fs::create_dir_all(dir.path().join("home").join(user)).unwrap();
        }
        dir
    }

    fn file(root: &Path, relative: &str) -> PathBuf {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"x").unwrap();
        path
    }

    fn now() -> i64 {
        Local::now().timestamp()
    }

    fn trash_of(root: &Path, user: &str) -> PathBuf {
        root.join("home").join(user).join(".local/share/Trash")
    }

    #[test]
    fn moving_and_restoring_round_trips() {
        let dir = root(&["ana"]);
        let since = now();
        let item = file(dir.path(), "home/ana/proj/a.txt");
        trash(dir.path(), &item).unwrap();
        assert!(!item.exists());
        let trash = trash_of(dir.path(), "ana");
        assert!(trash.join("files/a.txt").exists());
        let info = std::fs::read_to_string(trash.join("info/a.txt.trashinfo")).unwrap();
        assert!(info.starts_with("[Trash Info]\nPath=/home/ana/proj/a.txt\nDeletionDate="));
        restore(dir.path(), std::slice::from_ref(&item), since).unwrap();
        assert!(item.exists());
        assert!(!trash.join("files/a.txt").exists());
        assert!(!trash.join("info/a.txt.trashinfo").exists());
    }

    #[test]
    fn a_folder_moves_whole() {
        let dir = root(&["ana"]);
        file(dir.path(), "home/ana/proj/node_modules/x/y.js");
        let item = dir.path().join("home/ana/proj/node_modules");
        trash(dir.path(), &item).unwrap();
        assert!(!item.exists());
        let moved = trash_of(dir.path(), "ana").join("files/node_modules/x/y.js");
        assert!(moved.exists());
    }

    #[test]
    fn equal_names_get_a_free_name() {
        let dir = root(&["ana"]);
        let since = now();
        let a = file(dir.path(), "home/ana/one/same");
        let b = file(dir.path(), "home/ana/two/same");
        trash(dir.path(), &a).unwrap();
        trash(dir.path(), &b).unwrap();
        let files = trash_of(dir.path(), "ana").join("files");
        assert!(files.join("same").exists());
        assert!(files.join("same.2").exists());
        restore(dir.path(), &[a.clone(), b.clone()], since).unwrap();
        assert!(a.exists() && b.exists());
    }

    #[test]
    fn a_stale_info_file_also_takes_the_name() {
        let dir = root(&["ana"]);
        let info = trash_of(dir.path(), "ana").join("info");
        std::fs::create_dir_all(&info).unwrap();
        std::fs::write(info.join("a.trashinfo"), "x").unwrap();
        let item = file(dir.path(), "home/ana/a");
        trash(dir.path(), &item).unwrap();
        assert!(trash_of(dir.path(), "ana").join("files/a.2").exists());
        assert_eq!(
            std::fs::read_to_string(info.join("a.trashinfo")).unwrap(),
            "x"
        );
    }

    #[test]
    fn spaces_and_non_ascii_survive_the_encoding() {
        let dir = root(&["ana"]);
        let since = now();
        let item = file(dir.path(), "home/ana/mis fotos/año 100% #1.txt");
        trash(dir.path(), &item).unwrap();
        let info = trash_of(dir.path(), "ana").join("info/año 100% #1.txt.trashinfo");
        let text = std::fs::read_to_string(info).unwrap();
        assert!(text.contains("Path=/home/ana/mis%20fotos/a%C3%B1o%20100%25%20%231.txt\n"));
        restore(dir.path(), std::slice::from_ref(&item), since).unwrap();
        assert!(item.exists());
        assert_eq!(decode(&encode("/a b/ñ%")).unwrap(), "/a b/ñ%");
    }

    #[test]
    fn an_item_in_a_home_goes_to_that_users_trash() {
        let dir = root(&["ana", "bea"]);
        let item = file(dir.path(), "home/bea/a");
        trash(dir.path(), &item).unwrap();
        assert!(trash_of(dir.path(), "bea").join("files/a").exists());
        assert!(!trash_of(dir.path(), "ana").exists());
    }

    #[test]
    fn outside_a_home_the_default_user_is_used() {
        let dir = root(&["ana", "bea"]);
        std::fs::write(
            dir.path().join("etc/wsl.conf"),
            "[boot]\ndefault=ana\n[user]\n# comment\ndefault = bea\n",
        )
        .unwrap();
        let item = file(dir.path(), "opt/app/a");
        trash(dir.path(), &item).unwrap();
        assert!(trash_of(dir.path(), "bea").join("files/a").exists());
        let info = std::fs::read_to_string(trash_of(dir.path(), "bea").join("info/a.trashinfo"));
        assert!(info.unwrap().contains("Path=/opt/app/a\n"));
    }

    #[test]
    fn without_a_usable_default_user_the_only_user_is_used() {
        let dir = root(&["ana"]);
        let item = file(dir.path(), "opt/a");
        trash(dir.path(), &item).unwrap();
        assert!(trash_of(dir.path(), "ana").join("files/a").exists());
        // A default user without a home is ignored.
        let other = root(&["ana"]);
        std::fs::write(other.path().join("etc/wsl.conf"), "[user]\ndefault=ghost\n").unwrap();
        let item = file(other.path(), "opt/a");
        trash(other.path(), &item).unwrap();
        assert!(trash_of(other.path(), "ana").join("files/a").exists());
    }

    #[test]
    fn with_no_way_to_pick_a_user_it_fails_and_keeps_the_item() {
        let dir = root(&["ana", "bea"]);
        let item = file(dir.path(), "opt/a");
        assert!(trash(dir.path(), &item).is_err());
        assert!(item.exists());
        let none = root(&[]);
        let item = file(none.path(), "opt/a");
        assert!(trash(none.path(), &item).is_err());
        assert!(item.exists());
    }

    #[test]
    fn a_failed_rename_leaves_no_info_file() {
        let dir = root(&["ana"]);
        let missing = dir.path().join("home/ana/missing");
        assert!(trash(dir.path(), &missing).is_err());
        let info = trash_of(dir.path(), "ana").join("info");
        assert_eq!(std::fs::read_dir(info).unwrap().count(), 0);
    }

    #[test]
    fn restore_refuses_to_overwrite_and_ignores_older_trashings() {
        let dir = root(&["ana"]);
        let item = file(dir.path(), "home/ana/a");
        trash(dir.path(), &item).unwrap();
        // Trashed before `since`: no match.
        assert!(restore(dir.path(), std::slice::from_ref(&item), now() + 60).is_err());
        // The place is taken again: the item stays in the Trash.
        std::fs::write(&item, b"new").unwrap();
        assert!(restore(dir.path(), std::slice::from_ref(&item), now() - 10).is_err());
        assert_eq!(std::fs::read(&item).unwrap(), b"new");
        assert!(trash_of(dir.path(), "ana").join("files/a").exists());
    }

    #[test]
    fn the_newest_of_two_copies_comes_back() {
        let dir = root(&["ana"]);
        let item = file(dir.path(), "home/ana/a");
        trash(dir.path(), &item).unwrap();
        // Make the first record older than `since`.
        let info = trash_of(dir.path(), "ana").join("info/a.trashinfo");
        let text = std::fs::read_to_string(&info).unwrap();
        let old = text
            .lines()
            .map(|line| {
                if line.starts_with("DeletionDate=") {
                    "DeletionDate=2000-01-01T00:00:00"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&info, old).unwrap();
        let since = now();
        std::fs::write(&item, b"second").unwrap();
        trash(dir.path(), &item).unwrap();
        restore(dir.path(), std::slice::from_ref(&item), since).unwrap();
        assert_eq!(std::fs::read(&item).unwrap(), b"second");
        assert!(trash_of(dir.path(), "ana").join("files/a").exists());
    }
}
