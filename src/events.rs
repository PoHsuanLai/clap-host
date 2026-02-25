//! CLAP event list implementations.
//!
//! Events wrap the actual clap-sys C structs so that pointers returned by
//! `input_events_get` have the correct C memory layout for plugins to cast.

use crate::types::{
    MidiData, MidiEvent, NoteExpressionType, NoteExpressionValue, ParameterChanges, ParameterPoint,
    ParameterQueue,
};
use clap_sys::events::{
    clap_event_header, clap_event_midi, clap_event_midi_sysex, clap_event_note,
    clap_event_note_expression, clap_event_param_gesture, clap_event_param_mod,
    clap_event_param_value, clap_input_events, clap_output_events, CLAP_CORE_EVENT_SPACE_ID,
    CLAP_EVENT_MIDI, CLAP_EVENT_MIDI_SYSEX, CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_END,
    CLAP_EVENT_NOTE_EXPRESSION, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
    CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_MOD,
    CLAP_EVENT_PARAM_VALUE, CLAP_NOTE_EXPRESSION_BRIGHTNESS, CLAP_NOTE_EXPRESSION_EXPRESSION,
    CLAP_NOTE_EXPRESSION_PAN, CLAP_NOTE_EXPRESSION_PRESSURE, CLAP_NOTE_EXPRESSION_TUNING,
    CLAP_NOTE_EXPRESSION_VIBRATO, CLAP_NOTE_EXPRESSION_VOLUME,
};
use std::ptr;

/// CLAP event wrapping the actual C structs for correct memory layout.
///
/// Each variant stores the corresponding `#[repr(C)]` struct from clap-sys,
/// so that a pointer to its `header` field can be safely cast by the plugin
/// back to the full event struct type.
#[allow(dead_code)]
pub enum ClapEvent {
    NoteOn(clap_event_note),
    NoteOff(clap_event_note),
    NoteChoke(clap_event_note),
    NoteEnd(clap_event_note),
    Midi(clap_event_midi),
    NoteExpression(clap_event_note_expression),
    ParamValue(clap_event_param_value),
    ParamMod(clap_event_param_mod),
    ParamGestureBegin(clap_event_param_gesture),
    ParamGestureEnd(clap_event_param_gesture),
    /// Sysex owns the data buffer; the inner C struct's `buffer` pointer
    /// points into `_data`. Must not be moved after construction.
    MidiSysex {
        inner: clap_event_midi_sysex,
        _data: Vec<u8>,
    },
}

// Safety: Events don't contain non-Send types (cookie is just passed through)
unsafe impl Send for ClapEvent {}
unsafe impl Sync for ClapEvent {}

impl ClapEvent {
    /// Returns a pointer to the header of the underlying C struct.
    /// The plugin can safely cast this to the full event type.
    pub fn header(&self) -> &clap_event_header {
        match self {
            ClapEvent::NoteOn(e) => &e.header,
            ClapEvent::NoteOff(e) => &e.header,
            ClapEvent::NoteChoke(e) => &e.header,
            ClapEvent::NoteEnd(e) => &e.header,
            ClapEvent::Midi(e) => &e.header,
            ClapEvent::NoteExpression(e) => &e.header,
            ClapEvent::ParamValue(e) => &e.header,
            ClapEvent::ParamMod(e) => &e.header,
            ClapEvent::ParamGestureBegin(e) => &e.header,
            ClapEvent::ParamGestureEnd(e) => &e.header,
            ClapEvent::MidiSysex { inner, .. } => &inner.header,
        }
    }

    pub fn note_on(time: u32, channel: i16, key: i16, velocity: f64) -> Self {
        ClapEvent::NoteOn(clap_event_note {
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
        })
    }

    pub fn note_off(time: u32, channel: i16, key: i16, velocity: f64) -> Self {
        ClapEvent::NoteOff(clap_event_note {
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
        })
    }

    pub fn midi(time: u32, port_index: u16, data: [u8; 3]) -> Self {
        ClapEvent::Midi(clap_event_midi {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI,
                flags: 0,
            },
            port_index,
            data,
        })
    }

    pub fn param_value(time: u32, param_id: u32, value: f64) -> Self {
        ClapEvent::ParamValue(clap_event_param_value {
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
        })
    }

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

        ClapEvent::NoteExpression(clap_event_note_expression {
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
        })
    }

    pub fn from_midi_event(event: &MidiEvent) -> Option<Self> {
        let time = event.sample_offset as u32;
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
            MidiData::ProgramChange { program } => Some(ClapEvent::midi(
                time,
                0,
                [0xC0 | (channel as u8), program, 0],
            )),
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

    pub fn to_midi_event(&self) -> Option<MidiEvent> {
        match self {
            ClapEvent::NoteOn(e) => Some(MidiEvent {
                sample_offset: e.header.time as i32,
                channel: e.channel as u8,
                data: MidiData::NoteOn {
                    key: e.key as u8,
                    velocity: e.velocity,
                },
            }),
            ClapEvent::NoteOff(e) => Some(MidiEvent {
                sample_offset: e.header.time as i32,
                channel: e.channel as u8,
                data: MidiData::NoteOff {
                    key: e.key as u8,
                    velocity: e.velocity,
                },
            }),
            ClapEvent::Midi(e) => {
                let status = e.data[0];
                let channel = status & 0x0F;
                let data = match status & 0xF0 {
                    0x80 => MidiData::NoteOff {
                        key: e.data[1],
                        velocity: e.data[2] as f64 / 127.0,
                    },
                    0x90 => MidiData::NoteOn {
                        key: e.data[1],
                        velocity: e.data[2] as f64 / 127.0,
                    },
                    0xA0 => MidiData::PolyPressure {
                        key: e.data[1],
                        pressure: e.data[2] as f64 / 127.0,
                    },
                    0xB0 => MidiData::ControlChange {
                        controller: e.data[1],
                        value: e.data[2],
                    },
                    0xC0 => MidiData::ProgramChange { program: e.data[1] },
                    0xD0 => MidiData::ChannelPressure {
                        pressure: e.data[1],
                    },
                    0xE0 => MidiData::PitchBend {
                        value: (e.data[1] as u16) | ((e.data[2] as u16) << 7),
                    },
                    _ => return None,
                };
                Some(MidiEvent {
                    sample_offset: e.header.time as i32,
                    channel,
                    data,
                })
            }
            _ => None,
        }
    }
}

pub trait EventList {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear(&mut self);
}

#[repr(C)]
pub struct InputEventList {
    pub(crate) list: clap_input_events,
    pub(crate) events: Vec<ClapEvent>,
}

impl InputEventList {
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

    pub fn add_midi(&mut self, event: &MidiEvent) -> &mut Self {
        if let Some(clap_event) = ClapEvent::from_midi_event(event) {
            self.events.push(clap_event);
        }
        self
    }

    pub fn add_midi_events(&mut self, events: &[MidiEvent]) -> &mut Self {
        for event in events {
            if let Some(clap_event) = ClapEvent::from_midi_event(event) {
                self.events.push(clap_event);
            }
        }
        self
    }

    pub fn add_param_changes(&mut self, changes: &ParameterChanges) -> &mut Self {
        for queue in &changes.queues {
            for point in &queue.points {
                self.events.push(ClapEvent::param_value(
                    point.sample_offset as u32,
                    queue.param_id,
                    point.value,
                ));
            }
        }
        self
    }

    pub fn add_note_expressions(&mut self, expressions: &[NoteExpressionValue]) -> &mut Self {
        for expr in expressions {
            self.events.push(ClapEvent::note_expression(
                expr.sample_offset as u32,
                expr.expression_type,
                expr.note_id,
                expr.value,
            ));
        }
        self
    }

    pub fn sort_by_time(&mut self) -> &mut Self {
        self.events.sort_by_key(|e| e.header().time);
        self
    }

    pub fn as_raw(&self) -> *const clap_input_events {
        &self.list as *const _ as *const _
    }

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

#[repr(C)]
pub struct OutputEventList {
    pub(crate) list: clap_output_events,
    pub(crate) events: Vec<ClapEvent>,
}

impl OutputEventList {
    pub fn new() -> Self {
        Self {
            list: clap_output_events {
                ctx: ptr::null_mut(),
                try_push: Some(output_events_try_push),
            },
            events: Vec::new(),
        }
    }

    pub fn as_raw_mut(&mut self) -> *mut clap_output_events {
        &mut self.list as *mut _ as *mut _
    }

    pub fn events(&self) -> &[ClapEvent] {
        &self.events
    }

    pub fn take_events(&mut self) -> Vec<ClapEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn to_midi_events(&self) -> Vec<MidiEvent> {
        self.events
            .iter()
            .filter_map(|e| e.to_midi_event())
            .collect()
    }

    pub fn to_param_changes(&self) -> ParameterChanges {
        let mut changes = ParameterChanges::new();
        let mut queues: std::collections::HashMap<u32, ParameterQueue> =
            std::collections::HashMap::new();

        for event in &self.events {
            if let ClapEvent::ParamValue(e) = event {
                queues
                    .entry(e.param_id)
                    .or_insert_with(|| ParameterQueue::new(e.param_id))
                    .points
                    .push(ParameterPoint {
                        sample_offset: e.header.time as i32,
                        value: e.value,
                    });
            }
        }

        for (_, queue) in queues {
            changes.add_queue(queue);
        }
        changes
    }

    pub fn to_note_expressions(&self) -> Vec<NoteExpressionValue> {
        self.events
            .iter()
            .filter_map(|e| {
                if let ClapEvent::NoteExpression(ne) = e {
                    let expression_type = match ne.expression_id {
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
                        sample_offset: ne.header.time as i32,
                        note_id: ne.note_id,
                        port_index: ne.port_index,
                        channel: ne.channel,
                        key: ne.key,
                        expression_type,
                        value: ne.value,
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
            output_list.events.push(ClapEvent::NoteOn(*e));
            true
        }
        CLAP_EVENT_NOTE_OFF => {
            let e = &*(event as *const clap_event_note);
            output_list.events.push(ClapEvent::NoteOff(*e));
            true
        }
        CLAP_EVENT_MIDI => {
            let e = &*(event as *const clap_event_midi);
            output_list.events.push(ClapEvent::Midi(*e));
            true
        }
        CLAP_EVENT_NOTE_EXPRESSION => {
            let e = &*(event as *const clap_event_note_expression);
            output_list.events.push(ClapEvent::NoteExpression(*e));
            true
        }
        CLAP_EVENT_NOTE_CHOKE => {
            let e = &*(event as *const clap_event_note);
            output_list.events.push(ClapEvent::NoteChoke(*e));
            true
        }
        CLAP_EVENT_NOTE_END => {
            let e = &*(event as *const clap_event_note);
            output_list.events.push(ClapEvent::NoteEnd(*e));
            true
        }
        CLAP_EVENT_PARAM_VALUE => {
            let e = &*(event as *const clap_event_param_value);
            output_list.events.push(ClapEvent::ParamValue(*e));
            true
        }
        CLAP_EVENT_PARAM_MOD => {
            let e = &*(event as *const clap_event_param_mod);
            output_list.events.push(ClapEvent::ParamMod(*e));
            true
        }
        CLAP_EVENT_PARAM_GESTURE_BEGIN => {
            let e = &*(event as *const clap_event_param_gesture);
            output_list.events.push(ClapEvent::ParamGestureBegin(*e));
            true
        }
        CLAP_EVENT_PARAM_GESTURE_END => {
            let e = &*(event as *const clap_event_param_gesture);
            output_list.events.push(ClapEvent::ParamGestureEnd(*e));
            true
        }
        CLAP_EVENT_MIDI_SYSEX => {
            let e = &*(event as *const clap_event_midi_sysex);
            if !e.buffer.is_null() && e.size > 0 {
                let data = std::slice::from_raw_parts(e.buffer, e.size as usize).to_vec();
                // Build the inner struct with a pointer into the owned Vec.
                // The Vec is stored alongside and won't be moved independently.
                let inner = clap_event_midi_sysex {
                    header: *header,
                    port_index: e.port_index,
                    buffer: data.as_ptr(),
                    size: data.len() as u32,
                };
                output_list
                    .events
                    .push(ClapEvent::MidiSysex { inner, _data: data });
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_events_push_note_on() {
        let mut output = OutputEventList::new();
        let event = ClapEvent::note_on(0, 0, 60, 0.8);
        let header = event.header();

        // Call the raw FFI function via the output list
        let list_ptr = output.as_raw_mut();
        unsafe {
            let push_fn = (*list_ptr).try_push.unwrap();
            let result = push_fn(list_ptr, header as *const clap_event_header);
            assert!(result);
        }

        assert_eq!(output.events().len(), 1);
    }

    #[test]
    fn test_output_events_push_null_event() {
        let mut output = OutputEventList::new();
        let list_ptr = output.as_raw_mut();
        unsafe {
            let push_fn = (*list_ptr).try_push.unwrap();
            let result = push_fn(list_ptr, std::ptr::null());
            assert!(!result);
        }
        assert!(output.events().is_empty());
    }

    #[test]
    fn test_output_events_push_null_list() {
        // Build a valid event to pass
        let event = ClapEvent::note_on(0, 0, 60, 0.8);
        let header = event.header();
        unsafe {
            let result =
                output_events_try_push(std::ptr::null(), header as *const clap_event_header);
            assert!(!result);
        }
    }

    #[test]
    fn test_output_events_push_unknown_event_type() {
        let mut output = OutputEventList::new();
        let list_ptr = output.as_raw_mut();

        // Create a header with an unknown event type
        let header = clap_event_header {
            size: std::mem::size_of::<clap_event_header>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: 9999, // Unknown type
            flags: 0,
        };
        unsafe {
            let push_fn = (*list_ptr).try_push.unwrap();
            let result = push_fn(list_ptr, &header as *const clap_event_header);
            assert!(!result);
        }
        assert!(output.events().is_empty());
    }

    #[test]
    fn test_output_events_push_multiple() {
        let mut output = OutputEventList::new();
        let list_ptr = output.as_raw_mut();

        for i in 0..5 {
            let event = ClapEvent::note_on(i, 0, 60 + i as i16, 0.5);
            let header = event.header();
            unsafe {
                let push_fn = (*list_ptr).try_push.unwrap();
                let result = push_fn(list_ptr, header as *const clap_event_header);
                assert!(result);
            }
        }

        assert_eq!(output.events().len(), 5);
    }

    #[test]
    fn test_output_events_push_sysex_with_data() {
        let mut output = OutputEventList::new();
        let list_ptr = output.as_raw_mut();

        let sysex_data: Vec<u8> = vec![0xF0, 0x7E, 0x7F, 0x09, 0x01, 0xF7];
        let sysex = clap_event_midi_sysex {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi_sysex>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI_SYSEX,
                flags: 0,
            },
            port_index: 0,
            buffer: sysex_data.as_ptr(),
            size: sysex_data.len() as u32,
        };

        unsafe {
            let push_fn = (*list_ptr).try_push.unwrap();
            let result = push_fn(
                list_ptr,
                &sysex as *const clap_event_midi_sysex as *const clap_event_header,
            );
            assert!(result);
        }
        assert_eq!(output.events().len(), 1);
        // Verify the data was copied (not just pointer stored)
        match &output.events()[0] {
            ClapEvent::MidiSysex { _data, .. } => {
                assert_eq!(_data, &sysex_data);
            }
            _ => panic!("Expected MidiSysex event"),
        }
    }

    #[test]
    fn test_output_events_push_sysex_null_buffer() {
        let mut output = OutputEventList::new();
        let list_ptr = output.as_raw_mut();

        let sysex = clap_event_midi_sysex {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi_sysex>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI_SYSEX,
                flags: 0,
            },
            port_index: 0,
            buffer: std::ptr::null(),
            size: 0,
        };

        unsafe {
            let push_fn = (*list_ptr).try_push.unwrap();
            let result = push_fn(
                list_ptr,
                &sysex as *const clap_event_midi_sysex as *const clap_event_header,
            );
            // Should return true but not add event (null buffer skipped)
            assert!(result);
        }
        assert!(output.events().is_empty());
    }
}
