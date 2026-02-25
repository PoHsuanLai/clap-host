//! Safe, ergonomic API for hosting CLAP audio plugins.
//! Handles low-level FFI, event list implementations, and host callbacks.
//!
//! ## Example
//!
//! ```ignore
//! use clap_host::{ClapInstance, MidiEvent, ProcessContext, TransportInfo};
//!
//! // Load a CLAP plugin
//! let mut plugin = ClapInstance::load("/path/to/plugin.clap", 44100.0, 512)?;
//!
//! // Check capabilities
//! println!("Name: {}", plugin.info().name);
//!
//! // Process audio with MIDI
//! let transport = TransportInfo::default().with_tempo(120.0).with_playing(true);
//! plugin.process(&mut buffer, &ProcessContext {
//!     midi: &[MidiEvent::note_on(0, 0, 60, 100)],
//!     transport: Some(&transport),
//!     ..Default::default()
//! })?;
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

/// # Safety
/// `ptr` must be null or point to a valid, nul-terminated C string.
pub(crate) unsafe fn cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

pub use error::{ClapError, LoadStage, Result};
pub use events::{ClapEvent, EventList, InputEventList, OutputEventList};
pub use host::{ClapHost, HostState, InputStream, OutputStream};
pub use instance::{ClapInstance, ClapSample, ParamMapping, ProcessContext};
#[cfg(unix)]
pub use types::PosixFdFlags;
pub use types::{
    AmbisonicConfig, AmbisonicNormalization, AmbisonicOrdering, AudioBuffer, AudioBuffer32,
    AudioBuffer64, AudioPortConfig, AudioPortConfigRequest, AudioPortFlags, AudioPortInfo,
    AudioPortType, ClapMidiEvent, Color, ContextMenuItem, ContextMenuTarget, EditorSize, MidiData,
    MidiEvent, NoteDialect, NoteDialects, NoteExpressionType, NoteExpressionValue, NoteName,
    NotePortInfo, ParamAutomationState, ParameterChanges, ParameterFlags, ParameterInfo,
    ParameterPoint, ParameterQueue, PluginInfo, RemoteControlsPage, StateContext, SurroundChannel,
    TrackInfo, TransportInfo, TransportRequest, TriggerInfo, TuningInfo, UndoChange,
    UndoDeltaProperties, VoiceInfo, WindowHandle,
};
