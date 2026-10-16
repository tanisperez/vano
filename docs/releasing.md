# Releasing Vano

Publishing a version means pushing a tag. The `Release` workflow
(`.github/workflows/release.yml`) builds Vano for every platform, attaches the binaries to the
GitHub release of that tag and publishes it. The `Build` workflow (`build.yml`) is the per-push CI.
The `Website` workflow (`website.yml`) publishes `site/` to GitHub Pages (vano.tanis.codes) on
every push to `main` that touches it, rebuilt from scratch each time, with no gh-pages branch; its
download links point to the latest release, so it needs no step of its own here. There is no APT,
DNF, Homebrew or winget automation yet: only the AUR `PKGBUILD` template exists, and it is
published by hand.

## Versions and tags

- The version lives in one place: `version` under `[workspace.package]` in `Cargo.toml`.
  Versions follow SemVer.
- The tag is `v` plus that version: `v0.1.0`. The workflow fails if they differ.
- A tag with a pre-release suffix (`v0.1.0-rc.1`, `v0.2.0-beta.2`) is accepted when the part
  before the suffix matches the version (or when the version itself has the same suffix). It is
  published as a GitHub pre-release: built and attached like any release, but never marked as
  the latest one.

## Steps

The work of a version is tracked by its GitHub milestone (`gh issue list --milestone X.Y.Z`).

1. Check that every issue of the milestone is closed and `make format-check`, `make lint` and
   `make test` pass on `main`.
2. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, and commit.
3. Tag and push:

   ```
   git tag v0.1.0
   git push origin main v0.1.0
   ```

4. Create the release as a **draft**, with the notes in English Markdown (what changes and which
   milestone issues went in):

   ```
   gh release create v0.1.0 --draft --verify-tag --title v0.1.0 --notes-file notes.md
   ```

5. Wait for the `Release` workflow (`gh run watch`). It uploads the assets to the draft and then
   publishes it. If the draft does not exist yet when the builds finish, the workflow creates one
   with placeholder notes; edit them afterwards with `gh release edit v0.1.0 --notes-file notes.md`.

The release stays a draft until every asset is uploaded, so nobody sees a half-built release.

## Assets

| Asset | Content |
|---|---|
| `vano-X.Y.Z-linux-x86_64.tar.gz` | Linux binary plus desktop entry and icons in a `/usr`-like layout |
| `vano_X.Y.Z_amd64.deb` | Debian and Ubuntu package |
| `vano-X.Y.Z-1.x86_64.rpm` | Fedora and openSUSE package |
| `Vano-X.Y.Z-macos-arm64.dmg` | macOS app, Apple silicon, ad-hoc signed |
| `Vano-X.Y.Z-macos-arm64.app.tar.gz` | The same app, for the in-app updater |
| `Vano-X.Y.Z-windows-x86_64-setup.exe` | Per-user Windows installer |
| `Vano-X.Y.Z-windows-x86_64-portable.zip` | Portable Windows build: `vano.exe`, `LICENSE` and `NOTICE` at the top level of the zip, nothing to install |
| `latest.json` | Manifest read by the in-app updater: version, release page, and URL plus SHA-256 of the macOS app archive and the Windows installer (not the portable zip) |
| `SHA256SUMS` | SHA-256 of every asset above |

The portable zip is built by the Windows job right after the installer, with `Compress-Archive`,
from the same `vano.exe`. `latest.json` is written by globbing
`Vano-*-windows-x86_64-setup.exe`, which the zip name does not match, so the manifest keeps
pointing at the installer.

Linux binaries are built on Ubuntu 22.04 (glibc 2.35). Inside the deb and rpm a pre-release
version uses a tilde (`0.1.0~rc.1`) so it sorts before the final release. Every file name keeps
the dash, since GitHub renames a `~` in an asset name to `.`.

The packaging scripts live in `packaging/` and run in the workflow. `make dist` builds the
packages for the current OS locally into `target/dist` (Linux needs `nfpm` on `PATH`, macOS the
Xcode command line tools). The Windows installer is compiled with Inno Setup:

```
ISCC.exe /DAppVersion=0.1.0 /DSourceExe=..\..\target\release\vano.exe /DOutputDir=..\..\dist vano.iss
```

Layout of `packaging/`:

- `linux/`: `codes.tanis.vano.desktop`, `nfpm.yaml` (deb and rpm) and `package.sh` (tarball,
  deb, rpm).
- `macos/`: `Info.plist` (template, `bundle.sh` fills the version) and `bundle.sh`.
- `windows/`: `vano.iss`. It reads the icon from `crates/vano-app/assets/vano.ico`.
- `aur/`: `PKGBUILD` of `vano-bin`. Set `pkgver` to a stable version and run `updpkgsums`.
- `icons/`: the SVG master and the generated PNG sizes (`vano.svg`, `png/`).

Vano opens no file types, so the packages register no MIME type, no file association and no
document types.

## In-app updater

The `vano-update` crate (no UI, tested against a local `tiny_http` server). Package managers own
updates on Linux, so the updater exists only on macOS and Windows.

- **Compiled in only with `VANO_UPDATER=github`** at build time, set only on the macOS and Windows
  jobs of `release.yml`. The app must read it with `option_env!`; otherwise the feature is absent
  (deb, rpm, AUR and local builds).
- **Manifest:** `GET https://github.com/tanisperez/vano/releases/latest/download/latest.json`,
  compared with the app's own version using `semver`. GitHub redirects that URL to the newest
  stable release, never to a pre-release. The `VANO_UPDATE_URL` environment variable overrides
  it. To test an update between pre-releases, run the older one with it pointing at the newer
  manifest:

  ```sh
  VANO_UPDATE_URL=https://github.com/tanisperez/vano/releases/download/vX.Y.Z-rc.N/latest.json vano
  ```

- **Manifest format:** `version`, `notes_url` and `platforms`, with the keys `macos-aarch64` and
  `windows-x86_64`, each `{ url, sha256 }`. `release.yml` writes it with `jq`.
- **Download:** the asset for this platform is downloaded and its SHA-256 verified. Nothing
  installs without a click. Asset file names must be plain (no path separators).
- **Windows install:** run the downloaded installer with
  `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /relaunch=1` and quit. `vano.iss` closes a running
  Vano through the Restart Manager and relaunches it only because of `/relaunch=1`.
- **Portable Windows copy:** the installer always leaves `unins000.exe` next to `vano.exe`
  (`vano_update::install::windows_is_installed`). A copy without it is portable and must only
  point the user to the release page. Keep that file name and location if the installer script
  changes.
- **macOS install:** unpack the `.app.tar.gz` next to the installed bundle, swap the bundles from
  a detached script once the app exits, and relaunch with `open`.
- The app version comes from the workspace version, not from the tag. For a test pre-release set
  the full version in `Cargo.toml` (`version = "0.1.0-rc.4"`) before tagging.

## When something fails

- **Version mismatch**: delete the tag (`git push origin :refs/tags/v0.1.0`, `git tag -d v0.1.0`),
  fix `Cargo.toml`, commit and tag again.
- **A build fails**: fix it on `main`, then move the tag to the fixed commit (delete and push it
  again). The draft keeps its notes; re-running the workflow replaces the assets (`--clobber`).
- **Only the publish job failed**: re-run the failed jobs from the Actions page.

## Identity

- **App id:** `codes.tanis.vano`, used for the `.desktop` file, the Wayland `app_id`, the macOS
  bundle id and the Windows AppUserModelID.
- **Targets:** Linux x86_64, Windows x86_64 and macOS arm64.
- **Linux dependencies** declared by the packages: `libxkbcommon`, `libxkbcommon-x11`, `libxcb`,
  `libwayland-client`, `libwayland-egl`, `libegl1` (Vulkan only recommended).
- **Signing:** the builds are unsigned. macOS Gatekeeper needs "Open Anyway" on first launch
  (or `xattr -dr com.apple.quarantine /Applications/Vano.app`) and Windows SmartScreen shows
  "More info, Run anyway". Self updates are not affected.

## Later

macOS signing and notarization, ARM builds, signing `latest.json`, Windows code signing, APT and
DNF repositories, Homebrew and winget channels.
