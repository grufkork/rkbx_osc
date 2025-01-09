use std::os::windows::raw::HANDLE;
use std::thread;
use crate::config::Config;
use crate::log::ScopedLogger;
use crate::outputmodules::ModuleDefinition;
use crate::outputmodules::OutputModule;
use std::{marker::PhantomData, time::Duration};
use crate::offsets::Pointer;
use toy_arms::external::error::TAExternalError;
use toy_arms::external::{read, Process};
use std::mem::size_of_val;
use crate::RekordboxOffsets;

#[derive(PartialEq, Clone)]
struct ReadError {
    pointer: Option<Pointer>,
    address: usize,
    error: TAExternalError,
}
struct Value<T> {
    address: usize,
    handle: HANDLE,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: HANDLE, base: usize, offsets: &Pointer) -> Result<Value<T>, ReadError> {
        let mut address = base;

        for offset in &offsets.offsets {
            address = match read::<usize>(h, address + offset){
                Ok(val) => val,
                Err(e) => return Err(ReadError{pointer: Some(offsets.clone()), address: address+offset, error: e}),
            }
                // .unwrap_or_else(|_| panic!("\nMemory read failed, check your Rekordbox version! Try updating with -u.\nIf nothing works, wait for an update or send this entire error message to @grufkork. \n\nBase: {base:X}, Offsets: {offsets}"));
        }
        address += offsets.final_offset;

        Ok(Value::<T> {
            address,
            handle: h,
            _marker: PhantomData::<T>,
        })
    }
    fn pointers_to_vals(h: HANDLE, base: usize, pointers: Vec<Pointer>) -> Result<Vec<Value<T>>, ReadError> {
        pointers
            .iter()
            .map(|x| {Value::new(h, base, x)})
            .collect()
    }

    fn read(&self) -> Result<T, ReadError> {
        match read::<T>(self.handle, self.address){
            Ok(val) => Ok(val),
            Err(e) => Err(ReadError{pointer: None, address:self.address, error: e}),
        }
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

    fn read(&self) -> Result<T, ReadError> {
        Value::<T>::new(self.handle, self.base, &self.pointer)?.read()
    }
}



pub struct Rekordbox {
    master_bpm: Value<f32>,
    masterdeck_index: Value<u8>,
    original_bpms: Vec<Value<f32>>,
    beatgrid_seconds: Vec<Value<f64>>,
    beatgrid_beats: Vec<Value<i32>>,
    sample_positions: Vec<Value<i64>>,
    track_infos: Vec<PointerChainValue<[u8; 200]>>,
    deckcount: usize,
}



impl Rekordbox {
    fn new(offsets: RekordboxOffsets) -> Result<Self, ReadError> {
        let rb = match Process::from_process_name("rekordbox.exe"){
            Ok(p) => p,
            Err(e) => return Err(ReadError{pointer: None, address: 0, error: e}),
        };
        let h = rb.process_handle;


        let base = match rb.get_module_base("rekordbox.exe"){
            Ok(b) => b,
            Err(e) => return Err(ReadError{pointer: None, address: 0, error: e}),
        };

        let master_bpm_val: Value<f32> = Value::new(h, base, &offsets.master_bpm)?;

        let original_bpms = Value::pointers_to_vals(h, base, offsets.original_bpm)?;
        let beatgrid_shifts = Value::pointers_to_vals(h, base, offsets.beatgrid_shift)?;
        let beatgrid_beats = Value::pointers_to_vals(h, base, offsets.beatgrid_beat)?;
        let sample_positions = Value::pointers_to_vals(h, base, offsets.sample_position)?;
        let track_infos = PointerChainValue::pointers_to_vals(h, base, offsets.track_info);

        let deckcount = beatgrid_shifts.len();

        let masterdeck_index_val: Value<u8> = Value::new(h, base, &offsets.masterdeck_index)?;

        Ok(Self {
            master_bpm: master_bpm_val,
            original_bpms,
            beatgrid_seconds: beatgrid_shifts,
            beatgrid_beats,
            sample_positions,
            masterdeck_index: masterdeck_index_val,
            deckcount,
            track_infos,
        })
    }

    fn read_timing_data(&self) -> Result<TimingDataRaw, ReadError> {
        let master_bpm = self.master_bpm.read()?;
        let masterdeck_index = self.masterdeck_index.read()?.min(self.deckcount as u8 - 1);
        let sample_position = self.sample_positions[masterdeck_index as usize].read()?;
        let grid_shift = self.beatgrid_seconds[masterdeck_index as usize].read()?;
        let grid_beat = self.beatgrid_beats[masterdeck_index as usize].read()?;
        let original_bpm = self.original_bpms[masterdeck_index as usize].read()?;

        Ok(TimingDataRaw{
            master_bpm,
            masterdeck_index,
            sample_position,
            grid_shift,
            grid_beat,
            original_bpm
        })

    }

    fn get_track_infos(&self) -> Result<Vec<TrackInfo>, ReadError> {
        (0..self.deckcount)
            .map(|i| {
                let raw = self.track_infos[i]
                    .read()?
                    .into_iter()
                    .take_while(|x| *x != 0x00)
                    .collect::<Vec<u8>>();
                let text = String::from_utf8(raw).unwrap_or("ERR".to_string());
                let mut lines = text
                    .lines()
                    .map(|x| x.split_once(": ").unwrap_or(("", "")).1)
                    .map(|x| x.to_string());
                Ok(
                    TrackInfo {
                        title: lines.next().unwrap_or("".to_string()),
                        artist: lines.next().unwrap_or("".to_string()),
                        album: lines.next().unwrap_or("".to_string()),
                    }
                )
            })
        .collect()
    }

}

#[derive(Debug)]
struct TimingDataRaw{
    master_bpm: f32,
    masterdeck_index: u8,
    sample_position: i64,
    grid_shift: f64, // seconds shifted from the beat
    grid_beat: i32, // # of beats shifted
    original_bpm: f32,

}

#[derive(Debug, PartialEq, Clone)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
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
            self.value = value;
            true
        } else {
            false
        }
    }
}

pub struct BeatKeeper {
    masterdeck_index: usize,
    offset_micros: f32,
    master_bpm: ChangeTrackedValue<f32>,
    running_modules: Vec<Box<dyn OutputModule>>,
    tracks: Vec<ChangeTrackedValue<TrackInfo>>,
    logger: ScopedLogger,
    last_error: Option<ReadError>,
}

impl BeatKeeper {
    pub fn start(
        offsets: RekordboxOffsets,
        modules: Vec<ModuleDefinition>,
        config: Config,
        logger: ScopedLogger,
    ) {
        let keeper_config = config.reduce_to_namespace("keeper");
        let update_rate = keeper_config.get_or_default("update_rate", 50);
        let slow_update_denominator = keeper_config.get_or_default("slow_update_every_nth", 50);


        let mut running_modules = vec![];

        logger.info("Active modules:");
        for module in modules {
            if !config.get_or_default(&format!("{}.enabled", module.config_name), false) {
                continue;
            }

            let conf = config.reduce_to_namespace(&module.config_name);
            running_modules.push((module.create)(conf, ScopedLogger::new(&logger.logger, &module.pretty_name)));
            logger.info(&format!(" - {}", module.pretty_name));

            // running_modules.push((match module {
            //     OutputModules::AbletonLink => {
            //         outputmodules::abletonlink::AbletonLink::create(conf)
            //     }
            //     OutputModules::Osc => outputmodules::osc::Osc::create(conf),
            // }, module));
        }

        let mut keeper = BeatKeeper {
            masterdeck_index: 0,
            offset_micros: 0.,
            master_bpm: ChangeTrackedValue::new(120.),
            tracks: vec![ChangeTrackedValue::new(Default::default()); 4],
            running_modules,
            logger: logger.clone(),
            last_error: None,
        };

        let mut rekordbox = None;

        let period = Duration::from_micros(1000000 / update_rate); // 50Hz
        let mut n = 0;

        logger.info("Looking for Rekordbox...");
        println!();

        loop {
            if let Some(rb) = &rekordbox {
                if let Err(e) = keeper.update(rb, n == 0){
                    keeper.report_error(e);
                    
                    rekordbox = None;
                    logger.err("Connection to Rekordbox lost");
                    logger.info("Reconnecting...");

                }else{
                    n = (n + 1) % slow_update_denominator;
                    thread::sleep(period);
                }
            }else {
                match Rekordbox::new(offsets.clone()){
                    Ok(rb) => {
                        rekordbox = Some(rb);
                        println!();
                        logger.good("Connected to Rekordbox!");
                        keeper.last_error = None;
                    },
                    Err(e) => {
                        keeper.report_error(e);
                        logger.info("...");
                        thread::sleep(Duration::from_secs(3));
                    }
                }
            }


        }
    }

    fn report_error(&mut self, e: ReadError){
        if let Some(last) = &self.last_error{
            if e == *last{
                return;
            }
        }
        match &e.error {
            TAExternalError::ProcessNotFound | TAExternalError::ModuleNotFound => {
                self.logger.err("Rekordbox process not found!");
            },
            TAExternalError::SnapshotFailed(e) => {
                self.logger.err(&format!("Snapshot failed: {}", e));
                self.logger.info("    Ensure Rekordbox is running!");
            },
            TAExternalError::ReadMemoryFailed(e) => {
                self.logger.err(&format!("Read memory failed: {}", e));
                self.logger.info("    Wait for Rekordbox to start fully.");
                self.logger.info("    If the issue persists, check your configured Rekordbox version or try updating the offsets.");
                self.logger.info("    If nothing works, wait for an update - or enable Debug in config and send this entire error message to @grufkork.");
            },
            TAExternalError::WriteMemoryFailed(e) => {
                self.logger.err(&format!("Write memory failed: {}", e));
            },
        };
        if let Some(p) = &e.pointer{
            self.logger.debug(&format!("Pointer: {p:?}"));
        }
        if e.address != 0{
            self.logger.debug(&format!("Address: {:X}", e.address));
        }
        self.last_error = Some(e);
    }

    pub fn update(&mut self, rb: &Rekordbox, slow_update: bool) -> Result<(), ReadError> {
        let mut td = rb.read_timing_data()?;
        println!("{:?}", td);
        let bpm_changed = self.master_bpm.set(td.master_bpm);
        self.masterdeck_index = td.masterdeck_index as usize;
        if self.masterdeck_index >= rb.deckcount {
            self.masterdeck_index = 0;
        }

        // Sample position seems to always be counted as if the track is 44100Hz
        // - even when track or audio interface is 48kHz
        let seconds_played = td.sample_position as f32 / 44100.;

        // Unadjusted tracks have shift = 0. Adjusted tracks that begin on the first beat, have shift = 1
        td.grid_beat = td.grid_beat.max(1) - 1; 

        let grid_size = 60. / td.original_bpm;

        // Grid beat is how many whole beats the grid is shifted
        let grid_origin = td.grid_shift as f32 + td.grid_beat as f32 * grid_size; 

        let beat = (seconds_played - grid_origin) / grid_size;


        println!("beat: {}", beat);
        // println!("s played: {}", seconds_played);
        // println!("origin {}", grid_origin);
        // println!("shift: {}", grid_shift);
        // println!("grid beat: {}", grid_beat);

        for module in &mut self.running_modules {
            module.beat_update(beat);
            if bpm_changed {
                module.bpm_changed(self.master_bpm.value);
            }
        }


        if slow_update{
            for (i, track) in rb.get_track_infos()?.iter().enumerate(){
                if self.tracks[i].set(track.clone()){
                    for module in &mut self.running_modules {
                        module.track_changed(track.clone(), i);
                    }
                    if self.masterdeck_index == i {
                        self.logger.debug(&format!("Master track changed: {:?}", track));
                        for module in &mut self.running_modules {
                            module.master_track_changed(track.clone());
                        }
                    }
                }
            }
            for module in &mut self.running_modules{
                module.slow_update();
            }
        }

        Ok(())
    }

}

