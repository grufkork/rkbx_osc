use core::fmt;
use std::{collections::HashMap, fs::File, io::Read};

use crate::log::ScopedLogger;

impl RekordboxOffsets {
    pub fn from_lines(lines: &[String], logger: &ScopedLogger) -> Option<RekordboxOffsets> {
        let mut rows = lines.iter().peekable();

        let rb_version = rows.next()?.to_string();

        logger.debug("Masterdeck index");
        let masterdeck_index = Pointer::from_string(rows.next()?, logger);

        let mut sample_position = vec![];
        let mut current_bpm = vec![];
        let mut playback_speed = vec![];
        let mut beat_display = vec![];
        let mut track_info = vec![];

        while rows.peek().is_some() {
            logger.debug("Current BPM");
            current_bpm.push(Pointer::from_string(rows.next()?, logger));
            logger.debug("Beat display");
            beat_display.push(Pointer::from_string(rows.next()?, logger));
            logger.debug("Playback speed");
            playback_speed.push(Pointer::from_string(rows.next()?, logger));
            logger.debug("Sample position");
            sample_position.push(Pointer::from_string(rows.next()?, logger));
            logger.debug("Track info");
            track_info.push(Pointer::from_string(rows.next()?, logger));
        }

        Some(RekordboxOffsets {
            rbversion: rb_version,
            beat_display,
            sample_position,
            current_bpm,
            playback_speed,
            masterdeck_index,
            track_info,
        })
    }

    pub fn from_file(name: &str, logger: ScopedLogger) -> Result<HashMap<String, RekordboxOffsets>, String> {
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
                    if let Some(offsets) = RekordboxOffsets::from_lines(&lines, &logger) {
                        map.insert(offsets.rbversion.clone(), offsets);
                    } else {
                        return Err("Failed to parse offsets".to_string());
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
    pub masterdeck_index: Pointer,
    pub sample_position: Vec<Pointer>,
    pub current_bpm: Vec<Pointer>,
    pub playback_speed: Vec<Pointer>,
    pub beat_display: Vec<Pointer>,
    pub track_info: Vec<Pointer>,
}

#[derive(PartialEq, Clone, Debug)]
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

    pub fn from_string(input: &str, logger: &ScopedLogger) -> Self {
        logger.debug(&format!("Parsing pointer: {input}"));
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
