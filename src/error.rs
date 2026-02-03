//! Error types for CLAP plugin hosting.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for CLAP operations.
pub type Result<T> = std::result::Result<T, ClapError>;

/// Stage at which plugin loading failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStage {
    /// Failed to open the plugin file/bundle
    Opening,
    /// Failed to get plugin factory
    Factory,
    /// Failed to create plugin instance
    Instantiation,
    /// Failed to initialize plugin
    Initialization,
    /// Failed to activate plugin
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

/// Errors that can occur during CLAP plugin operations.
#[derive(Debug, Error)]
pub enum ClapError {
    /// Plugin loading failed at a specific stage
    #[error("Failed to load plugin at {path}: {stage} - {reason}")]
    LoadFailed {
        path: PathBuf,
        stage: LoadStage,
        reason: String,
    },

    /// Plugin processing error
    #[error("Processing error: {0}")]
    ProcessError(String),

    /// Plugin state error
    #[error("State error: {0}")]
    StateError(String),

    /// Plugin not activated
    #[error("Plugin not activated")]
    NotActivated,

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// GUI error
    #[error("GUI error: {0}")]
    GuiError(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
