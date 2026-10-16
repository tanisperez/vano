//! Installing a downloaded update. Plain logic with no OS gating, so it is tested on Linux.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::UpdateError;

/// Name of the folder, next to the bundle, where the new version is unpacked.
const STAGING_DIR: &str = ".vano-update";

/// Name of the uninstaller the Inno Setup installer leaves next to `vano.exe`.
const WINDOWS_UNINSTALLER: &str = "unins000.exe";

/// The arguments the Windows installer is started with.
#[must_use]
pub fn windows_installer_args() -> [&'static str; 4] {
    [
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/relaunch=1",
    ]
}

/// Windows: whether `exe` runs from a copy made by the installer, which always leaves its
/// uninstaller next to the program. A portable copy (the zip) has none and must not be updated by
/// running the installer, which would add a second copy somewhere else.
#[must_use]
pub fn windows_is_installed(exe: &Path) -> bool {
    exe.parent()
        .is_some_and(|dir| dir.join(WINDOWS_UNINSTALLER).is_file())
}

/// Windows: starts the downloaded installer silently and returns. The caller quits right after.
///
/// # Errors
///
/// When the installer cannot be started.
pub fn spawn_windows_installer(installer: &Path) -> io::Result<()> {
    Command::new(installer)
        .args(windows_installer_args())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// macOS: the `.app` folder that contains `exe` (`.../Vano.app` for
/// `.../Vano.app/Contents/MacOS/vano`), or `None` when it is not inside a bundle.
#[must_use]
pub fn macos_bundle_from_exe(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .skip(1)
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

/// macOS: unpacks `archive` (`.app.tar.gz`) with the system `tar` into a staging folder next to
/// `bundle` (same volume, so the swap is a rename) and returns the staged `Vano.app`.
///
/// # Errors
///
/// An [`UpdateError::Io`] of kind `PermissionDenied` when the folder that holds the bundle is
/// not writable (the app then falls back to opening the release page), or any other IO failure,
/// including `tar` failing or the archive not containing `Vano.app`.
pub fn macos_stage(archive: &Path, bundle: &Path) -> Result<PathBuf, UpdateError> {
    let parent = bundle
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the bundle has no parent"))?;
    let staging = parent.join(STAGING_DIR);
    match std::fs::remove_dir_all(&staging) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    std::fs::create_dir(&staging)?;
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("tar failed: {}", detail.trim())).into());
    }
    let staged = staging.join("Vano.app");
    if !staged.is_dir() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the archive does not contain Vano.app",
        )
        .into());
    }
    Ok(staged)
}

/// Quotes `text` for `sh` with single quotes, so any character is safe.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The script that swaps the bundle once the app has quit: waits for `pid` to exit, moves the old
/// bundle aside into the staging folder (the folder of `staged`), moves `staged` into place and
/// deletes the staging folder. If the new bundle cannot be moved in, the old one is restored.
/// Finally runs `reopen` with the bundle path as its last argument (`open` on macOS).
#[must_use]
pub fn macos_swap_script(pid: u32, bundle: &Path, staged: &Path, reopen: &str) -> String {
    let staging = staged.parent().unwrap_or(staged);
    let backup = staging.join("Vano.old.app");
    let bundle = quote(&bundle.to_string_lossy());
    let staged = quote(&staged.to_string_lossy());
    let staging = quote(&staging.to_string_lossy());
    let backup = quote(&backup.to_string_lossy());
    format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done\n\
         if mv {bundle} {backup}; then\n\
         \x20 if mv {staged} {bundle}; then\n\
         \x20   rm -rf {staging}\n\
         \x20 else\n\
         \x20   mv {backup} {bundle}\n\
         \x20 fi\n\
         fi\n\
         {reopen} {bundle}\n"
    )
}

/// Runs `script` with `/bin/sh -c`, detached (own process group, no stdio), and returns. The
/// caller quits right after.
///
/// # Errors
///
/// When the shell cannot be started.
pub fn spawn_detached_script(script: &str) -> io::Result<()> {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map(drop)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn windows_args() {
        assert_eq!(
            windows_installer_args(),
            [
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/relaunch=1"
            ]
        );
    }

    #[test]
    fn windows_installed_copy_has_its_uninstaller_next_to_the_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("vano.exe");
        fs::write(&exe, "exe").unwrap();
        assert!(!windows_is_installed(&exe));
        fs::write(dir.path().join(WINDOWS_UNINSTALLER), "uninstaller").unwrap();
        assert!(windows_is_installed(&exe));
    }

    #[test]
    fn windows_uninstaller_elsewhere_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(WINDOWS_UNINSTALLER), "uninstaller").unwrap();
        let portable = dir.path().join("portable");
        fs::create_dir(&portable).unwrap();
        assert!(!windows_is_installed(&portable.join("vano.exe")));
        // A folder with that name is not an uninstaller.
        let fake = dir.path().join("fake");
        fs::create_dir_all(fake.join(WINDOWS_UNINSTALLER)).unwrap();
        assert!(!windows_is_installed(&fake.join("vano.exe")));
        assert!(!windows_is_installed(Path::new("")));
    }

    #[test]
    fn bundle_from_exe() {
        let exe = Path::new("/Applications/Vano.app/Contents/MacOS/vano");
        assert_eq!(
            macos_bundle_from_exe(exe),
            Some(PathBuf::from("/Applications/Vano.app"))
        );
        assert_eq!(macos_bundle_from_exe(Path::new("/usr/bin/vano")), None);
        assert_eq!(macos_bundle_from_exe(Path::new("/x/Vano.app")), None);
    }

    #[test]
    fn quoting() {
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
    }

    fn make_bundle(path: &Path, marker: &str) {
        fs::create_dir_all(path.join("Contents")).unwrap();
        fs::write(path.join("Contents/marker"), marker).unwrap();
    }

    fn exited_pid() -> u32 {
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    fn run_script(script: &str) {
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn swap_script_replaces_the_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("my apps it's");
        let bundle = apps.join("Vano.app");
        make_bundle(&bundle, "old");
        let staging = apps.join(STAGING_DIR);
        let staged = staging.join("Vano.app");
        make_bundle(&staged, "new");
        let reopened = dir.path().join("reopened file");
        let reopen = format!("printf '%s\\n' > {}", quote(&reopened.to_string_lossy()));

        run_script(&macos_swap_script(exited_pid(), &bundle, &staged, &reopen));

        assert_eq!(
            fs::read_to_string(bundle.join("Contents/marker")).unwrap(),
            "new"
        );
        assert!(!staging.exists());
        assert_eq!(
            fs::read_to_string(reopened).unwrap().trim(),
            bundle.to_string_lossy()
        );
    }

    #[test]
    fn swap_script_restores_the_old_bundle_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Vano.app");
        make_bundle(&bundle, "old");
        let staging = dir.path().join(STAGING_DIR);
        fs::create_dir(&staging).unwrap();
        let staged = staging.join("Vano.app"); // does not exist, so the second move fails
        let reopened = dir.path().join("reopened");
        let reopen = format!("printf '%s\\n' > {}", quote(&reopened.to_string_lossy()));

        run_script(&macos_swap_script(exited_pid(), &bundle, &staged, &reopen));

        assert_eq!(
            fs::read_to_string(bundle.join("Contents/marker")).unwrap(),
            "old"
        );
        assert!(reopened.exists());
    }

    #[test]
    fn swap_script_waits_for_the_process() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Vano.app");
        make_bundle(&bundle, "old");
        let staging = dir.path().join(STAGING_DIR);
        let staged = staging.join("Vano.app");
        make_bundle(&staged, "new");
        let mut sleeper = Command::new("sleep").arg("1").spawn().unwrap();
        let script = macos_swap_script(sleeper.id(), &bundle, &staged, "true");
        let mut shell = Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            fs::read_to_string(bundle.join("Contents/marker")).unwrap(),
            "old"
        );
        sleeper.wait().unwrap();
        assert!(shell.wait().unwrap().success());
        assert_eq!(
            fs::read_to_string(bundle.join("Contents/marker")).unwrap(),
            "new"
        );
    }

    fn make_archive(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        make_bundle(&src.join("Vano.app"), "new");
        let archive = dir.join("Vano.app.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&src)
            .arg("Vano.app")
            .status()
            .unwrap();
        assert!(status.success());
        archive
    }

    #[test]
    fn stage_unpacks_next_to_the_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let archive = make_archive(dir.path());
        let apps = dir.path().join("apps");
        fs::create_dir(&apps).unwrap();
        // A leftover from a previous attempt is wiped.
        fs::create_dir_all(apps.join(STAGING_DIR)).unwrap();
        fs::write(apps.join(STAGING_DIR).join("junk"), "x").unwrap();

        let staged = macos_stage(&archive, &apps.join("Vano.app")).unwrap();

        assert_eq!(staged, apps.join(STAGING_DIR).join("Vano.app"));
        assert_eq!(
            fs::read_to_string(staged.join("Contents/marker")).unwrap(),
            "new"
        );
        assert!(!apps.join(STAGING_DIR).join("junk").exists());
    }

    #[test]
    fn stage_fails_without_the_app_in_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = make_archive(dir.path());
        // Re-pack with another name inside.
        let other = dir.path().join("other");
        fs::create_dir_all(other.join("Other.app")).unwrap();
        fs::write(other.join("Other.app/x"), "x").unwrap();
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&other)
            .arg("Other.app")
            .status()
            .unwrap();
        assert!(status.success());
        let apps = dir.path().join("apps");
        fs::create_dir(&apps).unwrap();
        assert!(macos_stage(&archive, &apps.join("Vano.app")).is_err());
        assert!(!apps.join(STAGING_DIR).exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage_reports_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let archive = make_archive(dir.path());
        let apps = dir.path().join("apps");
        fs::create_dir(&apps).unwrap();
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o555)).unwrap();
        let result = macos_stage(&archive, &apps.join("Vano.app"));
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o755)).unwrap();
        match result {
            // Running as root ignores the permissions: nothing to check.
            Ok(_) => {}
            Err(UpdateError::Io(error)) => {
                assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn detached_script_runs() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        spawn_detached_script(&format!("touch {}", quote(&marker.to_string_lossy()))).unwrap();
        for _ in 0..50 {
            if marker.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("the script did not run");
    }
}
