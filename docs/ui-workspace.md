# Studio workspace

Kilsat Studio's workspace groups screenplay editing, scene inspection and timeline
navigation around the existing deterministic preview. It is an original Kilsat
implementation in egui/eframe. Procedural characters remain placeholders; the
workspace does not provide a texture editor, bone editor or production compositing
system.

## Panels and navigation

| Area | Controls |
| --- | --- |
| Top toolbar | Open, Save, Validate, Undo, Redo, Load sound-check demo and task cancellation |
| Left Project tab | Scene list and character/asset references; select a scene or actor |
| Left MotionScript tab | YAML editor and explicit screenplay path; Save replaces that path |
| Center Scene | Procedural preview and scene controls |
| Right Inspector | Character asset/voice details, editable selected-event draft, dialogue preparation and collapsible runtime/alignment settings |
| Bottom Timeline | Actor tracks with Move, Emotion, Gesture and Speak lanes, playhead and playback controls |

Drag panel boundaries to resize the workspace. Click a timeline event to inspect
its editable properties, or an actor label to select that character. Click or drag the
time area to scrub at the shared 24 kHz scene position; this seeks audio when a
source is active. Timeline bars are navigation aids: dragging does not edit event
timing. Use the Inspector or edit MotionScript to change the scene. Before dialogue is
prepared, Speak events have point markers rather than measured duration bars.

The Inspector displays voice preset, language and speed; edit them in YAML.
Expand **Kokoro runtime / alignment** for local paths, diagnostics and timing JSON,
**Selected audio / waveform** for a single prepared line, or **Scene audio / mixed
waveform** under the timeline for the full scene. **Resume** continues the active
scene audio or the paused silent scene transport.

## Working with a screenplay

Open or edit the YAML screenplay, then validate it. Validation connects character
and scene references to the preview and dialogue selections. Editing invalidates
previous selections and prepared state so an old result cannot silently represent
a new script. Character asset strings currently identify screenplay resources;
the procedural preview does not load their image files.

Use silent scene playback to inspect movement before installing any speech
assets. Preparing full-scene audio is a separate background job. It generates
speech with the configured local Kokoro environment or imports explicit WAVs,
then mixes the scene. Missing alignment keeps mouths at rest. Speech generation
and a successful prerequisite check do not certify perceptual lip synchronization.

Runtime settings, alignment JSON and audio controls retain the behavior described
in the [desktop guide](../apps/studio-desktop/README.md). See [practical testing](practical-testing.md)
for the model-free sound check and real Mac acceptance checklist. Audio-device
playback is macOS-only; no real Kokoro or hardware acceptance is implied by the
workspace layout.

## Export the prepared scene

After **Prepare full scene audio** finishes, use the Inspector's **Scene export ·
SVG + WAV** section. Enter a new absolute output directory and choose **Export
scene bundle**. The Inspector and status strip show progress; the toolbar offers
**Cancel export** while the worker runs. Cancelling export preserves the prepared audio. The sound-check demo
can exercise this path without Kokoro; its tones are not speech and its mouth
cues are not measured alignment. See [frame export](frame-export.md) for output
files, limits and the separate headless commands. The bundle contains SVG frames,
WAV and JSON; it is not an encoded movie.

## Design boundary

High-level panel separation was informed by the public
[EffectCraft product description](https://github.com/storytold/effectcraft), which
describes Project, Composition, Timeline and Controls panels. Only that product
organization is a reference. Kilsat's source and interfaces are independently
authored, with no Storytold source or dependency integrated and no feature-parity
claim.

## Edit and undo

Select a timeline event to edit its actor and start time in the Inspector.
Move events expose position and duration; expressions and gestures expose their
supported values; dialogue exposes text, visual expression metadata and optional
imported-audio path. Choose **Apply event** to validate the entire screenplay.
Rejected edits leave the document unchanged. Applying normalizes YAML formatting
and removes comments; **Undo** restores the exact previous source.

The toolbar provides **Undo** and **Redo**, with Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z.
These shortcuts apply to the screenplay when its editor has focus or no other
text field is active. Other text fields retain their own undo behavior. A focused
screenplay editing session is grouped until focus leaves the editor or validation,
save, apply or undo commits it. Opening a file or demo starts a new history.
History retains up to 64 snapshots and 8 MiB, separately from the current document.

Applying an event invalidates prepared playback/export audio. Editing dialogue
also clears its pasted scene alignment and the selected-line alignment while
preserving other dialogue mappings. Free-form source edits, opening a file, Undo
and Redo clear pasted alignments because they may no longer match the text or
voice. History restores screenplay text, not audio assets or alignment paste data.
Prepare audio and provide fresh timing evidence when needed.
