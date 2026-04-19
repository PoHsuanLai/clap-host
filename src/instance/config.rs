//! Grouped configuration state for a ClapInstance.
//!
//! The bare fields `sample_rate`, `max_frames`, `supports_f64`,
//! `input_port_channels`, `output_port_channels`, `is_active`, `is_processing`,
//! and `gui_created` each belong to one of three cohesive groups.

/// Audio format the host presents to the plugin.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AudioConfig {
    pub sample_rate: f64,
    pub max_frames: u32,
    pub supports_f64: bool,
}

/// Per-port channel counts for audio IO.
/// E.g. `inputs = [2]` for stereo, `[2, 2]` for two stereo ports.
#[derive(Debug, Clone, Default)]
pub(crate) struct PortLayout {
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
}

impl PortLayout {
    pub fn input_channel_total(&self) -> usize {
        self.inputs.iter().map(|&c| c as usize).sum()
    }

    pub fn output_channel_total(&self) -> usize {
        self.outputs.iter().map(|&c| c as usize).sum()
    }
}

/// Tracks which lifecycle transitions have been performed. These three
/// flags form a state machine: `!active && !processing → active → processing`,
/// while `gui_created` is orthogonal.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LifecycleFlags {
    pub active: bool,
    pub processing: bool,
    pub gui_created: bool,
}
