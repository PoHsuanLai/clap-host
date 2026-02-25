//! Parameter methods for ClapInstance.

use super::ClapInstance;
use crate::events::{ClapEvent, InputEventList, OutputEventList};
use crate::types::{Color, ParamAutomationState, ParameterFlags, ParameterInfo};
use clap_sys::ext::param_indication::{
    CLAP_PARAM_INDICATION_AUTOMATION_NONE, CLAP_PARAM_INDICATION_AUTOMATION_OVERRIDING,
    CLAP_PARAM_INDICATION_AUTOMATION_PLAYING, CLAP_PARAM_INDICATION_AUTOMATION_PRESENT,
    CLAP_PARAM_INDICATION_AUTOMATION_RECORDING,
};
use std::ptr;

#[derive(Debug, Clone)]
pub struct ParamMapping {
    pub param_id: u32,
    pub has_mapping: bool,
    pub color: Option<Color>,
    pub label: Option<String>,
    pub description: Option<String>,
}

impl ParamMapping {
    pub fn new(param_id: u32, has_mapping: bool) -> Self {
        Self {
            param_id,
            has_mapping,
            color: None,
            label: None,
            description: None,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

impl ClapInstance {
    pub fn parameter_count(&self) -> usize {
        if self.extensions.params.params.is_null() {
            return 0;
        }
        let params = unsafe { &*self.extensions.params.params };
        match params.count {
            Some(f) => (unsafe { f(self.plugin) }) as usize,
            None => 0,
        }
    }

    pub fn parameter(&self, id: u32) -> Option<f64> {
        if self.extensions.params.params.is_null() {
            return None;
        }
        let params = unsafe { &*self.extensions.params.params };
        let get_value_fn = params.get_value?;
        let mut value: f64 = 0.0;
        if unsafe { get_value_fn(self.plugin, id, &mut value) } {
            Some(value)
        } else {
            None
        }
    }

    pub fn parameter_info(&self, index: u32) -> Option<ParameterInfo> {
        if self.extensions.params.params.is_null() {
            return None;
        }
        let params = unsafe { &*self.extensions.params.params };
        let get_info_fn = params.get_info?;

        let mut info: clap_sys::ext::params::clap_param_info = unsafe { std::mem::zeroed() };

        if !unsafe { get_info_fn(self.plugin, index, &mut info) } {
            return None;
        }

        let name = unsafe { crate::cstr_to_string(info.name.as_ptr()) };
        let module = unsafe { crate::cstr_to_string(info.module.as_ptr()) };

        Some(ParameterInfo {
            id: info.id,
            name,
            module,
            min_value: info.min_value,
            max_value: info.max_value,
            default_value: info.default_value,
            flags: ParameterFlags::from_bits_truncate(info.flags),
        })
    }

    pub fn parameters(&self) -> Vec<ParameterInfo> {
        let count = self.parameter_count() as u32;
        (0..count).filter_map(|i| self.parameter_info(i)).collect()
    }

    /// Flush parameter changes outside of process(). Sends input events to
    /// the plugin and collects any output events it produces.
    pub fn flush_params(&mut self, input_events: Vec<ClapEvent>) -> Vec<ClapEvent> {
        if self.extensions.params.params.is_null() {
            return Vec::new();
        }
        let params = unsafe { &*self.extensions.params.params };
        let flush_fn = match params.flush {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut input_list = InputEventList::from_events(input_events);
        input_list.sort_by_time();

        let mut output_list = OutputEventList::new();

        unsafe {
            flush_fn(
                self.plugin,
                input_list.as_raw() as *const _,
                output_list.as_raw_mut() as *const _,
            );
        }

        output_list.take_events()
    }

    /// Set a single parameter value immediately via flush.
    pub fn set_parameter(&mut self, id: u32, value: f64) -> &mut Self {
        let event = ClapEvent::param_value(0, id, value);
        self.flush_params(vec![event]);
        self
    }

    pub fn set_param_mapping(&self, mapping: &ParamMapping) {
        if self.extensions.params.indication.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.params.indication };
        let set_mapping = match ext.set_mapping {
            Some(f) => f,
            None => return,
        };
        let clap_color = mapping.color.map(|c| clap_sys::color::clap_color {
            alpha: c.alpha,
            red: c.red,
            green: c.green,
            blue: c.blue,
        });
        let color_ptr = clap_color
            .as_ref()
            .map(|c| c as *const _)
            .unwrap_or(ptr::null());
        let label_cstr = mapping
            .label
            .as_deref()
            .and_then(|s| std::ffi::CString::new(s).ok());
        let label_ptr = label_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        let desc_cstr = mapping
            .description
            .as_deref()
            .and_then(|s| std::ffi::CString::new(s).ok());
        let desc_ptr = desc_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());
        unsafe {
            set_mapping(
                self.plugin,
                mapping.param_id,
                mapping.has_mapping,
                color_ptr,
                label_ptr,
                desc_ptr,
            );
        }
    }

    pub fn set_param_automation(
        &self,
        param_id: u32,
        state: ParamAutomationState,
        color: Option<Color>,
    ) {
        if self.extensions.params.indication.is_null() {
            return;
        }
        let ext = unsafe { &*self.extensions.params.indication };
        let set_automation = match ext.set_automation {
            Some(f) => f,
            None => return,
        };
        let automation_state = match state {
            ParamAutomationState::None => CLAP_PARAM_INDICATION_AUTOMATION_NONE,
            ParamAutomationState::Present => CLAP_PARAM_INDICATION_AUTOMATION_PRESENT,
            ParamAutomationState::Playing => CLAP_PARAM_INDICATION_AUTOMATION_PLAYING,
            ParamAutomationState::Recording => CLAP_PARAM_INDICATION_AUTOMATION_RECORDING,
            ParamAutomationState::Overriding => CLAP_PARAM_INDICATION_AUTOMATION_OVERRIDING,
        };
        let clap_color = color.map(|c| clap_sys::color::clap_color {
            alpha: c.alpha,
            red: c.red,
            green: c.green,
            blue: c.blue,
        });
        let color_ptr = clap_color
            .as_ref()
            .map(|c| c as *const _)
            .unwrap_or(ptr::null());
        unsafe { set_automation(self.plugin, param_id, automation_state, color_ptr) };
    }
}
