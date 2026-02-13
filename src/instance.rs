//! CLAP plugin instance.

use crate::error::{ClapError, LoadStage, Result};
use crate::events::{ClapEvent, InputEventList, OutputEventList};
use crate::host::{ClapHost, HostState, InputStream, OutputStream};
use crate::types::{
    AmbisonicConfig, AmbisonicNormalization, AmbisonicOrdering, AudioBuffer32, AudioBuffer64,
    AudioPortConfig, AudioPortConfigRequest, AudioPortFlags, AudioPortInfo, AudioPortType, Color,
    ContextMenuItem, ContextMenuTarget, MidiEvent, NoteDialect, NoteDialects, NoteExpressionValue,
    NoteName, NotePortInfo, ParamAutomationState, ParameterChanges, ParameterFlags, ParameterInfo,
    PluginInfo, RemoteControlsPage, StateContext, SurroundChannel, TrackInfo, TransportInfo,
    TransportRequest, TriggerInfo, UndoDeltaProperties, VoiceInfo,
};
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::clap_event_transport;
use clap_sys::ext::ambisonic::{
    clap_ambisonic_config, clap_plugin_ambisonic, CLAP_AMBISONIC_NORMALIZATION_MAXN,
    CLAP_AMBISONIC_NORMALIZATION_N2D, CLAP_AMBISONIC_NORMALIZATION_N3D,
    CLAP_AMBISONIC_NORMALIZATION_SN2D, CLAP_AMBISONIC_NORMALIZATION_SN3D,
    CLAP_AMBISONIC_ORDERING_ACN, CLAP_AMBISONIC_ORDERING_FUMA, CLAP_EXT_AMBISONIC,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN,
    CLAP_AUDIO_PORT_PREFERS_64BITS, CLAP_AUDIO_PORT_REQUIRES_COMMON_SAMPLE_SIZE,
    CLAP_AUDIO_PORT_SUPPORTS_64BITS, CLAP_EXT_AUDIO_PORTS, CLAP_PORT_MONO, CLAP_PORT_STEREO,
};
use clap_sys::ext::audio_ports_activation::{
    clap_plugin_audio_ports_activation, CLAP_EXT_AUDIO_PORTS_ACTIVATION,
};
use clap_sys::ext::audio_ports_config::{
    clap_audio_ports_config, clap_plugin_audio_ports_config, CLAP_EXT_AUDIO_PORTS_CONFIG,
};
use clap_sys::ext::configurable_audio_ports::{
    clap_audio_port_configuration_request, clap_plugin_configurable_audio_ports,
    CLAP_EXT_CONFIGURABLE_AUDIO_PORTS,
};
use clap_sys::ext::context_menu::{
    clap_context_menu_builder, clap_context_menu_check_entry, clap_context_menu_entry,
    clap_context_menu_item_title, clap_context_menu_submenu, clap_context_menu_target,
    clap_plugin_context_menu, CLAP_CONTEXT_MENU_ITEM_BEGIN_SUBMENU,
    CLAP_CONTEXT_MENU_ITEM_CHECK_ENTRY, CLAP_CONTEXT_MENU_ITEM_END_SUBMENU,
    CLAP_CONTEXT_MENU_ITEM_ENTRY, CLAP_CONTEXT_MENU_ITEM_SEPARATOR, CLAP_CONTEXT_MENU_ITEM_TITLE,
    CLAP_CONTEXT_MENU_TARGET_KIND_GLOBAL, CLAP_CONTEXT_MENU_TARGET_KIND_PARAM,
    CLAP_EXT_CONTEXT_MENU,
};
use clap_sys::ext::draft::extensible_audio_ports::{
    clap_plugin_extensible_audio_ports, CLAP_EXT_EXTENSIBLE_AUDIO_PORTS,
};
use clap_sys::ext::draft::resource_directory::{
    clap_plugin_resource_directory, CLAP_EXT_RESOURCE_DIRECTORY,
};
use clap_sys::ext::draft::triggers::{clap_plugin_triggers, clap_trigger_info, CLAP_EXT_TRIGGERS};
use clap_sys::ext::draft::tuning::{clap_plugin_tuning_t, CLAP_EXT_TUNING};
use clap_sys::ext::draft::undo::{
    clap_plugin_undo_context, clap_plugin_undo_delta, clap_undo_delta_properties,
    CLAP_EXT_UNDO_CONTEXT, CLAP_EXT_UNDO_DELTA,
};
use clap_sys::ext::gui::{clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI};
use clap_sys::ext::latency::{clap_plugin_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::note_name::{clap_note_name, clap_plugin_note_name, CLAP_EXT_NOTE_NAME};
use clap_sys::ext::note_ports::{
    clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
    CLAP_NOTE_DIALECT_MIDI, CLAP_NOTE_DIALECT_MIDI2, CLAP_NOTE_DIALECT_MIDI_MPE,
};
use clap_sys::ext::param_indication::{
    clap_plugin_param_indication, CLAP_EXT_PARAM_INDICATION, CLAP_PARAM_INDICATION_AUTOMATION_NONE,
    CLAP_PARAM_INDICATION_AUTOMATION_OVERRIDING, CLAP_PARAM_INDICATION_AUTOMATION_PLAYING,
    CLAP_PARAM_INDICATION_AUTOMATION_PRESENT, CLAP_PARAM_INDICATION_AUTOMATION_RECORDING,
};
use clap_sys::ext::params::{clap_plugin_params, CLAP_EXT_PARAMS};
#[cfg(unix)]
use clap_sys::ext::posix_fd_support::{clap_plugin_posix_fd_support, CLAP_EXT_POSIX_FD_SUPPORT};
use clap_sys::ext::preset_load::{clap_plugin_preset_load, CLAP_EXT_PRESET_LOAD};
use clap_sys::ext::remote_controls::{
    clap_plugin_remote_controls, clap_remote_controls_page, CLAP_EXT_REMOTE_CONTROLS,
};
use clap_sys::ext::render::{
    clap_plugin_render, CLAP_EXT_RENDER, CLAP_RENDER_OFFLINE, CLAP_RENDER_REALTIME,
};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::ext::state_context::{
    clap_plugin_state_context, CLAP_EXT_STATE_CONTEXT, CLAP_STATE_CONTEXT_FOR_DUPLICATE,
    CLAP_STATE_CONTEXT_FOR_PRESET, CLAP_STATE_CONTEXT_FOR_PROJECT,
};
use clap_sys::ext::surround::{clap_plugin_surround, CLAP_EXT_SURROUND};
use clap_sys::ext::tail::{clap_plugin_tail, CLAP_EXT_TAIL};
use clap_sys::ext::thread_pool::{clap_plugin_thread_pool, CLAP_EXT_THREAD_POOL};
use clap_sys::ext::timer_support::{clap_plugin_timer_support, CLAP_EXT_TIMER_SUPPORT};
use clap_sys::ext::track_info::{clap_plugin_track_info, CLAP_EXT_TRACK_INFO};
use clap_sys::ext::voice_info::{
    clap_plugin_voice_info, clap_voice_info, CLAP_EXT_VOICE_INFO,
    CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES,
};
use clap_sys::factory::preset_discovery::CLAP_PRESET_DISCOVERY_LOCATION_FILE;
use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR};
use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
use clap_sys::ext::gui::CLAP_WINDOW_API_COCOA;
#[cfg(target_os = "windows")]
use clap_sys::ext::gui::CLAP_WINDOW_API_WIN32;
#[cfg(target_os = "linux")]
use clap_sys::ext::gui::CLAP_WINDOW_API_X11;

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

struct ExtensionCache {
    params: *const clap_plugin_params,
    state: *const clap_plugin_state,
    state_context: *const clap_plugin_state_context,
    gui: *const clap_plugin_gui,
    audio_ports: *const clap_plugin_audio_ports,
    audio_ports_config: *const clap_plugin_audio_ports_config,
    note_ports: *const clap_plugin_note_ports,
    note_name: *const clap_plugin_note_name,
    latency: *const clap_plugin_latency,
    tail: *const clap_plugin_tail,
    render: *const clap_plugin_render,
    voice_info: *const clap_plugin_voice_info,
    preset_load: *const clap_plugin_preset_load,
    timer_support: *const clap_plugin_timer_support,
    remote_controls: *const clap_plugin_remote_controls,
    param_indication: *const clap_plugin_param_indication,
    track_info: *const clap_plugin_track_info,
    context_menu: *const clap_plugin_context_menu,
    configurable_audio_ports: *const clap_plugin_configurable_audio_ports,
    thread_pool: *const clap_plugin_thread_pool,
    audio_ports_activation: *const clap_plugin_audio_ports_activation,
    extensible_audio_ports: *const clap_plugin_extensible_audio_ports,
    ambisonic: *const clap_plugin_ambisonic,
    surround: *const clap_plugin_surround,
    #[cfg(unix)]
    posix_fd_support: *const clap_plugin_posix_fd_support,
    triggers: *const clap_plugin_triggers,
    tuning: *const clap_plugin_tuning_t,
    resource_directory: *const clap_plugin_resource_directory,
    undo_delta: *const clap_plugin_undo_delta,
    undo_context: *const clap_plugin_undo_context,
}

impl ExtensionCache {
    fn query(plugin: *const clap_plugin) -> Self {
        let get_ext = unsafe { (*plugin).get_extension };
        Self {
            params: Self::get(plugin, get_ext, CLAP_EXT_PARAMS.as_ptr()),
            state: Self::get(plugin, get_ext, CLAP_EXT_STATE.as_ptr()),
            state_context: Self::get(plugin, get_ext, CLAP_EXT_STATE_CONTEXT.as_ptr()),
            gui: Self::get(plugin, get_ext, CLAP_EXT_GUI.as_ptr()),
            audio_ports: Self::get(plugin, get_ext, CLAP_EXT_AUDIO_PORTS.as_ptr()),
            audio_ports_config: Self::get(plugin, get_ext, CLAP_EXT_AUDIO_PORTS_CONFIG.as_ptr()),
            note_ports: Self::get(plugin, get_ext, CLAP_EXT_NOTE_PORTS.as_ptr()),
            note_name: Self::get(plugin, get_ext, CLAP_EXT_NOTE_NAME.as_ptr()),
            latency: Self::get(plugin, get_ext, CLAP_EXT_LATENCY.as_ptr()),
            tail: Self::get(plugin, get_ext, CLAP_EXT_TAIL.as_ptr()),
            render: Self::get(plugin, get_ext, CLAP_EXT_RENDER.as_ptr()),
            voice_info: Self::get(plugin, get_ext, CLAP_EXT_VOICE_INFO.as_ptr()),
            preset_load: Self::get(plugin, get_ext, CLAP_EXT_PRESET_LOAD.as_ptr()),
            timer_support: Self::get(plugin, get_ext, CLAP_EXT_TIMER_SUPPORT.as_ptr()),
            remote_controls: Self::get(plugin, get_ext, CLAP_EXT_REMOTE_CONTROLS.as_ptr()),
            param_indication: Self::get(plugin, get_ext, CLAP_EXT_PARAM_INDICATION.as_ptr()),
            track_info: Self::get(plugin, get_ext, CLAP_EXT_TRACK_INFO.as_ptr()),
            context_menu: Self::get(plugin, get_ext, CLAP_EXT_CONTEXT_MENU.as_ptr()),
            configurable_audio_ports: Self::get(
                plugin,
                get_ext,
                CLAP_EXT_CONFIGURABLE_AUDIO_PORTS.as_ptr(),
            ),
            thread_pool: Self::get(plugin, get_ext, CLAP_EXT_THREAD_POOL.as_ptr()),
            audio_ports_activation: Self::get(
                plugin,
                get_ext,
                CLAP_EXT_AUDIO_PORTS_ACTIVATION.as_ptr(),
            ),
            extensible_audio_ports: Self::get(
                plugin,
                get_ext,
                CLAP_EXT_EXTENSIBLE_AUDIO_PORTS.as_ptr(),
            ),
            ambisonic: Self::get(plugin, get_ext, CLAP_EXT_AMBISONIC.as_ptr()),
            surround: Self::get(plugin, get_ext, CLAP_EXT_SURROUND.as_ptr()),
            #[cfg(unix)]
            posix_fd_support: Self::get(plugin, get_ext, CLAP_EXT_POSIX_FD_SUPPORT.as_ptr()),
            triggers: Self::get(plugin, get_ext, CLAP_EXT_TRIGGERS.as_ptr()),
            tuning: Self::get(plugin, get_ext, CLAP_EXT_TUNING.as_ptr()),
            resource_directory: Self::get(plugin, get_ext, CLAP_EXT_RESOURCE_DIRECTORY.as_ptr()),
            undo_delta: Self::get(plugin, get_ext, CLAP_EXT_UNDO_DELTA.as_ptr()),
            undo_context: Self::get(plugin, get_ext, CLAP_EXT_UNDO_CONTEXT.as_ptr()),
        }
    }

    fn get<T>(
        plugin: *const clap_plugin,
        get_ext: Option<unsafe extern "C" fn(*const clap_plugin, *const i8) -> *const c_void>,
        id: *const i8,
    ) -> *const T {
        match get_ext {
            Some(f) => {
                let ptr = unsafe { f(plugin, id) };
                if ptr.is_null() {
                    ptr::null()
                } else {
                    ptr as *const T
                }
            }
            None => ptr::null(),
        }
    }
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
        // Use get::<*const T> to get a pointer-to-pointer, then dereference once
        // to get the actual address of the struct in the plugin's data segment.
        let entry_struct: &clap_plugin_entry = unsafe {
            let sym = library
                .get::<*const clap_plugin_entry>(b"clap_entry\0")
                .map_err(|e| ClapError::LoadFailed {
                    path: bundle_path.to_path_buf(),
                    stage: LoadStage::Opening,
                    reason: format!("No clap_entry symbol: {}", e),
                })?;
            // sym is Symbol<*const clap_plugin_entry>, which deref's to
            // &*const clap_plugin_entry (i.e. pointer-to-pointer).
            // The symbol address *is* the struct, so cast the symbol address itself.
            let ptr = sym.into_raw();
            &*(ptr.into_raw() as *const clap_plugin_entry)
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

        let features = if descriptor.features.is_null() {
            Vec::new()
        } else {
            let mut features = Vec::new();
            let mut ptr = descriptor.features;
            const MAX_FEATURES: usize = 256;
            unsafe {
                while !(*ptr).is_null() && features.len() < MAX_FEATURES {
                    features.push(CStr::from_ptr(*ptr).to_string_lossy().to_string());
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
        let input_port_channels = Self::port_channels_static(plugin, extensions.audio_ports, true);
        let output_port_channels =
            Self::port_channels_static(plugin, extensions.audio_ports, false);

        let audio_inputs: usize = input_port_channels.iter().map(|&c| c as usize).sum();
        let audio_outputs: usize = output_port_channels.iter().map(|&c| c as usize).sum();

        // Check if any output port advertises CLAP_AUDIO_PORT_SUPPORTS_64BITS
        let supports_f64 = Self::check_f64_support(plugin, extensions.audio_ports);

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

        self.is_processing = false;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        if (self.sample_rate - sample_rate).abs() < f64::EPSILON {
            return; // No change — skip deactivate/reactivate cycle
        }
        if self.is_active {
            self.deactivate();
        }
        self.sample_rate = sample_rate;
    }

    // ── Audio processing ──

    /// Split a flat slice of channel pointers into per-port `clap_audio_buffer` structs.
    ///
    /// If the caller provides fewer channels than the port layout requires,
    /// scratch buffers are allocated for the remaining channels so the plugin
    /// always gets valid pointers.
    fn build_port_buffers_f32(
        port_channels: &[u32],
        ptrs: &mut Vec<*mut f32>,
        scratch: &mut Vec<Vec<f32>>,
        num_samples: usize,
    ) -> Vec<clap_sys::audio_buffer::clap_audio_buffer> {
        // Ensure enough channel pointers for all ports
        let total_needed: usize = port_channels.iter().map(|&c| c as usize).sum();
        while ptrs.len() < total_needed {
            scratch.push(vec![0.0f32; num_samples]);
            ptrs.push(scratch.last_mut().unwrap().as_mut_ptr());
        }

        let mut offset = 0usize;
        port_channels
            .iter()
            .map(|&ch_count| {
                let ch = ch_count as usize;
                let buf = clap_sys::audio_buffer::clap_audio_buffer {
                    data32: ptrs[offset..].as_mut_ptr(),
                    data64: ptr::null_mut(),
                    channel_count: ch_count,
                    latency: 0,
                    constant_mask: 0,
                };
                offset += ch;
                buf
            })
            .collect()
    }

    fn build_port_buffers_f64(
        port_channels: &[u32],
        ptrs: &mut Vec<*mut f64>,
        scratch: &mut Vec<Vec<f64>>,
        num_samples: usize,
    ) -> Vec<clap_sys::audio_buffer::clap_audio_buffer> {
        let total_needed: usize = port_channels.iter().map(|&c| c as usize).sum();
        while ptrs.len() < total_needed {
            scratch.push(vec![0.0f64; num_samples]);
            ptrs.push(scratch.last_mut().unwrap().as_mut_ptr());
        }

        let mut offset = 0usize;
        port_channels
            .iter()
            .map(|&ch_count| {
                let ch = ch_count as usize;
                let buf = clap_sys::audio_buffer::clap_audio_buffer {
                    data32: ptr::null_mut(),
                    data64: ptrs[offset..].as_mut_ptr(),
                    channel_count: ch_count,
                    latency: 0,
                    constant_mask: 0,
                };
                offset += ch;
                buf
            })
            .collect()
    }

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

        let mut input_ptrs: Vec<*mut f32> = buffer
            .inputs
            .iter()
            .map(|s| s.as_ptr() as *mut f32)
            .collect();
        let mut output_ptrs: Vec<*mut f32> =
            buffer.outputs.iter_mut().map(|s| s.as_mut_ptr()).collect();

        let n = buffer.num_samples;
        let mut scratch_in = Vec::new();
        let mut scratch_out = Vec::new();
        let mut input_bufs = Self::build_port_buffers_f32(
            &self.input_port_channels,
            &mut input_ptrs,
            &mut scratch_in,
            n,
        );
        let mut output_bufs = Self::build_port_buffers_f32(
            &self.output_port_channels,
            &mut output_ptrs,
            &mut scratch_out,
            n,
        );

        self.do_process(
            &mut input_bufs,
            &mut output_bufs,
            num_samples,
            &input_events,
            &mut output_events,
            transport,
        )
    }

    pub fn process_f64(
        &mut self,
        buffer: &mut AudioBuffer64,
        midi_events: Option<&[MidiEvent]>,
        param_changes: Option<&ParameterChanges>,
        note_expressions: Option<&[NoteExpressionValue]>,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        if !self.supports_f64 {
            return Err(ClapError::ProcessError(format!(
                "Plugin '{}' does not support 64-bit audio processing \
                 (CLAP_AUDIO_PORT_SUPPORTS_64BITS not set)",
                self.info.name
            )));
        }

        self.start_processing()?;

        let num_samples = buffer.num_samples as u32;

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

        let mut input_ptrs: Vec<*mut f64> = buffer
            .inputs
            .iter()
            .map(|s| s.as_ptr() as *mut f64)
            .collect();
        let mut output_ptrs: Vec<*mut f64> =
            buffer.outputs.iter_mut().map(|s| s.as_mut_ptr()).collect();

        let n = buffer.num_samples;
        let mut scratch_in = Vec::new();
        let mut scratch_out = Vec::new();
        let mut input_bufs = Self::build_port_buffers_f64(
            &self.input_port_channels,
            &mut input_ptrs,
            &mut scratch_in,
            n,
        );
        let mut output_bufs = Self::build_port_buffers_f64(
            &self.output_port_channels,
            &mut output_ptrs,
            &mut scratch_out,
            n,
        );

        self.do_process(
            &mut input_bufs,
            &mut output_bufs,
            num_samples,
            &input_events,
            &mut output_events,
            transport,
        )
    }

    fn do_process(
        &mut self,
        audio_inputs: &mut [clap_sys::audio_buffer::clap_audio_buffer],
        audio_outputs: &mut [clap_sys::audio_buffer::clap_audio_buffer],
        num_samples: u32,
        input_events: &InputEventList,
        output_events: &mut OutputEventList,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
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
            audio_inputs: audio_inputs.as_mut_ptr(),
            audio_outputs: audio_outputs.as_mut_ptr(),
            audio_inputs_count: audio_inputs.len() as u32,
            audio_outputs_count: audio_outputs.len() as u32,
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

    // ── Parameters ──

    pub fn parameter_count(&self) -> usize {
        if self.extensions.params.is_null() {
            return 0;
        }
        let params = unsafe { &*self.extensions.params };
        match params.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_parameter(&self, id: u32) -> Option<f64> {
        if self.extensions.params.is_null() {
            return None;
        }
        let params = unsafe { &*self.extensions.params };
        let get_value_fn = params.get_value?;
        let mut value: f64 = 0.0;
        if unsafe { get_value_fn(self.plugin, id, &mut value) } {
            Some(value)
        } else {
            None
        }
    }

    pub fn get_parameter_info(&self, index: u32) -> Option<ParameterInfo> {
        if self.extensions.params.is_null() {
            return None;
        }
        let params = unsafe { &*self.extensions.params };
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
                is_automatable: (info.flags & clap_sys::ext::params::CLAP_PARAM_IS_AUTOMATABLE)
                    != 0,
                ..Default::default()
            },
        })
    }

    pub fn get_all_parameters(&self) -> Vec<ParameterInfo> {
        let count = self.parameter_count() as u32;
        (0..count)
            .filter_map(|i| self.get_parameter_info(i))
            .collect()
    }

    /// Flush parameter changes outside of process(). Sends input events to
    /// the plugin and collects any output events it produces.
    pub fn flush_params(&mut self, input_events: Vec<ClapEvent>) -> Vec<ClapEvent> {
        if self.extensions.params.is_null() {
            return Vec::new();
        }
        let params = unsafe { &*self.extensions.params };
        let flush_fn = match params.flush {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut input_list = InputEventList::from_events(input_events);
        input_list.sort_by_time();

        let mut output_list = OutputEventList::new();

        unsafe {
            flush_fn(
                self.plugin,
                input_list.as_raw() as *const _,
                output_list.as_raw_mut() as *const _,
            );
        }

        output_list.take_events()
    }

    /// Set a single parameter value immediately via flush.
    pub fn set_parameter(&mut self, id: u32, value: f64) {
        let event = ClapEvent::param_value(0, id, value);
        self.flush_params(vec![event]);
    }

    // ── State ──

    pub fn save_state(&self) -> Result<Vec<u8>> {
        if self.extensions.state.is_null() {
            return Err(ClapError::StateError("No state extension".to_string()));
        }
        let state_ext = unsafe { &*self.extensions.state };
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

        if self.extensions.state.is_null() {
            return Err(ClapError::StateError("No state extension".to_string()));
        }
        let state_ext = unsafe { &*self.extensions.state };
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
        if !self.extensions.state_context.is_null() {
            let ext = unsafe { &*self.extensions.state_context };
            if let Some(save_fn) = ext.save {
                let context_type = match context {
                    StateContext::ForPreset => CLAP_STATE_CONTEXT_FOR_PRESET,
                    StateContext::ForProject => CLAP_STATE_CONTEXT_FOR_PROJECT,
                    StateContext::ForDuplicate => CLAP_STATE_CONTEXT_FOR_DUPLICATE,
                };
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
        if !self.extensions.state_context.is_null() {
            let ext = unsafe { &*self.extensions.state_context };
            if let Some(load_fn) = ext.load {
                let context_type = match context {
                    StateContext::ForPreset => CLAP_STATE_CONTEXT_FOR_PRESET,
                    StateContext::ForProject => CLAP_STATE_CONTEXT_FOR_PROJECT,
                    StateContext::ForDuplicate => CLAP_STATE_CONTEXT_FOR_DUPLICATE,
                };
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
        !self.extensions.state_context.is_null()
    }

    // ── Audio ports ──

    pub fn audio_port_count(&self, is_input: bool) -> usize {
        if self.extensions.audio_ports.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.audio_ports };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin, is_input) }) as usize,
            None => 0,
        }
    }

    pub fn audio_port_info(&self, index: usize, is_input: bool) -> Option<AudioPortInfo> {
        if self.extensions.audio_ports.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio_ports };
        let get_fn = ext.get?;

        let mut info: clap_audio_port_info = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, is_input, &mut info) } {
            return None;
        }

        let port_type = if info.port_type.is_null() {
            AudioPortType::Custom(String::new())
        } else {
            let type_cstr = unsafe { CStr::from_ptr(info.port_type) };
            if type_cstr == CLAP_PORT_MONO {
                AudioPortType::Mono
            } else if type_cstr == CLAP_PORT_STEREO {
                AudioPortType::Stereo
            } else {
                AudioPortType::Custom(type_cstr.to_string_lossy().to_string())
            }
        };

        Some(AudioPortInfo {
            id: info.id,
            name: unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            channel_count: info.channel_count,
            flags: AudioPortFlags {
                is_main: (info.flags & CLAP_AUDIO_PORT_IS_MAIN) != 0,
                supports_64bit: (info.flags & CLAP_AUDIO_PORT_SUPPORTS_64BITS) != 0,
                prefers_64bit: (info.flags & CLAP_AUDIO_PORT_PREFERS_64BITS) != 0,
                requires_common_sample_size: (info.flags
                    & CLAP_AUDIO_PORT_REQUIRES_COMMON_SAMPLE_SIZE)
                    != 0,
            },
            port_type,
            in_place_pair_id: info.in_place_pair,
        })
    }

    pub fn num_input_channels(&self) -> usize {
        let count = self.audio_port_count(true);
        (0..count)
            .filter_map(|i| self.audio_port_info(i, true))
            .map(|p| p.channel_count as usize)
            .sum()
    }

    pub fn num_output_channels(&self) -> usize {
        let count = self.audio_port_count(false);
        (0..count)
            .filter_map(|i| self.audio_port_info(i, false))
            .map(|p| p.channel_count as usize)
            .sum()
    }

    // ── Note ports ──

    pub fn note_port_count(&self, is_input: bool) -> usize {
        if self.extensions.note_ports.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.note_ports };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin, is_input) }) as usize,
            None => 0,
        }
    }

    pub fn note_port_info(&self, index: usize, is_input: bool) -> Option<NotePortInfo> {
        if self.extensions.note_ports.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.note_ports };
        let get_fn = ext.get?;

        let mut info: clap_note_port_info = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, is_input, &mut info) } {
            return None;
        }

        let preferred_dialect = if (info.preferred_dialect & CLAP_NOTE_DIALECT_CLAP) != 0 {
            NoteDialect::Clap
        } else if (info.preferred_dialect & CLAP_NOTE_DIALECT_MIDI) != 0 {
            NoteDialect::Midi
        } else if (info.preferred_dialect & CLAP_NOTE_DIALECT_MIDI_MPE) != 0 {
            NoteDialect::MidiMpe
        } else {
            NoteDialect::Midi2
        };

        Some(NotePortInfo {
            id: info.id,
            name: unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            supported_dialects: NoteDialects {
                clap: (info.supported_dialects & CLAP_NOTE_DIALECT_CLAP) != 0,
                midi: (info.supported_dialects & CLAP_NOTE_DIALECT_MIDI) != 0,
                midi_mpe: (info.supported_dialects & CLAP_NOTE_DIALECT_MIDI_MPE) != 0,
                midi2: (info.supported_dialects & CLAP_NOTE_DIALECT_MIDI2) != 0,
            },
            preferred_dialect,
        })
    }

    // ── Audio ports config ──

    pub fn audio_port_config_count(&self) -> usize {
        if self.extensions.audio_ports_config.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.audio_ports_config };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_audio_port_config(&self, index: usize) -> Option<AudioPortConfig> {
        if self.extensions.audio_ports_config.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio_ports_config };
        let get_fn = ext.get?;

        let mut config: clap_audio_ports_config = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut config) } {
            return None;
        }

        Some(AudioPortConfig {
            id: config.id,
            name: unsafe { CStr::from_ptr(config.name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            input_port_count: config.input_port_count,
            output_port_count: config.output_port_count,
            has_main_input: config.has_main_input,
            main_input_channel_count: config.main_input_channel_count,
            has_main_output: config.has_main_output,
            main_output_channel_count: config.main_output_channel_count,
        })
    }

    pub fn select_audio_port_config(&mut self, config_id: u32) -> bool {
        if self.extensions.audio_ports_config.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio_ports_config };
        match ext.select {
            Some(f) => unsafe { f(self.plugin, config_id) },
            None => false,
        }
    }

    // ── Latency ──

    pub fn get_latency(&self) -> u32 {
        if self.extensions.latency.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.latency };
        match ext.get {
            Some(f) => unsafe { f(self.plugin) },
            None => 0,
        }
    }

    // ── Tail ──

    pub fn get_tail(&self) -> u32 {
        if self.extensions.tail.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.tail };
        match ext.get {
            Some(f) => unsafe { f(self.plugin) },
            None => 0,
        }
    }

    // ── Render mode ──

    pub fn set_render_mode(&mut self, offline: bool) -> bool {
        if self.extensions.render.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.render };
        match ext.set {
            Some(f) => {
                let mode = if offline {
                    CLAP_RENDER_OFFLINE
                } else {
                    CLAP_RENDER_REALTIME
                };
                unsafe { f(self.plugin, mode) }
            }
            None => false,
        }
    }

    pub fn has_hard_realtime_requirement(&self) -> bool {
        if self.extensions.render.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.render };
        match ext.has_hard_realtime_requirement {
            Some(f) => unsafe { f(self.plugin) },
            None => false,
        }
    }

    // ── Voice info ──

    pub fn get_voice_info(&self) -> Option<VoiceInfo> {
        if self.extensions.voice_info.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.voice_info };
        let get_fn = ext.get?;
        let mut info: clap_voice_info = unsafe { std::mem::zeroed() };
        if unsafe { get_fn(self.plugin, &mut info) } {
            Some(VoiceInfo {
                voice_count: info.voice_count,
                voice_capacity: info.voice_capacity,
                supports_overlapping_notes: (info.flags
                    & CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES)
                    != 0,
            })
        } else {
            None
        }
    }

    // ── Note names ──

    pub fn note_name_count(&self) -> usize {
        if self.extensions.note_name.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.note_name };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_note_name(&self, index: usize) -> Option<NoteName> {
        if self.extensions.note_name.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.note_name };
        let get_fn = ext.get?;
        let mut info: clap_note_name = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut info) } {
            return None;
        }
        Some(NoteName {
            name: unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            port: info.port,
            channel: info.channel,
            key: info.key,
        })
    }

    // ── Preset load ──

    pub fn load_preset(&mut self, path: &Path) -> Result<()> {
        if self.extensions.preset_load.is_null() {
            return Err(ClapError::StateError(
                "No preset-load extension".to_string(),
            ));
        }
        let ext = unsafe { &*self.extensions.preset_load };
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

    // ── GUI ──

    pub fn has_gui(&self) -> bool {
        !self.extensions.gui.is_null()
    }

    #[cfg(target_os = "macos")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        if self.extensions.gui.is_null() {
            return Err(ClapError::GuiError("No GUI extension".to_string()));
        }
        let gui = unsafe { &*self.extensions.gui };

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

    #[cfg(target_os = "windows")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        if self.extensions.gui.is_null() {
            return Err(ClapError::GuiError("No GUI extension".to_string()));
        }
        let gui = unsafe { &*self.extensions.gui };

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

    #[cfg(target_os = "linux")]
    pub fn open_gui(&mut self, parent: *mut std::ffi::c_void) -> Result<(u32, u32)> {
        if self.extensions.gui.is_null() {
            return Err(ClapError::GuiError("No GUI extension".to_string()));
        }
        let gui = unsafe { &*self.extensions.gui };

        if let Some(create_fn) = gui.create {
            if !unsafe { create_fn(self.plugin, CLAP_WINDOW_API_X11.as_ptr(), false) } {
                return Err(ClapError::GuiError("GUI create failed".to_string()));
            }
        }

        if let Some(set_parent_fn) = gui.set_parent {
            let window = clap_window {
                api: CLAP_WINDOW_API_X11.as_ptr(),
                specific: clap_window_handle { x11: parent as u64 },
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

    pub fn close_gui(&mut self) {
        if self.extensions.gui.is_null() {
            return;
        }
        let gui = unsafe { &*self.extensions.gui };
        if let Some(hide_fn) = gui.hide {
            unsafe { hide_fn(self.plugin) };
        }
        if let Some(destroy_fn) = gui.destroy {
            unsafe { destroy_fn(self.plugin) };
        }
    }

    // ── Host state polling ──

    pub fn host_state(&self) -> &Arc<HostState> {
        &self.host_state
    }

    pub fn poll_restart_requested(&self) -> bool {
        self.host_state.poll(&self.host_state.restart_requested)
    }

    pub fn poll_process_requested(&self) -> bool {
        self.host_state.poll(&self.host_state.process_requested)
    }

    pub fn poll_callback_requested(&self) -> bool {
        self.host_state.poll(&self.host_state.callback_requested)
    }

    pub fn poll_latency_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.latency_changed)
    }

    pub fn poll_tail_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.tail_changed)
    }

    pub fn poll_params_rescan(&self) -> bool {
        self.host_state
            .poll(&self.host_state.params_rescan_requested)
    }

    pub fn poll_params_flush_requested(&self) -> bool {
        self.host_state
            .poll(&self.host_state.params_flush_requested)
    }

    pub fn poll_state_dirty(&self) -> bool {
        self.host_state.poll(&self.host_state.state_dirty)
    }

    pub fn poll_audio_ports_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.audio_ports_changed)
    }

    pub fn poll_note_ports_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.note_ports_changed)
    }

    pub fn poll_gui_closed(&self) -> bool {
        self.host_state.poll(&self.host_state.gui_closed)
    }

    pub fn needs_restart(&self) -> bool {
        self.host_state
            .restart_requested
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Fire any expired timers. Call this periodically from the main thread.
    /// Returns the number of timer callbacks fired.
    pub fn poll_timers(&mut self) -> usize {
        if self.extensions.timer_support.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.timer_support };
        let on_timer = match ext.on_timer {
            Some(f) => f,
            None => return 0,
        };

        let now = std::time::Instant::now();
        let mut fired = 0usize;
        let mut expired_ids = Vec::new();

        if let Ok(mut timers) = self.host_state.timers.lock() {
            for timer in timers.iter_mut() {
                let elapsed = now.duration_since(timer.last_fire);
                if elapsed.as_millis() >= timer.period_ms as u128 {
                    expired_ids.push(timer.id);
                    timer.last_fire = now;
                }
            }
        }

        for id in expired_ids {
            unsafe { on_timer(self.plugin, id) };
            fired += 1;
        }

        fired
    }

    pub fn poll_audio_ports_config_changed(&self) -> bool {
        self.host_state
            .poll(&self.host_state.audio_ports_config_changed)
    }

    pub fn poll_remote_controls_changed(&self) -> bool {
        self.host_state
            .poll(&self.host_state.remote_controls_changed)
    }

    pub fn poll_suggested_remote_page(&self) -> Option<u32> {
        let val = self
            .host_state
            .suggested_remote_page
            .swap(u32::MAX, std::sync::atomic::Ordering::AcqRel);
        if val == u32::MAX {
            None
        } else {
            Some(val)
        }
    }

    pub fn drain_transport_requests(&self) -> Vec<TransportRequest> {
        if let Ok(mut reqs) = self.host_state.transport_requests.lock() {
            std::mem::take(&mut *reqs)
        } else {
            Vec::new()
        }
    }

    pub fn poll_note_names_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.note_names_changed)
    }

    pub fn poll_voice_info_changed(&self) -> bool {
        self.host_state.poll(&self.host_state.voice_info_changed)
    }

    pub fn poll_preset_loaded(&self) -> bool {
        self.host_state.poll(&self.host_state.preset_loaded)
    }

    /// Call `plugin.on_main_thread()` when the plugin has requested a main-thread callback.
    pub fn on_main_thread(&mut self) {
        let plugin_ref = unsafe { &*self.plugin };
        if let Some(f) = plugin_ref.on_main_thread {
            unsafe { f(self.plugin) };
        }
    }

    // ── Track info (host→plugin notification) ──

    pub fn set_track_info(&self, info: TrackInfo) {
        if let Ok(mut guard) = self.host_state.track_info.lock() {
            *guard = Some(info);
        }
    }

    pub fn notify_track_info_changed(&self) {
        if self.extensions.track_info.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.track_info };
        if let Some(f) = ext.changed {
            unsafe { f(self.plugin) };
        }
    }

    // ── Remote controls (plugin-side) ──

    pub fn remote_controls_page_count(&self) -> usize {
        if self.extensions.remote_controls.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.remote_controls };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_remote_controls_page(&self, index: usize) -> Option<RemoteControlsPage> {
        if self.extensions.remote_controls.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.remote_controls };
        let get_fn = ext.get?;
        let mut page: clap_remote_controls_page = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut page) } {
            return None;
        }
        Some(RemoteControlsPage {
            section_name: unsafe { CStr::from_ptr(page.section_name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            page_id: page.page_id,
            page_name: unsafe { CStr::from_ptr(page.page_name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            param_ids: page.param_ids,
            is_for_preset: page.is_for_preset,
        })
    }

    // ── Param indication (host→plugin) ──

    pub fn set_param_mapping(
        &self,
        param_id: u32,
        has_mapping: bool,
        color: Option<Color>,
        label: Option<&str>,
        description: Option<&str>,
    ) {
        if self.extensions.param_indication.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.param_indication };
        let set_mapping = match ext.set_mapping {
            Some(f) => f,
            None => return,
        };
        let clap_color = color.map(|c| clap_sys::color::clap_color {
            alpha: c.alpha,
            red: c.red,
            green: c.green,
            blue: c.blue,
        });
        let color_ptr = clap_color
            .as_ref()
            .map(|c| c as *const _)
            .unwrap_or(ptr::null());
        let label_cstr = label.and_then(|s| std::ffi::CString::new(s).ok());
        let label_ptr = label_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        let desc_cstr = description.and_then(|s| std::ffi::CString::new(s).ok());
        let desc_ptr = desc_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        unsafe {
            set_mapping(
                self.plugin,
                param_id,
                has_mapping,
                color_ptr,
                label_ptr,
                desc_ptr,
            );
        }
    }

    pub fn set_param_automation(
        &self,
        param_id: u32,
        state: ParamAutomationState,
        color: Option<Color>,
    ) {
        if self.extensions.param_indication.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.param_indication };
        let set_automation = match ext.set_automation {
            Some(f) => f,
            None => return,
        };
        let automation_state = match state {
            ParamAutomationState::None => CLAP_PARAM_INDICATION_AUTOMATION_NONE,
            ParamAutomationState::Present => CLAP_PARAM_INDICATION_AUTOMATION_PRESENT,
            ParamAutomationState::Playing => CLAP_PARAM_INDICATION_AUTOMATION_PLAYING,
            ParamAutomationState::Recording => CLAP_PARAM_INDICATION_AUTOMATION_RECORDING,
            ParamAutomationState::Overriding => CLAP_PARAM_INDICATION_AUTOMATION_OVERRIDING,
        };
        let clap_color = color.map(|c| clap_sys::color::clap_color {
            alpha: c.alpha,
            red: c.red,
            green: c.green,
            blue: c.blue,
        });
        let color_ptr = clap_color
            .as_ref()
            .map(|c| c as *const _)
            .unwrap_or(ptr::null());
        unsafe { set_automation(self.plugin, param_id, automation_state, color_ptr) };
    }

    // ── Context menu (plugin-side) ──

    pub fn context_menu_populate(&self, target: ContextMenuTarget) -> Option<Vec<ContextMenuItem>> {
        if self.extensions.context_menu.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.context_menu };
        let populate_fn = ext.populate?;

        let clap_target = match target {
            ContextMenuTarget::Global => clap_context_menu_target {
                kind: CLAP_CONTEXT_MENU_TARGET_KIND_GLOBAL,
                id: 0,
            },
            ContextMenuTarget::Param(id) => clap_context_menu_target {
                kind: CLAP_CONTEXT_MENU_TARGET_KIND_PARAM,
                id,
            },
        };

        let mut items: Vec<ContextMenuItem> = Vec::new();
        let items_ptr = &mut items as *mut Vec<ContextMenuItem> as *mut c_void;

        let builder = clap_context_menu_builder {
            ctx: items_ptr,
            add_item: Some(context_menu_builder_add_item),
            supports: Some(context_menu_builder_supports),
        };

        if unsafe { populate_fn(self.plugin, &clap_target, &builder) } {
            Some(items)
        } else {
            None
        }
    }

    pub fn context_menu_perform(&self, target: ContextMenuTarget, action_id: u32) -> bool {
        if self.extensions.context_menu.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.context_menu };
        let perform_fn = match ext.perform {
            Some(f) => f,
            None => return false,
        };
        let clap_target = match target {
            ContextMenuTarget::Global => clap_context_menu_target {
                kind: CLAP_CONTEXT_MENU_TARGET_KIND_GLOBAL,
                id: 0,
            },
            ContextMenuTarget::Param(id) => clap_context_menu_target {
                kind: CLAP_CONTEXT_MENU_TARGET_KIND_PARAM,
                id,
            },
        };
        unsafe { perform_fn(self.plugin, &clap_target, action_id) }
    }

    // ── Configurable audio ports (plugin-side) ──

    pub fn can_apply_audio_port_configuration(&self, requests: &[AudioPortConfigRequest]) -> bool {
        if self.extensions.configurable_audio_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.configurable_audio_ports };
        let can_apply_fn = match ext.can_apply_configuration {
            Some(f) => f,
            None => return false,
        };
        let clap_requests = build_port_config_requests(requests);
        unsafe {
            can_apply_fn(
                self.plugin,
                clap_requests.as_ptr(),
                clap_requests.len() as u32,
            )
        }
    }

    pub fn apply_audio_port_configuration(&mut self, requests: &[AudioPortConfigRequest]) -> bool {
        if self.extensions.configurable_audio_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.configurable_audio_ports };
        let apply_fn = match ext.apply_configuration {
            Some(f) => f,
            None => return false,
        };
        let clap_requests = build_port_config_requests(requests);
        unsafe {
            apply_fn(
                self.plugin,
                clap_requests.as_ptr(),
                clap_requests.len() as u32,
            )
        }
    }

    // ── Thread pool (plugin-side) ──

    pub fn thread_pool_exec(&self, task_index: u32) {
        if self.extensions.thread_pool.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.thread_pool };
        if let Some(f) = ext.exec {
            unsafe { f(self.plugin, task_index) };
        }
    }

    // ── Audio ports activation (plugin-side) ──

    pub fn can_activate_audio_port_while_processing(&self) -> bool {
        if self.extensions.audio_ports_activation.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio_ports_activation };
        match ext.can_activate_while_processing {
            Some(f) => unsafe { f(self.plugin) },
            None => false,
        }
    }

    pub fn set_audio_port_active(
        &mut self,
        is_input: bool,
        port_index: u32,
        is_active: bool,
        sample_size: u32,
    ) -> bool {
        if self.extensions.audio_ports_activation.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio_ports_activation };
        match ext.set_active {
            Some(f) => unsafe { f(self.plugin, is_input, port_index, is_active, sample_size) },
            None => false,
        }
    }

    // ── Extensible audio ports (plugin-side, draft) ──

    pub fn add_audio_port(
        &mut self,
        is_input: bool,
        channel_count: u32,
        port_type: Option<&str>,
    ) -> bool {
        if self.extensions.extensible_audio_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.extensible_audio_ports };
        let add_fn = match ext.add_port {
            Some(f) => f,
            None => return false,
        };
        let type_cstr = port_type.and_then(|s| std::ffi::CString::new(s).ok());
        let type_ptr = type_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        unsafe { add_fn(self.plugin, is_input, channel_count, type_ptr, ptr::null()) }
    }

    pub fn remove_audio_port(&mut self, is_input: bool, index: u32) -> bool {
        if self.extensions.extensible_audio_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.extensible_audio_ports };
        match ext.remove_port {
            Some(f) => unsafe { f(self.plugin, is_input, index) },
            None => false,
        }
    }

    // ── Ambisonic (plugin-side) ──

    pub fn is_ambisonic_config_supported(&self, config: &AmbisonicConfig) -> bool {
        if self.extensions.ambisonic.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.ambisonic };
        let f = match ext.is_config_supported {
            Some(f) => f,
            None => return false,
        };
        let clap_config = clap_ambisonic_config {
            ordering: match config.ordering {
                AmbisonicOrdering::Fuma => CLAP_AMBISONIC_ORDERING_FUMA,
                AmbisonicOrdering::Acn => CLAP_AMBISONIC_ORDERING_ACN,
            },
            normalization: match config.normalization {
                AmbisonicNormalization::MaxN => CLAP_AMBISONIC_NORMALIZATION_MAXN,
                AmbisonicNormalization::Sn3d => CLAP_AMBISONIC_NORMALIZATION_SN3D,
                AmbisonicNormalization::N3d => CLAP_AMBISONIC_NORMALIZATION_N3D,
                AmbisonicNormalization::Sn2d => CLAP_AMBISONIC_NORMALIZATION_SN2D,
                AmbisonicNormalization::N2d => CLAP_AMBISONIC_NORMALIZATION_N2D,
            },
        };
        unsafe { f(self.plugin, &clap_config) }
    }

    pub fn get_ambisonic_config(&self, is_input: bool, port_index: u32) -> Option<AmbisonicConfig> {
        if self.extensions.ambisonic.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.ambisonic };
        let get_fn = ext.get_config?;
        let mut config: clap_ambisonic_config = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, is_input, port_index, &mut config) } {
            return None;
        }
        let ordering = match config.ordering {
            CLAP_AMBISONIC_ORDERING_FUMA => AmbisonicOrdering::Fuma,
            _ => AmbisonicOrdering::Acn,
        };
        let normalization = match config.normalization {
            CLAP_AMBISONIC_NORMALIZATION_MAXN => AmbisonicNormalization::MaxN,
            CLAP_AMBISONIC_NORMALIZATION_SN3D => AmbisonicNormalization::Sn3d,
            CLAP_AMBISONIC_NORMALIZATION_N3D => AmbisonicNormalization::N3d,
            CLAP_AMBISONIC_NORMALIZATION_SN2D => AmbisonicNormalization::Sn2d,
            _ => AmbisonicNormalization::N2d,
        };
        Some(AmbisonicConfig {
            ordering,
            normalization,
        })
    }

    // ── Surround (plugin-side) ──

    pub fn is_surround_channel_mask_supported(&self, channel_mask: u64) -> bool {
        if self.extensions.surround.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.surround };
        match ext.is_channel_mask_supported {
            Some(f) => unsafe { f(self.plugin, channel_mask) },
            None => false,
        }
    }

    pub fn get_surround_channel_map(
        &self,
        is_input: bool,
        port_index: u32,
    ) -> Option<Vec<SurroundChannel>> {
        if self.extensions.surround.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.surround };
        let get_fn = ext.get_channel_map?;
        let mut map = [0u8; 64];
        let count =
            unsafe { get_fn(self.plugin, is_input, port_index, map.as_mut_ptr(), 64) } as usize;
        if count == 0 || count > map.len() {
            return None;
        }
        Some(
            map[..count]
                .iter()
                .filter_map(|&pos| SurroundChannel::from_position(pos))
                .collect(),
        )
    }

    // ── Triggers (plugin-side, draft) ──

    pub fn trigger_count(&self) -> usize {
        if self.extensions.triggers.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.triggers };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_trigger_info(&self, index: usize) -> Option<TriggerInfo> {
        if self.extensions.triggers.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.triggers };
        let get_fn = ext.get_info?;
        let mut info: clap_trigger_info = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut info) } {
            return None;
        }
        Some(TriggerInfo {
            id: info.id,
            flags: info.flags,
            name: unsafe { CStr::from_ptr(info.name.as_ptr()) }
                .to_string_lossy()
                .to_string(),
            module: unsafe { CStr::from_ptr(info.module.as_ptr()) }
                .to_string_lossy()
                .to_string(),
        })
    }

    // ── Tuning (plugin-side, draft) ──

    pub fn notify_tuning_changed(&self) {
        if self.extensions.tuning.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.tuning };
        if let Some(f) = ext.changed {
            unsafe { f(self.plugin) };
        }
    }

    // ── Resource directory (plugin-side, draft) ──

    pub fn resource_set_directory(&self, path: &str, is_shared: bool) {
        if self.extensions.resource_directory.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.resource_directory };
        if let Some(f) = ext.set_directory {
            if let Ok(cstr) = std::ffi::CString::new(path) {
                unsafe { f(self.plugin, cstr.as_ptr(), is_shared) };
            }
        }
    }

    pub fn resource_collect(&self, all: bool) {
        if self.extensions.resource_directory.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.resource_directory };
        if let Some(f) = ext.collect {
            unsafe { f(self.plugin, all) };
        }
    }

    pub fn resource_files_count(&self) -> u32 {
        if self.extensions.resource_directory.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.resource_directory };
        match ext.get_files_count {
            Some(f) => unsafe { f(self.plugin) },
            None => 0,
        }
    }

    pub fn resource_get_file_path(&self, index: u32) -> Option<String> {
        if self.extensions.resource_directory.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.resource_directory };
        let get_fn = ext.get_file_path?;
        let mut buf = [0i8; 4096];
        let result = unsafe { get_fn(self.plugin, index, buf.as_mut_ptr(), buf.len() as u32) };
        if result < 0 {
            return None;
        }
        Some(
            unsafe { CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .to_string(),
        )
    }

    // ── Undo delta (plugin-side, draft) ──

    pub fn undo_get_delta_properties(&self) -> Option<UndoDeltaProperties> {
        if self.extensions.undo_delta.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.undo_delta };
        let get_fn = ext.get_delta_properties?;
        let mut props: clap_undo_delta_properties = unsafe { std::mem::zeroed() };
        unsafe { get_fn(self.plugin, &mut props) };
        Some(UndoDeltaProperties {
            has_delta: props.has_delta,
            are_deltas_persistent: props.are_deltas_persistent,
            format_version: props.format_version,
        })
    }

    pub fn undo_can_use_format_version(&self, version: u32) -> bool {
        if self.extensions.undo_delta.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.undo_delta };
        match ext.can_use_delta_format_version {
            Some(f) => unsafe { f(self.plugin, version) },
            None => false,
        }
    }

    pub fn undo_apply_delta(&mut self, format_version: u32, delta: &[u8]) -> bool {
        if self.extensions.undo_delta.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.undo_delta };
        match ext.undo {
            Some(f) => unsafe {
                f(
                    self.plugin,
                    format_version,
                    delta.as_ptr() as *const _,
                    delta.len(),
                )
            },
            None => false,
        }
    }

    pub fn redo_apply_delta(&mut self, format_version: u32, delta: &[u8]) -> bool {
        if self.extensions.undo_delta.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.undo_delta };
        match ext.redo {
            Some(f) => unsafe {
                f(
                    self.plugin,
                    format_version,
                    delta.as_ptr() as *const _,
                    delta.len(),
                )
            },
            None => false,
        }
    }

    // ── Undo context (plugin-side, draft) ──

    pub fn undo_set_can_undo(&self, can_undo: bool) {
        if self.extensions.undo_context.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.undo_context };
        if let Some(f) = ext.set_can_undo {
            unsafe { f(self.plugin, can_undo) };
        }
    }

    pub fn undo_set_can_redo(&self, can_redo: bool) {
        if self.extensions.undo_context.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.undo_context };
        if let Some(f) = ext.set_can_redo {
            unsafe { f(self.plugin, can_redo) };
        }
    }

    pub fn undo_set_undo_name(&self, name: &str) {
        if self.extensions.undo_context.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.undo_context };
        if let Some(f) = ext.set_undo_name {
            if let Ok(cstr) = std::ffi::CString::new(name) {
                unsafe { f(self.plugin, cstr.as_ptr()) };
            }
        }
    }

    pub fn undo_set_redo_name(&self, name: &str) {
        if self.extensions.undo_context.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.undo_context };
        if let Some(f) = ext.set_redo_name {
            if let Ok(cstr) = std::ffi::CString::new(name) {
                unsafe { f(self.plugin, cstr.as_ptr()) };
            }
        }
    }

    // ── POSIX FD support (plugin-side, unix only) ──

    #[cfg(unix)]
    pub fn poll_posix_fds(&mut self) -> usize {
        if self.extensions.posix_fd_support.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.posix_fd_support };
        let on_fd = match ext.on_fd {
            Some(f) => f,
            None => return 0,
        };

        let fds: Vec<(i32, u32)> = if let Ok(guard) = self.host_state.posix_fds.lock() {
            guard.iter().map(|e| (e.fd, e.flags)).collect()
        } else {
            return 0;
        };

        let mut fired = 0;
        for (fd, flags) in fds {
            unsafe { on_fd(self.plugin, fd, flags) };
            fired += 1;
        }
        fired
    }
}

unsafe extern "C" fn context_menu_builder_add_item(
    builder: *const clap_context_menu_builder,
    item_kind: u32,
    item_data: *const c_void,
) -> bool {
    if builder.is_null() || (*builder).ctx.is_null() {
        return false;
    }
    let items = &mut *((*builder).ctx as *mut Vec<ContextMenuItem>);
    let item = match item_kind {
        CLAP_CONTEXT_MENU_ITEM_ENTRY => {
            if item_data.is_null() {
                return false;
            }
            let entry = &*(item_data as *const clap_context_menu_entry);
            ContextMenuItem::Entry {
                label: if entry.label.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(entry.label).to_string_lossy().to_string()
                },
                is_enabled: entry.is_enabled,
                action_id: entry.action_id,
            }
        }
        CLAP_CONTEXT_MENU_ITEM_CHECK_ENTRY => {
            if item_data.is_null() {
                return false;
            }
            let entry = &*(item_data as *const clap_context_menu_check_entry);
            ContextMenuItem::CheckEntry {
                label: if entry.label.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(entry.label).to_string_lossy().to_string()
                },
                is_enabled: entry.is_enabled,
                is_checked: entry.is_checked,
                action_id: entry.action_id,
            }
        }
        CLAP_CONTEXT_MENU_ITEM_SEPARATOR => ContextMenuItem::Separator,
        CLAP_CONTEXT_MENU_ITEM_TITLE => {
            if item_data.is_null() {
                return false;
            }
            let title = &*(item_data as *const clap_context_menu_item_title);
            ContextMenuItem::Title {
                title: if title.title.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(title.title).to_string_lossy().to_string()
                },
                is_enabled: title.is_enabled,
            }
        }
        CLAP_CONTEXT_MENU_ITEM_BEGIN_SUBMENU => {
            if item_data.is_null() {
                return false;
            }
            let sub = &*(item_data as *const clap_context_menu_submenu);
            ContextMenuItem::BeginSubmenu {
                label: if sub.label.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(sub.label).to_string_lossy().to_string()
                },
                is_enabled: sub.is_enabled,
            }
        }
        CLAP_CONTEXT_MENU_ITEM_END_SUBMENU => ContextMenuItem::EndSubmenu,
        _ => return false,
    };
    items.push(item);
    true
}

unsafe extern "C" fn context_menu_builder_supports(
    _builder: *const clap_context_menu_builder,
    item_kind: u32,
) -> bool {
    matches!(
        item_kind,
        CLAP_CONTEXT_MENU_ITEM_ENTRY
            | CLAP_CONTEXT_MENU_ITEM_CHECK_ENTRY
            | CLAP_CONTEXT_MENU_ITEM_SEPARATOR
            | CLAP_CONTEXT_MENU_ITEM_TITLE
            | CLAP_CONTEXT_MENU_ITEM_BEGIN_SUBMENU
            | CLAP_CONTEXT_MENU_ITEM_END_SUBMENU
    )
}

fn build_port_config_requests(
    requests: &[AudioPortConfigRequest],
) -> Vec<clap_audio_port_configuration_request> {
    requests
        .iter()
        .map(|r| clap_audio_port_configuration_request {
            is_input: r.is_input,
            port_index: r.port_index,
            channel_count: r.channel_count,
            port_type: ptr::null(),
            port_details: ptr::null(),
        })
        .collect()
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

#[derive(Debug, Clone, Default)]
pub struct ProcessOutput {
    pub midi_events: Vec<MidiEvent>,
    pub param_changes: ParameterChanges,
    pub note_expressions: Vec<NoteExpressionValue>,
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

fn build_clap_transport(transport: &TransportInfo) -> clap_event_transport {
    use clap_sys::events::{
        clap_event_header, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_TRANSPORT,
        CLAP_TRANSPORT_HAS_BEATS_TIMELINE, CLAP_TRANSPORT_HAS_SECONDS_TIMELINE,
        CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
        CLAP_TRANSPORT_IS_PLAYING, CLAP_TRANSPORT_IS_RECORDING,
    };
    use clap_sys::fixedpoint::CLAP_SECTIME_FACTOR;

    let mut flags: u32 = CLAP_TRANSPORT_HAS_TEMPO
        | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
        | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
        | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;

    if transport.playing {
        flags |= CLAP_TRANSPORT_IS_PLAYING;
    }
    if transport.recording {
        flags |= CLAP_TRANSPORT_IS_RECORDING;
    }
    if transport.cycle_active {
        flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
    }

    clap_event_transport {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_transport>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: CLAP_EVENT_TRANSPORT,
            flags: 0,
        },
        flags,
        song_pos_beats: (transport.song_pos_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        song_pos_seconds: (transport.song_pos_seconds * CLAP_SECTIME_FACTOR as f64) as i64,
        tempo: transport.tempo,
        tempo_inc: 0.0,
        loop_start_beats: (transport.loop_start_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        loop_end_beats: (transport.loop_end_beats * CLAP_BEATTIME_FACTOR as f64) as i64,
        loop_start_seconds: 0,
        loop_end_seconds: 0,
        bar_start: (transport.bar_start * CLAP_BEATTIME_FACTOR as f64) as i64,
        bar_number: transport.bar_number,
        tsig_num: transport.time_sig_numerator as u16,
        tsig_denom: transport.time_sig_denominator as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
