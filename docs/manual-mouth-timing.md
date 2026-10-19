# Manual mouth timing

This workstream follows [master-progress.md](master-progress.md) and the
[remaining-work checklist](remaining-work.md). It adds an editor for reviewing
and changing mouth cues from an explicitly generated or loaded local estimate.
It does not run a forced aligner or establish perceptual timing quality.

## Use

1. Prepare the selected dialogue line so its actual decoded audio is available.
2. In **Local estimated mouth timing (Rhubarb)**, explicitly estimate the prepared
   line or load its saved timing key. Eligible enrolled packages can reuse an
   exact saved estimate. No recognizer is needed when loading an existing key.
3. Expand **Edit mouth timing in audio samples**. Change each cue's start sample,
   end sample or shape; split or delete a cue, or append a rest cue. Reset restores
   the unchanged original estimate.
4. Click **Review manual edits**. A bounded background worker verifies the exact
   prepared PCM and current transcript. Changed drafts cannot accidentally apply
   the original estimate. Invalid intervals never replace project timing.
5. Click **Apply reviewed timing**. Existing audio is prepared again with the
   reviewed timing, without synthesis or a model download. The project stores
   timing inputs, and the application records this application as one Undo step.

Intervals use half-open sample boundaries `[start, end)`. Adjacent intervals
cannot overlap; gaps and samples outside cues are rest. Shapes A–H/X retain the
existing Rhubarb-to-viseme mapping. An empty cue list is an explicit all-rest edit.
The editor accepts at most 1,024 cues; line and combined scene JSON each have a
100 KB limit. The current estimate/audio path supports mono 24 kHz prepared audio
up to five minutes. Large audio hashing stays off the UI thread.

## Persistence and provenance

Manual results use the existing bound alignment envelope with `Manual` provenance.
The cache key includes exact decoded PCM, transcript, original estimate identity
and a digest of the edited sample intervals and shapes. The existing deterministic
evaluator stores sample boundaries as seconds divided by the same audio rate.
Representative labels such as `m` for a closed mouth carry a **viseme**, not a
claim that an actual /m/ phoneme was spoken. Manual edits do not overwrite the
recognizer's estimate cache or publish a result into automatic estimator reuse.

Optional project `timing.line` and `timing.scene` strings preserve the editor
inputs. Older projects default to empty timing. Project loading checks JSON syntax
and bounds; audio preparation separately checks exact PCM/text identity. Timing
from different audio or dialogue is rejected even when duration is unchanged.

Changing source, selected line, transcript or prepared audio discards pending
results. Edits are staged until explicit review and application; intermediate
table changes do not add project history entries.

## Still open

- Waveform dragging, playback audition of an unapplied draft and direct editing
  of previously applied manual envelopes; this table starts from a fresh, reused
  or explicitly loaded estimate.
- A verified forced-alignment environment and measured phoneme/viseme accuracy
  on licensed real speech material.
- Physical Mac audio, seek/pause/synchronization and accessibility acceptance.

Automated structural tests cover sample boundaries, every supported shape,
overlaps, budgets, exact identity rejection, independent edit keys, project
roundtrips and history. CI execution results belong in master-progress; these
tests do not establish perceptual quality or physical device acceptance.
