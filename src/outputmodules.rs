use std::fmt::Display;

use rusty_link::{AblLink, SessionState};

pub mod abletonlink;

#[derive(Clone, Copy)]
pub enum OutputModules{
    AbletonLink,
    OSC
}

impl Display for OutputModules{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self{
            OutputModules::AbletonLink => "Ableton Link",
            OutputModules::OSC => "OSC"
        })
    }
}
pub trait OutputModule{
    fn bpm_changed(&mut self, bpm: f32);
    fn beat_update(&mut self, beat: f32);
}
