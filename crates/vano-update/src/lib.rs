//! Checking for, downloading and installing new Vano versions.
//!
//! Standalone: no other vano crate is needed. Every call here blocks, so the app must run
//! them on a background executor. The flow is [`check`] against [`manifest_url`], then
//! [`download`] of the asset for the current [`Platform`], then the helpers in [`install`].
#![warn(missing_docs)]

mod error;
pub mod install;
mod manifest;

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

pub use error::UpdateError;

/// Where the manifest of the latest stable release is published.
pub const LATEST_URL: &str =
    "https://github.com/tanisperez/vano/releases/latest/download/latest.json";

/// Runtime override of [`LATEST_URL`], to test updates between pre-releases (`releases/latest`
/// skips them).
pub const URL_ENV: &str = "VANO_UPDATE_URL";

/// Largest manifest accepted.
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// The manifest URL: [`URL_ENV`] when set and not empty, otherwise [`LATEST_URL`].
#[must_use]
pub fn manifest_url() -> String {
    match std::env::var(URL_ENV) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => LATEST_URL.to_string(),
    }
}

/// A platform that has an in-app updater.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS on Apple silicon.
    MacosAarch64,
    /// Windows on x86_64.
    WindowsX86_64,
}

impl Platform {
    /// The platform this binary runs on, or `None` where there is no updater (Linux, macOS
    /// x86_64, ...).
    #[must_use]
    pub fn current() -> Option<Platform> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Platform::MacosAarch64)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Platform::WindowsX86_64)
        } else {
            None
        }
    }

    /// The key of this platform in the manifest.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Platform::MacosAarch64 => "macos-aarch64",
            Platform::WindowsX86_64 => "windows-x86_64",
        }
    }
}

/// A newer version, with the asset to install it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    /// The new version.
    pub version: semver::Version,
    /// The release page, with the notes.
    pub notes_url: String,
    /// The asset URL for the requested platform.
    pub url: String,
    /// The SHA-256 of the asset, lowercase hex.
    pub sha256: String,
}

fn agent(global_timeout: Duration) -> ureq::Agent {
    // The OS certificate store, not ureq's bundled Mozilla roots: a corporate network that
    // inspects TLS re-signs github.com with a company CA that IT installs only there (#111).
    let tls_config = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    // Redirects stay on (ureq's default): GitHub redirects release assets to another host.
    ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(global_timeout))
        .max_redirects(10)
        .build()
        .new_agent()
}

fn map_ureq(error: ureq::Error) -> UpdateError {
    match error {
        ureq::Error::StatusCode(code) => UpdateError::Status(code),
        other => UpdateError::Network(other.to_string()),
    }
}

/// Fetches the manifest and returns an [`Update`] only when its version is greater than
/// `current` (semver precedence, so `0.1.0-rc.3 < 0.1.0-rc.4 < 0.1.0`).
///
/// A newer manifest without an entry for `platform` is `Ok(None)` and a warning: there is
/// nothing to install.
///
/// # Errors
///
/// Network and HTTP status failures, a manifest that is too big or does not parse, and a
/// version that is not valid semver.
pub fn check(
    manifest_url: &str,
    current: &str,
    platform: Platform,
) -> Result<Option<Update>, UpdateError> {
    let current = parse_version(current)?;
    let mut response = agent(Duration::from_secs(30))
        .get(manifest_url)
        .call()
        .map_err(map_ureq)?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_MANIFEST_BYTES)
        .read_to_vec()
        .map_err(map_ureq)?;
    let manifest: manifest::Manifest =
        serde_json::from_slice(&bytes).map_err(|error| UpdateError::Parse(error.to_string()))?;
    let version = parse_version(&manifest.version)?;
    log::debug!("update manifest: version {version}, current {current}");
    if version <= current {
        return Ok(None);
    }
    let Some(asset) = manifest.platforms.get(platform.key()) else {
        log::warn!("version {version} has no asset for {}", platform.key());
        return Ok(None);
    };
    Ok(Some(Update {
        version,
        notes_url: manifest.notes_url,
        url: asset.url.clone(),
        sha256: asset.sha256.trim().to_ascii_lowercase(),
    }))
}

fn parse_version(text: &str) -> Result<semver::Version, UpdateError> {
    let trimmed = text.trim();
    semver::Version::parse(trimmed.strip_prefix('v').unwrap_or(trimmed))
        .map_err(|_| UpdateError::BadVersion(text.to_string()))
}

/// The last path segment of `url`, without query or fragment. Refuses anything that could
/// escape the download directory.
fn file_name(url: &str) -> Result<String, UpdateError> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.is_empty() || name == "." || name == ".." || name.contains('\\') || name.contains('\0')
    {
        return Err(UpdateError::BadAssetName(url.to_string()));
    }
    Ok(name.to_string())
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Downloads `update.url` into `dir` (created if missing), named after the last segment of the
/// URL. The body is streamed to a `.part` file while hashed, checked against
/// [`Update::sha256`] and renamed. Returns the final path.
///
/// A final file that already exists with the right hash is returned without downloading.
/// On a mismatch the partial file is deleted.
///
/// # Errors
///
/// Network and HTTP status failures, [`UpdateError::Checksum`] and IO failures.
pub fn download(update: &Update, dir: &Path) -> Result<PathBuf, UpdateError> {
    let expected = update.sha256.trim().to_ascii_lowercase();
    fs::create_dir_all(dir)?;
    let target = dir.join(file_name(&update.url)?);
    if target.is_file() && hash_file(&target).is_ok_and(|actual| actual == expected) {
        log::debug!("update already downloaded");
        return Ok(target);
    }
    let mut part_name = target.as_os_str().to_os_string();
    part_name.push(".part");
    let part = PathBuf::from(part_name);

    let result = stream_to(update, &part, &expected);
    match result {
        Ok(()) => {
            fs::rename(&part, &target)?;
            Ok(target)
        }
        Err(error) => {
            let _ = fs::remove_file(&part);
            Err(error)
        }
    }
}

fn stream_to(update: &Update, part: &Path, expected: &str) -> Result<(), UpdateError> {
    let mut response = agent(Duration::from_secs(30 * 60))
        .get(&update.url)
        .call()
        .map_err(map_ureq)?;
    let mut reader = response.body_mut().with_config().limit(1 << 30).reader();
    let mut file = File::create(part)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| UpdateError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
    }
    file.flush()?;
    drop(file);
    let actual = hex(&hasher.finalize());
    if actual != expected {
        return Err(UpdateError::Checksum {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
