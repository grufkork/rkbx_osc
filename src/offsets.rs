use core::fmt;
use std::{collections::HashMap, fs::File, io::Read};

pub type RekordboxOffsetCollection = HashMap<String, RekordboxOffsets>;

impl RekordboxOffsets {
    pub fn from_lines(lines: &[String]) -> Option<RekordboxOffsets> {
        let mut rows = lines.iter().peekable();

        let rb_version = rows.next()?.to_string();

        let master_bpm = Pointer::from_string(rows.next()?);
        let masterdeck_index = Pointer::from_string(rows.next()?);

        let mut beatgrid_shift = vec![];
        let mut beatgrid_beat = vec![];
        let mut sample_position = vec![];
        let mut sample_rate = vec![];
        let mut original_bpm = vec![];
        let mut track_info = vec![];

        while rows.peek().is_some() {
            original_bpm.push(Pointer::from_string(rows.next()?));
            beatgrid_shift.push(Pointer::from_string(rows.next()?));
            beatgrid_beat.push(Pointer::from_string(rows.next()?));
            sample_position.push(Pointer::from_string(rows.next()?));
            sample_rate.push(Pointer::from_string(rows.next()?));
            track_info.push(Pointer::from_string(rows.next()?));
        }

        Some(RekordboxOffsets {
            rbversion: rb_version,
            beatgrid_shift,
            beatgrid_beat,
            sample_position,
            sample_rate,
            original_bpm,
            master_bpm,
            masterdeck_index,
            track_info,
        })
    }

    pub fn from_file(name: &str) -> Result<HashMap<String, RekordboxOffsets>, String> {
        let Ok(mut file) = File::open(name) else {
            return Err(format!("Could not open offset file {name}"));
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return Err(format!("Could not read offset file {name}"));
        }
        drop(file);

        let mut empty_line_count = 0;

        let mut map = HashMap::new();

        let mut lines = vec![];
        for line in contents.lines() {
            if line.is_empty() {
                empty_line_count += 1;
                if empty_line_count >= 2 && !lines.is_empty() {
                    if let Some(offsets) = RekordboxOffsets::from_lines(&lines) {
                        map.insert(offsets.rbversion.clone(), offsets);
                    } else {
                        println!("Failed to parse offsets: {lines:?}");
                    }
                    lines.clear();
                }
            } else {
                empty_line_count = 0;
                if !line.starts_with('#') {
                    lines.push(line.to_string());
                }
            }
        }

        Ok(map)
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
    pub track_info: Vec<Pointer>,
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
        for offset in &self.offsets {
            res += &format!("{offset:X}, ");
        }
        res += &format!("{:X}]", self.final_offset);

        write!(f, "{res}")
    }
}

fn hexparse(input: &str) -> usize {
    usize::from_str_radix(input, 16).unwrap()
}
