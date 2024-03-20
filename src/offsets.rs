use std::{collections::HashMap, fs::File, io::Read};

impl RekordboxOffsets {
    pub fn from_string(input: &str) -> RekordboxOffsets {
        let mut rows = input.split("\r\n");
        RekordboxOffsets {
            rbversion: rows.next().unwrap().to_string(),
            beat_baseoffset: hexparse(rows.next().unwrap()),
            deck1: hexparse(rows.next().unwrap()),
            deck2: hexparse(rows.next().unwrap()),
            bar: hexparse(rows.next().unwrap()),
            beat: hexparse(rows.next().unwrap()),
            master_bpm: Offset::new(
                rows.next()
                    .unwrap()
                    .split(' ')
                    .map(hexparse)
                    .collect::<Vec<usize>>(),
                hexparse(rows.next().unwrap()),
            ),
            masterdeck_index: Offset::new(
                rows.next()
                    .unwrap()
                    .split(' ')
                    .map(hexparse)
                    .collect::<Vec<usize>>(),
                hexparse(rows.next().unwrap()),
            ),
        }
    }

    pub fn from_file(name: &str) -> HashMap<String, RekordboxOffsets> {
        let mut file = File::open(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        drop(file);

        let mut map = HashMap::new();
        for version in contents.split("\r\n\r\n") {
            let o = RekordboxOffsets::from_string(version);
            map.insert(o.rbversion.clone(), o);
        }
        map
    }
}

#[derive(Clone)]
pub struct RekordboxOffsets {
    pub rbversion: String,
    pub beat_baseoffset: usize,
    pub deck1: usize,
    pub deck2: usize,
    pub bar: usize,
    pub beat: usize,
    pub master_bpm: Offset,
    pub masterdeck_index: Offset,
}

#[derive(Clone)]
pub struct Offset {
    pub offsets: Vec<usize>,
    pub final_offset: usize,
}

impl Offset {
    pub fn new(offests: Vec<usize>, final_offset: usize) -> Offset {
        Offset {
            offsets: offests,
            final_offset,
        }
    }
}

fn hexparse(input: &str) -> usize {
    usize::from_str_radix(input, 16).unwrap()
}
