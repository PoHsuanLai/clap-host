//! CLAP plugin instance.

mod audio;
mod extensions;
mod params;
mod polling;
mod ports;
mod state;

pub use audio::{ClapSample, ProcessContext, ProcessOutput};
pub use params::ParamMapping;

use crate::cstr_to_string;
use crate::error::{ClapError, LoadStage, Result};
use crate::host::{ClapHost, HostState};
use crate::types::PluginInfo;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_SUPPORTS_64BITS,
};
use clap_sys::plugin::clap_plugin;
use extensions::ExtensionCache;
use std::collections::HashMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Global registry for CLAP entry init/deinit lifecycle.
///
/// The CLAP spec requires `clap_entry.init()` to be called once when a library
/// is first loaded and `clap_entry.deinit()` once when it's finally unloaded.
/// Many plugins do not tolerate repeated init/deinit cycles within the same
/// process — their global state becomes corrupted. This registry ensures
/// `init()` is called exactly once per library path for the lifetime of the
/// process. `deinit()` is intentionally NOT called, matching real-world DAW
/// behavior where plugins run in subprocesses that exit cleanly.
static ENTRY_REGISTRY: Mutex<Option<HashMap<PathBuf, bool>>> = Mutex::new(None);

/// RAII guard for CLAP entry lifetime. Does not call deinit on drop — the
/// entry stays initialized for the lifetime of the process.
struct EntryGuard {
    _path: PathBuf,
}

/// Register a CLAP entry for the given path.
/// Calls `init_fn` only on the first load of a given library. Subsequent
/// loads of the same library skip init (the entry is already initialized).
fn entry_registry_acquire(
    path: &Path,
    init_fn: unsafe extern "C" fn(*const i8) -> bool,
    path_cstr: &std::ffi::CString,
) -> std::result::Result<EntryGuard, String> {
    let mut registry = ENTRY_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let map = registry.get_or_insert_with(HashMap::new);

    if !map.contains_key(path) {
        if !unsafe { init_fn(path_cstr.as_ptr()) } {
            return Err("Entry init failed".to_string());
        }
        map.insert(path.to_path_buf(), true);
    }

    Ok(EntryGuard {
        _path: path.to_path_buf(),
    })
}

pub struct ClapInstance {
    plugin: *const clap_plugin,
    // IMPORTANT: Drop order matters! Fields are dropped top-to-bottom.
    // _entry_guard must be dropped BEFORE _library so that deinit() is
    // called while the library is still loaded in memory.
    _entry_guard: EntryGuard,
    _library: libloading::Library,
    _host: Box<ClapHost>,
    host_state: Arc<HostState>,
    extensions: ExtensionCache,
    info: PluginInfo,
    supports_f64: bool,
    sample_rate: f64,
    max_frames: u32,
    is_active: bool,
    is_processing: bool,
    /// Per-port channel counts for input ports (e.g. [2] for stereo, [2, 2] for two stereo ports).
    input_port_channels: Vec<u32>,
    /// Per-port channel counts for output ports.
    output_port_channels: Vec<u32>,
}

// Safety: CLAP plugins are designed to be called from a single thread
unsafe impl Send for ClapInstance {}

impl ClapInstance {
    pub fn load(path: impl AsRef<Path>, sample_rate: f64, max_frames: u32) -> Result<Self> {
        let bundle_path = path.as_ref();
        // On macOS, .clap plugins are bundles (directories). Resolve to the
        // actual binary at Contents/MacOS/<stem> for dlopen, but keep the
        // original bundle path for clap_plugin_entry.init() per CLAP spec.
        let resolved = resolve_bundle_path(bundle_path);
        let load_path = resolved.as_deref().unwrap_or(bundle_path);

        let library = unsafe {
            libloading::Library::new(load_path).map_err(|e| ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Opening,
                reason: format!("Failed to load library: {}", e),
            })?
        };

        // clap_entry is a static exported struct (not a function pointer).
        // get::<*const T> yields a Symbol whose Deref gives *const T.
        // We copy the pointer value out so the Symbol borrow can end,
        // then convert to a reference that lives as long as _library.
        let entry_struct: &clap_plugin_entry = unsafe {
            let sym = library
                .get::<*const clap_plugin_entry>(b"clap_entry\0")
                .map_err(|e| ClapError::LoadFailed {
                    path: bundle_path.to_path_buf(),
                    stage: LoadStage::Opening,
                    reason: format!("No clap_entry symbol: {}", e),
                })?;
            &*(*sym)
        };

        let init_fn = entry_struct.init.ok_or_else(|| ClapError::LoadFailed {
            path: bundle_path.to_path_buf(),
            stage: LoadStage::Opening,
            reason: "No init function".to_string(),
        })?;

        // Pass the original bundle path to init(), not the resolved binary path
        let path_cstr =
            std::ffi::CString::new(bundle_path.to_string_lossy().as_ref()).map_err(|e| {
                ClapError::LoadFailed {
                    path: bundle_path.to_path_buf(),
                    stage: LoadStage::Opening,
                    reason: format!("Invalid path: {}", e),
                }
            })?;

        // Use the entry registry to ensure init is called exactly once per
        // library. deinit is intentionally skipped — many plugins don't
        // tolerate repeated init/deinit cycles in the same process.
        let entry_guard =
            entry_registry_acquire(bundle_path, init_fn, &path_cstr).map_err(|reason| {
                ClapError::LoadFailed {
                    path: bundle_path.to_path_buf(),
                    stage: LoadStage::Opening,
                    reason,
                }
            })?;

        let host_state = Arc::new(HostState::new());
        let host = Box::new(ClapHost::new(host_state.clone()));

        let get_factory_fn = entry_struct
            .get_factory
            .ok_or_else(|| ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_factory function".to_string(),
            })?;

        let factory_ptr = unsafe {
            get_factory_fn(clap_sys::factory::plugin_factory::CLAP_PLUGIN_FACTORY_ID.as_ptr())
        };

        if factory_ptr.is_null() {
            return Err(ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugin factory".to_string(),
            });
        }

        let factory = unsafe {
            &*(factory_ptr as *const clap_sys::factory::plugin_factory::clap_plugin_factory)
        };

        let get_count_fn = factory
            .get_plugin_count
            .ok_or_else(|| ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_plugin_count function".to_string(),
            })?;

        let plugin_count = unsafe { get_count_fn(factory_ptr as *const _) };
        if plugin_count == 0 {
            return Err(ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugins in factory".to_string(),
            });
        }

        let get_desc_fn = factory
            .get_plugin_descriptor
            .ok_or_else(|| ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_plugin_descriptor function".to_string(),
            })?;

        let desc_ptr = unsafe { get_desc_fn(factory_ptr as *const _, 0) };
        if desc_ptr.is_null() {
            return Err(ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugin descriptor".to_string(),
            });
        }

        let descriptor = unsafe { &*desc_ptr };

        let plugin_id = unsafe { CStr::from_ptr(descriptor.id) }
            .to_string_lossy()
            .into_owned();
        let name = unsafe { CStr::from_ptr(descriptor.name) }
            .to_string_lossy()
            .into_owned();
        let vendor = unsafe { CStr::from_ptr(descriptor.vendor) }
            .to_string_lossy()
            .into_owned();
        let version = unsafe { CStr::from_ptr(descriptor.version) }
            .to_string_lossy()
            .into_owned();
        let url = unsafe { cstr_to_string(descriptor.url) };
        let description = unsafe { cstr_to_string(descriptor.description) };

        let features = if descriptor.features.is_null() {
            Vec::new()
        } else {
            let mut features = Vec::new();
            let mut ptr = descriptor.features;
            const MAX_FEATURES: usize = 256;
            unsafe {
                while !(*ptr).is_null() && features.len() < MAX_FEATURES {
                    features.push(CStr::from_ptr(*ptr).to_string_lossy().into_owned());
                    ptr = ptr.add(1);
                }
            }
            features
        };

        let plugin_id_cstr =
            std::ffi::CString::new(plugin_id.as_str()).map_err(|e| ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Instantiation,
                reason: format!("Invalid plugin ID: {}", e),
            })?;

        let create_fn = factory.create_plugin.ok_or_else(|| ClapError::LoadFailed {
            path: bundle_path.to_path_buf(),
            stage: LoadStage::Instantiation,
            reason: "No create_plugin function".to_string(),
        })?;

        let plugin = unsafe {
            create_fn(
                factory_ptr as *const _,
                host.as_raw(),
                plugin_id_cstr.as_ptr(),
            )
        };

        if plugin.is_null() {
            return Err(ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Instantiation,
                reason: "Failed to create plugin instance".to_string(),
            });
        }

        let plugin_ref = unsafe { &*plugin };
        let plugin_init_fn = plugin_ref.init.ok_or_else(|| ClapError::LoadFailed {
            path: bundle_path.to_path_buf(),
            stage: LoadStage::Initialization,
            reason: "No plugin init function".to_string(),
        })?;

        if !unsafe { plugin_init_fn(plugin) } {
            return Err(ClapError::LoadFailed {
                path: bundle_path.to_path_buf(),
                stage: LoadStage::Initialization,
                reason: "Plugin init failed".to_string(),
            });
        }

        let extensions = ExtensionCache::query(plugin);

        // Discover per-port channel counts from the audio-ports extension
        let input_port_channels = Self::port_channels_static(plugin, extensions.audio.ports, true);
        let output_port_channels =
            Self::port_channels_static(plugin, extensions.audio.ports, false);

        let audio_inputs: usize = input_port_channels.iter().map(|&c| c as usize).sum();
        let audio_outputs: usize = output_port_channels.iter().map(|&c| c as usize).sum();

        // Check if any output port advertises CLAP_AUDIO_PORT_SUPPORTS_64BITS
        let supports_f64 = Self::check_f64_support(plugin, extensions.audio.ports);

        let info = PluginInfo {
            id: plugin_id,
            name,
            vendor,
            version,
            url,
            description,
            features,
            audio_inputs: if audio_inputs > 0 { audio_inputs } else { 2 },
            audio_outputs: if audio_outputs > 0 { audio_outputs } else { 2 },
        };

        // Default to single stereo port if no port info available
        let input_port_channels = if input_port_channels.is_empty() {
            vec![2]
        } else {
            input_port_channels
        };
        let output_port_channels = if output_port_channels.is_empty() {
            vec![2]
        } else {
            output_port_channels
        };

        Ok(Self {
            plugin,
            _entry_guard: entry_guard,
            _library: library,
            _host: host,
            host_state,
            extensions,
            info,
            supports_f64,
            sample_rate,
            max_frames,
            is_active: false,
            is_processing: false,
            input_port_channels,
            output_port_channels,
        })
    }

    /// Get per-port channel counts (used during load before self exists).
    fn port_channels_static(
        plugin: *const clap_plugin,
        audio_ports: *const clap_plugin_audio_ports,
        is_input: bool,
    ) -> Vec<u32> {
        if audio_ports.is_null() {
            return Vec::new();
        }
        let ext = unsafe { &*audio_ports };
        let count_fn = match ext.count {
            Some(f) => f,
            None => return Vec::new(),
        };
        let get_fn = match ext.get {
            Some(f) => f,
            None => return Vec::new(),
        };
        let count = unsafe { count_fn(plugin, is_input) };
        let mut ports = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
            if unsafe { get_fn(plugin, i, is_input, &mut info) } {
                ports.push(info.channel_count);
            }
        }
        ports
    }

    /// Check if any output port advertises CLAP_AUDIO_PORT_SUPPORTS_64BITS.
    fn check_f64_support(
        plugin: *const clap_plugin,
        audio_ports: *const clap_plugin_audio_ports,
    ) -> bool {
        if audio_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*audio_ports };
        let count_fn = match ext.count {
            Some(f) => f,
            None => return false,
        };
        let get_fn = match ext.get {
            Some(f) => f,
            None => return false,
        };
        let count = unsafe { count_fn(plugin, false) };
        for i in 0..count {
            let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
            if unsafe { get_fn(plugin, i, false, &mut info) }
                && (info.flags & CLAP_AUDIO_PORT_SUPPORTS_64BITS) != 0
            {
                return true;
            }
        }
        false
    }

    pub fn supports_f64(&self) -> bool {
        self.supports_f64
    }

    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    pub fn max_frames(&self) -> u32 {
        self.max_frames
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    pub fn activate(&mut self) -> Result<()> {
        if self.is_active {
            return Ok(());
        }

        let plugin_ref = unsafe { &*self.plugin };
        let activate_fn = plugin_ref.activate.ok_or(ClapError::NotActivated)?;

        if !unsafe { activate_fn(self.plugin, self.sample_rate, 1, self.max_frames) } {
            return Err(ClapError::LoadFailed {
                path: std::path::PathBuf::new(),
                stage: LoadStage::Activation,
                reason: "Activate failed".to_string(),
            });
        }

        self.is_active = true;
        Ok(())
    }

    pub fn deactivate(&mut self) {
        if !self.is_active {
            return;
        }

        if self.is_processing {
            self.stop_processing();
        }

        let plugin_ref = unsafe { &*self.plugin };
        if let Some(deactivate_fn) = plugin_ref.deactivate {
            unsafe { deactivate_fn(self.plugin) };
        }

        self.is_active = false;
    }

    pub fn start_processing(&mut self) -> Result<()> {
        if !self.is_active {
            self.activate()?;
        }

        if self.is_processing {
            return Ok(());
        }

        let plugin_ref = unsafe { &*self.plugin };
        if let Some(start_fn) = plugin_ref.start_processing {
            if !unsafe { start_fn(self.plugin) } {
                return Err(ClapError::ProcessError(
                    "Start processing failed".to_string(),
                ));
            }
        }

        self.is_processing = true;
        Ok(())
    }

    pub fn stop_processing(&mut self) {
        if !self.is_processing {
            return;
        }

        let plugin_ref = unsafe { &*self.plugin };
        if let Some(stop_fn) = plugin_ref.stop_processing {
            unsafe { stop_fn(self.plugin) };
        }

        if let Ok(mut guard) = self.host_state.audio_thread_id.lock() {
            *guard = None;
        }

        self.is_processing = false;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) -> &mut Self {
        if (self.sample_rate - sample_rate).abs() < f64::EPSILON {
            return self; // No change — skip deactivate/reactivate cycle
        }
        if self.is_active {
            self.deactivate();
        }
        self.sample_rate = sample_rate;
        self
    }
}

impl Drop for ClapInstance {
    fn drop(&mut self) {
        let plugin_ref = unsafe { &*self.plugin };

        if self.is_processing {
            if let Some(stop_fn) = plugin_ref.stop_processing {
                unsafe { stop_fn(self.plugin) };
            }
        }

        if self.is_active {
            if let Some(deactivate_fn) = plugin_ref.deactivate {
                unsafe { deactivate_fn(self.plugin) };
            }
        }

        if let Some(destroy_fn) = plugin_ref.destroy {
            unsafe { destroy_fn(self.plugin) };
        }

        // entry.deinit() is intentionally NOT called. The ENTRY_REGISTRY
        // keeps entries initialized for the process lifetime. Many plugins
        // corrupt global state when init/deinit are called repeatedly.
    }
}

/// On macOS, `.clap` plugins are bundles (directories). Resolve to the actual
/// binary at `<bundle>/Contents/MacOS/<stem>`. Returns `None` if the path is
/// already a file or we're not on macOS.
fn resolve_bundle_path(path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if path.is_dir() {
            let stem = path.file_stem()?;
            let binary = path.join("Contents").join("MacOS").join(stem);
            if binary.is_file() {
                return Some(binary);
            }
        }
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::polling::{context_menu_builder_add_item, context_menu_builder_supports};
    use crate::types::ContextMenuItem;
    use clap_sys::ext::context_menu::{
        clap_context_menu_builder, clap_context_menu_entry, CLAP_CONTEXT_MENU_ITEM_ENTRY,
        CLAP_CONTEXT_MENU_ITEM_SEPARATOR,
    };
    use std::ffi::c_void;

    #[test]
    fn test_context_menu_builder_null_builder() {
        unsafe {
            let result = context_menu_builder_add_item(
                std::ptr::null(),
                CLAP_CONTEXT_MENU_ITEM_SEPARATOR,
                std::ptr::null(),
            );
            assert!(!result);
        }
    }

    #[test]
    fn test_context_menu_builder_null_ctx() {
        let builder = clap_context_menu_builder {
            ctx: std::ptr::null_mut(),
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };
        unsafe {
            let result = context_menu_builder_add_item(
                &builder,
                CLAP_CONTEXT_MENU_ITEM_SEPARATOR,
                std::ptr::null(),
            );
            assert!(!result);
        }
    }

    #[test]
    fn test_context_menu_builder_separator() {
        let mut items: Vec<ContextMenuItem> = Vec::new();
        let builder = clap_context_menu_builder {
            ctx: &mut items as *mut Vec<ContextMenuItem> as *mut c_void,
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };
        unsafe {
            let result = context_menu_builder_add_item(
                &builder,
                CLAP_CONTEXT_MENU_ITEM_SEPARATOR,
                std::ptr::null(),
            );
            assert!(result);
        }
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], ContextMenuItem::Separator));
    }

    #[test]
    fn test_context_menu_builder_entry_null_data() {
        let mut items: Vec<ContextMenuItem> = Vec::new();
        let builder = clap_context_menu_builder {
            ctx: &mut items as *mut Vec<ContextMenuItem> as *mut c_void,
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };
        unsafe {
            // Entry with null item_data should return false
            let result = context_menu_builder_add_item(
                &builder,
                CLAP_CONTEXT_MENU_ITEM_ENTRY,
                std::ptr::null(),
            );
            assert!(!result);
        }
        assert!(items.is_empty());
    }

    #[test]
    fn test_context_menu_builder_entry_with_data() {
        let mut items: Vec<ContextMenuItem> = Vec::new();
        let builder = clap_context_menu_builder {
            ctx: &mut items as *mut Vec<ContextMenuItem> as *mut c_void,
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };

        let label = std::ffi::CString::new("Test Entry").unwrap();
        let entry = clap_context_menu_entry {
            label: label.as_ptr(),
            is_enabled: true,
            action_id: 42,
        };

        unsafe {
            let result = context_menu_builder_add_item(
                &builder,
                CLAP_CONTEXT_MENU_ITEM_ENTRY,
                &entry as *const clap_context_menu_entry as *const c_void,
            );
            assert!(result);
        }
        assert_eq!(items.len(), 1);
        match &items[0] {
            ContextMenuItem::Entry {
                label,
                is_enabled,
                action_id,
            } => {
                assert_eq!(label, "Test Entry");
                assert!(*is_enabled);
                assert_eq!(*action_id, 42);
            }
            _ => panic!("Expected Entry"),
        }
    }

    #[test]
    fn test_context_menu_builder_unknown_type() {
        let mut items: Vec<ContextMenuItem> = Vec::new();
        let builder = clap_context_menu_builder {
            ctx: &mut items as *mut Vec<ContextMenuItem> as *mut c_void,
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };
        unsafe {
            let result = context_menu_builder_add_item(&builder, 9999, std::ptr::null());
            assert!(!result);
        }
        assert!(items.is_empty());
    }

    #[test]
    fn test_context_menu_builder_supports() {
        unsafe {
            assert!(context_menu_builder_supports(
                std::ptr::null(),
                CLAP_CONTEXT_MENU_ITEM_ENTRY
            ));
            assert!(context_menu_builder_supports(
                std::ptr::null(),
                CLAP_CONTEXT_MENU_ITEM_SEPARATOR
            ));
            assert!(!context_menu_builder_supports(std::ptr::null(), 9999));
        }
    }
}
