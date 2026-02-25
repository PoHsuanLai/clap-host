//! Audio/note port methods for ClapInstance.

use super::ClapInstance;
use crate::types::{
    AmbisonicConfig, AmbisonicNormalization, AmbisonicOrdering, AudioPortConfig,
    AudioPortConfigRequest, AudioPortFlags, AudioPortInfo, AudioPortType, NoteDialect, NoteDialects,
    NotePortInfo, NoteName, SurroundChannel, VoiceInfo,
};
use clap_sys::ext::ambisonic::{
    clap_ambisonic_config, CLAP_AMBISONIC_NORMALIZATION_MAXN, CLAP_AMBISONIC_NORMALIZATION_N2D,
    CLAP_AMBISONIC_NORMALIZATION_N3D, CLAP_AMBISONIC_NORMALIZATION_SN2D,
    CLAP_AMBISONIC_NORMALIZATION_SN3D, CLAP_AMBISONIC_ORDERING_ACN,
    CLAP_AMBISONIC_ORDERING_FUMA,
};
use clap_sys::ext::audio_ports::{clap_audio_port_info, CLAP_PORT_MONO, CLAP_PORT_STEREO};
use clap_sys::ext::audio_ports_config::clap_audio_ports_config;
use clap_sys::ext::configurable_audio_ports::clap_audio_port_configuration_request;
use clap_sys::ext::note_name::clap_note_name;
use clap_sys::ext::note_ports::{
    clap_note_port_info, CLAP_NOTE_DIALECT_CLAP, CLAP_NOTE_DIALECT_MIDI,
    CLAP_NOTE_DIALECT_MIDI_MPE,
};
use clap_sys::ext::render::{CLAP_RENDER_OFFLINE, CLAP_RENDER_REALTIME};
use clap_sys::ext::voice_info::{clap_voice_info, CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES};
use std::ffi::CStr;
use std::ptr;

use crate::cstr_to_string;

impl ClapInstance {
    pub fn audio_port_count(&self, is_input: bool) -> usize {
        if self.extensions.audio.ports.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.audio.ports };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin, is_input) }) as usize,
            None => 0,
        }
    }

    pub fn audio_port_info(&self, index: usize, is_input: bool) -> Option<AudioPortInfo> {
        if self.extensions.audio.ports.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio.ports };
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
                AudioPortType::Custom(type_cstr.to_string_lossy().into_owned())
            }
        };

        Some(AudioPortInfo {
            id: info.id,
            name: unsafe { cstr_to_string(info.name.as_ptr()) },
            channel_count: info.channel_count,
            flags: AudioPortFlags::from_bits_truncate(info.flags),
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

    pub fn note_port_count(&self, is_input: bool) -> usize {
        if self.extensions.notes.ports.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.notes.ports };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin, is_input) }) as usize,
            None => 0,
        }
    }

    pub fn note_port_info(&self, index: usize, is_input: bool) -> Option<NotePortInfo> {
        if self.extensions.notes.ports.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.notes.ports };
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
            name: unsafe { cstr_to_string(info.name.as_ptr()) },
            supported_dialects: NoteDialects::from_bits_truncate(info.supported_dialects),
            preferred_dialect,
        })
    }

    pub fn audio_port_config_count(&self) -> usize {
        if self.extensions.audio.ports_config.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.audio.ports_config };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_audio_port_config(&self, index: usize) -> Option<AudioPortConfig> {
        if self.extensions.audio.ports_config.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio.ports_config };
        let get_fn = ext.get?;

        let mut config: clap_audio_ports_config = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut config) } {
            return None;
        }

        Some(AudioPortConfig {
            id: config.id,
            name: unsafe { cstr_to_string(config.name.as_ptr()) },
            input_port_count: config.input_port_count,
            output_port_count: config.output_port_count,
            has_main_input: config.has_main_input,
            main_input_channel_count: config.main_input_channel_count,
            has_main_output: config.has_main_output,
            main_output_channel_count: config.main_output_channel_count,
        })
    }

    pub fn select_audio_port_config(&mut self, config_id: u32) -> bool {
        if self.extensions.audio.ports_config.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.ports_config };
        match ext.select {
            Some(f) => unsafe { f(self.plugin, config_id) },
            None => false,
        }
    }

    pub fn get_latency(&self) -> u32 {
        if self.extensions.system.latency.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.system.latency };
        match ext.get {
            Some(f) => unsafe { f(self.plugin) },
            None => 0,
        }
    }

    pub fn get_tail(&self) -> u32 {
        if self.extensions.system.tail.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.system.tail };
        match ext.get {
            Some(f) => unsafe { f(self.plugin) },
            None => 0,
        }
    }

    pub fn set_render_mode(&mut self, offline: bool) -> bool {
        if self.extensions.system.render.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.system.render };
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
        if self.extensions.system.render.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.system.render };
        match ext.has_hard_realtime_requirement {
            Some(f) => unsafe { f(self.plugin) },
            None => false,
        }
    }

    pub fn get_voice_info(&self) -> Option<VoiceInfo> {
        if self.extensions.system.voice_info.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.system.voice_info };
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

    pub fn note_name_count(&self) -> usize {
        if self.extensions.notes.name.is_null() {
            return 0;
        }
        let ext = unsafe { &*self.extensions.notes.name };
        match ext.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn get_note_name(&self, index: usize) -> Option<NoteName> {
        if self.extensions.notes.name.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.notes.name };
        let get_fn = ext.get?;
        let mut info: clap_note_name = unsafe { std::mem::zeroed() };
        if !unsafe { get_fn(self.plugin, index as u32, &mut info) } {
            return None;
        }
        Some(NoteName {
            name: unsafe { cstr_to_string(info.name.as_ptr()) },
            port: info.port,
            channel: info.channel,
            key: info.key,
        })
    }

    pub fn can_apply_audio_port_configuration(&self, requests: &[AudioPortConfigRequest]) -> bool {
        if self.extensions.audio.configurable_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.configurable_ports };
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
        if self.extensions.audio.configurable_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.configurable_ports };
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

    pub fn can_activate_audio_port_while_processing(&self) -> bool {
        if self.extensions.audio.ports_activation.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.ports_activation };
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
        if self.extensions.audio.ports_activation.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.ports_activation };
        match ext.set_active {
            Some(f) => unsafe { f(self.plugin, is_input, port_index, is_active, sample_size) },
            None => false,
        }
    }

    pub fn add_audio_port(
        &mut self,
        is_input: bool,
        channel_count: u32,
        port_type: Option<&str>,
    ) -> bool {
        if self.extensions.audio.extensible_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.extensible_ports };
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
        if self.extensions.audio.extensible_ports.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.extensible_ports };
        match ext.remove_port {
            Some(f) => unsafe { f(self.plugin, is_input, index) },
            None => false,
        }
    }

    pub fn is_ambisonic_config_supported(&self, config: &AmbisonicConfig) -> bool {
        if self.extensions.audio.ambisonic.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.ambisonic };
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
        if self.extensions.audio.ambisonic.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio.ambisonic };
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

    pub fn is_surround_channel_mask_supported(&self, channel_mask: u64) -> bool {
        if self.extensions.audio.surround.is_null() {
            return false;
        }
        let ext = unsafe { &*self.extensions.audio.surround };
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
        if self.extensions.audio.surround.is_null() {
            return None;
        }
        let ext = unsafe { &*self.extensions.audio.surround };
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
