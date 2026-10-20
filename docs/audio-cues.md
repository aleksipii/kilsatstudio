# Audio cues

Non-speech sound effects and stings are authored in YAML under a scene's
`composition.audio_cues`. There is no native editor control yet; YAML is the
interface. Cues are mixed with dialogue by the same `motion_audio::mix_timeline`
call on the same 24 kHz scene sample clock, so preview, scene export and sequence
export read one buffer.

```yaml
scenes:
  - id: intro
    duration: 8
    composition:
      audio_cues:
        - { id: door, asset: audio/door.wav, at: 1.5, gain: 0.8 }
        - { id: sting, asset: audio/sting.wav, at: 6, trim_start: 0.25, duration: 1.5 }
```

| Field | Meaning |
| --- | --- |
| `id` | Unique in the scene, 1..64 of letters, digits, `_`, `-` |
| `asset` | Project-relative `.wav` path (no absolute path, `..` or backslash) |
| `at` | Scene seconds, `0 <= at < scene duration`; rounded to the nearest sample like dialogue |
| `gain` | Linear, finite, `0..4`, default 1 |
| `trim_start` | Source seconds skipped (rounded to samples), default 0 |
| `duration` | Playback seconds; must end within the scene. Omitted plays the rest of the source |

## Diagnostics

- Syntax: unknown fields or missing `id`/`asset`/`at` (`MotionScript syntax`).
- Invalid reference/values: unsafe paths, non-finite or out-of-range times and gain,
  duplicate ids, more than 64 cues per scene (`MotionScript validation`). At
  preparation time a missing file, a path (or symlink) resolving outside the project
  root, a source that is not nonempty 24 kHz mono PCM16 WAV, a `trim_start` past the
  source, or a cue that rounds to the scene's end sample is rejected with the scene,
  cue id and asset in the message.
- Unsupported: asset extensions other than `.wav` and an unknown event type such as
  `type: sound` (`MotionScript unsupported`).

## Behaviour and limits

- Cues longer than the scene are truncated at the scene's last sample (dialogue
  overflow remains an error). The manifest `cues` list records the effective start,
  trim and duration frames.
- Overlaps sum in f32 with no limiter, like dialogue; only WAV writing clips.
  Dialogue is summed first, then cues in script order.
- Decoded source audio is capped at 300 s (24 kHz) per scene, checked from the WAV
  header before decoding, and each scene has at most 64 cues. Decoding runs in
  `prepare_scene*`, which callers already run off the UI thread.
- The result is a pure function of the script and assets. Seeking and pausing read
  absolute sample indices of the prepared buffer, so there is no cue state to drift.
- Sequence crossfades weight the whole scene buffer, cues included.

## Not covered

No resampling or format conversion, MP3/AAC, per-cue fades or envelopes, looping,
a native cue editor or cue waveform display, and no perceptual loudness
management. Tests are headless sample-level checks (`crates/motion-voice/tests/cues.rs`);
no audio device or GUI was exercised.
