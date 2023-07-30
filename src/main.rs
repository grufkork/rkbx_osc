use rosc::{encoder::encode, OscMessage, OscPacket, OscType};
use std::{
    env,
    io::{stdout, Write},
    marker::PhantomData,
    net::UdpSocket,
    thread::sleep,
    time::{Duration, Instant},
};
use toy_arms::external::{read, Process};
use winapi::um::winnt::HANDLE;



struct RekordboxOffsets{
    beat_baseoffset: usize,
    deck1: usize,
    deck2: usize,
    bar: usize,
    beat: usize,
    master_bpm: Offset,
    masterdeck_index: Offset
}

struct Offset{
    offests: Vec<usize>,
    final_offset: usize
}

impl Offset{
    fn new(offests: Vec<usize>, final_offset: usize) -> Offset{
        Offset { offests, final_offset }
    }
}

struct Value<T> {
    address: usize,
    handle: HANDLE,
    _marker: PhantomData<T>,
}

impl<T> Value<T> {
    fn new(h: HANDLE, base: usize, offsets: &[usize], final_offset: usize) -> Value<T> {
        let mut address = base;

        for offset in offsets {
            address = read::<usize>(h, address + offset).unwrap();
        }
        address += final_offset;

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

        let master_bpm_val: Value<f32> =
            Value::new(h, base, &[0x0434A4F0, 0x18, 0x110, 0x0, 0x70], 0x158);
        //println!("{}", master_bpm_val.read());

        const BEAT_BASEOFFSET: usize = 0x043498E0;
        let bar1_val: Value<i32> = Value::new(h, base, &[BEAT_BASEOFFSET, 0x118], 0x1e18);
        let beat1_val: Value<i32> = Value::new(h, base, &[BEAT_BASEOFFSET, 0x118], 0x1e1c);

        let bar2_val: Value<i32> = Value::new(h, base, &[BEAT_BASEOFFSET, 0x120], 0x1e18);
        let beat2_val: Value<i32> = Value::new(h, base, &[BEAT_BASEOFFSET, 0x120], 0x1e1c);

        // println!("{}.{}   {}.{}", bar1_val.read(), beat1_val.read(), bar2_val.read(), beat2_val.read());

        let masterdeck_index: Value<u8> = Value::new(h, base, &[0x043498E0, 0x90], 0x19C);
        //println!("{}", masterdeck_index.read());

        Self {
            master_bpm_val,
            bar1_val,
            beat1_val,
            bar2_val,
            beat2_val,

            masterdeck_index_val: masterdeck_index,

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
}

impl BeatKeeper {
    pub fn new() -> Self {
        BeatKeeper {
            rb: Some(Rekordbox::new()),
            last_beat: 0,
            beat_fraction: 1.,
            last_masterindex: 0,
        }
    }

    pub fn dummy() -> Self {
        BeatKeeper {
            rb: None,
            last_beat: 0,
            beat_fraction: 1.,
            last_masterindex: 0,
        }
    }

    pub fn update(&mut self, delta: Duration) {
        if let Some(rb) = &mut self.rb {
            let beats_per_micro = rb.master_bpm / 60. / 1000000.;

            self.beat_fraction =
                (self.beat_fraction + delta.as_micros() as f32 * beats_per_micro).min(1.);

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
}

const CHARS: [&str; 4] = ["|", "/", "-", "\\"];

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!(
            "Too few arguments!

 - Rekordbox OSC v0.1.0 -
A tool for sending Rekordbox timing data to visualizers using OSC
Usage: rkbox_osc.exe [source IP] [target IP] <Rekordbox version>"
        );
        return;
    }

    //let args = ["192.168.1.221:1337", "192.168.1.38:6669"];//.iter().map(|x|{x.to_string()}).collect();

    println!("Connecting from: {}", args[1]);
    println!("Connecting to:   {}", args[2]);

    let socket = UdpSocket::bind(&args[1]).unwrap();
    socket.connect(&args[2]).unwrap();

    let mut keeper = BeatKeeper::new();

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
            args: vec![OscType::Float(keeper.beat_fraction)],
        });
        let packet = encode(&msg).unwrap();
        socket.send(&packet[..]).unwrap();

        if count % 10 == 0 {
            step = (step + 1) % 4;

            let frac = (keeper.last_beat - 1) % 4;

            print!(
                "\rRunning {} [{}] Deck {}",
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
                keeper.last_masterindex
            );

            stdout.flush().unwrap();
        }
        count = (count + 1) % 120;

        sleep(period);
    }
}
