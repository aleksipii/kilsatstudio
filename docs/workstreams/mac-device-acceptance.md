# Physical Mac device acceptance workstream

Status: **open**, 2026-10-18. Owner: `mac_acceptance`. This document prepares
device work; it records no new physical execution and closes no release gate.

## Retained evidence and current artifact

The user's 2026-10-16 report remains valid: Apple M1, macOS 27.1, Homebrew
Python 3.12, build `98d265b`, managed in-app setup and audible English CPU tests
with `af_heart` and `am_michael`. No WAV, JSON report or BUILD-INFO was attached.
Offline operation, device changes, full-project synchronization and Metal were
not established. See [the original record](../kokoro-in-app-acceptance.md#recorded-mac-evidence).

At the PR #38 checkpoint, the tracker identifies source
`fc18aac32c5bc9068885601b23db9d21e323df85`, preview run `39090819133`, and
[artifact 12122542757](https://github.com/aleksipii/kilsatstudio/actions/runs/39090819133/artifacts/12122542757).
Its recorded artifact SHA256 is
`81a68cc17af367e3c1a06237c4aa3f8b09b4ac1c833b273f37baa91247b9e01c`.
This is historical CI metadata, not a newly downloaded or rehashed artifact.
For a later build replace all three identifiers together and retain its
`Contents/Resources/BUILD-INFO.txt`. Distinguish the Actions artifact archive hash
from the contained preview ZIP hash; record each archive's filename and hash.
An expired 14-day artifact must be rebuilt and tested under its new identity.

| Evidence | What it establishes | What remains open |
| --- | --- | --- |
| CI arm64 build, architecture/dylib checks, packaged helper, ad-hoc signature verification | Artifact construction and tested package layout | Finder launch, icon, GUI and physical device behavior |
| User's earlier M1 speech report | Reported setup and two audible voice tests on that build | Offline repeatability, structured corpus, timing and current-build regression |
| Successful Metal helper report on a real Mac | Only the original offscreen fixture on its named adapter | Full scene parity, production GPU selection and performance |
| WAV structural checks or evidence-tool exit zero | Validated signal/evidence structure | Intelligibility, device playback and perceptual synchronization |

Ad-hoc signing is not Developer ID signing or notarization. The package contains
no model, voices, Python environment or inference dependencies. CI does not launch
the editor, synthesize with Kokoro or implicitly run the real-adapter test.

## Short device execution order

Use a native Apple Silicon Mac and a trusted, exact CI build. Store evidence
outside the repository and use new output paths for every run. Install permitted
assets separately with explicit download consent; never upload model/voice files.
For each observation record `pass`, `fail` or `not_tested`, expected versus actual
behavior, date, reviewer, build, macOS, chip, Python and named output device.

1. **Package and launch.** Retain the downloaded archives, their SHA256 values and
   BUILD-INFO. Open the app as described in [practical testing](../practical-testing.md#download-and-launch).
   Confirm editor/preview, Finder/Dock icon and recovery from invalid YAML. Test
   the app's Open/Save controls and a `.kilsat` round trip including authored
   animation. Record Finder double-click/Open With separately: LaunchServices
   declarations alone do not implement native file-open events. This is an open
   integration gap, not an expected passing test. Never disable Gatekeeper globally.
2. **Model-free transport.** Run **Load sound-check demo (synthetic tones)**,
   **Play scene audio**, pause/resume, stop and seek at boundaries. Expect tones,
   not Kokoro speech. Then open `examples/scene-sequence.yaml`, validate and use
   **Whole project · sequence → Prepare whole project**. Expect 1.500 seconds;
   global 24 kHz samples 12000/18000/24000 show red/equal red-blue/blue. Test
   reverse seeks, EOF restart, source invalidation and no overlapping transports.
   `Silent visual transport` must not count as audio-device success.
3. **Metal fixture.** Expand **Metal device check**, choose a new absolute
   directory with an existing parent and click **Run Metal device check**.
   Preserve `report.json` and `fixture.rgba`. Expected success: schema 1,
   `fixture=original-offscreen-v1`, Metal IntegratedGpu/DiscreteGpu, 256×256,
   `format=rgba8unorm-linear`, `row_padding_removed=true`,
   `interior_probes=passed-tolerance-2`, empty `validation_errors`, and
   `full_scene_parity=false`. Record Cancel/timeout separately, using new output
   directories; neither may claim success. Raw RGBA must be 262144 bytes.
4. **Offline real speech.** Use an already installed profile and Python 3.10–3.12.
   Disable external networking; do not enable installer downloads. Run a new,
   uncached sentence for both voices and two supported speeds. Listen through
   the named device; retain WAV and smoke JSON. Repeat unchanged requests to
   inspect synthesis-cache reuse, then change text/speed and confirm distinct
   output. Kokoro speech caching is distinct from estimated-mouth-result reuse.
   Test cancel/retry, restart/profile reload and output-device recovery. Repeat
   consent decline/install-cancel checks separately when testing setup itself.
5. **Scene and export synchronization.** Use a persistent owned two-scene
   dialogue fixture with overlap, prepared real speech and explicitly annotated
   or imported timing. Check pause/seek/restart before, within and after overlap
   and EOF. Export **PNG + WAV**, **SVG + WAV**, then **MP4 + WAV** into different
   new directories; MP4 needs an explicit installed FFmpeg path. Retain
   `sequence.json`, `audio.wav`, frames and `sequence.mp4`; compare equal samples
   and play MP4 on the same physical device. Record longer-run drift and
   perceptual mouth timing separately from structural duration correctness.
   Test export cancellation without damaging previous output. See the complete
   [sequence procedure](../practical-testing.md#test-4-whole-project-sequence-new-mac-acceptance).

## Existing commands and report expectations

Run from a source checkout matching BUILD-INFO, with the selected managed Python
environment active. Replace `/local/...` with actual absolute paths; the parent
of each output must exist and outputs must not already exist. This command uses
only installed assets and is an explicit real-inference opt-in:

```sh
python3 services/kokoro-runtime/smoke_macos.py \
  --allow-local-inference --config /local/config.json \
  --model /local/kokoro.pth --voice /local/af_heart.pt \
  --text 'Hello from Kilsat Studio.' --language en-us --speed 1.0 \
  --corpus-id short-01 --output /local/short-01.wav \
  --report /local/short-01.smoke.json
```

Exit zero means 24 kHz mono PCM16 non-silent WAV passed structural checks. Expect
`backend=Python/PyTorch CPU`, `provenance.kind=recorded_local_model`,
`synthesis=structural_checks_passed`, `alignment=not_aligned` and
`perceptual_validation=requires listening`. The report binds WAV SHA256, exact
text/language/speed, model/config/voice hashes, package versions and platform.
Repeat with `am_michael.pt` and new filenames. macOS/architecture fields are
unauthenticated environment declarations, not hardware attestation.

For a manually annotated **phoneme** reference and matching candidate, use the
existing [reference/review schemas](../speech-acceptance.md#reference-candidate-and-human-review):

```sh
python3 services/kokoro-runtime/acceptance.py \
  --wav /local/short-01.wav --smoke /local/short-01.smoke.json \
  --reference /local/reference.json --candidate /local/candidate.json \
  --review /local/review.json --output /local/short-01.acceptance.json
```

Exit zero means evidence validated and metrics were generated. Expected
`production_acceptance=not_granted` even when human observations say `pass`.
Use [viseme_acceptance.py](../viseme-quality.md) for Rhubarb mouth shapes instead;
do not relabel shapes as phonemes. No real recognizer or timing corpus has been
executed by this workstream.

For a source-level Metal check, the actual existing example/test commands are:

```sh
cargo run --locked -p motion-render --features gpu --example gpu_probe -- \
  --metal /local/new-metal-output
KILSAT_GPU_BACKEND=metal cargo test --locked -p motion-render --features gpu \
  gpu::tests::real_fixture -- --ignored --exact
```

The ignored test renders twice but does not replace the fixture report archive.
Hash retained outputs with the standard macOS utility, for example from the
Metal output directory:

```sh
shasum -a 256 report.json fixture.rgba
```

Also retain hashes for archives, BUILD-INFO, WAVs, smoke/review/reference/candidate
JSON, acceptance reports and export manifest/media. Review reports for private
local paths before sharing. Never substitute a checksum for a human observation.

## Missing evidence and next bounded improvement

- [ ] Current-build physical Mac Finder/UI/project round-trip observations.
- [ ] Actual Metal fixture files, adapter identity, errors/cancellation evidence.
- [ ] Offline uncached CPU WAVs and full smoke reports for both voices/speeds.
- [ ] Human listening/device recovery notes tied to the exact WAV hashes.
- [ ] Real two-scene export/playback, timing corpus and perceptual drift review.
- [ ] Separate full-GPU, production signing/notarization and third-party license gates.

This Linux development environment cannot execute Finder, CoreAudio playback,
native Apple Silicon UI or Metal, and has not loaded Kokoro assets. Only command
definitions/source and `smoke_macos.py --help` / `acceptance.py --help` were checked
for this document; no device/model commands above were run.

The next bounded automatable improvement is an offline evidence-bundle validator:
validate BUILD-INFO/source identity, bounded file inventories and SHA256 links
between existing Metal/speech/export reports; flag missing observations without
manufacturing `pass` or changing `production_acceptance`. It should never run
inference, acquire a GPU, download assets or turn CI packaging into device
approval. This proposal is not implemented by this document.
