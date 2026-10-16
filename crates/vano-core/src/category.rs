//! What a node mostly holds, which colors it in the treemap's Category mode.

/// The six categories of the design (`--cat-*` tokens), in legend order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Category {
    /// Documents, photos, music, videos.
    Media,
    /// Anything without a better match.
    #[default]
    Other,
    /// Source code projects (folders with a `.git`, a manifest, ...).
    Code,
    /// Mobile and platform SDKs, emulators, simulators.
    Sdk,
    /// Local AI model weights.
    Ai,
    /// Package manager and tool caches.
    Caches,
}

impl Category {
    /// Every category, in legend order.
    pub const ALL: [Category; 6] = [
        Category::Media,
        Category::Other,
        Category::Code,
        Category::Sdk,
        Category::Ai,
        Category::Caches,
    ];

    /// A stable id, used for translation keys and logs.
    pub fn id(self) -> &'static str {
        match self {
            Category::Media => "media",
            Category::Other => "other",
            Category::Code => "code",
            Category::Sdk => "sdk",
            Category::Ai => "ai",
            Category::Caches => "caches",
        }
    }
}
