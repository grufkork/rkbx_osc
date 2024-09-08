use application::{AppToKeeperMessage, ToAppMessage};
use iced::window::Settings;
use iced::{Application};
use outputmodules::{OutputModule, OutputModules};
use rosc::{encoder::encode, OscMessage, OscPacket, OscType};
use rusty_link::{AblLink, SessionState};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::{
    env,
    io::{stdout, Write},
    marker::PhantomData,
    net::UdpSocket,
    path::Path,
    thread::{sleep, spawn},
    time::{Duration, Instant},
};
use toy_arms::external::{read, Process};
use winapi::um::winnt::HANDLE;

mod offsets;
use offsets::{Pointer, RekordboxOffsets};

mod application;
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
        }
    }

    fn update(&mut self) {
        // self.master_bpm = self.master_bpm.read();
        

        // self.masterdeck_index = self.masterdeck_index.read();

        // self.master_beats = self.beats[self.masterdeck_index as usize];
    }
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
    pub fn start(offsets: RekordboxOffsets, modules: Vec<(outputmodules::OutputModules, bool)>, rx: Receiver<AppToKeeperMessage>, tx: Sender<ToAppMessage>){

        thread::spawn(move || {
            let mut running_modules = vec![];

            for (module, active) in modules{
                if !active{
                    continue;
                }

                match module{
                    OutputModules::AbletonLink => {
                        running_modules.push(outputmodules::abletonlink::AbletonLink::new());
                    },
                    OutputModules::OSC => {

                    }
                }
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

            let period = Duration::from_micros(1000000 / 50); // 50Hz
            loop{
                keeper.update();
                thread::sleep(period);
            }

        });
    }

    pub fn update(&mut self) -> f32 {
        self.master_bpm = self.rb.master_bpm.read();
        self.masterdeck_index = self.rb.masterdeck_index.read() as usize;

        let samplerate = self.rb.sample_rates[self.masterdeck_index].read();
        let sample_poisition = self.rb.sample_positions[self.masterdeck_index].read();
        let seconds_played = sample_poisition as f32 / samplerate as f32;

        let grid_shift = self.rb.beatgrid_seconds[self.masterdeck_index].read();
        let grid_beat = self.rb.beatgrid_beats[self.masterdeck_index].read() as f32;
        let original_bpm = self.rb.original_bpms[self.masterdeck_index].read();
        let grid_size = 60. / original_bpm;

        let grid_origin = grid_shift as f32 + (grid_beat) * grid_size;

        let beat = (seconds_played - grid_origin) / grid_size;

        let bpm_changed = self.master_bpm != self.last_master_bpm;

        for module in &mut self.running_modules{
            module.beat_update(beat);
            if bpm_changed{
                module.bpm_changed(self.master_bpm);
            }
        }
        self.last_master_bpm = self.master_bpm;

        beat
    }
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    println!("Checking for updates...");




    /*let (tx, rx) = channel::<i8>();
      spawn(move || loop {
      tx.send(getch()).unwrap();
      });*/

    let args: Vec<String> = env::args().collect();

    let mut source_address = "0.0.0.0:0".to_string();
    let mut target_address = "127.0.0.1:6669".to_string();

    let mut osc_enabled = false;

    crate::application::App::run(iced::settings::Settings::default()).unwrap();



    let mut args_iter = args.iter();
    args_iter.next();
    /*while let Some(arg) = args_iter.next() {
        let mut chars = arg.chars();
        if let Some(char) = chars.next() {
            if char == '-' {
                if let Some(flag) = chars.next() {
                    match flag.to_string().as_str() {
                        "u" => {
                            println!("Updating offsets...");
                            download_offsets();
                            return;
                        }
                        "o" => {
                            osc_enabled = true;
                        }
                        "s" => {
                            source_address = args_iter.next().unwrap().to_string();
                        }
                        "t" => {
                            target_address = args_iter.next().unwrap().to_string();
                        }
                        "v" => {
                            target_version = args_iter.next().unwrap().to_string();
                        }
                        "h" => {
                            println!(
                                " - Rekordbox OSC v{} -
                                A tool for sending Rekordbox timing data to visualizers using OSC

                                Flags:

                                -h  Print this help
                                -u  Fetch latest offset list from GitHub and exit
                                -v  Rekordbox version to target, eg. 6.7.3

                                -- OSC --
                                -o  Enable OSC
                                -s  Source address, eg. 127.0.0.1:1337
                                -t  Target address, eg. 192.168.1.56:6667

                                Use i/k to change the beat offset by +/- 1ms

                                Current default version: {}
                                Available versions:",
                                env!("CARGO_PKG_VERSION"),
                                versions[0]
                                    );
                                println!("{}", versions.join(", "));

                                /*for v in  {
                                  print!("{v}, ");
                                  }*/
                                println!();
                                return;
                        }

                        c => {
                            println!("Unknown flag -{c}");
                        }
                    }
                }
            }
        }
    }*/


    let socket = if osc_enabled {
        println!("Connecting from: {}", source_address);
        println!("Connecting to:   {}", target_address);
        let socket = match UdpSocket::bind(&source_address) {
            Ok(socket) => socket,
            Err(e) => {
                println!("Failed to bind to address {source_address}. Error:\n{}", e);
                return;
            }
        };
        match socket.connect(&target_address) {
            Ok(_) => (),
            Err(e) => {
                println!(
                    "Failed to open socket to address {target_address}. Error:\n{}",
                    e
                );
                return;
            }
        };
        Some(socket)
    } else {
        None
    };

    println!();
    println!(
        "Press i/k to change offset in milliseconds. c to quit. -h flag for help and version info."
    );
    println!();

    /*let mut keeper = BeatKeeper::new(offsets.clone());
      let link = AblLink::new(120.);
      link.enable(false);

      let mut state = SessionState::new();
      link.capture_app_session_state(&mut state);
      link.enable(true);

      let period = Duration::from_micros(1000000 / 50); // 50Hz

      let mut count = 0;
      let mut step = 0;

      let mut stdout = stdout();

      let mut last_bpm = 120.;


      println!("Entering loop");
      loop {
      let master_beat = keeper.update(); // Get values, advance time

      let bfrac = master_beat % 1.;

      if let Some(socket) = &socket {
      let msg = OscPacket::Message(OscMessage {
      addr: "/beat".to_string(),
      args: vec![OscType::Float(bfrac)],
      });
      let packet = encode(&msg).unwrap();
      socket.send(&packet[..]).unwrap();
      }

      if keeper.master_bpm != last_bpm {
      state.set_tempo(keeper.master_bpm.into(), link.clock_micros());
      link.commit_app_session_state(&state);

      if let Some(socket) = &socket {
      let msg = OscPacket::Message(OscMessage {
      addr: "/bpm".to_string(),
      args: vec![OscType::Float(keeper.master_bpm)],
      });
      let packet = encode(&msg).unwrap();
      socket.send(&packet[..]).unwrap();
      }

      last_bpm = keeper.master_bpm;
      }

      let current_link_beat_approx = state.beat_at_time(link.clock_micros(), 4.).round();
      let target_beat = ((master_beat as f64) % 4. - current_link_beat_approx % 4. + 4.)
      % 4.
      + current_link_beat_approx
      - 1.; // Ensure the 1 is on the 1

      let target_beat = (master_beat as f64 + 1.) % 4.;

      state.force_beat_at_time(target_beat, link.clock_micros() as u64, 4.);
      link.commit_app_session_state(&state);
    /*if keeper.get_new_beat() {
    let current_link_beat_approx = state.beat_at_time(link.clock_micros(), 4.).round();
    let target_beat = ((keeper.last_beat as f64) % 4. - current_link_beat_approx % 4. + 4.)
    % 4
    + current_link_beat_approx
    - 1.; // Ensure the 1 is on the 1

    state.request_beat_at_time(target_beat, link.clock_micros(), 4.);
    link.commit_app_session_state(&state);
    }*/

    /*while let Ok(key) = rx.try_recv() {
      match key {
      99 => {
    //"c"
    return;
    }
    105 => {
    keeper.change_beat_offset(1000.);
    }
    107 => {
    keeper.change_beat_offset(-1000.);
    }
    _ => (),
    }
    }*/

    if count % 5 == 0 {
        step = (step + 1) % 4;

        let frac = (keeper.last_beat - 1) % 4;

        print!(
            "\rRunning {} [{}] Deck {}    Pos {}  OSC Offset: {}ms     Peers:{}   BPM:{}    ",
            CHARS[step],
            (0..4)
            .map(|i| {
                if i == frac {
                    "."
                } else {
                    " "
                }
            })
            .collect::<String>(),
            keeper.masterdeck_index,
            master_beat%4.,
            keeper.offset_micros / 1000.,
            link.num_peers(),
            keeper.master_bpm
        );

        stdout.flush().unwrap();
    }
    count = (count + 1) % 120;

    sleep(period);
}*/
}





// !cargo r -- -v 6.8.5
