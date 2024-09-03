use rusty_link::{AblLink, SessionState};

use crate::outputmodules::OutputModule;


pub struct AbletonLink{
    link: AblLink,
    state: SessionState
}

impl AbletonLink{
    pub fn new() -> Box<dyn OutputModule>{
        let link = AblLink::new(120.);
        link.enable(false);

        let mut state = SessionState::new();
        link.capture_app_session_state(&mut state);

        link.enable(true);

        Box::new(AbletonLink{
            link,
            state
        })
    }
}

impl OutputModule for AbletonLink{
    fn bpm_changed(&mut self, bpm: f32){
        self.state.set_tempo(bpm as f64, self.link.clock_micros());
        self.link.commit_app_session_state(&self.state);
    }

    fn beat_update(&mut self, beat: f32){

        let target_beat = (beat as f64 + 1.) % 4.;

        self.state.force_beat_at_time(target_beat, self.link.clock_micros() as u64, 4.);
        self.link.commit_app_session_state(&self.state);
    }
}
