# AudioTimeline v1 and the dialogue sample clock (A07)

Contract owners (ORK-G1-04): the clock is `motion-audio` (`src/clock.rs`), the
timeline is `motion-lipsync` (`src/audio_timeline.rs`). `motion-voice` builds a
timeline from a prepared scene (`src/audio_timeline.rs`) and provides the strict
model-free preparation (`src/model_free.rs`). `motion-lipsync` does not depend on
`motion-tts`: synthesis provenance is stored as data.

## Sample clock

`motion_audio::clock::DIALOGUE_SAMPLE_RATE` = 24 000 Hz is the one dialogue,
sequence and export rate. The conversions implement the rules A04 recorded in
[editable-timeline.md § Time conversion](editable-timeline.md):

| Function | Rule |
| --- | --- |
| `seconds_to_sample` | nearest sample, halves away from zero; negative, non-finite or out of range → `ClockError::InvalidTime` |
| `sample_to_seconds` | `sample / 24000` |
| `frame_to_sample` | `floor(frame × 24000 / fps)` in integer arithmetic; `fps = 0` → `InvalidFrameRate` |
| `frame_to_seconds` | `frame_to_sample / 24000`; bit-identical to `frame / fps` when fps divides 24000 |
| `sample_to_frame` | the last frame whose first sample is not after the sample |

Copies moved onto it in A07's crates: `motion_lipsync::english::DIALOGUE_SAMPLE_RATE`
and `motion_voice::export::SAMPLE_RATE` are now aliases, and dialogue placement in
`DialogueClip` uses `seconds_to_sample`. Still separate (owned by other agents):
`motion_scene::SEQUENCE_SAMPLE_RATE` / `motion_scene::time` (A04),
`motion_script::auto_direction::SAMPLE_RATE` (A02) and `motion-export`'s test support
(A10). They have the same value and rules; delegating to this module is their owners'
change.

## Format

JSON, strict (`deny_unknown_fields`), `format_version: 1`, `sample_rate: 24000`.

```text
AudioTimeline { format_version, scene_id, sample_rate, duration_samples, clips[] }
AudioTimelineClip {
  id               stable dialogue ID (motion_voice::dialogue_jobs order)
  actor, language  e.g. "AINO", "fi-fi"
  source           imported { path, pcm_sha256 }
                 | synthesized { provider, model_revision, voice, voice_revision?, cache_key, pcm_sha256 }
  source_samples   untrimmed source length
  start_sample     scene sample where playback starts
  trim_start_samples, duration_samples, gain
  mouth?           { source: AlignmentSource, phonemes?: [{phoneme, start_sample, end_sample}],
                     visemes: [{start_sample, end_sample, viseme}] }
}
```

- Placement equals `motion_audio::MixClip`: source samples
  `[trim, trim + duration)` play from `start_sample`. `AudioTimelineClip::mix_clip`
  produces the mixer input and refuses PCM of another rate, channel count or length.
- Mouth intervals are half-open and **source-local** (sample 0 = first sample of the
  untrimmed source), so a trim never moves the mouth against the audio. Gaps and
  everything outside the playing window are `Rest`.
- `mouth.source` is `manual`, `forced {tool, version}` or `estimated {tool, version}`
  and is kept verbatim through save, reopen and export. A hand-corrected timing is
  `manual`. None of the sources means the timing was validated perceptually.
- `pcm_sha256` is `motion_lipsync::rhubarb::pcm_identity` of the decoded PCM, the
  same identity the alignment cache uses.

Validation (`AudioTimelineError`, typed): version, sample rate, labels, at most 4096
clips and 65 536 intervals per clip, unique IDs, trim/duration within the source,
clip end within the scene, finite gain, provenance fields present, intervals ordered,
non-empty and within the source, IPA phonemes known (see
[english-lipsync.md § IPA coverage](english-lipsync.md#ipa-coverage)), and no two
clips of one actor overlapping (touching is allowed).

Examples, valid and invalid: [examples/audio-timeline-v1](../examples/audio-timeline-v1/).

## Viseme blending

`AudioTimeline::blend_at(actor, sample, transition_samples)` returns
`VisemeBlend { from, to, weight }` (`weight` 0 = `from`, 1 = `to`). Each change of
shape at boundary `b` crossfades linearly over `[b − h, b + h)` with
`h = min(transition / 2, half of each neighbouring segment)`; equal neighbouring
cues are one segment. Weight at `b − h` is 0, at `b` 0.5, and the shape is pure from
`b + h`. The clip's own (trimmed) edges are hard cuts like its audio, and
`transition_samples = 0` is a hard cut everywhere. The function is pure: a seek gives
the same result as playback. It is available for A08's renderer; the current
renderer still shows `viseme_at` (no blending).

## Viseme set: 13 kept

Decision (A07, per ADR-A01-08 and ORK-G0): the 13-state `motion_lipsync::Viseme` stays
canonical and its serialized names do not change. The report's 10 letter-based states
relate to it as follows; no 10-state projection is implemented because no consumer
needs one yet.

| Report (10) | Current (13) |
| --- | --- |
| 0 Neutral | `Rest` |
| 1 AEI | `A`, `E`, `I` |
| 2 BMP | `Closed` |
| 3 FV | `TeethLip` |
| 4 CDGKNRSTXYZ | `TongueUp`, `Sibilant`, `R`, `TongueTeeth`; k/g → `Rest` |
| 5 U | `U` |
| 6 ChJSh (+H) | `Sh`; h → `Rest` |
| 7 L | `TongueUp` |
| 8 O | `O` |
| 9 QW | `U` |

## Strict model-free path

`motion_project::ModelPolicy` (`LocalPreprocessing` default, `Strict`) with
`permit(ModelBackedStep)` is the single check point (ADR-A01-09). `Strict` refuses
`SpeechSynthesis`, `RecognizerAlignment`, `CameraFaceTracking` and `ModelDownload`
with a typed `PolicyViolation`. On the strict path:

1. `motion_voice::model_free::require_imported_audio` checks, before any audio is read
   or provider called, that every line names an existing WAV. Missing lines are
   reported together as `ModelFreeError::MissingImportedAudio` (no audio field, or file
   not found). There is no fallback.
2. `prepare_scene_model_free` then prepares with `motion_tts::DisabledSpeechProvider`,
   whose every call fails, so a missed check still cannot synthesize.
3. Mouth timing comes from manual/imported alignments or the rule-based English and
   Finnish estimates (`Estimated`).

Tested in `crates/motion-voice/tests/model_free.rs` with a provider whose every call
fails and is counted: preparation, AudioTimeline build, export of WAV + manifest +
timeline, project save and reopen (`motion_project::save_atomic`/`load`), reloading the
timeline and remixing from it all run with zero provider calls, and the remix equals the
exported mix sample for sample. A missing WAV is a typed error with zero calls.

Not done here (other owners): storing the policy in `ProjectDocument` and the UI/worker
check points (`scene_worker`, `sequence_worker`, `direction_worker`, Rhubarb,
`head_pose`, `kokoro_setup`) are A09's; `motion-export` calling the check before speech
preparation is A10's.

## Limits

- Structural validation only. None of the timings in tests or examples were measured
  against recorded speech, so mouth timing is not validated perceptually.
- The examples name WAV files that are not in the repository and use placeholder
  hashes; real Finnish and English recordings with reviewed timing are still needed.
