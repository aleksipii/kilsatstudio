# Actual-audio conversational direction

The legacy direct function retains its previous one-shot-per-line draft behavior.
The opt-in direct_conversational path adds at most four half-second listening
reactions, bounded head accents, and cameras fit to actual prepared custom-rig
geometry (including current expressions, blink and mouth alternatives). The
shared shot compiler and scene clock remain authoritative. Up to sixteen final
shots and sixty seconds include reactions; optional cuts are omitted when the
speech already fills the budget. Speech is never shortened to fit a reaction.

The pure direct_with_audio adapter receives sample counts and audio references;
it does not inspect PCM or certify them. Imported references are inserted before
semantic validation, permitting imported speech for characters without a voice.
Plain dialogue remains explicit actor:text with optional validated emotion tags.
This is deterministic staging, not general intent recognition or professional
artistic acceptance.

## Desktop integration contract

The bounded direction_worker accepts a source snapshot, dialogue, existing
absolute project root, cancellation token, Arc<dyn SpeechProvider>, generation
and an optional map of imported WAVs keyed by one-based physical dialogue line.
Imports take precedence over inference. Work stays off the UI thread.

The worker ignores provider duration metadata. It snapshots each bounded regular
WAV, validates its complete mono PCM16 24kHz samples and derives sample counts
from the decoded bytes. Files above sixty seconds/8MiB, malformed headers,
truncation, unknown import keys and symlinks are rejected before unbounded work.
Valid WAVs are published atomically, without overwriting prior files, under
assets/auto-direction/<SHA256>.wav. Existing snapshots are verified before reuse.
The whole preparation/direction batch has one cooperative 300-second watchdog;
providers must honor the shared cancellation token. Rendering/export timing is
not included in this preparation measurement.

Ready contains the original source, directed result, verified source-line/actor/
relative-path/sample-count/SHA256 records and elapsed preparation time. The UI
must check generation, current source/document revision/dialogue/project context
and cancellation before applying exactly one Undo transaction. A cancelled or
stale result must never replace the current document. Retained immutable WAVs are
not removed by Undo, so unrelated projects/media cannot be deleted by a history
operation. No source is mutated by the worker itself.

## Validation and remaining acceptance

Regression tests cover speech preservation, optional-cut budgets, deterministic
reactions, voiceless imported speech, small-canvas custom-rig camera controls,
provider metadata lies, immutable audio snapshots, malformed WAVs and cancellation.
The parent integrator records actual Rust/CI results; this document is not a test
pass claim. Real Finnish/English voice quality, annotated lip-sync, physical Mac
GUI/camera behavior, original asset rights, artistic/comic review and the complete
<=300-second render/export pipeline remain independent acceptance gates.
