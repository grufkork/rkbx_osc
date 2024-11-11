use application::{AppToKeeperMessage, ToAppMessage};
use catch_panic::payload_to_string;
use iced::{Application, Size};
use outputmodules::{ModuleConfig, OutputModule, OutputModules};
use std::collections::HashMap;
use std::os::windows::raw::HANDLE;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::{marker::PhantomData, time::Duration};
use toy_arms::external::{read, Process};

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
    fn new(h: HANDLE, base: usize, offsets: &Pointer) -> Value<T> {
        let mut address = base;

        for offset in &offsets.offsets {
            address = read::<usize>(h, address + offset)
                .unwrap_or_else(|_| panic!("\nMemory read failed, check your Rekordbox version! Try updating with -u.\nIf nothing works, wait for an update or send this entire error message to @grufkork. \n\nBase: {base:X}, Offsets: {offsets}"));
        }
        address += offsets.final_offset;

        Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        }
    }
    fn pointers_to_vals(h: HANDLE, base: usize, pointers: Vec<Pointer>) -> Vec<Value<T>> {
        pointers
            .iter()
            .map(|x| Value::new(h, base, x))
            .collect()
    }

    fn read(&self) -> T {
        read::<T>(self.handle, self.address).unwrap()
    }
}

struct PointerChainValue<T> {
    handle: HANDLE,
    base: usize,
    pointer: Pointer,
    _marker: PhantomData<T>,
}

impl<T> PointerChainValue<T>{
    fn new(h: HANDLE, base: usize, pointer: Pointer) -> PointerChainValue<T>{
        Self{
            handle: h,
            base,
            pointer,
            _marker: PhantomData::<T>,
        }
    }

    fn pointers_to_vals(h: HANDLE, base: usize, pointers: Vec<Pointer>) -> Vec<PointerChainValue<T>> {
        pointers
            .iter()
            .map(|x| PointerChainValue::new(h, base, x.clone()))
            .collect()
    }

    fn read(&self) -> T{
        Value::<T>::new(self.handle, self.base, &self.pointer).read()
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
    track_infos: Vec<PointerChainValue<[u8; 200]>>,
    deckcount: usize,
}



impl Rekordbox {
    fn new(offsets: RekordboxOffsets) -> Self {
        let rb = Process::from_process_name("rekordbox.exe")
            .expect("Could not find Rekordbox process! ");
        let h = rb.process_handle;

        let base = rb.get_module_base("rekordbox.exe").unwrap();

        let master_bpm_val: Value<f32> = Value::new(h, base, &offsets.master_bpm);

        let original_bpms = Value::pointers_to_vals(h, base, offsets.original_bpm);
        let beatgrid_shifts = Value::pointers_to_vals(h, base, offsets.beatgrid_shift);
        let beatgrid_beats = Value::pointers_to_vals(h, base, offsets.beatgrid_beat);
        let sample_positions = Value::pointers_to_vals(h, base, offsets.sample_position);
        let sample_rates = Value::pointers_to_vals(h, base, offsets.sample_rate);
        let track_infos = PointerChainValue::pointers_to_vals(h, base, offsets.track_info);

        let deckcount = beatgrid_shifts.len();

        let masterdeck_index_val: Value<u8> = Value::new(h, base, &offsets.masterdeck_index);

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
}

#[derive(Debug, PartialEq, Clone)]
struct TrackInfo {
    title: String,
    artist: String,
    album: String,
}
impl Default for TrackInfo {
    fn default() -> Self {
        Self {
            title: "".to_string(),
            artist: "".to_string(),
            album: "".to_string(),
        }
    }
}

#[derive(Clone)]
struct ChangeTrackedValue<T> {
    value: T,
}
impl<T: std::cmp::PartialEq> ChangeTrackedValue<T> {
    fn new(value: T) -> Self {
        Self { value }
    }
    fn set(&mut self, value: T) -> bool {
        if self.value != value {
            println!("Val changed");
            self.value = value;
            true
        } else {
            false
        }
    }
}

pub struct BeatKeeper {
    rb: Rekordbox,
    masterdeck_index: usize,
    offset_micros: f32,
    master_bpm: ChangeTrackedValue<f32>,
    running_modules: Vec<(Box<dyn OutputModule>, OutputModules)>,
    tracks: Vec<ChangeTrackedValue<TrackInfo>>,
    rx: Receiver<AppToKeeperMessage>,
    tx: Sender<ToAppMessage>,
}

type StatusMessage = Result<Option<String>, String>;

impl BeatKeeper {
    pub fn start(
        offsets: RekordboxOffsets,
        modules: Vec<(outputmodules::OutputModules, bool)>,
        config: HashMap<String, ModuleConfig>,
        rx: Receiver<AppToKeeperMessage>,
        tx: Sender<ToAppMessage>,
    ) {
        let update_rate = if let Some(map) = config.get("keeper") {
            map.get("update_rate")
                .unwrap_or(&"".to_string())
                .parse::<u64>()
                .unwrap_or(50)
        } else {
            50
        };
        let slow_update_denominator = if let Some(map) = config.get("keeper") {
            map.get("slow_update_every_nth")
                .unwrap_or(&"".to_string())
                .parse::<u64>()
                .unwrap_or(50)
        } else {
            1000
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

                    running_modules.push((match module {
                        OutputModules::AbletonLink => {
                            outputmodules::abletonlink::AbletonLink::create(conf)
                        }
                        OutputModules::Osc => outputmodules::osc::Osc::create(conf),
                    }, module));
                }

                let mut keeper = BeatKeeper {
                    rx,
                    tx,
                    rb: Rekordbox::new(offsets),
                    masterdeck_index: 0,
                    offset_micros: 0.,
                    master_bpm: ChangeTrackedValue::new(120.),
                    tracks: vec![ChangeTrackedValue::new(Default::default()); 4],
                    running_modules,
                };

                let period = Duration::from_micros(1000000 / update_rate); // 50Hz
                let mut n = 0;
                loop {
                    keeper.update(n == 0);
                    n = (n + 1) % slow_update_denominator;
                    thread::sleep(period);
                }
            }) {
                crash_tx
                    .send(ToAppMessage::Crash("Beatkeeper".to_string(), payload_to_string(&e)))
                    .unwrap();
                }
        });
    }

    pub fn update(&mut self, slow_update: bool) -> f32 {
        let bpm_changed = self.master_bpm.set(self.rb.master_bpm.read());
        self.masterdeck_index = self.rb.masterdeck_index.read() as usize;
        if self.masterdeck_index >= self.rb.deckcount {
            self.masterdeck_index = 0;
        }

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


        // println!("beat: {}", beat);
        // println!("s played: {}", seconds_played);
        // println!("origin {}", grid_origin);
        // println!("shift: {}", grid_shift);
        // println!("grid beat: {}", grid_beat);

        for module in &mut self.running_modules {
            module.0.beat_update(beat);
            if bpm_changed {
                module.0.bpm_changed(self.master_bpm.value);
            }
        }
        for (i, track) in self.get_track_infos().iter().enumerate(){
            if self.tracks[i].set(track.clone()){
                for module in &mut self.running_modules {
                    module.0.track_changed(track.clone(), i);
                }
                if self.masterdeck_index == i {
                    for module in &mut self.running_modules {
                        module.0.master_track_changed(track.clone());
                    }
                }
            }
        }

        if slow_update{
            let a = self.running_modules.iter_mut().map(|m| (m.1, m.0.slow_update())).collect::<Vec<(OutputModules, Result<Option<String>, String>)>>();
            for (module, res) in a{
                self.handle_response(module, res);
            }
        }

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
                let text = String::from_utf8(raw).unwrap_or("ERR".to_string());
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

    fn handle_response(&mut self, module: OutputModules, res: Result<Option<String>, String>){
        match res{
            Ok(msg) => {
                if let Some(msg) = msg{
                    self.tx.send(ToAppMessage::Status(module.to_string(), msg)).unwrap();
                } 
            },
            Err(err) => {
                self.tx.send(ToAppMessage::Crash(module.to_string(), err)).unwrap();
            }
        }
    }
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    let error_tx = catch_panic::start_panic_listener();

    let mut settings = iced::settings::Settings::with_flags(error_tx);
    settings.window.size = Size::new(600., 150.);


    crate::application::App::run(settings).unwrap();
}

// !cargo r
