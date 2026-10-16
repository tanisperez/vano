use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use super::*;

/// A local server with fixed routes: path to (status, extra header, body).
struct Server {
    server: Arc<tiny_http::Server>,
    base: String,
    hits: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

type Routes = HashMap<&'static str, (u16, Option<(&'static str, String)>, Vec<u8>)>;

impl Server {
    fn start(routes: Routes) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let base = format!("http://{}", server.server_addr().to_ip().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let thread = {
            let server = Arc::clone(&server);
            let hits = Arc::clone(&hits);
            std::thread::spawn(move || {
                for request in server.incoming_requests() {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let (status, header, body) =
                        routes
                            .get(request.url())
                            .cloned()
                            .unwrap_or((404, None, Vec::new()));
                    let mut response =
                        tiny_http::Response::from_data(body).with_status_code(status);
                    if let Some((name, value)) = header {
                        response.add_header(
                            tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes())
                                .unwrap(),
                        );
                    }
                    let _ = request.respond(response);
                }
            })
        };
        Self {
            server,
            base,
            hits,
            thread: Some(thread),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn manifest(version: &str, with_windows: bool) -> Vec<u8> {
    let mut platforms = serde_json::json!({
        "macos-aarch64": { "url": "http://x/mac.tar.gz", "sha256": "ABC", "extra": 1 },
        "linux-riscv": { "url": "http://x/l", "sha256": "d" }
    });
    if with_windows {
        platforms["windows-x86_64"] =
            serde_json::json!({ "url": "http://x/w.exe", "sha256": "def" });
    }
    serde_json::to_vec(&serde_json::json!({
        "version": version,
        "notes_url": "http://x/notes",
        "future_field": true,
        "platforms": platforms,
    }))
    .unwrap()
}

fn serve_manifest(body: Vec<u8>) -> Server {
    Server::start(HashMap::from([("/latest.json", (200, None, body))]))
}

fn check_with(
    body: Vec<u8>,
    current: &str,
    platform: Platform,
) -> Result<Option<Update>, UpdateError> {
    let server = serve_manifest(body);
    check(&server.url("/latest.json"), current, platform)
}

#[test]
fn newer_version_is_an_update() {
    let update = check_with(manifest("0.2.0", true), "0.1.0", Platform::MacosAarch64)
        .unwrap()
        .unwrap();
    assert_eq!(update.version, semver::Version::new(0, 2, 0));
    assert_eq!(update.notes_url, "http://x/notes");
    assert_eq!(update.url, "http://x/mac.tar.gz");
    assert_eq!(update.sha256, "abc");
    let windows = check_with(manifest("0.2.0", true), "0.1.0", Platform::WindowsX86_64)
        .unwrap()
        .unwrap();
    assert_eq!(windows.url, "http://x/w.exe");
}

#[test]
fn same_or_older_is_not_an_update() {
    assert_eq!(
        check_with(manifest("0.1.0", true), "0.1.0", Platform::MacosAarch64).unwrap(),
        None
    );
    assert_eq!(
        check_with(manifest("0.1.0", true), "0.2.0", Platform::MacosAarch64).unwrap(),
        None
    );
}

#[test]
fn pre_release_ordering() {
    let mac = Platform::MacosAarch64;
    assert!(
        check_with(manifest("0.1.0-rc.4", true), "0.1.0-rc.3", mac)
            .unwrap()
            .is_some()
    );
    assert!(
        check_with(manifest("0.1.0", true), "0.1.0-rc.4", mac)
            .unwrap()
            .is_some()
    );
    assert!(
        check_with(manifest("0.1.0-rc.3", true), "0.1.0-rc.4", mac)
            .unwrap()
            .is_none()
    );
    assert!(
        check_with(manifest("0.1.0-rc.4", true), "0.1.0", mac)
            .unwrap()
            .is_none()
    );
}

#[test]
fn missing_platform_is_not_an_error() {
    let result = check_with(manifest("0.2.0", false), "0.1.0", Platform::WindowsX86_64);
    assert_eq!(result.unwrap(), None);
}

#[test]
fn malformed_manifest_is_a_parse_error() {
    let result = check_with(b"not json".to_vec(), "0.1.0", Platform::MacosAarch64);
    assert!(matches!(result, Err(UpdateError::Parse(_))));
}

#[test]
fn bad_versions_are_reported() {
    let result = check_with(manifest("banana", true), "0.1.0", Platform::MacosAarch64);
    assert!(matches!(result, Err(UpdateError::BadVersion(_))));
    let result = check_with(manifest("0.2.0", true), "nope", Platform::MacosAarch64);
    assert!(matches!(result, Err(UpdateError::BadVersion(_))));
}

#[test]
fn http_404_is_a_status_error() {
    let server = Server::start(HashMap::new());
    let result = check(&server.url("/latest.json"), "0.1.0", Platform::MacosAarch64);
    assert!(matches!(result, Err(UpdateError::Status(404))));
}

#[test]
fn oversized_manifest_is_rejected() {
    let result = check_with(vec![b' '; 70 * 1024], "0.1.0", Platform::MacosAarch64);
    assert!(result.is_err());
}

#[test]
fn unreachable_server_is_a_network_error() {
    let result = check(
        "http://127.0.0.1:1/latest.json",
        "0.1.0",
        Platform::MacosAarch64,
    );
    assert!(matches!(result, Err(UpdateError::Network(_))));
}

#[test]
fn manifest_url_defaults_to_latest() {
    // `set_var` is unsafe and the workspace forbids unsafe code, so only the unset case is checked.
    if std::env::var(URL_ENV).is_err() {
        assert_eq!(manifest_url(), LATEST_URL);
    }
}

fn sha_of(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn update_for(server: &Server, path: &str, data: &[u8]) -> Update {
    Update {
        version: semver::Version::new(0, 2, 0),
        notes_url: String::new(),
        url: server.url(path),
        sha256: sha_of(data),
    }
}

#[test]
fn download_verifies_and_renames() {
    let data = vec![7u8; 300_000];
    let server = Server::start(HashMap::from([(
        "/a/Vano-setup.exe",
        (200, None, data.clone()),
    )]));
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("nested");
    let update = update_for(&server, "/a/Vano-setup.exe", &data);

    let path = download(&update, &target).unwrap();

    assert_eq!(path, target.join("Vano-setup.exe"));
    assert_eq!(std::fs::read(&path).unwrap(), data);
    assert!(!target.join("Vano-setup.exe.part").exists());
}

#[test]
fn download_checksum_mismatch_deletes_the_partial_file() {
    let server = Server::start(HashMap::from([("/f.bin", (200, None, b"hello".to_vec()))]));
    let dir = tempfile::tempdir().unwrap();
    let update = update_for(&server, "/f.bin", b"other");

    let result = download(&update, dir.path());

    assert!(matches!(result, Err(UpdateError::Checksum { .. })));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn download_reuses_a_verified_file() {
    let server = Server::start(HashMap::from([("/f.bin", (200, None, b"hello".to_vec()))]));
    let dir = tempfile::tempdir().unwrap();
    let update = update_for(&server, "/f.bin", b"hello");

    download(&update, dir.path()).unwrap();
    assert_eq!(server.hits(), 1);
    download(&update, dir.path()).unwrap();
    assert_eq!(server.hits(), 1);

    // A corrupt file with the same name is downloaded again.
    std::fs::write(dir.path().join("f.bin"), "bad").unwrap();
    download(&update, dir.path()).unwrap();
    assert_eq!(server.hits(), 2);
    assert_eq!(std::fs::read(dir.path().join("f.bin")).unwrap(), b"hello");
}

#[test]
fn download_follows_redirects() {
    let server = Server::start(HashMap::from([
        (
            "/old/f.bin",
            (
                302,
                Some(("Location", "/new/f.bin".to_string())),
                Vec::new(),
            ),
        ),
        ("/new/f.bin", (200, None, b"payload".to_vec())),
    ]));
    let dir = tempfile::tempdir().unwrap();
    let update = update_for(&server, "/old/f.bin", b"payload");

    let path = download(&update, dir.path()).unwrap();

    assert_eq!(std::fs::read(path).unwrap(), b"payload");
}

#[test]
fn download_404_is_a_status_error() {
    let server = Server::start(HashMap::new());
    let dir = tempfile::tempdir().unwrap();
    let update = update_for(&server, "/missing.bin", b"x");
    assert!(matches!(
        download(&update, dir.path()),
        Err(UpdateError::Status(404))
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn file_names_are_taken_from_the_url() {
    assert_eq!(file_name("https://h/a/b/c.zip?x=1#y").unwrap(), "c.zip");
    assert!(file_name("https://h/a/").is_err());
    assert!(file_name("https://h/a/..").is_err());
}

#[test]
fn platform_keys() {
    assert_eq!(Platform::MacosAarch64.key(), "macos-aarch64");
    assert_eq!(Platform::WindowsX86_64.key(), "windows-x86_64");
}
