//! The `latest.json` format. Unknown fields and platforms are ignored.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Manifest {
    pub version: String,
    pub notes_url: String,
    #[serde(default)]
    pub platforms: HashMap<String, Asset>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Asset {
    pub url: String,
    pub sha256: String,
}
