//! The in-app updater, on top of `telaio-update`: Vano's [`UPDATER`] configuration and the build
//! gate, as Postino's.
//! The checks, downloads, portable detection and installs are `telaio_update::flow`, re-exported
//! here; `views/update.rs` runs the blocking parts on the background executor and renders the
//! result.
//!
//! The feature exists only in builds made with `VANO_UPDATER=github` (the release CI sets it
//! for macOS, Windows and Linux) on a platform `telaio-update` knows. On Linux, a copy the install
//! script installed in a prefix the user can write (`~/.local` by default) downloads and installs
//! updates like macOS and Windows; the other manual installs (the tarball, a script install under
//! `/usr/local`) only announce new versions, and a package managed copy (deb, rpm, AUR) shows
//! nothing. Everything compiles
//! everywhere; [`updater_enabled`] is the one switch the setting row, the palette command and the
//! startup check consult.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use telaio_update::{Platform, UpdateError, UpdaterConfig, flow};

pub use telaio_update::flow::{
    CheckOutcome, CheckStatus, InstallOutcome, ReadyUpdate, STARTUP_DELAY, UpdateState,
    should_check_at_startup,
};

/// Vano's updater: releases of `tanisperez/vano`, `VANO_UPDATE_URL` to test an update between
/// pre-releases (`docs/releasing.md`).
pub const UPDATER: UpdaterConfig = UpdaterConfig {
    owner: "tanisperez",
    repo: "vano",
    app_name: "Vano",
    url_env: "VANO_UPDATE_URL",
    version: CURRENT_VERSION,
};

/// The running version, what a manifest is compared against.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The command that installs Vano on Linux and, run again, updates it. The same as on the
/// website.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://vano.tanis.codes/install.sh | bash";

/// Whether this build has an updater: built with `VANO_UPDATER=github`, running on a platform
/// `telaio-update` knows, and not a Linux copy owned by a package manager. Read once (the
/// executable path is a syscall), so it is cheap in `render`.
pub fn updater_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let exe = std::env::current_exe().ok();
        flow::updater_enabled(option_env!("VANO_UPDATER"))
            && !flow::is_package_managed(Platform::current(), exe.as_deref(), flow::is_sandboxed())
    })
}

/// Whether this copy only announces new versions instead of downloading and installing them:
/// a portable Windows copy, or a Linux copy other than a script install in a writable prefix
/// ([`flow::announces_only`]). Reads the filesystem, so call it once.
pub fn announces_only(platform: Option<Platform>, exe: Option<&Path>) -> bool {
    flow::announces_only(&UPDATER, platform, exe)
}

/// How a copy that downloads nothing ([`ReadyUpdate::is_manual_download`]) gets a new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualUpdate {
    /// A portable Windows copy: the new zip from the release page.
    PortableZip,
    /// A Linux tarball or AppImage: the new one from the release page.
    Download,
    /// Installed by the install script in a prefix that needs root (`/usr/local`): run
    /// [`INSTALL_COMMAND`] again. A script install the user can write updates itself.
    InstallScript,
}

/// The [`ManualUpdate`] of this copy. `install_script` is [`installed_by_script`].
pub fn manual_update(platform: Option<Platform>, install_script: bool) -> ManualUpdate {
    match platform {
        Some(Platform::LinuxX86_64) if install_script => ManualUpdate::InstallScript,
        Some(Platform::LinuxX86_64) => ManualUpdate::Download,
        _ => ManualUpdate::PortableZip,
    }
}

/// Whether the install script installed this executable: it writes the list of its files to
/// `<prefix>/share/vano/installed-files`, and the binary is `<prefix>/bin/vano`. Reads the
/// filesystem, so call it once.
pub fn installed_by_script(exe: &Path) -> bool {
    telaio_update::install::linux_script_prefix(&UPDATER, exe).is_some()
}

/// Where updates are downloaded: `vano/updates` under the OS cache folder.
pub fn download_dir() -> Option<PathBuf> {
    UPDATER.download_dir()
}

/// [`flow::check_and_download`] for Vano. Blocking.
///
/// # Errors
///
/// Whatever `telaio_update` reports.
pub fn check_and_download(platform: Platform, portable: bool) -> Result<CheckOutcome, UpdateError> {
    flow::check_and_download(&UPDATER, platform, portable)
}

/// Records a check that found `outcome` while an update may already be ready (Postino #300).
/// `UpdateState` keeps the first update it finds, so a copy left open across two releases would
/// install the older one: a different version replaces the ready one. Returns whether it did.
pub fn keep_latest(state: &mut UpdateState, outcome: &CheckOutcome) -> bool {
    let CheckOutcome::Ready(found) = outcome else {
        return false;
    };
    if state
        .ready()
        .is_some_and(|ready| ready.version == found.version)
    {
        return false;
    }
    let mut fresh = UpdateState::default();
    fresh.begin_check();
    fresh.finish_check(outcome);
    *state = fresh;
    true
}

/// The update to install: `ready`, unless the manifest now names another version, whose
/// asset is downloaded and returned instead. A failed check keeps `ready`. Blocking.
pub fn latest_ready(platform: Platform, portable: bool, ready: ReadyUpdate) -> ReadyUpdate {
    match check_and_download(platform, portable) {
        Ok(CheckOutcome::Ready(found)) if found.version != ready.version => {
            log::info!(
                "update {} replaced by the newer {}",
                ready.version,
                found.version
            );
            found
        }
        Ok(_) => ready,
        Err(error) => {
            log::warn!("update check before installing failed: {error}");
            ready
        }
    }
}

/// [`flow::start_install`] for Vano. Blocking.
pub fn start_install(platform: Platform, ready: &ReadyUpdate, exe: &Path) -> InstallOutcome {
    flow::start_install(&UPDATER, platform, ready, exe)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn ready(version: &str) -> ReadyUpdate {
        ReadyUpdate {
            version: version.to_string(),
            notes_url: format!("https://example.com/v{version}"),
            path: Some(PathBuf::from(format!("Vano-{version}-setup.exe"))),
        }
    }

    #[test]
    fn a_later_check_replaces_the_ready_update_with_the_latest() {
        let mut state = UpdateState::default();
        assert!(keep_latest(
            &mut state,
            &CheckOutcome::Ready(ready("1.0.0"))
        ));
        assert_eq!(state.ready().map(|r| r.version.as_str()), Some("1.0.0"));
        assert!(!keep_latest(
            &mut state,
            &CheckOutcome::Ready(ready("1.0.0"))
        ));
        assert!(!keep_latest(&mut state, &CheckOutcome::UpToDate));
        assert_eq!(state.ready().map(|r| r.version.as_str()), Some("1.0.0"));
        assert!(keep_latest(
            &mut state,
            &CheckOutcome::Ready(ready("1.0.1"))
        ));
        assert_eq!(state.ready().map(|r| r.version.as_str()), Some("1.0.1"));
        assert!(state.begin_install().is_some());
    }

    #[test]
    fn linux_copies_update_by_hand_or_with_the_script() {
        let linux = Some(Platform::LinuxX86_64);
        assert_eq!(manual_update(linux, true), ManualUpdate::InstallScript);
        assert_eq!(manual_update(linux, false), ManualUpdate::Download);
        let windows = Some(Platform::WindowsX86_64);
        assert_eq!(manual_update(windows, false), ManualUpdate::PortableZip);
    }

    #[test]
    fn the_script_install_is_told_by_its_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin dir");
        let exe = bin.join("vano");
        std::fs::write(&exe, "").expect("binary");
        assert!(!installed_by_script(&exe));
        let share = dir.path().join("share/vano");
        std::fs::create_dir_all(&share).expect("share dir");
        std::fs::write(share.join("installed-files"), "# Vano 1.0.0\n").expect("manifest");
        assert!(installed_by_script(&exe));
        assert!(!installed_by_script(&dir.path().join("vano")));
    }

    #[test]
    fn the_manifest_is_vano_s_latest_release() {
        assert_eq!(
            UPDATER.latest_url(),
            "https://github.com/tanisperez/vano/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn downloads_go_under_the_vano_cache_dir() {
        if let Some(dir) = download_dir() {
            assert!(dir.ends_with("vano/updates"));
        }
    }
}
