# Rekordbox OSC
Connecting Rekordbox to visualizers and music software over Ableton Link and OSC

## What does it do?
When run on the same computer as an instance of Rekordbox, it will read the current timing information and send this over your protocol of choice. By default it outputs a 4-beat aligned signal using Ableton Link, but it can also transmit equivalent data over OSC, although with less accurate timing. 

The program does not interact with the audio stream in any way, but reads the onscreen text values through memory. Thus your beatgrid must be correct for it to work as expected. 

## Why?
Rekordbox's Ableton Link integration only allows for receiving a signal, not extracting it.

## Usage
`rkbx_osc.exe [flags]`
where
``` 
 -h  Print help and available versions
 -u  Fetch latest offset list from GitHub and exit
 -v  Rekordbox version to target, eg. 6.7.3

-- OSC --
 -o  Enable OSC
 -s  Source address, eg. 127.0.0.1:1337
 -t  Target address, eg. 192.168.1.56:6667
```
If no arguments are given, it defaults to the latest supported rekordbox version and Ableton Link. If OSC is enabled, it will send to 127.0.0.1:6669. As messages are sent with UDP, source address should not need to be set.

## OSC Addresses
 - `/beat`: the current beat fraction, as a float counting from 0 to 1
 - `/bpm`: the master deck tempo in BPM

## How it works
The timing information is extracted through reading Rekordbox's memory. The program reads the current beat and measure from the text display on top of the large waveform, and detects when these change.
When a change occurs, the beat fraction is set to 0 and then counts linearly upwards at a pace dictated by the master track BPM.

## Limitations
- Only supports two decks.
- Might register an extra beat when switching master deck.
- Assumes 4/4 time signature - Rekordbox does not support anything else without manually editing the database
- Windows only

## Supported versions
Any version not listed will 99% not work, but you can always try using an adjacent version.

| Rekordbox Version  | Verified? |
| ----- | --- |
| 6.8.4 | ✔️ |
| 6.8.3 | ✔️ |
| 6.8.2 | ✔️ |
| 6.7.7 | ✔️ |
| 6.7.4 | ✔️ |
| 6.7.3 | ✔️ |

# Technical Details

## Offsets file format
The `offsets` file contain the hex memory addresses (without the leading 0x) for the values we need to fetch. The file supports basic comments (# at start of line). Versions are separated by two newlines.

Example entry with explanations:
```
6.8.3               version number
0443F650            beat_baseoffset
120                 deck1
128                 deck2
1e18                bar
1e1c                beat
04440260 48 F8 28   master_bpm offsets
B98                 master_bpm final offset
043DBDD0 20 278     masterdeck_index offsets
E20                 masterdeck_index final offset
```

"Offsets" and "final offset" refers to the chain of offsets found in eg. Cheat Engine, where the final offset is the value added to the address found at the end of the chain. 

## Updating
Every Rekordbox update the memory offsets change. Some (deck1, bar and beat mostly) have proven to remain the same, but usually the first offsets in the paths require updating. 
To find these, I use Cheat Engine, using pointerscans and trying to find the shortest pointer paths.

Easiest method seems to be to find each value, pointerscan, save that, then reopen rekordbox and filter the pointerscans by value.

Updates are welcome, put them in the `offsets` file.

### `master_bpm`
The BPM value of the current master track. Find by loading a track on deck 1 & 2, then search for a float containing the BPM of the deck currently set as Master.

### `masterdeck_index`
The index of the deck currently set as Master. 0 for deck 1, 1 for deck 2. Not sure if the value I've found is the index of the selected deck, or a boolean dictating if Deck 2 is master. Search for a byte.

This one is usually the trickiest. There are a couple of other values wich correlate but actually change on hover etc., so be careful. The path should not be longer than 4 addresses, so find a bunch of candidates (should be able to reduce to <30) and then pointer scan for each until you get a short one - that should be it.

### `beat_baseoffset`
The first value in the path to any of the measure/beat displays at the top of the large waveform, shown as "measure.beat". Search for 32-bit ints

### `deck1, deck2, bar, beat`
Appear to remain the same. These are offsets added to `beat_baseoffset` to find the specific values.

## Notes on timing
Windows, by default, only has sleeps in increments of ~16ms. As such, the the sending frequency is a bit uneven. The rate is set to 120Hz in the code, but that results in about 60Hz update rate. I'm not sure if the method measuring the delta time is accurate enough, or 
