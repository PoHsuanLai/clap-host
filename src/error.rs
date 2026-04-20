//! Error types for CLAP plugin hosting.

use std::path::PathBuf;
use thiserror::Error;

/// Convenience alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, ClapError>;

/// Which phase of plugin loading failed, to help diagnose [`ClapError::LoadFailed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStage {
    /// Opening the plugin library (`dlopen`/`LoadLibrary`) or reading `clap_entry`.
    Opening,
    /// Retrieving the plugin factory from the entry.
    Factory,
    /// Creating the plugin instance from the factory.
    Instantiation,
    /// Calling `clap_plugin.init()`.
    Initialization,
    /// Calling `clap_plugin.activate()`.
    Activation,
}

impl std::fmt::Display for LoadStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Opening => write!(f, "opening"),
            Self::Factory => write!(f, "factory"),
            Self::Instantiation => write!(f, "instantiation"),
            Self::Initialization => write!(f, "initialization"),
            Self::Activation => write!(f, "activation"),
        }
    }
}

/// All error conditions reported by the CLAP host.
#[derive(Debug, Error)]
pub enum ClapError {
    /// The plugin could not be loaded. `stage` pinpoints which step failed.
    #[error("Failed to load plugin at {path}: {stage} - {reason}")]
    LoadFailed {
        path: PathBuf,
        stage: LoadStage,
        reason: String,
    },

    /// Audio processing failed — the plugin returned `CLAP_PROCESS_ERROR`
    /// or a 64-bit buffer was passed to a 32-bit-only plugin.
    #[error("Processing error: {0}")]
    ProcessError(String),

    /// Saving or loading plugin state failed.
    #[error("State error: {0}")]
    StateError(String),

    /// An operation that requires an active plugin was called on an
    /// inactive instance.
    #[error("Plugin not activated")]
    NotActivated,

    /// A parameter ID or value was rejected by the plugin.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Editor/GUI creation, resize, or teardown failed.
    #[error("GUI error: {0}")]
    GuiError(String),

    /// Underlying IO failure (file system, stream).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
