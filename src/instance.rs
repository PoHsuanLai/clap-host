//! CLAP plugin instance.

use crate::error::{ClapError, LoadStage, Result};
use crate::events::{InputEventList, OutputEventList};
use crate::host::{ClapHost, InputStream, OutputStream};
use crate::types::{
    AudioBuffer32, AudioBuffer64, MidiEvent, NoteExpressionValue, ParameterChanges, ParameterFlags,
    ParameterInfo, PluginInfo, TransportInfo,
};
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::clap_event_transport;
use clap_sys::ext::gui::{clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI};
use clap_sys::ext::params::{clap_plugin_params, CLAP_EXT_PARAMS};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR};
use std::ffi::CStr;
use std::path::Path;
use std::ptr;

#[cfg(target_os = "macos")]
use clap_sys::ext::gui::CLAP_WINDOW_API_COCOA;
#[cfg(target_os = "windows")]
use clap_sys::ext::gui::CLAP_WINDOW_API_WIN32;
#[cfg(target_os = "linux")]
use clap_sys::ext::gui::CLAP_WINDOW_API_X11;

/// CLAP plugin instance.
///
/// Represents a loaded and initialized CLAP plugin that can process audio.
pub struct ClapInstance {
    plugin: *const clap_plugin,
    _library: libloading::Library,
    _host: Box<ClapHost>,
    info: PluginInfo,
    sample_rate: f64,
    max_frames: u32,
    is_active: bool,
    is_processing: bool,
}

// Safety: CLAP plugins are designed to be called from a single thread
unsafe impl Send for ClapInstance {}

impl ClapInstance {
    /// Load a CLAP plugin from a file path.
    ///
    /// # Arguments
    /// * `path` - Path to the .clap plugin bundle
    /// * `sample_rate` - Initial sample rate
    /// * `max_frames` - Maximum block size (samples per process call)
    pub fn load(path: impl AsRef<Path>, sample_rate: f64, max_frames: u32) -> Result<Self> {
        let path = path.as_ref();

        // Load the library
        let library = unsafe {
            libloading::Library::new(path).map_err(|e| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Opening,
                reason: format!("Failed to load library: {}", e),
            })?
        };

        // Get entry point
        let entry: libloading::Symbol<unsafe extern "C" fn() -> *const clap_plugin_entry> = unsafe {
            library
                .get(b"clap_entry\0")
                .map_err(|e| ClapError::LoadFailed {
                    path: path.to_path_buf(),
                    stage: LoadStage::Opening,
                    reason: format!("No clap_entry symbol: {}", e),
                })?
        };

        let entry_ptr = unsafe { entry() };
        if entry_ptr.is_null() {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Opening,
                reason: "clap_entry returned null".to_string(),
            });
        }

        let entry_struct = unsafe { &*entry_ptr };

        // Initialize entry
        let init_fn = entry_struct.init.ok_or_else(|| ClapError::LoadFailed {
            path: path.to_path_buf(),
            stage: LoadStage::Opening,
            reason: "No init function".to_string(),
        })?;

        let path_cstr = std::ffi::CString::new(path.to_string_lossy().as_ref())
            .map_err(|e| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Opening,
                reason: format!("Invalid path: {}", e),
            })?;

        if !unsafe { init_fn(path_cstr.as_ptr()) } {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Opening,
                reason: "Entry init failed".to_string(),
            });
        }

        // Create host
        let host = Box::new(ClapHost::default());

        // Get factory
        let get_factory_fn = entry_struct
            .get_factory
            .ok_or_else(|| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_factory function".to_string(),
            })?;

        let factory_ptr = unsafe {
            get_factory_fn(clap_sys::factory::plugin_factory::CLAP_PLUGIN_FACTORY_ID.as_ptr())
        };

        if factory_ptr.is_null() {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugin factory".to_string(),
            });
        }

        let factory =
            unsafe { &*(factory_ptr as *const clap_sys::factory::plugin_factory::clap_plugin_factory) };

        // Get plugin count
        let get_count_fn = factory
            .get_plugin_count
            .ok_or_else(|| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_plugin_count function".to_string(),
            })?;

        let plugin_count = unsafe { get_count_fn(factory_ptr as *const _) };
        if plugin_count == 0 {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugins in factory".to_string(),
            });
        }

        // Get first plugin descriptor
        let get_desc_fn = factory
            .get_plugin_descriptor
            .ok_or_else(|| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No get_plugin_descriptor function".to_string(),
            })?;

        let desc_ptr = unsafe { get_desc_fn(factory_ptr as *const _, 0) };
        if desc_ptr.is_null() {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Factory,
                reason: "No plugin descriptor".to_string(),
            });
        }

        let descriptor = unsafe { &*desc_ptr };

        // Extract plugin info
        let plugin_id = unsafe { CStr::from_ptr(descriptor.id) }
            .to_string_lossy()
            .to_string();
        let name = unsafe { CStr::from_ptr(descriptor.name) }
            .to_string_lossy()
            .to_string();
        let vendor = unsafe { CStr::from_ptr(descriptor.vendor) }
            .to_string_lossy()
            .to_string();
        let version = unsafe { CStr::from_ptr(descriptor.version) }
            .to_string_lossy()
            .to_string();
        let url = if descriptor.url.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(descriptor.url) }
                .to_string_lossy()
                .to_string()
        };
        let description = if descriptor.description.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(descriptor.description) }
                .to_string_lossy()
                .to_string()
        };

        let info = PluginInfo {
            id: plugin_id.clone(),
            name,
            vendor,
            version,
            url,
            description,
            features: Vec::new(), // TODO: Parse features
            audio_inputs: 2,
            audio_outputs: 2,
        };

        // Create plugin instance
        let plugin_id_cstr =
            std::ffi::CString::new(plugin_id.as_str()).map_err(|e| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Instantiation,
                reason: format!("Invalid plugin ID: {}", e),
            })?;

        let create_fn = factory
            .create_plugin
            .ok_or_else(|| ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Instantiation,
                reason: "No create_plugin function".to_string(),
            })?;

        let plugin =
            unsafe { create_fn(factory_ptr as *const _, host.as_raw(), plugin_id_cstr.as_ptr()) };

        if plugin.is_null() {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Instantiation,
                reason: "Failed to create plugin instance".to_string(),
            });
        }

        // Initialize plugin
        let plugin_ref = unsafe { &*plugin };
        let plugin_init_fn = plugin_ref.init.ok_or_else(|| ClapError::LoadFailed {
            path: path.to_path_buf(),
            stage: LoadStage::Initialization,
            reason: "No plugin init function".to_string(),
        })?;

        if !unsafe { plugin_init_fn(plugin) } {
            return Err(ClapError::LoadFailed {
                path: path.to_path_buf(),
                stage: LoadStage::Initialization,
                reason: "Plugin init failed".to_string(),
            });
        }

        Ok(Self {
            plugin,
            _library: library,
            _host: host,
            info,
            sample_rate,
            max_frames,
            is_active: false,
            is_processing: false,
        })
    }

    /// Get plugin info.
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    /// Get the current sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Get the maximum frames per process call.
    pub fn max_frames(&self) -> u32 {
        self.max_frames
    }

    /// Check if the plugin is active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Check if the plugin is processing.
    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    /// Activate the plugin.
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

    /// Deactivate the plugin.
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

    /// Start processing.
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
                return Err(ClapError::ProcessError("Start processing failed".to_string()));
            }
        }

        self.is_processing = true;
        Ok(())
    }

    /// Stop processing.
    pub fn stop_processing(&mut self) {
        if !self.is_processing {
            return;
        }

        let plugin_ref = unsafe { &*self.plugin };
        if let Some(stop_fn) = plugin_ref.stop_processing {
            unsafe { stop_fn(self.plugin) };
        }

        self.is_processing = false;
    }

    /// Set the sample rate (requires deactivation).
    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        if self.is_active {
            self.deactivate();
        }
        self.sample_rate = sample_rate;
    }

    /// Process audio with f32 buffers.
    pub fn process_f32(
        &mut self,
        buffer: &mut AudioBuffer32,
        midi_events: Option<&[MidiEvent]>,
        param_changes: Option<&ParameterChanges>,
        note_expressions: Option<&[NoteExpressionValue]>,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        self.start_processing()?;

        let num_samples = buffer.num_samples as u32;

        // Build input events
        let mut input_events = InputEventList::new();
        if let Some(midi) = midi_events {
            input_events.add_midi_events(midi);
        }
        if let Some(params) = param_changes {
            input_events.add_param_changes(params);
        }
        if let Some(exprs) = note_expressions {
            input_events.add_note_expressions(exprs);
        }
        input_events.sort_by_time();

        let mut output_events = OutputEventList::new();

        // Build audio buffers
        let mut input_ptrs: Vec<*mut f32> = buffer
            .inputs
            .iter()
            .map(|s| s.as_ptr() as *mut f32)
            .collect();
        let mut output_ptrs: Vec<*mut f32> = buffer.outputs.iter_mut().map(|s| s.as_mut_ptr()).collect();

        let mut audio_inputs = clap_sys::audio_buffer::clap_audio_buffer {
            data32: input_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: buffer.inputs.len() as u32,
            latency: 0,
            constant_mask: 0,
        };

        let mut audio_outputs = clap_sys::audio_buffer::clap_audio_buffer {
            data32: output_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: buffer.outputs.len() as u32,
            latency: 0,
            constant_mask: 0,
        };

        self.do_process(
            &mut audio_inputs,
            &mut audio_outputs,
            num_samples,
            &input_events,
            &mut output_events,
            transport,
        )
    }

    /// Process audio with f64 buffers.
    pub fn process_f64(
        &mut self,
        buffer: &mut AudioBuffer64,
        midi_events: Option<&[MidiEvent]>,
        param_changes: Option<&ParameterChanges>,
        note_expressions: Option<&[NoteExpressionValue]>,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        self.start_processing()?;

        let num_samples = buffer.num_samples as u32;

        // Build input events
        let mut input_events = InputEventList::new();
        if let Some(midi) = midi_events {
            input_events.add_midi_events(midi);
        }
        if let Some(params) = param_changes {
            input_events.add_param_changes(params);
        }
        if let Some(exprs) = note_expressions {
            input_events.add_note_expressions(exprs);
        }
        input_events.sort_by_time();

        let mut output_events = OutputEventList::new();

        // Build audio buffers
        let mut input_ptrs: Vec<*mut f64> = buffer
            .inputs
            .iter()
            .map(|s| s.as_ptr() as *mut f64)
            .collect();
        let mut output_ptrs: Vec<*mut f64> = buffer.outputs.iter_mut().map(|s| s.as_mut_ptr()).collect();

        let mut audio_inputs = clap_sys::audio_buffer::clap_audio_buffer {
            data32: ptr::null_mut(),
            data64: input_ptrs.as_mut_ptr(),
            channel_count: buffer.inputs.len() as u32,
            latency: 0,
            constant_mask: 0,
        };

        let mut audio_outputs = clap_sys::audio_buffer::clap_audio_buffer {
            data32: ptr::null_mut(),
            data64: output_ptrs.as_mut_ptr(),
            channel_count: buffer.outputs.len() as u32,
            latency: 0,
            constant_mask: 0,
        };

        self.do_process(
            &mut audio_inputs,
            &mut audio_outputs,
            num_samples,
            &input_events,
            &mut output_events,
            transport,
        )
    }

    fn do_process(
        &mut self,
        audio_inputs: &mut clap_sys::audio_buffer::clap_audio_buffer,
        audio_outputs: &mut clap_sys::audio_buffer::clap_audio_buffer,
        num_samples: u32,
        input_events: &InputEventList,
        output_events: &mut OutputEventList,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        // Build transport
        let clap_transport = transport.map(build_clap_transport);
        let transport_ptr = clap_transport
            .as_ref()
            .map(|t| t as *const _)
            .unwrap_or(ptr::null());

        let steady_time = transport
            .map(|t| (t.song_pos_seconds * self.sample_rate) as i64)
            .unwrap_or(0);

        let process_data = clap_process {
            steady_time,
            frames_count: num_samples,
            transport: transport_ptr,
            audio_inputs,
            audio_outputs,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: input_events.as_raw(),
            out_events: output_events.as_raw_mut(),
        };

        let plugin_ref = unsafe { &*self.plugin };
        let status = if let Some(process_fn) = plugin_ref.process {
            unsafe { process_fn(self.plugin, &process_data) }
        } else {
            CLAP_PROCESS_CONTINUE
        };

        if status == CLAP_PROCESS_ERROR {
            return Err(ClapError::ProcessError("Plugin returned error".to_string()));
        }

        Ok(ProcessOutput {
            midi_events: output_events.to_midi_events(),
            param_changes: output_events.to_param_changes(),
            note_expressions: output_events.to_note_expressions(),
        })
    }

    // Extension accessors

    fn get_params_extension(&self) -> Option<&clap_plugin_params> {
        let plugin_ref = unsafe { &*self.plugin };
        let get_ext = plugin_ref.get_extension?;
        let ext_ptr = unsafe { get_ext(self.plugin, CLAP_EXT_PARAMS.as_ptr()) };
        if ext_ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(ext_ptr as *const clap_plugin_params) })
        }
    }

    fn get_state_extension(&self) -> Option<&clap_plugin_state> {
        let plugin_ref = unsafe { &*self.plugin };
        let get_ext = plugin_ref.get_extension?;
        let ext_ptr = unsafe { get_ext(self.plugin, CLAP_EXT_STATE.as_ptr()) };
        if ext_ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(ext_ptr as *const clap_plugin_state) })
        }
    }

    fn get_gui_extension(&self) -> Option<&clap_plugin_gui> {
        let plugin_ref = unsafe { &*self.plugin };
        let get_ext = plugin_ref.get_extension?;
        let ext_ptr = unsafe { get_ext(self.plugin, CLAP_EXT_GUI.as_ptr()) };
        if ext_ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(ext_ptr as *const clap_plugin_gui) })
        }
    }

    // Parameter methods

    /// Get the number of parameters.
    pub fn parameter_count(&self) -> usize {
        if let Some(params) = self.get_params_extension() {
            if let Some(count_fn) = params.count {
                return unsafe { count_fn(self.plugin) } as usize;
            }
        }
        0
    }

    /// Get a parameter value.
    pub fn get_parameter(&self, id: u32) -> Option<f64> {
        if let Some(params) = self.get_params_extension() {
            if let Some(get_value_fn) = params.get_value {
                let mut value: f64 = 0.0;
                if unsafe { get_value_fn(self.plugin, id, &mut value) } {
                    return Some(value);
                }
            }
        }
        None
    }

    /// Get parameter info.
    pub fn get_parameter_info(&self, index: u32) -> Option<ParameterInfo> {
        let params = self.get_params_extension()?;
        let get_info_fn = params.get_info?;

        let mut info: clap_sys::ext::params::clap_param_info = unsafe { std::mem::zeroed() };

        if !unsafe { get_info_fn(self.plugin, index, &mut info) } {
            return None;
        }

        let name = unsafe { CStr::from_ptr(info.name.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let module = unsafe { CStr::from_ptr(info.module.as_ptr()) }
            .to_string_lossy()
            .to_string();

        Some(ParameterInfo {
            id: info.id,
            name,
            module,
            min_value: info.min_value,
            max_value: info.max_value,
            default_value: info.default_value,
            flags: ParameterFlags {
                is_stepped: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_STEPPED) != 0,
                is_periodic: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_PERIODIC) != 0,
                is_hidden: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_HIDDEN) != 0,
                is_readonly: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_READONLY) != 0,
                is_bypass: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_BYPASS) != 0,
                is_automatable: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_AUTOMATABLE) != 0,
                ..Default::default()
            },
        })
    }

    /// Get all parameter info.
    pub fn get_all_parameters(&self) -> Vec<ParameterInfo> {
        let count = self.parameter_count() as u32;
        (0..count).filter_map(|i| self.get_parameter_info(i)).collect()
    }

    // State methods

    /// Save plugin state.
    pub fn save_state(&self) -> Result<Vec<u8>> {
        let state_ext = self
            .get_state_extension()
            .ok_or_else(|| ClapError::StateError("No state extension".to_string()))?;

        let save_fn = state_ext
            .save
            .ok_or_else(|| ClapError::StateError("No save function".to_string()))?;

        let stream = OutputStream::new();
        if !unsafe { save_fn(self.plugin, stream.as_raw()) } {
            return Err(ClapError::StateError("Save failed".to_string()));
        }

        Ok(stream.into_data())
    }

    /// Load plugin state.
    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let state_ext = self
            .get_state_extension()
            .ok_or_else(|| ClapError::StateError("No state extension".to_string()))?;

        let load_fn = state_ext
            .load
            .ok_or_else(|| ClapError::StateError("No load function".to_string()))?;

        let mut stream = InputStream::new(data);
        if !unsafe { load_fn(self.plugin, stream.as_raw()) } {
            return Err(ClapError::StateError("Load failed".to_string()));
        }

        Ok(())
    }

    // GUI methods

    /// Check if the plugin has a GUI.
    pub fn has_gui(&self) -> bool {
        self.get_gui_extension().is_some()
    }

    /// Open the plugin GUI.
    #[cfg(target_os = "macos")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        let gui = self
            .get_gui_extension()
            .ok_or_else(|| ClapError::GuiError("No GUI extension".to_string()))?;

        if let Some(create_fn) = gui.create {
            if !unsafe { create_fn(self.plugin, CLAP_WINDOW_API_COCOA.as_ptr(), false) } {
                return Err(ClapError::GuiError("GUI create failed".to_string()));
            }
        }

        if let Some(set_parent_fn) = gui.set_parent {
            let window = clap_window {
                api: CLAP_WINDOW_API_COCOA.as_ptr(),
                specific: clap_window_handle { cocoa: parent },
            };
            if !unsafe { set_parent_fn(self.plugin, &window) } {
                return Err(ClapError::GuiError("Set parent failed".to_string()));
            }
        }

        let (width, height) = if let Some(get_size_fn) = gui.get_size {
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            if unsafe { get_size_fn(self.plugin, &mut w, &mut h) } {
                (w, h)
            } else {
                (800, 600)
            }
        } else {
            (800, 600)
        };

        if let Some(show_fn) = gui.show {
            unsafe { show_fn(self.plugin) };
        }

        Ok((width, height))
    }

    /// Open the plugin GUI (Windows).
    #[cfg(target_os = "windows")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        let gui = self
            .get_gui_extension()
            .ok_or_else(|| ClapError::GuiError("No GUI extension".to_string()))?;

        if let Some(create_fn) = gui.create {
            if !unsafe { create_fn(self.plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false) } {
                return Err(ClapError::GuiError("GUI create failed".to_string()));
            }
        }

        if let Some(set_parent_fn) = gui.set_parent {
            let window = clap_window {
                api: CLAP_WINDOW_API_WIN32.as_ptr(),
                specific: clap_window_handle { win32: parent },
            };
            if !unsafe { set_parent_fn(self.plugin, &window) } {
                return Err(ClapError::GuiError("Set parent failed".to_string()));
            }
        }

        let (width, height) = if let Some(get_size_fn) = gui.get_size {
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            if unsafe { get_size_fn(self.plugin, &mut w, &mut h) } {
                (w, h)
            } else {
                (800, 600)
            }
        } else {
            (800, 600)
        };

        if let Some(show_fn) = gui.show {
            unsafe { show_fn(self.plugin) };
        }

        Ok((width, height))
    }

    /// Open the plugin GUI (Linux).
    #[cfg(target_os = "linux")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        let gui = self
            .get_gui_extension()
            .ok_or_else(|| ClapError::GuiError("No GUI extension".to_string()))?;

        if let Some(create_fn) = gui.create {
            if !unsafe { create_fn(self.plugin, CLAP_WINDOW_API_X11.as_ptr(), false) } {
                return Err(ClapError::GuiError("GUI create failed".to_string()));
            }
        }

        if let Some(set_parent_fn) = gui.set_parent {
            let window = clap_window {
                api: CLAP_WINDOW_API_X11.as_ptr(),
                specific: clap_window_handle {
                    x11: parent as u64,
                },
            };
            if !unsafe { set_parent_fn(self.plugin, &window) } {
                return Err(ClapError::GuiError("Set parent failed".to_string()));
            }
        }

        let (width, height) = if let Some(get_size_fn) = gui.get_size {
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            if unsafe { get_size_fn(self.plugin, &mut w, &mut h) } {
                (w, h)
            } else {
                (800, 600)
            }
        } else {
            (800, 600)
        };

        if let Some(show_fn) = gui.show {
            unsafe { show_fn(self.plugin) };
        }

        Ok((width, height))
    }

    /// Close the plugin GUI.
    pub fn close_gui(&mut self) {
        if let Some(gui) = self.get_gui_extension() {
            if let Some(hide_fn) = gui.hide {
                unsafe { hide_fn(self.plugin) };
            }
            if let Some(destroy_fn) = gui.destroy {
                unsafe { destroy_fn(self.plugin) };
            }
        }
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
    }
}

/// Output from process call.
#[derive(Debug, Clone, Default)]
pub struct ProcessOutput {
    pub midi_events: Vec<MidiEvent>,
    pub param_changes: ParameterChanges,
    pub note_expressions: Vec<NoteExpressionValue>,
}

fn build_clap_transport(transport: &TransportInfo) -> clap_event_transport {
    use clap_sys::events::{
        clap_event_header, CLAP_CORE_EVENT_SPACE_ID, CLAP_TRANSPORT_HAS_BEATS_TIMELINE,
        CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
        CLAP_TRANSPORT_IS_PLAYING, CLAP_TRANSPORT_IS_RECORDING,
    };

    let mut flags: u32 =
        CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_HAS_BEATS_TIMELINE | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;

    if transport.playing {
        flags |= CLAP_TRANSPORT_IS_PLAYING;
    }
    if transport.recording {
        flags |= CLAP_TRANSPORT_IS_RECORDING;
    }
    if transport.loop_active {
        flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
    }

    clap_event_transport {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_transport>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: 9, // CLAP_EVENT_TRANSPORT
            flags: 0,
        },
        flags,
        song_pos_beats: (transport.song_pos_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        song_pos_seconds: 0,
        tempo: transport.tempo,
        tempo_inc: 0.0,
        loop_start_beats: (transport.loop_start_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        loop_end_beats: (transport.loop_end_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        loop_start_seconds: 0,
        loop_end_seconds: 0,
        bar_start: (transport.bar_start * CLAP_BEATTIME_FACTOR as f64) as i64,
        bar_number: transport.bar_number,
        tsig_num: transport.time_sig_numerator,
        tsig_denom: transport.time_sig_denominator,
    }
}
