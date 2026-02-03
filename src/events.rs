//! CLAP event list implementations.
//!
//! This module provides implementations of the CLAP event list interfaces
//! (`clap_input_events` and `clap_output_events`) which are required for
//! sending events to and receiving events from CLAP plugins.

use crate::types::{
    MidiData, MidiEvent, NoteExpressionType, NoteExpressionValue, ParameterChanges, ParameterPoint,
    ParameterQueue,
};
use clap_sys::events::{
    clap_event_header, clap_event_midi, clap_event_note, clap_event_note_expression,
    clap_event_param_value, clap_input_events, clap_output_events, CLAP_CORE_EVENT_SPACE_ID,
    CLAP_EVENT_MIDI, CLAP_EVENT_NOTE_EXPRESSION, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
    CLAP_EVENT_PARAM_VALUE, CLAP_NOTE_EXPRESSION_BRIGHTNESS, CLAP_NOTE_EXPRESSION_EXPRESSION,
    CLAP_NOTE_EXPRESSION_PAN, CLAP_NOTE_EXPRESSION_PRESSURE, CLAP_NOTE_EXPRESSION_TUNING,
    CLAP_NOTE_EXPRESSION_VIBRATO, CLAP_NOTE_EXPRESSION_VOLUME,
};
use std::ptr;

/// Internal representation of a CLAP event.
#[allow(dead_code)]
pub enum ClapEvent {
    NoteOn {
        header: clap_event_header,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        velocity: f64,
    },
    NoteOff {
        header: clap_event_header,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        velocity: f64,
    },
    Midi {
        header: clap_event_header,
        port_index: u16,
        data: [u8; 3],
    },
    NoteExpression {
        header: clap_event_header,
        expression_id: i32,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        value: f64,
    },
    ParamValue {
        header: clap_event_header,
        param_id: u32,
        cookie: *mut std::ffi::c_void,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        value: f64,
    },
}

// Safety: Events don't contain non-Send types (cookie is just passed through)
unsafe impl Send for ClapEvent {}
unsafe impl Sync for ClapEvent {}

impl ClapEvent {
    /// Get the event header.
    pub fn header(&self) -> &clap_event_header {
        match self {
            ClapEvent::NoteOn { header, .. }
            | ClapEvent::NoteOff { header, .. }
            | ClapEvent::Midi { header, .. }
            | ClapEvent::NoteExpression { header, .. }
            | ClapEvent::ParamValue { header, .. } => header,
        }
    }

    /// Create a note on event.
    pub fn note_on(time: u32, channel: i16, key: i16, velocity: f64) -> Self {
        ClapEvent::NoteOn {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_ON,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        }
    }

    /// Create a note off event.
    pub fn note_off(time: u32, channel: i16, key: i16, velocity: f64) -> Self {
        ClapEvent::NoteOff {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_OFF,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        }
    }

    /// Create a MIDI event.
    pub fn midi(time: u32, port_index: u16, data: [u8; 3]) -> Self {
        ClapEvent::Midi {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI,
                flags: 0,
            },
            port_index,
            data,
        }
    }

    /// Create a parameter value event.
    pub fn param_value(time: u32, param_id: u32, value: f64) -> Self {
        ClapEvent::ParamValue {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_param_value>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: 0,
            },
            param_id,
            cookie: ptr::null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        }
    }

    /// Create a note expression event.
    pub fn note_expression(
        time: u32,
        expression_type: NoteExpressionType,
        note_id: i32,
        value: f64,
    ) -> Self {
        let expression_id = match expression_type {
            NoteExpressionType::Volume => CLAP_NOTE_EXPRESSION_VOLUME,
            NoteExpressionType::Pan => CLAP_NOTE_EXPRESSION_PAN,
            NoteExpressionType::Tuning => CLAP_NOTE_EXPRESSION_TUNING,
            NoteExpressionType::Vibrato => CLAP_NOTE_EXPRESSION_VIBRATO,
            NoteExpressionType::Brightness => CLAP_NOTE_EXPRESSION_BRIGHTNESS,
            NoteExpressionType::Pressure => CLAP_NOTE_EXPRESSION_PRESSURE,
            NoteExpressionType::Expression => CLAP_NOTE_EXPRESSION_EXPRESSION,
        };

        ClapEvent::NoteExpression {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note_expression>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_EXPRESSION,
                flags: 0,
            },
            expression_id,
            note_id,
            port_index: 0,
            channel: -1,
            key: -1,
            value,
        }
    }

    /// Convert a MidiEvent to a ClapEvent.
    pub fn from_midi_event(event: &MidiEvent) -> Option<Self> {
        let time = event.sample_offset;
        let channel = event.channel as i16;

        match event.data {
            MidiData::NoteOn { key, velocity } => {
                Some(ClapEvent::note_on(time, channel, key as i16, velocity))
            }
            MidiData::NoteOff { key, velocity } => {
                Some(ClapEvent::note_off(time, channel, key as i16, velocity))
            }
            MidiData::ControlChange { controller, value } => Some(ClapEvent::midi(
                time,
                0,
                [0xB0 | (channel as u8), controller, value],
            )),
            MidiData::ProgramChange { program } => {
                Some(ClapEvent::midi(time, 0, [0xC0 | (channel as u8), program, 0]))
            }
            MidiData::ChannelPressure { pressure } => Some(ClapEvent::midi(
                time,
                0,
                [0xD0 | (channel as u8), pressure, 0],
            )),
            MidiData::PitchBend { value } => Some(ClapEvent::midi(
                time,
                0,
                [
                    0xE0 | (channel as u8),
                    (value & 0x7F) as u8,
                    ((value >> 7) & 0x7F) as u8,
                ],
            )),
            MidiData::PolyPressure { key, pressure } => {
                let pressure_byte = (pressure * 127.0) as u8;
                Some(ClapEvent::midi(
                    time,
                    0,
                    [0xA0 | (channel as u8), key, pressure_byte],
                ))
            }
        }
    }

    /// Convert a ClapEvent back to a MidiEvent (if possible).
    pub fn to_midi_event(&self) -> Option<MidiEvent> {
        match self {
            ClapEvent::NoteOn {
                header,
                channel,
                key,
                velocity,
                ..
            } => Some(MidiEvent {
                sample_offset: header.time,
                channel: *channel as u8,
                data: MidiData::NoteOn {
                    key: *key as u8,
                    velocity: *velocity,
                },
            }),
            ClapEvent::NoteOff {
                header,
                channel,
                key,
                velocity,
                ..
            } => Some(MidiEvent {
                sample_offset: header.time,
                channel: *channel as u8,
                data: MidiData::NoteOff {
                    key: *key as u8,
                    velocity: *velocity,
                },
            }),
            ClapEvent::Midi { header, data, .. } => {
                let status = data[0];
                let channel = status & 0x0F;
                let data = match status & 0xF0 {
                    0x80 => MidiData::NoteOff {
                        key: data[1],
                        velocity: data[2] as f64 / 127.0,
                    },
                    0x90 => MidiData::NoteOn {
                        key: data[1],
                        velocity: data[2] as f64 / 127.0,
                    },
                    0xA0 => MidiData::PolyPressure {
                        key: data[1],
                        pressure: data[2] as f64 / 127.0,
                    },
                    0xB0 => MidiData::ControlChange {
                        controller: data[1],
                        value: data[2],
                    },
                    0xC0 => MidiData::ProgramChange { program: data[1] },
                    0xD0 => MidiData::ChannelPressure { pressure: data[1] },
                    0xE0 => MidiData::PitchBend {
                        value: (data[1] as u16) | ((data[2] as u16) << 7),
                    },
                    _ => return None,
                };
                Some(MidiEvent {
                    sample_offset: header.time,
                    channel,
                    data,
                })
            }
            _ => None,
        }
    }
}

/// Trait for event list operations.
pub trait EventList {
    /// Get the number of events.
    fn len(&self) -> usize;

    /// Check if the list is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all events.
    fn clear(&mut self);
}

/// Input event list for sending events to plugins.
pub struct InputEventList {
    pub(crate) list: clap_input_events,
    pub(crate) events: Vec<ClapEvent>,
}

impl InputEventList {
    /// Create a new empty input event list.
    pub fn new() -> Self {
        Self {
            list: clap_input_events {
                ctx: ptr::null_mut(),
                size: Some(input_events_size),
                get: Some(input_events_get),
            },
            events: Vec::new(),
        }
    }

    /// Create an input event list from a vector of events.
    pub fn from_events(events: Vec<ClapEvent>) -> Self {
        Self {
            list: clap_input_events {
                ctx: ptr::null_mut(),
                size: Some(input_events_size),
                get: Some(input_events_get),
            },
            events,
        }
    }

    /// Add a MIDI event.
    pub fn add_midi(&mut self, event: &MidiEvent) {
        if let Some(clap_event) = ClapEvent::from_midi_event(event) {
            self.events.push(clap_event);
        }
    }

    /// Add multiple MIDI events.
    pub fn add_midi_events(&mut self, events: &[MidiEvent]) {
        for event in events {
            self.add_midi(event);
        }
    }

    /// Add parameter changes.
    pub fn add_param_changes(&mut self, changes: &ParameterChanges) {
        for queue in &changes.queues {
            for point in &queue.points {
                self.events
                    .push(ClapEvent::param_value(point.sample_offset, queue.param_id, point.value));
            }
        }
    }

    /// Add note expression events.
    pub fn add_note_expressions(&mut self, expressions: &[NoteExpressionValue]) {
        for expr in expressions {
            self.events.push(ClapEvent::note_expression(
                expr.sample_offset,
                expr.expression_type,
                expr.note_id,
                expr.value,
            ));
        }
    }

    /// Sort events by time.
    pub fn sort_by_time(&mut self) {
        self.events.sort_by_key(|e| e.header().time);
    }

    /// Get the raw clap_input_events pointer (for FFI).
    pub fn as_raw(&self) -> *const clap_input_events {
        &self.list as *const _ as *const _
    }

    /// Get the events slice.
    pub fn events(&self) -> &[ClapEvent] {
        &self.events
    }
}

impl Default for InputEventList {
    fn default() -> Self {
        Self::new()
    }
}

impl EventList for InputEventList {
    fn len(&self) -> usize {
        self.events.len()
    }

    fn clear(&mut self) {
        self.events.clear();
    }
}

unsafe extern "C" fn input_events_size(list: *const clap_input_events) -> u32 {
    let event_list = &*(list as *const InputEventList);
    event_list.events.len() as u32
}

unsafe extern "C" fn input_events_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    let event_list = &*(list as *const InputEventList);
    if index >= event_list.events.len() as u32 {
        return ptr::null();
    }
    event_list.events[index as usize].header() as *const _
}

/// Output event list for receiving events from plugins.
pub struct OutputEventList {
    pub(crate) list: clap_output_events,
    pub(crate) events: Vec<ClapEvent>,
}

impl OutputEventList {
    /// Create a new empty output event list.
    pub fn new() -> Self {
        Self {
            list: clap_output_events {
                ctx: ptr::null_mut(),
                try_push: Some(output_events_try_push),
            },
            events: Vec::new(),
        }
    }

    /// Get the raw clap_output_events pointer (for FFI).
    pub fn as_raw_mut(&mut self) -> *mut clap_output_events {
        &mut self.list as *mut _ as *mut _
    }

    /// Get the events slice.
    pub fn events(&self) -> &[ClapEvent] {
        &self.events
    }

    /// Extract MIDI events from the output.
    pub fn to_midi_events(&self) -> Vec<MidiEvent> {
        self.events.iter().filter_map(|e| e.to_midi_event()).collect()
    }

    /// Extract parameter changes from the output.
    pub fn to_param_changes(&self) -> ParameterChanges {
        let mut changes = ParameterChanges::new();
        let mut queues: std::collections::HashMap<u32, ParameterQueue> =
            std::collections::HashMap::new();

        for event in &self.events {
            if let ClapEvent::ParamValue {
                header,
                param_id,
                value,
                ..
            } = event
            {
                queues
                    .entry(*param_id)
                    .or_insert_with(|| ParameterQueue::new(*param_id))
                    .points
                    .push(ParameterPoint {
                        sample_offset: header.time,
                        value: *value,
                    });
            }
        }

        for (_, queue) in queues {
            changes.add_queue(queue);
        }
        changes
    }

    /// Extract note expression changes from the output.
    pub fn to_note_expressions(&self) -> Vec<NoteExpressionValue> {
        self.events
            .iter()
            .filter_map(|e| {
                if let ClapEvent::NoteExpression {
                    header,
                    expression_id,
                    note_id,
                    port_index,
                    channel,
                    key,
                    value,
                } = e
                {
                    let expression_type = match *expression_id {
                        id if id == CLAP_NOTE_EXPRESSION_VOLUME => NoteExpressionType::Volume,
                        id if id == CLAP_NOTE_EXPRESSION_PAN => NoteExpressionType::Pan,
                        id if id == CLAP_NOTE_EXPRESSION_TUNING => NoteExpressionType::Tuning,
                        id if id == CLAP_NOTE_EXPRESSION_VIBRATO => NoteExpressionType::Vibrato,
                        id if id == CLAP_NOTE_EXPRESSION_BRIGHTNESS => {
                            NoteExpressionType::Brightness
                        }
                        id if id == CLAP_NOTE_EXPRESSION_PRESSURE => NoteExpressionType::Pressure,
                        id if id == CLAP_NOTE_EXPRESSION_EXPRESSION => {
                            NoteExpressionType::Expression
                        }
                        _ => return None,
                    };
                    Some(NoteExpressionValue {
                        sample_offset: header.time,
                        note_id: *note_id,
                        port_index: *port_index,
                        channel: *channel,
                        key: *key,
                        expression_type,
                        value: *value,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for OutputEventList {
    fn default() -> Self {
        Self::new()
    }
}

impl EventList for OutputEventList {
    fn len(&self) -> usize {
        self.events.len()
    }

    fn clear(&mut self) {
        self.events.clear();
    }
}

unsafe extern "C" fn output_events_try_push(
    list: *const clap_output_events,
    event: *const clap_event_header,
) -> bool {
    if event.is_null() || list.is_null() {
        return false;
    }

    let output_list = &mut *(list as *mut OutputEventList);
    let header = &*event;

    match header.type_ {
        CLAP_EVENT_NOTE_ON => {
            let e = &*(event as *const clap_event_note);
            output_list.events.push(ClapEvent::NoteOn {
                header: *header,
                note_id: e.note_id,
                port_index: e.port_index,
                channel: e.channel,
                key: e.key,
                velocity: e.velocity,
            });
            true
        }
        CLAP_EVENT_NOTE_OFF => {
            let e = &*(event as *const clap_event_note);
            output_list.events.push(ClapEvent::NoteOff {
                header: *header,
                note_id: e.note_id,
                port_index: e.port_index,
                channel: e.channel,
                key: e.key,
                velocity: e.velocity,
            });
            true
        }
        CLAP_EVENT_MIDI => {
            let e = &*(event as *const clap_event_midi);
            output_list.events.push(ClapEvent::Midi {
                header: *header,
                port_index: e.port_index,
                data: e.data,
            });
            true
        }
        CLAP_EVENT_NOTE_EXPRESSION => {
            let e = &*(event as *const clap_event_note_expression);
            output_list.events.push(ClapEvent::NoteExpression {
                header: *header,
                expression_id: e.expression_id,
                note_id: e.note_id,
                port_index: e.port_index,
                channel: e.channel,
                key: e.key,
                value: e.value,
            });
            true
        }
        CLAP_EVENT_PARAM_VALUE => {
            let e = &*(event as *const clap_event_param_value);
            output_list.events.push(ClapEvent::ParamValue {
                header: *header,
                param_id: e.param_id,
                cookie: e.cookie,
                note_id: e.note_id,
                port_index: e.port_index,
                channel: e.channel,
                key: e.key,
                value: e.value,
            });
            true
        }
        _ => false,
    }
}
