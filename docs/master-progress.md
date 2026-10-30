# Master prompt implementation and acceptance tracker

Source of requirements: [master build prompt](master-build-prompt.md). Updated
2026-10-30. A checked item identifies integrated, inspected implementation, not a
claim that every hardware or product acceptance test has passed. An unchecked
item includes work in progress until integration and relevant checks succeed.
The parent integrator owns completion marking and evidence. Never replace a
hardware, real-model or perceptual gate with mocked success.

## Art angles 2: camera angles front/reverse/high/low — PR #156 (2026-10-30)

Merged with the user's authorisation (merge commit `5ca9d29`, no force-push). See
[camera-angles.md](camera-angles.md).
- MotionScript camera keys take `angle`, held until the next key as a cut. Scene evaluation
  reports `camera.angle`; `reverse` mirrors actor roots and the camera x about the canvas
  centre and turns views around before rig evaluation.
- Art layers take an optional `angle`, and SVG, GPU and desktop renderers filter by the scene's
  angle. `Layer::mirrored_x` mirrors vector content and rejects images/raster masks/attachments.
  Project validation requires artwork drawn for every non-front angle a scene with artwork uses.
- Story beats take `angle`; sets list their angles and unsupported ones are reported. The importer
  mirrors vector sets and chairs for reverse scenes; all four bundled sets offer front and reverse.
- Desktop: angle pickers (camera keys, art layers); framing keeps the angle; reverse drags fixed.

Checks actually run locally (macOS, Apple Silicon), after merging `origin/main` (#155) into the
branch: fmt, `clippy --workspace --all-targets -D warnings` (0), `cargo test --workspace
--all-targets` 1277 passed / 0 failed / 13 ignored; earlier on the branch `--features gpu`
motion-render/motion-export 265 passed. A first CI run failed in `gpu-contracts` (a missing field
in the `gpu_scene_probe` example test that the local run had not compiled); fixed in `3a13849`.
All 15 CI checks on the final head were green before merging. Front and reverse frames of a café
story were rendered to PNG and inspected; a seated character showing a face in reverse was fixed.

Still open: no hand-run desktop review; mirroring flops lettering; draw order does not change
with the angle (chair back, hand props); lights and attached art are not mirrored; `high`/`low`
only select drawn art and no bundled set has any; library prop views not wired (motion-library is
unused by the desktop). Next: redrawn starter art (full characters with per-view drawings, sets
with drawn reverse/high/low layers) and per-view SVG/PNG import. `kilsatstudio1.0` not advanced.

## Art angles 1: three-quarter view and per-view part drawings — PR #153 (2026-10-30)

Merged with the user's authorisation (merge commit `835e459`, no force-push):
- `FaceView::ThreeQuarter`; MotionScript and story `face` accept `three_quarter_left` /
  `three_quarter_right` (mirrored like `left`). Desktop facing labels added.
- Rig parts take optional `views` (`three_quarter`, `side`, `back`) with variant fields,
  so characters can be drawn or textured separately per angle. Precedence: variant, view
  drawing, base. Drawn feature views skip the automatic face turn; missing drawings fall
  back to the front drawing (three-quarter) or the automatic turn.
- The built-in stick figure turns its face partway in three-quarter on every renderer
  (shared `procedural_face`). The model sheet turnaround adds "puolisivulta".

Checks actually run locally (macOS, Apple Silicon): fmt, check, clippy (0 warnings) and
`cargo test --workspace --no-fail-fast` 1231 passed / 0 failed / 13 ignored. Model sheet
reference hashes were updated after visually inspecting the rendered sheet. All 14 GitHub
CI checks on the PR head were green before merging.

Still open: no hand-run desktop review; bundled characters have no per-view drawings yet
(the stick body is unchanged in three-quarter, only the face turns); older app builds reject
rigs using `views`. Next: per-view art for props and sets with screenplay camera angles
(reverse/high/low), a redrawn starter set with full drawn characters alongside stick
figures, and per-view SVG/PNG import in the desktop app. `kilsatstudio1.0` was not advanced.

## PRO studio series P01–P08 — PRs #144–#151 (2026-10-30)

Merged with the user's authorisation by the PRO orchestrator, in waves (merge commits,
no force-push):
- **Wave 1:** **#145** (P01, `08df5c2`) easings, exposure on twos/threes and stepped blocking;
  **#146** (P04, `b29c21f`) continuity warnings for shot lists (180° rule, jump cuts,
  cut-on-action hints); **#147** (P02, `a98cbf6`) A–H/X mouth chart projected from the 13
  visemes, plus the X-sheet and its desktop panel; **#149** (P07, `8087a4c`) OpenTimelineIO
  timeline, ProRes 422 HQ/4444 and colour tags; **#144** (P06, `3fffe30`) prop states,
  hand-offs, set-downs and prop cards.
- **Wave 2:** **#151** (P03, `fb98d47`) animation principles as MotionScript settings;
  **#150** (P05, `06cd1a8`) multiplane parallax, aerial perspective and value-structure check.
- **Wave 3:** **#148** (P08, `1cd90a1`) model sheet, silhouette check and character style
  presets.

Orchestrator changes: #144 first failed CI because the new `PropError::States` was not
handled in `apps/studio-desktop/src/hand_props.rs`. A Finnish message was added (`0baa8fb`).
#144, #151, #150 and #148 then had `origin/main` merged in to resolve additive conflicts in
shared hot files (`motion-script/src/lib.rs`, `motion-scene/src/lib.rs`, `studio-desktop/src/main.rs`,
`motion-library/src/lib.rs` and its tests, `motion-render/src/lib.rs`), keeping both sides.

Checks actually run locally (macOS, Apple Silicon): all eight branches were merged together
on top of `c6ad82c` before any merge. On that combination, `cargo fmt --all --check`,
`cargo check --workspace`, `cargo clippy --workspace --all-targets` (0 warnings) and
`cargo test --workspace` passed. The same checks passed on each conflict-resolution commit
before it was pushed. The final #148 head (`4ba5107`), whose tree equals `main` `1cd90a1`,
gave 1226 passed / 0 failed / 13 ignored, using a clean target directory. One false
`motion-render` compile error came from a target directory shared between worktrees and
disappeared with a clean one. GitHub CI was green on every PR head before it was merged.

Still open: no graphical application run of the X-sheet or model sheet panels, or of the new
easing labels; no word-level timing in the X-sheet; continuity thresholds not reviewed against
real edits, and the desktop does not show the notes; the official `opentimelineio` reader not
run locally; EXR output needs an A01 dependency decision; the H.264 path is still untagged
BT.601; no art or perceptual review of parallax, principles or style presets. `kilsatstudio1.0`
has not been advanced. That needs the user's separate approval, as does any version or tag change.

## Screen-slot follow-ups and Kilsat Studio 1.0 — PRs #135–#139 (2026-10-29)

Merged with the user's authorisation (merge commits, no force-push):
- **#135** streaming export of decoded screen-video frames (`ArtContext.screen_videos`,
  `images_at`, cache-backed preflight) and the example episode `examples/screen-demo`
  ("The Live Demo", English, Kokoro `af_heart`/`am_michael`).
- **#136** release 1.0.0: `Kilsat Studio.app`, bundle id `studio.kilsat.studio`,
  `scripts/build_release_app.sh`, `macos-release-app.yml`, `docs/release-1.0.md`.
- **#137** (A09) desktop screen-cue editor, video import with ffprobe, bounded background
  proxy decode for the preview.
- **#138** (A05/A02) screen taps drive the free hand (two-bone IK, pure function of scene
  time, `tap_diagnostics`); `holds: {item, hand}` shorthand.
- **#139** (A08/A04) screen `brightness`/`glow`/`glow_radius` on SVG and GPU paths, editable
  in the desktop cue editor; KS-003d seek ≡ frame-by-frame test with screen cues.

Checks actually run locally (macOS, Apple M1): each branch merged with the then-current
main passed `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked`
(0 warnings) and `cargo test --workspace --locked` (#138: 1087 passed / 0 failed / 11 ignored;
#139: 1092 / 0 / 11). GitHub CI passed on #137 and #138 before merge. The example episode
was rendered locally at 1920×1080, 45.6 s, H.264 + AAC with local Kokoro speech. The 1.0
package was built, passed `package_audit.py` and `codesign --verify` (ad-hoc), and the app
process launched.

Still open: Developer ID signing and notarisation (no Developer ID identity available); hands-on
GUI use of the screen-cue editor; desktop full-size video export (use `production_export`);
glow does not light the holder; art/perceptual review; `feat/night-room-grading` (one old
lighting commit plus uncommitted edits, 200+ commits behind main) is **not** merged.

## Anchored asset library — `feat/asset-library` (2026-10-28, PR open, not merged)

Status: **review**, pull request open; not merged. Contract ORK-LIB-01.

Implemented: new crate `motion-library` (manifest schema v1 with typed validation,
licence/distribution gate incl. AI-generated and unknown licences, deterministic search,
SHA-256 content-addressed store rejecting corrupt objects, kit-bash assembler into
`motion_art` planes with world-space anchors, deterministic parametric room generator with
day/evening/rain/messy variants). Content in `examples/stick-studio/library/`: 7
backgrounds, 101 props (20 migrated, 81 new original art props), 18 locations, 18 ready
sets with camera positions and stand/sit marks; 182 anchors. CI workflow
`asset-library.yml` (macos-15). Docs: `docs/asset-library.md`.

Checks (local, macOS Apple M1): fmt ok, check ok, clippy 0 warnings, `cargo test
--workspace --locked` 1002 passed / 0 failed / 9 ignored, `validate_library --distribution`
exit 0. Every location × variant passes `motion_art::style::check_background`.

Open: no MotionScript reference to sets yet; parallax not rendered; 11 migrated props are in
thumbnail coordinates and `rig_project` props cannot be kit-bashed; no rendered art review.

## Phone and tablet screen slots — PR #130 merged (2026-10-28)

Status: **merged** to `main` (`2ddd16f`, merged by aleksipii 2026-10-28T09:09:46Z,
before the orchestrator review). Post-merge ORK review: **changes requested as
follow-ups** ([comment](https://github.com/aleksipii/kilsatstudio/pull/130#issuecomment-6467734702)).
The feature is not `done`: two CI jobs fail because of this PR.

Completed (implementation, inspected):
- `motion-rig` (A05): `RigDefinition.screens` named screen slots (glass part, parallelogram,
  overlay parts), `TextureFit` cover/contain/stretch on `EvaluatedTexture`, catalog slots
  `phone`/`tablet`, attach/detach copies or removes the slot, and duplicate slot names are refused.
- `motion-render` (A08): SVG and GPU texture paths share `EvaluatedTexture::placement`.
- `motion-script` (A02): `composition.screens` cues (off/image/sequence/video, fit, trim,
  speed, loop, taps), validation errors, `tap_times()`. The new fields are `serde(default)`,
  so older projects load unchanged.
- `motion-scene` (A04): cues are applied as a pure function of scene time on the same joint
  transforms.
- `motion-export`/`motion-project` (A10): asset preflight, `screen_video` FFmpeg PNG frame
  cache (content-addressed, bounded, cancellable, timeout, atomic rename), P3→BT.709 filter,
  HDR rejected; CI workflow `screen-video.yml` (macOS arm64).

Checks actually seen: on head `cc16d49`, CI had 12 jobs pass (`rust` macOS/Ubuntu,
`gpu-contracts`, `media-encoding`, `g2-export-macos`, `apple-silicon-preview`) and **2 fail**:
- `screen-slots-macos`: `real_ffmpeg_screen_video_decodes_proxy_and_converts_p3` fails at
  `crates/motion-export/src/screen_video.rs:571`. With FFmpeg 9.0.1, the synthetic
  `smpte432` clip probes as `Bt709`. The P3 conversion is therefore **not yet verified**
  with real FFmpeg.
- `real-asset-pack`: `examples/stick-studio/manifest.json:63` `hand_props_sha256` is stale
  after the `hand-props.json` change. Local regeneration changes it `9b1843d9…` →
  `d016de0e…`. The missing-zip error is a consequence of this failure.

On `main` from `2ddd16f` onward, both workflows fail or were cancelled. The PR reports local
fmt, clippy 0 warnings and `cargo test --workspace` 1003/0/10 (ORK did not re-run these).

Open (owner):
- P3 test clip tagging → CI green (A10, `fix/screen-video-p3-test`).
- Regenerate stick studio manifest (A10).
- Desktop: screen-cue editor, video import, background decode scheduling and frame
  loading by time (A09; cache interface A10).
- Streaming per-frame loading in export with a memory budget (A10).
- `tap_times` driving finger motion in auto direction (A02 + A05).
- Screen brightness and glow in dark scenes (A08).
- P3 on a real iPhone recording (A10; needs the user's recording).
- `smpte431` and untagged-source handling (A10).
- `holds: phone` shorthand (A02; deferred by ORK).
- Seek and frame-by-frame equivalence with screens in the G2 scene (A04, KS-003d).

## A10 export follow-ups — PR #126 merged (2026-10-28)

Status: **merged** to `main` (`9342979`) with the user's explicit approval.

Completed:
- G2 regression hashes and the scene SHA-256 have one source,
  `crates/motion-export/tests/support/g2.rs`, read by the test, the `g2_export` command
  and the `Export QA` workflow. A scene change fails first on `G2_SCENE_SHA256`.
- `g2_export` reports `model_policy_refused` read back from `ModelPolicy::permit`
  (replaces a fixed field) and `sequence_matches_reference`.
- Streaming MP4 export returns `Error::PartialOutput` when cleanup after a failure fails;
  new unix test with a locking fake encoder.
- Local re-measurement (iMac M1, macOS 27.0.1, loaded machine): SVG median 30.5 ms,
  PNG median 1964.7 ms, `docs/evidence/g2-export/local-apple-m1-2026-10-28.json`.

Checks: CI 13/13 on #126. Locally (macOS 27.0.1, Apple M1): `cargo fmt --all --check` ok,
`cargo check --workspace` ok, `cargo clippy --workspace --all-targets` 0 warnings,
`cargo test --workspace` 986 passed, 0 failed, 9 ignored; sandboxed model-free SVG/PNG
runs pass.

Open: model-free run with imported WAV speech (A07 KS-002e, needs the user's recording);
MP4 on a physical Mac (no FFmpeg); A10 status update (ORK); `sha2` row in
`docs/a01-dependencies.md` (A01).

## ORK G4 evidence summary and merges — PRs #123, #118, #124 merged (2026-10-27)

Status: **merged** to `main` with the user's explicit approval: #123 (A10 export QA,
`c194e83`), #118 (A09 UI levels, No AI mode, drag logging, `a2ddf7f`) and #124 (ORK-G4-01/02,
which carries #119 and #122). #121 (KS-003b review, renamed ORK-G2-02a) and #108 are merged after it; #108's content
(ORK-G1-A04) was already in `main` via #111, so it is merged with `main`'s versions and no content change.

Completed:
- A10: canonical frame clock in `motion-export`, typed `EncoderUnavailable` and
  `PartialOutput` errors, G2 frame-count/audio-length/preview-parity/regression tests, failed
  export tests incl. a real full disk, reproducible `g2_export` command, `Export QA` macOS
  workflow with a sandboxed model-free export and MP4 check, measurements with conditions.
- A09: three UI levels (UI state only), per-project `ModelPolicy` (No AI), EventLog
  drag/release logging, transport test on the shared evaluator.
- ORK: G4 evidence summary (ORK-G4-01), merge record (ORK-G4-02); architecture report
  chapter 8 checklist 6/11.

Checks: CI 12/12 on #123 and #118, 11/11 on #124. Locally on merged code (macOS 27.0.1,
Apple M1): `cargo fmt --all --check` ok, `cargo check --workspace --locked` ok,
`cargo clippy --workspace --all-targets --locked` 0 warnings, `cargo test --workspace
--locked` 985 passed, 0 failed, 9 ignored. Release `studio-desktop` packaged with
`scripts/package_macos.py` (ad hoc signed, `codesign --verify --deep --strict` ok),
installed to `~/Applications` and launched: the process started and stayed running.

Open: the window was not captured and the UI was not exercised by hand (no screen
recording permission); G2 is still returned (phone A08, KS-003d A04, WAV speech A07 blocked on
the user's recording); G3 and G4 not approved; KS-002h, KS-006, KS-007; MP4 export on a
physical Mac (no FFmpeg installed). No notarization or Developer ID signing.

## A06 dangle contract — PR #120 merged; ORK-G2-02a KS-003b review (2026-10-27)

Status: **merged** to `main` with the user's explicit approval (merge `d9b88ae`, commits
`b0fa1c4` + `b517c39`). ORK decision ORK-G2-02a (recorded in PR #121, merged after #124): KS-003b **accepted** (spring module #115
`974ef76` plus this dangle contract).

Completed:
- `motion_animation::spring`: `dangle_joint` accepts only non-root joints that are neither
  A05 attachment-frame joints (pelvis, torso, the twelve `STICK_LIMBS` joints) nor their
  ancestors; `apply_dangle` adds a validated angle (`|angle| <= π`) to rotation only;
  typed `DangleError`. Re-exported from `lib.rs`.
- 6 new tests in `crates/motion-animation/tests/spring.rs` (25 total), including A05's
  review condition `dangles_leave_ik_grip_and_held_prop_unchanged` (IK solution, limb
  joints, grip point and held phone bit-identical with and without springs).
- Contract in `docs/physics-springs.md`; A05 reviewed it and accepted with that condition.

Checks: A06 locally before merge – fmt, check, clippy (no warnings), test (968 passed,
0 failed). PR #120 CI 12/12; the first media-encoding run failed with exit 124 on an apt
download timeout while installing ffmpeg (infrastructure) and passed on rerun
(`gh run rerun 40551418227 --failed`). ORK spot check on a separate `origin/main` worktree
(`d9b88ae`, own target dir, macOS): `cargo test -p motion-animation` 49 passed, 0 failed;
`cargo test -p motion-rig` 32 passed, 0 failed; clippy for motion-animation clean;
`cargo fmt --all --check` clean.

Open: visual quality of springs/dangles not assessed in rendered frames or motion (tests
are numerical); EventLog → release initial state, IK target resolution and follow-through
from parent motion (A04, KS-003d); marking dangle joints in MotionScript or the rig
(A02/A05); controls (A09); coupled chains (multi-link hair). G2 itself remains open.

## ORK G2 first implementation round — PRs #111, #113–#117 merged (2026-10-27)

Status: **merged** to `main` with the user's explicit approval, in order: #111
(`5f659f0`, ORK-G1 / ORK-G2-01 file areas, reconciled with the parallel #112 record),
#113 (`5e93709`), #114 (`3551257`), #115 (`974ef76`), #117 (`824c399`), #116 (`9c69902`).

Completed:
- KS-002f (A03, #113): EventLog v1 accepts only the 24 kHz dialogue clock.
- KS-002g (A04, #114): `motion_scene::time` delegates to `motion_audio::clock`; equivalence
  test over seconds, frame rates and sample ranges.
- KS-003b (A06, #115): `motion_animation::spring` – analytic damped springs, constant and
  profiled forces at a fixed 1/1200 s step, five profiles, release initial state.
- KS-003a (A05, #117): limb attachment contract, joint limits, two-bone IK, prop catalogue
  and `grip` contract moved into `motion-rig`; `examples/g2-rig/`.
- KS-003c (A08, #116, without the phone): depth bands and paint order, wide/close-up
  framing rules, background style checks, `examples/g2-minimal-scene/`, two 960x540
  reference PNGs (CPU/resvg).

Checks: CI 12/12 on each code PR's final commit (#111 11/11). Branches were updated to
`main` before merging; #117 lockfile verified with `--locked`; #116 re-checked locally
against the new `main` (`cargo check --workspace --all-targets --locked`; motion-render,
motion-art and motion-rig tests 99 passed, 0 failed). Agent-local runs used a shared
target directory that caused intermittent cross-worktree build errors, so CI is the
authoritative evidence.

Open: phone-in-hand scene and references (A08), KS-003d (A04), KS-002e (A07), KS-002h
(A02/A10), GPU parity for the G2 scene, visual art review, G2 sample. PR #108 is still
open although its content reached `main` via #111. No hardware, perceptual or GPU
acceptance is claimed.

## ORK G1 gate approved with condition — PR #112 merged (2026-10-27)

Status: **merged** to `main` as `ecfa0de` from branch `docs/ork-g1-gate`
([PR #112](https://github.com/aleksipii/kilsatstudio/pull/112)) with the user's explicit
approval. Documentation only (`docs/agents/**`).

Completed: ORK-G1 decision. SceneSpec (A02, PR #105), TrackSpec (A04, PR #107) and
AudioTimeline (A07, PR #109) `done`; EventLog (A03, PR #106) `done` with condition
KS-002f, because `EventLog::validate` accepts any `sample_rate` in 1..192 000 Hz instead of
the single 24 kHz clock (ORK-G1-03, shared rule 2). G2 opened: KS-003 and A04 (G2), A05,
A06, A07 (KS-002e) and A08 are `ready`. New tasks KS-002f (A03, blocks EventLog consumers
A06/A09) and KS-002g (A04/A02/A10, clock copies delegate to `motion_audio::clock`).

Checks: ORK spot checks on `main` `393c2f1` (macOS, local): `cargo test -p motion-script`
132 passed, `-p motion-scene` 65 passed, `-p motion-export` 143 passed / 8 ignored, 0 failed;
all four contract example folders are read by tests; each contract PR's CI 12/12. PR #112
CI 11/11 passed.

Open: KS-002f, KS-002g, KS-002e; closing superseded PR #102 (needs user permission); the G2
minimal scene. KS-006 and KS-007 remain G4 conditions.

## ORK G1 review of A07 KS-002d — PR #110 merged (2026-10-27)

Status: **merged** to `main` as `393c2f1` from branch `docs/ork-a07-merge`
([PR #110](https://github.com/aleksipii/kilsatstudio/pull/110)) with the user's explicit
approval. Documentation only: ORK-G1-KS-002d approves A07's G1 part (KS-002d `done`),
records KS-002c as merged, adds KS-002e (A07, G2: human-recorded fi/en WAV examples and
AudioTimeline use in mouth evaluation) and updates status lines. Checks: PR CI 11/11
passed; evidence for the approval is PR #109's CI and local checks (see below). Open: the
G1 gate as a whole (KS-002a/b/c review).

## A07 AudioTimeline v1, sample clock and strict model-free path — PR #109 merged (2026-10-27)

Status: **merged** to `main` as `ae39c4d` from branch `feat/a07-audio-timeline`
([PR #109](https://github.com/aleksipii/kilsatstudio/pull/109)) with the user's explicit
approval. Owner A07, task KS-002d, contract ORK-G1-04.

Completed: `motion_audio::clock` (canonical `DIALOGUE_SAMPLE_RATE` = 24 000 Hz and A04's
seconds/sample/frame rules with typed `ClockError`; the motion-lipsync and motion-voice
constants are now aliases). `motion_lipsync::audio_timeline` — AudioTimeline v1:
`format_version: 1`, strict JSON, imported or synthesized source provenance (provider,
model revision, voice, cache key, PCM hash), `MixClip`-compatible placement,
source-local phoneme/viseme intervals with `AlignmentSource`, typed validation errors,
`viseme_at` and a pure, boundary-tested `blend_at`. Strict model-free path:
`motion_project::ModelPolicy::permit` (single check point, ADR-A01-09),
`motion_tts::DisabledSpeechProvider`, `motion_voice::model_free` (missing WAV → typed
error before any provider call, no fallback) and
`motion_voice::audio_timeline::scene_audio_timeline`. Finnish estimator v2 (tests for
ä/ö/y, long vowels, geminates; new rules for n + k/g, š/ž, x, NFD letters) and Finnish IPA
`y ø ʋ ç x ɦ`. The 13 visemes are unchanged. Contract in
[audio-timeline.md](audio-timeline.md); examples in `examples/audio-timeline-v1/`.

Checks: local macOS on the branch: `cargo fmt --all --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets` (0 warnings), `cargo test --workspace` (892 passed,
0 failed, 8 ignored); the branch merged with `main` after PR #107: fmt ok, clippy 0
warnings, 906 passed, 0 failed, 8 ignored. `python3 -m unittest discover -s
services/kokoro-runtime/tests -v`: 55 run, OK, 5 skipped (synthesis-path tests need NumPy,
not installed). No model was downloaded and no synthesis ran. PR CI on `62f96be` 12/12
passed. Acceptance test `crates/motion-voice/tests/model_free.rs`: a counting provider
whose every call fails is called 0 times through preparation, export, project
save/reopen and remix from the reloaded timeline.

Open: human-recorded Finnish/English WAV examples and timing measured against them (mouth
timing is structurally validated only); `ProjectDocument.model_policy` and UI/worker check
points (A09); `motion-export` check before speech preparation (A10); `motion_scene::time`,
`motion_script::auto_direction::SAMPLE_RATE` and export test support delegating to
`motion_audio::clock` (A04/A02/A10); viseme blending in the renderer (A08). The Finnish
estimator version bump invalidates earlier cached Finnish estimates by design.

## A04 TrackSpec, time conversions and seek equivalence — PR #107 merged (2026-10-27)

Status: **merged** to `main` as `10135ad` from branch `feat/a04-timeline`
([PR #107](https://github.com/aleksipii/kilsatstudio/pull/107)). The user delegated the
review and merge decision to A04 after the first CI pass; the review added the missing
G1 examples before merging. Owner A04, task KS-002c, contract ORK-G1-02 (resolved TrackSpec).

Completed: `motion_scene::tracks` (`TrackTarget::{Abs, Rel, Revert}` resolved once into
`{from, to}` spans with start/end samples; later start wins and interrupts, equal starts
→ later written wins, finished spans hold exact `to`; `MAX_TRACK_SPANS`); `Event::Move`
positions now come from resolved tracks in `PreparedAnimation` (endpoints exact; at most
rounding difference from the old per-call chain, tested ≤ 1e-9); `-0.0` normalized;
`ValidatedScript::evaluate_sample`. `motion_scene::time` names the existing 24 kHz rules
(seconds → nearest sample, `floor(frame·24000/fps)` as in `motion-export`, sample → frame);
`SEQUENCE_SAMPLE_RATE` moved there with the same public path. Contract, mapping of current
authored data, conversion rules and determinism levels in
[editable-timeline.md](editable-timeline.md#trackspec-overlap-order-and-time-conversion-a04);
examples in `examples/trackspec-v1/`. No new clock or evaluator.

Checks: local macOS `cargo fmt --all --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets` (0 warnings), `cargo test --workspace`
(884 passed, 0 failed, 8 ignored). Direct seek is bit-identical to frame-by-frame playback
(test). PR CI on `c95fbd9` 12/12 passed (rust macOS/Ubuntu, gpu-contracts, media-encoding,
apple-silicon-preview, real-asset-pack).

Open: authored `Rel`/`Revert` moves in MotionScript (A02 schema request); emotions,
backgrounds, camera, gestures and clips keep their own evaluators (rules documented and
tested, not migrated); `motion_scene::time` delegates to A07's `motion-audio` clock once it
lands (ORK-G1-04); pixel/video determinism (A08/A10). G1 gate still needs A07 AudioTimeline v1
(merged later in PR #109).

## ORK G0 approval and G1 contract owners — PR #104 merged (2026-10-27)

Status: **merged** to `main` as `698fd2f` from branch `docs/ork-g1` with the user's
explicit approval ([PR #104](https://github.com/aleksipii/kilsatstudio/pull/104)).
Documentation only (orchestrator, `docs/agents/**`).

Completed: G0 approved (ORK-G0: A01 review accepted; report/code differences decided:
egui stays, 13 visemes canonical, own rule engine instead of a RETE library, head-pose
classification, No-AI policy location); G1 contract owner crates decided (ORK-G1-01…06:
SceneSpec `motion-script`, TrackSpec written in `motion-script`/resolved in
`motion-scene`, EventLog `motion-script`, sample clock `motion-audio` and AudioTimeline
`motion-lipsync`, rig attachment `motion-rig`, render contract `motion-render`); task
register KS-001…KS-007 and agent status lines updated, including A02 (PR #105) and
A03 (PR #106) as `review` awaiting the ORK G1 review.

Checks: PR CI 11/11 passed. No Rust code changed, so cargo checks are not evidence for
this entry. Open: G1 gate still needs A04 TrackSpec and A07 sample clock + AudioTimeline
v1 and ORK's G1 review of A02/A03; A04 `ready` is ORK's decision; KS-006 (wheel hash
lock) and KS-007 (licence texts/NOTICEs) are G4 conditions.

## A03 rule contract and EventLog v1 — PR #106 merged (2026-10-27)

Status: **merged** to `main` as `6ddfdcc` from branch `feat/a03-rules-eventlog` with the
user's explicit approval ([PR #106](https://github.com/aleksipii/kilsatstudio/pull/106)).
Owner A03, task KS-002b, contract ORK-G1-03 (EventLog).

Completed: `motion_script::event_log` (EventLog v1: versioned JSON log of `key`, `drag`,
`camera_pose` and `release` events with integer scene-local sample positions, recorded
`sample_rate`, source/type compatibility, per-scene order, bounds and script-reference
validation with typed `EventLogError` paths); `key` rule trigger and
`Script::expand_rules_with_events` returning the expanded script plus an ordered firing
trace; `expand_rules()` order, IDs and messages unchanged (`rules_tests.rs` byte-identical).
Emotion conditions use a per-scene history with binary search (same results). Decisions
A03-D1 (rule name is the priority, no priority field) and A03-D2 (no chaining), rule
contract, EventLog format and cost measurement in [animation-rules.md](animation-rules.md);
valid/invalid examples in `examples/event-log-v1/`. One label line added to the desktop
rule editor for the new trigger (A09 file; no menu entry). No RETE library added.

Measurement (`rule_expansion_cost` example, Apple M1, 8 GB, macOS 27.0.1, rustc 1.99.1,
release, single machine): 64 rules × 1024 events with a false emotion condition
213 ms → 1.5 ms; 64 key rules × 1024 logged presses 0.25 ms. Not a constant-time claim.

Checks: local macOS `cargo fmt --all --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets` (0 warnings), `cargo test --workspace`
(870 passed, 0 failed, 8 ignored; 12 new EventLog tests). PR CI 12/12 passed (rust
macOS/Ubuntu, gpu-contracts, media-encoding, apple-silicon-preview, real-asset-pack).

Open: storing/recording the log in the project file and UI (A09, G3); consuming drag,
release and camera-pose events in timeline and physics (A04/A06); `key` in the rule
editor menu (A09); comparing the log's sample rate with the canonical clock once A07
adds it (ORK-G1-04). G1 gate still needs A04 TrackSpec and A07 AudioTimeline v1.

## A02 versioned MotionScript compiler — PR #105 merged (2026-10-27)

Status: **merged** to `main` as `d9db5be` from branch `feat/a02-dsl` with the user's
explicit approval ([PR #105](https://github.com/aleksipii/kilsatstudio/pull/105); replaces
conflicting PR #102). Owner A02, task KS-002a, contract ORK-G1-01 (SceneSpec).

Completed: decision A02-D1…D3 recorded before implementation (seq/par/stagger are
compiler sugar in a `version: 1` envelope, lowered to absolute `Event` times before
`Script::validate`; half-open intervals); `motion_script::compiler::{compile,
compile_with_origins, Compilation}`; typed `DiagnosticCode` with YAML paths and
line/column for syntax errors; envelope JSON Schema in `crates/motion-script/schema/`;
specification and SceneSpec handoff in [motionscript.md](motionscript.md); valid and
invalid examples in `examples/motionscript-v1/`. Legacy versionless `parse` unchanged;
all five legacy `examples/*.yaml` parse and wrap identically. `rules.rs` untouched.

Checks: local macOS `cargo fmt --all --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets` (0 warnings), `cargo test --workspace`
(0 failures; compiler suite 18/18). PR CI 12/12 passed (rust macOS/Ubuntu,
gpu-contracts, media-encoding, apple-silicon-preview, real-asset-pack).

Open: direct v1 opening in the desktop editor (A09); line/column for semantic errors
(path only); G1 gate still needs A03 EventLog v1, A04 TrackSpec and A07 AudioTimeline v1.

## Architecture agent task files — 2026-10-27

Status: **merged** to `main` as `ce57c3da14a35a52a95e24a119852d65abae9d5d` from branch `docs/architecture-agents`
with the user's explicit approval (no pull request: `gh` was not authenticated).
Documentation only. [docs/agents/](agents/README.md) contains one task file each for
the orchestrator and agents A01–A10: baseline verified against `496a8a4`, allowed scope,
dependencies, task checklist, acceptance criteria and outcome section. Thin Claude Code
subagent definitions are in `.claude/agents/kilsat-*.md`.

Completed: task files, subagent definitions, relative link check (no broken links).
Not run: Rust checks (no code changed) and CI for this merge. Open: every A02–A10 task
(`todo`) and the remaining A01 items (`review`); no feature is implemented by this entry.

## A01 architecture and dependency review — 2026-10-26

Status: **merged** in [PR #101](https://github.com/aleksipii/kilsatstudio/pull/101)
on 2026-10-26 as `cdff71dae61d1e4a861215a61158225ee9b9f54d`; not release-approved. Source checkpoint:
`773ac9417ee000668fd4f92750e25797a9f57e34`. See the
[A01 architecture decisions](a01-architecture.md),
[dependency rationale and status](a01-dependencies.md), and
[machine-readable inventory](a01-dependency-inventory.json).

Completed: source/manifest review, 14-module normal dependency DAG check,
15 direct external dependency rationales and 434 lock-package entries.
Inventory tests: 16/16 passed. No production code or interfaces changed.
Open: enforced model-free policy, exact-artifact licensing and distribution
review, target-specific graph, Python wheel lock and physical-device acceptance.
Cargo is unavailable in the local review environment; local Rust checks were not run.
Before merge, GitHub Actions passed for head `b60c023dc0286b9f7006d589e91aaf9eb2d83822`:
[Audio foundation #653](https://github.com/aleksipii/kilsatstudio/actions/runs/40395163758)
(Rust fmt/check/Clippy/workspace tests and Python tests on macOS and Ubuntu,
GPU contracts on both platforms, and media encoding), plus
[macOS practical preview #356](https://github.com/aleksipii/kilsatstudio/actions/runs/40395163508).
Codex review completed without findings. CI does not replace physical-device,
real-model, licensing or model-free-policy acceptance.

## Editable stick studio asset pack — PR #98 merged (2026-10-25)

[PR #98](https://github.com/aleksipii/kilsatstudio/pull/98) merged as
`bc81fd21dd6dfc2627ad35ce633c38b22eff5223` from final source
`16dc6f8e83254019ef2a339276f0a935237ff281` after all final-source checks passed.

The [starter pack](../examples/stick-studio/README.md) adds six original custom
rigs: Noutaja the Grim Reaper, Veikko the commuter, Päsmä the boss, Kipinä the
inventor, Kiire the courier and Sisu the grandmother. They share 15 joints,
13 configured visemes, expressions, gaze/blink and five reusable joint clips.
Four editable sets and 14 props accompany 24 individual native projects and a
self-contained six-shot, 30-second 1920x1080/24fps starter. The Finnish
**Tikku-ukkostudio** button loads it through the existing source/artwork history.
Undo/Redo preserves the preceding work; loading clears the save path.

Normal editing serializes the compact 54,099-byte starter above the old 100 KB
editor limit. The shared screenplay limit is now 256 KiB, including the desktop,
history, opening/saving, direction workers and all three core director entry points.
Individual-rig, timing, 64-entry
and eight-MiB history limits are unchanged. An actual head-edit regression tests
normalization above 100 KB, Undo/Redo and save/reopen; oversized input is rejected.

The shared SVG renderer now optionally omits only its two default canvas fills.
The native asset exporter reuses project decode, scene evaluation and rendering
to produce six transparent character PNG/SVG pairs, four opaque background pairs
and 14 transparent prop pairs. Legacy opaque SVG output is regression-tested.
Packaging verifies source/tool hashes, alpha reports, all image dimensions and
independently probes/fully decodes the real H.264/AAC showcase. The actual local
video has **720 frames / 30.000 seconds**, with exactly 720,000 zero PCM samples;
it is deliberately silent, with no synthesized speech or model downloads.
The final local asset/video/probe/package stage took **38.855 seconds**; this is not a
physical-Mac or cold-start production benchmark. All six review frames and the
24-asset contact sheet were visually inspected for framing and readability.

Local validation passed format, workspace/all-target/all-feature strict Clippy,
141 repository Python and 55 runtime tests, 259 desktop tests and all four native
asset-export tests (including changed pixels confined to each configured mouth).
The first broad local Rust run hit two existing recognizer-enrollment tests that
need unavailable host `/proc` data. Remaining crate/target tests passed with
those two and the existing desktop exact-cache enrollment test excluded locally;
all three passed unfiltered in normal CI.

Final review caught the core directors retaining their own 100 KB guards; the
shared core/editor constant fixes that second boundary too. All three director
entry points now accept the actual normalized six-rig starter and reject one
byte above the bound before parsing. Six targeted starter tests, 16 core direction
tests and refreshed workspace/all-target/all-feature strict Clippy passed after
that fix. The asset guide also uses the actual Hahmot/Kuvitus workspace labels.

Final-source [Linux/macOS/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/40215469157),
[Mac packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40215469059)
and [native asset/video export](https://github.com/aleksipii/kilsatstudio/actions/runs/40215469193)
all passed: **839 Linux / 838 macOS Rust** tests with eight default-ignored tests
per OS, **55 runtime Python** tests and **140 passed / one skipped repository
Python** tests per OS, **52 GPU library +
two probe tests** per OS (four physical GPU tests ignored), and **four actual
FFmpeg tests**. Core director limits, all six configured mouths and normal
normalized-source editing were exercised on the final source.

Merged main [Mac packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40216189692),
[native asset/video export](https://github.com/aleksipii/kilsatstudio/actions/runs/40216189597)
and [broad verification](https://github.com/aleksipii/kilsatstudio/actions/runs/40216189665)
all passed with the same Rust, Python, GPU and real-media counts. The skipped
Python test needs FFmpeg/ffprobe, absent from the general test jobs; the separate
media job executed all four actual Rust FFmpeg tests successfully. The integrator downloaded main
[asset artifact 12608190948](https://github.com/aleksipii/kilsatstudio/actions/runs/40216189597/artifacts/12608190948),
verified its outer archive digest and all declared project/image/video hashes,
then independently probed and fully decoded its 720-frame / 30-second Full HD
H.264/AAC video. All 48 native images and six review PNGs are byte-identical to
the visually inspected local exports. The main CI asset/video/probe/package
stage took **39.047 seconds**, excluding compilation. This is an Ubuntu CI
observation, not a physical Mac benchmark. [Exact evidence](evidence/stick-studio-ci.json)
records the source, merge and artifact identities separately from acceptance.

The [new main Apple Silicon app](https://github.com/aleksipii/kilsatstudio/actions/runs/40216189692/artifacts/12608086135)
includes the embedded starter and corrected source budget. Unzip the GitHub
artifact and its contained app ZIP, open the app in Finder, and choose
**Tikku-ukkostudio** in the left project panel. Packaging, helper compilation,
ad-hoc signing, inventory and executable linkage checks passed; no physical GUI,
camera, audio-device or production notarization acceptance is inferred.
This final documentation/evidence record changes no checked application source.

The [professional studio roadmap](professional-studio-next.md) prioritizes a
thumbnail/search library and recovery, visual IK/contact/curve/shot tools and
integrated multitrack sound with measured FI/EN speech quality. The assets and
silent demo do not complete those features. Physical Mac/device/Metal acceptance,
human artistic and real-speech review, exact rights review and Developer ID /
notarization remain open. ArtCraft marketing was used only as a high-level
workflow reference; no Storytold source, dependency or adapter was introduced.

## Three English episodes — PR #97 merged (2026-10-25)

[PR #97](https://github.com/aleksipii/kilsatstudio/pull/97) merged as
`ef97286d2bae44e42f50d962406a6e2c6edac92e`, from final source
`8d68d021937dce8f0961e42a5d233b6b7d418e2f`. Three original English
Aino/Rane adaptations now have actual 1920x1080/24fps H.264/AAC videos,
English captions and an English EXAMPLE mileage-log prop, using the existing
original rigs/sets. The 31 Finnish projects and Finnish prop bytes are unchanged.
[Scripts, project files and downloads](kilsat-english.md) describe the pack.
Actual speech is local Kokoro af_heart (Aino) / am_michael (Rane), en-us:
24 measured dialogue WAV records, with no placeholder or system-voice substitution.

The [successful render](https://github.com/aleksipii/kilsatstudio/actions/runs/40178484861)
produced The Free Commute (**44.125 s / 1059 frames**), The No-Travel Meeting
(**45.208333 s / 1085 frames**) and The Savings Bonus (**46.625 s / 1119 frames**).
The integrator downloaded [artifact 12593905647](https://github.com/aleksipii/kilsatstudio/actions/runs/40178484861/artifacts/12593905647),
verified its archive SHA-256 and all MP4/WAV/pipeline hashes, independently
decoded/probed the videos and inspected opening/later frames from each episode.
[Exact evidence](evidence/kilsat-english-ci.json) remains bound to that source/run.
Source manifests retain source-only not-rendered state; actual receipts record
what the run completed.

Observed per-episode project/speech/render/probe times were **206.336 / 158.701 /
194.814 seconds** on VirtualMac2,1 Apple Silicon CI. Model/environment installation
and compilation are outside these timers; the entire three-episode batch took
**567.248 seconds**. No physical Mac or full-production timing gate is granted.
The bounded runner reuses the production exporter/sample clock and independent
media checks, binds source/model/voice/tool hashes and validates actual English
dialogue PCM provenance. Initial real inference exposed Preshed 3.0.14 breaking
spaCy tokenizer cache initialization; managed/development setup now pins 3.0.13,
rebuilds the manifest-keyed environment and retains fresh freeze/evidence.
The actual tokenizer load and all 24 real spoken lines passed with that pin.

Final-source [Linux/macOS/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/40178487305),
[Mac packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40178487300),
[voice selection](https://github.com/aleksipii/kilsatstudio/actions/runs/40178487303)
and the real English render passed before merge: **825 Linux / 824 macOS Rust**
tests, eight default-ignored tests each, **55 runtime + 132 repository Python**
tests per platform, **49 GPU library + 2 probe tests** per platform (four physical
GPU tests ignored), and **four real FFmpeg tests**. Format/check/strict Clippy
and the local 132 repository + 55 runtime Python tests passed.

All merged-source main verification passed on
`ef97286d2bae44e42f50d962406a6e2c6edac92e`:
[Rust/GPU/media](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515217),
[Mac packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515247),
[English render](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515230),
[voice selection](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515204).
Main passed the same **825/824 Rust**, **55 runtime + 132 repository Python**
tests per platform, **49 GPU library + 2 probe tests** per platform (four physical
GPU tests ignored) and **four real FFmpeg tests**. Eight default Rust tests remain
ignored per platform. All three English episodes completed again with the same
decoded frame counts and durations; [main video artifact 12593402307](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515230/artifacts/12593402307)
is a separate repeat run. The downloaded/hash-verified media evidence above
continues to describe the original successful source run.
[Current main Mac preview](https://github.com/aleksipii/kilsatstudio/actions/runs/40179515247/artifacts/12593781644)
includes the corrected English setup pin; it is ad-hoc signed and not Developer
ID notarized. Unzip both the artifact and its contained app ZIP and open the app
in Finder; no physical GUI or audio-device run is inferred from packaging.
These status-only documentation changes preserve the checked source bytes.
Still open: human listening/annotated lip-sync and artistic/comic-timing review,
approved real Kilsat capture, exact model/voice/dependency/asset rights, physical
Mac/device/GPU acceptance and production Developer ID/notarization.

## Finnish desktop completion — PR #96 merged (2026-10-25)

[PR #96](https://github.com/aleksipii/kilsatstudio/pull/96) was normally merged as
`3a33b6f11e2b0ba611ea7a1f8e94d2ec3b43b70b`, from final source
`111f13c911805df07dabefda0d089fa236ce5028`. It completes Finnish controls,
guidance, status/error context and enum display names across the desktop's scene,
speech, timing, rig/clip, artwork, sequence, export, GPU and performance panels.
The camera helper's purpose/usage/errors and bundled permission description are
also Finnish. Protocol/serialization keys, stored IDs, source examples and exact
backend error details are preserved. Six agents owned separate modules; the
parent integrated the main workspace, truthful mouth-source display, user guide
and checks.

An absent imported alignment no longer falsely reports a resting mouth when a
text estimate is actually prepared. The UI uses `DialogueClip::mouth_source()`;
legacy tool estimates are labelled generically, without attributing them all to
Rhubarb or claiming measured quality. A regression test checks estimated versus
absent mouth data in the rendered UI while preserving the document and history.

Local verification: 254 desktop tests passed (one existing host-information test
excluded locally), 121 repository Python tests passed, and strict all-target,
all-feature workspace checking passed. Actual egui meshes were captured and
visually inspected at 1440x900; existing minimum-window checks remain. Serde
attributes and raw fixture literals were compared unchanged; the camera's
non-string code is unchanged.

Final-source [Linux/macOS/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/40136324331)
and [Apple Silicon packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40136324321)
passed before merge. Merged-source main
[Rust/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/40174527148)
and [Mac packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40174527149)
also passed. Both source and main runs passed **825 Linux / 824 macOS Rust
tests**, with **eight default-ignored tests** each, **55 runtime + 121 repository
Python tests** per platform, **49 GPU library + 2 probe tests** per platform
(four physical GPU tests ignored) and **four explicitly executed real FFmpeg
tests**. Format, workspace check and strict Clippy passed. The native Swift
camera helper was built and its no-camera help/invalid-argument checks passed;
the packaged archive and build/linkage evidence were checked.

[Download the current main Apple Silicon preview](https://github.com/aleksipii/kilsatstudio/actions/runs/40174527149/artifacts/12592070238).
GitHub artifact `12592070238` has archive digest
`sha256:f9e12e11c3eee8daa9549b9d0b8f175a2fb1ca249523502ea6099a49f537abad` and expires on 2026-11-08.
Unzip the outer artifact and its contained application ZIP, then open
**Kilsat Studio Preview.app** in Finder. Downloading does not launch it.
The [twelve-topic Finnish guide](../apps/studio-desktop/KAYTTOOHJE.md) is embedded
under **Ohjeet**. Finnish synthesis requires a configured local Piper profile or
explicit WAVs. The preview is ad-hoc signed; Developer ID/notarization remain
open. This status-only documentation update preserves the checked source bytes.

Still open: physical Mac GUI/audio/camera and signing acceptance, manual
real-speech lip-sync annotations/listening, artistic/comic-timing review, an
approved real Kilsat screen capture, final upstream/asset/dependency rights and
production GPU/device acceptance. M1–M6 whole-product acceptance is not granted.

## Production integration — PR #95 baseline (2026-10-24)

Main now integrates the outlined,
rounded rigs from PR #89 and implements measured-audio direction with reaction
shots, local Finnish speech, continuous reviewed head-tilt recording, bounded
streaming Full HD export, expanded GPU commands, release tooling and 31 original
Finnish episode sources. See [the integration record](production-integration.md)
and [Finnish season instructions](kilsat-season.md). Older dated sections below
are historical snapshots; their statements about missing Finnish speech,
streaming Full HD or GPU captions are superseded by this integration.

An actual Finnish 1920x1080/24fps, 42.708333-second episode completed the entire
pipeline and independent media checks in 77.197seconds on Apple Silicon CI
(VirtualMac2,1; CI/VM observed). The physical-Mac gate remains false. The exact
source, host and artifact are recorded in the integration document. A separate
[Full HD CI run](https://github.com/aleksipii/kilsatstudio/actions/runs/40127342644)
completed all 31 Finnish episode videos and seven compilations (1920x1080/24fps,
22min8.4s total episode runtime). All seven downloaded artifact archives and their
MP4/WAV/pipeline hashes match the recorded [evidence](evidence/kilsat-season-fullhd-ci.json).
The final desktop correction invalidates prepared/pending audio on Piper profile
changes; 253 local desktop tests pass with one existing host-information test
excluded. All final-source checks passed before the authorized merge.

PR #95 was normally merged as `048ea9c87577fd46f18a4ee250552ee7c6cc21f6`,
preserving PR #89's ancestry; GitHub also marks PR #89 merged. Final source
`e31c6b60f2ee97f06847daab8e6ebd7ff4133e42` passed
[Linux/macOS/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/40128482356)
and [Apple Silicon packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/40128482272).
Linux/macOS passed **824/823 Rust tests**, with **eight default-ignored tests**
each, **55 runtime and 121 repository Python tests** each. Both GPU jobs passed;
the media job explicitly executed all **four real FFmpeg tests**, including the
new streaming frame-completeness check. Ad-hoc signed packaging passed the
camera helper's no-camera help/invalid-argument checks, archive audit and native
linkage/build-evidence checks. No physical camera or GUI session is inferred.

All merged-code main runs passed:
[Rust/GPU/media](https://github.com/aleksipii/kilsatstudio/actions/runs/40129209964),
[Mac package](https://github.com/aleksipii/kilsatstudio/actions/runs/40129209962),
[Full HD season](https://github.com/aleksipii/kilsatstudio/actions/runs/40129209986).
Main verification on `048ea9c87577fd46f18a4ee250552ee7c6cc21f6` passed the
same **824/823 Rust**, **55 runtime + 121 repository Python tests** per platform,
**49 GPU library + 2 probe tests** per platform (four physical GPU tests remain
ignored) and **four real FFmpeg tests**. All seven Full HD groups completed again:
31 actual episode MP4s and seven compilations. The independent main Apple Silicon
CI measurement completed in **88.205seconds**; this is CI timing, not physical
Mac acceptance.

[Download the main Apple Silicon preview](https://github.com/aleksipii/kilsatstudio/actions/runs/40129209962/artifacts/12575020684).
The build is ad-hoc signed, not Developer ID notarized. Unzip the outer artifact
and its contained application ZIP, then open Kilsat Studio Preview.app in Finder.
Finnish speech requires a configured local Piper profile or explicit WAVs.
Documentation-only status commits preserve the checked source bytes; no new
Rust test claim is inferred from their text edits.

Still open: physical Mac GUI/audio/camera and signing acceptance, manual
real-speech lip-sync annotations/listening, artistic/comic-timing review, an
approved real Kilsat screen capture, final upstream/asset/dependency rights,
production GPU/device acceptance and authorized Developer ID/notarization.
M1–M6 whole-product acceptance is not granted by code or CI alone.

## Script-to-animation automation design (2026-10-20)

The requested ten agents produced a linked
[rule-based automation design](automation/README.md), grounded in source
`7bb5e21331a672b15be5b2227c93ccc8171e8fa7`. This is design work, not an
implemented script-to-professional-animation product or a milestone acceptance.

- [x] Ten bounded workstream reports: ingestion, directing, acting, assets,
  camera, speech, performance, UX, quality and integration.
- [x] Shared proposed contracts, deterministic rule precedence, existing
  MotionScript/shot/rule integration, 30–60 s / two-actor / three-story-scene /
  sixteen-final-shot profile and a 300 s proposed serial stage budget.
- [x] Finnish and English screenplay/speech/caption design contracts (fi-FI,
  en-US, en-GB); language switches invalidate dependent audio/timing caches.
- [x] New Finnish and English Aino/Rane pilots, source-bound authored intent
  fixtures and design-only checker with 20 locally passing contract tests.
  CI configuration checks these fixtures; it does not synthesize or render.
- [ ] Implement the intent adapters, directing/acting solution and native UX.
- [ ] Author and review the actual character/scene/prop packages and Kilsat capture.
- [ ] Add/verify Finnish speech or import actual approved Finnish recordings;
  current SpeechRequest language validation accepts only en-us/en-gb.
- [ ] Preserve memory/work limits while enabling streamed Full HD export:
  current one-billion cumulative raster-pixel bound rejects 1080p24 at 30–60 s.
- [ ] Measure the complete <=300 s run on named physical Mac hardware, including
  audio, queueing, cache verification and final media. No timing SLA is measured.
- [ ] Pass perceptual acting/readability/lip-sync review and all remaining
  licensing/device/release gates. Pixar/Disney-level quality is not established.

The new pilot is newly authored design material; it is not a recovered copy of
the unavailable 31-episode season. All other open product acceptance gates remain
open. No new Rust implementation, real speech, physical Mac or rendered video is
claimed by this design delivery.

## Latest merge status

### Reference correction and integration audit (2026-10-21)

The initial live GitHub audit examined all 82 existing PRs: each was already
merged. `main` and `kilsatstudio1.0` were identical at `9e06318`, with successful
Linux/macOS, GPU-contract, media-encoding and Apple Silicon packaging workflows.
There was no backlog of unmerged historical PRs to reapply.

PR #83 (`2a347c7`) adds CI review artifacts: four PNG shots, a contact sheet,
timing report and an FFmpeg MP4, retained for 14 days. Final head `c71daffa`
passed [Linux/macOS/GPU/media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39561420340)
and [Apple Silicon packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/39561420400).
The workflow runs on pushes to both target branches. Its first export attempt
exposed a missing output parent; the final workflow creates it before exporting.
The actual downloaded baseline MP4 is H.264 640×360 with AAC, 11.666667 s.
It uses generated placeholder tones, unlike the separate prior 13.5 s Kokoro run.

PR #84 (`8dc0a3d`) adds an inventory of locally installed English voice files,
explicit boy/father voice audition options and language/speed/model preflight.
Its final head `e5ecb210` passed [Linux/macOS CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39561301750),
[Apple Silicon packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/39561301698)
and [voice-selection tests](https://github.com/aleksipii/kilsatstudio/actions/runs/39561301744).
Six model-free selection tests passed locally. Existing voice defaults remain;
no new model download, Kokoro inference or listening approval occurred here.

PR #85 (`afbe853`) integrates the visual corrections and canonical fixture
regeneration diagnostics. Its final head `f26141d` includes current main and
passed [full Linux/macOS Rust, GPU-contract and real-media CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39564466120)
and [Apple Silicon packaging](https://github.com/aleksipii/kilsatstudio/actions/runs/39564466112).
All 17 reference tests passed (one real-speech test remains explicitly ignored
without external WAVs), including legacy SVG compatibility, backlight actor
occlusion, enlarged-screen animation, caption separation and oval removal.
Earlier CI failures exposed stale probes and formatting; the final probe-only
fixes measure actual background lighting rather than light over a character.
The generated project was taken from the real CI serializer and matches its
source/art/assets generator. The original stale-fixture check remains active.

The reference correction review used the actual CI-produced MP4 from PR #85
head `2fa7e4f`, artifact `12328976009` in
[run 39562652466](https://github.com/aleksipii/kilsatstudio/actions/runs/39562652466).
The parent extracted and viewed all four shots at 1, 4, 7.5 and 10.5 seconds.
It confirmed the blue oval is absent in both boy shots (the actual source was a
pillow ellipse), the father is darker with no green overlay, eyes/brows and the
phone are larger, captions have a backing clear of the blanket, and mouth
contours are stronger. The H.264/AAC video is 640×360 and 11.666667 seconds,
using placeholder tones. Later test/provenance integration changes preserve
those reviewed renderer/art bytes. This accepts the scoped visual corrections;
it does not establish professional animation, comic timing, acoustic mouth
alignment, voice choice, a hand-run GUI or GPU support for these effects.

The combined head `634aadd` passed 163 model-free Python tests locally
(6 voice-selection, 20 plan contracts, 55 runtime helpers, 82 packaging/audit),
the Finnish/English plan checker and `git diff --check`. Rust was unavailable
in this workspace; Rust checks and real render/export evidence came from CI.
No new Kokoro inference or original external WAV review occurred here. Anton
and CMUdict legal approval, physical Mac/device acceptance, Finnish synthesis,
production art and the measured 300-second full pipeline remain open.

PRs #78 (`2186f81`), #79 (`7bb5e21`) and #81 (`af705a6`) merged on 2026-10-21 after green Linux/macOS CI on each final
head (#79 and #81 were updated with main first; PR #80, a separate automation plan, merged in between and is not covered
here): #78 desktop caption choice (Follow screenplay / Show / Hide) applied in one shared path for previews and exports, kept out
of the screenplay and undo history and stored as an optional `.kilsat` field; #79 text-estimated mouth visemes in dialogue
preparation (precedence manual > forced > Rhubarb estimated > text-estimated, always labelled estimated, cache identity
includes overrides and lexicon version, non-fatal diagnostics); #81 the night-room reference sequence with captions,
acting rigs, lights and an animated phone screen driven by the same project. One local hand run of real offline Kokoro
(af_heart and am_michael, CPU, Apple M1, macOS 27.0.1; WAVs kept outside git) produced a 13.5 s sequence; wall time per
line was 1.9-2.4x the audio, including a fresh process and model load. The work was done by parallel agents and checked via
CI, their local runs and one reading of the contact sheet by the parent; the frames were not compared with the audio.
Visible problems noted by the agent and confirmed on the contact sheet: lights are drawn over the characters (the green
hallway light tints the father), eyes/brows and the phone screen are tiny at 640 px, captions overlap the blanket in the boy
shot. Not established: style, comic timing, voice choice (the boy uses a female preset only because it is installed), mouth
quality, MP4 export (no ffmpeg on this Mac), GUI behaviour, GPU support for captions/lights, and font/lexicon licence sign-off.

PRs #71–#76 merged on 2026-10-20 (plan, then five workstreams of
[professional-animation-plan.md](professional-animation-plan.md)), each after green Linux/macOS CI on its final head
(branches were updated with main first; #74 needed a fix because gitignored `*.wav` fixtures
were missing in CI, so the tests now regenerate the placeholder tones): #72 `323716e` captions
with a bundled SIL OFL font (Anton, recorded in NOTICE; licence sign-off still a human task);
#73 `49a140e` open-vocabulary English normaliser, CMUdict lexicon with rule-based fallback,
pronunciation overrides, unknown-word report and an estimated phoneme-to-viseme track (BSD-2
CMUdict licence read from upstream, not legally reviewed); #74 `007ddfd` shot library, cut
rules and an original night-room reference sequence (placeholder tones, TODO hooks); #75
`7fcdc4a` deterministic scene lights and animated image props (CPU only); #76 `cfc056e` custom-rig
acting layer (part variants, viseme mouths, expressions, gaze, blink). The work was done by
parallel agents and checked via CI and their local runs, not by reading every line.
Not established: any integrated end-to-end scene using all five (the reference scene still has
TODO hooks for captions, visemes, acting, lighting and the phone screen), lip-sync quality (no
annotated audio), Kokoro speech in the reference scene, style and comic timing, GUI behaviour,
GPU support for lights/animated props/captions, and font/lexicon legal sign-off.

PRs #66–#69 merged on 2026-10-20 after green Linux/macOS CI on each final head; branches that
conflicted on `docs/remaining-work.md` were merged with `origin/main` and re-checked locally
(fmt, all-features Clippy, `cargo test --workspace`) on an Apple M1 first: #66 native audio
cue and animation rule editors (headless-tested only), #67 numeric texture/UV/skin-weight
controls in the custom-rig editor, #68 GPU support for animated polygon/painted masks and
textured/skinned rig parts with 8 new parity fixtures, #69 sequence preparation on the job
graph (one worker), audio/frame memory budgets and hand-run release benchmarks (one M1).
A follow-up fix after #68 found that the ignored soft-brush Metal probe had never passed on
hardware (GPU soft edges ~15 levels too dark vs CPU); the shader now blends bands in CPU order
and the probe passes, with parity rows re-measured (RMS down, isolated max error up).
The work was done by parallel agents; claims were checked via CI, local runs and the one
probe re-run above. Not established: GUI behaviour of any editor, perceptual quality, GPU
text (needs a label/font decision), concurrent speech inference, input-to-display latency,
other machines, physical Mac/audio/stylus acceptance and licensing.

PRs #60–#64 merged on 2026-10-20 after green Linux/macOS CI on each final head (fmt, check,
Clippy, tests, GPU contracts, media encoding, Apple Silicon preview). Branches that
conflicted on `docs/remaining-work.md` or added `Script` fields were merged with
`origin/main` first, and fmt, all-features Clippy and `cargo test --workspace` were
re-run locally on an Apple M1 before pushing: #60 (`85042e6`) measured CPU/GPU parity
harness (17 fixtures, documented tolerances, one run on M1 macOS 27.0.1; GPU text not
implemented because the CPU raster path draws no text and no licensed font is bundled);
#61 (`d051790`) custom-rig part textures and two-joint skinning (CPU only; GPU reports
unsupported); #62 (`e905b10`) process memory budget for images/export, dependency job
graph (tested, not yet used by a pipeline) and an opt-in application-side input-latency
recorder; #63 (`9293e1a`) YAML audio cues on the shared dialogue sample clock;
#64 (`443190d`) reusable/conditional animation rules (YAML only). The work was done by
parallel agents; their claims were checked only through CI and the local runs above.
Not established: any physical Mac/audio/stylus/GUI behaviour, perceptual quality,
Apple Silicon benchmarks, input-to-display latency, rule/cue editors, and licensing.

PRs #56 (`9ccf873`), #57 (`384be89`) and #58 (`868480a`) merged on 2026-10-19, each after
green Linux/macOS CI (fmt, check, Clippy, tests, GPU contracts, media encoding,
Apple Silicon preview) and local fmt/all-features Clippy/`cargo test --workspace` on an
Apple M1: rigid motion for polygon and painted masks, a copy-based custom-rig clip
library with retargeting, and experimental GPU feathered polygons (Metal checked by
an ignored test run by hand against an analytic edge profile, straight edges only).
Open: see `docs/remaining-work.md`. No physical-device, real-speech or licence gate
was closed, and no GUI was exercised by hand.

PR [#55](https://github.com/aleksipii/kilsatstudio/pull/55) merged as
`c03bfe21779e9ed357c700f930f6d10086a5da86` on 2026-10-19 after final Linux/macOS CI.
Six agents and parent integration advanced the first packages across the remaining
areas: original-rig clip exchange, animated image-mask bounds, reviewed manual
mouth timings with project/history storage, camera presets, experimental GPU image
masks, update-time observations and Mac scenario evidence coverage. The linked
[remaining work list](remaining-work.md) records completed packages and the next
open work separately. External acceptance and all M1–M6 gates remain open.

PR [#54](https://github.com/aleksipii/kilsatstudio/pull/54) merged as
`dceb8a79b9bebee386e74e312eeb6c767c804a74` on 2026-10-19 after final Linux/macOS CI.
It adds optional real touch-force samples, soft coverage bands and experimental
GPU painted-mask commands. Pressure capture requires backend force events; Mac
stylus/tablet support is not established. Physical Mac input/Metal checks remain
unexecuted. See [painted masks](painted-masks.md), the
[manual Mac procedure](painted-mask-mac-acceptance.md) and delivery evidence below.
Main and the release branch include this implementation and progress record.

PR [#53](https://github.com/aleksipii/kilsatstudio/pull/53) merged as
`5251cc5ddc850f1b2c9493a0076bae5fce0a64b0` on 2026-10-19 after final Linux/macOS CI.
Vector mask painting/erasing stores ordered bounded strokes, explicit initial
coverage and round dabs, with native brush/eraser gestures, shared Undo/Redo and
CPU preview/export. See [painted masks](painted-masks.md) and the evidence below.
At that merge, pressure, soft brushes, animated paint, experimental GPU support and physical Mac
input acceptance remained open. No product milestone or device/license gate is
closed by this implementation. The release branch is synchronized with this record.

PRs #47–#51 reached main on 2026-10-19. #47 merged first (`511b008`, Mac Metal probe and
native arm64 Rhubarb record). Stacked PRs #48 (feathered polygon masks), #49 (image
masks) and #50 (custom-rig joint tracks plus the desktop joint key editor) were merged
into their stacked base branches (`7e7c5a8`, `1b75639`, `8e689d0`), so they reached
main only through the integration PR #51 (`678e41b`). Linux/macOS CI (fmt, check,
Clippy, tests, GPU contracts, media encoding, Apple Silicon preview) passed on each
head and on #51, and fmt/Clippy/`cargo test --workspace` passed locally on an Apple M1.
At that merge, open work included mask painting/erasing and pressure input, animated polygon/image masks,
GPU parity for the new masks, reusable custom-rig clips and retargeting, textures and
deformation, perceptual Rhubarb/Kokoro quality, packaged arm64 Rhubarb, licence
corpus, and physical Mac GUI/audio acceptance. No GUI was exercised by hand.

PR #46 merged as `45f31bd02352da9ac9c17ab9b5407b89198bdb2c` after final Linux/macOS CI.
The six-area delivery adds normalized original-rig groups, editable custom rigid
characters, isolated artwork groups, expanded experimental GPU composition,
explicitly enrolled local-package automatic estimate reuse, and Mac/license
observation reports. Full GPU/native parity, broader artist/character tools,
actual recognizer/device quality and final rights/release approval remain open.
Exact checks and artifact identities are recorded in the final section. The
release branch is synchronized with this implementation and progress record.

Earlier PR #45 merged as `85ca2dcc2f4b26e8922e0af0d560155ebdfc9719` after final Linux/macOS CI passed.
Original seven-joint live clips now offer explicit additive local offsets alongside
default-compatible override, including native controls, saved projects, Undo/Redo,
shared preview/export and conservative work limits. Independent rendered fixtures
and numeric checks passed. Normalized groups, arbitrary rigs, physical motion
acceptance and the other workstreams remain open. The release branch is
synchronized with this implementation and progress record.

Documentation-only PR #44 merged as `443e3f7b1881c02a7b0fdd55fbd647aeeba583cb`, assigning all eight
remaining areas to named agents and recording inspected next-package handoffs.
Local Markdown targets, whitespace checks and independent cross-workstream
review passed. No production code changed, no Rust/Python execution was newly
claimed, and no product/device/quality/license gate was closed. PR #43 below
records the preceding parallel implementation increment. The release branch is synchronized
with this assignment and progress record.

PR #43 merged as `ebd7287825c10c8dcfdbf7805ac920c03545eedc` after final Linux/macOS CI passed.
Live clips now support original-speed loops and explicit seven-joint selection;
soft ellipse masks support feathering and scene-local key animation. Native scene
reordering uses a versioned editable timing document and shared sequence clock.
The evaluated-geometry GPU experiment remains opt-in and incomplete; the engine
index foundation still has no eligible real profile. CI now retains final-ZIP-bound
build/native observations without granting license or device approval.
The release branch is synchronized with this progress record.

Earlier PR #42 merged as `ed149b4d363c490b2025733c8e9644e268dcd4bf` after final Linux/macOS CI passed.
Final signed preview ZIPs now receive a bounded offline inventory sidecar with
exact member hashes and current resource-boundary checks. CI attaches both ZIP
and report. Collection explicitly leaves distribution approval false; target
build graphs, native provenance, notices and legal review remain open.
The release branch is synchronized with this progress record.

Earlier PR #41 merged as `19667de847753bd1e0c7628e596449da3b061aa7` after final Linux/macOS CI passed.
Live original-rig layers now support per-instance animated weight curves and
linear entrance/exit fades, detached native editing, source Undo/Redo and project
persistence. Exact defaults retain the prior constant-weight behavior. The
release branch is synchronized with this progress record.

Earlier PR #40 merged as `aeca29cef42efa1b9a9393dfd95ef140ac1aa266` after final Linux/macOS CI passed.
The first original-rig live clip package now supports constant-weight ordered
override layers, half-open windows, persistent references, native editing and
prepared shared preview/export evaluation. Broader blending and hardware gates
remain open. The release branch is synchronized with this progress record.

Documentation-only PR #39 merged as `cf03e847674a00162766bceb81ba7e5ea44348f8`, assigning the
five open workstreams and recording inspected implementation/evidence plans.
Local Markdown link targets and diff whitespace checks passed; the license owner
also passed 16 existing inventory tests while the strict release gate stayed
blocked. This planning merge implements no production feature or acceptance.

Earlier implementation PR #38 merged as `797e7d4fb3c97b9577bef56a8570f25c49f2c34c` after its final
Linux/macOS CI passed. PR #37 merged earlier as
`0aa890bb6fa6ce597ba5f2a21fd942db720b7389`; PR #36 as
`e49f7add105409ed4d98859d1be4b55ae1c803b1`; PR #35 as
`6bd988ab162adb64cdcea621bb01600099abbeb2`; PR #34 as
`f89c59b8113b3b578c82fe648850615e704fdaf0`; PR #33 as
`504166d85daa15e88f937a777a17123162df5e8a`; PR #32 as
`f2e7b79f23968ac3b32637680ff2c48ac7f43453`; PR #31 as
`82f1b3116f6d8647e20076b0e2803f2fd2adedba`; PR #30 as
`de41228c0b68daece521cef1406361b21df0c3a6`. The release branch
`kilsatstudio1.0` is synchronized with this merge and progress record.
The authored bone editor, chronological history, geometric masks/CPU blending,
estimated-cue import, exact-audio mouth-shape quality evaluator, optional GPU
fixture and license inventory are integrated.
PR #27's user-reported first M1 Kokoro speech evidence is preserved.

The native acceptance tooling below is merged. Its implementation and CI checks
are complete; physical hardware, real recognizer quality, full-scene GPU rendering
and final redistribution clearance remain open.

| Completed | Still open |
| --- | --- |
| Timed solid colors/images, native editors, shared preview/export and SVG project sequence compositing | Physical Mac transition and SVG-viewer acceptance |
| Persisted per-bone tracks/editor, procedural strengths and editable versioned custom rigid characters with shared rest-part/root rendering | Custom-rig joint animation, textures/deformation and arbitrary custom-clip blending |
| Project clips, ordered override/additive and normalized original-rig groups, weight curves/fades, loops/joint selection; isolated artwork groups, image masks, polygon feathering, vector mask painting/erasing, soft edges and optional event pressure | Mac stylus input, animated painting, retouching, measured GPU mask parity and physical Mac compositing acceptance |
| Chronological source/artwork/asset-inventory Undo/Redo | Physical Mac shortcuts/accessibility and advanced retouching |
| Managed Kokoro, bound cache, reported M1 speech, estimated cues/archive and automatic reuse for explicitly enrolled self-contained local Phonetic packages | Real package compatibility/audited closure, offline/device acceptance and measured recognizer quality |
| CPU SVG/PNG/MP4, bounded experimental GPU artwork/masks/groups/blends/crossfades, input reuse and bound package/build/review-gap reports | Shared GPU fonts, full measured scene parity/production backend, physical Metal/performance and final license clearance |

Maintain this summary and the full checklist after each authorized merge. A new
implementation remains pending until its relevant checks pass; hardware gates
require actual evidence.

## Ownership and current increment

The execution list is [remaining-work.md](remaining-work.md), with named agents,
integrated first packages, next open work and separate external acceptance.
It is subordinate to this tracker; a scoped delivery does not close a product gate.

PR #46 delivery (2026-10-19): `normalized_groups`, `custom_rigs`, `gpu_complete`,
`artist_groups`, `speech_production_reuse` and `release_mac_evidence`; parent
integrated shared boundaries, tests, review findings, CI and release progress.
The broader eight-area ownership below remains active for unfinished work.

Previous parallel implementation increment (2026-10-18, integrated in PR #43):
`gpu_scene_implementation`, `animation_loop_masks`, `artist_mask_tools`,
`speech_reuse_implementation`, `mac_release_evidence` and
`planned_timeline_features`; parent owns cross-stream integration and release.
The follow-up table below assigns the remaining product responsibilities. Agent activity
does not constitute hardware, quality, license or full-milestone acceptance.

Eight-area follow-up assignment (2026-10-18). Each owner inspects the current
implementation and defines a concrete next package, interfaces, file boundaries,
checks and remaining acceptance evidence. This assignment changes no production
feature and grants no milestone, hardware, quality or license acceptance.

| Workstream | Assigned agent | Handoff and remaining scope |
| --- | --- | --- |
| GPU | `gpu_production_owner` | [GPU production handoff](workstreams/next-gpu-production.md): text, textures, masks, blends, transitions, shared production preview/export and Metal measurements |
| Characters and animation | `character_animation_owner` | [Character/animation handoff](workstreams/next-character-animation.md): custom rigs, textured parts, deformation, idle, general mixing and cross-project libraries |
| Artist tools | `artist_tools_owner` | [Artist tools handoff](workstreams/next-artist-tools.md): groups, image masks, polygon feathering, painting, pressure and retouching |
| Speech and lip sync | `speech_quality_owner` | [Speech/quality handoff](workstreams/next-speech-quality.md): verified real environment, automatic reuse, timing editing, forced alignment and real-audio quality |
| Screenplay and timeline | `screenplay_timeline_owner` | [Screenplay/timeline handoff](workstreams/next-screenplay-timeline.md): audio cues, reusable/conditional rules, camera presets and general compiled-track editing |
| Performance and architecture | `performance_architecture_owner` | [Performance/architecture handoff](workstreams/next-performance-architecture.md): benchmarks, UI latency, process memory and dependency scheduling |
| Mac acceptance | `mac_device_owner` | [Mac acceptance handoff](workstreams/next-mac-device.md): offline speech, devices, synchronization, Finder, shortcuts, accessibility and full workflow evidence |
| Release and licenses | `release_license_owner` | [Release/license handoff](workstreams/next-release-license.md): exact components/assets, obligations/notices, production package, signing and notarization |
| Integration and QA | Parent agent | Shared schema/render/desktop contracts, non-overlapping edits, actual checks, review, progress and release synchronization |

- [x] Complete all eight owner inspections and publish linked handoffs.
- [ ] Implement and validate the next packages; keep feature checkboxes open until
  integration and relevant checks succeed.
- [ ] Collect external physical-device, real-audio and final upstream license
  evidence; source inspection and synthetic tests cannot replace it.

Owners describe responsibilities, not completed acceptance. Agent assignment is
maintained by the parent; shared files are changed only by their assigned owner.

## 1. Architecture and technical lead

- [x] Actual Rust workspace, lockfile and narrow working crates.
- [x] Repository engineering/independence rules in `AGENTS.md`.
- [x] Shared typed MotionScript, scene evaluation and audio provider boundaries.
- [x] Deterministic scene evaluation shared by desktop and SVG export.
- [x] Local core workflow without mandatory cloud service.
- [ ] Full document/evaluation/UI separation across all planned tools.
- [ ] Architecture decision records covering the final renderer/job architecture.
- [ ] Final dependency/license audit and macOS-first acceptance.

## 2. MotionScript compiler

- [x] Typed English YAML, syntax parsing and semantic diagnostics.
- [x] Project, characters, voices and multiple scene definitions.
- [x] Actor references, finite times, movement bounds and duration rules.
- [x] Move, emotion, gesture and speak commands with deterministic evaluation.
- [x] Explicit unsupported-command errors and documented examples.
- [x] Atomic event/voice/settings source edits and tests.
- [x] Scene add/duplicate/delete and character add/delete with reference/final-scene protection.
- [x] Validated scene camera keyframes and deterministic pan/zoom evaluation.
- [x] Explicit push/pull/pan camera presets expanded into existing editable keys (PR #55).
- [ ] Camera event commands and reusable/conditional cinematic rules.
- [x] Timed solid background-color tracks shared by preview and export.
- [x] Timed background image placement shared by native preview and export.
- [x] Validated outgoing cuts/crossfades, compiled sample timeline and headless PNG/WAV sequence export.
- [x] Native whole-project sequence preview/transport and PNG/MP4 sequence export.
- [x] SVG sequence compositing (PR #33).
- [ ] Audio cue commands distinct from dialogue import.
- [ ] Reusable/conditional rules and extensible animation presets.
- [x] Versioned, validated EventLog of external input replayed deterministically through rule expansion (PR #106).
- [x] Versioned editable scene-order/duration document compiled through the shared sequence clock (PR #43).
- [x] Versioned seq/par/stagger schedules compiled to editable events with located, typed diagnostics (PR #105).
- [x] Resolved Abs/Rel/Revert tracks with defined overlap order, shared s ↔ sample ↔ frame rules and bit-identical seek vs playback (PR #107).
- [ ] General versioned editable compiled event/animation tracks.

## 3. Character rigging and animation

- [x] Original procedural Emma/Alex preview characters.
- [x] Deterministic position interpolation and frame evaluation.
- [x] Basic expressions and wave gesture.
- [x] Editable rigid character definitions (PR #46) and custom-rig joint tracks/editor (PR #50).
- [ ] Textured sprite parts and deformation.
- [x] Hierarchical transforms, bones and joints (`motion-rig`).
- [x] Pose evaluation and original seven-joint procedural rig shared by preview/export.
- [x] Deterministic procedural walk/wave presets and static rest pose.
- [ ] Expressive idle cycles and extensible artist-authored motion presets.
- [x] Validated transform keyframe interpolation API: linear/smoothstep/hold and shortest rotation.
- [x] Persistent per-scene actor root transform tracks and detached keyframe/easing editor.
- [x] Headless per-joint PoseTrack sampler and validated weighted pose-blending API.
- [x] Rule-based keyframe clip library in shared preview/export: emotion posture blending, walk, wave/point/nod/shrug gestures and viseme-driven speech head motion.
- [x] Persisted parent-local per-bone tracks and detached editor in shared preview/export.
- [x] Versioned editable custom rig hierarchy and colored rigid polygon parts, native definition editor, persistence and shared root/rest rendering (PR #46).
- [x] Custom-rig joint animation tracks and detached native joint editor (PR #50 via #51).
- [ ] Custom-rig textured parts and deformation.
- [x] Authored bone offsets layer over rule clips in shared evaluation.
- [x] Artist-controlled per-scene/actor posture, walk and gesture strengths with shared evaluation (PR #35).
- [x] Scene-local animated procedural strength tracks with outgoing easing and native editor (PR #36).
- [x] Named project-local authored bone clip library with capture/explicit overwrite, inspection, copied-track application and deletion (PR #37).
- [x] Explicit copied-clip start/duration retiming with native controls, atomic validation and shared tracks (PR #38).
- [x] Persistent original-rig live clip instances, half-open windows and constant-weight ordered local-offset override blending (PR #40).
- [x] Per-instance animated weight curves, linear fades, shared sampling and native authoring (PR #41).
- [x] Live clip original-speed looping and explicit original-rig joint selection, including native controls and shared evaluation (PR #43).
- [x] Explicit original-rig additive live offsets with checked accumulation and default-compatible override (PR #45).
- [x] Explicit normalized original-rig override groups with per-joint residuals and continuous authored-base-anchored rotations (PR #46).
- [x] Versioned original-rig clip clipboard exchange between projects with explicit replacement (PR #55).
- [ ] Arbitrary-rig pose mixing, custom-rig clip exchange and retargeting.
- [x] Integrated rig/animation transform, seek and interpolation tests.

## 4. Graphics and rendering

- [x] Native animated scene preview with scene-coordinate scaling.
- [x] Shared evaluated scene and deterministic SVG frame renderer.
- [x] Basic background and original procedural character composition.
- [x] Bounded PNG/JPEG foreground image layers shared by native preview and SVG/PNG/MP4.
- [ ] Textured per-bone character sprites and deformation.
- [x] Actor z-order/visibility composition metadata and editor.
- [x] Persisted ordered/visible/opacity image and basic drawing layers with rectangular clip masks.
- [x] Validated ellipse/simple-polygon masks and normal/multiply/screen shared CPU blending.
- [x] Persisted soft animated ellipse masks in shared SVG/PNG/native preview and sequence evaluation (PR #43).
- [x] Bounded isolated nested artwork groups with explicit sibling order, native history controls and shared CPU composition (PR #46).
- [x] Image alpha/luminance masks and polygon feathering (PRs #48–#49 via #51).
- [x] Scene-local image-mask rectangle animation, explicit native key Apply and shared CPU/GPU sampling (PR #55).
- [x] Vector mask painting/erasing with shared history and CPU preview/export (PR #53).
- [x] Soft mask brushes, optional actual touch-force samples and experimental painted-mask GPU commands (PR #54).
- [ ] Mac stylus input, animated painting, retouching, measured GPU mask parity and physical Mac compositing acceptance.
- [x] Selected-scene shared CPU-composited advanced frames and final integration checks (PR #31).
- [x] Camera positioning/zoom shared by native preview and SVG/PNG/MP4.
- [x] Solid background color, timed step-color tracks and scene-edge fades to black.
- [x] Timed background image planes and validated half-open visibility intervals.
- [x] Whole opaque-frame cuts/crossfades in headless PNG sequence export.
- [x] Shared scaled raster transition preview and sequence MP4 compositing.
- [x] SVG project sequence compositing (PR #33).
- [x] Default-disabled evaluated-scene geometry GPU experiment with explicit label omissions, work bounds and decoded RGBA diagnostics (PR #43).
- [x] Expanded experimental GPU images, rigid parts, existing masks, isolated groups, backdrop blends and whole-frame crossfades; replayable project/sequence probe (PR #46).
- [ ] Verified production GPU preview.
- [x] Optional genuine offscreen GPU fixture, shader validation, bounded readback and explicit backend/device metadata.
- [ ] Full-scene GPU renderer/parity and actual macOS Metal hardware evidence.
- [x] Composition SVG golden fixture plus shared geometry/clock regression tests.
- [ ] Wider image/texture golden coverage as rendering grows.

## 5. Desktop UI/UX

- [x] Screenplay editor, preview, timeline and Inspector panels.
- [x] Play/pause/restart, scrubbing and time/frame display.
- [x] Validation diagnostics, file open/save and dark interface.
- [x] Selected event and character voice editing.
- [x] Timeline event retiming, add/duplicate/delete and source Undo/Redo.
- [x] Project title/fps/dimensions and scene ID/duration drafts.
- [x] Scene/character structure controls, confirmed deletion and source Undo.
- [x] `.kilsat` exact-source persistence and portable asset inventory preservation.
- [x] Explicit v1→v2 migration preserving source/assets and persisting typed artwork.
- [x] Bounded unified source/artwork/asset-inventory history and source/reference guards.
- [x] SVG/PNG/MP4 background export selection and explicit FFmpeg path.
- [x] Audio waveform display and prepared-dialogue preview controls.
- [x] Explicit local asset import/relink/remove-reference UI and bounded worker.
- [x] `.kilsat` inventory dirty tracking and chronological metadata-only Undo/Redo.
- [x] Background PNG/JPEG decoding, texture loading and foreground image rendering.
- [x] Native cached image thumbnails with aspect-fit and virtualized asset rows.
- [x] Asset inventory operations participate in the global document history; physical files are retained.
- [x] Actor layer order/visibility and composition camera-key editor.
- [x] Character root-transform keyframe editor plus pen/rectangle/ellipse canvas tools and rectangular masks.
- [x] Native detached per-bone editor and validated ellipse/polygon mask/blend controls.
- [x] Native procedural layer strength editor with Apply, unified Undo/Redo and project persistence (PR #35).
- [x] Animated procedural strength key editor with timing/easing, explicit Apply and safe track removal (PR #36).
- [x] Native project clip library with detached names/selection, explicit replacement/deletion confirmations and unified source history (PR #37).
- [x] Opt-in clip start/duration controls, explicit reset and replacement confirmation after timing edits (PR #38).
- [x] Native live-instance editor with saved layer order, source history, reference warnings and cached shared preview (PR #40).
- [x] Detached fade/key/easing editor with invalid draft preservation, Apply, reset and Undo/Redo (PR #41).
- [x] Native detached soft ellipse mask/key authoring with atomic Apply and shared history (PR #43).
- [x] Native stable-ID scene reorder actions with validated transitions and source Undo/Redo (PR #43).
- [x] Native override/additive live-instance mode control with Apply, reset preservation and shared history (PR #45).
- [x] Native normalized clip groups, custom rigid-character definition editing and nested artwork group controls through existing history (PR #46).
- [x] Soft mask painting and optional pressure samples supplied by actual input events (PR #54).
- [ ] Physical Mac stylus support, animated mask painting and image retouching.
- [x] Camera controls, timed background-color editor and scene-edge fades.
- [x] Outgoing Cut/Crossfade editor with atomic settings Undo and invalid-overlap rejection.
- [x] Whole-project sequence preview/playback, global sample seek, waveform and PNG/MP4 export controls.
- [ ] Real Mac sequence playback/synchronization and end-to-end acceptance.
- [x] Chronological document history across source, artwork and inventory.
- [ ] Comprehensive Mac shortcut/accessibility acceptance.
- [ ] Comprehensive interactive macOS workflow acceptance.

## 6. Audio, Kokoro and lip sync

- [x] Replaceable Rust SpeechProvider with strict requests/results/errors.
- [x] Per-character Kokoro preset/language/speed configuration.
- [x] Local CPU Python sidecar protocol and actionable missing-runtime errors.
- [x] Content-addressed synthesis cache and integrity checks.
- [x] Imported WAV fallback and explicit alignment provenance.
- [x] Dialogue jobs, sample-clock placement, mixing and overlap checks.
- [x] Device playback integration and waveform UI implementation.
- [x] Separate imported/manual alignment and timed viseme evaluation.
- [x] Clear distinction between facial emotion and acoustic conditioning.
- [x] WAV-bound local acceptance reports and manual/candidate alignment error metrics.
- [x] Opt-in bounded Rhubarb CLI backend and exact-PCM estimated-cue import into line/scene/sequence preparation.
- [ ] Actual Rhubarb engine/device/corpus quality acceptance; stub tests do not establish these.
- [x] Consented managed Kokoro provisioning with pinned asset revisions, sizes and SHA256.
- [x] Reproducible Cargo/Python/model/voice/binary evidence inventory with fail-closed unresolved gate.
- [x] Final signed preview ZIP/member inventory, exact package resources and CI sidecar (PR #42).
- [x] Offline artifact-bound license corpus/review-gap and Mac observation reports with bounded inputs and no automatic approval (PR #46).
- [ ] Complete artifact-specific dependency/asset license and redistribution audit.
- [x] Real local English Kokoro CPU WAV on Apple Silicon (user report 2026-10-16: M1, macOS 27.1, both voices; see [acceptance](kokoro-in-app-acceptance.md#recorded-mac-evidence)).
- [ ] Offline synthesis after provisioning with networking disabled.
- [x] Exact-PCM/transcript/provider/config/manual-edit alignment cache with atomic writes and headless import workflow.
- [x] Exact-PCM/current-text bound alignment envelopes and shared line/scene desktop cache integration.
- [x] Native explicit estimated-aligner execution controls (PR #31).
- [x] Explicit saved estimated-cue cache and native exact-PCM/text reload (PR #34).
- [x] Immutable exact-request engine-index foundation with private eligibility, corruption/conflict/cancellation checks and explicit result origin (PR #43).
- [x] Automatic lookup/miss/publication and fresh bypass for explicitly trusted self-contained local Phonetic packages with exact package/runtime identity and controlled execution (PR #46).
- [ ] Actual supplied recognizer compatibility, audited complete environment and real-audio quality acceptance.
- [x] Estimated cues preserve separate provenance and integer-sample trim/offset evaluation in preview/export.
- [x] Bounded sample/shape edits from reviewed estimates, exact-audio manual provenance, project save and shared Undo/Redo (PR #55).
- [ ] Direct reopening/editing of applied manual envelopes, waveform dragging and validated forced-aligner integration.
- [ ] Annotated real-audio corpus and measured timing error.
- [ ] Perceptual mouth-quality assessment across voices/speeds/pauses.
- [ ] Physical-device seek/pause/offset/trim/overlap synchronization checks.
- [ ] Synchronized encoded-video acceptance with real speech.
- [ ] Evaluated ONNX/Core ML alternatives with exact tested versions.

## 7. Export and media pipeline

- [x] Frame-accurate deterministic SVG sequence export and CLI examples.
- [x] WAV mix plus synchronized SVG/audio manifest bundle.
- [x] Export progress, cancellation and recoverable error reporting.
- [x] PNG sequence export, including decoded 1080p dimension validation.
- [x] Explicit local FFmpeg H.264/AAC encoding from the prepared preview mix.
- [x] Explicit encoder availability, invocation and external distribution/license boundary documented.
- [ ] Complete redistribution audit before bundling any encoder or dependency notices.
- [x] Real encoder/ffprobe frame count and duration checks for integer/fractional scenes.
- [ ] Long-run audiovisual drift and perceptual sync with real speech.
- [ ] Export acceptance with generated Kokoro and imported dialogue.

## 8. Concurrency and performance

- [x] Background speech preparation/export work with UI status.
- [x] Existing bounded speech worker and cancellation controls.
- [x] `motion-jobs` fixed scoped pool, bounded queues/batches and ordered consumption used by export.
- [ ] Explicit dependency-DAG scheduling beyond frame batches.
- [x] Background asset copying/relink checks and bounded parallel frame rendering.
- [x] Background-thread image decoding and native texture upload.
- [ ] Background scene-image replacement and texture-rig loading.
- [x] Estimated raster frame working-set budget limits worker count; byte/work caps remain.
- [ ] Process-wide measured memory/resource budgeting across all workers.
- [x] Ordered 10,000-job stress, worker cap, cancellation/failure/panic and byte-identical serial/parallel tests.
- [x] Runnable JSON export benchmark with debug Linux smoke measurement.
- [ ] Production release-profile and Apple Silicon benchmark series.
- [ ] UI responsiveness measurements under real workloads.
- [x] Opt-in bounded elapsed application-update observations and copyable tracked-job summaries (PR #55); this is instrumentation, not responsiveness acceptance.

## 9. QA and release

- [x] Rust tests and Linux/macOS CI workflows present.
- [x] Sidecar protocol/error/diagnostic unit tests present.
- [x] Sample projects, build/run and practical-test documentation.
- [x] Native preview `.app` staging, ad-hoc signing and ZIP workflow.
- [x] Original-source MIT LICENSE, inherited Cargo metadata and byte-exact license in app resources/ZIP root (PR #36).
- [x] Integrated direct rustfmt, cargo check, strict Clippy and all-target Rust tests run.
- [x] Python runtime and package tests run; actual FFmpeg test run separately.
- [x] Original generated ICNS and Editor/Alternate file-type metadata structurally tested.
- [ ] Verify icon and Launch Services behavior on an actual Mac.
- [ ] Finder document event handling and open/save round-trip acceptance.
- [ ] Real Apple Silicon synthesis and device playback evidence.
- [ ] Production release packaging and release checklist acceptance.
- [ ] Optional Developer ID signing/notarization decision and validation.

## 10. Research and documentation

- [x] MotionScript, audio, UI, export and practical testing guides.
- [x] Kokoro dependency review with explicit outstanding obligations.
- [x] Development stages distinguish implementation from hardware acceptance.
- [x] Rigging, versioned project format and CPU media-export implementation guides.
- [x] Shared scheduler, resource estimates and benchmark design documented.
- [ ] Final GPU renderer decision and actual implementation.
- [ ] Forced-alignment quality research and documented corpus results.
- [ ] Exact pinned asset/dependency redistribution review and notices.
- [x] Local CPU raster/FFmpeg design, limits and integer/fractional test results documented.
- [ ] Production rendering performance measurements.
- [ ] Complete artist-facing documentation for remaining product tools.

## Whole-product milestone gates

These gates remain open until all constituent work and acceptance are evidenced.

- [ ] **M1 — First application:** Rust/parser/sample/deterministic preview/editor/
  transport/save/load/tests implemented and integrated checks passed; comprehensive
  interactive Mac acceptance still needs attachment. Prior user report
  established app launch only.
- [ ] **M2 — Character animation:** hierarchy, rigging, keyframes/easing, presets,
  expressions/gestures and blending integrated and tested; procedural rig and
  interpolation, root/per-bone editors and additive authored layering are implemented;
  textured custom rigs, general mixing and physical motion acceptance remain open.
- [ ] **M3 — Dialogue/lip sync:** import/tracks/mouth/timing foundations implemented;
  reported real M1 Kokoro speech and estimated-cue import are recorded; complete
  Speech Stages 2–4 and quality/hardware gates remain open.
- [ ] **M4 — Professional editing:** timeline and source Undo/Redo implemented;
  actor order/visibility, camera tracks and inventory tools implemented; transform
  root/per-bone keys, geometric masks, CPU blend modes, unified document history
  and transitions are implemented; advanced assets/retouching and full native
  compositor parity remain open.
- [ ] **M5 — Production rendering:** CPU SVG/PNG/MP4 and bounded parallel frames
  implemented/tested; optional GPU fixture compiles and validates shaders without
  a device. Full GPU scene parity, production performance and hardware acceptance
  remain open.
- [ ] **M6 — macOS distribution:** preview bundle, generated icon and metadata implemented; actual Finder,
  release packaging and Apple Silicon release acceptance remain open.

## Five speech acceptance gates

- [x] **Speech 1 — Foundation:** provider contract, voice schema and strict tests
  present in inspected baseline; this check does not imply real inference.
- [ ] **Speech 2 — Local synthesis:** consented/pinned assets, real macOS CPU WAV,
  Apple Silicon evidence and offline repeatability.
- [ ] **Speech 3 — Timeline:** implemented jobs/sample placement/playback integration;
  physical-device timing acceptance still pending.
- [ ] **Speech 4 — Mouth:** implemented alignment import/timed mouths; measured and
  perceptual quality acceptance still pending.
- [ ] **Speech 5 — Production:** implemented cache/voice settings/waveform/mix;
  complete timing editor, preset preview and synchronized-video acceptance pending.

## Completion log

| Date | Item | Evidence and scope |
| --- | --- | --- |
| 2026-10-15 | Baseline inspection | Read master prompt, AGENTS, workspace, existing source and development-stage docs. Existing features checked above; concurrent new modules deliberately remain unchecked. |
| 2026-10-15 | Packaging implementation | Original generated ICNS and alternate Editor document declarations implemented; `python3 -m unittest discover -s tests -p test_package_macos.py -v`: 8 tests passed. Integration/macOS validation still pending. |

When closing an item, append commit/PR, exact executed check and remaining platform
limitations here. Do not mark whole milestones complete from module presence.


| Date | Item | Evidence and scope |
| --- | --- | --- |
| 2026-10-16 | PR #18 integrated implementation | `.kilsat` persistence, source lifecycle, shared procedural rig/keyframe API, desktop controls, PNG/MP4 and speech-evidence tooling implemented. Six agents: media, rig, project, desktop, speech, tracker_release. |
| 2026-10-16 | Local validation | 150 Rust tests passed, one real-encoder test separately passed (1.0s and0.9s cases), 29 runtime +8 packaging Python tests passed; strict Clippy and direct rustfmt passed. Linux evidence only; GitHub CI result belongs to its exact head commit. |

Master prompt completion remains **open**. Per-bone sprite rendering, advanced drawing/masks, true cross-scene transitions,
custom rig/per-bone editor, blending, automatic forced alignment,
GPU rendering, production performance evidence, final distribution and
real Mac/model/device/perceptual acceptance are still unchecked above. No agent
continues running after the reported task completes; future increments must be
explicitly resumed and validated.


## Composition increment — completed implementation checks

Six agents inspected and implemented this pass: composition, assets,
desktop_compose, jobs, alignment_cache and acceptance_review. The attached
tracker was the input; this repository copy is the maintained implementation
record. PR #18 Linux/macOS CI passed at aa2f137; it and #19 were subsequently
merged after approval (main/release0866e31).

Library/headless alignment caching is complete; automatic forced alignment and
desktop cache integration remain separate. Imported images are inventory files,
not rendered character textures. Camera fades do not implement scene transitions.


| Date | Completed implementation | Actual evidence |
| --- | --- | --- |
| 2026-10-16 | Camera/layer/fade composition and native controls | Shared evaluator, clipped SVG golden, atomic invalid-edit rollback and source Undo tests. |
| 2026-10-16 | Asset import/relink/reference removal and UI | Bounded background copy, canonical-root and stale-result guards, inventory dirty/one-step Undo; source files preserved. |
| 2026-10-16 | Bounded parallel frame jobs and benchmark | Serial/parallel PNG/SVG byte equivalence, ordered stress/cancel/panic tests, documented debug Linux measurement. |
| 2026-10-16 | Exact-audio alignment cache | PCM/transcript/config/edit invalidation, integrity, provenance, concurrent writes and offline example; no automatic aligner or desktop preparation integration. |
| 2026-10-16 | Integrated local validation | 169 Rust tests passed; separate real FFmpeg/ffprobe test passed; strict workspace Clippy, cargo check, all46-file direct rustfmt and diff check passed. |

Remaining items stay unchecked. That composition increment did not supply artwork or actor-keyframe tools;
the subsequent artwork increment below implements the described basic subset.
Per-bone rig parts, advanced masks, full unified project history, true transitions,
automatic alignment, GPU rendering and hardware/model acceptance remain open.


## Artwork and actor keyframes increment

PRs #18 and #19 were merged in order after explicit approval; both had passing
Linux/macOS CI. `main` and `kilsatstudio1.0` were synchronized to0866e31.

Six agents implement this increment: art_model, keyframe_core, art_render,
desktop_art, keyframe_ui and art_qa. Component tests cover v1/v2 migration, image
limits, typed edits, root interpolation, raster pixels/masks, and referenced
source/artwork Undo safety. Final integrated results are recorded below.

Scope: artwork is foreground document order; rectangular masks and translations
for actor attachments are supported. Artwork attachments do not inherit root
rotation/scale; procedural actors do. This is not textured bone rigging, a full
image editor, unified project history, GPU export or real Mac acceptance.


| Date | Artwork increment completion | Evidence |
| --- | --- | --- |
| 2026-10-16 | Persisted root keyframes/editor and artwork v2 | 191 workspace/all-target Rust tests passed, including gesture/history/refguards, migration, PNG/JPEG, interpolation, raster and clip tests. |
| 2026-10-16 | Actual artwork media pipeline | Two explicit FFmpeg tests passed, including embedded artwork→PNG→H.264/AAC MP4 and integer/fractional media timing. |
| 2026-10-16 | Code checks | Workspace check, strict installed-driver Clippy, direct rustfmt for51 Rust files and diff check passed. Linux local evidence; new GitHub CI applies to its exact head. |

The current release acceptance gates remain open: no actual Mac GUI/device test,
real Kokoro inference, GPU rendering or perceptual mouth-sync acceptance was
performed for this artwork increment.

## Managed Kokoro desktop setup increment

- [x] PR #20 merged with approval; main and kilsatstudio1.0 synchronized to
  6634e68190b3588e58719c659bca99ff11e346bf.
- [x] Desktop consent, isolated setup, progress/cancel and existing-profile connection.
- [x] Verified model/voice/language asset manifest and packaged setup helpers.
- [x] Separate English CPU speech-test and playback controls without document edits.
- [x] Runtime Python tests: 38 passed; macOS packaging unit tests: 8 passed.
- [ ] Actual Apple Silicon installation, synthesis, listening and device acceptance.
- [ ] Complete transitive dependency lock and redistribution/license audit.

Compatible host Python 3.10–3.12 is required. Model assets and Python are not
bundled. No model installation or real inference was performed in development.
See managed-kokoro.md and kokoro-in-app-acceptance.md for the acceptance procedure.

## Timed background colors increment

PR #21 was merged with approval as 3448fb031a6bb3953e09f893874952fa8c77427d;
`main` and `kilsatstudio1.0` were synchronized. Its Linux/macOS CI and Apple
Silicon preview packaging passed. Real Kokoro synthesis/listening remains open.

This increment implements bounded, validated scene-local background color keys,
inclusive step evaluation, native draft editing and shared preview/SVG/PNG/MP4
behavior. It covers solid colors; background images and cross-scene transitions
remain open. Core, desktop and independent review agents contributed; the parent
owns export regressions, integration and completion marking. Validation evidence
is recorded in the pull request after final checks.

| Date | Completed increment | Evidence and scope |
| --- | --- | --- |
| 2026-10-16 | Timed solid background colors, PR #22 | 205 Rust workspace/all-target tests passed locally; decoded PNG/SVG color boundary coverage; strict installed-driver Clippy/workspace check, direct rustfmt and diff check passed. |
| 2026-10-16 | Atomic settings Apply | Shrink/remove, shrink/retime, extend/add and exact source Undo regressions passed; independent review blocker resolved. Real Mac GUI and background-image/transition work remain open. |

## Timed background images increment

PR #22 review completed without findings. Its final head572d779 passed all CI;
205 Rust tests passed locally. The merge status above records completed and
remaining work. New work adds persisted background/foreground artwork planes and
optional scene-local visibility intervals; final evidence follows in the PR.

| Date | Completed implementation | Actual evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Background/foreground artwork planes and timed images | 217 Rust workspace/all-target tests passed locally, including legacy v2 defaults, interval bounds, invalid atomic edits, Undo, shared SVG ordering and decoded PNG frame pixels. |
| 2026-10-16 | Composition fade parity and future-resource preflight | Independent review found the native fade mismatch; one final canvas fade now follows both planes/actors. Shape-order tests and future-image/duration preflight tests passed. No physical Mac GUI test claimed. |

Cross-scene transitions, textured per-bone rigs, advanced masks/blending, unified
history, forced alignment/quality, GPU rendering, production benchmarks, license
audits and actual Mac/Kokoro/device acceptance remain open.

Final local checks: direct rustfmt and diff checks passed; strict installed-driver
Clippy/workspace all-target check passed. The full 20-test motion-export suite
also passed with actual /usr/bin/ffmpeg and /usr/bin/ffprobe, including both
normally ignored real-media tests. GitHub CI applies only to its exact head.

## Open workstream assignment and results

The user requested six agents for the remaining areas on 2026-10-16. Each
completed the bounded task below; these are not claims that the whole feature or
acceptance gate is finished. Reports are implementation contracts, not completed
product features.

| Agent | Delivered scope | Still open |
| --- | --- | --- |
| bone_animation | Local per-joint PoseTrack sampling, weighted pose blend API, tests and runnable example; [details](workstreams/animation.md) | Persistence, per-bone editor, textured rigs and preview/export integration |
| transition_workstream | [Cross-scene clock, composition, audio and test contract](workstreams/transitions.md) | Scene transition implementation and acceptance |
| mask_workstream | [Masks and blend-mode implementation contract](workstreams/masks-blending.md) | Ellipse/arbitrary masks, isolated compositing and blend modes |
| history_workstream | [Whole-project transaction/history migration](workstreams/unified-history.md) | Shared bounded history across source, art and inventory |
| alignment_workstream | [Exact-audio import/cache integration plan](workstreams/alignment.md) | Desktop identity-bound import, automatic backend and measured quality |
| gpu_release_workstream | [GPU proof, Mac/device and license evidence gaps](workstreams/gpu-release-gates.md) | Offscreen Metal path, physical-device/offline Kokoro acceptance and full license audit |

Current code increment is a headless animation foundation. Do not mark full
per-bone rigging, animation blending, masks, transitions, alignment, GPU or Mac
acceptance complete based on these APIs or reports. The exact final validation
is recorded below and in its pull request.

| Date | Completion evidence | Remaining scope |
| --- | --- | --- |
| 2026-10-16 | Six agent workstream audits delivered in docs/workstreams; 220 Rust workspace/all-target tests passed after the new animation APIs. Strict installed-driver Clippy/workspace check, direct rustfmt and diff checks passed; blend_pose example executed with weights 0/0.5/1. | Library foundation only; full per-bone editing and integrated animation blending remain open. |

PR #23 head7bdf5f1 passed Linux/macOS CI, media encoding and Apple Silicon
packaging; GitHub code review completed without findings. PRs #23 and #24
were subsequently merged in order, with the latter retargeted to main.

## Exact-audio alignment and desktop cache increment

Merge status is refreshed above and in each merged PR description. PR #23
completed timed background images and composition fade parity; PR #24 completed
headless joint-pose APIs and six scoped workstream reports. Neither merge closes
physical Mac/model acceptance, full per-bone editing, transition, mask, unified
history, automatic alignment or GPU milestones.

This bounded implementation binds supplied timings to the decoded PCM and
current transcript, adds explicit bound/cache/legacy status, and reuses the
existing local cache from line/scene background preparation. Core, voice and
desktop agents owned separate files; the parent integrated and verified them.
No automatic timing generation, model installation or measured quality is implied.

| Date | Completed increment | Executed evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Bound alignment envelope, line/scene cache resolution and visible statuses | 235 Rust workspace/all-target tests passed locally; strict installed-driver Clippy/workspace check, direct rustfmt and diff checks passed. |
| 2026-10-16 | Exact audio/text rejection and immutable concurrent cache publication | Same-length different PCM/current-text mismatch, cache-only hit/miss, corruption, legacy one-sample tolerance, competing conflicting writers and single/scene parity regressions passed. |
| 2026-10-16 | Preview invalidation and review fixes | Selected timing replacement/clear invalidates prior preview; cache no-replace publication reviewed. Independent review confirmed both blockers resolved. |
| 2026-10-16 | Bound import example | Executed on a synthetic local silent WAV and synthetic phone fixture; creates a strict envelope without inference/downloads. This is not measured real speech. |

Automatic alignment, phone-boundary editing, real-audio corpus quality, physical
Mac synchronization and actual Kokoro/offline synthesis remain open alongside
transitions, full per-bone tooling, advanced compositing, unified history, GPU
rendering, production performance and final license/distribution audit.

## Rule-based character animation increment

PR #26 replaces the hand-written humanoid pose math with an additive keyframe
clip library in `motion-animation`. Emotion postures blend over 0.25 s with
`blend_pose`, a keyframed walk cycle plays during moves, `wave`, `point`, `nod`
and `shrug` gestures play under a sine in/out envelope, and
`motion_scene::apply_speech` dips the head by mouth openness from the same
per-actor visemes the mouth renders, in desktop preview and frame export.
Unknown gestures remain explicit unsupported errors. The master build prompt
now also describes Rhubarb Lip Sync as a candidate offline mouth-timing backend;
it is documentation only, not an implemented or evaluated integration.

| Date | Completed increment | Executed evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Clip library, posture/gesture/walk layers and speech head motion | Local Linux: `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` passed on the merged head with PR #25 (238 tests passed, 2 ignored, including the interrupted posture-blend regression). No Mac GUI run, visual motion review or perceptual speech-motion check. |

Persisted or artist-authored clips, per-bone editing, textured parts, a Rhubarb
or other automatic timing backend and measured mouth-timing quality remain open.

## First real Kokoro speech on a Mac

| Date | Evidence | Remaining |
| --- | --- | --- |
| 2026-10-16 | The user installed Kokoro through the in-app managed setup on an Apple M1 running macOS 27.1, using the preview built from `main` at 98d265b with Homebrew Python 3.12. They ran the English CPU speech test for `af_heart` and `am_michael` and reported that it works. | Offline synthesis with networking disabled, the remaining in-app acceptance checklist, real scene dialogue playback/export and the Speech 2 gate. |

## Scene sequence implementation increment (PR #28)

PR #28 added validated outgoing cuts/crossfades, a shared global 24 kHz
timeline, matching audio mixing and bounded headless PNG/WAV sequence export.
Inspector authoring is included; native preview/transport and scene-export buttons
remain scene-local. Full native sequence preview and MP4/SVG sequence export stay
open. See [guide](scene-sequences.md) and [sample](../examples/scene-sequence.yaml).

The initial uncommitted sequence work was lost when the transient workspace reset.
Four agents restored their implementations additively on main0ab1ba1, preserving
the new rule-based animation and user Mac evidence. The restored source was
checkpointed immediately in PR #28 before revalidation. Old incomplete test logs
are not reused as proof; final results for this head are recorded below.

| Date | Completed implementation | Actual validation and remaining scope |
| --- | --- | --- |
| 2026-10-16 | Shared sequence timeline, outgoing transition editor, linear audio and whole-frame PNG crossfades | Restored source head f6c62bdd passed GitHub Linux/macOS fmt/check/strict Clippy/all-target tests, runtime38 + packaging8 Python tests, real media encoding and Apple Silicon preview packaging. Linux job logs contain261 passed Rust tests, two encoder tests separately covered by media CI. |
| 2026-10-16 | Recovery review and local reproduction | Independent read-only re-review found no blocker; current apply_speech path and PR26/27 changes preserved. Locally4 sequence export tests passed. Recovered CLI fixture produced6 PNG frames,36000 audio samples and correct overlap midpoint PCM; synthetic tones only, no real speech inference. |

The checked scope at PR #28 was authoring plus headless PNG/WAV sequence export.
Native whole-project preview/transport, MP4 sequence export and CLI `.kilsat`
resource loading were open at that merge and are addressed by PR #29 below. SVG
sequence compositing remains open. Per-bone editing/textured rigs, advanced
compositing, unified
history, automatic alignment/quality, offline/device acceptance, GPU/performance
and final licensing/distribution gates also remain open. User-reported first
real M1 Kokoro success (PR27) remains distinct from those comprehensive gates.

## Whole-project desktop increment (PR #29)

- [x] Merge PR #28 and synchronize the release branch; retain all earlier Mac evidence.
- [x] Integrate and validate native whole-project preview, global playback/seek and waveform.
- [x] Integrate and validate shared scaled raster preview and PNG/MP4 sequence export.
- [x] Validate .kilsat sequence CLI resource loading with canonical image bounds.
- [x] Integrate and validate asset thumbnails and virtualized image inventory.

Four agents implemented these scopes. Parent validation passed all 274 Rust tests
(three explicit encoder tests ignored by default), direct rustfmt, all-target check
and strict Clippy. Seven sequence tests including the explicit real FFmpeg test
passed. All three opt-in real-encoder tests also passed; sequence encoding is now
covered by the media CI job. Preparation snapshots are created only on explicit
Prepare clicks; the final desktop 98 tests and strict Clippy passed after that
UI optimization. [Mac instructions](practical-testing.md) include the new pending
whole-project device/offline/transition acceptance checklist. A synthetic .kilsat
fixture exported 6 PNGs and a 1.5 s MP4 with 36000 audio samples; decoded PNG artwork matched the expected green pixel. Traversal input
was rejected. Independent review found EOF retry, paused-seek repaint and competing
playback bugs; fixes and regression tests are included. Implementation head
`6fb9696798bb31c222d5264dee3898b80526dcda` passed GitHub Linux/macOS
fmt/check/strict Clippy/all-target Rust tests, runtime and packaging Python tests,
and all three real media tests (Audio foundation run 38918205145). Apple Silicon
preview build/package/signature/helper checks and artifact upload also passed
(run 38918205156). This final tracker edit changes documentation only. These are
implementation checks.
At the PR #29 checkpoint, real Mac sequence/offline/perceptual acceptance,
unified history, per-bone editing, advanced composition, GPU and final licensing
were still open. The implementation slices completed next are recorded below;
physical acceptance and full product gates are not implied.

## Artist tools and offline pipeline increment (PR #30)

- [x] Merge PR #29, synchronize release branch and record actual checks.
- [x] Integrate persisted per-bone editor and shared evaluation, preserving rule clips.
- [x] Integrate chronological source/artwork/asset inventory Undo/Redo.
- [x] Integrate validated ellipse/polygon masks and verified multiply/screen blending.
- [x] Integrate bounded offline Rhubarb estimated cues and exact-PCM import.
- [x] Integrate optional offscreen GPU fixture; full scene GPU parity stays open.
- [x] Integrate reproducible dependency evidence inventory and strict unknown gate.
- [ ] Real Mac Metal, sequence/audio/offline and perceptual mouth acceptance.
- [ ] Full dependency/voice redistribution audit and reviewed third-party notices.

Parent integrated 244 focused Rust tests (108 desktop), workspace/all-target
check and strict Clippy with local Rust 1.90. The optional GPU library passed
14 adapter-free tests (one actual GPU test deliberately ignored) and feature
Clippy. Eighteen Python packaging/audit tests passed again after restoring the
checkpoint. Two independent reviews found no blocker in unified history/masks
and estimated-cue/GPU bounds. Final implementation/documentation head
`f986a736c0166c2b9b652fed6ce61b0c77296955` passed all Linux/macOS CI:
Audio foundation run 38955230988 includes fmt/check/strict Clippy, 303 default
Rust tests, the optional GPU feature tests/Clippy, runtime/packaging/audit Python
tests and all three actual media-encoder tests. GPU feature testing passes
14 adapter-free tests and intentionally skips actual device execution.
Apple Silicon preview run 38955231046 passed build/package/signature checks and
uploaded the arm64 artifact (SHA256
`3a41892e8da0afea8bd7cf9b5435aa76e09a16b43cc24d340aa9b5be11c7be6a`).
This subsequent edit records evidence only; it changes no implementation.

No actual Rhubarb engine, Kokoro inference or GPU device was executed here.
Estimated mouth shapes are not phonemes or forced alignment. GPU fixture
compilation is not physical hardware acceptance or full-scene GPU rendering.
Audit collection is not license clearance. See [bones](per-bone-editor.md),
[history](unified-history.md), [composition](advanced-compositing.md),
[estimated cues](rhubarb-estimated-cues.md), [GPU fixture](gpu-offscreen.md)
and [dependency evidence](dependency-audit.md).


## Native acceptance tooling and retained provenance (PR #31, merged)

Implemented source passed local validation, independent review and final CI:

- [x] Selected advanced-layer canvas routes immutable snapshots through a bounded
  shared CPU compositor with generation/sample guards and speech-mouth overrides.
- [x] Explicit native Rhubarb action runs against an already prepared line on a
  bounded worker; separate Apply reuses exact-PCM/current-text validation.
- [x] Packaged independently signed Metal fixture helper, explicit device-check
  UI, bounded evidence readback and cancellation/watchdog path.
- [x] New consented managed installs retain sanitized freeze bytes, installed
  distribution metadata and bounded license texts with manifest/hash binding.
  Missing or budget-omitted texts stay unresolved; distribution approval is false.
- [x] Local workspace/all-target tests: 329 passed, three explicit encoder tests ignored.
- [x] Independent integration reviews; EOF/error retry and Mac path-alias regressions fixed.
- [x] Final Linux/macOS CI and merge for this increment.
- [ ] Actual Mac compositor/device timing, real Rhubarb quality, physical Metal
  fixture results, full-scene GPU parity and final artifact-specific license clearance.

Current validation: workspace/all-target check and strict Clippy passed; 329 Rust
tests, 44 runtime, 16 inventory and 10 packaging Python tests passed. The first
CI checkpoint found a macOS canonical path alias regression, fixed with an alias
and escape regression test. Final head `5e0237143799d4d2bde61d91c5b8831cb11374fc`
passed Audio foundation run 38959215597 (Linux/macOS fmt/check/strict Clippy,
329 default tests, optional GPU tests/Clippy, Python checks and three actual
encoder tests) and Apple Silicon preview run 38959215483. Local optional GPU
validation passed 14 adapter-free tests with actual device execution ignored;
all three actual encoder tests passed locally too. The arm64 package includes
the independently signed Metal helper; bundle/helper architecture, signatures
and packaged executable permissions were checked. Uploaded artifact SHA256:
`d1ee4aea05b1dc2791cbdb6871debd9fed7b4db8a6e1813f5e9b8e14c127d651`.
Package: https://github.com/aleksipii/kilsatstudio/actions/runs/38959215483/artifacts/12071129527
This post-merge edit records evidence only and changes no implementation.
No real engine, speech inference or GPU hardware execution is claimed here.
See [native compositor](native-compositor.md), [native alignment](native-alignment.md),
[Metal device check](gpu-device-check.md) and [retained provenance](license-provenance.md).

## Estimated mouth-shape quality tooling (PR #32, merged 2026-10-18)

- [x] Integrate offline Rhubarb mouth-shape comparison against manual sample-based
  references, with whole-recording and active-union agreement, exact confusion
  counts and boundary errors only for matching collapsed shape sequences.
- [x] Integrate exact WAV/decoded-PCM/current-text evidence binding, bounded input
  snapshots, exclusive report publication and shared Rust/Python PCM golden vector.
- [x] Complete final CI, independent review and merge for this increment.
- [ ] Run representative real Rhubarb corpus and review audible/visible timing;
  establish reviewed targets and physical Mac synchronization evidence.

Local validation: 55 runtime tests (44 existing, 11 new) and 26 inventory/packaging
tests passed. Independent review reran all 55 runtime tests and cross-checked
signal measurements against the smoke reader at six lengths around chunk boundaries.
Final head `6fefd300fd57725b183b317455a73d0d1ab959c2` passed Audio foundation
39081856002 and Apple Silicon preview 39081856023: Linux/macOS fmt, workspace
check, strict Clippy, 330 default Rust tests, 14 adapter-free GPU tests/feature
Clippy, Python checks and three actual encoder tests. The shared Rust/Python
PCM golden vector passed. Rust checks ran in CI because no local toolchain was
available. The actual GPU test remains ignored.
The arm64 package passed architecture/signature/packaging verification. Artifact:
https://github.com/aleksipii/kilsatstudio/actions/runs/39081856023/artifacts/12118704932
SHA256: `0670ee100b0019713b25fee8049cb4957e84564fa0e9a4f6e1616925beab6a33`.
These quality-tool tests use synthetic audio; no recognizer, model or GPU was
executed. This post-merge edit records evidence only. The evaluator
always leaves production acceptance `not_granted`. Full GPU scene parity,
physical Mac acceptance and final artifact-specific license clearance remain open.
See [estimated mouth-shape evidence](viseme-quality.md).

## Whole-project SVG sequence export (PR #33, merged 2026-10-18)

- [x] Integrate vector SVG/WAV sequence export with cuts/crossfades, namespaced
  generated definitions, shared audio/sample manifest and bounded cleanup.
- [x] Integrate SVG format into the existing native export worker and headless
  YAML/`.kilsat` CLI; require FFmpeg only for MP4.
- [x] Validate SVG-vs-PNG raster fixtures with camera/fades, masks/blending,
  embedded images and prepared timed mouths, plus cancellation/output ownership.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Physical Mac SVG-viewer comparison and real speech/device synchronization.

Final head `de29b3732b3ed491ad5db82cb19db9a5fdf8478c` passed Audio foundation
39082961887 and Apple Silicon preview 39082961885: Linux/macOS fmt/check/strict
Clippy, 337 default Rust tests, 14 adapter-free GPU tests/feature Clippy, 55 runtime
and 26 inventory/packaging Python tests, three actual encoder tests and signed
arm64 package verification. Rust checks ran in CI; no local toolchain was
available. SVG raster comparisons cover cuts/crossfades, cameras, scene fades,
mask IDs, multiply blending, an embedded PNG and prepared mouth samples, with
at most two channel values of rounding tolerance. Cancellation, existing outputs,
future missing images and repeated-image preflight have regressions.
Independent review found repeated references could evade the unique-image budget;
preflight now counts each occurrence before SVG/raster generation, even when
hidden or active later. Per-scene base64 image occurrences are capped at 32 MiB,
serialized scene/frame SVG at 64 MiB, total output at 256 MiB. Temporary copies
are included in the conservative working-set estimate, which is not an allocator cap.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39082961885/artifacts/12120160079
SHA256: `6f4e56e221ffd93ec3d8e34dfb35da278a17b2afe6fe6d3958b291f0bda10455`.
This post-merge record changes no implementation. No real model/recognizer,
GPU device or physical Mac viewer acceptance was executed. The tracker also
corrects two stale unchecked items already completed in PR #31:
selected advanced CPU frames and explicit native estimated-aligner controls.
Estimated-cue caching, full-scene GPU parity, real corpus quality, physical Mac
acceptance and final artifact-specific license clearance remain open.

## Selected estimated-cue cache (PR #34, merged 2026-10-18)

- [x] Integrate immutable saved estimated results selected by a full-content key,
  exact PCM/text validation and preserved original tool/recognizer provenance.
- [x] Integrate best-effort save after explicit Estimate, explicit Load cached
  timing and separate Apply on the existing bounded native worker.
- [x] Validate corruption/bounds/schema, concurrent publication, owned cleanup,
  no-engine load and stale/mismatched prepared audio/text.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Automatic fresh-engine memoization with verified external resource identity.
- [ ] Project timing persistence, real corpus quality and physical Mac synchronization.

This increment is explicit selected-result reuse. It does not infer the current
recognizer environment from a binary hash or automatically substitute past cues
for a new execution. Final head `f5fc7d7334332fdc02218e7817bd170101a7ed75`
passed Audio foundation 39083898741 and Apple Silicon preview 39083898740:
Linux/macOS formatting, workspace check, strict Clippy, 345 default Rust tests,
14 adapter-free GPU tests/feature Clippy, 55 runtime plus 26 inventory/packaging
Python tests, three actual encoder tests and arm64 package/signature checks.
Rust checks ran in CI because no local toolchain was available. Independent
review found no blocker; same-frame settings/action handling was corrected.
Synthetic workflow tests save a result, remove the stub executable and WAV,
reload the selected entry and reject changed PCM/text. Save failures retain
usable timing with an explicit warning. Corrupt existing entries are preserved;
concurrent identical publication and owned staging cleanup have regressions.
The original result provenance is retained and no engine executes on Load.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39083898740/artifacts/12119862185
SHA256: `5423e84b9cabc704d5bdf6e91271483f294c235ace22b0d03cc4f7cf219bb3d1`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. Full GPU parity, advanced
artist tools, automatic engine memoization, physical acceptance and final license
clearance remain open.
See [saved estimated timing](estimated-cue-cache.md).

## Artist procedural layer strengths (PR #35, merged 2026-10-18)

- [x] Integrate per-scene/actor posture, walk and gesture strengths with strict
  defaults, finite range validation, atomic edits and character cleanup.
- [x] Integrate shared weighted evaluation while retaining authored bone offsets,
  facial/root/speech stages and existing default geometry.
- [x] Integrate detached native Inspector controls, explicit Apply, unified
  Undo/Redo, project persistence and stale/busy draft guards.
- [x] Validate actual SVG/PNG frames and shared preview on the speech sample clock.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Editable custom clips, animated blend tracks, arbitrary/textured rigs,
  advanced masks and physical Mac motion/artist acceptance.

Final source head `fa03a4fecc0404b80d1b6c44d354ccaaca8ade98` passed Audio
foundation 39085331814 and Apple Silicon preview 39085331803. CI validated
formatting, workspace check, strict Clippy, 359 default Rust tests on Linux and
358 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 26 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature checks passed. Rust checks ran in CI because no
local toolchain was available. Independent review found no blocker. A headless
regression caught Slider normalizing a nonfinite draft during paint; invalid
values now remain detached and block Apply until explicitly reset.
Actual SVG/PNG regressions cover absent versus explicit default weights,
gesture suppression, preserved authored offsets and sample-clock preview/export.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39085331803/artifacts/12120487413
SHA256: `e88a51f79cc8842e6ee205afa59496efd95d149bb34c8a7043d81b2ebde317a4`.
This post-merge record changes no implementation. No real recognizer/model,
GPU device or physical Mac acceptance was executed. These are controls for the
three existing procedural layers; advanced artist tools remain incomplete.
Full GPU parity, automatic fresh-engine memoization, physical acceptance and
final licenses remain open.
See [artist layer strengths](artist-clip-weights.md).


## Source license and animated strengths (PR #36, merged 2026-10-18)

- [x] Integrate root MIT LICENSE, consistent Cargo package metadata and README scope.
- [x] Include source license in the signed app resources and ZIP root, with missing
  license rejection and byte-exact package verification.
- [x] Integrate validated scene-local procedural strength keys, outgoing
  linear/smoothstep/hold sampling, static fallback and character cleanup.
- [x] Integrate detached animated strength editor, Apply/Undo/Redo, project
  persistence and stale/busy guards.
- [x] Validate actual SVG/PNG exports, sample-clock preview and deterministic seeks.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Arbitrary authored clip libraries/blending, custom/textured rigs and advanced masks.
- [ ] Full GPU/native parity, automatic fresh-engine estimated result reuse,
  physical Mac acceptance and final artifact-specific third-party license clearance.

Final source head `7a55ab5c506efec1ffedf2fd8fe71f4d86fd5978` passed Audio
foundation 39086245253 and Apple Silicon preview 39086245328. CI validated
formatting, workspace check, strict Clippy, 372 default Rust tests on Linux and
371 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature checks and byte-exact LICENSE checks inside the app
and at ZIP root passed. Rust checks ran in CI because no local toolchain was
available. The same 55 runtime and 29 packaging/inventory tests passed locally.
Independent review found an invalid empty-track export fixture; the corrected
test removes the track through the public API before comparing SVG/PNG output.
An existing worker format-routing test hit its five-second limit while rasterizing
default full-HD frames; its fixture now uses 64x64 frames and retains real exports.
Animated track tests cover strict schema/bounds, independent scalar interpolation,
outgoing easing, endpoint clamps, repeated/backwards seeks, actual SVG/PNG versus
sample-clock preview, static fallback, saved projects and one-edit Undo/Redo.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39086245328/artifacts/12120942405
SHA256: `91c80638a12768b504fc9d0e1db61fd45b551893672b6b7592b7bc3616996172`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. Declaring an original-source
license does not complete model/voice/tool/dependency review. Animated strengths
control the existing three procedural layers; custom clip libraries, arbitrary
rigs, full GPU parity, automatic fresh-engine reuse and physical acceptance remain
open. See [artist strengths](artist-clip-weights.md) and the
[dependency audit](dependency-audit.md).

## Project-local authored bone clip library (PR #37, merged 2026-10-18)

- [x] Integrate bounded, strict named animation clips in project source with
  original-rig bone tracks, duration and backward-compatible empty defaults.
- [x] Integrate atomic capture/explicit overwrite, full target-track replacement
  and removal; applied copies remain independent of library/source deletion.
- [x] Integrate native library capture/selection/inspection, explicit replacement
  confirmations, unified Undo/Redo, saved projects and stale/busy context guards.
- [x] Validate independent expected SVG/PNG frames, sample-clock preview,
  repeated/backwards seeking and deletion preserving already applied animation.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Live custom clip instances/blending, retiming, arbitrary/textured rigs and
  cross-project library exchange.
- [ ] Full GPU/native parity, automatic fresh-engine estimated-result reuse,
  physical Mac acceptance and final third-party release license clearance.

Final source head `85c5cb5948c17ac1073810744ac22963ee383912` passed Audio
foundation 39089591769 and Apple Silicon preview 39089591760. CI validated
formatting, workspace check, strict Clippy, 381 default Rust tests on Linux and
380 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. The 29 packaging/
inventory tests also passed locally. Rust checks ran in CI because no local
toolchain was available. Independent review found no remaining blocker.
An integration regression caught the general source edit helper clearing the
target actor selection; accepted library edits now retain that validated actor,
while stale/busy guards and rejected/noop action behavior remain unchanged.
Tests cover strict schema/resource bounds, full target replacement, independent
actor/source-library lifetimes, saved projects, source Undo/Redo and real SVG/PNG
frames versus independently authored expectations and sample-clock preview.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39089591760/artifacts/12122271546
SHA256: `781e30c3d8a36021db9e5d8e28182719f7725c8372a369685235ff18588021fc`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. These clips copy authored
offsets into the existing shared evaluator; live instances/blending/retiming,
custom/textured rigs, full GPU parity, automatic fresh-engine estimated reuse,
physical acceptance and final third-party licenses remain open.
See [clip library](animation-clip-library.md).


## Explicit authored clip retiming (PR #38, merged 2026-10-18)

- [x] Integrate atomic start/duration mapping of copied authored bone keys,
  finite scene bounds, preserved transforms/easing and precision-collapse rejection.
- [x] Integrate opt-in native retiming controls, explicit target replacement,
  detached invalid values and confirmation resets after option changes.
- [x] Validate saved source, one-edit Undo/Redo, unchanged library data and
  actual SVG/PNG/sample-clock preview against independently retimed tracks.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Live clip instances, arbitrary clip blending, cross-project exchange,
  custom/textured rigs and physical motion acceptance.
- [ ] Full GPU parity, automatic fresh-engine estimated-result reuse and final
  third-party artifact/license clearance.

Final source head `fc18aac32c5bc9068885601b23db9d21e323df85` passed Audio
foundation 39090819205 and Apple Silicon preview 39090819133. CI validated
formatting, workspace check, strict Clippy, 388 default Rust tests on Linux and
387 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. Rust checks ran in CI
because no local toolchain was available. Independent review found no blocker.
Numeric tests cover all seven bones, preserved transform/easing data, compression,
shifted placement, invalid intervals/references, overflow and precision collapse.
Native tests cover detached invalid drafts, action routing, original Apply,
source Undo/Redo and .kilsat persistence. Actual SVG/PNG regressions use
independently retimed tracks and compare sample-clock preview before/after keys
and during repeated/backwards seeks. A difference check originally chose a time
where both mappings correctly cross the same pose; the corrected quarter-second
check verifies independent geometry and visible pixel differences. Explicit
Reset copied key timing restores valid draft values and clears approval.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39090819133/artifacts/12122542757
SHA256: `81a68cc17af367e3c1a06237c4aa3f8b09b4ac1c833b273f37baa91247b9e01c`.
This post-merge record changes no implementation. Retiming copies keys into
existing tracks; endpoint holding remains normal bone-track behavior rather than
introducing a live activation window. No real recognizer/model, GPU device,
physical Mac or perceptual acceptance was performed. Full GPU parity, automatic
fresh-engine estimated-result reuse, live custom-clip blending, arbitrary rigs
and final third-party artifact/license clearance remain open.
See [clip library](animation-clip-library.md).


## Five open workstreams assigned (PR #39, merged 2026-10-18)

- [x] Assign a separate agent to each of the five requested open workstreams.
- [x] Inspect current implementations and record bounded next changes, interfaces,
  acceptance tests and evidence blockers in the linked workstream documents.
- [ ] Implement and verify full GPU scene/native parity.
- [ ] Implement automatic estimated-result lookup with verified resources/environment.
- [x] Implement the first original-rig persistent live-instance and ordered override package (PR #40).
- [x] Extend live layers with animated scalar weights and linear fades (PR #41).
- [x] Extend live layers with looping and explicit joint masks (PR #43).
- [ ] Extend live layers with more general mixing.
- [ ] Collect and review physical Mac/device/offline/synchronization acceptance evidence.
- [ ] Complete the final artifact-specific third-party license/redistribution review.

This assignment increment changes documentation only. No renderer, inference,
live blend evaluator or release approval is implemented by these plans. The
parent owns shared schema/scene/desktop files and coordinates implementation
before agents may edit overlapping production code. The existing user-reported
M1 Kokoro speech evidence is retained; it does not close the remaining device
gates. Plans and agent ownership are not completed product acceptance.


## Ordered live authored clip layers (PR #40, merged 2026-10-18)

- [x] Integrate strict persistent scene instances, reference/numeric/order/budget
  validation, atomic upsert/remove/reorder, deletion guards and actor cleanup.
- [x] Integrate prepared shared referenced tracks, checked local-offset override
  blending, half-open windows, unchanged default behavior and typed failures.
- [x] Integrate native detached instance editing, global layer order, library
  reference warnings, source Undo/Redo, persistence and stale/busy/actor guards.
- [x] Reuse prepared snapshots in scene/project export and cached native preview;
  preflight inactive/zero layers and enforce bounded work before output creation.
- [x] Validate independent actual SVG/PNG frames, sample-clock preview, seek order,
  overlap order, root/speech stages, library replacement and copied-track independence.
- [x] Final Linux/macOS CI, independent review and merge.
- [x] Animated weights/fades followed in PR #41; see the integration record below.
- [x] Looping and original-rig joint selection followed in PR #43.
- [ ] Arbitrary/textured rigs, cross-project exchange and physical/perceptual
  motion acceptance.
- [ ] Full GPU parity, verified-environment automatic estimated reuse and final
  third-party artifact/license approval.

Final source head `b6700bdf47f16cdfa900cd960fbf6877f7e29784` passed Audio
foundation 39125173890 and Apple Silicon preview 39125173903. CI validated
formatting, workspace check, strict Clippy, 409 default Rust tests on Linux and
408 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. Rust checks ran in CI
because no local toolchain was available. Independent review found no remaining
blocker after correcting unused-library preparation and total live-work bounds.
Prepared snapshots store only referenced definitions, share their keys across
instances/clones and are retained across production preview/export frames.
Existing bounded preflight may prepare separately; it retains its early errors.
The work estimate counts inactive/zero layers and binary-search/scanning work;
a regression rejects excessive valid live work before any output directory.
Tests cover strict schema/references/count limits, independent weight/order and
rotation/scale expectations, half-open/adjacent/compressed windows, repeated
seeks, copied/live definition independence, guarded deletion, source history,
project round trips, cross-actor/stale/busy rejection and prepared-storage sharing.
Actual SVG/PNG frames match independent authored expectations and sample-clock
preview with procedural, root and speech stages. Native live projects use the
existing bounded compositor worker rather than reconstructing tracks per draw.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39125173903/artifacts/12135299806
SHA256: `de5eb79159a1556c95f6389ca09c833a3247d68c80f169210d559beef71c0231`.
This post-merge record changes no implementation. First support is constant-weight
ordered override of original-rig local offsets, with explicit half-open activation
and no implicit fades. No real recognizer/model, GPU device or physical Mac
acceptance was performed. Animated weights/fades, loops, arbitrary rigs, full
GPU parity, verified-environment estimated reuse and final licenses remain open.
See [live clip blending](live-clip-blending.md).


## Live clip weight curves and fades (PR #41, merged 2026-10-18)

- [x] Integrate backward-compatible per-instance linear fades and local scalar
  weight keys with outgoing easing and exact static-envelope defaults.
- [x] Validate finite durations/multipliers, non-overlapping fades, ordered local
  key bounds and 256/8192/65536 instance/scene/project key budgets.
- [x] Integrate detached native fade/key authoring, explicit reset/Apply, invalid
  draft preservation, source Undo/Redo and project persistence.
- [x] Account for scalar searches/fades in preview/export work estimates even for
  inactive/zero layers; reject over-budget output before creating directories.
- [x] Validate independently expected actual SVG/PNG/sample-clock preview frames,
  faded base-pose boundaries, overlap order, backwards seeks and old defaults.
- [x] Final Linux/macOS CI, independent review and merge.
- [x] Looping and original-rig joint masks followed in PR #43.
- [ ] Custom/textured rigs, cross-project exchange and physical/perceptual
  motion acceptance.
- [ ] Full GPU parity, verified-environment automatic estimated-result reuse and
  final third-party artifact/license clearance.

Final source `9247f6a00c9167e2bc1ec9ffae9caf7518bb2a11` passed Audio foundation 39126660947 and Apple Silicon
preview 39126660943. CI verified formatting, workspace check, strict Clippy,
420 default Rust tests on Linux and 419 on macOS, 14 adapter-free GPU
tests/feature Clippy per platform, 55 runtime and 29 packaging Python tests per
platform, and three actual encoder tests. Independent review found no remaining
blocker. The actual-frame fixture binds PCM and manifest duration to the scene;
frame 8 tests the instance end inside a longer scene. The preflight regression
rejects dormant zero-weight envelope work before any output directory is created.
Local Rust tools are unavailable; these execution results come from CI.

Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39126660943/artifacts/12135693647
SHA256: `5738e7406bcb8c36b0b08a381b847801b177c0bb1c2a270551567658ac3bc0f0`.
This post-merge record changes documentation only. This is an envelope for
ordered override weights, not a normalized or additive mixer. No real recognizer,
model, GPU device or physical Mac acceptance is performed by fixture tests. See
[live clip blending](live-clip-blending.md).

## Final package inventory (PR #42, merged 2026-10-18)

- [x] Integrate bounded offline final-ZIP/member inventory and exact current
  resource boundary, licenses, generated metadata and build-info checks.
- [x] Integrate explicit signature/AppleDouble observations, optional helper
  component identity, atomic external sidecar and distinct collection/strict exits.
- [x] Integrate post-signing macOS CI report attachment, independent review and
  package/security regression tests.
- [x] Collect separate resolved target-filtered Cargo closures, compiler identity
  and final-ZIP/member-bound native observations in CI (PR #43).
- [ ] Prove exact compiled/embedded dependency closure and review upstream artifact
  notices, license alternatives and redistribution obligations.
- [ ] Final third-party license clearance, GPU parity, automatic estimated-result
  reuse and physical Mac/hardware/model quality acceptance.

Final source `6bf46d2f2edebcfbd217e5d2927467b88ffb886f` passed Audio foundation 39128020733 and Apple Silicon
preview 39128020717. CI verified fmt/check/strict Clippy, 420/419 default Rust tests
on Linux/macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
and 49 repository Python tests per platform, and three actual media encoder
tests. The 20 new offline audit tests cover exact resources/licenses/generated
metadata, optional helper, unsafe ZIPs, budgets, malformed XML/DEFLATE, strict
exits and atomic output/alias protection. Local 49-test Python suite passed;
independent review found no remaining blockers. No Rust code/dependency changed.

Final signed ZIP collection in Mac CI passed with 21 file records,
`package_valid: true` and `distribution_approved: false`. Observed final ZIP
SHA256: `c869d8ebd96a1821a400bb9efa8f79f01cfc29d639969dbd8a148c5f38edfc89`.
ZIP and external sidecar artifact: https://github.com/aleksipii/kilsatstudio/actions/runs/39128020717/artifacts/12137460298
Artifact container SHA256 (distinct from contained preview ZIP): `c44972df2bb305ad4dc8eccd184d3fdeaaa7a65bfdc97163c53ef80c2ce4cb8b`.

See [final package audit](final-package-audit.md). This post-merge record changes
documentation only;
package-boundary success never grants distribution approval. No external assets,
recognizer, GPU device or physical Mac acceptance is exercised by fixture tests.

## Parallel studio workstreams (PR #43, merged 2026-10-18)

- [x] Integrate bounded evaluated-geometry GPU experiment, explicit font-label
  omission/unsupported-feature boundaries, adapter-free command/shader tests and
  full decoded-RGBA comparison metrics. Production full-scene parity stays open.
- [x] Integrate live-clip original-speed looping and explicit seven-joint masks,
  native controls, persisted defaults and independent SVG/PNG/preview tests.
- [x] Integrate persisted soft animated ellipse masks, native Apply/history,
  shared SVG/PNG/preview/sequence sampling and strict geometry/key/work bounds.
- [x] Integrate exact-request immutable engine-index foundation, synthetic
  conflict/corruption/cancellation tests and fresh versus explicit archive origin.
  Production automatic reuse remains ineligible without a verified real package.
- [x] Integrate final-ZIP-bound separate resolved Cargo closures, compiler and
  final-member native observations, retained CI inputs and never-approved report.
- [x] Integrate versioned editable scene-order/duration snapshot using the shared
  sequence clock, native scene-order actions, source history and validation.
- [x] Final Linux/macOS CI, independent cross-stream review and authorized merge.
- [ ] Full GPU/native parity and actual Metal performance; custom rigs, textured
  parts/deformation; artist groups, painting, pressure and image retouching.
- [ ] Verified real recognizer environment/automatic hits, editable mouth timings
  and measured real-audio quality; physical Mac/offline/device/GUI acceptance.
- [ ] Final licenses/redistribution review, production measurements/release,
  separate audio cues, conditional animation rules and general compiled-track editing.

No downloaded model, real recognizer, physical Mac or GPU execution is claimed.
See [GPU experiment](gpu-evaluated-scene.md), [live clips](live-clip-blending.md),
[soft masks](soft-animated-masks.md), [engine reuse](workstreams/estimated-auto-reuse.md),
[build evidence](build-evidence.md) and [editable timeline](editable-timeline.md).

Final source `de1dbc446510e2a895b7138612d80a292814c015` passed Audio foundation 39156985428 and Apple Silicon
preview 39156985436. CI passed workspace fmt/check/strict Clippy, 446/445
default Rust tests, 23 adapter-free optional GPU tests/feature Clippy,
55 runtime and 59 repository Python tests per platform and three media encoder
tests. No local Rust toolchain was available. Cross-stream and final integration
reviews found no remaining blockers. The mask pixel regression uses an excluded
head translation, since leaf-joint rotation alone does not move its plotted point.

Mac package, package audit, build report and retained inputs: https://github.com/aleksipii/kilsatstudio/actions/runs/39156985436/artifacts/12147780773
Artifact container SHA256: `42877ed0a8258ac459c7d59640aaec0448d50dc32817f303825993aa4b493be9`.
Final signed preview ZIP SHA256: `331db8d6e9c873c17c3e63b640e930ff4cae1569fb9d947ba149bcc4de1a5be6` (distinct from artifact container).
The package audit accepted 21 file records; build collection completed with
`distribution_approved: false`. These observations do not prove exact compile
units, upstream rights, physical device behavior, real recognizer quality or
GPU/Metal execution. No model/voice assets were downloaded or inference run.
This post-merge record changes documentation only.

## Additive original-rig live clips (PR #45, merged 2026-10-19)

- [x] Integrate strict default-compatible blend mode and checked signed local
  additive translation/rotation and multiplicative scale, with accumulated bounds.
- [x] Integrate shared prepared sampling with existing saved order, loops, masks
  and envelopes; preserve exact legacy override/default behavior.
- [x] Integrate detached native mode control, reset preservation, source history,
  persistence and invalid-context/candidate protection.
- [x] Validate independently authored actual SVG/PNG/sample-clock preview frames,
  mixed-mode ordering, visible parent rotation, repeated seeks and round trips.
- [x] Charge dormant/zero additive layers and reject excessive work before output.
- [x] Final Linux/macOS CI, independent review and authorized merge.
- [ ] Normalized groups, arbitrary rigs/textures/deformation, cross-project clips,
  physical Mac motion/performance acceptance and the other seven area workstreams.

Final source `a1aa04933d90ae0077ae8a78e1cdd1d6b55fd285` passed Audio foundation
39206324716 and Apple Silicon preview 39206324721. Workspace fmt/check/strict
Clippy passed with 460 Linux and 459 macOS default Rust tests, 23 adapter-free
optional GPU tests and feature Clippy per platform, 55 runtime and 59 repository
Python tests per platform, and three actual media encoder tests. Fourteen new
Rust regressions cover this increment. CI rustfmt differences were corrected
before the final passing run. No local Rust toolchain was available.

Mac package, audits and retained build inputs:
https://github.com/aleksipii/kilsatstudio/actions/runs/39206324721/artifacts/12166270889
Artifact container SHA256: `d0563a6f1ee0ee5138128d548dea7ebacbbdb942e536c935cb626136112d8087`.
Final signed preview ZIP SHA256: `09539600fc743785c595896b811326bc694690394b1d1f9802e90279d09da1bd`.
The package audit accepted 21 file records, with `distribution_approved: false`.
Independent review found no remaining implementation blockers. No real recognizer,
model inference, GPU/device execution or final license clearance is claimed. See
[live clip controls](live-clip-blending.md) and the initial
[animation handoff](workstreams/next-character-animation.md).

## Combined production workstreams (PR #46, merged 2026-10-19)

User requested all six remaining areas in one next PR. Six implementation owners
worked on separate boundaries with parent integration. The scoped implementations
below passed final checks and merged; the entire six-area production backlog is
not complete. No whole-product milestone or external acceptance gate is closed.

- [x] Normalized original-rig override groups: compatible explicit contiguous
  groups, per-joint residual/normalization, continuous authored-base rotation anchor, native
  controls and independent expected SVG/PNG/shared-preview tests.
- [x] Versioned custom rigid characters: stable joint hierarchy, bounded colored
  polygon parts, cached rest evaluation, detached native definition editor,
  source history and saved projects. Root motion works; original-rig bone/live
  tracks and gestures are explicitly unsupported for custom actors.
- [x] Expanded experimental GPU composition: original decoded images, artwork,
  custom polygons, existing hard/soft masks, isolated groups and backdrop blends,
  bounded source-texture residency and whole-frame weighted crossfade commands.
- [x] Isolated nested artist groups: explicit mixed sibling order, visibility,
  opacity applied once, normal/multiply/screen, native atomic controls/history,
  shared CPU composition and dormant group work/memory preflight.
- [x] Automatic estimate reuse: explicit local self-contained Phonetic package
  enrollment, exact package/environment identity, private controlled execution,
  automatic hit/miss/publication and explicit fresh-run escape. Native loader
  formats/resources have narrow supported boundaries; unknown packages remain
  explicit-only. Enrollment is local trust, not quality or rights approval.
- [x] Offline release-license corpus/review-gap report and artifact-bound Mac
  observation report: exact input identities, bounded evidence, atomic new-only
  output, actionable unresolved gates and never automatic final approval.
- [x] Final Linux/macOS fmt/check/strict Clippy/default/optional GPU/Python/media
  checks, independent review and authorized integration.
- [ ] Finish the remaining software and external acceptance work listed below.

Still open beyond these concrete increments: custom-rig animation tracks,
textures/deformation and clip exchange; shared deterministic GPU fonts, measured
full CPU/GPU parity, production GPU preview/export and physical Metal performance;
image masks, polygon feathering, painting, pressure and retouching; actual enrolled
recognizer execution/perceptual quality; physical Mac offline/audio/GUI acceptance;
complete upstream license/NOTICE corpus, reviewed obligations, final packaged
notices and authorized production signing/release. Source tests and evidence
collectors cannot manufacture physical acceptance or rights clearance.

Final source `6f25ccd808cc383312162a9df3d131c37fd079b2` passed Audio foundation
39211804287 and Apple Silicon preview 39211804376. Workspace fmt/check/strict
Clippy passed with 492 Linux / 491 macOS default Rust tests (32 new), 28 adapter-free
optional GPU library tests and two project-probe tests per platform, feature
Clippy, 55 runtime / 79 repository Python tests per platform (20 new repository
tests), and three actual media encoder tests. GPU contracts run independently
on both platforms. No local Rust toolchain was available.

Independent review findings were corrected: normalized rotations no longer
switch anchors at total weight one; accepted rig polygons are simple; isolated
single-scene targets share the retained-image/audio working-set budget; probe
inputs reject nonregular files before opening; Mac fixtures use canonical OS
paths; package manifests/copies retain cumulative bounds. Targeted numeric,
actual SVG/PNG/preview, persistence/history, malformed-input and resource tests
passed. CI formatting, type/lint and shader validation findings were fixed before
the final successful source. No adapter was requested by default GPU tests.

Mac preview, package/build reports and retained inputs:
https://github.com/aleksipii/kilsatstudio/actions/runs/39211804376/artifacts/12167669246
Artifact container SHA256: `0efd67320e8f17ae8bf79bd4abfda17cbd0fa44c71b1f490eb2bc0a774e43315`.
Final signed preview ZIP SHA256: `788b404698e01b9fa0b142e9e1ad0a20959ef461e19f52c8f195614787e9b403`.
The final audit accepted 21 file records with `distribution_approved: false`.
No real recognizer/model, physical Mac/GPU execution, perceptual quality,
performance measurement or final license clearance is claimed.

Open the original asset-free [studio demo](../examples/studio-production-demo.kilsat)
to inspect custom rigid parts, normalized clips and nested groups. See
[rig editing](custom-rigid-characters.md), [clip controls](live-clip-blending.md),
[GPU experiment/probe](gpu-evaluated-scene.md),
[local package reuse](workstreams/estimated-auto-reuse.md) and
[release/Mac reports](release-license-report.md). This post-merge record changes
documentation only.

## Physical Mac GPU probe and native Rhubarb run (2026-10-19)

Run by hand on one Mac, outside CI, from `main` at `6a09873`.

- Machine: iMac21,2, Apple M1, macOS 27.0.1, native arm64, Rust 1.99.1.
- Metal fixture probe: `cargo run -p motion-render --features gpu --example gpu_probe -- --metal <new dir>`.
  Report: adapter `Apple M1`, backend `Metal`, device type `IntegratedGpu`, 256x256
  `rgba8unorm-linear`, `interior_probes: passed-tolerance-2`, empty `validation_errors`,
  `full_scene_parity: false`. SHA256 `report.json`
  `edba908c4fce2e1a4d91908a6194cea6def30a069c488e59e65e06f0d0fb2508`, `fixture.rgba`
  `0e86ddf8a97a89ca162fd9d805c45d41d99f4ba98fbfdc557080d75f78f6027b`.
  This is the fixture gate only; evaluated-scene parity and performance remain open.
- Rhubarb 1.14.0 official macOS release (zip SHA256
  `f991deacac6c973a14a4431a16a58b842f436531e120cfaea142c87c0d3ab4c5`) is x86_64-only.
  Under Rosetta 2 it started (`--version`) but crashed with SIGSEGV on speech input
  (phonetic and PocketSphinx, 16 and 24 kHz) when run directly, so the fault is in the
  binary, not in `motion-lipsync`. Cause not investigated.
- A native arm64 build of the v1.14.0 tag (CMake, Homebrew Boost 1.92, `rhubarb`
  target only, `res/` copied from the release) ran successfully. Executable SHA256
  `75364e2772d9f23ee8035f794a889b53eea755af54cc243ca7b5cf7089bc5314`.
- `motion-lipsync` example `rhubarb_align` with that executable (PocketSphinx) wrote
  estimated cues (`source: estimated_rhubarb`, 75,977 frames at 24 kHz) for a macOS
  `say` voice line. This shows the execution chain works on Apple Silicon.

Not established: lip-sync perceptual quality (input is `say`, not Kokoro, and has no
manual annotation comparison), Rhubarb/PocketSphinx redistribution terms, a packaged
arm64 Rhubarb, Metal scene parity or performance, and any other physical Mac gate.

## Feathered polygon masks (PR #48, on main via #51)

Adds optional polygon `feather` (blur sigma, scene units) through `motion-art`
validation/persistence, the shared SVG renderer, and the inspector slider; the
experimental GPU path reports it unsupported. On the Apple M1 Mac, fmt, workspace
check, Clippy (no warnings) and `cargo test --workspace` passed, including new
pixel tests. Not done: feather animation, image masks, painting, GPU feather
parity, blur-area work budgeting, CI on this branch, and physical GUI inspection.

## Image masks (PR #49, on main via #51)

Adds `clip_shape` type `image` (project image asset, rect, alpha or luminance channel)
through `motion-art` validation, shared SVG rendering, project/history/export asset
checks (`Layer::image_assets`), and an inspector control; the experimental GPU path
reports it unsupported. On the Apple M1 Mac fmt, Clippy (no warnings) and
`cargo test --workspace` passed with new pixel tests. Not done: animation, feathering
of mask images, painting/erasing, GPU parity, CI on this branch and GUI inspection.

## Custom-rig joint animation tracks (PR #50, on main via #51; includes the desktop joint key editor)

Adds scene `composition.joint_tracks` for custom rigid characters: validated offset
keys per actor and joint, a prepared rig (`PreparedRig`) so sampling only composes
the hierarchy, shared scene evaluation for preview/SVG/PNG/video, an
`apply_joint_track` source edit and character-removal cleanup. On the Apple M1 Mac
fmt, Clippy (no warnings) and `cargo test --workspace` passed with new rig, scene
and script tests (posed geometry, determinism across seeks, validation errors).
Not done: reusable custom-rig clips and retargeting,
textures/deformation, CI on this branch, physical Mac inspection and a rendered
pixel test of a posed custom rig.

## Vector mask painting and erasing — PR #53 delivery evidence

- [x] Explicit painted-mask selection and visible/hidden base coverage; ordered
  fixed-radius vector paint/erase strokes, round dabs/caps/joins and no new assets.
- [x] Native Inspector brush/eraser/radius, detached gesture drafts and one shared
  Undo/Redo transaction per release. Selection, time, camera, viewport or layer
  changes cancel drafts. Busy/playing/stale compositor frames cannot be painted.
- [x] Shared luminance SVG masks for CPU scene preview, SVG/PNG/video export;
  actual pixel tests verify paint → erase → repaint, both bases, dabs, repeated
  nonsequential seeks and preview/PNG/SVG equality. Persistence and gesture/history
  tests passed. Layer-local bounds preserve camera and actor translation.
- [x] Finite radius/coordinate checks, 512 strokes/mask, 4096 strokes/document,
  2048 points/stroke and the shared 262144-point document limit. Export/preview
  charge mask work, including dormant masks; excessive export work fails before
  creating output. Experimental GPU returns an explicit unsupported-feature error.
- [ ] Pressure, soft brushes, animated paint, raster retouch, GPU painted-mask
  parity and physical Mac pointer/accessibility/GUI acceptance.

Final tested source: `1bdb44fce061ceb567320b33d8b243eabadd9c98`; merged tree
`d2dd058791b4f4a8dd57c26c129c425e87b1b535` matches the tested source.
[Linux/macOS CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39242281276)
passed fmt, workspace check and strict all-target Clippy. Default all-target Rust
tests passed: **515 Linux / 514 macOS**, with three intentionally ignored real
encoder tests each. Separate GPU jobs each passed **31 library + 2 probe tests**
(one hardware-only test ignored), plus strict optional-feature Clippy. The media
job explicitly ran and passed all **3 actual FFmpeg tests**. Both platforms passed
**55 runtime + 79 repository Python tests**. Locally, those same Python suites and
`git diff --check` passed; Rust tooling was unavailable locally. Independent source
and native gesture review found no remaining integration blocker after the
stale-frame painting guard was added.

[Apple Silicon preview CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39242281274)
built and packaged the same final source. Download the
[arm64 preview artifact](https://github.com/aleksipii/kilsatstudio/actions/runs/39242281274/artifacts/12182640232).
Inner final preview ZIP SHA-256:
`d7dbd728855bf392255ab4165220250665c0d4f2581b2665d811e053015e8781`.
Outer artifact container digest:
`d5bb0861c3503b0209cc54abbcb42096edf0e933561c4d9e2308d021254c6a7f`.
Package validation passed; `distribution_approved` remains false. This automated
build establishes neither physical Mac input/audio acceptance nor final licences,
notarization, real-model quality or full GPU/native parity. M1–M6 remain subject
to their existing open acceptance gates.

## Soft pressure masks and experimental GPU masks — PR #54 delivery evidence

- [x] Backward-compatible `MaskStroke.pressures` and `hardness`: empty samples
  mean full radius, hardness defaults to 1, values are finite and bounded, and
  nonempty samples match point count. Radius interpolates continuously; zero
  pressure contributes no coverage. Old hard fixed-radius SVG output is preserved.
- [x] Shared swept endpoint disks/tangent quads and 32 nested soft coverage bands.
  One gesture takes maximum coverage, avoiding opacity buildup and winding holes;
  later paint/erase gestures retain ordered composition. Model validation caps
  expanded work at 1,048,576 units and all preview/export preflights charge it.
- [x] Native hardness/pressure controls, actual initiating-contact `Touch.force`
  capture, mouse/missing-force full-radius fallback, stationary pressure samples,
  contact/cancel/context rejection, persistence and one shared Undo transaction.
  This establishes input-event handling, not Mac stylus driver support.
- [x] Experimental GPU painted-mask commands use shared band geometry, ordered
  coverage and crossfade resource relocation. Existing point/command/fragment-work
  budgets reject excessive work before adapter initialization. Production scene
  preview/export remain CPU; image masks and feathered polygons remain explicitly
  unsupported in the experiment. Full pixel/edge parity is not established.
- [x] Self-contained [manual input fixture](../examples/painted-mask-input.kilsat)
  and [physical Mac procedure](painted-mask-mac-acceptance.md). The documented
  256×144 Metal probe fits an adapter-free work-budget test. The fixture is decoded,
  source-parsed, validated and roundtripped by CI.
- [ ] Execute physical Mac mouse/tablet/force, Finder, accessibility and complete
  input workflow; retain actual current-build evidence. All manual observations
  start `not_tested`. The Linux host has no physical Mac, native tablet or Metal.
- [ ] Run the ignored painted-mask hardware probe and independent CPU/GPU decoded
  comparisons, set justified edge tolerances and measure Metal performance.
  Animated paint, raster retouch, full GPU/native parity, device/audio acceptance,
  real-model quality and final licensing/distribution approval remain open.

Final tested source: `8f5693441a7b494ad052f6e8ead89f6b6dab0882`. Merge tree
`7ed741ea8542b8b65dec3ba89b886c02fdd98365` matches the tested implementation.
[Linux/macOS CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39244528956)
passed fmt, workspace check, strict all-target Clippy and default all-target Rust
tests: **525 Linux / 524 macOS**, with three intentionally ignored encoder tests
each. Separate GPU jobs each passed **34 library + 2 probe tests** and strict
optional-feature Clippy; **two physical GPU tests remained ignored**, including
the new painted-mask probe. Naga validated the extended shader without an adapter.
The media job explicitly passed all **3 real FFmpeg tests**. Each platform passed
**55 runtime + 79 repository Python tests**. Local Python suites and whitespace
checks passed; Rust tooling was unavailable locally. Independent static review
found no remaining blocker; it explicitly retained GPU edge/parity acceptance.

Actual CPU pixels cover soft ramps, varying-pressure width, paint/erase/repaint,
duplicate-point nonaccumulation, self-crossing/sharp-turn coverage and shared
preview/PNG/SVG output. Real egui event tests cover device force, fallback and
gesture cancellation; project/history tests retain both new fields.

[Apple Silicon build/package CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39244528752)
passed for the same source. Download the
[arm64 preview artifact](https://github.com/aleksipii/kilsatstudio/actions/runs/39244528752/artifacts/12183420529).
Inner final preview ZIP SHA-256:
`a9de2b211accaac38c780bd18de54fc51eb903623ae9860d85975fb63aa22bfc`.
Outer artifact container digest:
`1795739a6f130242b17ea2b0a441844fa552ad9eba6177fa0deab88aa6153d6a`.
Package validation passed with `distribution_approved=false`. Automated build and
shader tests do not grant physical-device, licensing or M1–M6 acceptance.

## Remaining workstream increment — PR #55 delivery evidence

The requested linked [execution list](remaining-work.md) assigns all eight areas
to six agents plus parent performance/integration. These first concrete packages
are integrated; the full remaining product backlog is not complete.

- [x] Original-rig clip clipboard exchange: strict version/rig/size checks,
  selected/all export, explicit matching-name replacement, atomic import and
  existing source/history integration. Custom-rig retargeting stays open.
- [x] Image-mask rectangle keys: scene-local linear interpolation/end holds,
  strict duration/key/bounds validation, native explicit Apply, shared CPU/GPU
  sampling and key/work preflight. Reverse-seek CPU preview/PNG/SVG tests use
  independently sampled static rectangles as their pixel oracle.
- [x] Manual mouth timing from reviewed estimated cues: sample/shape table,
  split/delete/rest/reset, worker verification of exact PCM/text, separate Manual
  provenance and edit identity without recognizer execution or cache overwrite.
  Representative phoneme labels carry artist-selected visemes, not a measured
  transcription. Optional bounded project timing metadata survives `.kilsat`
  save/reload and shared Undo/Redo; YAML save refuses to lose timing. History
  snapshots retain runtime invalidations. Applied manual envelopes cannot yet be
  reopened directly into this table; waveform dragging/forced alignment stay open.
- [x] Push/pull/left/right cinematic camera presets expanded to two editable
  existing camera keys, explicit track replacement and detached Apply/history.
- [x] Experimental GPU alpha/luminance image masks with existing bounded image
  residency, bilinear sampling, animation and crossfade offset relocation. Naga
  and command tests passed; hardware/edge/color acceptance remains open.
- [x] Opt-in 256-sample elapsed update-wall-time window and copyable summary for
  explicitly tracked job flags. This measures neither CPU usage nor input/display
  latency; production responsiveness/memory/Apple Silicon benchmarks remain open.
- [x] Mac operator report requires named coverage for 24 scenarios across eight
  broad gates, with hashed retained evidence references and contradictory/unknown
  claims rejected. Old broad labels remain readable but incomplete, never approved.
- [ ] Custom-rig clip libraries/mixing/retargeting, textures/deformation/idle,
  polygon/paint animation, image retouch, audio cues/conditional rules/general
  compiled editing, broader timing/real-speech quality, production GPU/full parity,
  process-wide budgets/DAG scheduling, physical Mac acceptance and final licences.

Merge: `c03bfe21779e9ed357c700f930f6d10086a5da86`. Final tested source:
`c98ac9cbb1cc909114ebd3707ebe3a0f53194f3a`, tree
`7229f16b89d49fd8667407e3c45d99c8142abed6`, identical to the merge tree.
[Linux/macOS CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39248869347)
passed fmt, workspace check, strict all-target Clippy, **543 Linux / 542 macOS**
default Rust tests (3 encoder tests ignored per platform), **35 GPU library + 2
probe tests** per platform and feature Clippy (3 physical GPU tests ignored).
All **3 actual FFmpeg tests** ran separately and passed. Each platform passed
**55 runtime + 82 repository Python tests**. Local Python suites, whitespace and
new guide link targets passed; local Rust tools were unavailable. Independent
review fixes covered timing persistence/history/YAML loss, mask draft context,
GPU/public-type consumers, budgets and honest measurement terminology.

[Apple Silicon package CI](https://github.com/aleksipii/kilsatstudio/actions/runs/39248869339)
passed for that source. The
[arm64 preview artifact](https://github.com/aleksipii/kilsatstudio/actions/runs/39248869339/artifacts/12185025968)
contains final preview ZIP SHA-256
`0a1f1b2412729da5d84632bb7afea72e9cd56e3aaa40a3661b911b57add7b19b`;
outer artifact digest
`e3053c51c59c2f33828b357003c8dca522687bccc7b1742d62adff219885ece6`.
Package validation passed with `distribution_approved=false`. No physical Mac,
GPU device, recognizer quality, rights clearance or complete milestone acceptance
was newly executed/granted. Main and `kilsatstudio1.0` include this delivery record.

## Guided desktop animation start

- Implemented a Project-tab dialogue entry workflow (`CHARACTER: dialogue`) that
  appends/selects an editable scene through existing validation and Undo/Redo.
  It preserves existing scenes, lays speakers across the stage and exposes scene
  audio preparation, project folder/save and existing video export controls.
- Explicit input budgets, actionable line errors, no partial mutation, no invented
  speech/timestamps, no new dependencies. English Kokoro and imported Finnish WAV
  paths are explained in the UI and desktop guide. Fixed speech slots need artist
  adjustment against real audio; this is not automatic professional direction.
- Local formatting, workspace compilation and strict all-target Clippy passed
  using a direct stable toolchain, explicit sysroot and GNU linker to accommodate
  the sandbox without `/proc/self/exe`. Packaging tests: 13 passed. Desktop tests:
  222 passed, one existing package-enrollment test blocked by absent
  `/proc/sys/kernel/osrelease`; all four new tests passed. Full workspace testing
  and exact Linux/macOS CI results are tracked in PR #90.
- Initial Apple Silicon package build succeeded for `e1c1df7`; final follow-up
  explicitly hides non-speaking actors in newly created scenes. Download the
  final-head package via PR #90 rather than assuming the initial artifact is current.
- This increment does not complete the broader product backlog or establish real
  Mac interaction, speech quality, physical device acceptance or release approval.

## Automatic direction — orchestrator and parallel implementation

Six task agents (one orchestrator, five workstreams) implemented a bounded
`motion_script::auto_direction` pipeline: dialogue parsing, shared 24 kHz timing,
existing ShotList generation, conservative acting and independent acceptance.
The desktop exposes Auto-direct beside the existing simple scene importer.

This first increment preserves authored scenes, exposes source/timing diagnostics,
uses normal editable MotionScript/Undo and rejects excess speakers, text, shot
counts, durations and source size. Estimated speech remains explicitly a draft.
Existing ten-agent design documents remain broader plans, not completed gates.

Local validation: 92 motion-script unit tests and 4 independent acceptance tests
passed; crate documentation checks passed. Workspace/all-target strict Clippy
passed through the direct driver with explicit sysroot/GNU linker. The desktop
Auto-direct append/selection/Undo/Redo/busy-state test passed. Final CI evidence
is recorded in the delivery PR. Physical Mac,
rendered artistic review, real-speech retiming, general intent/action recognition,
rig-bound framing and five-minute production acceptance remain unverified/open.


## PR #90 and #91 merged into main — 2026-10-23

Both merges were explicitly authorized by the user:

- PR #90 (guided desktop creation), merge `8aef5a448383565ea407aba9029d5ca3e0414c13`.
- PR #91 (automatic direction orchestrator), merge `fdd5ae060f96726a37e3f7036ec4a312631eecc5`.

The resulting main tree is `eeed7af21a3f7dcc924d04f1979c88f37cc1de0d`, identical
to tested/built source `e6fb2cc082adaf619278a40242c7df441f941211`.
Full Rust formatting, workspace compilation, strict Clippy/all-target tests and
Python suites passed on Linux/macOS for that source: 766/765 Rust tests passed,
7 intentionally ignored per platform, plus 55 runtime and 82 repository Python
tests each. GPU-contract jobs passed on both platforms. The separate real-MP4
job was still in progress at merge and is not recorded as passed.

Apple Silicon app/package build passed:
https://github.com/aleksipii/kilsatstudio/actions/runs/39936876796
Download: https://github.com/aleksipii/kilsatstudio/actions/runs/39936876796/artifacts/12493459931
The main push triggers fresh CI/package jobs; their status must be read from
Actions rather than inferred from prior evidence. Physical Mac interaction,
real-speech automatic retiming, professional artistic acceptance and the measured
five-minute production target remain open. No kilsatstudio1.0 synchronization was
requested in this merge action.


## Main CI and real-MP4 recovery

The encoder CI job was observed waiting over 25 minutes in its unbounded apt
installation step before reaching tests. The workflow now reuses the runner's
existing absolute FFmpeg/FFprobe paths, bounds fallback installation to five
minutes with noninteractive package operations/network timeouts, and bounds the
media job to 20 minutes. New runs cancel obsolete runs for the same workflow/ref.

All three actual FFmpeg tests passed locally: H.264/AAC scene encoding, embedded
art retained in MP4, and sequence duration/crossfade decoded-color assertions.
The exact workflow setup shell passed syntax/execution checks against installed
FFmpeg/FFprobe. No application/media-export implementation was changed. Final CI
results and main integration are recorded in the delivery PR; old queued/running
jobs are not treated as passed.

The first bounded CI attempt exposed a slow Azure Ubuntu mirror download of
libcodec2 and correctly failed on the installation time limit before encoding.
The follow-up selects the official Ubuntu archive/security mirror lists on the
ephemeral runner and omits recommended packages; the same real media tests and
strict time limits remain in place. No failed setup is counted as a media pass.

## Direct desktop editing follow-up (PR #93)

Adds visible scene/episode creation, character picking and root dragging with a
single key/Undo transaction, frame-snapped head drafts, and explicit local
one-shot head tilt capture for articulated custom head joints. New episode Undo
retains the preceding project and artwork. CPU drag preview moves the actor;
native compositor preview moves its bounds until release. Default procedural
heads support X/Y, not camera-driven rotation.

Camera helper uses AVFoundation/Vision, explicit macOS permission and bounded
capture. Package inventory and build evidence include its executable/hash/native
links. Python packaging checks: 36 passed; build evidence checks: 11 passed.
Desktop Rust check passed locally. Further Rust/CI results belong in PR #93;
physical Mac/camera interaction, continuous mocap and artistic acceptance remain
open. This follow-up was merged in PR #93; see the integration record below.

## Finnish desktop workspace and user guide

The desktop inspector now separates Edit, Speech, Video export and Advanced
settings instead of showing every tool in one long list. Scene/character lists
precede optional creation forms; primary actions, gesture guidance and camera
controls have Finnish labels. The toolbar wraps and body/button text is larger.
The offline Ohjeet window embeds the same ten-topic Finnish guide maintained at
apps/studio-desktop/KAYTTOOHJE.md. It covers project paths and overwrite behavior,
scene/episode creation, mouse keyframes/Undo, head/camera limits, dialogue,
English Kokoro versus imported Finnish WAV, artwork and selected/whole-project
export. Some advanced tools/errors remain in English.

Headless workspace/tab and all-ten-topic help rendering tests pass. Switching
away from head controls cancels pending camera capture. Earlier PR #93 CI for
1f424fd0a5a4b7a51035d2461edb67bd513e7c4b passed the full Audio foundation and
Apple Silicon package workflows. Final application source was verified by those same
workflows on main after the user-authorized merge; results are recorded below. Physical Mac GUI,
camera permission/calibration and continuous mocap acceptance remain open.


## PR #93 main integration (2026-10-23)

User-authorized PR #93 was squash-merged to main as
622d68afa9daea6c802e11c850c3e5b9631ec9bc. This includes direct character editing,
scene/episode creation, one-shot local camera head tilt, the focused Finnish UI
and the offline ten-topic Finnish guide.

Local final-source validation: cargo fmt, workspace/all-target Clippy with warnings
denied, and 233 desktop tests passed. One existing recognizer-enrollment test
requires /proc/sys/kernel/osrelease unavailable in the container; normal CI runs
that test without exclusions. Headless UI navigation does not change project
contents and all ten help topics render.

Fresh main Audio foundation CI:
https://github.com/aleksipii/kilsatstudio/actions/runs/39972447151
Fresh main Apple Silicon package build:
https://github.com/aleksipii/kilsatstudio/actions/runs/39972447150
Both fresh main workflows passed. Linux: 776 Rust tests passed, 7 intentionally
ignored. macOS: 775 Rust tests passed, 7 intentionally ignored. Each platform
also passed 55 Kokoro-runtime tests and 86 repository Python tests. The
container-blocked recognizer-enrollment test passed unfiltered on both CI hosts.
Both GPU-contract jobs passed. All three actual FFmpeg tests passed, and the
280-frame night-room reference MP4 exported in 238.2 seconds (placeholder tones,
not measured speech or artistic acceptance).

The Apple Silicon app passed compilation, Swift helper compilation, ad-hoc
signing, diagnostic checks, strict package inventory and executable linkage/build
evidence. Main application artifact:
https://github.com/aleksipii/kilsatstudio/actions/runs/39972447150/artifacts/12509887781
Artifact SHA-256: a32a449565f1913069610abc027c4128f087c9dd2efe0d0e56206744de07261f

The documentation follow-up corrects the guide's dialogue example to use ALEX,
the actual starter character, and records these post-merge observations. Its
embedded-guide rendering test passed. It changes no editor behavior. Further
workflow runs are recorded in the documentation PR rather than inferred from
these earlier results. Physical Mac GUI/camera, calibration, continuous mocap
and artistic acceptance remain open.


## 2026-10-25 — Tikku-ukkostudio 2: nivelet, käsiotteet ja kuvakoot

Käyttäjän kuvissa näkyneet lantion ja hartioiden aukot korjataan tasapaksuilla,
nivelen keskelle päättyvillä pyöristetyillä raajoilla, lantio-/hartiaviivoilla
ja nivelen mukaan jaetulla selkärangalla. Hahmot säilyvät tikkuhahmoina.
Esine käteen -työkalu kiinnittää yhdeksän vaihtoehtoa olemassa oleviin käden
niveliin; puhelimesta on näyttö-, taka- ja vinonäkymä. Kiinnitys koskee hahmoa
kaikissa kohtauksissa, säilyttää tunnusesineet ja tukee kumoa/toista-historiaa.
Viisi 2D-kuvakokoa kirjoittaa tavallisen kameran avainruudun. Neljän taustan
syvyyttä ja kontrastia on parannettu, suuri harmaa soikio ja pienet irralliset
reunarekvisiitat poistettu.

Sisältöversio 2 sisältää 30 erillistä projektia, 60 PNG/SVG-kuvaa, yhdeksän
käsiotteen luettelon ja kuuden kohtauksen 30 sekunnin äänettömän Full HD -demon.
Kuvallinen suunnitelma ja hyväksyntäehdot ovat
[stick-studio-art-direction.md](stick-studio-art-direction.md).

Paikallisesti läpäisivät seitsemän natiivivientitestiä (42 taivutussiluettia
ja puhelimen FK-/rasterivastaavuus), 272 työpöytätestiä, 146 Python-testiä,
55 runtime-testiä, rustfmt ja tiukka kaikkien targettien/featureiden Clippy.
Yksi desktop-testi rajattiin paikallisesti puuttuvan /proc-tiedoston vuoksi;
CI ajetaan rajaamatta. Koko workspacen 850 yksittäistä Rust-testiä läpäisivät;
kolme /proc-riippuvaista testiä rajattiin paikallisesti. Yksi viallinen
käännösvälimuistin suoritettava tiedosto rakennettiin uudelleen ja loput
paketit ajettiin erikseen. PR #99 ja mainin kaikki CI-ajot läpäisivät; tarkat ajot ja artifactit ovat alla. Todellinen MP4 tarkistettiin: 1920×1080, 24 fps,
720 ruutua / 30 s, täysi dekoodaus ja 720 000 nollasamplea. Natiivit sisältökuvat,
kuusi taivutuskuvaa ja kuusi lopullista kohtausta tarkastettiin kuvallisesti.
Tarkat paikalliset tiedot: [stick-studio-v2-local.json](evidence/stick-studio-v2-local.json).

Fyysisen Macin UI-/ääni-/kamera-/Metal-käyttötestit, ajastetut esinekontaktit,
visuaalinen IK ja jalkalukitus sekä taiteellinen/puheen laadun hyväksyntä jäävät
erikseen tehtäviksi. Developer ID, notarisointi ja lopullinen oikeustarkistus
eivät seuraa teknisten testien läpäisystä.

Paketin arkistointi rajattiin valmiisiin toimituskansioihin ja tiedostoihin.
Renderöinnin väliaikaiset työpuut eivät päädy jaettavaan ZIPiin; tuntematon
toimituksen juuritiedosto tai väärä tiedostolaji estää arkistoinnin.
Kaksi lisäregressiota tarkistaa nämä rajat.


### PR #99 ja mainin tarkistettu toimitus

[PR #99](https://github.com/aleksipii/kilsatstudio/pull/99) yhdistettiin mainiin
commitilla `43e92a6cfb5287a562838269bfd9e5a494569ce7`. Sen tree vastaa
paikallisesti tarkistettua lähdepuuta. PR:n kaikki viisi push/PR-ajoa ja
mainin kaikki kolme työnkulkua valmistuivat onnistuneesti:

- [Audio foundation](https://github.com/aleksipii/kilsatstudio/actions/runs/40239796240):
  Linux 853 ja macOS 852 Rust-testiä läpäisi; kummallakin kahdeksan oletuksena
  ohitettua testiä. Runtime-testit 55/55 kummallakin. Repositorion Python-testejä
  ajettiin 146: 145 läpäisi, yksi FFmpeg/ffprobea tarvitseva testi ohitettiin
  yleisessä testityössä. Erillinen oikean median työ läpäisi kaikki neljä
  Rust/FFmpeg-testiä. GPU-työssä 52 kirjastotestiä ja kaksi probe-testiä
  läpäisi kummallakin käyttöjärjestelmällä; neljä fyysisen GPU:n testiä jäi
  oletuksena ohitetuksi. Rustfmt, check ja tiukka Clippy läpäisivät.
- [Apple Silicon -paketointi](https://github.com/aleksipii/kilsatstudio/actions/runs/40239796225):
  valmis arm64/macOS 13+ -sovellus, artifact `12616766597`. Ladatun ulomman
  ZIPin tiiviste tarkistettiin. Sisempi sovellus-ZIP on
  `1c3b57f9317b64f84042e223edbf81416b3b38527d63b353d47f6115eee26964`.
  Alkuperäiset ZIP-tavut, suoritusoikeudet ja allekirjoitusmetadata säilyivät.
  Mach-O/arm64, build-commit ja lisenssi tarkistettiin. Sovelluksen binäärissä
  olevat malliprojekti ja käsiesineluettelo vastaavat täsmälleen mainin tavuja.
  Paikallinen pakettiauditointi läpäisi; tämä ei ole notarisointi tai
  fyysisen Macin käyttökokeen hyväksyntä.
- [Sisältö- ja videopaketti](https://github.com/aleksipii/kilsatstudio/actions/runs/40239796213):
  artifact `12616582079` ladattiin ja sen ulompi ZIP tarkistettiin. Kaikki
  34 kanonista lähdetiivistettä ja 60 PNG/SVG-tiivistettä vastaavat paikallisesti
  katsottuja lähteitä ja kuvia. Jaettavassa ZIPissä on yksi valmis MP4,
  ei renderöinnin väliaikaisia työpuita. Oikean main-MP4:n erillinen tarkistus
  vahvisti 720 ruutua, 30 s, 1920×1080, 24 fps, täyden dekoodauksen ja
  720 000 nollasamplea. Main-videon SHA-256 on
  `9993c3df6b47793eeed06d6ba744b4dbdae3fae50c1bc19b08e2c9b51cb8663d`.

Paikallisen jaettavan sisältö-ZIPin tiiviste on
`38eb6ca8d34be2c503419afcbaa45a0ed6f3b2c5f590f2d3d5f3e9969109a634`;
se sisältää lisäksi natiiveista kuvista kootun kuvaston. Sen lähteet ja
60 sisältökuvaa vastaavat tarkistettua main-artifactia. Kuuden kohtauksen
äänetön esittely ja muokattavat projektit ovat mukana.
Koneelliset havainnot: [stick-studio-v2-ci.json](evidence/stick-studio-v2-ci.json).

Tämä loppukirjaus muuttaa vain dokumentaatiota ja testitodisteita.
Tarkistetun sovelluksen ja sisältöprojektien tavut säilyvät samoina.
Fyysisen Macin käyttö-, ääni-, kamera- ja Metal-hyväksyntä, ajastetut kontaktit,
IK/jalkalukitus, taiteellinen/puheen laatukatselmus sekä tuotantoallekirjoitus,
notarisointi ja lopullinen oikeustarkistus jäävät avoimiksi.


## 2026-10-26 — Arkkitehtuuri ja agenttisuunnitelma, PR #100 yhdistetty

[PR #100](https://github.com/aleksipii/kilsatstudio/pull/100) yhdistettiin käyttäjän luvalla päähaaraan commitilla `ec442c25e84c705f0dbcf54d080f32758c7caf92`.

- Tehty: [arkkitehtuuridokumentti](KILSAT-STUDIO-ARKKITEHTUURI-JA-AGENTIT.md), kymmenen agentin tehtävänannot, orkestroijan ohje, G0–G4-suunnitelma, tiivistelmä ja vaihekohtainen tehty/tekemättä-seuranta ovat päähaarassa.
- Tarkistettu: GitHub vahvisti yhdistämisen; ennen yhdistämistä tallennettu dokumentti luettiin takaisin ja vastasi toimitusta täsmälleen. Sovellustestejä ei ajettu tässä dokumentointitoimituksessa eikä uusia CI-tuloksia oleteta.
- Tekemättä: tämän suunnitelman toteutustyö ja sen validointi odottavat erillistä jatkotehtävää. Agenttimäärittelyt eivät tarkoita käynnistettyjä agenttiprosesseja. Nykyisen sovelluksen aiempi toteutus ja hyväksymisnäyttö säilyvät yllä olevissa merkinnöissä.
- Rajaus: sovelluskoodia ei muutettu. Raportin SwiftUI-ehdotuksen suhde nykyiseen egui/eframe-linjaukseen ratkaistaan G0-vaiheessa.

## 2026-10-29 — Tarinakäsikirjoitus tekoälyltä, PR #141 ja #142 yhdistetty

[PR #141](https://github.com/aleksipii/kilsatstudio/pull/141) (commit `ae4bfe20e092b8ae57e386bc33f4168b54f32739`) ja [PR #142](https://github.com/aleksipii/kilsatstudio/pull/142) (commit `9f867050def1c01ee21db8d259225af4e50c7a66`) yhdistettiin käyttäjän luvalla päähaaraan.

- Tehty: korkean tason `kilsat_story: 1` -muoto ([story-format.md](story-format.md)), jonka Claude tai ChatGPT kirjoittaa sovelluksen kopioimasta ohjeesta. Kääntäjä tekee siitä tavallisen MotionScriptin: kohtaukset, lavasteet, kuvakoot, kovat leikkaukset, kameran ajot, katseet, 12 elettä, kävelyt, ajastetut käsiesineet (`hold`), istumisen (`pose`) tuoleineen ja tekstitykset. Työpöytäpaneeli tuottaa repliikit hahmojen äänillä taustatyönä ja ajoittaa tarinan mitattujen WAV-kestojen mukaan. Repliikkikohtaiset WAVit tallennetaan sisältötiivisteellä. Avoimen projektin omat rigatut hahmot ja kuvitetut kohtaukset ovat käytettävissä. Luonnin jälkeen näytetään automaattinen leikkaustarkastus.
- Tarkistettu: paikallisesti `cargo fmt --check`, `check`, `clippy --all-targets` (0 varoitusta) ja `cargo test --workspace` (1120 läpi, 0 epäonnistui). GitHub CI oli vihreä molemmissa PR:issä: rust macOS/Ubuntu, gpu-contracts, media-encoding, g2-export-macos, real-asset-pack, screen-slots-macos ja apple-silicon-preview. Esimerkkitarina ja jokainen uusi ele renderöitiin CPU-polulla ja kuvat katsottiin.
- Tekemättä: oikeaa Kokoro- tai Piper-synteesiä ei ajettu, ja puheajoitus testattiin valepuhepalvelulla. Työpöytäsovellusta ei käytetty käsin, eikä fyysisen Macin hyväksyntää ole. Suomessa on yksi Piper-ääni, joten hahmot erottuvat vain nopeudella. Tunteita on viisi, ja tuoli on etunäkymän piirros. Käsi- ja istumatila alkaa jokaisessa tarinakohtauksessa alusta. Komiikan, näyttelyn ja huulisynkan laatu vaatii ihmisen arvion.
