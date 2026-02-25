//! State save/load methods for ClapInstance.

use super::ClapInstance;
use crate::error::{ClapError, Result};
use crate::host::{InputStream, OutputStream};
use crate::types::StateContext;
use clap_sys::factory::preset_discovery::CLAP_PRESET_DISCOVERY_LOCATION_FILE;
use std::path::Path;
use std::ptr;

impl ClapInstance {
    pub fn save_state(&self) -> Result<Vec<u8>> {
        if self.extensions.state.state.is_null() {
            return Err(ClapError::StateError("No state extension".to_string()));
        }
        let state_ext = unsafe { &*self.extensions.state.state };
        let save_fn = state_ext
            .save
            .ok_or_else(|| ClapError::StateError("No save function".to_string()))?;

        let mut stream = OutputStream::new();
        if !unsafe { save_fn(self.plugin, stream.as_raw()) } {
            return Err(ClapError::StateError("Save failed".to_string()));
        }

        Ok(stream.into_data())
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        if self.extensions.state.state.is_null() {
            return Err(ClapError::StateError("No state extension".to_string()));
        }
        let state_ext = unsafe { &*self.extensions.state.state };
        let load_fn = state_ext
            .load
            .ok_or_else(|| ClapError::StateError("No load function".to_string()))?;

        let mut stream = InputStream::new(data);
        if !unsafe { load_fn(self.plugin, stream.as_raw()) } {
            return Err(ClapError::StateError("Load failed".to_string()));
        }

        Ok(())
    }

    /// Save state with context. Falls back to regular save_state if
    /// the plugin doesn't support CLAP_EXT_STATE_CONTEXT.
    pub fn save_state_with_context(&self, context: StateContext) -> Result<Vec<u8>> {
        if !self.extensions.state.context.is_null() {
            let ext = unsafe { &*self.extensions.state.context };
            if let Some(save_fn) = ext.save {
                let context_type = context.into();
                let mut stream = OutputStream::new();
                if unsafe { save_fn(self.plugin, stream.as_raw(), context_type) } {
                    return Ok(stream.into_data());
                }
            }
        }
        // Fallback to regular state save
        self.save_state()
    }

    /// Load state with context. Falls back to regular load_state if
    /// the plugin doesn't support CLAP_EXT_STATE_CONTEXT.
    pub fn load_state_with_context(&mut self, data: &[u8], context: StateContext) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        if !self.extensions.state.context.is_null() {
            let ext = unsafe { &*self.extensions.state.context };
            if let Some(load_fn) = ext.load {
                let context_type = context.into();
                let mut stream = InputStream::new(data);
                if unsafe { load_fn(self.plugin, stream.as_raw(), context_type) } {
                    return Ok(());
                }
            }
        }
        // Fallback to regular state load
        self.load_state(data)
    }

    pub fn supports_state_context(&self) -> bool {
        !self.extensions.state.context.is_null()
    }

    pub fn load_preset(&mut self, path: &Path) -> Result<()> {
        if self.extensions.state.preset_load.is_null() {
            return Err(ClapError::StateError(
                "No preset-load extension".to_string(),
            ));
        }
        let ext = unsafe { &*self.extensions.state.preset_load };
        let from_location_fn = ext
            .from_location
            .ok_or_else(|| ClapError::StateError("No from_location function".to_string()))?;
        let location = std::ffi::CString::new(path.to_string_lossy().as_ref())
            .map_err(|e| ClapError::StateError(format!("Invalid path: {}", e)))?;
        if unsafe {
            from_location_fn(
                self.plugin,
                CLAP_PRESET_DISCOVERY_LOCATION_FILE,
                location.as_ptr(),
                ptr::null(),
            )
        } {
            Ok(())
        } else {
            Err(ClapError::StateError("Preset load failed".to_string()))
        }
    }
}
