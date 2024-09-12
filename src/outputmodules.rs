use rusty_link::{AblLink, SessionState};
use std::collections::HashMap;
use std::fmt::Display;

use crate::TrackInfo;

pub mod abletonlink;
pub mod osc;

pub type ModuleConfig = HashMap<String, String>;

#[derive(Clone, Copy)]
pub enum OutputModules {
    AbletonLink,
    Osc,
}

impl OutputModules {
    pub fn to_config_name(&self) -> String {
        match self {
            OutputModules::AbletonLink => "link".to_string(),
            OutputModules::Osc => "osc".to_string(),
        }
    }
}

impl Display for OutputModules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                OutputModules::AbletonLink => "Ableton Link",
                OutputModules::Osc => "OSC",
            }
        )
    }
}
pub trait OutputModule {
    fn bpm_changed(&mut self, _bpm: f32) {}
    fn beat_update(&mut self, _beat: f32) {}

    fn track_changed(&mut self, _track: TrackInfo, _deck: usize) {}
    fn master_track_changed(&mut self, _track: TrackInfo) {}
}
