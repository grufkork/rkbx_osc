use std::collections::HashMap;

impl RekordboxOffsets {
    pub fn default_version() -> &'static str {
        "6.8.3"
    }

    pub fn get_available_versions() -> HashMap<&'static str, RekordboxOffsets> {
        let mut map = HashMap::new();

        map.insert(
            "6.8.3",
            RekordboxOffsets {
                beat_baseoffset: 0x0443F650,
                deck1: 0x120,
                deck2: 0x128,
                bar: 0x1e18,
                beat: 0x1e1c,
                master_bpm: Offset::new(vec![0x04440260, 0x48, 0xF8, 0x28], 0xB98),
                masterdeck_index: Offset::new(vec![0x043DBDD0, 0x20, 0x278], 0xE20),
            }
        );

        map.insert(
            "6.8.2",
            RekordboxOffsets {
                beat_baseoffset: 0x043FB790,
                deck1: 0x120,
                deck2: 0x128,
                bar: 0x1e18,
                beat: 0x1e1c,
                master_bpm: Offset::new(vec![0x043FC3A0, 0x18, 0xF8, 0x0], 0x128),
                masterdeck_index: Offset::new(vec![0x04399C88, 0x20, 0x278], 0xe18),
            },
        );

        map.insert(
            "6.7.7",
            RekordboxOffsets {
                beat_baseoffset: 0x043BB250,
                deck1: 0x120,
                deck2: 0x128,
                bar: 0x1e18,
                beat: 0x1e1c,
                master_bpm: Offset::new(vec![0x043BBE60, 0x28, 0x208, 0x1d8], 0x140),
                masterdeck_index: Offset::new(vec![0x043BB250, 0x18, 0x720], 0x1058),
            },
        );

        map.insert(
            "6.7.4",
            RekordboxOffsets {
                beat_baseoffset: 0x04392560,
                deck1: 0x120,
                deck2: 0x128,
                bar: 0x1e18,
                beat: 0x1e1c,
                master_bpm: Offset::new(vec![0x0434c088, 0xe8, 0x1c0, 0x0], 0xb48),
                masterdeck_index: Offset::new(vec![0x04393430, 0x0, 0x58, 0x0, 0x530, 0x80], 0x144),
            },
        );

        map.insert(
            "6.7.3",
            RekordboxOffsets {
                beat_baseoffset: 0x043498e0,
                deck1: 0x118,
                deck2: 0x120,
                bar: 0x1e18,
                beat: 0x1e1c,
                master_bpm: Offset::new(vec![0x0434A4F0, 0x18, 0x110, 0x0, 0x70], 0x158),
                masterdeck_index: Offset::new(vec![0x043498e0, 0x90], 0x19c),
            },
        );

        map
    }
}

#[derive(Clone)]
pub struct RekordboxOffsets {
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
