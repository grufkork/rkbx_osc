use iced::Error;
use rusty_link::{AblLink, SessionState};

use crate::outputmodules::OutputModule;

use super::ModuleConfig;

pub struct AbletonLink {
    link: AblLink,
    state: SessionState,
    last_num_links: u64
}

impl AbletonLink {
    pub fn create(_conf: ModuleConfig) -> Box<dyn OutputModule> {
        let link = AblLink::new(120.);
        link.enable(false);

        let mut state = SessionState::new();
        link.capture_app_session_state(&mut state);

        link.enable(true);

        Box::new(AbletonLink { link, state, last_num_links: 9999})
    }
}

impl OutputModule for AbletonLink {
    fn bpm_changed(&mut self, bpm: f32){
        self.state.set_tempo(bpm as f64, self.link.clock_micros());
        self.link.commit_app_session_state(&self.state);
    }

    fn beat_update(&mut self, beat: f32){
        // let target_beat = (beat as f64) % 4.;

        self.state
            .force_beat_at_time(beat.into(), self.link.clock_micros() as u64, 4.);
        self.link.commit_app_session_state(&self.state);
    }

    fn slow_update(&mut self) -> Result<Option<String>, String> {
        let num_links = self.link.num_peers();
        if num_links != self.last_num_links {
            self.last_num_links = num_links;
            Ok(Some(format!("{} peers", num_links)))
        } else {
            Ok(None)
        }
    }
}
