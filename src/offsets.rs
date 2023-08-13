use std::collections::HashMap;

impl RekordboxOffsets {
    pub fn default_version() -> &'static str {
    }

    pub fn get_available_versions() -> HashMap<&'static str, RekordboxOffsets> {
        let mut map = HashMap::new();

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
