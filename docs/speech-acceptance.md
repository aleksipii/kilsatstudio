# Offline speech evidence and human acceptance

- [x] Opt-in local-model macOS smoke runner and immutable output evidence.
- [x] Corpus text, language, speed and identifier recorded with smoke evidence.
- [x] Offline WAV hash/measurement checks and manual-reference boundary comparison.
- [x] Explicit human listening, device playback and animation synchronization declarations.
- [ ] Run real Kokoro corpus on an Apple Silicon Mac with installed assets.
- [ ] Listen to recordings, test the physical output device and inspect animation synchronization.
- [ ] Establish reviewed quality targets and assess measured corpus results.

These are separate gates. Unit tests use synthetic PCM and mocked inference; they
establish evidence handling only. Nothing in this tooling automatically approves
production quality. No model is downloaded by either command.

## Record a corpus sample

Install authorized model, voice and language assets separately. On macOS, use a
new WAV/report pair for each sample. The runner requires explicit inference opt-in:

```sh
python3 services/kokoro-runtime/smoke_macos.py \
  --allow-local-inference --config /local/config.json \
  --model /local/kokoro.pth --voice /local/af_heart.pt \
  --text 'Hello from Kilsat Studio.' --language en-us --speed 1.0 \
  --corpus-id short-01 --output /local/short-01.wav --report /local/short-01.smoke.json
```

Choose short/long speech, pauses, punctuation, numbers and multiple voices; retain
separate evidence per sample. `en-us` and `en-gb` are supported corpus languages;
speed is bounded to 0.5–2.0. Smoke reports record model/config/voice SHA-256,
package versions, platform, architecture, backend, exact text and signal measures.
The provenance kind is `recorded_local_model`. Test/stub evidence must identify
itself as `synthetic` or `stub`; a Darwin string alone is not Mac verification.
Provenance remains an unauthenticated declaration, not a hardware attestation.

## Reference, candidate and human review

Create a manual phoneme reference by inspecting the exact recording. Both files
use seconds relative to WAV start, ordered nonoverlapping intervals, the exact
same ordered phoneme labels, and the WAV SHA-256:

```json
{
  "schema_version": 1,
  "audio_sha256": "REPLACE_WITH_WAV_SHA256",
  "provenance": "manual",
  "intervals": [{"phoneme": "h", "start": 0.10, "end": 0.18}]
}
```

Candidate provenance is `manual`, `estimated`, `forced_aligned` or `synthetic`.
Reference provenance must be `manual`. This tool compares an existing alignment;
it does not generate a forced alignment or claim Kokoro emits timestamps.

The reviewer supplies a separate review file after actual observation. Leave each
unperformed observation as `not_tested`; never mark a fixture as listened speech:

```json
{
  "schema_version": 1,
  "audio_sha256": "REPLACE_WITH_WAV_SHA256",
  "reviewer": "Reviewer name",
  "reviewed_at": "2026-10-15T22:00:00+03:00",
  "device": "Actual Mac and output device, or explicitly not tested",
  "listening": "not_tested",
  "device_playback": "not_tested",
  "animation_sync": "not_tested",
  "notes": "Describe procedure, observed defects and unperformed checks"
}
```

Observation values are `pass`, `fail` or `not_tested`. Record seek, pause and
restart behavior in synchronization notes. Reviews are human declarations bound
to a recording; they are not machine-verifiable proof of listening.

```sh
python3 services/kokoro-runtime/acceptance.py \
  --wav /local/short-01.wav --smoke /local/short-01.smoke.json \
  --reference /local/reference.json --candidate /local/candidate.json \
  --review /local/review.json --output /local/short-01.acceptance.json
```

The offline command rehashes the WAV, recomputes every PCM measurement and checks
it matches smoke evidence exactly. It binds reference, candidate and review to
that WAV, hashes all JSON inputs and exclusively creates a new report. It reports
mean, maximum and nearest-rank p95 absolute start/end boundary errors in
milliseconds. A small corpus or inaccurate manual reference limits the conclusion.
No quality threshold is silently applied; `production_acceptance` always remains
`not_granted`, including when all human declarations say `pass`.
