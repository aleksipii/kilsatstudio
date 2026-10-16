# GPU, Mac/Kokoro acceptance and redistribution workstream

Status: open gates, inspected 2026-10-16. This document records implementation
boundaries and next evidence; it does not certify hardware or legal compliance.
The current development environment is Linux and cannot physically certify a
Mac, Metal adapter, Mac audio device, or real Kokoro installation/playback.
No model installation, asset downloads or inference were performed for this review.

## Current rendering architecture

- `motion-scene` evaluates scene state; `motion-render` emits deterministic SVG.
- `motion-export/src/media.rs` rasterizes that SVG through resvg 0.45.1 and
  tiny-skia on the CPU, then invokes an explicitly selected local FFmpeg for MP4.
- Desktop `draw_scene` and `art_editor::paint` submit egui painter primitives;
  desktop eframe 0.31.1 enables `glow`, not a verified Metal offscreen renderer.
- A GPU-backed application window does not establish GPU scene export, identical
  preview/export composition, or a verified Metal path. These remain open.

## Smallest reviewable GPU implementation

- [ ] Write an architecture decision selecting the GPU API/backend and preserving
  the evaluated scene as input. Define color space, premultiplied alpha, texture
  lifetime, clipping, blend order and bounded readback ownership explicitly.
- [ ] Add an opt-in offscreen backend for one fixture: background, textured quad,
  transformed actor primitive and clipped translucent foreground. Keep CPU export
  available with a visible backend selection and actionable initialization errors.
- [ ] Render into an offscreen RGBA texture, copy with row-padding accounted for,
  and produce a PNG from readback. Do not label a GPU upload of an already CPU
  rasterized final frame as GPU scene rendering.
- [ ] Compare decoded pixels against the CPU fixture using a documented tolerance
  for antialiasing/color differences. Check transparent edges, clip boundaries,
  plane order and the exact same scene/sample time; byte-identical PNG is optional.
- [ ] Add bounded allocation, oversized-frame rejection, cancellation, resource
  cleanup and repeated-render tests. Exercise unavailable-adapter and device-loss
  behavior without silently claiming that a CPU fallback passed the GPU test.
- [ ] On a real Mac, explicitly select/verify Metal and record adapter name, backend,
  macOS version, CPU/GPU, commit, dimensions and readback result. Reject software
  adapters for the hardware proof. Retain a validation-layer/error-scope report.
- [ ] Measure warm/cold wall time, GPU duration where supported, memory and readback
  separately from encoding; compare the same scene against CPU. A speedup is not
  assumed, and a single fixture is not full-scene feature parity.
- [ ] Only after parity coverage, route shared preview/export composition through
  the selected backend and extend tests to masks, blending, rigging and transitions.

## Mac/Kokoro device acceptance

Use `docs/kokoro-in-app-acceptance.md` for detailed scenarios. Keep these gates
unchecked until evidence from the actual packaged build and machine is retained.

- [ ] Record commit, artifact digest, native arm64 architecture, macOS, exact Python
  3.10–3.12 interpreter, manifest revision, dependency freeze and output device.
- [ ] Launch the downloaded app, inspect setup wording, decline consent and verify
  no installation starts. Consent and install into a new directory; test cancel,
  retry and responsive UI. Inspect descendant processes after cancellation.
- [ ] Restart the app and connect the final profile without relocation or automatic
  downloads. Run prerequisite diagnostics; keep this distinct from synthesis.
- [ ] Deny external networking at the OS/network boundary after provisioning.
  Run real CPU synthesis with both `af_heart` and `am_michael`, two speeds, and
  fresh cache keys. Retain request/settings, WAV hashes and machine-readable report.
- [ ] Verify finite, non-silent audio and personally listen through the selected Mac
  output. Record intelligibility, clipping/truncation and playback-device errors.
- [ ] Exercise stop, cancellation, seek, pause/restart, device reconnect and cache
  invalidation on changed voice/settings. Confirm retries and no stale test audio.
- [ ] Prepare real project dialogue, play scene boundaries and export audio/video;
  verify standalone tests preserve screenplay/history. Measure mouth alignment
  against actual speech separately: phoneme tokens are not measured timestamps.
- [ ] Record offline failure cases for missing models/resources and an unavailable
  output device. Python network guards alone are not OS-level offline evidence.
- [ ] Retain packaging/signing/notarization/Gatekeeper results separately from local
  development launch and from the Mac CI build. Hardware listening is still manual.

## Exact pins and license evidence currently recorded

Evidence below is local repository documentation and manifests, not a newly
completed artifact-level license audit. See `docs/kokoro-dependency-review.md` for
upstream links and limitations; `docs/kokoro-setup-sources.md` records exact sources.

| Item | Current pin/evidence | Remaining gate |
| --- | --- | --- |
| Kokoro Python | Managed manifest 0.9.4; prior upstream review records Apache-2.0 | Verify selected wheel hash, LICENSE/NOTICE and modifications |
| Misaki English | Managed manifest 0.9.4; prior review records Apache-2.0 | Inspect English extras, dictionaries and actual resolved artifacts |
| Torch | Managed manifest 2.6.0; prior review records BSD-style upstream terms | Select exact arm64/ABI wheel; inventory native libraries/notices |
| NumPy | Managed manifest 1.26.4; prior review records BSD-style upstream terms | Inspect wheel and bundled numerical-library licenses |
| spaCy | Managed manifest 3.7.5; prior review records MIT | Verify selected wheel and dependencies/notices |
| English pipeline | 3.7.1 at immutable revision `8fb6f8155286360bf5486fe399e2738d1df7ded2`; metadata records MIT | Preserve model/source acknowledgements, including WordNet; inspect actual wheel |
| Kokoro config/weights | Revision `f3ff3571791e39611d31c381e3a41a3af07b4987`; model repository reviewed as Apache-2.0 | Preserve revision-specific card/notices and determine their applicability |
| English voice tensors | Same revision; both files have exact SHA256/size in manifest | Separate voice provenance/redistribution review remains unresolved |
| Python interpreter | Host-supplied 3.10–3.12, not bundled; exact distribution/license unknown | Record selected interpreter and terms if distribution boundary changes |
| resvg | Rust direct dependency 0.45.1; license evidence not established here | Inspect crate and tiny-skia/usvg dependency closure and notices |
| eframe/glow, rodio | eframe 0.31.1 declaration, rodio 0.20.1 macOS; license evidence not established here | Inventory exact Cargo.lock artifacts, fonts, platform/native dependencies |
| serde, serde_json, thiserror | Workspace dependencies; exact resolution in Cargo.lock; license evidence not established here | Collect resolved versions, licenses and required notices |
| FFmpeg | Explicit user-selected executable, unbundled; exact build/license unknown | Capture build configuration, codecs and applicable licenses before bundling |
| eSpeak/phonemizer path | Transitive English extras; prior review identifies eSpeak NG GPLv3 | Identify actual loader, libraries/data and obligations before redistribution |

The manifest includes full SHA256/size for config, weights, both voices and the
English wheel. Its five direct Python pins are not a full transitive hash lock;
`pip freeze` records an installed resolution but does not authenticate all wheels.
The older `requirements.txt` contains broader development constraints and must not
be treated as the managed manifest or as a release lock.

## Dependency and notices completion

- [ ] Resolve one supported native arm64/Python environment without installing it
  here; retain exact wheel filenames, hashes, platform tags and full dependency graph.
- [ ] Inventory actual distributions, embedded libraries, fonts, dictionaries and
  model data; record license texts, copyright, NOTICE, source obligations and unknowns.
- [ ] Inspect voice provenance and phonemizer/eSpeak loading/distribution details;
  escalate unresolved rights/obligations to qualified review before bundling.
- [ ] Produce reviewed third-party notices/SBOM tied to the shipped artifact digest,
  retain source offers where applicable, and verify contents against the package.
- [ ] Keep distribution boundaries explicit: preview packages small scripts/manifest,
  not model weights, voices, English wheel, Python environment or FFmpeg. User-triggered
  provisioning does not by itself settle applicable redistribution obligations.

Update `docs/master-progress.md` only when each gate has corresponding evidence;
implementation, automated fixture coverage and actual device acceptance are separate.
