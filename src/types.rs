//! Common types for CLAP plugin hosting.

use bitflags::bitflags;
use smallvec::SmallVec;
use std::fmt;

pub struct AudioBuffer<'a, T = f32> {
    pub inputs: &'a [&'a [T]],
    pub outputs: &'a mut [&'a mut [T]],
    pub num_samples: usize,
    pub sample_rate: f64,
}

pub type AudioBuffer32<'a> = AudioBuffer<'a, f32>;
pub type AudioBuffer64<'a> = AudioBuffer<'a, f64>;

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub url: String,
    pub description: String,
    pub features: Vec<String>,
    pub audio_inputs: usize,
    pub audio_outputs: usize,
}

impl PluginInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            vendor: String::new(),
            version: String::new(),
            url: String::new(),
            description: String::new(),
            features: Vec::new(),
            audio_inputs: 2,
            audio_outputs: 2,
        }
    }

    pub fn vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = vendor.into();
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn audio_io(mut self, inputs: usize, outputs: usize) -> Self {
        self.audio_inputs = inputs;
        self.audio_outputs = outputs;
        self
    }
}

impl fmt::Display for PluginInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} v{} by {}", self.name, self.version, self.vendor)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransportInfo {
    pub playing: bool,
    pub recording: bool,
    pub cycle_active: bool,
    pub tempo: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub song_pos_beats: f64,
    pub song_pos_seconds: f64,
    pub loop_start_beats: f64,
    pub loop_end_beats: f64,
    pub bar_start: f64,
    pub bar_number: i32,
}

impl TransportInfo {
    pub fn new() -> Self {
        Self {
            tempo: 120.0,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
            ..Default::default()
        }
    }

    pub fn with_tempo(mut self, tempo: f64) -> Self {
        self.tempo = tempo;
        self
    }

    pub fn with_playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    pub fn with_recording(mut self, recording: bool) -> Self {
        self.recording = recording;
        self
    }

    pub fn with_loop(mut self, active: bool, start: f64, end: f64) -> Self {
        self.cycle_active = active;
        self.loop_start_beats = start;
        self.loop_end_beats = end;
        self
    }

    pub fn with_time_signature(mut self, numerator: i32, denominator: i32) -> Self {
        self.time_sig_numerator = numerator;
        self.time_sig_denominator = denominator;
        self
    }

    pub fn with_position(mut self, beats: f64, seconds: f64) -> Self {
        self.song_pos_beats = beats;
        self.song_pos_seconds = seconds;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    pub sample_offset: i32,
    pub channel: u8,
    pub data: MidiData,
}

#[derive(Debug, Clone, Copy)]
pub enum MidiData {
    NoteOn { key: u8, velocity: f64 },
    NoteOff { key: u8, velocity: f64 },
    PolyPressure { key: u8, pressure: f64 },
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
    ChannelPressure { pressure: u8 },
    PitchBend { value: u16 },
}

impl MidiEvent {
    pub fn note_on(sample_offset: i32, channel: u8, key: u8, velocity: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::NoteOn {
                key,
                velocity: velocity as f64 / 127.0,
            },
        }
    }

    pub fn note_off(sample_offset: i32, channel: u8, key: u8, velocity: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::NoteOff {
                key,
                velocity: velocity as f64 / 127.0,
            },
        }
    }

    pub fn control_change(sample_offset: i32, channel: u8, controller: u8, value: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::ControlChange { controller, value },
        }
    }

    pub fn program_change(sample_offset: i32, channel: u8, program: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::ProgramChange { program },
        }
    }

    pub fn pitch_bend(sample_offset: i32, channel: u8, value: u16) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::PitchBend { value },
        }
    }
}

pub trait ClapMidiEvent {
    fn sample_offset(&self) -> i32;
    fn channel(&self) -> u8;
    fn to_midi_data(&self) -> Option<MidiData>;
}

impl ClapMidiEvent for MidiEvent {
    fn sample_offset(&self) -> i32 {
        self.sample_offset
    }

    fn channel(&self) -> u8 {
        self.channel
    }

    fn to_midi_data(&self) -> Option<MidiData> {
        Some(self.data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteExpressionType {
    Volume,
    Pan,
    Tuning,
    Vibrato,
    Brightness,
    Pressure,
    Expression,
}

#[derive(Debug, Clone, Copy)]
pub struct NoteExpressionValue {
    pub sample_offset: i32,
    pub note_id: i32,
    pub port_index: i16,
    pub channel: i16,
    pub key: i16,
    pub expression_type: NoteExpressionType,
    pub value: f64,
}

impl NoteExpressionValue {
    pub fn new(expression_type: NoteExpressionType, note_id: i32, value: f64) -> Self {
        Self {
            sample_offset: 0,
            note_id,
            port_index: 0,
            channel: -1,
            key: -1,
            expression_type,
            value,
        }
    }

    pub fn at(mut self, sample_offset: i32) -> Self {
        self.sample_offset = sample_offset;
        self
    }

    pub fn port(mut self, port_index: i16) -> Self {
        self.port_index = port_index;
        self
    }

    pub fn on_channel(mut self, channel: i16) -> Self {
        self.channel = channel;
        self
    }

    pub fn on_key(mut self, key: i16) -> Self {
        self.key = key;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParameterPoint {
    pub sample_offset: i32,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ParameterQueue {
    pub param_id: u32,
    pub points: SmallVec<[ParameterPoint; 8]>,
}

impl ParameterQueue {
    pub fn new(param_id: u32) -> Self {
        Self {
            param_id,
            points: SmallVec::new(),
        }
    }

    pub fn add_point(&mut self, sample_offset: i32, value: f64) -> &mut Self {
        self.points.push(ParameterPoint {
            sample_offset,
            value,
        });
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParameterChanges {
    pub queues: SmallVec<[ParameterQueue; 16]>,
}

impl ParameterChanges {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_queue(&mut self, queue: ParameterQueue) -> &mut Self {
        self.queues.push(queue);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct ParameterFlags: u32 {
        const STEPPED                 = 1 << 0;
        const PERIODIC                = 1 << 1;
        const HIDDEN                  = 1 << 2;
        const READONLY                = 1 << 3;
        const BYPASS                  = 1 << 4;
        const AUTOMATABLE             = 1 << 5;
        const AUTOMATABLE_PER_NOTE_ID = 1 << 6;
        const AUTOMATABLE_PER_KEY     = 1 << 7;
        const AUTOMATABLE_PER_CHANNEL = 1 << 8;
        const AUTOMATABLE_PER_PORT    = 1 << 9;
        const MODULATABLE             = 1 << 10;
        const MODULATABLE_PER_NOTE_ID = 1 << 11;
        const MODULATABLE_PER_KEY     = 1 << 12;
        const MODULATABLE_PER_CHANNEL = 1 << 13;
        const MODULATABLE_PER_PORT    = 1 << 14;
        const REQUIRES_PROCESS        = 1 << 15;
    }
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub flags: ParameterFlags,
}

impl ParameterInfo {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            module: String::new(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
            flags: ParameterFlags::default(),
        }
    }

    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = module.into();
        self
    }

    pub fn range(mut self, min: f64, max: f64, default: f64) -> Self {
        self.min_value = min;
        self.max_value = max;
        self.default_value = default;
        self
    }

    pub fn flags(mut self, flags: ParameterFlags) -> Self {
        self.flags = flags;
        self
    }
}

#[derive(Debug, Clone)]
pub struct AudioPortInfo {
    pub id: u32,
    pub name: String,
    pub channel_count: u32,
    pub flags: AudioPortFlags,
    pub port_type: AudioPortType,
    pub in_place_pair_id: u32,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct AudioPortFlags: u32 {
        const MAIN                      = 1 << 0;
        const SUPPORTS_64BIT            = 1 << 1;
        const PREFERS_64BIT             = 1 << 2;
        const REQUIRES_COMMON_SAMPLE_SIZE = 1 << 3;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioPortType {
    Mono,
    Stereo,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct NotePortInfo {
    pub id: u32,
    pub name: String,
    pub supported_dialects: NoteDialects,
    pub preferred_dialect: NoteDialect,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct NoteDialects: u32 {
        const CLAP     = 1 << 0;
        const MIDI     = 1 << 1;
        const MIDI_MPE = 1 << 2;
        const MIDI2    = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteDialect {
    Clap,
    Midi,
    MidiMpe,
    Midi2,
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceInfo {
    pub voice_count: u32,
    pub voice_capacity: u32,
    pub supports_overlapping_notes: bool,
}

#[derive(Debug, Clone)]
pub struct AudioPortConfig {
    pub id: u32,
    pub name: String,
    pub input_port_count: u32,
    pub output_port_count: u32,
    pub has_main_input: bool,
    pub main_input_channel_count: u32,
    pub has_main_output: bool,
    pub main_output_channel_count: u32,
}

#[derive(Debug, Clone)]
pub struct NoteName {
    pub name: String,
    pub port: i16,
    pub channel: i16,
    pub key: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateContext {
    ForPreset,
    ForProject,
    ForDuplicate,
}

impl From<StateContext> for clap_sys::ext::state_context::clap_plugin_state_context_type {
    fn from(ctx: StateContext) -> Self {
        match ctx {
            StateContext::ForPreset => clap_sys::ext::state_context::CLAP_STATE_CONTEXT_FOR_PRESET,
            StateContext::ForProject => {
                clap_sys::ext::state_context::CLAP_STATE_CONTEXT_FOR_PROJECT
            }
            StateContext::ForDuplicate => {
                clap_sys::ext::state_context::CLAP_STATE_CONTEXT_FOR_DUPLICATE
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub alpha: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            alpha: 255,
            red,
            green,
            blue,
        }
    }

    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            alpha,
            red,
            green,
            blue,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub name: Option<String>,
    pub color: Option<Color>,
    pub audio_channel_count: Option<i32>,
    pub audio_port_type: Option<String>,
    pub is_return_track: bool,
    pub is_bus: bool,
    pub is_master: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamAutomationState {
    None,
    Present,
    Playing,
    Recording,
    Overriding,
}

#[derive(Debug, Clone)]
pub struct RemoteControlsPage {
    pub section_name: String,
    pub page_id: u32,
    pub page_name: String,
    pub param_ids: [u32; 8],
    pub is_for_preset: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransportRequest {
    Start,
    Stop,
    Continue,
    Pause,
    TogglePlay,
    Jump {
        position_beats: f64,
    },
    LoopRegion {
        start_beats: f64,
        duration_beats: f64,
    },
    ToggleLoop,
    EnableLoop(bool),
    Record(bool),
    ToggleRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuTarget {
    Global,
    Param(u32),
}

#[derive(Debug, Clone)]
pub enum ContextMenuItem {
    Entry {
        label: String,
        is_enabled: bool,
        action_id: u32,
    },
    CheckEntry {
        label: String,
        is_enabled: bool,
        is_checked: bool,
        action_id: u32,
    },
    Separator,
    Title {
        title: String,
        is_enabled: bool,
    },
    BeginSubmenu {
        label: String,
        is_enabled: bool,
    },
    EndSubmenu,
}

#[derive(Debug, Clone)]
pub struct AudioPortConfigRequest {
    pub is_input: bool,
    pub port_index: u32,
    pub channel_count: u32,
    pub port_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbisonicOrdering {
    Fuma,
    Acn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbisonicNormalization {
    MaxN,
    Sn3d,
    N3d,
    Sn2d,
    N2d,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmbisonicConfig {
    pub ordering: AmbisonicOrdering,
    pub normalization: AmbisonicNormalization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SurroundChannel {
    FrontLeft = 0,
    FrontRight = 1,
    FrontCenter = 2,
    LowFrequency = 3,
    BackLeft = 4,
    BackRight = 5,
    FrontLeftCenter = 6,
    FrontRightCenter = 7,
    BackCenter = 8,
    SideLeft = 9,
    SideRight = 10,
    TopCenter = 11,
    TopFrontLeft = 12,
    TopFrontCenter = 13,
    TopFrontRight = 14,
    TopBackLeft = 15,
    TopBackCenter = 16,
    TopBackRight = 17,
}

impl SurroundChannel {
    pub fn from_position(pos: u8) -> Option<Self> {
        match pos {
            0 => Some(Self::FrontLeft),
            1 => Some(Self::FrontRight),
            2 => Some(Self::FrontCenter),
            3 => Some(Self::LowFrequency),
            4 => Some(Self::BackLeft),
            5 => Some(Self::BackRight),
            6 => Some(Self::FrontLeftCenter),
            7 => Some(Self::FrontRightCenter),
            8 => Some(Self::BackCenter),
            9 => Some(Self::SideLeft),
            10 => Some(Self::SideRight),
            11 => Some(Self::TopCenter),
            12 => Some(Self::TopFrontLeft),
            13 => Some(Self::TopFrontCenter),
            14 => Some(Self::TopFrontRight),
            15 => Some(Self::TopBackLeft),
            16 => Some(Self::TopBackCenter),
            17 => Some(Self::TopBackRight),
            _ => None,
        }
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PosixFdFlags {
    pub read: bool,
    pub write: bool,
    pub error: bool,
}

#[derive(Debug, Clone)]
pub struct TriggerInfo {
    pub id: u32,
    pub flags: u32,
    pub name: String,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct TuningInfo {
    pub tuning_id: u32,
    pub name: String,
    pub is_dynamic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct UndoDeltaProperties {
    pub has_delta: bool,
    pub are_deltas_persistent: bool,
    pub format_version: u32,
}

#[derive(Debug, Clone)]
pub struct UndoChange {
    pub name: String,
    pub delta: Vec<u8>,
    pub delta_can_undo: bool,
}
