use core::fmt;
use std::{collections::HashMap, fs::File, io::Read};

impl RekordboxOffsets {
    pub fn from_lines(lines: &[String]) -> RekordboxOffsets {
        let mut rows = lines.iter().peekable();

        let rb_version = rows.next().unwrap().to_string();

        let master_bpm = Pointer::from_string(rows.next().unwrap());
        let masterdeck_index = Pointer::from_string(rows.next().unwrap());

        let mut beatgrid_shift = vec![];
        let mut beatgrid_beat = vec![];
        let mut sample_position = vec![];
        let mut sample_rate = vec![];
        let mut original_bpm = vec![];

        while rows.peek().is_some(){
            original_bpm.push(Pointer::from_string(rows.next().unwrap()));
            beatgrid_shift.push(Pointer::from_string(rows.next().unwrap()));
            beatgrid_beat.push(Pointer::from_string(rows.next().unwrap()));
            sample_position.push(Pointer::from_string(rows.next().unwrap()));
            sample_rate.push(Pointer::from_string(rows.next().unwrap()));
        }

        RekordboxOffsets {
            rbversion: rb_version,
            beatgrid_shift,
            beatgrid_beat,
            sample_position,
            sample_rate,
            original_bpm,
            master_bpm,
            masterdeck_index,
        }


    }

    pub fn from_file(name: &str) -> HashMap<String, RekordboxOffsets> {
        let mut file = File::open(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        drop(file);

        let mut map = HashMap::new();

        let mut lines = vec![];
        for line in contents.lines() {
            if line.is_empty() {
                if !lines.is_empty() {
                    let o = RekordboxOffsets::from_lines(&lines);
                    map.insert(o.rbversion.clone(), o);
                    lines.clear();
                }
            } else if !line.starts_with('#') {
                lines.push(line.to_string());
            }
        }

        map
    }
}

#[derive(Clone, Debug)]
pub struct RekordboxOffsets {
    pub rbversion: String,
    pub master_bpm: Pointer,
    pub masterdeck_index: Pointer,
    pub beatgrid_shift: Vec<Pointer>,
    pub beatgrid_beat: Vec<Pointer>,
    pub sample_position: Vec<Pointer>,
    pub sample_rate: Vec<Pointer>,
    pub original_bpm: Vec<Pointer>,
}

#[derive(Clone, Debug)]
pub struct Pointer {
    pub offsets: Vec<usize>,
    pub final_offset: usize,
}

impl Pointer {
    pub fn new(offests: Vec<usize>, final_offset: usize) -> Pointer {
        Pointer {
            offsets: offests,
            final_offset,
        }
    }

    pub fn from_string(input: &str) -> Self {
        let split = input.split(' ').map(hexparse).collect::<Vec<usize>>();
        Self::new(split[0..split.len() - 1].to_vec(), *split.last().unwrap())
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = "[".to_string();
        for offset in &self.offsets{
            res += &format!("{offset:X}, ");
        }
        res += &format!("{:X}]", self.final_offset);

        write!(f, "{res}")
    }
}

fn hexparse(input: &str) -> usize {
    usize::from_str_radix(input, 16).unwrap()
}
