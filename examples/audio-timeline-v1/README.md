# AudioTimeline v1 examples

Format: `motion_lipsync::audio_timeline::AudioTimeline`, documented in
[docs/audio-timeline.md](../../docs/audio-timeline.md). Checked by
`crates/motion-lipsync/tests/audio_timeline_examples.rs`.

| File | Valid | Shows |
| --- | --- | --- |
| `fi-manual.audio-timeline.json` | yes | Finnish line "Hyvää yötä." with hand-authored (`manual`) IPA phonemes `y`, `ø`, `ʋ`, long `æː` and their visemes |
| `en-estimated.audio-timeline.json` | yes | English line with rule-estimated visemes, a trimmed source and gain |
| `invalid-overlapping-actor.audio-timeline.json` | no | Two clips of one actor overlap |
| `invalid-trim-exceeds-source.audio-timeline.json` | no | Trim plus duration run past the source audio |
| `invalid-future-version.audio-timeline.json` | no | `format_version: 2` is not supported |
| `invalid-unknown-phoneme.audio-timeline.json` | no | IPA `ɯ` has no mapped viseme |

The WAV files the examples name are **not included**, and the `pcm_sha256` values are
placeholders: these files show the data format only. The timings were written by hand
for illustration; they were not measured against recorded speech, and no model
produced any audio or timing here.
