//! The single error type every conversion family returns, so the Tauri
//! command layer has one shape to turn into a user-facing message
//! (filename + error detail, per Interaction.md's progress/completion
//! feedback contract) regardless of which engine handled the conversion.

use std::fmt;

/// Why a conversion failed. Every family module (`image`, `video`, `audio`,
/// `document`, `archive`) returns this from its `convert` function.
#[derive(Debug)]
pub enum ConvertError {
    /// No route from `from` to `to` exists in the format matrix at all
    /// (should be unreachable if the frontend only ever offers targets
    /// from `super::supported_targets`, but kept as a real error rather
    /// than a panic since it's also reachable from a stale/forged
    /// frontend call).
    UnsupportedConversion {
        /// The source file's extension.
        from: String,
        /// The extension that was requested.
        to: String,
    },
    /// The route exists in principle but this build/platform can't
    /// actually perform it — e.g. the FFmpeg sidecar binary wasn't found,
    /// or a source file needs an engine that isn't compiled in.
    EngineUnavailable(String),
    /// The source path doesn't exist, isn't a file, or couldn't be read.
    InvalidSource(String),
    /// Reading or writing a file failed.
    Io(std::io::Error),
    /// The source file's own bytes couldn't be decoded as the format its
    /// extension claims (corrupt/truncated file, or an unsupported
    /// sub-variant of the format).
    Decode(String),
    /// Any other engine-specific failure not covered above, with a
    /// human-readable detail string safe to surface directly to the user.
    Other(String),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::UnsupportedConversion { from, to } => {
                write!(f, "cannot convert .{from} to .{to}")
            }
            ConvertError::EngineUnavailable(detail) => write!(f, "conversion engine unavailable: {detail}"),
            ConvertError::InvalidSource(detail) => write!(f, "invalid source file: {detail}"),
            ConvertError::Io(error) => write!(f, "file I/O error: {error}"),
            ConvertError::Decode(detail) => write!(f, "could not read source file: {detail}"),
            ConvertError::Other(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<std::io::Error> for ConvertError {
    fn from(error: std::io::Error) -> Self {
        ConvertError::Io(error)
    }
}
