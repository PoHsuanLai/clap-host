//! Audio processing methods for ClapInstance.

use super::ClapInstance;
use crate::error::{ClapError, Result};
use crate::events::{InputEventList, OutputEventList};
use crate::types::{AudioBuffer, MidiEvent, NoteExpressionValue, ParameterChanges, TransportInfo};
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::events::{
    clap_event_header, clap_event_transport, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_TRANSPORT,
    CLAP_TRANSPORT_HAS_BEATS_TIMELINE, CLAP_TRANSPORT_HAS_SECONDS_TIMELINE,
    CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
    CLAP_TRANSPORT_IS_PLAYING, CLAP_TRANSPORT_IS_RECORDING,
};
use clap_sys::fixedpoint::{CLAP_BEATTIME_FACTOR, CLAP_SECTIME_FACTOR};
use clap_sys::process::{clap_process, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR};
use std::ptr;

#[derive(Debug, Clone, Default)]
pub struct ProcessOutput {
    pub midi_events: Vec<MidiEvent>,
    pub param_changes: ParameterChanges,
    pub note_expressions: Vec<NoteExpressionValue>,
}

/// All inputs for a single process call. Use `..Default::default()` to fill
/// fields you don't need — compiles to zero-cost empty slices and None.
///
/// ```ignore
/// plugin.process(&mut buffer, &ProcessContext {
///     midi: &[MidiEvent::note_on(0, 0, 60, 100)],
///     transport: Some(&transport),
///     ..Default::default()
/// })?;
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessContext<'a> {
    pub midi: &'a [MidiEvent],
    pub params: Option<&'a ParameterChanges>,
    pub expressions: &'a [NoteExpressionValue],
    pub transport: Option<&'a TransportInfo>,
}

/// Trait abstracting over f32/f64 for CLAP audio buffer construction.
///
/// CLAP's `clap_audio_buffer` has separate `data32` and `data64` fields.
/// Each implementation populates the correct field and nulls the other.
pub trait ClapSample: Copy + Default + 'static {
    fn requires_f64() -> bool;

    fn build_port_buffers(
        port_channels: &[u32],
        ptrs: &mut Vec<*mut Self>,
        scratch: &mut Vec<Vec<Self>>,
        num_samples: usize,
    ) -> Vec<clap_audio_buffer>;
}

impl ClapSample for f32 {
    fn requires_f64() -> bool {
        false
    }

    fn build_port_buffers(
        port_channels: &[u32],
        ptrs: &mut Vec<*mut f32>,
        scratch: &mut Vec<Vec<f32>>,
        num_samples: usize,
    ) -> Vec<clap_audio_buffer> {
        pad_scratch(port_channels, ptrs, scratch, num_samples);
        let mut offset = 0usize;
        port_channels
            .iter()
            .map(|&ch_count| {
                let buf = clap_audio_buffer {
                    data32: ptrs[offset..].as_mut_ptr(),
                    data64: ptr::null_mut(),
                    channel_count: ch_count,
                    latency: 0,
                    constant_mask: 0,
                };
                offset += ch_count as usize;
                buf
            })
            .collect()
    }
}

impl ClapSample for f64 {
    fn requires_f64() -> bool {
        true
    }

    fn build_port_buffers(
        port_channels: &[u32],
        ptrs: &mut Vec<*mut f64>,
        scratch: &mut Vec<Vec<f64>>,
        num_samples: usize,
    ) -> Vec<clap_audio_buffer> {
        pad_scratch(port_channels, ptrs, scratch, num_samples);
        let mut offset = 0usize;
        port_channels
            .iter()
            .map(|&ch_count| {
                let buf = clap_audio_buffer {
                    data32: ptr::null_mut(),
                    data64: ptrs[offset..].as_mut_ptr(),
                    channel_count: ch_count,
                    latency: 0,
                    constant_mask: 0,
                };
                offset += ch_count as usize;
                buf
            })
            .collect()
    }
}

fn pad_scratch<T: Copy + Default>(
    port_channels: &[u32],
    ptrs: &mut Vec<*mut T>,
    scratch: &mut Vec<Vec<T>>,
    num_samples: usize,
) {
    let total_needed: usize = port_channels.iter().map(|&c| c as usize).sum();
    while ptrs.len() < total_needed {
        scratch.push(vec![T::default(); num_samples]);
        let buf = scratch.last_mut().expect("just pushed");
        ptrs.push(buf.as_mut_ptr());
    }
}

impl ClapInstance {
    /// Process audio through the plugin.
    ///
    /// Generic over [`ClapSample`] — pass an `AudioBuffer32` for f32 or
    /// `AudioBuffer64` for f64. The f64 path automatically checks that the
    /// plugin advertises 64-bit support.
    ///
    /// ```ignore
    /// plugin.process(&mut buffer, &ProcessContext {
    ///     midi: &[MidiEvent::note_on(0, 0, 60, 100)],
    ///     transport: Some(&transport),
    ///     ..Default::default()
    /// })?;
    /// ```
    pub fn process<T: ClapSample>(
        &mut self,
        buffer: &mut AudioBuffer<T>,
        ctx: &ProcessContext<'_>,
    ) -> Result<ProcessOutput> {
        if T::requires_f64() && !self.supports_f64 {
            return Err(ClapError::ProcessError(format!(
                "Plugin '{}' does not support 64-bit audio processing \
                 (CLAP_AUDIO_PORT_SUPPORTS_64BITS not set)",
                self.info.name
            )));
        }
        let empty_params = ParameterChanges::new();
        let params = ctx.params.unwrap_or(&empty_params);
        self.process_impl(buffer, ctx.midi, params, ctx.expressions, ctx.transport)
    }

    fn process_impl<T: ClapSample>(
        &mut self,
        buffer: &mut AudioBuffer<T>,
        midi_events: &[MidiEvent],
        param_changes: &ParameterChanges,
        note_expressions: &[NoteExpressionValue],
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        self.start_processing()?;

        let num_samples = buffer.num_samples as u32;

        let mut input_events = InputEventList::new();
        if !midi_events.is_empty() {
            input_events.add_midi_events(midi_events);
        }
        if !param_changes.is_empty() {
            input_events.add_param_changes(param_changes);
        }
        if !note_expressions.is_empty() {
            input_events.add_note_expressions(note_expressions);
        }
        input_events.sort_by_time();

        let mut output_events = OutputEventList::new();

        let mut input_ptrs: Vec<*mut T> =
            buffer.inputs.iter().map(|s| s.as_ptr() as *mut T).collect();
        let mut output_ptrs: Vec<*mut T> =
            buffer.outputs.iter_mut().map(|s| s.as_mut_ptr()).collect();

        let n = buffer.num_samples;
        let mut scratch_in = Vec::new();
        let mut scratch_out = Vec::new();
        let mut input_bufs = T::build_port_buffers(
            &self.input_port_channels,
            &mut input_ptrs,
            &mut scratch_in,
            n,
        );
        let mut output_bufs = T::build_port_buffers(
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
        audio_inputs: &mut [clap_audio_buffer],
        audio_outputs: &mut [clap_audio_buffer],
        num_samples: u32,
        input_events: &InputEventList,
        output_events: &mut OutputEventList,
        transport: Option<&TransportInfo>,
    ) -> Result<ProcessOutput> {
        // Record the audio thread ID so is_audio_thread checks work correctly.
        if let Ok(mut guard) = self.host_state.audio_thread_id.lock() {
            *guard = Some(std::thread::current().id());
        }

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
}

pub(super) fn build_clap_transport(transport: &TransportInfo) -> clap_event_transport {
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
