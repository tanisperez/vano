//! Text for numbers the UI shows: sizes, paths, durations and ages, in the active language.

use std::path::Path;

use rust_i18n::t;
use vano_core::size::split;

use super::locale::plural_key;
use super::number::{format_decimal, format_integer};

/// The translation of a key built at runtime (`rules.<id>.name`).
pub fn tr(key: &str) -> String {
    t!(key).into_owned()
}

/// A byte count as the UI shows it: `13.4 GB` (`13,4 GB` in Spanish).
pub fn size(bytes: u64) -> String {
    let (value, unit) = size_parts(bytes);
    format!("{value} {unit}")
}

/// [`size`] split into its number and its unit, for the big stat values that draw the unit
/// smaller.
pub fn size_parts(bytes: u64) -> (String, &'static str) {
    let (value, unit, decimals) = split(bytes);
    (format_decimal(value, decimals), unit.symbol())
}

/// A size estimate: `~9.0 GB`.
pub fn approx_size(bytes: u64) -> String {
    format!("~{}", size(bytes))
}

/// Bytes per second: `12.2 GB/s`.
pub fn throughput(bytes: u64, seconds: f64) -> String {
    if seconds < 0.01 {
        return size(0) + "/s";
    }
    format!("{}/s", size((bytes as f64 / seconds) as u64))
}

/// Seconds with one decimal: `7.0 s`.
pub fn seconds(value: f64) -> String {
    format!("{} s", format_decimal(value, 1))
}

/// A count with the language's grouping: `1,240,312`.
pub fn count(value: u64) -> String {
    format_integer(value)
}

/// A count and its noun: `5 items`, `1 file`. `key` names the two plural keys
/// (`<key>.one`, `<key>.other`), whose text takes the number as `%{n}`.
pub fn counted(key: &str, value: u64) -> String {
    let key = plural_key(key, usize::try_from(value).unwrap_or(usize::MAX));
    t!(key.as_str(), n = format_integer(value)).into_owned()
}

/// `path` with the home folder written as `~`, the way the design shows paths.
pub fn short_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix(home)
    {
        if rest.as_os_str().is_empty() {
            return "~".to_string();
        }
        return format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
    }
    path.display().to_string()
}

/// How long ago `then` was, seen from `now` (both Unix seconds): `Today`, `Yesterday`,
/// `3 days ago`, `2 weeks ago`, `4 months ago`, `2 years ago`.
pub fn age(then: u64, now: u64) -> String {
    let days = now.saturating_sub(then) / 86_400;
    match days {
        0 => t!("format.today").into_owned(),
        1 => t!("format.yesterday").into_owned(),
        2..=13 => counted("format.days_ago", days),
        14..=59 => counted("format.weeks_ago", days / 7),
        60..=729 => counted("format.months_ago", days / 30),
        _ => counted("format.years_ago", days / 365),
    }
}

/// The current time in Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn short_path_uses_tilde() {
        let home = PathBuf::from("/home/u");
        assert_eq!(short_path(Path::new("/home/u"), Some(&home)), "~");
        assert_eq!(short_path(Path::new("/home/u/.npm"), Some(&home)), "~/.npm");
        assert_eq!(short_path(Path::new("/var/log"), Some(&home)), "/var/log");
    }
}
