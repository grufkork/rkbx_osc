use rusty_link::{AblLink, SessionState};

pub mod abletonlink;

#[derive(Clone, Copy)]
pub enum OutputModules{
    AbletonLink,
    OSC
}

pub trait OutputModule{
    fn bpm_changed(&mut self, bpm: f32);
    fn beat_update(&mut self, beat: f32);
}
