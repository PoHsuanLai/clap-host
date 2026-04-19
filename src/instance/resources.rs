//! CLAP resource-directory extension methods.

use super::ext;
use super::ClapInstance;
use crate::cstr_to_string;

impl ClapInstance {
    pub fn resource_set_directory(&self, path: &str, is_shared: bool) {
        let Some(ext) = (unsafe { ext::opt(self.extensions.system.resource_directory) }) else {
            return;
        };
        if let Some(f) = ext.set_directory {
            if let Ok(cstr) = std::ffi::CString::new(path) {
                unsafe { f(self.plugin.as_ptr(), cstr.as_ptr(), is_shared) };
            }
        }
    }

    pub fn resource_collect(&self, all: bool) {
        let Some(ext) = (unsafe { ext::opt(self.extensions.system.resource_directory) }) else {
            return;
        };
        if let Some(f) = ext.collect {
            unsafe { f(self.plugin.as_ptr(), all) };
        }
    }

    pub fn resource_files_count(&self) -> u32 {
        let Some(ext) = (unsafe { ext::opt(self.extensions.system.resource_directory) }) else {
            return 0;
        };
        ext.get_files_count
            .map(|f| unsafe { f(self.plugin.as_ptr()) })
            .unwrap_or(0)
    }

    pub fn resource_get_file_path(&self, index: u32) -> Option<String> {
        let ext = unsafe { ext::opt(self.extensions.system.resource_directory) }?;
        let get_fn = ext.get_file_path?;
        let mut buf = [0i8; 4096];
        let result =
            unsafe { get_fn(self.plugin.as_ptr(), index, buf.as_mut_ptr(), buf.len() as u32) };
        if result < 0 {
            return None;
        }
        Some(unsafe { cstr_to_string(buf.as_ptr()) })
    }
}
