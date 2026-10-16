//! Writes the made-up home folder the website screenshots show, as a saved scan the app opens
//! with `VANO_OPEN=results`, so no real folder of the person taking them ever appears.
//!
//! Run with `cargo run -p vano-rules --example demo_home -- <home> <config dir> <cache dir>`.
//! It writes `<cache dir>/vano/scans/demo.vtree` and `<config dir>/vano/recents.json`, and
//! prints the recommendations the rules find in the tree. `site/screenshots/capture.sh` runs it.
//! With a fourth argument it also writes there a manifest of every folder and file (one per line,
//! tab separated: `D path` or `F path size mtime`, paths relative to the home), which
//! `site/screenshots/record-video.sh` turns into real files so the app can scan the made-up home live.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vano_core::tree::FLAG_COMPLETE;
use vano_core::{EntryData, Kind, NodeId, ROOT, Tree, persist};
use vano_rules::{Context, Os, evaluate};

const KB: u64 = 1_000;
const MB: u64 = 1_000_000;
const GB: u64 = 1_000_000_000;
const DAY: u32 = 86_400;
const BLOCK: u64 = 4_096;

/// A deterministic random generator, so every run writes the same tree.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn hex(&mut self, len: usize) -> String {
        (0..len)
            .map(|_| char::from_digit(self.below(16) as u32, 16).unwrap_or('0'))
            .collect()
    }
}

/// One entry of the made-up home, before it becomes tree nodes.
enum Spec {
    Dir(String, Vec<Spec>),
    File(String, u64, u32),
}

/// The context every builder needs: the generator and the time files are dated from.
struct Gen {
    rng: Rng,
    now: u32,
}

impl Gen {
    fn ago(&mut self, days: u32) -> u32 {
        self.now - days * DAY - self.rng.below(u64::from(DAY)) as u32
    }

    fn file(&mut self, name: &str, size: u64, days: u32) -> Spec {
        let mtime = self.ago(days);
        Spec::File(name.to_string(), size, mtime)
    }

    /// `count` files sharing `total` bytes unevenly, named by `name`.
    fn files(
        &mut self,
        count: usize,
        total: u64,
        days: u32,
        mut name: impl FnMut(&mut Rng, usize) -> String,
    ) -> Vec<Spec> {
        let weights: Vec<u64> = (0..count)
            .map(|_| {
                let w = 1 + self.rng.below(40);
                w * w
            })
            .collect();
        let sum: u64 = weights.iter().sum();
        (0..count)
            .map(|i| {
                let size = (total * weights[i] / sum).max(1).div_ceil(BLOCK) * BLOCK;
                let file_name = name(&mut self.rng, i);
                let age = days + self.rng.below(u64::from(days / 2 + 1)) as u32;
                self.file(&file_name, size, age)
            })
            .collect()
    }

    /// `dirs` folders with `per_dir` files each, sharing `total` bytes.
    fn spread(
        &mut self,
        dirs: usize,
        per_dir: usize,
        total: u64,
        days: u32,
        mut dir_name: impl FnMut(&mut Rng, usize) -> String,
        ext: &str,
    ) -> Vec<Spec> {
        (0..dirs)
            .map(|i| {
                let name = dir_name(&mut self.rng, i);
                let share = total / dirs as u64 * (1 + self.rng.below(3)) / 2;
                let kids = self.files(per_dir, share, days, |rng, _| {
                    format!("{}.{ext}", rng.hex(12))
                });
                Spec::Dir(name, kids)
            })
            .collect()
    }

    fn rust_project(&mut self, name: &str, target: u64, days: u32) -> Spec {
        let crates = [
            "serde", "tokio", "hyper", "axum", "regex", "syn", "rustls", "sqlx", "tracing", "clap",
            "rand", "image", "wgpu", "naga", "bevy_ecs", "prost",
        ];
        let deps = self.files(2_400, target * 55 / 100, days, |rng, i| {
            let krate = crates[i % crates.len()];
            let ext = ["rlib", "rmeta", "d", "so"][i % 4];
            format!("lib{krate}-{}.{ext}", rng.hex(16))
        });
        let incremental = self.spread(
            40,
            180,
            target * 20 / 100,
            days,
            |rng, i| format!("{name}-{}-{i}", rng.hex(8)),
            "o",
        );
        let build = self.spread(
            60,
            12,
            target * 8 / 100,
            days,
            |rng, i| format!("{}-{}", crates[i % crates.len()], rng.hex(16)),
            "o",
        );
        let release_deps = self.files(900, target * 17 / 100, days + 6, |rng, i| {
            format!("lib{}-{}.rlib", crates[i % crates.len()], rng.hex(16))
        });
        let sources = self.files(48, 900 * KB, days, |_, i| format!("module_{i}.rs"));
        let git = self.files(320, 140 * MB, days, |rng, _| rng.hex(38));
        Spec::Dir(
            name.to_string(),
            vec![
                self.file("Cargo.toml", 2 * KB, days),
                self.file("Cargo.lock", 90 * KB, days),
                self.file("README.md", 6 * KB, days + 20),
                Spec::Dir("src".into(), sources),
                Spec::Dir(".git".into(), vec![Spec::Dir("objects".into(), git)]),
                Spec::Dir(
                    "target".into(),
                    vec![
                        self.file("CACHEDIR.TAG", 4 * KB, days),
                        Spec::Dir(
                            "debug".into(),
                            vec![
                                Spec::Dir("deps".into(), deps),
                                Spec::Dir("incremental".into(), incremental),
                                Spec::Dir("build".into(), build),
                            ],
                        ),
                        Spec::Dir(
                            "release".into(),
                            vec![Spec::Dir("deps".into(), release_deps)],
                        ),
                    ],
                ),
            ],
        )
    }

    fn node_modules(&mut self, total: u64, files: usize, days: u32) -> Spec {
        let packages = [
            "react",
            "react-dom",
            "next",
            "typescript",
            "@babel",
            "@swc",
            "esbuild",
            "webpack",
            "eslint",
            "lodash",
            "date-fns",
            "@types",
            "postcss",
            "tailwindcss",
            "zod",
            "prettier",
        ];
        let dirs = packages.len() * 6;
        let kids = self.spread(
            dirs,
            files / dirs,
            total,
            days,
            |rng, i| {
                let base = packages[i % packages.len()];
                if i < packages.len() {
                    base.to_string()
                } else {
                    format!("{base}-{}", rng.hex(4))
                }
            },
            "js",
        );
        Spec::Dir("node_modules".into(), kids)
    }

    fn js_project(&mut self, name: &str, modules: u64, next: u64, days: u32) -> Spec {
        let mut kids = vec![
            self.file("package.json", 3 * KB, days),
            self.file("package-lock.json", 420 * KB, days),
        ];
        let sources = self.files(160, 3 * MB, days, |_, i| format!("component_{i}.tsx"));
        kids.push(Spec::Dir("src".into(), sources));
        let public = self.files(40, 85 * MB, days + 30, |_, i| format!("image_{i}.webp"));
        kids.push(Spec::Dir("public".into(), public));
        kids.push(self.node_modules(modules, 24_000, days));
        if next > 0 {
            let cache = self.spread(30, 90, next, days, |rng, _| rng.hex(10), "pack");
            kids.push(Spec::Dir(".next".into(), cache));
        }
        Spec::Dir(name.to_string(), kids)
    }

    fn cache(&mut self, name: &str, total: u64, files: usize, days: u32) -> Spec {
        let dirs = (files / 200).max(1);
        let kids = self.spread(dirs, files / dirs, total, days, |rng, _| rng.hex(2), "bin");
        Spec::Dir(name.to_string(), kids)
    }

    fn home(&mut self) -> Vec<Spec> {
        let dev = vec![
            self.rust_project("atlas-api", 22 * GB, 1),
            self.rust_project("photon", 11 * GB, 3),
            self.js_project("storefront", 1_400 * MB, 1_900 * MB, 2),
            self.js_project("legacy-dashboard", 1_100 * MB, 0, 240),
            self.android_project(),
            self.python_project(),
            self.flutter_project(),
        ];

        let cache = vec![
            Spec::Dir(
                "huggingface".into(),
                vec![Spec::Dir("hub".into(), self.models())],
            ),
            self.cache("pip", 3_100 * MB, 6_000, 12),
            Spec::Dir(
                "google-chrome".into(),
                vec![Spec::Dir(
                    "Default".into(),
                    vec![
                        self.cache("Cache", 1_700 * MB, 9_000, 1),
                        self.cache("Code Cache", 600 * MB, 3_000, 1),
                    ],
                )],
            ),
            self.cache("mozilla", 900 * MB, 4_000, 1),
            self.cache("JetBrains", 2_400 * MB, 5_000, 4),
            self.cache("thumbnails", 450 * MB, 12_000, 2),
            self.cache("go-build", 1_300 * MB, 8_000, 20),
            self.cache("ms-playwright", 1_600 * MB, 2_000, 45),
            self.cache("yarn", 1_900 * MB, 7_000, 30),
            self.cache("mesa_shader_cache", 300 * MB, 1_200, 1),
        ];

        let android_sdk = vec![
            self.cache("system-images", 8_200 * MB, 600, 90),
            self.cache("platforms", 1_900 * MB, 1_400, 90),
            self.cache("build-tools", 900 * MB, 800, 90),
            self.cache("emulator", 1_100 * MB, 500, 60),
            self.cache("ndk", 2_600 * MB, 4_000, 120),
        ];

        vec![
            Spec::Dir("dev".into(), dev),
            Spec::Dir(".cache".into(), cache),
            Spec::Dir(
                ".npm".into(),
                vec![self.cache("_cacache", 3_600 * MB, 18_000, 3)],
            ),
            Spec::Dir(
                ".cargo".into(),
                vec![
                    Spec::Dir(
                        "registry".into(),
                        vec![
                            self.cache("cache", 900 * MB, 1_800, 5),
                            self.cache("src", 2_100 * MB, 30_000, 5),
                        ],
                    ),
                    Spec::Dir(
                        "git".into(),
                        vec![self.cache("checkouts", 600 * MB, 4_000, 15)],
                    ),
                    self.cache("bin", 120 * MB, 12, 30),
                ],
            ),
            self.cache(".rustup", 4_200 * MB, 16_000, 10),
            Spec::Dir(
                ".gradle".into(),
                vec![
                    self.cache("caches", 6_400 * MB, 14_000, 8),
                    Spec::Dir(
                        "wrapper".into(),
                        vec![self.cache("dists", 1_100 * MB, 400, 60)],
                    ),
                ],
            ),
            Spec::Dir(
                ".m2".into(),
                vec![self.cache("repository", 2_200 * MB, 9_000, 50)],
            ),
            Spec::Dir("Android".into(), vec![Spec::Dir("Sdk".into(), android_sdk)]),
            Spec::Dir(
                ".android".into(),
                vec![Spec::Dir(
                    "avd".into(),
                    vec![self.cache("Pixel_8_API_35.avd", 7_800 * MB, 40, 70)],
                )],
            ),
            Spec::Dir(
                ".ollama".into(),
                vec![Spec::Dir(
                    "models".into(),
                    vec![
                        Spec::Dir("blobs".into(), self.ollama_blobs()),
                        self.cache("manifests", 2 * MB, 12, 40),
                    ],
                )],
            ),
            Spec::Dir(
                ".local".into(),
                vec![Spec::Dir(
                    "share".into(),
                    vec![
                        Spec::Dir(
                            "Trash".into(),
                            vec![self.cache("files", 2_700 * MB, 900, 25)],
                        ),
                        self.cache("docker", 8_400 * MB, 40_000, 35),
                        self.cache("fonts", 300 * MB, 400, 200),
                    ],
                )],
            ),
            Spec::Dir("Videos".into(), self.videos()),
            Spec::Dir("Pictures".into(), self.pictures()),
            Spec::Dir("Music".into(), self.music()),
            Spec::Dir("Documents".into(), self.documents()),
            Spec::Dir("Downloads".into(), self.downloads()),
            Spec::Dir(
                "Desktop".into(),
                vec![
                    self.file("notes.md", 12 * KB, 3),
                    self.file("roadmap.pdf", 2 * MB, 20),
                ],
            ),
            self.file(".bashrc", 4 * KB, 100),
            self.file(".gitconfig", 4 * KB, 300),
        ]
    }

    fn android_project(&mut self) -> Spec {
        let build = self.cache("build", 2_800 * MB, 9_000, 4);
        let sources = self.files(220, 6 * MB, 4, |_, i| format!("Screen{i}.kt"));
        Spec::Dir(
            "weather-android".into(),
            vec![
                self.file("build.gradle.kts", 4 * KB, 4),
                self.file("settings.gradle.kts", KB, 30),
                Spec::Dir(
                    "app".into(),
                    vec![
                        self.file("build.gradle.kts", 6 * KB, 4),
                        build,
                        Spec::Dir("src".into(), sources),
                    ],
                ),
                self.cache(".gradle", 400 * MB, 600, 4),
            ],
        )
    }

    fn python_project(&mut self) -> Spec {
        let data = self.files(36, 7_500 * MB, 30, |_, i| {
            format!("events_2025_{i:02}.parquet")
        });
        let notebooks = self.files(24, 60 * MB, 9, |_, i| format!("experiment_{i:02}.ipynb"));
        Spec::Dir(
            "ml-lab".into(),
            vec![
                self.file("requirements.txt", KB, 9),
                self.cache(".venv", 5_200 * MB, 42_000, 9),
                self.cache("__pycache__", 40 * MB, 300, 9),
                Spec::Dir("data".into(), data),
                Spec::Dir("notebooks".into(), notebooks),
            ],
        )
    }

    fn flutter_project(&mut self) -> Spec {
        let sources = self.files(90, 2 * MB, 14, |_, i| format!("widget_{i}.dart"));
        Spec::Dir(
            "tide".into(),
            vec![
                self.file("pubspec.yaml", 2 * KB, 14),
                self.cache("build", 2_100 * MB, 7_000, 14),
                self.cache(".dart_tool", 700 * MB, 3_000, 14),
                Spec::Dir("lib".into(), sources),
            ],
        )
    }

    fn models(&mut self) -> Vec<Spec> {
        [
            ("models--meta-llama--Llama-3.1-8B-Instruct", 9_100 * MB),
            ("models--mistralai--Mistral-7B-v0.3", 4_300 * MB),
            ("models--openai--whisper-large-v3", 1_600 * MB),
            ("models--stabilityai--sdxl-turbo", 900 * MB),
        ]
        .into_iter()
        .map(|(name, size)| {
            let shards = self.files(4, size, 50, |_, i| format!("model-{i:05}.safetensors"));
            Spec::Dir(name.into(), vec![Spec::Dir("blobs".into(), shards)])
        })
        .collect()
    }

    fn ollama_blobs(&mut self) -> Vec<Spec> {
        [4_700 * MB, 4_100 * MB, 2_000 * MB, 1_300 * MB, 600 * MB]
            .into_iter()
            .map(|size| {
                let name = format!("sha256-{}", self.rng.hex(64));
                self.file(&name, size, 40)
            })
            .collect()
    }

    fn videos(&mut self) -> Vec<Spec> {
        let screencasts = vec![
            self.file("demo-walkthrough.mp4", 3_400 * MB, 40),
            self.file("conference-talk.mkv", 2_800 * MB, 120),
            self.file("bug-repro.webm", 900 * MB, 15),
        ];
        let trips = self.files(60, 9_800 * MB, 200, |_, i| format!("VID_2025_{i:04}.mov"));
        vec![
            Spec::Dir("Screencasts".into(), screencasts),
            Spec::Dir("Lisbon 2025".into(), trips),
        ]
    }

    fn pictures(&mut self) -> Vec<Spec> {
        let mut years = Vec::new();
        for (year, total) in [(2023, 3_100 * MB), (2024, 4_600 * MB), (2025, 5_200 * MB)] {
            let photos = self.files(900, total, 90, |rng, _| {
                format!(
                    "IMG_{year}{:04}_{:06}.jpg",
                    rng.below(1231),
                    rng.below(240_000)
                )
            });
            years.push(Spec::Dir(year.to_string(), photos));
        }
        let shots = self.files(400, 600 * MB, 10, |_, i| format!("Screenshot_{i:04}.png"));
        years.push(Spec::Dir("Screenshots".into(), shots));
        years
    }

    fn music(&mut self) -> Vec<Spec> {
        [
            "Daft Punk",
            "Radiohead",
            "Bonobo",
            "Khruangbin",
            "Nils Frahm",
            "Massive Attack",
        ]
        .into_iter()
        .map(|artist| {
            let tracks = self.files(30, 1_200 * MB, 400, |_, i| {
                format!("{:02} Track.flac", i + 1)
            });
            Spec::Dir(artist.into(), tracks)
        })
        .collect()
    }

    fn documents(&mut self) -> Vec<Spec> {
        let invoices = self.files(140, 90 * MB, 60, |_, i| format!("invoice-{i:04}.pdf"));
        let papers = self.files(80, 900 * MB, 120, |_, i| format!("paper-{i:03}.pdf"));
        let design = self.files(30, 2_600 * MB, 40, |_, i| format!("mockup-{i:02}.fig"));
        vec![
            Spec::Dir("Invoices".into(), invoices),
            Spec::Dir("Papers".into(), papers),
            Spec::Dir("Design".into(), design),
            self.file("taxes-2025.ods", 400 * KB, 150),
        ]
    }

    fn downloads(&mut self) -> Vec<Spec> {
        let mut files = vec![
            self.file("ubuntu-24.04.2-desktop-amd64.iso", 6_100 * MB, 95),
            self.file("Fedora-Workstation-Live-42.iso", 2_300 * MB, 150),
            self.file("Obsidian-1.8.9.AppImage", 120 * MB, 70),
            self.file("slack-desktop-4.43.51-amd64.deb", 85 * MB, 110),
            self.file("vscode-1.99.3.rpm", 140 * MB, 160),
        ];
        files.extend(self.files(60, 400 * MB, 5, |_, i| format!("document-{i:03}.pdf")));
        files
    }
}

/// Turns the specs into tree nodes, level by level, so the children of a folder get
/// consecutive ids, as the scanner gives them.
fn build(home: &Path, specs: Vec<Spec>) -> Tree {
    let mut tree = Tree::new(home);
    let mut queue: VecDeque<(NodeId, Vec<Spec>)> = VecDeque::from([(ROOT, specs)]);
    while let Some((dir, kids)) = queue.pop_front() {
        let first = tree.len() as NodeId;
        let entries: Vec<EntryData> = kids
            .iter()
            .map(|spec| match spec {
                Spec::Dir(name, _) => EntryData {
                    name: name.as_bytes().to_vec(),
                    kind: Kind::Dir,
                    size: 0,
                    mtime: 0,
                    flags: 0,
                },
                Spec::File(name, size, mtime) => EntryData {
                    name: name.as_bytes().to_vec(),
                    kind: Kind::File,
                    size: *size,
                    mtime: *mtime,
                    flags: 0,
                },
            })
            .collect();
        tree.set_children(dir, first, &entries);
        tree.set_flag(dir, FLAG_COMPLETE);
        for (i, spec) in kids.into_iter().enumerate() {
            if let Spec::Dir(_, grandkids) = spec {
                queue.push_back((first + i as NodeId, grandkids));
            }
        }
    }
    tree
}

/// Appends one line per folder and file of `specs`, parents before their children.
fn write_manifest(specs: &[Spec], parent: &str, out: &mut String) {
    for spec in specs {
        match spec {
            Spec::Dir(name, kids) => {
                let path = format!("{parent}{name}");
                out.push_str(&format!("D\t{path}\n"));
                write_manifest(kids, &format!("{path}/"), out);
            }
            Spec::File(name, size, mtime) => {
                out.push_str(&format!("F\t{parent}{name}\t{size}\t{mtime}\n"));
            }
        }
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let (home, config, cache, manifest) = match args.as_slice() {
        [home, config, cache] => (home, config, cache, None),
        [home, config, cache, manifest] => (home, config, cache, Some(manifest)),
        _ => {
            eprintln!("usage: demo_home <home> <config dir> <cache dir> [manifest]");
            std::process::exit(2);
        }
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX));

    let mut generator = Gen {
        rng: Rng(0x005e_ed0f_7a40),
        now,
    };
    let specs = generator.home();
    if let Some(manifest) = manifest {
        let mut lines = String::new();
        write_manifest(&specs, "", &mut lines);
        if let Err(error) = std::fs::write(manifest, lines) {
            eprintln!("demo_home: {error}");
            std::process::exit(1);
        }
    }
    let tree = build(home, specs);
    let root = tree.node(ROOT);
    let (size, files) = (root.size, root.files);

    let scans = cache.join("vano").join("scans");
    let config = config.join("vano");
    let written = std::fs::create_dir_all(&scans)
        .and_then(|()| std::fs::write(scans.join("demo.vtree"), persist::encode(&tree)))
        .and_then(|()| std::fs::create_dir_all(&config))
        .and_then(|()| {
            std::fs::write(config.join("recents.json"), recents(home, size, files, now))
        });
    if let Err(error) = written {
        eprintln!("demo_home: {error}");
        std::process::exit(1);
    }

    println!(
        "{} nodes, {} files, {:.1} GB",
        tree.len(),
        files,
        size as f64 / GB as f64
    );
    let ctx = Context {
        home: Some(home.clone()),
        now,
        os: Os::Linux,
    };
    for rec in evaluate(&tree, &ctx) {
        println!(
            "{:?} {:<22} {:>7.1} GB  {}",
            rec.status,
            rec.rule,
            rec.size as f64 / GB as f64,
            rec.display_path
        );
    }
}

/// The recent scans of the home screen: the demo home first (the one `VANO_OPEN=results` opens),
/// then two older scans that are only listed.
fn recents(home: &Path, size: u64, files: u32, now: u32) -> String {
    let home = home
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let entry = |kind: &str, path: &str, size: u64, files: u64, ago: u32, cache: &str| {
        format!(
            "  {{\"target\": {{\"kind\": \"{kind}\", \"path\": \"{path}\"}}, \"size\": {size}, \
             \"files\": {files}, \"scanned_at\": {}, \"seconds\": 0.41, \"cache\": \"{cache}\"}}",
            now - ago
        )
    };
    let list = [
        entry("home", &home, size, u64::from(files), 60, "demo.vtree"),
        entry(
            "disk",
            "/",
            288 * GB,
            1_904_211,
            3 * DAY,
            "missing-disk.vtree",
        ),
        entry(
            "folder",
            &format!("{home}/dev"),
            61 * GB,
            301_877,
            9 * DAY,
            "missing-dev.vtree",
        ),
    ];
    format!("[\n{}\n]\n", list.join(",\n"))
}
