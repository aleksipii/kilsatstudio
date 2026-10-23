# Real-speech night-room reference on CI (opt-in)

The normal CI (`.github/workflows/audio.yml`) renders the night-room reference
MP4 with generated placeholder tones. `.github/workflows/real-speech-reference.yml`
is a separate workflow that renders the same sequence with real Kokoro speech.
It is **manual only** (`workflow_dispatch`); it never runs on push, pull request
or a schedule.

## Consent

Starting the workflow with `consent_download` set to `true` is the repository
owner's explicit consent to the downloads below, on a GitHub-hosted runner. With
`consent_download` false (the default) the first step fails and nothing is
downloaded. Only someone with write access can dispatch a workflow.

```sh
gh workflow run real-speech-reference.yml -f consent_download=true
# optional: -f dad_voice=am_michael -f boy_voice=af_heart -f language=en-us -f speed=1.0
```

Or use **Actions → Real speech reference (opt-in) → Run workflow**.

## What it downloads

All on the runner, by the existing consent-gated installer
`services/kokoro-runtime/setup_kokoro.py --allow-downloads`, which checks every
file's size and SHA-256 and deletes the install on any failure:

| Item | Pin |
| --- | --- |
| `hexgrad/Kokoro-82M` `config.json`, `kokoro-v1_0.pth` (327,212,226 bytes) | revision `f3ff3571791e39611d31c381e3a41a3af07b4987`, hashes in `services/kokoro-runtime/setup-manifest.json` |
| Voice tensors (`af_heart`, `am_michael`, plus the selected voices; about 0.5 MB each) | same revision; hashes of all 28 English voices in `services/kokoro-runtime/english-voices.json` (from the Hub tree API's Git LFS SHA-256, 2026-10-21) |
| spaCy `en_core_web_sm` 3.7.1 wheel | hash-pinned in the setup manifest |
| `kokoro==0.9.4`, `misaki[en]==0.9.4`, `torch==2.6.0`, `numpy==1.26.4`, `spacy==3.7.5` and their transitive wheels | exact direct pins; transitive wheels resolved by pip (captured in `freeze.txt`), wheels only |
| FFmpeg | Homebrew, used only to encode the MP4 |

No cloud speech API is used. Synthesis is local CPU inference through
`services/kokoro-runtime/runtime.py` (protocol 1, network guarded inside Python),
called by the existing `scripts/night_room_real_speech.py`; no new synthesis code
path exists.

## Steps

1. `scripts/real_speech_ci.py manifest` derives a setup manifest from the
   checked-in manifest plus the selected voices, rejecting unknown, unpinned or
   wrong-language voices, and prints its content hash.
2. `actions/cache` restores the Kokoro environment and assets, keyed by runner
   OS/architecture, exact Python version and that content hash (so a different
   revision, voice or hash is a different key). On a miss `setup_kokoro.py`
   installs; the cache is saved only if the whole job succeeds.
3. `real_speech_ci.py verify` re-hashes every model/config/voice file, whether
   freshly downloaded or restored from cache.
4. `night_room_real_speech.py` synthesizes the three reference lines.
5. `night_room_reference report|frames|export <dir> <wav-dir>` builds the
   timing report, per-shot frames with `contact-sheet.png`, and `sequence.mp4`
   from the real WAVs. With a WAV directory the example has no tone fallback: a
   missing WAV is a hard error.
6. `real_speech_ci.py provenance` writes `provenance.json`.

Any failed download, hash check, install, synthesis or render fails the job.
There is no fallback to tones and no partial artifact.

## Artifact (`night-room-real-speech-<run id>`, 30 days)

`sequence.mp4`, `wav/audio/*.wav`, `frames/*.png` and `frames/contact-sheet.png`,
`timing.txt`, `speech-report.json` (per line: voice, text, seconds, wall time,
real-time factor, raw phonemes), `setup-manifest.json`, `setup-log.jsonl` (fresh
installs only), `freeze.txt` and `provenance.json`. Provenance records the
consenting actor and event, commit and run, runner OS/architecture/image/macOS
version, model revision, re-verified hashes of every model asset, whether the cache
was hit, voice selection with upstream traits, language and speed, package pins
and installed freeze, Python/rustc/FFmpeg versions, step timings, per-line and
total real-time factor (wall seconds per audio second, including per-line process
start and model load), SHA-256 of every output file and the licence record below.
Model, voice and wheel files are never uploaded or committed.

## Licence record

From the model card at the pinned revision (README.md SHA-256
`91dcabced89db6f109b8786642f50402d3ee87450e8189589b6f85520e7f4d78`): licence tag
**apache-2.0**. The card's Creative Commons Attribution section lists CC BY
training audio for v1.0 (Koniwa `tnc`, CC BY 3.0; SIWIS, CC BY 4.0). VOICES.md at
the same revision (SHA-256
`ec7e4941ad7e194af61e3455928528a9ff5360c7c505e412efab27d6a69ea106`) has no
voice-specific licence; voice tensors are distributed in the same apache-2.0
repository, and CC BY source attribution is listed only for some non-English
voices, none of the English ones. This is recorded upstream metadata, not a legal
review or a redistribution approval; the transitive Python packages and the
generated WAVs/MP4 are not licence-audited by this workflow.

## Voices

Defaults match the current reference: DAD `am_michael`, STAN `af_heart`.
`af_heart` is a female preset; upstream describes voices only as female or male
and none at this revision as a child or boy voice. Choosing a boy voice remains a
listening decision (see [night-room voice selection](night-room-voice-selection.md)).
`language=en-gb` requires British (`b…`) presets for both characters.

## Status and limits

The workflow YAML passed `actionlint` 1.7.7 locally (without shellcheck), and the
helper has unit tests (`scripts/tests/test_real_speech_ci.py`). Before merge the
helper's `manifest`, `verify` and `provenance` steps, the synthesis script and the
example `report` were run by hand on one Apple M1 against an existing local
Kokoro install (no download). The workflow itself, including the runner install,
cache behaviour, Homebrew FFmpeg and MP4 export, had not run on GitHub when this
was written. Its outputs are structural: mouth timing is estimated from text, not
aligned to the audio, and no listening or perceptual review is performed.
