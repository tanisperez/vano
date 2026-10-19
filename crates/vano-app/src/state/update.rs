//! The in-app updater (#45), on top of `telaio-update`: Vano's [`UPDATER`] configuration and the
//! build gate. The checks, downloads, portable detection and installs are
//! `telaio_update::flow`, re-exported here; `views/update.rs` runs the blocking parts on the
//! background executor and renders the result.
//!
//! The feature exists only in builds made with `VANO_UPDATER=github` (the release CI sets it
//! for macOS and Windows) on a platform `telaio-update` knows. Everything compiles everywhere;
//! [`updater_enabled`] is the one switch the setting row, the palette command and the startup
//! check consult.

use std::path::{Path, PathBuf};

use telaio_update::{Platform, UpdateError, UpdaterConfig, flow};

pub use telaio_update::flow::{
    CheckOutcome, CheckStatus, InstallOutcome, ReadyUpdate, STARTUP_DELAY, UpdateState,
    is_portable, should_check_at_startup,
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

/// Whether this build has an updater: built with `VANO_UPDATER=github` and running on a platform
/// with an installable asset.
pub fn updater_enabled() -> bool {
    flow::updater_enabled(option_env!("VANO_UPDATER"))
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

/// [`flow::start_install`] for Vano. Blocking.
pub fn start_install(platform: Platform, ready: &ReadyUpdate, exe: &Path) -> InstallOutcome {
    flow::start_install(&UPDATER, platform, ready, exe)
}

#[cfg(test)]
mod tests {
    use super::*;

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
