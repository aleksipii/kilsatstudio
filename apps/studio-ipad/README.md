# Koeta Studio for iPad

Native SwiftUI app for iPad (iPadOS 16 or newer, every iPad from 2017 onwards:
iPad, iPad mini, iPad Air and iPad Pro). It uses the same Rust animation core as
the Mac app through `crates/motion-ffi`, so a scene looks the same on iPad as in
the Mac preview and export.

## What it does

Phase 1:

- Open MotionScript (`.yaml`) or a Koeta project (`.kilsat`) from the Files app,
  or one of the bundled examples.
- Scene list, play/pause, frame stepping, a timeline slider, and scrubbing by
  dragging across the picture with a finger or Apple Pencil. Tap the picture to
  play or pause. A hardware keyboard works too: space, ← and →, ⌘O.
- Works in portrait and landscape, Split View, Slide Over and Stage Manager.

Phase 2 (images and audio):

- **Open a folder** (⇧⌘O) to give the app access to a project's files. A file
  picked on its own only grants access to that file, so a project with images
  or audio files should be opened through its folder. With several project
  files in the folder, the app asks which one to open. The app's own folder
  also shows up in Files as *Koeta Studio*, so whole project folders can be
  copied onto the iPad.
- **Image assets** of a `.kilsat` project load as on the Mac (PNG or JPEG
  inside the project folder, 128 MiB decoded at most). A missing or unreadable
  image stops opening with a message that names the file.
- **Audio**: each scene's dialogue and audio cues play from imported WAV files
  (24 kHz mono PCM16, the same rule as the Mac), mixed by the same code and on
  the same 24 kHz clock as export. While audio plays, the audio position picks
  the frame, so picture and sound stay together. Scenes up to 10 minutes long.
- **Mouths** follow the same scene manifest as export. Without a manual or
  forced alignment the mouth timing is estimated from the line's text, as in a
  model-free Mac export, not measured from the audio.
- **No speech synthesis on iPad.** If a line has no `audio:` file or the file
  is missing, the scene plays silently and a note under the controls lists the
  lines. Nothing is generated in their place.

Phase 3 (screenplay editing and movie export):

- **Edit the screenplay** (pencil button, ⌘E): the MotionScript editor opens
  beside the preview (below it in narrow windows). Smart quotes, dashes and
  autocorrection are off, since they would change the YAML. A pause in typing
  checks the text with the shared core: valid text updates the preview and
  scene list straight away, without saving; an error names its kind (syntax,
  unsupported command, invalid value or reference) and line, tapping it selects
  the line, and the preview keeps the last valid version.
- **Save** (⌘S) writes over the opened file with file coordination. A `.kilsat`
  project keeps its artwork, assets and settings; only its screenplay changes.
  A bundled example cannot be overwritten, so *Tallenna kopio* copies it (with
  its folder and assets) into the app's Files folder and continues there.
  Opening another file with unsaved edits asks first.
- **Export video** (film button, ⇧⌘E): one scene, or the whole project with its
  crossfades, as an MP4 (H.264 and AAC, BT.709) at the project's own size, into
  Files › Koeta Studio › Viennit, then share it or save it to Photos. Frames and
  the mix come from the same core code as the Mac export (`ExportFrames` in
  `motion-export`); AVFoundation encodes them instead of FFmpeg. Up to 10
  minutes, cancellable. Dialogue must come from imported WAV files, as in
  playback; a missing file stops the export with the lines named.

Phase 4 (timeline and events by touch):

- **Timeline** (⌘T) below the preview: one track per character with four lanes
  (movement; facing, pose and expression; gestures and props; dialogue), a
  ruler and the playhead. Tap or drag on the ruler or an empty spot to move the
  playhead; pinch or use the magnifier buttons to zoom.
- **Tap an event** to select it and jump to it. **Drag the selected event** to
  move it, or its end handle to change the length of a move or gesture. While
  dragging, the position snaps to frames in the shared core
  (`motion_scene::event_edit`), the same code the Mac timeline uses, so an
  event lands on the same frame on both.
- The bar under the timeline edits the selected event: its gesture,
  expression, facing, pose or gait from the accepted values, a line's text or
  the item held, one frame earlier or later, *Toistopäähän* (to the playhead),
  length ±1 frame, *Kopioi toistopäähän* and *Poista*. **Add** (+) creates a
  move, gesture, expression, facing or line for a character at the playhead.
  A new line needs the character's voice or an imported audio file, as on the
  Mac; nothing is synthesized on iPad.
- Every edit is checked as a whole screenplay and becomes an unsaved change
  shown at once in the preview and the editor; *Kumoa* and *Tee uudelleen*
  (⌘Z, ⇧⌘Z) step through them, and ⌘S saves as before. MotionScript written
  by an edit is normalized YAML, so comments in the screenplay are not kept
  (the same as event edits on the Mac). While the screenplay has an error the
  timeline shows the last valid version and does not edit.
- A line's length comes from its audio, so dialogue shows as a chip at its
  start rather than a bar.

Not yet included: posing with Apple Pencil, speech synthesis (Kokoro) and
screen-video cues.

## Building

Requires Xcode, [XcodeGen](https://github.com/yonaskolb/XcodeGen) and the Rust
targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
scripts/build_ipad_core.sh release           # Rust core -> Build/KoetaCore.xcframework
cd apps/studio-ipad && xcodegen generate     # -> KoetaStudio.xcodeproj
open KoetaStudio.xcodeproj
```

To run on your own iPad, choose your Apple ID team under Signing & Capabilities
(a free Apple ID works for personal testing; TestFlight and the App Store need a
paid Apple Developer account). The simulator build is arm64 only, so build it
on an Apple Silicon Mac.

For simulator smoke tests, the launch arguments `--open-example NAME` (a bundled
example such as `kilsat-season/ep02.kilsat`, or a path inside the app's Files
folder), `--scene N` (zero-based) and `--play` open a file, select a scene and
start playback once its audio is ready. `--export` (with `--export-all` for the
whole project) exports once the file opens and prints `KOETA_EXPORT finished
PATH` or `KOETA_EXPORT failed MESSAGE`; `--edit` opens the editor, and
`--draft-append TEXT` (`\n` for line breaks) with `--save-draft` types into
and saves the screenplay. `--timeline` shows the timeline, and
`--timeline-test` moves, resizes and adds an event, then undoes and redoes,
printing `KOETA_TIMELINE step events=N first=...` after each step.
