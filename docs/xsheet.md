# Mouth chart and X-sheet

Kilsat evaluates 13 visemes (`motion_lipsync::Viseme`). Drawn replacement mouths
usually come as the nine classic shapes (Preston Blair / Rhubarb `A`–`H` and `X`).
`motion_lipsync::mouth_chart` projects the 13 visemes onto those nine shapes, and
`motion_lipsync::xsheet` lists the result frame by frame as an exposure sheet
(X-sheet) that can be exported as CSV or fixed-width text.

The 13-viseme enum is unchanged; the chart is a derived view.

## Mouth chart (13 → 9)

`MouthChart::from_viseme` is the only projection. `EstimatedVisemeTrack::to_mouth_cues`
and the X-sheet both use it. It is total: every viseme has exactly one shape, and
every shape is reached.

| Shape | Meaning | Kilsat visemes |
| --- | --- | --- |
| `A` | Closed lips: P, B, M | `closed` |
| `B` | Slightly open, teeth together: most consonants, EE | `i` |
| `C` | Open: EH, AE | `e`, `sibilant`, `sh`, `r` |
| `D` | Wide open: AA | `a` |
| `E` | Slightly rounded: AO, ER | `o` |
| `F` | Puckered: UW, OW, W | `u` |
| `G` | Upper teeth on lower lip: F, V | `teeth_lip` |
| `H` | Tongue raised or visible: L, T, D, N, TH | `tongue_up`, `tongue_teeth` |
| `X` | Idle / rest | `rest` |

The projection is the inverse of the Rhubarb import mapping (`rhubarb::shape_viseme`)
on that mapping's image, so an imported Rhubarb cue keeps its letter when it is shown
on the chart again. The projection loses information. `sibilant`, `sh` and `r` share `C`
with `e`, and `tongue_teeth` shares `H` with `tongue_up`. A rig with all 13
mouth shapes should keep using the visemes directly.

## X-sheet

`xsheet::build(scene_id, duration_samples, fps, tracks)` returns one row per video frame
and one cell per speaker. `XSheet::from_audio_timeline` builds the tracks from a
validated `AudioTimeline`.

### Timing rules

- All times are whole samples on the shared 24 kHz dialogue clock
  (`motion_audio::clock`). No floating-point seconds are used.
- Frame `f` covers scene samples `[frame_to_sample(f), frame_to_sample(f + 1))`.
- The mouth in a cell is the viseme at the frame's **first** sample. Video export
  uses the same rule, so the sheet matches the exported mouths.
- The timecode `HH:MM:SS:FF` (non-drop-frame) is derived from the integer frame
  index.
- The row count is the number of frames that show at least one sample of the scene.
  Empty audio gives an empty sheet. Frame rates of 1–240 fps and up to 864 000
  frames are accepted; anything else is a typed error instead of an allocation.
- `XSheetTrack::from_timeline_clip` converts source-local cues to scene samples. It
  applies the clip offset (`start_sample`) and trim (`trim_start_samples`) and clips
  the cues to the audible window.

### Validation

Tracks are validated before any row is produced. An invalid input returns an
`XSheetError`, never a partly filled sheet. A clip must be non-empty and must end
within the scene. Viseme, phoneme and word spans must be ordered, non-empty,
non-overlapping and inside their clip. Text is limited to 4096 bytes and must not
contain control characters. Two clips of the same speaker must not overlap.

### Columns

| Column | Content |
| --- | --- |
| `frame` | Frame index from 0 |
| `timecode` | `HH:MM:SS:FF` |
| `speaker` | Actor name; one record per speaker and frame |
| `dialogue` | Text starting in this frame, `|` while a line is held, empty otherwise |
| `phonemes` | Phonemes starting in this frame (when the source provides them) |
| `viseme` | Kilsat viseme name (`rest`, `closed`, …) |
| `chart` | `A`–`H` or `X` |
| `timing` | `manual`, `forced:<tool>/<version>`, `estimated:<tool>/<version>`, `none` (audible clip without mouth timing) or empty (speaker silent) |

A frame without any speaker keeps one record with empty speaker fields, so every frame
appears in the export.

### Export

- `XSheet::to_csv`: RFC 4180 CSV with `\n` line endings and a header row.
- `XSheet::to_text`: fixed-width text with a `# X-sheet · scene … · fps · samples @ Hz`
  header line. Columns are padded by character count, and control characters are
  replaced by spaces.

Both are byte-identical for identical input, and a test checks this.

### Dialogue text

There is no word-level timing in the pipeline yet. `XSheetTrack::with_line` attaches the
whole line as one span over the clip. The line therefore appears on its first frame and
is held (`|`) after that. Word times are never guessed. When word timing becomes
available, the caller can fill `XSheetTrack::words`, and the same notation shows one
entry per word.

## Desktop panel

`apps/studio-desktop/src/xsheet_panel.rs` adds a collapsible
**X-sheet ja suukartta (A–H, X)** panel next to the viseme check (UI level 2).

- The sheet is built from the prepared scene's export manifest, the same data video
  export writes. It is rebuilt only when the manifest or the project fps changes.
- Building the sheet and writing the export file run on one worker thread with bounded
  channels. The event loop only draws finished rows, using virtualized `show_rows`.
- With **Seuraa toistopäätä** on, the list scrolls with the playhead and the row of the
  current frame is highlighted. Clicking a row seeks the playhead to that frame's
  first sample.
- Export writes CSV or text to the given path. A relative path is resolved against the
  project root.
- Dialogue text is matched to manifest entries by screenplay order and actor. If they
  do not match, the text is left out instead of being attached to the wrong clip.

The panel has unit tests for manifest-to-sheet conversion and for the worker. The
graphical panel has not been exercised by hand.

## Limitations

- No word-level timing (see above).
- The phoneme column is empty in the desktop panel because the scene manifest does not
  carry phonemes. It is filled when tracks come from an `AudioTimeline` with mouth
  timing.
- No exposure (ones/twos) column yet; it depends on P01.
- The X-sheet reports timing structure and provenance. It does not measure perceptual
  mouth timing quality.
