# Audio architecture and acceptance status

## Backend decision

Choose the upstream Python/PyTorch CPU API first: explicit local KModel config
and weights, existing KPipeline model and local float32 voice tensors avoid
conversion uncertainty and keep a native replacement behind SpeechProvider.
The process is one synthesis per invocation, so startup is expensive. A future
persistent service can reuse the model behind the same trait.

Alternatives investigated:

| Backend | Evidence | Decision |
| --- | --- | --- |
| Upstream PyTorch CPU | KModel accepts local config/checkpoint; KPipeline accepts model/tensor voice | Implemented, actual inference untested |
| ONNX Runtime | kokoro-onnx documents M1 support, CPU inference and conversion assets | Candidate replacement; conversion, G2P parity and Rust runtime unverified |
| Core ML / Metal / MPS | Requires independent export/provider validation and numerical/audio comparison | Deferred; no acceleration claim |

Sources reviewed 2026-10-15:
- https://github.com/hexgrad/kokoro/blob/main/kokoro/model.py
- https://github.com/hexgrad/kokoro/blob/main/kokoro/pipeline.py
- https://github.com/thewh1teagle/kokoro-onnx
- https://huggingface.co/hexgrad/Kokoro-82M
- https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md

## Data flow and synchronization

Script validation produces dialogue jobs with scene, actor and scene-relative
start seconds. The worker provides started/completed/error updates. A bounded
queue rejects extra submissions without blocking the UI. A disconnected UI stops
future work; dropping the worker does not cancel an in-flight process, which is
bounded by the provider timeout. Provider panics are caught at the worker boundary and reported as failed jobs.

Provider cache identity contains text, voice, language, speed, style, immutable
model/environment version, local model/config/voice and runtime-script hashes.
Cache WAV metadata and full PCM16 decoding are checked. File contents are hashed
each request (correctness over startup performance). Installers must provide an
honest environment digest; changing installed dependencies without updating that
digest is unsupported. Force regeneration can use an empty/new cache directory.
No asset downloader or consent bypass is provided.

Prepared clips convert seconds to nearest 24kHz sample frame. Preview adapters
and export use that same clock and clip offset; lipsync queries subtract the
clip start and add the same source trim used by mixing. Dialogue duration comes
from decoded audio. Waveform min/max bins preserve transients. Mixing supports
overlap and gain, clips only at PCM export, and requires explicit prior sample
rate/channel conversion. Sample-based contracts are implemented; no physical
audio device, desktop transport, rig mouth renderer or video encoder exists yet.
Those adapters must use the audio device clock, reset/flush on seek and pause,
and apply the same evaluated clip data during frame export. No independent
wall-clock animation timer should accumulate drift.

## Alignment decision and quality gate

Kokoro output phoneme strings have no certified timestamps. Uniformly spacing
phonemes or deriving word timing from text is not implemented or presented as
accurate lip sync. `Alignment` accepts human-authored/corrected IPA intervals or
external forced alignment with tool/version provenance. Structural validation
rejects overlap, unknown phonemes, nonfinite times and intervals past duration.
Missing alignment produces a resting mouth.

Montreal Forced Aligner is a candidate for offline forced alignment of a known
transcript and generated WAV; its acoustic models/dictionaries require separate
installation, consent and license checks. See
https://montreal-forced-aligner.readthedocs.io/en/latest/ . It has not been installed
or benchmarked here. Importing its TextGrid/ARPAbet output requires a future
adapter and explicit inventory conversion. Energy envelopes can drive a rough
open/closed mouth but cannot validate phoneme timing; not used as a substitute.

Before declaring timing validated, generate an English test corpus with both
presets, speeds 0.75/1/1.25, pauses, punctuation, names and short plosives. Have a
reviewer annotate at least 50 visible phoneme boundaries against the waveform.
Measure median and 95th-percentile boundary error against those annotations;
set and document acceptable thresholds for the intended frame rate, then inspect
real-time and exported mouth animation at 24/30/60fps. No perceptual measurements
were possible in this environment. Unit tests verify only clock and mapping rules.

## Licensing and installation gate

The upstream model repository declares Apache-2.0 for its weights; retain exact
revision LICENSE and notices when distributing. The voice directory and VOICES
metadata were inspected; this does not establish an independent license audit
for each selected asset's provenance or every redistribution scenario. Record
hash, revision, license text and notices per af_heart/am_michael asset before
shipping. Upstream inference code identifies Apache-2.0, kokoro-onnx identifies
MIT; the latter is not bundled here. Audit the actual installed Kokoro, PyTorch,
Misaki, spaCy, language package and eSpeak NG dependency tree. eSpeak NG has GPL
obligations that need distribution review. No model, voice or third-party code
is redistributed in this bundle. Installation instructions are an explicit
user-managed path; the application never starts a download.

## Acceptance checklist

| Requirement | Status |
| --- | --- |
| MotionScript → local synthesis request | Implemented; Rust checks/tests passed on Linux |
| Kokoro English WAV on macOS CPU | Backend authored; not executed |
| Individual cached audio asset | Implemented; cache tests passed on Linux |
| Correct scene placement / mixing | Implemented; sample-placement tests passed on Linux |
| Validated mouth timing | Structural/manual alignment only; perceptual quality pending |
| Playback / UI preview selection / physical device sync | Adapter and UI pending |
| Waveform preview | Peak extraction only; UI pending |
| Synchronized video export | WAV mix and mouth clock available; renderer/encoder pending |
| Clear failures / off-UI work | Structured protocol, bounded worker and timeout implemented |
| Apple Silicon / acceleration | Not tested; no acceleration enabled |
| Redistribution audit / dependency lock | Cargo.lock generated; Python full lock and audit pending |

Milestone 1 is a source-level foundation. Milestones 2–5 are not accepted as
complete; no generated speech, validated automatic alignment or desktop playback
is claimed. This is appropriate for the currently empty repository, with explicit
interfaces available for the other engineering workstreams.
