# Local head pose helper

The packaged Mac app starts this helper only after an explicit user action.
AVFoundation captures video frames; Vision reports the largest confidently
detected face. No microphone input, frame/audio files, network or downloaded
model is used. The app asks macOS for camera permission when needed.

No arguments preserves the one-shot JSON response: roll, optional yaw/pitch in
radians, then exit. `--stream --duration 60 --fps 12` writes JSONL with seq (from
zero), time (monotonic seconds since capture setup), roll and optional yaw/pitch.
Duration accepts 1..60 seconds and FPS accepts 1..20. `--help` exits successfully
without opening a camera; unknown/duplicate/invalid options exit 2 before access.

The helper bounds permission/setup to 30 seconds, no measurable face to five
seconds and stream capture to the selected duration. A last-resort 92-second
helper watchdog and 95-second parent bound remain. The parent drains bounded
JSONL and stderr, validates every angle/sequence/time, and kills/waits on cancel,
Stop, malformed data or deadline. A parent-death monitor exits the helper when
the requesting desktop dies. Nothing opens a camera at application startup.

The UI stages one-shot values or a maximum 30-second/600-sample recording. Stop
is not Apply. A separate reviewed Apply supplies ordinary frame-snapped custom
head keys to the document's existing single Undo transaction. Only 2D roll is
recorded; yaw/pitch are displayed as measurements, not body or 3D mocap.
Default procedural heads still support X/Y only. Context changes cancel the
camera and discard uncommitted draft data. Live preview must use the caller's
cloned Script, never its saved document or actor root.

## Physical Mac operator checks (not automated acceptance)

1. Open the packaged app in Finder. Confirm no camera indicator before Capture
   or Start tracking. Expand/collapse Webkamera while active: its indicator and
   Stop/Cancel remain visible outside the collapsing body.
2. Select a custom rig with a head joint; pause playback. Start tracking, approve
   the macOS prompt, tilt your head and check direction. Deny permission once and
   confirm an actionable error. Repeat after changing macOS camera permission.
3. Record a short movement, Stop recording, and confirm the document has not
   changed. Review/invert the draft, then Apply. One Undo must remove all recorded
   head keys while preserving actor root, head XY/scale and other authored keys.
4. Change source, scene, selected actor, playhead, tab or playback while tracking.
   Confirm camera exit and no stale preview/result. Cancel must discard a draft.
5. Leave the frame for over a second while recording. Confirm rejected draft;
   leave it for five seconds and confirm capture exits. Test a camera in use by
   another app and unplug an external camera. No process may remain indefinitely.
6. Attempt a recording longer than the scene's remaining time. Confirm error and
   no partial source edit. Export reviewed head motion and compare preview/video.

CI compiles/signs/inventories the executable and may call --help/invalid options;
it does not exercise physical camera permission, pose direction/calibration,
face reliability, artistic quality or GUI device behaviour. Those checks remain
unverified until an operator records actual retained evidence.

Primary APIs:
- https://developer.apple.com/documentation/vision/vnfaceobservation
- https://developer.apple.com/documentation/avfoundation/avcapturedevice/requestaccess(for:completionhandler:)
