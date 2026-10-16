//! The persistent log file (GitHub #36): a small logger behind the `log` facade, which gpui,
//! gpui-component, wgpu, ureq and rustls already log through, so installing it here also collects
//! their messages.
//!
//! The file lives where each OS expects application logs (see [`log_dir`]) and rotates by size:
//! once `vano.log` would grow past [`MAX_FILE_SIZE`] it becomes `vano.1.log`, the previous
//! `vano.1.log` becomes `vano.2.log`, and so on, keeping at most [`MAX_FILES`] files.
//!
//! The level comes from Settings, "Advanced" (`state::settings::LogLevel`, Info by default),
//! and the [`ENV_VAR`] environment variable overrides it at startup. Crates other than Vano's
//! own and gpui's are capped at Warn, or Info at Trace, so their internals do not drown the
//! app's own messages: at Debug, `naga` alone writes about 20 MB of lines while compiling gpui's
//! shaders at startup, and the HTTP stack's Debug and Trace output would include header values
//! this app never logs itself.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::state::settings::LogLevel;

/// The name of the current log file.
pub const LOG_FILE_NAME: &str = "vano.log";

/// Size at which the current log file is rotated.
pub const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Most log files kept: the current one plus `vano.1.log` to `vano.9.log`.
pub const MAX_FILES: usize = 10;

/// Environment variable that overrides the Settings log level at startup, for example
/// `VANO_LOG=debug`.
pub const ENV_VAR: &str = "VANO_LOG";

/// Target prefixes logged at the chosen level. Every other target is capped at Warn, or at Info
/// when the level is Trace.
const FULL_LEVEL_TARGETS: [&str; 2] = ["vano", "gpui"];

/// The folder of the log files, following each OS convention:
///
/// - Linux and other Unix: `$XDG_STATE_HOME/vano` (`~/.local/state/vano` by default). The
///   XDG spec reserves the state directory for logs, unlike the config or cache ones.
/// - macOS: `~/Library/Logs/Vano`, which Console.app also lists.
/// - Windows: `%LOCALAPPDATA%\Vano\logs`, local to the machine rather than roaming.
///
/// `None` when the OS does not report the base directory.
pub fn log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| home.join("Library").join("Logs").join("Vano"))
    }
    #[cfg(windows)]
    {
        dirs::data_local_dir().map(|base| base.join("Vano").join("logs"))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        dirs::state_dir().map(|base| base.join("vano"))
    }
}

/// The path of the current log file, `None` when [`log_dir`] is unknown.
pub fn log_path() -> Option<PathBuf> {
    log_dir().map(|dir| dir.join(LOG_FILE_NAME))
}

/// The level to start with: [`ENV_VAR`] when it holds a valid level, `setting` otherwise.
pub fn startup_level(setting: LogLevel, env: Option<&str>) -> LogLevel {
    env.and_then(LogLevel::parse).unwrap_or(setting)
}

/// Installs the logger at `level` and a panic hook that writes panics to it. Returns the path
/// of the log file, or `None` when it could not be opened, in which case messages only go to
/// stderr. Call once, before anything else logs.
pub fn init(level: LevelFilter) -> Option<PathBuf> {
    let path = log_path();
    let file =
        path.as_ref().and_then(
            |path| match RotatingFile::open(path, MAX_FILE_SIZE, MAX_FILES) {
                Ok(file) => Some(file),
                Err(error) => {
                    eprintln!("vano: could not open {}: {error}", path.display());
                    None
                }
            },
        );
    let opened = file.as_ref().and(path);
    let logger = Logger {
        // A development build also echoes to the terminal it was launched from.
        stderr: cfg!(debug_assertions) || file.is_none(),
        file: file.map(Mutex::new),
    };
    if log::set_logger(Box::leak(Box::new(logger))).is_ok() {
        log::set_max_level(level);
    }
    install_panic_hook();
    opened
}

/// Changes the level live, from the Settings view.
pub fn set_level(level: LevelFilter) {
    log::set_max_level(level);
}

/// Logs every panic, with its backtrace, before the default hook prints it. A panic usually
/// closes the app, so this is often the only trace left of it.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {info}\n{backtrace}");
        previous(info);
    }));
}

/// Whether a message of `level` from `target` is written when the level is `max`.
fn target_allowed(target: &str, level: Level, max: LevelFilter) -> bool {
    if level > max {
        return false;
    }
    let third_party_cap = if max == LevelFilter::Trace {
        Level::Info
    } else {
        Level::Warn
    };
    level <= third_party_cap
        || FULL_LEVEL_TARGETS
            .iter()
            .any(|prefix| target.starts_with(prefix))
}

/// One log line: local time with milliseconds and UTC offset, level, target and message.
fn format_line(now: chrono::DateTime<chrono::Local>, record: &Record) -> String {
    format!(
        "{} {:<5} {}: {}\n",
        now.format("%Y-%m-%dT%H:%M:%S%.3f%:z"),
        record.level(),
        record.target(),
        record.args()
    )
}

/// The installed logger.
struct Logger {
    file: Option<Mutex<RotatingFile>>,
    stderr: bool,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        target_allowed(metadata.target(), metadata.level(), log::max_level())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format_line(chrono::Local::now(), record);
        if let Some(file) = &self.file {
            // A panic while holding the lock leaves the file itself usable.
            let mut file = file.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            // Nowhere left to report a failure to write the log itself.
            let _ = file.write_line(&line);
        }
        if self.stderr {
            let _ = io::stderr().write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

/// A log file that rotates by size, keeping at most `max_files` files. Every line is written
/// straight to the file, unbuffered, so nothing is lost when the app crashes.
struct RotatingFile {
    path: PathBuf,
    file: Option<File>,
    size: u64,
    max_size: u64,
    max_files: usize,
}

impl RotatingFile {
    /// Opens `path` for appending, creating it and its folder if needed.
    fn open(path: &Path, max_size: u64, max_files: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len();
        Ok(RotatingFile {
            path: path.to_path_buf(),
            file: Some(file),
            size,
            max_size,
            max_files,
        })
    }

    /// Appends `line`, rotating first when it would push the file past `max_size`. A single
    /// line longer than `max_size` is still written whole, to a fresh file.
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let len = line.len() as u64;
        if self.size > 0 && self.size + len > self.max_size {
            self.rotate()?;
        }
        if self.file.is_none() {
            // A previous rotation failed halfway: try again to have a file to write to.
            *self = RotatingFile::open(&self.path, self.max_size, self.max_files)?;
        }
        if let Some(file) = self.file.as_mut() {
            file.write_all(line.as_bytes())?;
            self.size += len;
        }
        Ok(())
    }

    /// Shifts every file one number up, dropping the oldest, and starts an empty current file.
    fn rotate(&mut self) -> io::Result<()> {
        // Closed first: Windows does not rename a file that is still open.
        self.file = None;
        for index in (1..self.max_files).rev() {
            let from = rotated_path(&self.path, index - 1);
            if from.exists() {
                fs::rename(&from, rotated_path(&self.path, index))?;
            }
        }
        // Truncates the current file when nothing was renamed away (`max_files` of 1).
        self.file = Some(File::create(&self.path)?);
        self.size = 0;
        Ok(())
    }
}

/// `vano.log` for `index` 0, `vano.<index>.log` otherwise.
fn rotated_path(path: &Path, index: usize) -> PathBuf {
    if index == 0 {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match path.extension() {
        Some(extension) => format!("{stem}.{index}.{}", extension.to_string_lossy()),
        None => format!("{stem}.{index}"),
    };
    path.with_file_name(name)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn third_party_targets_are_capped_at_warn() {
        let max = LevelFilter::Debug;
        assert!(target_allowed("vano_app::views", Level::Debug, max));
        assert!(target_allowed(
            "gpui_pre_wgpu::wgpu_context",
            Level::Info,
            max
        ));
        assert!(!target_allowed("wgpu_core::device", Level::Info, max));
        assert!(target_allowed("wgpu_core::device", Level::Warn, max));
        assert!(!target_allowed("vano_app", Level::Trace, max));
    }

    #[test]
    fn trace_raises_the_third_party_cap_to_info_only() {
        let max = LevelFilter::Trace;
        assert!(target_allowed("vano_app::views", Level::Trace, max));
        assert!(target_allowed("gpui_pre::window", Level::Trace, max));
        assert!(target_allowed("wgpu_hal::vulkan", Level::Info, max));
        assert!(!target_allowed("naga::front", Level::Debug, max));
        assert!(!target_allowed("ureq_proto::client", Level::Trace, max));
    }

    #[test]
    fn off_lets_nothing_through() {
        assert!(!target_allowed("vano_app", Level::Error, LevelFilter::Off));
    }

    #[test]
    fn env_var_overrides_the_setting_only_when_valid() {
        assert_eq!(
            startup_level(LogLevel::Info, Some("DEBUG")),
            LogLevel::Debug
        );
        assert_eq!(startup_level(LogLevel::Warn, Some("loud")), LogLevel::Warn);
        assert_eq!(startup_level(LogLevel::Warn, None), LogLevel::Warn);
    }

    #[test]
    fn lines_carry_time_level_target_and_message() {
        let now = chrono::Local::now();
        let line = format_line(
            now,
            &Record::builder()
                .level(Level::Info)
                .target("vano_app::views::send")
                .args(format_args!("GET https://example.com 200"))
                .build(),
        );
        assert!(line.ends_with(" INFO  vano_app::views::send: GET https://example.com 200\n"));
        assert!(
            line.starts_with(&now.format("%Y-%m-%dT").to_string()),
            "{line}"
        );
    }

    #[test]
    fn rotated_paths_number_the_files() {
        let path = Path::new("/logs/vano.log");
        assert_eq!(rotated_path(path, 0), PathBuf::from("/logs/vano.log"));
        assert_eq!(rotated_path(path, 3), PathBuf::from("/logs/vano.3.log"));
    }

    #[test]
    fn appends_to_an_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("logs").join(LOG_FILE_NAME);
        RotatingFile::open(&path, 100, 3)
            .expect("open")
            .write_line("first\n")
            .expect("write");
        RotatingFile::open(&path, 100, 3)
            .expect("reopen")
            .write_line("second\n")
            .expect("write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "first\nsecond\n");
    }

    #[test]
    fn rotates_by_size_and_keeps_at_most_max_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(LOG_FILE_NAME);
        let mut file = RotatingFile::open(&path, 10, 3).expect("open");
        // Six 6-byte lines, at most one per 10-byte file: six files written, three kept.
        for index in 0..6 {
            file.write_line(&format!("line {index}\n")).expect("write");
        }

        let mut names: Vec<String> = fs::read_dir(dir.path())
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names, vec!["vano.1.log", "vano.2.log", "vano.log"]);
        let read = |name: &str| fs::read_to_string(dir.path().join(name)).expect("read");
        assert_eq!(read("vano.log"), "line 5\n");
        assert_eq!(read("vano.1.log"), "line 4\n");
        assert_eq!(read("vano.2.log"), "line 3\n");
    }

    #[test]
    fn a_line_longer_than_the_limit_is_written_whole() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(LOG_FILE_NAME);
        let mut file = RotatingFile::open(&path, 4, 2).expect("open");
        file.write_line("a long line\n").expect("write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "a long line\n");
    }

    #[test]
    fn a_single_file_limit_truncates_instead_of_rotating() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(LOG_FILE_NAME);
        let mut file = RotatingFile::open(&path, 10, 1).expect("open");
        file.write_line("line 0\n").expect("write");
        file.write_line("line 1\n").expect("write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "line 1\n");
        assert_eq!(fs::read_dir(dir.path()).expect("read dir").count(), 1);
    }
}
