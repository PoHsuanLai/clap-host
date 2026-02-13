//! CLAP host implementation.
//!
//! Provides the host-side callbacks that plugins use to communicate
//! with the host application. Supports thread-check, log, params, state,
//! latency, tail, gui, audio-ports, and note-ports host extensions.

use crate::types::{TrackInfo, TransportRequest, TuningInfo, UndoChange};
use clap_sys::ext::ambisonic::{clap_host_ambisonic, CLAP_EXT_AMBISONIC, CLAP_PORT_AMBISONIC};
use clap_sys::ext::audio_ports::{
    clap_host_audio_ports, CLAP_EXT_AUDIO_PORTS, CLAP_PORT_MONO, CLAP_PORT_STEREO,
};
use clap_sys::ext::audio_ports_config::{
    clap_host_audio_ports_config, CLAP_EXT_AUDIO_PORTS_CONFIG,
};
use clap_sys::ext::context_menu::{
    clap_context_menu_builder, clap_context_menu_target, clap_host_context_menu,
    CLAP_EXT_CONTEXT_MENU,
};
use clap_sys::ext::draft::resource_directory::{
    clap_host_resource_directory, CLAP_EXT_RESOURCE_DIRECTORY,
};
use clap_sys::ext::draft::transport_control::{
    clap_host_transport_control, CLAP_EXT_TRANSPORT_CONTROL,
};
use clap_sys::ext::draft::triggers::{clap_host_triggers, CLAP_EXT_TRIGGERS};
use clap_sys::ext::draft::tuning::{clap_host_tuning, clap_tuning_info, CLAP_EXT_TUNING};
use clap_sys::ext::draft::undo::{clap_host_undo, CLAP_EXT_UNDO};
use clap_sys::ext::event_registry::{clap_host_event_registry, CLAP_EXT_EVENT_REGISTRY};
use clap_sys::ext::gui::{clap_host_gui, CLAP_EXT_GUI};
use clap_sys::ext::latency::{clap_host_latency, CLAP_EXT_LATENCY};
use clap_sys::ext::log::{
    clap_host_log, clap_log_severity, CLAP_EXT_LOG, CLAP_LOG_DEBUG, CLAP_LOG_ERROR, CLAP_LOG_FATAL,
    CLAP_LOG_HOST_MISBEHAVING, CLAP_LOG_INFO, CLAP_LOG_PLUGIN_MISBEHAVING, CLAP_LOG_WARNING,
};
use clap_sys::ext::note_name::{clap_host_note_name, CLAP_EXT_NOTE_NAME};
use clap_sys::ext::note_ports::{
    clap_host_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP, CLAP_NOTE_DIALECT_MIDI,
};
use clap_sys::ext::params::{clap_host_params, CLAP_EXT_PARAMS};
#[cfg(unix)]
use clap_sys::ext::posix_fd_support::{clap_host_posix_fd_support, CLAP_EXT_POSIX_FD_SUPPORT};
use clap_sys::ext::preset_load::{clap_host_preset_load, CLAP_EXT_PRESET_LOAD};
use clap_sys::ext::remote_controls::{clap_host_remote_controls, CLAP_EXT_REMOTE_CONTROLS};
use clap_sys::ext::state::{clap_host_state, CLAP_EXT_STATE};
use clap_sys::ext::surround::CLAP_PORT_SURROUND;
use clap_sys::ext::surround::{clap_host_surround, CLAP_EXT_SURROUND};
use clap_sys::ext::tail::{clap_host_tail, CLAP_EXT_TAIL};
use clap_sys::ext::thread_check::{clap_host_thread_check, CLAP_EXT_THREAD_CHECK};
use clap_sys::ext::thread_pool::{clap_host_thread_pool, CLAP_EXT_THREAD_POOL};
use clap_sys::ext::timer_support::{clap_host_timer_support, CLAP_EXT_TIMER_SUPPORT};
use clap_sys::ext::track_info::{
    clap_host_track_info, clap_track_info, CLAP_EXT_TRACK_INFO, CLAP_TRACK_INFO_HAS_AUDIO_CHANNEL,
    CLAP_TRACK_INFO_HAS_TRACK_COLOR, CLAP_TRACK_INFO_HAS_TRACK_NAME, CLAP_TRACK_INFO_IS_FOR_BUS,
    CLAP_TRACK_INFO_IS_FOR_MASTER, CLAP_TRACK_INFO_IS_FOR_RETURN_TRACK,
};
use clap_sys::ext::voice_info::{clap_host_voice_info, CLAP_EXT_VOICE_INFO};
use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
use clap_sys::host::clap_host;
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::CLAP_VERSION;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Instant;

pub(crate) struct TimerEntry {
    pub id: u32,
    pub period_ms: u32,
    pub last_fire: Instant,
}

#[cfg(unix)]
pub struct PosixFdEntry {
    pub fd: i32,
    pub flags: u32,
}

/// Shared state for host↔plugin communication via atomic flags.
pub struct HostState {
    pub restart_requested: AtomicBool,
    pub process_requested: AtomicBool,
    pub callback_requested: AtomicBool,
    pub latency_changed: AtomicBool,
    pub tail_changed: AtomicBool,
    pub params_rescan_requested: AtomicBool,
    pub params_flush_requested: AtomicBool,
    pub audio_ports_changed: AtomicBool,
    pub note_ports_changed: AtomicBool,
    pub state_dirty: AtomicBool,
    pub gui_closed: AtomicBool,
    pub gui_resize_hints_changed: AtomicBool,
    pub gui_request_resize_width: AtomicU32,
    pub gui_request_resize_height: AtomicU32,
    pub main_thread_id: ThreadId,
    pub(crate) timers: Mutex<Vec<TimerEntry>>,
    pub(crate) next_timer_id: AtomicU32,
    pub audio_ports_config_changed: AtomicBool,
    pub remote_controls_changed: AtomicBool,
    pub(crate) suggested_remote_page: AtomicU32,
    pub(crate) track_info: Mutex<Option<TrackInfo>>,
    pub(crate) event_spaces: Mutex<HashMap<String, u16>>,
    pub(crate) next_event_space: AtomicU16,
    pub(crate) transport_requests: Mutex<Vec<TransportRequest>>,
    pub thread_pool_pending: AtomicU32,
    pub ambisonic_changed: AtomicBool,
    pub surround_changed: AtomicBool,
    #[cfg(unix)]
    pub posix_fds: Mutex<Vec<PosixFdEntry>>,
    pub triggers_rescan_requested: AtomicBool,
    pub(crate) tuning_infos: Mutex<Vec<TuningInfo>>,
    pub(crate) resource_directory_shared: Mutex<Option<std::path::PathBuf>>,
    pub(crate) resource_directory_private: Mutex<Option<std::path::PathBuf>>,
    pub note_names_changed: AtomicBool,
    pub voice_info_changed: AtomicBool,
    pub preset_loaded: AtomicBool,
    pub undo_in_progress: AtomicBool,
    pub undo_requested: AtomicBool,
    pub redo_requested: AtomicBool,
    pub undo_wants_context: AtomicBool,
    pub undo_changes: Mutex<Vec<UndoChange>>,
}

impl HostState {
    pub fn new() -> Self {
        Self {
            restart_requested: AtomicBool::new(false),
            process_requested: AtomicBool::new(false),
            callback_requested: AtomicBool::new(false),
            latency_changed: AtomicBool::new(false),
            tail_changed: AtomicBool::new(false),
            params_rescan_requested: AtomicBool::new(false),
            params_flush_requested: AtomicBool::new(false),
            audio_ports_changed: AtomicBool::new(false),
            note_ports_changed: AtomicBool::new(false),
            state_dirty: AtomicBool::new(false),
            gui_closed: AtomicBool::new(false),
            gui_resize_hints_changed: AtomicBool::new(false),
            gui_request_resize_width: AtomicU32::new(0),
            gui_request_resize_height: AtomicU32::new(0),
            main_thread_id: std::thread::current().id(),
            timers: Mutex::new(Vec::new()),
            next_timer_id: AtomicU32::new(1),
            audio_ports_config_changed: AtomicBool::new(false),
            remote_controls_changed: AtomicBool::new(false),
            suggested_remote_page: AtomicU32::new(u32::MAX),
            track_info: Mutex::new(None),
            event_spaces: Mutex::new(HashMap::new()),
            next_event_space: AtomicU16::new(512),
            transport_requests: Mutex::new(Vec::new()),
            thread_pool_pending: AtomicU32::new(0),
            ambisonic_changed: AtomicBool::new(false),
            surround_changed: AtomicBool::new(false),
            #[cfg(unix)]
            posix_fds: Mutex::new(Vec::new()),
            triggers_rescan_requested: AtomicBool::new(false),
            tuning_infos: Mutex::new(Vec::new()),
            resource_directory_shared: Mutex::new(None),
            resource_directory_private: Mutex::new(None),
            note_names_changed: AtomicBool::new(false),
            voice_info_changed: AtomicBool::new(false),
            preset_loaded: AtomicBool::new(false),
            undo_in_progress: AtomicBool::new(false),
            undo_requested: AtomicBool::new(false),
            redo_requested: AtomicBool::new(false),
            undo_wants_context: AtomicBool::new(false),
            undo_changes: Mutex::new(Vec::new()),
        }
    }

    /// Check and clear an atomic flag. Returns true if it was set.
    pub fn poll(&self, flag: &AtomicBool) -> bool {
        flag.swap(false, Ordering::AcqRel)
    }
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ClapHost {
    inner: clap_host,
    state: Arc<HostState>,
}

impl ClapHost {
    pub fn new(state: Arc<HostState>) -> Self {
        let mut host = Self {
            inner: clap_host {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: c"clap-host".as_ptr(),
                vendor: c"Rust".as_ptr(),
                url: c"".as_ptr(),
                version: c"0.1.0".as_ptr(),
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_request_process),
                request_callback: Some(host_request_callback),
            },
            state,
        };
        host.inner.host_data = Arc::as_ptr(&host.state) as *mut c_void;
        host
    }

    pub fn as_raw(&self) -> *const clap_host {
        &self.inner
    }

    pub fn state(&self) -> &Arc<HostState> {
        &self.state
    }
}

impl Default for ClapHost {
    fn default() -> Self {
        Self::new(Arc::new(HostState::new()))
    }
}

// ── Helper to extract HostState from host_data ──

unsafe fn get_host_state<'a>(host: *const clap_host) -> Option<&'a HostState> {
    if host.is_null() {
        return None;
    }
    let data = (*host).host_data;
    if data.is_null() {
        return None;
    }
    Some(&*(data as *const HostState))
}

// ── Core host callbacks ──

unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    extension_id: *const i8,
) -> *const c_void {
    if extension_id.is_null() {
        return ptr::null();
    }
    let id = CStr::from_ptr(extension_id);

    if id == CLAP_EXT_THREAD_CHECK {
        return &HOST_THREAD_CHECK as *const clap_host_thread_check as *const c_void;
    }
    if id == CLAP_EXT_LOG {
        return &HOST_LOG as *const clap_host_log as *const c_void;
    }
    if id == CLAP_EXT_PARAMS {
        return &HOST_PARAMS as *const clap_host_params as *const c_void;
    }
    if id == CLAP_EXT_STATE {
        return &HOST_STATE as *const clap_host_state as *const c_void;
    }
    if id == CLAP_EXT_LATENCY {
        return &HOST_LATENCY as *const clap_host_latency as *const c_void;
    }
    if id == CLAP_EXT_TAIL {
        return &HOST_TAIL as *const clap_host_tail as *const c_void;
    }
    if id == CLAP_EXT_GUI {
        return &HOST_GUI as *const clap_host_gui as *const c_void;
    }
    if id == CLAP_EXT_AUDIO_PORTS {
        return &HOST_AUDIO_PORTS as *const clap_host_audio_ports as *const c_void;
    }
    if id == CLAP_EXT_NOTE_PORTS {
        return &HOST_NOTE_PORTS as *const clap_host_note_ports as *const c_void;
    }
    if id == CLAP_EXT_TIMER_SUPPORT {
        return &HOST_TIMER_SUPPORT as *const clap_host_timer_support as *const c_void;
    }
    if id == CLAP_EXT_NOTE_NAME {
        return &HOST_NOTE_NAME as *const clap_host_note_name as *const c_void;
    }
    if id == CLAP_EXT_VOICE_INFO {
        return &HOST_VOICE_INFO as *const clap_host_voice_info as *const c_void;
    }
    if id == CLAP_EXT_PRESET_LOAD {
        return &HOST_PRESET_LOAD as *const clap_host_preset_load as *const c_void;
    }
    if id == CLAP_EXT_AUDIO_PORTS_CONFIG {
        return &HOST_AUDIO_PORTS_CONFIG as *const clap_host_audio_ports_config as *const c_void;
    }
    if id == CLAP_EXT_REMOTE_CONTROLS {
        return &HOST_REMOTE_CONTROLS as *const clap_host_remote_controls as *const c_void;
    }
    if id == CLAP_EXT_TRACK_INFO {
        return &HOST_TRACK_INFO as *const clap_host_track_info as *const c_void;
    }
    if id == CLAP_EXT_EVENT_REGISTRY {
        return &HOST_EVENT_REGISTRY as *const clap_host_event_registry as *const c_void;
    }
    if id == CLAP_EXT_TRANSPORT_CONTROL {
        return &HOST_TRANSPORT_CONTROL as *const clap_host_transport_control as *const c_void;
    }
    if id == CLAP_EXT_CONTEXT_MENU {
        return &HOST_CONTEXT_MENU as *const clap_host_context_menu as *const c_void;
    }
    if id == CLAP_EXT_THREAD_POOL {
        return &HOST_THREAD_POOL as *const clap_host_thread_pool as *const c_void;
    }
    if id == CLAP_EXT_AMBISONIC {
        return &HOST_AMBISONIC as *const clap_host_ambisonic as *const c_void;
    }
    if id == CLAP_EXT_SURROUND {
        return &HOST_SURROUND as *const clap_host_surround as *const c_void;
    }
    #[cfg(unix)]
    if id == CLAP_EXT_POSIX_FD_SUPPORT {
        return &HOST_POSIX_FD_SUPPORT as *const clap_host_posix_fd_support as *const c_void;
    }
    if id == CLAP_EXT_TRIGGERS {
        return &HOST_TRIGGERS as *const clap_host_triggers as *const c_void;
    }
    if id == CLAP_EXT_TUNING {
        return &HOST_TUNING as *const clap_host_tuning as *const c_void;
    }
    if id == CLAP_EXT_RESOURCE_DIRECTORY {
        return &HOST_RESOURCE_DIRECTORY as *const clap_host_resource_directory as *const c_void;
    }
    if id == CLAP_EXT_UNDO {
        return &HOST_UNDO as *const clap_host_undo as *const c_void;
    }

    ptr::null()
}

unsafe extern "C" fn host_request_restart(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.restart_requested.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_request_process(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.process_requested.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_request_callback(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.callback_requested.store(true, Ordering::Release);
    }
}

// ── Thread check extension ──

static HOST_THREAD_CHECK: clap_host_thread_check = clap_host_thread_check {
    is_main_thread: Some(host_thread_check_is_main),
    is_audio_thread: Some(host_thread_check_is_audio),
};

unsafe extern "C" fn host_thread_check_is_main(host: *const clap_host) -> bool {
    match get_host_state(host) {
        Some(state) => std::thread::current().id() == state.main_thread_id,
        None => false,
    }
}

unsafe extern "C" fn host_thread_check_is_audio(host: *const clap_host) -> bool {
    // Audio thread is any thread that is NOT the main thread
    match get_host_state(host) {
        Some(state) => std::thread::current().id() != state.main_thread_id,
        None => false,
    }
}

// ── Log extension ──

static HOST_LOG: clap_host_log = clap_host_log {
    log: Some(host_log),
};

unsafe extern "C" fn host_log(
    _host: *const clap_host,
    severity: clap_log_severity,
    msg: *const c_char,
) {
    if msg.is_null() {
        return;
    }
    let msg_str = CStr::from_ptr(msg).to_string_lossy();
    match severity {
        CLAP_LOG_DEBUG => eprintln!("[clap-plugin DEBUG] {}", msg_str),
        CLAP_LOG_INFO => eprintln!("[clap-plugin INFO] {}", msg_str),
        CLAP_LOG_WARNING => eprintln!("[clap-plugin WARN] {}", msg_str),
        CLAP_LOG_ERROR => eprintln!("[clap-plugin ERROR] {}", msg_str),
        CLAP_LOG_FATAL => eprintln!("[clap-plugin FATAL] {}", msg_str),
        CLAP_LOG_HOST_MISBEHAVING => eprintln!("[clap-plugin HOST-MISBEHAVING] {}", msg_str),
        CLAP_LOG_PLUGIN_MISBEHAVING => eprintln!("[clap-plugin PLUGIN-MISBEHAVING] {}", msg_str),
        _ => eprintln!("[clap-plugin ?{}] {}", severity, msg_str),
    }
}

// ── Params extension ──

static HOST_PARAMS: clap_host_params = clap_host_params {
    rescan: Some(host_params_rescan),
    clear: Some(host_params_clear),
    request_flush: Some(host_params_request_flush),
};

unsafe extern "C" fn host_params_rescan(host: *const clap_host, _flags: u32) {
    if let Some(state) = get_host_state(host) {
        state.params_rescan_requested.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_params_clear(_host: *const clap_host, _param_id: u32, _flags: u32) {
    // No-op: we don't cache parameter values on the host side
}

unsafe extern "C" fn host_params_request_flush(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.params_flush_requested.store(true, Ordering::Release);
    }
}

// ── State extension ──

static HOST_STATE: clap_host_state = clap_host_state {
    mark_dirty: Some(host_state_mark_dirty),
};

unsafe extern "C" fn host_state_mark_dirty(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.state_dirty.store(true, Ordering::Release);
    }
}

// ── Latency extension ──

static HOST_LATENCY: clap_host_latency = clap_host_latency {
    changed: Some(host_latency_changed),
};

unsafe extern "C" fn host_latency_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.latency_changed.store(true, Ordering::Release);
    }
}

// ── Tail extension ──

static HOST_TAIL: clap_host_tail = clap_host_tail {
    changed: Some(host_tail_changed),
};

unsafe extern "C" fn host_tail_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.tail_changed.store(true, Ordering::Release);
    }
}

// ── GUI extension ──

static HOST_GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(host_gui_resize_hints_changed),
    request_resize: Some(host_gui_request_resize),
    request_show: Some(host_gui_request_show),
    request_hide: Some(host_gui_request_hide),
    closed: Some(host_gui_closed),
};

unsafe extern "C" fn host_gui_resize_hints_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state
            .gui_resize_hints_changed
            .store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_gui_request_resize(
    host: *const clap_host,
    width: u32,
    height: u32,
) -> bool {
    if let Some(state) = get_host_state(host) {
        state
            .gui_request_resize_width
            .store(width, Ordering::Release);
        state
            .gui_request_resize_height
            .store(height, Ordering::Release);
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_gui_request_show(_host: *const clap_host) -> bool {
    true
}

unsafe extern "C" fn host_gui_request_hide(_host: *const clap_host) -> bool {
    true
}

unsafe extern "C" fn host_gui_closed(host: *const clap_host, _was_destroyed: bool) {
    if let Some(state) = get_host_state(host) {
        state.gui_closed.store(true, Ordering::Release);
    }
}

// ── Audio ports extension ──

static HOST_AUDIO_PORTS: clap_host_audio_ports = clap_host_audio_ports {
    is_rescan_flag_supported: Some(host_audio_ports_is_rescan_flag_supported),
    rescan: Some(host_audio_ports_rescan),
};

unsafe extern "C" fn host_audio_ports_is_rescan_flag_supported(
    _host: *const clap_host,
    _flag: u32,
) -> bool {
    // We support all rescan flags — we re-query ports from scratch
    true
}

unsafe extern "C" fn host_audio_ports_rescan(host: *const clap_host, _flags: u32) {
    if let Some(state) = get_host_state(host) {
        state.audio_ports_changed.store(true, Ordering::Release);
    }
}

// ── Note ports extension ──

static HOST_NOTE_PORTS: clap_host_note_ports = clap_host_note_ports {
    supported_dialects: Some(host_note_ports_supported_dialects),
    rescan: Some(host_note_ports_rescan),
};

unsafe extern "C" fn host_note_ports_supported_dialects(_host: *const clap_host) -> u32 {
    CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI
}

unsafe extern "C" fn host_note_ports_rescan(host: *const clap_host, _flags: u32) {
    if let Some(state) = get_host_state(host) {
        state.note_ports_changed.store(true, Ordering::Release);
    }
}

// ── Timer support extension ──

static HOST_TIMER_SUPPORT: clap_host_timer_support = clap_host_timer_support {
    register_timer: Some(host_timer_register),
    unregister_timer: Some(host_timer_unregister),
};

unsafe extern "C" fn host_timer_register(
    host: *const clap_host,
    period_ms: u32,
    timer_id: *mut u32,
) -> bool {
    if timer_id.is_null() {
        return false;
    }
    let Some(state) = get_host_state(host) else {
        return false;
    };
    let id = state.next_timer_id.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut timers) = state.timers.lock() {
        timers.push(TimerEntry {
            id,
            period_ms,
            last_fire: Instant::now(),
        });
        *timer_id = id;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_timer_unregister(host: *const clap_host, timer_id: u32) -> bool {
    let Some(state) = get_host_state(host) else {
        return false;
    };
    if let Ok(mut timers) = state.timers.lock() {
        let len_before = timers.len();
        timers.retain(|t| t.id != timer_id);
        timers.len() < len_before
    } else {
        false
    }
}

// ── Note name extension ──

static HOST_NOTE_NAME: clap_host_note_name = clap_host_note_name {
    changed: Some(host_note_name_changed),
};

unsafe extern "C" fn host_note_name_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.note_names_changed.store(true, Ordering::Release);
    }
}

// ── Voice info extension ──

static HOST_VOICE_INFO: clap_host_voice_info = clap_host_voice_info {
    changed: Some(host_voice_info_changed),
};

unsafe extern "C" fn host_voice_info_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.voice_info_changed.store(true, Ordering::Release);
    }
}

// ── Preset load extension ──

static HOST_PRESET_LOAD: clap_host_preset_load = clap_host_preset_load {
    on_error: Some(host_preset_load_on_error),
    loaded: Some(host_preset_load_loaded),
};

unsafe extern "C" fn host_preset_load_on_error(
    _host: *const clap_host,
    _location_kind: u32,
    _location: *const c_char,
    _load_key: *const c_char,
    _os_error: i32,
    _msg: *const c_char,
) {
    // No-op: host can log or display error if needed
}

unsafe extern "C" fn host_preset_load_loaded(
    host: *const clap_host,
    _location_kind: u32,
    _location: *const c_char,
    _load_key: *const c_char,
) {
    if let Some(state) = get_host_state(host) {
        state.preset_loaded.store(true, Ordering::Release);
    }
}

// ── Audio ports config extension ──

static HOST_AUDIO_PORTS_CONFIG: clap_host_audio_ports_config = clap_host_audio_ports_config {
    rescan: Some(host_audio_ports_config_rescan),
};

unsafe extern "C" fn host_audio_ports_config_rescan(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state
            .audio_ports_config_changed
            .store(true, Ordering::Release);
    }
}

// ── Remote controls extension ──

static HOST_REMOTE_CONTROLS: clap_host_remote_controls = clap_host_remote_controls {
    changed: Some(host_remote_controls_changed),
    suggest_page: Some(host_remote_controls_suggest_page),
};

unsafe extern "C" fn host_remote_controls_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.remote_controls_changed.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_remote_controls_suggest_page(host: *const clap_host, page_id: u32) {
    if let Some(state) = get_host_state(host) {
        state
            .suggested_remote_page
            .store(page_id, Ordering::Release);
    }
}

// ── Track info extension ──

static HOST_TRACK_INFO: clap_host_track_info = clap_host_track_info {
    get: Some(host_track_info_get),
};

unsafe extern "C" fn host_track_info_get(
    host: *const clap_host,
    info: *mut clap_track_info,
) -> bool {
    if info.is_null() {
        return false;
    }
    let Some(state) = get_host_state(host) else {
        return false;
    };
    let Ok(guard) = state.track_info.lock() else {
        return false;
    };
    let Some(track) = guard.as_ref() else {
        return false;
    };

    let out = &mut *info;
    out.flags = 0;

    if let Some(ref name) = track.name {
        out.flags |= CLAP_TRACK_INFO_HAS_TRACK_NAME;
        let bytes = name.as_bytes();
        let len = bytes.len().min(out.name.len() - 1);
        ptr::copy_nonoverlapping(bytes.as_ptr(), out.name.as_mut_ptr() as *mut u8, len);
        out.name[len] = 0;
    }

    if let Some(color) = track.color {
        out.flags |= CLAP_TRACK_INFO_HAS_TRACK_COLOR;
        out.color.alpha = color.alpha;
        out.color.red = color.red;
        out.color.green = color.green;
        out.color.blue = color.blue;
    }

    if let Some(ch) = track.audio_channel_count {
        out.flags |= CLAP_TRACK_INFO_HAS_AUDIO_CHANNEL;
        out.audio_channel_count = ch;
    }

    // SAFETY: audio_port_type must point to a well-known CLAP constant string (e.g.
    // CLAP_PORT_MONO, CLAP_PORT_STEREO) or be null. We match known values to static
    // C string pointers to avoid a dangling-pointer from a temporary CString.
    out.audio_port_type = match track.audio_port_type.as_deref() {
        Some("mono") => CLAP_PORT_MONO.as_ptr(),
        Some("stereo") => CLAP_PORT_STEREO.as_ptr(),
        Some("surround") => CLAP_PORT_SURROUND.as_ptr(),
        Some("ambisonic") => CLAP_PORT_AMBISONIC.as_ptr(),
        _ => ptr::null(),
    };

    if track.is_return_track {
        out.flags |= CLAP_TRACK_INFO_IS_FOR_RETURN_TRACK;
    }
    if track.is_bus {
        out.flags |= CLAP_TRACK_INFO_IS_FOR_BUS;
    }
    if track.is_master {
        out.flags |= CLAP_TRACK_INFO_IS_FOR_MASTER;
    }

    true
}

// ── Event registry extension ──

static HOST_EVENT_REGISTRY: clap_host_event_registry = clap_host_event_registry {
    query: Some(host_event_registry_query),
};

unsafe extern "C" fn host_event_registry_query(
    host: *const clap_host,
    space_name: *const c_char,
    space_id: *mut u16,
) -> bool {
    if space_name.is_null() || space_id.is_null() {
        return false;
    }
    let Some(state) = get_host_state(host) else {
        return false;
    };
    let name = CStr::from_ptr(space_name).to_string_lossy().to_string();
    let Ok(mut spaces) = state.event_spaces.lock() else {
        return false;
    };
    let id = *spaces
        .entry(name)
        .or_insert_with(|| state.next_event_space.fetch_add(1, Ordering::Relaxed));
    *space_id = id;
    true
}

// ── Transport control extension (draft) ──

static HOST_TRANSPORT_CONTROL: clap_host_transport_control = clap_host_transport_control {
    request_start: Some(host_transport_request_start),
    request_stop: Some(host_transport_request_stop),
    request_continue: Some(host_transport_request_continue),
    request_pause: Some(host_transport_request_pause),
    request_toggle_play: Some(host_transport_request_toggle_play),
    request_jump: Some(host_transport_request_jump),
    request_loop_region: Some(host_transport_request_loop_region),
    request_toggle_loop: Some(host_transport_request_toggle_loop),
    request_enable_loop: Some(host_transport_request_enable_loop),
    request_record: Some(host_transport_request_record),
    request_toggle_record: Some(host_transport_request_toggle_record),
};

fn push_transport_request(host: *const clap_host, req: TransportRequest) {
    if let Some(state) = unsafe { get_host_state(host) } {
        if let Ok(mut reqs) = state.transport_requests.lock() {
            reqs.push(req);
        }
    }
}

unsafe extern "C" fn host_transport_request_start(host: *const clap_host) {
    push_transport_request(host, TransportRequest::Start);
}

unsafe extern "C" fn host_transport_request_stop(host: *const clap_host) {
    push_transport_request(host, TransportRequest::Stop);
}

unsafe extern "C" fn host_transport_request_continue(host: *const clap_host) {
    push_transport_request(host, TransportRequest::Continue);
}

unsafe extern "C" fn host_transport_request_pause(host: *const clap_host) {
    push_transport_request(host, TransportRequest::Pause);
}

unsafe extern "C" fn host_transport_request_toggle_play(host: *const clap_host) {
    push_transport_request(host, TransportRequest::TogglePlay);
}

unsafe extern "C" fn host_transport_request_jump(
    host: *const clap_host,
    position: clap_sys::fixedpoint::clap_beattime,
) {
    push_transport_request(
        host,
        TransportRequest::Jump {
            position_beats: position as f64 / CLAP_BEATTIME_FACTOR as f64,
        },
    );
}

unsafe extern "C" fn host_transport_request_loop_region(
    host: *const clap_host,
    start: clap_sys::fixedpoint::clap_beattime,
    duration: clap_sys::fixedpoint::clap_beattime,
) {
    push_transport_request(
        host,
        TransportRequest::LoopRegion {
            start_beats: start as f64 / CLAP_BEATTIME_FACTOR as f64,
            duration_beats: duration as f64 / CLAP_BEATTIME_FACTOR as f64,
        },
    );
}

unsafe extern "C" fn host_transport_request_toggle_loop(host: *const clap_host) {
    push_transport_request(host, TransportRequest::ToggleLoop);
}

unsafe extern "C" fn host_transport_request_enable_loop(host: *const clap_host, is_enabled: bool) {
    push_transport_request(host, TransportRequest::EnableLoop(is_enabled));
}

unsafe extern "C" fn host_transport_request_record(host: *const clap_host, is_recording: bool) {
    push_transport_request(host, TransportRequest::Record(is_recording));
}

unsafe extern "C" fn host_transport_request_toggle_record(host: *const clap_host) {
    push_transport_request(host, TransportRequest::ToggleRecord);
}

// ── Context menu extension ──

static HOST_CONTEXT_MENU: clap_host_context_menu = clap_host_context_menu {
    populate: Some(host_context_menu_populate),
    perform: Some(host_context_menu_perform),
    can_popup: Some(host_context_menu_can_popup),
    popup: Some(host_context_menu_popup),
};

unsafe extern "C" fn host_context_menu_populate(
    _host: *const clap_host,
    _target: *const clap_context_menu_target,
    _builder: *const clap_context_menu_builder,
) -> bool {
    // Host doesn't add items to plugin context menus by default
    true
}

unsafe extern "C" fn host_context_menu_perform(
    _host: *const clap_host,
    _target: *const clap_context_menu_target,
    _action_id: u32,
) -> bool {
    false
}

unsafe extern "C" fn host_context_menu_can_popup(_host: *const clap_host) -> bool {
    false
}

unsafe extern "C" fn host_context_menu_popup(
    _host: *const clap_host,
    _target: *const clap_context_menu_target,
    _screen_index: i32,
    _x: i32,
    _y: i32,
) -> bool {
    false
}

// ── Ambisonic extension ──

static HOST_AMBISONIC: clap_host_ambisonic = clap_host_ambisonic {
    changed: Some(host_ambisonic_changed),
};

unsafe extern "C" fn host_ambisonic_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.ambisonic_changed.store(true, Ordering::Release);
    }
}

// ── Surround extension ──

static HOST_SURROUND: clap_host_surround = clap_host_surround {
    changed: Some(host_surround_changed),
};

unsafe extern "C" fn host_surround_changed(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.surround_changed.store(true, Ordering::Release);
    }
}

// ── Thread pool extension ──

static HOST_THREAD_POOL: clap_host_thread_pool = clap_host_thread_pool {
    request_exec: Some(host_thread_pool_request_exec),
};

unsafe extern "C" fn host_thread_pool_request_exec(host: *const clap_host, num_tasks: u32) -> bool {
    if let Some(state) = get_host_state(host) {
        state
            .thread_pool_pending
            .store(num_tasks, Ordering::Release);
        true
    } else {
        false
    }
}

// ── Triggers extension (draft) ──

static HOST_TRIGGERS: clap_host_triggers = clap_host_triggers {
    rescan: Some(host_triggers_rescan),
    clear: Some(host_triggers_clear),
};

unsafe extern "C" fn host_triggers_rescan(host: *const clap_host, _flags: u32) {
    if let Some(state) = get_host_state(host) {
        state
            .triggers_rescan_requested
            .store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_triggers_clear(_host: *const clap_host, _trigger_id: u32, _flags: u32) {
    // No-op: host doesn't cache trigger state
}

// ── Tuning extension (draft) ──

static HOST_TUNING: clap_host_tuning = clap_host_tuning {
    get_relative: Some(host_tuning_get_relative),
    should_play: Some(host_tuning_should_play),
    get_tuning_count: Some(host_tuning_get_count),
    get_info: Some(host_tuning_get_info),
};

unsafe extern "C" fn host_tuning_get_relative(
    _host: *const clap_host,
    _tuning_id: u32,
    _channel: i32,
    _key: i32,
    _sample_offset: u32,
) -> f64 {
    // Return 0.0 = equal temperament (no detuning)
    0.0
}

unsafe extern "C" fn host_tuning_should_play(
    _host: *const clap_host,
    _tuning_id: u32,
    _channel: i32,
    _key: i32,
) -> bool {
    true
}

unsafe extern "C" fn host_tuning_get_count(host: *const clap_host) -> u32 {
    let Some(state) = get_host_state(host) else {
        return 0;
    };
    state
        .tuning_infos
        .lock()
        .map(|infos| infos.len() as u32)
        .unwrap_or(0)
}

unsafe extern "C" fn host_tuning_get_info(
    host: *const clap_host,
    tuning_index: u32,
    info: *mut clap_tuning_info,
) -> bool {
    if info.is_null() {
        return false;
    }
    let Some(state) = get_host_state(host) else {
        return false;
    };
    let Ok(infos) = state.tuning_infos.lock() else {
        return false;
    };
    let Some(tuning) = infos.get(tuning_index as usize) else {
        return false;
    };
    let out = &mut *info;
    out.tuning_id = tuning.tuning_id;
    out.is_dynamic = tuning.is_dynamic;
    let bytes = tuning.name.as_bytes();
    let len = bytes.len().min(out.name.len() - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr(), out.name.as_mut_ptr() as *mut u8, len);
    out.name[len] = 0;
    true
}

// ── Resource directory extension (draft) ──

static HOST_RESOURCE_DIRECTORY: clap_host_resource_directory = clap_host_resource_directory {
    request_directory: Some(host_resource_request_directory),
    release_directory: Some(host_resource_release_directory),
};

unsafe extern "C" fn host_resource_request_directory(
    host: *const clap_host,
    is_shared: bool,
) -> bool {
    let Some(state) = get_host_state(host) else {
        return false;
    };
    let lock = if is_shared {
        &state.resource_directory_shared
    } else {
        &state.resource_directory_private
    };
    // Return true if a directory is already configured
    lock.lock().map(|g| g.is_some()).unwrap_or(false)
}

unsafe extern "C" fn host_resource_release_directory(host: *const clap_host, is_shared: bool) {
    if let Some(state) = get_host_state(host) {
        let lock = if is_shared {
            &state.resource_directory_shared
        } else {
            &state.resource_directory_private
        };
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
}

// ── Undo extension (draft) ──

static HOST_UNDO: clap_host_undo = clap_host_undo {
    begin_change: Some(host_undo_begin_change),
    cancel_change: Some(host_undo_cancel_change),
    change_made: Some(host_undo_change_made),
    request_undo: Some(host_undo_request_undo),
    request_redo: Some(host_undo_request_redo),
    set_wants_context_updates: Some(host_undo_set_wants_context),
};

unsafe extern "C" fn host_undo_begin_change(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.undo_in_progress.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_undo_cancel_change(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.undo_in_progress.store(false, Ordering::Release);
    }
}

unsafe extern "C" fn host_undo_change_made(
    host: *const clap_host,
    name: *const c_char,
    delta: *const c_void,
    delta_size: usize,
    delta_can_undo: bool,
) {
    let Some(state) = get_host_state(host) else {
        return;
    };
    state.undo_in_progress.store(false, Ordering::Release);
    let change_name = if name.is_null() {
        String::new()
    } else {
        CStr::from_ptr(name).to_string_lossy().to_string()
    };
    let delta_data = if delta.is_null() || delta_size == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(delta as *const u8, delta_size).to_vec()
    };
    if let Ok(mut changes) = state.undo_changes.lock() {
        changes.push(UndoChange {
            name: change_name,
            delta: delta_data,
            delta_can_undo,
        });
    }
}

unsafe extern "C" fn host_undo_request_undo(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.undo_requested.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_undo_request_redo(host: *const clap_host) {
    if let Some(state) = get_host_state(host) {
        state.redo_requested.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn host_undo_set_wants_context(host: *const clap_host, is_subscribed: bool) {
    if let Some(state) = get_host_state(host) {
        state
            .undo_wants_context
            .store(is_subscribed, Ordering::Release);
    }
}

// ── POSIX FD support extension (unix only) ──

#[cfg(unix)]
static HOST_POSIX_FD_SUPPORT: clap_host_posix_fd_support = clap_host_posix_fd_support {
    register_fd: Some(host_posix_fd_register),
    modify_fd: Some(host_posix_fd_modify),
    unregister_fd: Some(host_posix_fd_unregister),
};

#[cfg(unix)]
unsafe extern "C" fn host_posix_fd_register(host: *const clap_host, fd: i32, flags: u32) -> bool {
    let Some(state) = get_host_state(host) else {
        return false;
    };
    if let Ok(mut fds) = state.posix_fds.lock() {
        // Don't register duplicates
        if fds.iter().any(|e| e.fd == fd) {
            return false;
        }
        fds.push(PosixFdEntry { fd, flags });
        true
    } else {
        false
    }
}

#[cfg(unix)]
unsafe extern "C" fn host_posix_fd_modify(host: *const clap_host, fd: i32, flags: u32) -> bool {
    let Some(state) = get_host_state(host) else {
        return false;
    };
    if let Ok(mut fds) = state.posix_fds.lock() {
        if let Some(entry) = fds.iter_mut().find(|e| e.fd == fd) {
            entry.flags = flags;
            true
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(unix)]
unsafe extern "C" fn host_posix_fd_unregister(host: *const clap_host, fd: i32) -> bool {
    let Some(state) = get_host_state(host) else {
        return false;
    };
    if let Ok(mut fds) = state.posix_fds.lock() {
        let len_before = fds.len();
        fds.retain(|e| e.fd != fd);
        fds.len() < len_before
    } else {
        false
    }
}

// ── Stream implementations (unchanged) ──

pub struct OutputStream {
    buffer: Vec<u8>,
    stream: clap_ostream,
}

impl OutputStream {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            stream: clap_ostream {
                ctx: ptr::null_mut(),
                write: Some(ostream_write),
            },
        }
    }

    pub fn as_raw(&mut self) -> *const clap_ostream {
        self.stream.ctx = &mut self.buffer as *mut Vec<u8> as *mut c_void;
        &self.stream
    }

    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    pub fn into_data(self) -> Vec<u8> {
        self.buffer
    }
}

impl Default for OutputStream {
    fn default() -> Self {
        Self::new()
    }
}

unsafe extern "C" fn ostream_write(
    stream: *const clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    let out_buffer = &mut *((*stream).ctx as *mut Vec<u8>);
    let data = std::slice::from_raw_parts(buffer as *const u8, size as usize);
    out_buffer.extend_from_slice(data);
    size as i64
}

pub struct InputStream<'a> {
    data: &'a [u8],
    position: usize,
    stream: clap_istream,
}

impl<'a> InputStream<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            stream: clap_istream {
                ctx: ptr::null_mut(),
                read: Some(istream_read),
            },
        }
    }

    /// The returned pointer is only valid for the lifetime of this `InputStream`.
    pub fn as_raw(&mut self) -> *const clap_istream {
        self.stream.ctx = self as *mut InputStream as *mut c_void;
        &self.stream
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }
}

unsafe extern "C" fn istream_read(
    stream: *const clap_istream,
    buffer: *mut c_void,
    size: u64,
) -> i64 {
    let input = &mut *((*stream).ctx as *mut InputStream);
    let remaining = input.data.len() - input.position;
    let to_read = (size as usize).min(remaining);

    if to_read == 0 {
        return 0;
    }

    let source = &input.data[input.position..input.position + to_read];
    let dest = std::slice::from_raw_parts_mut(buffer as *mut u8, to_read);
    dest.copy_from_slice(source);

    input.position += to_read;
    to_read as i64
}
