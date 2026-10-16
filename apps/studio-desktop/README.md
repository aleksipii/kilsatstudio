# Native studio workspace

Run from the repository root:

```sh
cargo run -p studio-desktop
```

This native egui/eframe app edits and validates MotionScript YAML, selects dialogue,
submits bounded background generation/import jobs, previews duration and waveform,
and scrubs a sample-based symbolic mouth display. macOS device playback is available as described below; it also renders original procedural characters with deterministic scene movement, expressions and waves. PNG sequences and MP4/WAV exports use the same scene evaluation. macOS GUI execution and real Kokoro
synthesis require local platform testing.

## Set up Kokoro inside the application

Open **Kokoro runtime / alignment** and use **Kokoro · local speech setup**.
The application starts the CPU runtime locally; no Terminal commands are needed
for installation, connection, speech testing or playback. Python 3.10–3.12 must
already be installed. Leave the Python path blank for detection, or enter its
absolute executable path.

1. Choose an installation folder (defaults to `~/Library/Application Support/Kilsat Studio/Kokoro`).
2. Review and enable the download consent checkbox, then choose **Install local Kokoro**.
   This explicitly downloads dependencies, model weights, the two English voices
   and language assets. The checkbox starts unchecked. Long operations run on a
   bounded background worker with progress, cancellation and recoverable errors.
3. For an existing installation created by this installer, choose the same folder
   and **Connect existing local installation**. Connection performs no downloads.
4. Choose `af_heart` or `am_michael` and press **Test English CPU speech**.
   The test generates a separate WAV and JSON report; their locations and metrics
   appear in the panel. Choose **Play speech test** and listen on your Mac.
5. Prepare selected dialogue or full-scene audio with the normal studio controls.

Installation/connection fills the advanced Python, model, voice, cache and
revision fields automatically. It does not modify the screenplay, dialogue jobs,
Undo history or manual alignment. The standalone speech test uses separate audio
and playback state, so it does not become a scene clip or drive its mouth cues.
A successful test report verifies generation and basic WAV validity, not perceptual
speech or lip-sync quality. Actual playback needs a macOS audio output device.
The profile is saved by the installer; reconnect the folder after reopening the app.

**Advanced local runtime settings** remain available for separately installed
assets: Python executable, sidecar script, model config, weights, voice directory,
cache and immutable model/dependency revision. Manual edits disconnect the selected
managed profile. Voice preset, language, speed and imported `audio` path come from
YAML. Imported WAV must be nonempty 24 kHz mono PCM16. Paths resolve from the
configured project root. Screenplay edits invalidate selections until revalidation.

Before generation, choose **Check local Kokoro setup**. A background check reads
local asset hashes and installed package metadata using the configured Python.
Missing files or packages are reported without freezing the editor. It performs
no inference or download; successful diagnostics do not prove voice quality,
G2P operation or license compliance. See the [runtime guide](../../services/kokoro-runtime/README.md)
for JSON reports and the opt-in real macOS smoke test.

Paste optional alignment JSON before generation/import:

```json
{"source":{"kind":"manual"},"duration_seconds":1.0,"phonemes":[{"phoneme":"m","start_seconds":0.1,"end_seconds":0.3}]}
```

Set duration to the actual audio duration. The alignment parser checks interval
structure and supported phonemes; it does not certify perceptual lip-sync quality.
Without alignment, the mouth remains at rest. The preview offset is session-only;
edit `at` in YAML to retain a timing change. Use the explicit screenplay path and
Open/Save buttons for UTF-8 YAML (100 KB limit); Save replaces that path. Runtime
settings and preview offsets remain session-only.

The UI deliberately labels these boundaries. No native Rust TTS, Metal acceleration,
hardware latency calibration or perceptual alignment validation is claimed.

## Playback stage

On macOS, prepare a dialogue WAV and use Play from cursor, Pause, Resume, Stop,
or the scene playhead slider. A bounded worker opens the current default output
device and retains the stream. PCM stays shared; seek replaces the source at an
exact clip-local frame. Mouth preview reads Rodio's consumed source position,
including the seek offset, rather than advancing a separate wall-clock timer.
This does not measure hardware latency; perceptual synchronization requires
listening and visual validation on a real Mac. The UI continues polling while a
device opens or reports an error. Linux offers waveform scrubbing and an explicit
unsupported-device message; audio-device output is currently macOS-only.

Device-open errors appear in the UI and Play can retry. Rodio runtime device
faults currently go to stderr; an unplugged/stalled device may require restarting
the app. The user has confirmed opening the preview on a Mac. Hardware playback and
interaction beyond launch have not yet been confirmed.


## Scene preview stage

The default screenplay is `examples/hello-emma.yaml`. Validate it, select a scene,
and use Play scene (silent), Pause scene, Restart scene or the scene slider.
Movement, expressions and wave gestures are evaluated from the same scene time
on every frame. Scene selection, screenplay edits and validation failures discard
stale state. The preview uses simple original procedural characters rather than
loading external character assets.

Silent playback advances a quantized 24 kHz scene clock using elapsed time. While
a prepared dialogue source is loaded for this scene, its source sample position
drives the scene and matching actor mouth instead. Scene controls stop that source
before switching to silent transport; silent playback does not automatically mix
or play all dialogue events. The waveform and standalone mouth detail remain
available. Alignment quality and device latency still require real Mac testing.

## Full-scene dialogue stage

After validating a screenplay, choose a scene and prepare its dialogue. Preparation
runs on one bounded background worker and reports completed lines. Generated and
imported WAVs are mixed using the same `prepare_scene`/`render_scene` pipeline as
the headless exporter. Different actors may overlap; overlapping lines from the
same actor fail explicitly because one mouth cannot follow two tracks reliably.
The complete scene, including leading silence, has one source clock for playback,
movement, waveform cursor and actor mouth states. Pause/resume and seek use that
same mixed track. macOS hardware latency and perceptual sync remain unverified.

Optional scene alignment JSON maps global dialogue IDs to `Alignment` objects.
IDs are the source-order IDs from `dialogue_jobs`, including prior scenes. Use only
IDs belonging to the selected scene; unknown IDs fail instead of being ignored.
Missing alignments keep the relevant mouth at rest. The single-line alignment
field remains separate from full-scene alignments.

Preparation errors include scene, dialogue ID and actor where applicable. Failed
preparation publishes no partial playable scene; completed cached speech remains
available for retry. No model assets are downloaded and no timestamps are invented.

### Cancel speech generation

While dialogue or scene preparation is running, choose **Cancel generation**. The interface stays responsive and waits for the worker to stop before allowing a retry. Kokoro terminates and reaps its active sidecar and removes partial staging files; completed cache entries remain available. Imported audio and mixing stop at processing boundaries. Custom speech providers using the default cancellation implementation stop before or after their synthesis call.

See the [practical test guide](../../docs/practical-testing.md) for cancellation,
retry, voice listening and cache checks.

## Workspace guide

See [Studio workspace](../../docs/ui-workspace.md) for screenplay, preview and
timeline navigation, and [frame export](../../docs/frame-export.md) for rendered
SVG output and the distinction between a frame sequence and encoded video.

To export a complete prepared scene, open **Scene export · SVG + WAV** in the
Inspector, enter a new absolute output directory, and choose **Export scene
bundle**. Its parent must exist; existing directories are rejected. Progress and
**Cancel export** are shown in the Inspector/status strip and toolbar respectively. Cancelling preserves prepared scene
audio and removes partial output where possible. The sound-check demo can test
this path without speech assets. Output is SVG frames, WAV and JSON, not encoded
video.

### Project files and structure

Open or save a versioned `.kilsat` document using the same path field as YAML;
launching with a file path also opens that document. A project preserves the exact
screenplay text and its asset inventory. Imported audio resolves from the project
file's parent directory. Saving a `.kilsat` file requires that directory to match
the configured project root, so Save As cannot silently relocate relative paths.
YAML retains its explicit configured root behavior. The desktop source limit is
100 KB; project image inventory is retained but image rendering is not implemented.

The Project tab's **Add / duplicate / delete** section accepts a new stable ID,
adds or duplicates scenes and characters, and confirms deletion. Referenced
characters and the final scene cannot be removed. Every accepted change validates
the complete screenplay, creates one source Undo step and invalidates prepared
audio. Character duplication preserves voice settings and asset references.

### Media export

The scene export section selects SVG, PNG or MP4. All formats use prepared full
scene audio and the shared evaluated rig. MP4 requires an explicit absolute path
to an already installed local FFmpeg executable and even project dimensions; no
encoder is downloaded or searched on PATH. Choose a new absolute output directory.
MP4 retains the PNG/WAV/timing interchange files. Rendering and encoding run on the
existing bounded export worker and can be cancelled. When all frame images are
ready, the UI reports finalizing/encoding until the worker confirms completion.


## Artwork and actor transform tracks

The editor now supports background and foreground pen/rectangle/ellipse and PNG/JPEG image
layers with rectangular clips, visibility, order and opacity. Resources decode on
a bounded worker; Reload images refreshes same-path edits. Save `.kilsat` v2 to
retain artwork; v1 files migrate without losing source/assets. YAML save refuses
artwork loss. Artwork history is separate/bounded and reference-safe; it is not
full project Undo. Actor attachments follow translation only.

A detached keyframe editor stores actor root offsets, radians, scales and easing
in MotionScript. Procedural actor geometry and faces use the same evaluated root
transform as export. Per-bone textures, arbitrary masks, advanced image editing
and Mac visual acceptance remain open. See `docs/art-format.md`,
`docs/art-rendering.md`, `docs/keyframe-editor.md` and `docs/art-acceptance.md`.

## Timed scene background colors

In the Inspector's project/scene settings, expand **Background, camera, layers and
fades**. Set the base `#RRGGBB` color, then add background keys with times in seconds
and colors. Each key switches the scene background at its exact time and holds
until the next key; the base color applies before the first key. Keys can be
edited or removed in the detached settings draft. Times must be finite, unique,
ascending and within the scene duration (up to 4096 keys).

**Apply settings** validates and commits the full draft as one screenplay Undo
step. Invalid colors/times leave the screenplay unchanged. Preview and exported
frames use the same background evaluation. Settings controls are disabled while
an active operation holds the document.

## Timed background artwork

In **Artwork / layers**, use **Place background image** for a project-canvas-sized
PNG/JPEG behind the actors, or **Place image** for a foreground image. The plane
selector also moves existing drawings and images between background and foreground.
Layer order is preserved within each plane. Camera transforms, opacity, actor
translation attachments and rectangular masks still apply.

Enable **Visible during interval** and enter scene-local start/end seconds. The
start is included and the end excluded; disabled intervals span the whole scene.
Use `0 ≤ start < end ≤ scene duration`. Invalid intervals leave the artwork
unchanged. Artwork Undo/Redo includes the plane and timing; screenplay and audio
are unaffected. Save as `.kilsat` to retain these fields. Preview and export use
the same timing boundaries. Scene time is independent of the audio playback test.

## Bound alignment imports and cache

The selected-line and full-scene alignment inputs accept bound timing envelopes
(`schema_version`, `key`, optional `alignment`) as well as legacy Alignment JSON.
For a scene, map dialogue IDs to either form. Legacy timing is explicitly shown
as **unbound**: its audio duration is checked, but it does not carry exact audio
and transcript identity. Blank inputs keep mouths at rest.

Bound envelopes check exact decoded PCM and the current dialogue text. A timing
payload is stored under the configured Kokoro cache directory's `alignments`
subdirectory; both preparation paths use that location. An envelope without a
payload requests only its exact cache entry. Missing entries leave the mouth at
rest. A changed transcript/audio or corrupt entry reports a recoverable preparation
error rather than silently using stale timing. Use a new manual edit identity
when deliberately revising an existing bound alignment.

Preparation shows imported, cache-hit, cache-miss, legacy-unbound or absent timing
status per line. JSON parsing, PCM hashing and cache I/O run on bounded background
workers, with a 100 KB JSON input limit. These statuses establish structural
provenance only; they do not measure perceptual quality or provide an automatic
aligner. Device playback and listening acceptance still require Mac testing.
