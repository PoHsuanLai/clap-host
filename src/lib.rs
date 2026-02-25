//! # clap-host
//!
//! A Rust library for hosting CLAP audio plugins.
//!
//! This crate provides a safe and ergonomic API for loading, configuring, and
//! processing audio through CLAP plugins. It handles the low-level FFI details,
//! event list implementations, and host callbacks.
//!
//! ## Features
//!
//! - Load CLAP plugins from `.clap` bundles (macOS, Linux, Windows)
//! - Process audio in f32 or f64 format
//! - Send MIDI events to plugins (converted to CLAP note events)
//! - Parameter automation with sample-accurate timing
//! - Note expression (MPE-style per-note control)
//! - Transport/tempo synchronization
//! - Plugin state save/load
//! - Editor window support
//!
//! ## Example
//!
//! ```ignore
//! use clap_host::{ClapInstance, AudioBuffer, MidiEvent, TransportInfo};
//!
//! // Load a CLAP plugin
//! let mut plugin = ClapInstance::load("/path/to/plugin.clap", 44100.0, 512)?;
//!
//! // Check capabilities
//! println!("Name: {}", plugin.info().name);
//!
//! // Process audio with MIDI
//! let midi = vec![MidiEvent::note_on(0, 60, 100)];
//! let transport = TransportInfo::default().with_tempo(120.0).with_playing(true);
//! let params = ParameterChanges::new();
//! plugin.process_f32(&mut buffer, &midi, &params, &[], Some(&transport))?;
//! ```
//!
//! ## Custom MIDI Types
//!
//! If you have your own MIDI event type, implement the `ClapMidiEvent` trait:
//!
//! ```ignore
//! use clap_host::{ClapMidiEvent, ClapNoteEvent};
//!
//! impl ClapMidiEvent for MyMidiEvent {
//!     fn to_clap_events(&self) -> Vec<ClapNoteEvent> { /* ... */ }
//! }
//! ```

pub mod error;
pub mod events;
pub mod host;
pub mod instance;
pub mod types;

/// Convert a nullable C string pointer to an owned `String`.
/// Returns an empty string if the pointer is null.
///
/// # Safety
/// `ptr` must be null or point to a valid, nul-terminated C string.
pub(crate) unsafe fn cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(ptr)
            .to_string_lossy()
            .into_owned()
    }
}

pub use error::{ClapError, LoadStage, Result};
pub use events::{ClapEvent, EventList, InputEventList, OutputEventList};
pub use host::{ClapHost, HostState, InputStream, OutputStream};
pub use instance::{ClapInstance, ParamMapping};
#[cfg(unix)]
pub use types::PosixFdFlags;
pub use types::{
    AmbisonicConfig, AmbisonicNormalization, AmbisonicOrdering, AudioBuffer, AudioBuffer32,
    AudioBuffer64, AudioPortConfig, AudioPortConfigRequest, AudioPortFlags, AudioPortInfo,
    AudioPortType, ClapMidiEvent, Color, ContextMenuItem, ContextMenuTarget, MidiData, MidiEvent,
    NoteDialect, NoteDialects, NoteExpressionType, NoteExpressionValue, NoteName, NotePortInfo,
    ParamAutomationState, ParameterChanges, ParameterFlags, ParameterInfo, ParameterPoint,
    ParameterQueue, PluginInfo, RemoteControlsPage, StateContext, SurroundChannel, TrackInfo,
    TransportInfo, TransportRequest, TriggerInfo, TuningInfo, UndoChange, UndoDeltaProperties,
    VoiceInfo,
};
