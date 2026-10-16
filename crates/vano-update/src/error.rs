//! The error type of this crate.

/// Everything that can go wrong while checking, downloading or installing an update.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// The server could not be reached, or the connection broke or timed out.
    #[error("network error: {0}")]
    Network(String),

    /// The server answered with an error status.
    #[error("the server answered with HTTP status {0}")]
    Status(u16),

    /// The manifest is too big or is not valid JSON of the expected shape.
    #[error("could not read the update manifest: {0}")]
    Parse(String),

    /// A version is not valid semver.
    #[error("invalid version {0:?}")]
    BadVersion(String),

    /// The asset URL has no usable file name.
    #[error("the asset URL has no valid file name: {0}")]
    BadAssetName(String),

    /// The downloaded file does not match the SHA-256 of the manifest. It was deleted.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    Checksum {
        /// Hash promised by the manifest.
        expected: String,
        /// Hash of what was downloaded.
        actual: String,
    },

    /// A filesystem or process failure. `macos_stage` reports a folder that cannot be written as
    /// kind `PermissionDenied`.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
