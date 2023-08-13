use rosc::{encoder::encode, OscMessage, OscPacket, OscType};
use std::{
    env,
    io::{stdout, Write},
    marker::PhantomData,
    net::UdpSocket,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::{Duration, Instant},
};
use toy_arms::external::{read, Process};
use winapi::um::winnt::HANDLE;

mod offsets;
use offsets::{Offset, RekordboxOffsets};

extern "C" {
    fn _getch() -> core::ffi::c_char;
}

fn getch() -> i8 {
    unsafe { _getch() }
}

struct Value<T> {
    address: usize,
    handle: HANDLE,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: HANDLE, base: usize, offsets: Offset) -> Value<T> {
        let mut address = base;

        for offset in offsets.offsets {
            address = read::<usize>(h, address + offset).expect("Memory read failed, check your Rekordbox version!");
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
    master_bpm_val: Value<f32>,
    bar1_val: Value<i32>,
    beat1_val: Value<i32>,
    bar2_val: Value<i32>,
    beat2_val: Value<i32>,
    masterdeck_index_val: Value<u8>,

    pub beats1: i32,
    pub beats2: i32,
    pub master_beats: i32,
    pub master_bpm: f32,
    pub masterdeck_index: u8,
}

impl Rekordbox {
    fn new(offsets: RekordboxOffsets) -> Self {
        //println!("Hello, world!");
        let rb = Process::from_process_name("rekordbox.exe").unwrap();
        let h = rb.process_handle;
        /*println!(
            "process id = {}, \nprocess handle = {:?}",
            rb.process_id, h
        );*/

        let base = rb.get_module_base("rekordbox.exe").unwrap();
        //base = 0x300905A4D;
        //base = 0x266E1532160;
        //println!("Base: {:X}", base);

        let master_bpm_val: Value<f32> = Value::new(h, base, offsets.master_bpm);
        //println!("{}", master_bpm_val.read());

        let bar1_val: Value<i32> = Value::new(
            h,
            base,
            Offset::new(vec![offsets.beat_baseoffset, offsets.deck1], offsets.bar),
        );
        let beat1_val: Value<i32> = Value::new(
            h,
            base,
            Offset::new(vec![offsets.beat_baseoffset, offsets.deck1], offsets.beat),
        );
        let bar2_val: Value<i32> = Value::new(
            h,
            base,
            Offset::new(vec![offsets.beat_baseoffset, offsets.deck2], offsets.bar),
        );
        let beat2_val: Value<i32> = Value::new(
            h,
            base,
            Offset::new(vec![offsets.beat_baseoffset, offsets.deck2], offsets.beat),
        );

        // println!("{}.{}   {}.{}", bar1_val.read(), beat1_val.read(), bar2_val.read(), beat2_val.read());

        let masterdeck_index_val: Value<u8> = Value::new(h, base, offsets.masterdeck_index);
        //println!("{}", masterdeck_index.read());

        Self {
            master_bpm_val,
            bar1_val,
            beat1_val,
            bar2_val,
            beat2_val,

            masterdeck_index_val,

            beats1: -1,
            beats2: -1,
            master_bpm: 120.0,
            masterdeck_index: 0,
            master_beats: 0,
        }
    }

    fn update(&mut self) {
        self.master_bpm = self.master_bpm_val.read();
        self.beats1 = self.bar1_val.read() * 4 + self.beat1_val.read();
        self.beats2 = self.bar2_val.read() * 4 + self.beat2_val.read();
        self.masterdeck_index = self.masterdeck_index_val.read();

        self.master_beats = if self.masterdeck_index == 0 {
            self.beats1
        } else {
            self.beats2
        };
    }
}

pub struct BeatKeeper {
    rb: Option<Rekordbox>,
    last_beat: i32,
    pub beat_fraction: f32,
    pub last_masterindex: u8,
    pub offset_micros: f32,
}

impl BeatKeeper {
    pub fn new(offsets: RekordboxOffsets) -> Self {
        BeatKeeper {
            rb: Some(Rekordbox::new(offsets)),
            last_beat: 0,
            beat_fraction: 1.,
            last_masterindex: 0,
            offset_micros: 0.,
        }
    }

    pub fn dummy() -> Self {
        BeatKeeper {
            rb: None,
            last_beat: 0,
            beat_fraction: 1.,
            last_masterindex: 0,
            offset_micros: 0.,
        }
    }

    pub fn update(&mut self, delta: Duration) {
        if let Some(rb) = &mut self.rb {
            let beats_per_micro = rb.master_bpm / 60. / 1000000.;

            self.beat_fraction =
                (self.beat_fraction + delta.as_micros() as f32 * beats_per_micro) % 1.;

            rb.update();

            if rb.masterdeck_index != self.last_masterindex {
                self.last_masterindex = rb.masterdeck_index;
                self.last_beat = rb.master_beats;
            }

            if (rb.master_beats - self.last_beat).abs() > 0 {
                self.last_beat = rb.master_beats;
                self.beat_fraction = 0.;
            }
        } else {
            self.beat_fraction = (self.beat_fraction + delta.as_secs_f32() * 130. / 60.) % 1.;
        }
    }
    pub fn get_beat_faction(&mut self) -> f32 {
        (self.beat_fraction
            + if let Some(rb) = &self.rb {
                let beats_per_micro = rb.master_bpm / 60. / 1000000.;
                self.offset_micros * beats_per_micro
            } else {
                0.
            })
            % 1.
    }

    pub fn change_beat_offset(&mut self, offset: f32) {
        self.offset_micros += offset;
    }
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    let (tx, rx) = channel::<i8>();
    spawn(move || loop {
        tx.send(getch()).unwrap();
    });

    let args: Vec<String> = env::args().collect();

    let mut source_address = "127.0.0.1:1337".to_string();
    let mut target_address = "127.0.0.1:6669".to_string();
    let mut version = RekordboxOffsets::default_version().to_string();
    //let mut d = ""

    let versions = RekordboxOffsets::get_available_versions();

    let mut args_iter = args.iter();
    args_iter.next();
    while let Some(arg) = args_iter.next() {
        let mut chars = arg.chars();
        if let Some(char) = chars.next() {
            if char == '-' {
                if let Some(flag) = chars.next() {
                    match flag.to_string().as_str() {
                        "s" => {
                            source_address = args_iter.next().unwrap().to_string();
                        }
                        "t" => {
                            target_address = args_iter.next().unwrap().to_string();
                        }
                        "v" => {
                            version = args_iter.next().unwrap().to_string();
                        }
                        "h" => {
                            println!(
                                " - Rekordbox OSC v0.1.0 -
A tool for sending Rekordbox timing data to visualizers using OSC

Flags:

 -s  Source address, eg. 127.0.0.1:1337
 -t  Target address, eg. 192.168.1.56:6667
 -v  Rekordbox version to target, eg. 6.7.3
 -h  Print this help

Use i/k to change the beat offset by +/- 1ms

Current default version: {}
Available versions:",
                                RekordboxOffsets::default_version()
                            );
                            for v in versions.keys() {
                                print!("{v}, ");
                            }
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
    }

    /*if args.len() < 3 {
            println!(
                "Too few arguments!

     - Rekordbox OSC v0.1.0 -
    A tool for sending Rekordbox timing data to visualizers using OSC
    Usage: rkbox_osc.exe [source IP] [target IP] <Rekordbox version>

    Current default version: {}
    Available versions:",
            RekordboxOffsets::default_version());
            for v in versions.keys(){
                print!("{v}, ");
            }
            println!();
            return;
        }*/

    //let version = if args.len() > 3 {&args[3]}else{RekordboxOffsets::default_version()};

    let offsets = if let Some(offsets) = versions.get(version.as_str()) {
        offsets
    } else {
        println!("Unsupported version! {version}");
        return;
    };
    println!("Targeting Rekordbox version {version}");

    //let args = ["192.168.1.221:1337", "192.168.1.38:6669"];//.iter().map(|x|{x.to_string()}).collect();

    println!("Connecting from: {}", source_address);
    println!("Connecting to:   {}", target_address);

    println!("");
    println!("Press i/k to change offset in milliseconds. c to quit.");
    println!("");

    let socket = match UdpSocket::bind(&source_address) {
        Ok(socket) => socket,
        Err(e) => {
            println!(
                "Failed to bind to address {source_address}. Error:\n{}",
                e.to_string()
            );
            return;
        }
    };
    match socket.connect(&target_address) {
        Ok(_) => (),
        Err(e) => {
            println!(
                "Failed to open socket to address {target_address}. Error:\n{}",
                e.to_string()
            );
            return;
        }
    }

    let mut keeper = BeatKeeper::new(offsets.clone());

    let period = Duration::from_millis(1000 / 60);

    let mut last_instant = Instant::now();

    let mut count = 0;
    let mut step = 0;

    let mut stdout = stdout();

    println!("Entering loop");
    loop {
        let delta = Instant::now() - last_instant;
        last_instant = Instant::now();

        keeper.update(delta);

        let msg = OscPacket::Message(OscMessage {
            addr: "/beat".to_string(),
            args: vec![OscType::Float(keeper.get_beat_faction())],
        });
        let packet = encode(&msg).unwrap();
        socket.send(&packet[..]).unwrap();

        while let Ok(key) = rx.try_recv() {
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
        }

        if count % 10 == 0 {
            step = (step + 1) % 4;

            let frac = (keeper.last_beat - 1) % 4;

            print!(
                "\rRunning {} [{}] Deck {}     Offset: {}ms       ",
                CHARS[step],
                (0..4)
                    .map(|i| {
                        if i <= frac {
                            "."
                        } else {
                            " "
                        }
                    })
                    .collect::<String>(),
                keeper.last_masterindex,
                keeper.offset_micros / 1000.
            );

            stdout.flush().unwrap();
        }
        count = (count + 1) % 120;

        sleep(period);
    }
}
