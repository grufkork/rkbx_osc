# Rekordbox OSC
A tiny tool for sending Rekordbox timing information to visualizers etc. over OSC. 
Currently an MVP, with future functions including sending more information and better UX.

## Usage
`rkbx_osc.exe [Source IP] [Target IP] <Rekordbox version>`

IP's are required, version defaults to latest available version. Parameters are given as strings like so: `rkbx_osc.exe 127.0.0.1:1337 127.0.0.1:6669 6.7.3`

The program will then send the current beat fraction, as a float counting from 0 to 1, to the OSC address `/beat`.

Run without arguments to list available Rekordbox versions. 

## How it works
The timing information is extracted through reading Rekordbox's memory. The program reads the current beat and measure from the text display on top of the large waveform, and detects when these change.
When a change occurs, the beat fraction is set to 0 and then counts linearly upwards at a pace dictated by the master track BPM.

## Limitations
- Only supports two decks.
- Might register extra beats when switching

## Updating
Every Rekordbox update the memory offsets change. Some have proven to remain the same, but usually the first offsets in the chains require updating. 
To find these, I use Cheatengine, doing pointerscans to find the shortest pointer path.

- `master_bpm` - The BPM value of the current master track. Find by loading a track on deck 1 & 2, then search for a float containing the BPM of the deck currently set as Master.
- `masterdeck_index` - The index of the deck currently set as Master. 0 for deck 1, 1 for deck 2. Not sure if the value I've found is the index of the selected deck, or a boolean dictating if Deck 2 is master. I searched for a 32-bit int.
- `beat_baseoffset` - The first value in the chain to find the measure/beat displayed on the large waveform.
- `deck1, deck2, bar, beat` - Appear to remain the same. These are offsets from the `beat_baseoffset`.

## Supported versions
| Rekordbox Version  | Verified? |
| ----- | --- |
| 6.7.3 | ✔️ |
