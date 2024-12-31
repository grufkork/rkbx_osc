use rusty_link::{AblLink, SessionState};
use std::collections::HashMap;
use std::fmt::Display;

use crate::config::Config;
use crate::beatkeeper::TrackInfo;
use crate::log::ScopedLogger;

pub mod abletonlink;
pub mod osc;

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
    fn bpm_changed(&mut self, _bpm: f32){}
    fn beat_update(&mut self, _beat: f32){}

    fn track_changed(&mut self, _track: TrackInfo, _deck: usize){}
    fn master_track_changed(&mut self, _track: TrackInfo){}

    fn slow_update(&mut self);

    fn get_name(&self) -> String;
    fn get_pretty_name(&self) -> String;
}


pub struct ModuleDefinition{
    pub config_name: String,
    pub pretty_name: String,
    pub create: fn(Config, ScopedLogger) -> Box<dyn OutputModule>
}

impl ModuleDefinition{
    pub fn new(confname: &str, prettyname: &str, create: fn(Config, ScopedLogger) -> Box<dyn OutputModule>) -> Self{
        ModuleDefinition{
            config_name: confname.to_string(), 
            pretty_name: prettyname.to_string(),
            create
        }
    }
}
