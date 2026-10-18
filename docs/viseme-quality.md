# Estimated mouth-shape quality evidence

`viseme_acceptance.py` compares an existing native Rhubarb result with manually
annotated mouth shapes on the exact recording. It runs offline and does not
execute a recognizer, load a model or download assets. This complements the
[phoneme boundary evaluator](speech-acceptance.md): Rhubarb shapes are not phonemes.

Keep the PCM16 WAV and smoke report from the real Mac corpus procedure. Export
the estimated JSON from the configured Rhubarb workflow and save the exact UTF-8
dialogue in `transcript.txt`, including any intended trailing newline. The
candidate must match both decoded PCM and transcript hashes. A same-duration
replacement recording or changed text is rejected.

Manually annotate the recording using the original `A B C D E F G H X` Rhubarb
shape alphabet. `X` is Rest; see the [shape mapping](rhubarb-estimated-cues.md).
The reference uses integer sample positions at 24 kHz, not milliseconds:

```json
{
  "schema_version": 1,
  "audio_sha256": "REPLACE_WITH_EXACT_WAV_SHA256",
  "provenance": "manual",
  "sample_rate": 24000,
  "frame_count": 24000,
  "cues": [{"start_sample": 2400, "end_sample": 9600, "shape": "D"}]
}
```

Set `frame_count` to the actual recording length. Cues must be ordered,
nonoverlapping and inside the recording. Intervals are half-open; uncovered
samples are Rest. Adjacent identical shapes are collapsed for boundary analysis.
Use the same human review schema as [speech acceptance](speech-acceptance.md),
bound to the WAV SHA-256. Unperformed listening, device playback and animation
observations must remain `not_tested`.

```sh
python3 services/kokoro-runtime/viseme_acceptance.py \
  --wav /local/short-01.wav --smoke /local/short-01.smoke.json \
  --reference /local/manual-shapes.json --candidate /local/estimated-cues.json \
  --transcript /local/transcript.txt --review /local/review.json \
  --output /local/short-01.viseme-evidence.json
```

The output path must be new. Input hashes refer to captured bytes, avoiding a
second file read after validation. Inputs are bounded regular files: JSON 4 MiB,
transcript 64 KiB, WAV 32 MiB and at most five minutes of mono 24 kHz PCM16. The
candidate preserves the existing Rust schema, recognizer and executable identity;
the executable hash is a declaration here, not proof of tool execution or rights.

The report contains:

- `shape_agreement`: matching samples divided by all recording samples. Long
  shared Rest intervals can inflate this metric.
- `active_shape_agreement`: matching non-Rest samples divided by samples where
  either annotation is non-Rest. It is `null` when both are entirely Rest.
- `confusion_samples`: reference shapes as rows and candidate shapes as columns,
  counting the exact sample overlap, including Rest.
- `boundary_error`: internal transition error in milliseconds only when the
  collapsed shape sequences match. Otherwise it is explicitly unavailable;
  there is no invented correspondence between different labels. Matching
  constant shapes have zero transitions, reported as such.

Inspect the mismatches against the audio and animation. Report corpus size,
voices, text types, annotation method and defects. No threshold is selected
automatically. `production_acceptance` always remains `not_granted`, even when
metrics are perfect and all review declarations say `pass`. Synthetic tests
verify arithmetic, identity and failure handling; actual Rhubarb execution,
representative recorded corpus results, perceptual quality and physical Mac
synchronization remain open until real evidence is reviewed.
