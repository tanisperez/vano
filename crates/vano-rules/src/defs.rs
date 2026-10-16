//! The rule catalog: which rules exist and which well-known paths belong to them.
//!
//! Everything here is data. The matching logic lives in `engine.rs`.

use vano_core::Category;

use crate::{Os, Status};

/// When a rule can be decided during a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fire {
    /// As soon as the matched folder itself is complete.
    SelfDone,
    /// When the folder that contains the match is complete: the decision needs its siblings
    /// (project activity for `node_modules`) or the match is a plain file.
    ParentDone,
}

/// One rule of the catalog.
#[derive(Debug)]
pub(crate) struct RuleDef {
    pub id: &'static str,
    pub status: Status,
    /// The category its matches take in the treemap.
    pub cat: Category,
    pub fire: Fire,
    /// The size is an estimate (worktrees share objects with the main repository).
    pub approx: bool,
}

macro_rules! rules {
    ($($variant:ident: $id:literal, $status:ident, $cat:ident, $fire:ident, $approx:literal;)*) => {
        /// A rule, as an index into [`RULES`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum R { $($variant),* }

        /// Every rule, Safe ones first, indexed by `R as usize`.
        pub(crate) static RULES: &[RuleDef] = &[
            $(RuleDef {
                id: $id,
                status: Status::$status,
                cat: Category::$cat,
                fire: Fire::$fire,
                approx: $approx,
            }),*
        ];
    };
}

rules! {
    Npm: "npm", Safe, Caches, SelfDone, false;
    YarnPnpm: "yarn_pnpm", Safe, Caches, SelfDone, false;
    CargoCache: "cargo_cache", Safe, Caches, SelfDone, false;
    RustTarget: "rust_target", Safe, Caches, SelfDone, false;
    GradleCache: "gradle_cache", Safe, Caches, SelfDone, false;
    PythonCaches: "python_caches", Safe, Caches, SelfDone, false;
    BrowserDownloads: "browser_downloads", Safe, Caches, SelfDone, false;
    GoCache: "go_cache", Safe, Caches, SelfDone, false;
    JsBuild: "js_build", Safe, Caches, SelfDone, false;
    PythonBytecode: "python_bytecode", Safe, Caches, SelfDone, false;
    Thumbnails: "thumbnails", Safe, Caches, SelfDone, false;
    ShaderCache: "shader_cache", Safe, Caches, SelfDone, false;
    BrowserCache: "browser_cache", Safe, Caches, SelfDone, false;
    JetbrainsCache: "jetbrains_cache", Safe, Caches, SelfDone, false;
    PubCache: "pub_cache", Safe, Caches, SelfDone, false;
    DartBuild: "dart_build", Safe, Caches, SelfDone, false;
    GradleBuild: "gradle_build", Safe, Caches, SelfDone, false;
    MavenTarget: "maven_target", Safe, Caches, SelfDone, false;
    XcodeDerived: "xcode_derived", Safe, Caches, SelfDone, false;
    MacCaches: "mac_caches", Safe, Caches, SelfDone, false;
    FlatpakCache: "flatpak_cache", Safe, Caches, SelfDone, false;
    MavenRepo: "maven_repo", Review, Caches, SelfDone, false;
    PackageManagerCache: "package_manager_cache", Review, Caches, SelfDone, false;
    JournalLogs: "journal_logs", Review, Other, SelfDone, false;
    Trash: "trash", Review, Other, SelfDone, false;
    AndroidSdk: "android_sdk", Review, Sdk, SelfDone, false;
    Ollama: "ollama", Review, Ai, SelfDone, false;
    Huggingface: "huggingface", Review, Ai, SelfDone, false;
    LmStudio: "lm_studio", Review, Ai, SelfDone, false;
    NodeModules: "node_modules", Review, Caches, ParentDone, false;
    Worktrees: "worktrees", Review, Code, SelfDone, true;
    Docker: "docker", Review, Other, SelfDone, false;
    Installers: "installers", Review, Other, ParentDone, false;
    Recordings: "recordings", Review, Media, ParentDone, false;
    PythonVenv: "python_venv", Review, Caches, SelfDone, false;
    Cocoapods: "cocoapods", Review, Caches, SelfDone, false;
    IosSimulators: "ios_simulators", Review, Sdk, SelfDone, false;
}

/// Whether the rule matches plain files (its display path is a glob of extensions).
pub(crate) fn is_file_rule(rule: R) -> bool {
    matches!(rule, R::Installers | R::Recordings)
}

pub(crate) const L: u8 = 1;
pub(crate) const M: u8 = 2;
pub(crate) const W: u8 = 4;
pub(crate) const ALL: u8 = L | M | W;

/// The bit of `os` in the masks of [`TABLE`].
pub(crate) fn os_bit(os: Os) -> u8 {
    match os {
        Os::Linux => L,
        Os::MacOs => M,
        Os::Windows => W,
    }
}

/// Where a path of [`TABLE`] starts.
#[derive(Clone, Copy)]
pub(crate) enum Base {
    Home,
    Abs,
}

/// What a path of [`TABLE`] means.
#[derive(Clone, Copy)]
pub(crate) enum Entry {
    /// The folder is a recommendation of the rule.
    Hit(R),
    /// The folder and everything in it takes the category, unless something inside is more
    /// specific.
    Cat(Category),
    /// Like `Cat`, and name based rules and project detection are off inside (SDKs, system).
    CatNo(Category),
    /// Name based rules and project detection are off inside.
    NoNames,
    /// The user's home: never a "code project" itself, even with a dotfiles repository.
    Home,
    /// The files directly inside may match the rule (installers, recordings).
    Files(R),
}

use Base::{Abs as A, Home as H};
use Category::{Ai, Caches, Media, Sdk};
use Entry::{Cat, CatNo, Files, Hit, NoNames};

/// Fixed locations, relative to the home or absolute (no leading slash). `*` matches any one
/// folder name. Only paths whose owner tool recreates or can safely re-download the data are
/// `Hit`s with a Safe rule.
pub(crate) static TABLE: &[(u8, Base, &str, Entry)] = &[
    (ALL, H, "", Entry::Home),
    // Media.
    (ALL, H, "Documents", Cat(Media)),
    (ALL, H, "Pictures", Cat(Media)),
    (ALL, H, "Music", Cat(Media)),
    (ALL, H, "Videos", Cat(Media)),
    (M, H, "Movies", Cat(Media)),
    (ALL, H, "Desktop", Files(R::Recordings)),
    (ALL, H, "Downloads", Files(R::Installers)),
    (ALL, H, "Videos/Screencasts", Files(R::Recordings)),
    // Generic cache homes.
    (ALL, H, ".cache", Cat(Caches)),
    (M, H, "Library/Caches", Cat(Caches)),
    // JavaScript.
    (ALL, H, ".npm", Hit(R::Npm)),
    (W, H, "AppData/Local/npm-cache", Hit(R::Npm)),
    (L, H, ".cache/yarn", Hit(R::YarnPnpm)),
    (ALL, H, ".yarn/berry/cache", Hit(R::YarnPnpm)),
    (L, H, ".local/share/pnpm/store", Hit(R::YarnPnpm)),
    (L, H, ".cache/pnpm", Hit(R::YarnPnpm)),
    (M, H, "Library/Caches/Yarn", Hit(R::YarnPnpm)),
    (M, H, "Library/pnpm/store", Hit(R::YarnPnpm)),
    (M, H, "Library/Caches/pnpm", Hit(R::YarnPnpm)),
    (W, H, "AppData/Local/Yarn/Cache", Hit(R::YarnPnpm)),
    (W, H, "AppData/Local/pnpm/store", Hit(R::YarnPnpm)),
    (ALL, H, ".yarn", Cat(Caches)),
    // Rust.
    (ALL, H, ".cargo/registry", Cat(Caches)),
    (ALL, H, ".cargo/registry/cache", Hit(R::CargoCache)),
    (ALL, H, ".cargo/registry/src", Hit(R::CargoCache)),
    (ALL, H, ".cargo/git", Cat(Caches)),
    (ALL, H, ".cargo/git/checkouts", Hit(R::CargoCache)),
    (ALL, H, ".rustup", CatNo(Sdk)),
    // JVM.
    (ALL, H, ".gradle", Cat(Caches)),
    (ALL, H, ".gradle/caches", Hit(R::GradleCache)),
    (ALL, H, ".gradle/wrapper/dists", Hit(R::GradleCache)),
    (ALL, H, ".m2", Cat(Caches)),
    (ALL, H, ".m2/repository", Hit(R::MavenRepo)),
    (ALL, H, ".jdks", CatNo(Sdk)),
    (ALL, H, ".sdkman", CatNo(Sdk)),
    // Python.
    (ALL, H, ".cache/pip", Hit(R::PythonCaches)),
    (ALL, H, ".cache/uv", Hit(R::PythonCaches)),
    (ALL, H, ".cache/pypoetry", Hit(R::PythonCaches)),
    (M, H, "Library/Caches/pip", Hit(R::PythonCaches)),
    (M, H, "Library/Caches/uv", Hit(R::PythonCaches)),
    (M, H, "Library/Caches/pypoetry", Hit(R::PythonCaches)),
    (W, H, "AppData/Local/pip/Cache", Hit(R::PythonCaches)),
    (W, H, "AppData/Local/uv/cache", Hit(R::PythonCaches)),
    (W, H, "AppData/Local/pypoetry/Cache", Hit(R::PythonCaches)),
    // Downloaded browsers for automation.
    (ALL, H, ".cache/puppeteer", Hit(R::BrowserDownloads)),
    (L, H, ".cache/ms-playwright", Hit(R::BrowserDownloads)),
    (
        M,
        H,
        "Library/Caches/ms-playwright",
        Hit(R::BrowserDownloads),
    ),
    (
        W,
        H,
        "AppData/Local/ms-playwright",
        Hit(R::BrowserDownloads),
    ),
    // Go.
    (L, H, ".cache/go-build", Hit(R::GoCache)),
    (M, H, "Library/Caches/go-build", Hit(R::GoCache)),
    (W, H, "AppData/Local/go-build", Hit(R::GoCache)),
    (ALL, H, "go/pkg/mod", Hit(R::GoCache)),
    // Dart and Flutter.
    (ALL, H, ".pub-cache", Hit(R::PubCache)),
    (W, H, "AppData/Local/Pub/Cache", Hit(R::PubCache)),
    (ALL, H, "flutter", CatNo(Sdk)),
    (ALL, H, "development/flutter", CatNo(Sdk)),
    // Desktop caches.
    (L, H, ".cache/thumbnails", Hit(R::Thumbnails)),
    (L, H, ".thumbnails", Hit(R::Thumbnails)),
    (L, H, ".cache/mesa_shader_cache", Hit(R::ShaderCache)),
    (L, H, ".cache/mesa_shader_cache_db", Hit(R::ShaderCache)),
    (L, H, ".cache/fontconfig", Hit(R::ShaderCache)),
    (L, H, ".cache/nvidia", Hit(R::ShaderCache)),
    (L, H, ".cache/radv_builtin_shaders", Hit(R::ShaderCache)),
    (L, H, ".nv/GLCache", Hit(R::ShaderCache)),
    (W, H, "AppData/Local/D3DSCache", Hit(R::ShaderCache)),
    (L, H, ".cache/mozilla", Hit(R::BrowserCache)),
    (M, H, "Library/Caches/Firefox", Hit(R::BrowserCache)),
    (L, H, ".cache/JetBrains", Hit(R::JetbrainsCache)),
    (M, H, "Library/Caches/JetBrains", Hit(R::JetbrainsCache)),
    (L, H, ".var/app/*/cache", Hit(R::FlatpakCache)),
    // macOS.
    (M, H, "Library/Caches/Homebrew", Hit(R::MacCaches)),
    (M, H, "Library/Caches/CocoaPods", Hit(R::MacCaches)),
    (M, H, "Library/Caches/org.swift.swiftpm", Hit(R::MacCaches)),
    (M, H, "Library/Caches/com.apple.dt.Xcode", Hit(R::MacCaches)),
    (M, H, "Library/Developer/Xcode", CatNo(Sdk)),
    (
        M,
        H,
        "Library/Developer/Xcode/DerivedData",
        Hit(R::XcodeDerived),
    ),
    (
        M,
        H,
        "Library/Developer/Xcode/iOS DeviceSupport",
        Hit(R::XcodeDerived),
    ),
    (M, H, "Library/Developer/CoreSimulator", CatNo(Sdk)),
    (
        M,
        H,
        "Library/Developer/CoreSimulator/Devices",
        Hit(R::IosSimulators),
    ),
    (M, A, "Applications/Xcode.app", CatNo(Sdk)),
    // Android.
    (ALL, H, ".android", CatNo(Sdk)),
    (ALL, H, ".android/avd", Hit(R::AndroidSdk)),
    (L, H, "Android", CatNo(Sdk)),
    (L, H, "Android/Sdk", Hit(R::AndroidSdk)),
    (M, H, "Library/Android", CatNo(Sdk)),
    (M, H, "Library/Android/sdk", Hit(R::AndroidSdk)),
    (W, H, "AppData/Local/Android", CatNo(Sdk)),
    (W, H, "AppData/Local/Android/Sdk", Hit(R::AndroidSdk)),
    (L, A, "opt/android-sdk", Hit(R::AndroidSdk)),
    // AI models.
    (ALL, H, ".ollama", Cat(Ai)),
    (ALL, H, ".ollama/models", Hit(R::Ollama)),
    (L, A, "usr/share/ollama", Cat(Ai)),
    (L, A, "usr/share/ollama/.ollama/models", Hit(R::Ollama)),
    (L, A, "var/lib/ollama", Cat(Ai)),
    (ALL, H, ".cache/huggingface", Hit(R::Huggingface)),
    (ALL, H, ".cache/lm-studio", Cat(Ai)),
    (ALL, H, ".cache/lm-studio/models", Hit(R::LmStudio)),
    (ALL, H, ".lmstudio", Cat(Ai)),
    (ALL, H, ".lmstudio/models", Hit(R::LmStudio)),
    (ALL, H, ".cache/torch", Cat(Ai)),
    // Containers.
    (L, H, ".local/share/docker", Hit(R::Docker)),
    (M, H, "Library/Containers/com.docker.docker", Hit(R::Docker)),
    (W, H, "AppData/Local/Docker", Hit(R::Docker)),
    (L, A, "var/lib/docker", Hit(R::Docker)),
    // Trash and system caches.
    (L, H, ".local/share/Trash", Hit(R::Trash)),
    (M, H, ".Trash", Hit(R::Trash)),
    (L, A, "var/cache/pacman/pkg", Hit(R::PackageManagerCache)),
    (L, A, "var/cache/apt/archives", Hit(R::PackageManagerCache)),
    (L, A, "var/log/journal", Hit(R::JournalLogs)),
    // System folders: only the fixed rules above apply there.
    (L, A, "usr", NoNames),
    (L, A, "bin", NoNames),
    (L, A, "sbin", NoNames),
    (L, A, "lib", NoNames),
    (L, A, "lib64", NoNames),
    (L, A, "etc", NoNames),
    (L, A, "boot", NoNames),
    (L, A, "opt", NoNames),
    (L, A, "var", NoNames),
    (L, A, "srv", NoNames),
    (L, A, "snap", NoNames),
    (L, A, "sys", NoNames),
    (L, A, "proc", NoNames),
    (L, A, "dev", NoNames),
    (L, A, "run", NoNames),
    (M, A, "System", NoNames),
    (M, A, "Library", NoNames),
    (M, A, "usr", NoNames),
    (M, A, "private", NoNames),
    (M, A, "bin", NoNames),
    (M, A, "sbin", NoNames),
    (M, A, "opt", NoNames),
    (M, A, "cores", NoNames),
    (M, A, "Applications", NoNames),
    (W, A, "C:/Windows", NoNames),
    (W, A, "C:/Program Files", NoNames),
    (W, A, "C:/Program Files (x86)", NoNames),
    (W, A, "C:/ProgramData", NoNames),
];

/// Chromium based browsers: the cache folders inside each profile (`*`), by OS.
pub(crate) fn chromium_bases(os: Os) -> &'static [&'static str] {
    match os {
        Os::Linux => &[
            ".cache/google-chrome",
            ".cache/chromium",
            ".cache/BraveSoftware/Brave-Browser",
            ".cache/microsoft-edge",
        ],
        Os::MacOs => &[
            "Library/Caches/Google/Chrome",
            "Library/Caches/BraveSoftware/Brave-Browser",
            "Library/Caches/Microsoft Edge",
        ],
        Os::Windows => &[
            "AppData/Local/Google/Chrome/User Data",
            "AppData/Local/Microsoft/Edge/User Data",
            "AppData/Local/BraveSoftware/Brave-Browser/User Data",
        ],
    }
}

/// The cache folders inside a Chromium profile.
pub(crate) const CHROMIUM_CACHES: [&str; 3] = ["Cache", "Code Cache", "GPUCache"];

/// Names of folders that name based rules can match, lowercase. Used to skip quickly the folders
/// that cannot be a match when a scan reports them complete.
pub(crate) const NAME_RULE_NAMES: [&str; 17] = [
    "target",
    ".next",
    ".turbo",
    ".parcel-cache",
    ".nuxt",
    ".svelte-kit",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".gradle",
    "build",
    ".dart_tool",
    ".venv",
    "venv",
    "pods",
    "worktrees",
];
