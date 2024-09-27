use std::net::UdpSocket;

use rosc::{encoder::encode, OscMessage, OscPacket};
use winapi::shared::d3d9types::D3DSAMP_ADDRESSW;

use super::{ModuleConfig, OutputModule};

pub struct Osc {
    socket: UdpSocket,
    info_sent: bool
}

impl Osc {
    fn send_float(&mut self, addr: &str, value: f32) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::Float(value)],
        });
        let packet = encode(&msg).unwrap();
        self.socket.send(&packet).unwrap();
    }

    fn send_string(&mut self, addr: &str, value: &str) {
        let msg = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![rosc::OscType::String(value.to_string())],
        });
        let packet = encode(&msg).unwrap();
        self.socket.send(&packet).unwrap();
    }
}

impl Osc {
    pub fn create(conf: ModuleConfig) -> Box<dyn OutputModule> {
        let socket = UdpSocket::bind(
            conf.get("source_address")
                .unwrap_or(&"127.0.0.1:8888".to_string()),
        )
        .unwrap();
        socket
            .connect(
                conf.get("destination")
                    .unwrap_or(&String::from("127.0.0.1:9999")),
            )
            .unwrap();

        Box::new(Osc { socket, info_sent: false })
    }
}

impl OutputModule for Osc {
    fn bpm_changed(&mut self, bpm: f32){
        self.send_float("/bpm", bpm);
    }

    fn beat_update(&mut self, beat: f32){
        self.send_float("/beat/total", beat);
        self.send_float("/beat/fraction", beat % 1.);
    }

    fn track_changed(&mut self, track: crate::TrackInfo, deck: usize){
        self.send_string(&format!("/track/{deck}/title"), &track.title);
        self.send_string(&format!("/track/{deck}/artist"), &track.artist);
        self.send_string(&format!("/track/{deck}/album"), &track.album);
    }

    fn master_track_changed(&mut self, track: crate::TrackInfo){
        self.send_string("/track/master/title", &track.title);
        self.send_string("/track/master/artist", &track.artist);
        self.send_string("/track/master/album", &track.album);
    }

    fn slow_update(&mut self) -> Result<Option<String>, String> {
        if !self.info_sent {
            self.info_sent = true;
            
            let mut target_addr = if let Ok(addr) = self.socket.peer_addr() {
                addr.to_string()
            } else {
                "No target!!".to_string()
            };

            let mut source_addr = if let Ok(addr) = self.socket.local_addr() {
                addr.to_string()
            } else {
                "No source!!".to_string()
            };


            return Ok(Some(format!("{} -> {}", source_addr, target_addr)));
        }
        Ok(None)
    }
}
