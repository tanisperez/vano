//! The WSL 2 distributions of a Windows computer: listing them, scanning inside one, and
//! deleting inside one.
//!
//! Each distribution keeps its files in an `ext4.vhdx` that grows but never shrinks by itself.
//! Reading it from Windows (`\\wsl$`) goes through 9P and is slow, so Vano runs its own Linux
//! program inside the distribution instead (`vano-wsl`, shipped next to `vano.exe`): a
//! native scan whose tree comes back through `wsl.exe`'s standard output, and a native permanent
//! deletion. The Recycle Bin cannot take a distribution's files, so deleting there is always for
//! good.
//!
//! The parsing is plain Rust, unit tested everywhere. Only [`list_distros`], [`scan`] and
//! [`delete`] touch `wsl.exe` and the registry, and they do nothing outside Windows.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use vano_core::Tree;
use vano_scan::ExternalScan;

/// One WSL 2 distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    /// `Ubuntu-24.04`, `docker-desktop`.
    pub name: String,
    pub running: bool,
    /// The one `wsl` opens without `-d`.
    pub default: bool,
    /// Its virtual disk, when found.
    pub vhdx: Option<PathBuf>,
    /// The size of the virtual disk on the Windows disk.
    pub vhdx_size: u64,
    /// Bytes used inside, only known for a running distribution (asking would start it).
    pub used_inside: Option<u64>,
}

impl Distro {
    /// Docker Desktop's own distributions, which Docker manages.
    pub fn is_docker(&self) -> bool {
        self.name.starts_with("docker-desktop")
    }
}

/// The name of the Linux program next to `vano.exe`.
pub const AGENT: &str = "vano-wsl";

/// Where the Linux program is: `VANO_WSL_AGENT` (for development), or next to the running
/// executable.
pub fn agent() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VANO_WSL_AGENT") {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join(AGENT);
    path.is_file().then_some(path)
}

/// The WSL 2 distributions, the default first. Empty outside Windows or without WSL. Runs
/// `wsl.exe` (tens of milliseconds, more for each running distribution): call it in the
/// background.
pub fn list_distros() -> Vec<Distro> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let registered = registered();
    if registered.is_empty() {
        return Vec::new();
    }
    let running = wsl_output(&["--list", "--running", "--quiet"])
        .map(|text| parse_names(&text))
        .unwrap_or_default();
    let mut distros: Vec<Distro> = registered
        .into_iter()
        .map(|entry| {
            let is_running = running.contains(&entry.name);
            let vhdx = entry.base_path.map(|base| base.join(&entry.vhd_file));
            let vhdx_size = vhdx
                .as_ref()
                .and_then(|path| path.metadata().ok())
                .map_or(0, |metadata| metadata.len());
            let used_inside = if is_running {
                wsl_output(&[
                    "--distribution",
                    &entry.name,
                    "--exec",
                    "df",
                    "-B1",
                    "--output=used",
                    "/",
                ])
                .and_then(|text| parse_df_used(&text))
            } else {
                None
            };
            Distro {
                name: entry.name,
                running: is_running,
                default: entry.default,
                vhdx: vhdx.filter(|path| path.is_file()),
                vhdx_size,
                used_inside,
            }
        })
        .collect();
    distros.sort_by(|a, b| b.default.cmp(&a.default).then(a.name.cmp(&b.name)));
    distros
}

/// A distribution as the registry lists it.
#[derive(Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Registered {
    name: String,
    base_path: Option<PathBuf>,
    vhd_file: String,
    default: bool,
}

/// The WSL 2 distributions under `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss`.
#[cfg(windows)]
fn registered() -> Vec<Registered> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let Ok(lxss) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Lxss")
    else {
        return Vec::new();
    };
    let default: String = lxss.get_value("DefaultDistribution").unwrap_or_default();
    lxss.enum_keys()
        .flatten()
        .filter_map(|guid| {
            let key = lxss.open_subkey(&guid).ok()?;
            let version: u32 = key.get_value("Version").unwrap_or(0);
            if version != 2 {
                return None;
            }
            let name: String = key.get_value("DistributionName").ok()?;
            let base: Option<String> = key.get_value("BasePath").ok();
            Some(Registered {
                name,
                base_path: base.map(|base| PathBuf::from(strip_verbatim(&base))),
                vhd_file: key
                    .get_value("VhdFileName")
                    .unwrap_or_else(|_| "ext4.vhdx".to_string()),
                default: guid.eq_ignore_ascii_case(&default),
            })
        })
        .collect()
}

#[cfg(not(windows))]
fn registered() -> Vec<Registered> {
    Vec::new()
}

/// `\\?\C:\Users\...` as `C:\Users\...`: some distributions register their path verbatim.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn strip_verbatim(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}

/// A `wsl.exe` command that opens no console window, with UTF-8 output.
fn wsl_command(args: &[&str]) -> Command {
    let mut command = Command::new("wsl.exe");
    command.args(args).env("WSL_UTF8", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // `CREATE_NO_WINDOW` (`winbase.h`): the GUI has no console, and wsl.exe would open one.
        command.creation_flags(0x0800_0000);
    }
    command
}

/// The text `wsl.exe` prints, or `None` when it fails.
fn wsl_output(args: &[&str]) -> Option<String> {
    let output = wsl_command(args).stdin(Stdio::null()).output().ok()?;
    output
        .status
        .success()
        .then(|| decode_wsl_text(&output.stdout))
}

/// `wsl.exe` prints UTF-16 unless `WSL_UTF8` is set, which older versions ignore.
fn decode_wsl_text(bytes: &[u8]) -> String {
    let utf16 = bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).take(8).all(|b| *b == 0);
    if utf16 {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// The names of `wsl --list --quiet`, one per line.
fn parse_names(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}'))
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// The second line of `df -B1 --output=used /`.
fn parse_df_used(text: &str) -> Option<u64> {
    text.lines().nth(1)?.trim().parse().ok()
}

/// One line of the progress `vano-wsl scan` writes to stderr.
#[derive(Debug, PartialEq, Eq)]
pub enum ScanLine {
    /// `home <path>`: the user's home inside the distribution.
    Home(PathBuf),
    /// `progress <files> <dirs> <bytes> <errors>`.
    Progress {
        files: u64,
        dirs: u64,
        bytes: u64,
        errors: u64,
    },
}

impl ScanLine {
    /// Parses one line; anything else (a warning of the distribution) is `None`.
    pub fn parse(line: &str) -> Option<Self> {
        let (word, rest) = line.trim_end().split_once(' ')?;
        match word {
            "home" => Some(ScanLine::Home(PathBuf::from(rest))),
            "progress" => {
                let mut numbers = rest.split(' ').map(str::parse::<u64>);
                let mut next = || numbers.next()?.ok();
                Some(ScanLine::Progress {
                    files: next()?,
                    dirs: next()?,
                    bytes: next()?,
                    errors: next()?,
                })
            }
            _ => None,
        }
    }

    /// The line, as `vano-wsl` writes it.
    pub fn to_line(&self) -> String {
        match self {
            ScanLine::Home(path) => format!("home {}\n", path.display()),
            ScanLine::Progress {
                files,
                dirs,
                bytes,
                errors,
            } => format!("progress {files} {dirs} {bytes} {errors}\n"),
        }
    }
}

/// Runs the Linux program in `distro`, with `args` after it. It runs from the folder that
/// holds it, which `wsl.exe` translates (`--cd`), so nothing is copied into the distribution.
fn spawn_agent(distro: &str, agent: &Path, args: &[&str]) -> Result<Child, String> {
    let folder = agent
        .parent()
        .ok_or_else(|| format!("{} has no folder", agent.display()))?;
    let name = agent
        .file_name()
        .ok_or_else(|| format!("{} has no name", agent.display()))?
        .to_string_lossy();
    let program = format!("./{name}");
    let folder = folder.to_string_lossy();
    let mut all = vec![
        "--distribution",
        distro,
        "--cd",
        &*folder,
        "--exec",
        &*program,
    ];
    all.extend_from_slice(args);
    wsl_command(&all)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run wsl.exe: {error}"))
}

/// Scans `distro` from `/` with the Linux program `agent`, reporting its progress to `scan`.
/// Starts the distribution if it is stopped.
///
/// # Errors
///
/// Why it could not scan: no `wsl.exe`, the program could not run, or its output was not a tree.
pub fn scan(distro: &str, agent: &Path, scan: &ExternalScan) -> Result<Tree, String> {
    let mut child = spawn_agent(distro, agent, &["scan", "/"])?;
    drop(child.stdin.take());
    let stdout = child.stdout.take().ok_or("no output")?;
    let stderr = child.stderr.take().ok_or("no error output")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stdout = stdout;
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<ScanLine>();
    let errors = std::thread::spawn(move || {
        let mut other = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            match ScanLine::parse(&line) {
                Some(parsed) => {
                    let _ = lines_tx.send(parsed);
                }
                None => {
                    other.push_str(&line);
                    other.push('\n');
                }
            }
        }
        other
    });
    let status = loop {
        for line in lines_rx.try_iter() {
            match line {
                ScanLine::Home(home) => scan.set_home(home),
                ScanLine::Progress {
                    files,
                    dirs,
                    bytes,
                    errors,
                } => scan.set_counts(files, dirs, bytes, errors),
            }
        }
        if scan.cancelled() {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(error.to_string()),
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "the output reader stopped".to_string())?
        .map_err(|error| error.to_string())?;
    let other = errors.join().unwrap_or_default();
    if scan.cancelled() {
        return Ok(Tree::new("/"));
    }
    if !status.success() {
        return Err(format!("{status}: {}", other.trim()));
    }
    vano_core::persist::decode(&bytes).map_err(|error| format!("{error:?}"))
}

/// One line of what `vano-wsl delete` writes to stdout.
#[derive(Debug, PartialEq, Eq)]
pub enum DeleteLine {
    /// `progress <files> <bytes>`: files and bytes gone so far, in total.
    Progress(u64, u64),
    /// `gone <index>`: the item at that index is gone.
    Gone(usize),
    /// `failed <index> <reason>`.
    Failed(usize, String),
    /// `stopped <index>`: stopped in the middle of that item.
    Stopped(usize),
}

impl DeleteLine {
    pub fn parse(line: &str) -> Option<Self> {
        let mut parts = line.trim_end().splitn(3, ' ');
        let word = parts.next()?;
        let number = parts.next()?;
        match word {
            "progress" => Some(DeleteLine::Progress(
                number.parse().ok()?,
                parts.next()?.parse().ok()?,
            )),
            "gone" => Some(DeleteLine::Gone(number.parse().ok()?)),
            "stopped" => Some(DeleteLine::Stopped(number.parse().ok()?)),
            "failed" => Some(DeleteLine::Failed(
                number.parse().ok()?,
                parts.next().unwrap_or_default().to_string(),
            )),
            _ => None,
        }
    }

    pub fn to_line(&self) -> String {
        match self {
            DeleteLine::Progress(files, bytes) => format!("progress {files} {bytes}\n"),
            DeleteLine::Gone(index) => format!("gone {index}\n"),
            DeleteLine::Failed(index, reason) => {
                format!("failed {index} {}\n", reason.replace('\n', " "))
            }
            DeleteLine::Stopped(index) => format!("stopped {index}\n"),
        }
    }
}

/// Deletes `paths` (Linux paths) for good inside `distro`, with the Linux program `agent`.
/// `on_line` gets every line of progress. Setting `cancel` asks it to stop before the next file.
///
/// The paths go through standard input, each ended by a NUL, then an empty one: any name works,
/// and there is no command line length limit. Writing `c` afterwards cancels.
///
/// # Errors
///
/// When the program could not run at all.
pub fn delete(
    distro: &str,
    agent: &Path,
    paths: &[String],
    cancel: &AtomicBool,
    mut on_line: impl FnMut(DeleteLine),
) -> Result<(), String> {
    let mut child = spawn_agent(distro, agent, &["delete"])?;
    let mut stdin = child.stdin.take().ok_or("no input")?;
    let mut list = Vec::new();
    for path in paths {
        list.extend_from_slice(path.as_bytes());
        list.push(0);
    }
    list.push(0);
    stdin
        .write_all(&list)
        .and_then(|()| stdin.flush())
        .map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("no output")?;
    let (tx, rx) = std::sync::mpsc::channel::<DeleteLine>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(parsed) = DeleteLine::parse(&line) {
                let _ = tx.send(parsed);
            }
        }
    });
    let mut cancel_sent = false;
    loop {
        for line in rx.try_iter() {
            on_line(line);
        }
        if cancel.load(Ordering::Relaxed) && !cancel_sent {
            let _ = stdin.write_all(b"c").and_then(|()| stdin.flush());
            cancel_sent = true;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(error) => return Err(error.to_string()),
        }
    }
    let _ = reader.join();
    for line in rx.try_iter() {
        on_line(line);
    }
    Ok(())
}

/// The Linux path of a node of a tree scanned inside a distribution, built from the names
/// themselves: on Windows a `PathBuf` would mix `\` in, and a Linux name may hold one.
pub fn linux_path(tree: &Tree, id: vano_core::NodeId) -> String {
    let chain = tree.ancestry(id);
    let mut path = tree
        .root_path()
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();
    for part in chain.iter().skip(1) {
        path.push('/');
        path.push_str(&String::from_utf8_lossy(tree.name_bytes(*part)));
    }
    if path.is_empty() {
        path.push('/');
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn utf16_output_is_decoded() {
        let bytes: Vec<u8> = "Ubuntu\r\nDebian\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            parse_names(&decode_wsl_text(&bytes)),
            vec!["Ubuntu", "Debian"]
        );
    }

    #[test]
    fn utf8_output_is_read_as_is() {
        assert_eq!(
            parse_names(&decode_wsl_text(b"\xef\xbb\xbfUbuntu-24.04\n\n")),
            vec!["Ubuntu-24.04"]
        );
    }

    #[test]
    fn df_gives_the_used_bytes() {
        assert_eq!(parse_df_used("  Used\n92341760\n"), Some(92_341_760));
        assert_eq!(parse_df_used("Used\n"), None);
    }

    #[test]
    fn verbatim_paths_lose_their_prefix() {
        assert_eq!(strip_verbatim(r"\\?\C:\wsl\ubuntu"), r"C:\wsl\ubuntu");
        assert_eq!(strip_verbatim(r"C:\wsl"), r"C:\wsl");
    }

    #[test]
    fn scan_lines_round_trip() {
        for line in [
            ScanLine::Home(PathBuf::from("/home/ana")),
            ScanLine::Progress {
                files: 1,
                dirs: 2,
                bytes: 3,
                errors: 4,
            },
        ] {
            assert_eq!(ScanLine::parse(&line.to_line()), Some(line));
        }
        assert_eq!(ScanLine::parse("wsl: warning"), None);
    }

    #[test]
    fn delete_lines_round_trip() {
        for line in [
            DeleteLine::Progress(12, 4096),
            DeleteLine::Gone(3),
            DeleteLine::Failed(1, "Permission denied (os error 13)".to_string()),
            DeleteLine::Stopped(2),
        ] {
            assert_eq!(DeleteLine::parse(&line.to_line()), Some(line));
        }
    }

    #[test]
    fn docker_distributions_are_told_apart() {
        let distro = |name: &str| Distro {
            name: name.to_string(),
            running: false,
            default: false,
            vhdx: None,
            vhdx_size: 0,
            used_inside: None,
        };
        assert!(distro("docker-desktop").is_docker());
        assert!(distro("docker-desktop-data").is_docker());
        assert!(!distro("Ubuntu").is_docker());
    }

    #[test]
    fn linux_paths_are_built_from_the_names() {
        use vano_core::tree::{EntryData, Kind};
        let mut tree = Tree::new("/");
        let entry = |name: &[u8], kind| EntryData {
            name: name.to_vec(),
            kind,
            size: 0,
            mtime: 0,
            flags: 0,
        };
        tree.set_children(vano_core::ROOT, 1, &[entry(b"home", Kind::Dir)]);
        tree.set_children(1, 2, &[entry(b"back\\slash", Kind::File)]);
        assert_eq!(linux_path(&tree, 2), "/home/back\\slash");
        assert_eq!(linux_path(&tree, vano_core::ROOT), "/");
    }
}
