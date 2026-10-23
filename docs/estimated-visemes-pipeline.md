# Text-estimated visemes in the dialogue pipeline

Connects `motion_lipsync::english::estimate_viseme_track` (see
[english-lipsync.md](english-lipsync.md)) to scene preparation, so a dialogue line
with no alignment still drives the mouth variants of custom rigs
([custom-rigid-characters.md](custom-rigid-characters.md), acting layer) and legacy
mouths. The estimate comes from the line's text only. It does not hear the audio and
is never forced alignment. Perceptual quality is unmeasured.

## Precedence (highest first)

1. Manual alignment (`AlignmentSource::Manual`)
2. Forced alignment (`AlignmentSource::Forced`)
3. Rhubarb estimated cues (`estimated_alignment`, manifest tool `rhubarb`)
4. Text estimate (`DialogueClip::text_estimated`, manifest source
   `Estimated { tool: "kilsat-english-estimator", version }`)
5. Nothing: mouth at rest (only for lines with no speakable text or text over the
   token limit; a typed diagnostic says why)

`DialogueClip::mouth_source()` returns the winning `MouthSource`. A lower source is
never derived when a higher one exists; `derive_text_estimate` clears a stale one.

## Flow

`prepare_scene_common` (used by every scene/sequence preparation path) builds the
project's `Pronunciations` from `pronunciations:` and, per clip after alignment
resolution, calls `scene_audio::derive_text_mouths`. The track covers exactly
`audio.frames()` samples of the 24 kHz dialogue clock; the estimate is scaled into the
detected speech span (`timeline::speech_span`, RMS threshold max(-50 dBFS, peak - 35 dB))
with Rest over leading/trailing silence, or the whole clip when nothing passes the
threshold. See [lipsync-timing-evaluation.md](lipsync-timing-evaluation.md).

One lookup per consumer, one source of cues:

- Preview and the live scene: `DialogueClip::mouth_at(scene_frame)` adds the clip's
  `start_frame` and `trim_start_frame` exactly as for alignments, then reads the track.
- Export: `export::render_scene_with_cues` turns the same cues into
  `SceneManifest` viseme intervals (clipped by trim, shifted by `start_frame`);
  `motion-export` reads the manifest. A test compares both for every sample.

Lookups are stateless random access, so out-of-order seeks and scene boundaries behave
identically to existing alignments.

## Cache identity

Text estimates are cheap and deterministic, so they are recomputed rather than stored
on disk; `TextEstimate::identity` is `english::estimate_identity_in_span(text, overrides,
audio_frames, speech_start, speech_end)` (equal to `estimate_identity` for a whole-clip
span), a SHA-256 over the text, every pronunciation override (sorted), the
sample rate, the audio length, and `lexicon_version()` (hash of the embedded CMUdict
plus the estimator name/version). `derive_text_estimate` keeps an existing estimate
only if the identity still matches. Rhubarb cues keep their own cache
(`estimated_cache.rs`) and are not affected. TTS audio caching is unchanged:
pronunciation overrides do not change Kokoro input here.

## Diagnostics

`PreparedScene::diagnostics: Vec<PreparationDiagnostic>` (typed, non-fatal):

- `LowConfidencePronunciation { dialogue_id, actor, words }` for fallback-rule words
- `PoorTextEstimateFit { .., time_scale }` when audio length / model length is outside
  0.4..=2.5 (heuristic, not a quality measure)
- `TextEstimateUnavailable { .., reason }` (no speakable text, too long)

`message()` renders text. The desktop scene preview lists them under the alignment
statuses; the single-line preview applies the same derivation. The sequence panel does
not list them yet.

## Tests

`crates/motion-voice/tests/text_visemes.rs` (precedence, exact samples, scaling,
offset/trim, out-of-order seeks, identity and override invalidation, diagnostics,
manifest versus `mouth_at`), and in `crates/motion-export/src/rig_acting_tests.rs` an
end-to-end pixel test on `examples/acting-rig.yaml` showing the open-mouth variant
appearing at the expected frames, plus a no-estimate/manual-alignment unchanged test.
WAV fixtures are generated in code (`*.wav` is gitignored). No Kokoro run, no audio
device and no GUI were used.
