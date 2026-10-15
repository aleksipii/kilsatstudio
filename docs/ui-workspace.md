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
its editable properties, or an actor label to select that character. Click or drag
blank ruler or track space to scrub at the shared 24 kHz scene position; this seeks
audio when a source is active. Drag an event bar or marker horizontally to change
its start time on the project frame grid. Before dialogue is prepared, Speak
events have point markers rather than measured duration bars.

The Inspector edits voice preset, language and speed through a detached character voice draft.
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


## Drag event timing

Drag a Move, Emotion, Gesture or Speak bar/marker horizontally. The provisional
position appears as a ghost while the screenplay remains unchanged. Release to
validate the complete screenplay and apply one undoable edit; a rejected edit
keeps the original source. Press **Escape** to cancel before release. A click
selects the event without retiming it. Event dragging is disabled while a
background task is active.

Start times snap to the project's frame rate and stay within the scene. Move and
gesture durations are preserved and included in the end-boundary clamp. Dragging
does not trim durations, change actors, create events or delete them. Use the
Inspector for other supported property edits.

Accepted retiming invalidates prepared audio/export state through the same edit
path as the Inspector. Prepare the scene again before playback or export. Speech
length is known only after audio preparation: dragging a Speak event does not
analyze its duration or guarantee that the generated/imported line will fit.
Scene preparation can reject speech that overruns the scene or overlaps another
line from the same actor. Mouth timing still requires explicit alignment.

## Add, duplicate and remove events

In the Inspector, choose **+ Event** and then **Move**, **Expression**,
**Gesture** or **Dialogue**. A new draft starts at the frame nearest the playhead,
within the selected scene. Choose its actor and properties, then **Apply event**.
**Cancel draft** discards it without changing the screenplay. The event is appended
to the scene's source list; timing determines playback, while simultaneous events
retain source order. New moves use the evaluated actor position. New dialogue uses
the configured voice; characters without a voice need an explicit imported-audio
path before Apply can succeed.

Select an existing event and choose **Duplicate** to create a detached copy at the
playhead. Its fields are preserved, but the start is frame-snapped. Choose a valid
start/duration before applying: copied movement can overlap and a copied duration
can exceed the scene. No duplicate is committed until validation succeeds.

**Delete event** requests removal. **Confirm delete** performs one undoable edit;
**Cancel delete** leaves the source unchanged. Changing the source, scene or event
selection cancels a stale confirmation. No keyboard Delete action removes events.
These actions are disabled during generation, diagnostics and export.

Each accepted add or removal invalidates prepared scene audio. Adding/removing
**Dialogue** also clears all pasted alignments because sequential dialogue IDs may
shift, including in later scenes. Other event kinds preserve the timing maps.
Generated audio cache entries remain available; reprepare the scene to reuse valid
entries. Undo restores exact source text, not alignment paste data or prepared
assets. All Apply/add/remove operations normalize YAML and remove comments, as
described above. Full speech duration is still checked during audio preparation.

## Character voice settings

Select a character and expand **Character voice · Kokoro**. **Enable Kokoro voice**
controls whether the character has a synthesis configuration. Edit the preset name,
English language and speed (0.5–2.0), then choose **Apply voice**. Quick choices
`af_heart` and `am_michael` set American English; other presets must already exist
in the configured local voice directory. The editor accepts preset identifiers of
1–128 ASCII letters, digits or underscores. Validation does not establish that an
asset is installed or licensed. No assets are downloaded or scanned by this panel.

**Reset voice draft** discards uncommitted voice changes. Accepted voice edits use
the same whole-screenplay validation and Undo/Redo as event edits. Removing a voice
is rejected when any of the character's dialogue still needs synthesis; explicit
imported audio remains available. Applying a changed voice resets pending event
edits and prepared scene audio. Prepare the scene or generate a selected line again
to hear the configured voice; real Kokoro synthesis still requires local assets.

Pasted alignments for that character's synthesized lines are cleared across all
scenes. Other characters and explicit imported-WAV alignments are preserved.
Generated cache entries remain on disk and are reused only when their content
identity matches. Undo/Redo restore screenplay text and clear pasted timing data,
as described above. No-op or invalid voice Apply leaves the document/history and
prepared state unchanged. Native Mac listening and perceptual lip-sync quality
remain separate acceptance checks.
