//! Error types for CLAP plugin hosting.

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ClapError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStage {
    Opening,
    Factory,
    Instantiation,
    Initialization,
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

#[derive(Debug, Error)]
pub enum ClapError {
    #[error("Failed to load plugin at {path}: {stage} - {reason}")]
    LoadFailed {
        path: PathBuf,
        stage: LoadStage,
        reason: String,
    },

    #[error("Processing error: {0}")]
    ProcessError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("Plugin not activated")]
    NotActivated,

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("GUI error: {0}")]
    GuiError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
