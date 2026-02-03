//! Common types for CLAP plugin hosting.

/// Audio buffer for plugin processing.
pub struct AudioBuffer<'a, T = f32> {
    pub inputs: &'a [&'a [T]],
    pub outputs: &'a mut [&'a mut [T]],
    pub num_samples: usize,
}

pub type AudioBuffer32<'a> = AudioBuffer<'a, f32>;
pub type AudioBuffer64<'a> = AudioBuffer<'a, f64>;

/// Plugin metadata.
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
}

/// Transport state for plugin processing.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportInfo {
    pub playing: bool,
    pub recording: bool,
    pub loop_active: bool,
    pub tempo: f64,
    pub time_sig_numerator: u16,
    pub time_sig_denominator: u16,
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
        self.loop_active = active;
        self.loop_start_beats = start;
        self.loop_end_beats = end;
        self
    }

    pub fn with_time_signature(mut self, numerator: u16, denominator: u16) -> Self {
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

/// MIDI event for plugin input.
#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    /// Sample offset within the buffer
    pub sample_offset: u32,
    /// MIDI channel (0-15)
    pub channel: u8,
    /// Event data
    pub data: MidiData,
}

/// MIDI event data variants.
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
    pub fn note_on(sample_offset: u32, channel: u8, key: u8, velocity: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::NoteOn {
                key,
                velocity: velocity as f64 / 127.0,
            },
        }
    }

    pub fn note_off(sample_offset: u32, channel: u8, key: u8, velocity: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::NoteOff {
                key,
                velocity: velocity as f64 / 127.0,
            },
        }
    }

    pub fn control_change(sample_offset: u32, channel: u8, controller: u8, value: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::ControlChange { controller, value },
        }
    }

    pub fn program_change(sample_offset: u32, channel: u8, program: u8) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::ProgramChange { program },
        }
    }

    pub fn pitch_bend(sample_offset: u32, channel: u8, value: u16) -> Self {
        Self {
            sample_offset,
            channel,
            data: MidiData::PitchBend { value },
        }
    }
}

/// Trait for converting custom MIDI types to CLAP events.
pub trait ClapMidiEvent {
    fn sample_offset(&self) -> u32;
    fn channel(&self) -> u8;
    fn to_midi_data(&self) -> Option<MidiData>;
}

impl ClapMidiEvent for MidiEvent {
    fn sample_offset(&self) -> u32 {
        self.sample_offset
    }

    fn channel(&self) -> u8 {
        self.channel
    }

    fn to_midi_data(&self) -> Option<MidiData> {
        Some(self.data)
    }
}

/// Note expression type.
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

/// Note expression value.
#[derive(Debug, Clone, Copy)]
pub struct NoteExpressionValue {
    pub sample_offset: u32,
    pub note_id: i32,
    pub port_index: i16,
    pub channel: i16,
    pub key: i16,
    pub expression_type: NoteExpressionType,
    pub value: f64,
}

/// Parameter automation point.
#[derive(Debug, Clone, Copy)]
pub struct ParameterPoint {
    pub sample_offset: u32,
    pub value: f64,
}

/// Parameter automation queue.
#[derive(Debug, Clone)]
pub struct ParameterQueue {
    pub param_id: u32,
    pub points: Vec<ParameterPoint>,
}

impl ParameterQueue {
    pub fn new(param_id: u32) -> Self {
        Self {
            param_id,
            points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, sample_offset: u32, value: f64) {
        self.points.push(ParameterPoint {
            sample_offset,
            value,
        });
    }
}

/// Parameter changes for automation.
#[derive(Debug, Clone, Default)]
pub struct ParameterChanges {
    pub queues: Vec<ParameterQueue>,
}

impl ParameterChanges {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_queue(&mut self, queue: ParameterQueue) {
        self.queues.push(queue);
    }

    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }
}

/// Parameter flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParameterFlags {
    pub is_stepped: bool,
    pub is_periodic: bool,
    pub is_hidden: bool,
    pub is_readonly: bool,
    pub is_bypass: bool,
    pub is_automatable: bool,
    pub is_automatable_per_note_id: bool,
    pub is_automatable_per_key: bool,
    pub is_automatable_per_channel: bool,
    pub is_automatable_per_port: bool,
    pub is_modulatable: bool,
    pub is_modulatable_per_note_id: bool,
    pub is_modulatable_per_key: bool,
    pub is_modulatable_per_channel: bool,
    pub is_modulatable_per_port: bool,
    pub requires_process: bool,
}

/// Parameter information.
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
}
