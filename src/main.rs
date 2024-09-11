use application::{AppToKeeperMessage, ToAppMessage};
use catch_panic::{payload_to_string, ErrorInfo};
use iced::Application;
use outputmodules::{ModuleConfig, OutputModule, OutputModules};
use std::collections::HashMap;
use std::panic::PanicHookInfo;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::{env, marker::PhantomData, net::UdpSocket, time::Duration};
use toy_arms::external::{read, Process};
use winapi::um::winnt::HANDLE;

mod offsets;
use offsets::{Pointer, RekordboxOffsets};

mod application;
mod catch_panic;
mod outputmodules;

struct Value<T> {
    address: usize,
    handle: HANDLE,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: HANDLE, base: usize, offsets: Pointer) -> Value<T> {
        let mut address = base;

        for offset in &offsets.offsets {
            address = read::<usize>(h, address + offset)
                .expect(&format!("\nMemory read failed, check your Rekordbox version! Try updating with -u.\nIf nothing works, wait for an update or send this entire error message to @grufkork. \n\nBase: {base:X}, Offsets: {offsets}"));
        }
        address += offsets.final_offset;

        Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        }
    }

    fn read(&self) -> T {
        read::<T>(self.handle, self.address).unwrap()
    }
}

pub struct Rekordbox {
    master_bpm: Value<f32>,
    masterdeck_index: Value<u8>,
    original_bpms: Vec<Value<f32>>,
    beatgrid_seconds: Vec<Value<f64>>,
    beatgrid_beats: Vec<Value<i32>>,
    sample_positions: Vec<Value<i64>>,
    sample_rates: Vec<Value<u32>>,
    track_infos: Vec<Value<[u8; 100]>>,
    deckcount: usize,
}

fn pointers_to_vals<T>(h: HANDLE, base: usize, pointers: Vec<Pointer>) -> Vec<Value<T>> {
    pointers
        .iter()
        .map(|x| Value::new(h, base, x.clone()))
        .collect()
}

impl Rekordbox {
    fn new(offsets: RekordboxOffsets) -> Self {
        let rb = Process::from_process_name("rekordbox.exe")
            .expect("Could not find Rekordbox process! ");
        let h = rb.process_handle;

        let base = rb.get_module_base("rekordbox.exe").unwrap();

        let master_bpm_val: Value<f32> = Value::new(h, base, offsets.master_bpm);

        let original_bpms = pointers_to_vals(h, base, offsets.original_bpm);
        let beatgrid_shifts = pointers_to_vals(h, base, offsets.beatgrid_shift);
        let beatgrid_beats = pointers_to_vals(h, base, offsets.beatgrid_beat);
        let sample_positions = pointers_to_vals(h, base, offsets.sample_position);
        let sample_rates = pointers_to_vals(h, base, offsets.sample_rate);
        let track_infos = pointers_to_vals(h, base, offsets.track_info);

        let deckcount = beatgrid_shifts.len();

        let masterdeck_index_val: Value<u8> = Value::new(h, base, offsets.masterdeck_index);

        Self {
            master_bpm: master_bpm_val,
            original_bpms,
            beatgrid_seconds: beatgrid_shifts,
            beatgrid_beats,
            sample_positions,
            sample_rates,
            masterdeck_index: masterdeck_index_val,
            deckcount,
            track_infos,
        }
    }

    fn update(&mut self) {
        // self.master_bpm = self.master_bpm.read();

        // self.masterdeck_index = self.masterdeck_index.read();

        // self.master_beats = self.beats[self.masterdeck_index as usize];
    }
}

#[derive(Debug)]
struct TrackInfo {
    title: String,
    artist: String,
    album: String,
}

pub struct BeatKeeper {
    rb: Rekordbox,
    last_beat: i32,
    beat_fraction: f32,
    masterdeck_index: usize,
    offset_micros: f32,
    master_bpm: f32,
    last_master_bpm: f32,
    running_modules: Vec<Box<dyn OutputModule>>,
    rx: Receiver<AppToKeeperMessage>,
    tx: Sender<ToAppMessage>,
}

impl BeatKeeper {
    pub fn start(
        offsets: RekordboxOffsets,
        modules: Vec<(outputmodules::OutputModules, bool)>,
        config: HashMap<String, ModuleConfig>,
        rx: Receiver<AppToKeeperMessage>,
        tx: Sender<ToAppMessage>,
        panic_tx: Sender<ErrorInfo>,
    ) {
        let update_rate = if let Some(map) = config.get("keeper") {
            map.get("update_rate")
                .unwrap_or(&"".to_string())
                .parse::<u64>()
                .unwrap_or(50)
        } else {
            50
        };
        let crash_tx = tx.clone();
        thread::spawn(move || {
            if let Err(e) = std::panic::catch_unwind(move || {
                let mut running_modules = vec![];

                for (module, active) in modules {
                    if !active {
                        continue;
                    }

                    let conf = config
                        .get(&module.to_config_name())
                        .unwrap_or(&HashMap::new())
                        .clone();

                    running_modules.push(match module {
                        OutputModules::AbletonLink => {
                            outputmodules::abletonlink::AbletonLink::create(conf)
                        }
                        OutputModules::Osc => outputmodules::osc::Osc::create(conf),
                    });
                }

                let mut keeper = BeatKeeper {
                    rx,
                    tx,
                    rb: Rekordbox::new(offsets),
                    last_beat: 0,
                    beat_fraction: 1.,
                    masterdeck_index: 0,
                    offset_micros: 0.,
                    master_bpm: 120.,
                    last_master_bpm: 120.,
                    running_modules,
                };

                let period = Duration::from_micros(1000000 / update_rate); // 50Hz
                loop {
                    keeper.update();
                    thread::sleep(period);
                }
            }) {
                crash_tx
                    .send(ToAppMessage::Crash(payload_to_string(&e)))
                    .unwrap();
            }
        });
    }

    pub fn update(&mut self) -> f32 {
        self.master_bpm = self.rb.master_bpm.read();
        self.masterdeck_index = self.rb.masterdeck_index.read() as usize;

        // let samplerate = self.rb.sample_rates[self.masterdeck_index].read();
        let sample_position = self.rb.sample_positions[self.masterdeck_index].read();
        let seconds_played = sample_position as f32 / 44100.; //samplerate as f32;

        let grid_shift = self.rb.beatgrid_seconds[self.masterdeck_index].read();
        let mut grid_beat = self.rb.beatgrid_beats[self.masterdeck_index].read();
        if grid_beat < 1 {
            grid_beat = 1;
        }

        let original_bpm = self.rb.original_bpms[self.masterdeck_index].read();
        let grid_size = 60. / original_bpm;

        let grid_origin = grid_shift as f32 + grid_beat as f32 * grid_size;

        let beat = (seconds_played - grid_origin) / grid_size;

        let bpm_changed = self.master_bpm != self.last_master_bpm;

        println!("beat: {}", beat);
        println!("s played: {}", seconds_played);
        println!("origin {}", grid_origin);
        println!("shift: {}", grid_shift);
        println!("grid beat: {}", grid_beat);

        for module in &mut self.running_modules {
            println!("mod");
            module.beat_update(beat);
            if bpm_changed {
                module.bpm_changed(self.master_bpm);
            }
        }
        self.last_master_bpm = self.master_bpm;
        panic!("foqueup");

        println!("{:?}", self.get_track_infos());

        beat
    }

    fn get_track_infos(&self) -> Vec<TrackInfo> {
        (0..self.rb.deckcount)
            .map(|i| {
                let raw = self.rb.track_infos[i]
                    .read()
                    .into_iter()
                    .take_while(|x| *x != 0x00)
                    .collect::<Vec<u8>>();
                let text = String::from_utf8(raw).unwrap_or_default();
                let mut lines = text
                    .lines()
                    .map(|x| x.split_once(": ").unwrap_or(("", "")).1)
                    .map(|x| x.to_string());
                TrackInfo {
                    title: lines.next().unwrap_or("".to_string()),
                    artist: lines.next().unwrap_or("".to_string()),
                    album: lines.next().unwrap_or("".to_string()),
                }
            })
            .collect()
    }
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    let error_tx = catch_panic::start_panic_listener();

    crate::application::App::run(iced::settings::Settings::with_flags(error_tx)).unwrap();
}

// !cargo r -- -v 6.8.5
