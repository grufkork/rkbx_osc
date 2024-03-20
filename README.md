# Rekordbox OSC
A tiny tool for sending Rekordbox timing information to visualizers etc. over OSC. 
Currently an MVP, with future functions including sending more information and better UX.

## What does it do?
When run on the same computer as a Rekordbox instance, it will send the current beat fraction on OSC address `/beat` to the specified target IP. This can be received by, for instance, a visualiser and drive an animation. The value is a float, set to zero on the beat and increasing to 1 just before the next beat. It should send updates at approximately 60Hz. 

The program does not interact with the audio stream in any way, but reads the onscreen text values through memory. Thus your beatgrid must be correct for it to work as expected. 

## Why?
Rekordbox's Ableton Link integration leaves some to be desired.

## Usage
`rkbx_osc.exe [flags]`
where
```
 -s  Source address, eg. 127.0.0.1:1337
 -t  Target address, eg. 192.168.1.56:6667
 -v  Rekordbox version to target, eg. 6.7.3
 -h  Print help and available versions
```
If no arguments are given, it defaults to the latest supported rekordbox version and sending to 127.0.0.1:6669. As messages are sent with UDP, source address should not need to be set.

The program will then send:
 - the current beat fraction, as a float counting from 0 to 1, to the OSC address `/beat`
 - the master deck tempo in BPM on OSC address `/bpm`

## How it works
The timing information is extracted through reading Rekordbox's memory. The program reads the current beat and measure from the text display on top of the large waveform, and detects when these change.
When a change occurs, the beat fraction is set to 0 and then counts linearly upwards at a pace dictated by the master track BPM.

## Limitations
- Only supports two decks.
- Might register extra beats when switching master deck
- Assumes 4/4 time signature. (Does Rekordbox support anything else? 3/4 and lower shoud work OK, 5/4 and higher might behave strangely)
- Windows only

## Supported versions
Any version not listed will 99% not work, but you can always try using an adjacent version.

| Rekordbox Version  | Verified? |
| ----- | --- |
| 6.8.3 | ✔️ |
| 6.8.2 | ✔️ |
| 6.7.7 | ✔️ |
| 6.7.4 | ✔️ |
| 6.7.3 | ✔️ |

# Technical Details
## Updating
Every Rekordbox update the memory offsets change. Some have proven to remain the same, but usually the first offsets in the paths require updating. 
To find these, I use Cheatengine, using pointerscans and trying to find the shortest pointer paths.

Easiest method seems to be to find each value, pointerscan, save that, then reopen rekordbox and filter the pointerscans by value.

Updates are welcome, put them in the `offsets.rs` file.

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
Windows, by default, only has sleeps in increments of ~16ms. As such, the the sending frequency is a bit uneven. The rate is set to 120Hz in the code, but that results in about 60Hz update rate.
