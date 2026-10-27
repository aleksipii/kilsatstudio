# Cinematic camera presets

Status and remaining production work are tracked in [master progress](master-progress.md).
This package adds editable camera templates, not a new runtime camera command.

Open the scene's composition settings and expand **Cinematic camera presets**.
Choose **Push in**, **Pull out**, **Pan left** or **Pan right**, adjust the generated
keys if desired, then use **Apply settings**. The complete composition edit uses
one existing screenplay/history transaction; Undo restores the previous source.
Before Apply, these changes remain in the composition draft.

Each preset generates exactly two ordinary scene-local camera keys, at zero and
scene duration. Push in interpolates zoom from 1 to 1.25, pull out from 1.25 to 1.
Pans move the camera center from zero to -100 or +100 scene pixels at zoom 1;
scene content therefore moves in the opposite screen direction. All four start
from centered framing. Existing camera keys are replaced only by an explicitly
labelled **Replace keys: …** button. Other composition fields are preserved.
The camera track remains manually editable; applying a preset does not retime
actor events, dialogue, masks or scene transitions.

MotionScript's `apply_camera_preset` API additionally rejects an existing track
unless `replace_existing` is true. It validates the complete candidate script,
returns normalized YAML, and writes no files. Camera templates use the ordinary
linear camera evaluator shared by preview and export, with no runtime preset
state or new serialized schema.

Tests cover editable key round trips, explicit replacement, scene selection and
invalid durations, plus deterministic scene evaluation after seeking backward.
Physical Mac interaction and perceptual framing acceptance remain separate.
Actor-free audio cues, conditional/reusable rules and general compiled track
editing remain open; see [screenplay workstream](workstreams/next-screenplay-timeline.md).

Presets and shot templates only write camera keys. Whether a resulting frame keeps
its subjects inside the wide (4 %) or close-up (8 %, face at least 20 % of frame
height) margins is checked separately by `motion_render::framing::check_framing`;
see [render contract § Framing](render-contract.md#framing-wide-and-close-shots).
The G2 test scene (`examples/g2-minimal-scene/`) uses a hard cut built from two
camera keys 10 ms apart, which no 24 fps frame samples.
