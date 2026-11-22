//! What Settings, "About" shows about the running build: version, platform, executable path and
//! the release links. Computed once (the executable path is a syscall), never in `render`.

use std::path::PathBuf;

use telaio_update::Platform;

use super::update::{CURRENT_VERSION, announces_only, download_dir, installed_by_script};

/// Who Vano is, for `telaio-app`. Defined with the command line, which needs it without gpui.
pub use vano_cli::IDENTITY;

/// The install page of the website, with the apt, dnf and AUR instructions.
pub const INSTALL_URL: &str = "https://vano.tanis.codes/install.html";

/// The OS and architecture as shown to the user, for example "Linux x86_64" or "macOS arm64".
pub fn platform_label(os: &str, arch: &str) -> String {
    let (os, arch) = match (os, arch) {
        ("macos", "aarch64") => ("macOS", "arm64"),
        ("macos", arch) => ("macOS", arch),
        ("linux", arch) => ("Linux", arch),
        ("windows", arch) => ("Windows", arch),
        (os, arch) => (os, arch),
    };
    format!("{os} {arch}")
}

/// The facts "About" shows, gathered once.
#[derive(Debug, Clone)]
pub struct AboutInfo {
    /// The running version.
    pub version: &'static str,
    /// For example "Linux x86_64".
    pub platform: String,
    /// The running executable, `None` when the OS cannot tell.
    pub exe: Option<PathBuf>,
    /// Where updates are downloaded, `None` when the cache directory is unknown or this is a
    /// portable copy, which downloads nothing.
    pub download_dir: Option<PathBuf>,
    /// Whether this copy only announces updates ([`announces_only`]): a portable Windows copy,
    /// told by the files next to the exe, or a Linux copy other than a script install the user
    /// can write. Read here once and never in `render`.
    pub portable: bool,
    /// Whether the Linux install script installed this copy ([`installed_by_script`]).
    pub install_script: bool,
}

impl AboutInfo {
    /// Reads the build and the environment.
    pub fn detect() -> Self {
        let exe = std::env::current_exe()
            .ok()
            .map(|exe| dunce::canonicalize(&exe).unwrap_or(exe));
        let platform = Platform::current();
        let portable = announces_only(platform, exe.as_deref());
        let install_script = platform == Some(Platform::LinuxX86_64)
            && exe.as_deref().is_some_and(installed_by_script);
        Self {
            version: CURRENT_VERSION,
            platform: platform_label(std::env::consts::OS, std::env::consts::ARCH),
            exe,
            download_dir: download_dir().filter(|_| !portable),
            portable,
            install_script,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn the_platform_is_readable() {
        assert_eq!(platform_label("linux", "x86_64"), "Linux x86_64");
        assert_eq!(platform_label("macos", "aarch64"), "macOS arm64");
        assert_eq!(platform_label("macos", "x86_64"), "macOS x86_64");
        assert_eq!(platform_label("windows", "x86_64"), "Windows x86_64");
        assert_eq!(platform_label("freebsd", "x86_64"), "freebsd x86_64");
    }

    #[test]
    fn detect_reports_the_running_version() {
        assert_eq!(AboutInfo::detect().version, env!("CARGO_PKG_VERSION"));
    }
}
