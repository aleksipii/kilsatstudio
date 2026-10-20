# Plan: professional dialogue animation from a screenplay alone

Status: plan, 2026-10-20. Nothing here is implemented unless a later section of
[master-progress.md](master-progress.md) says so. The authoritative requirement
list stays in the [master build prompt](master-build-prompt.md); open work is in
[remaining-work.md](remaining-work.md).

## Target

A MotionScript screenplay alone (plus project-local original artwork and the
approved offline speech model) must be enough to produce a finished short
dialogue animation in a flat cut-out style: a dark bedroom lit by a phone, a
doorway reaction shot, close-ups, speaker-coloured bold captions, comic timing,
room tone and phone sounds. The reference shot list is the four-panel sequence
below. The art is **original**: no third-party characters, backgrounds or logos
(see [AGENTS.md](../AGENTS.md); the layout is a style reference only).

1. Night, bed, close-up of a boy (pompom beanie, football pyjamas) lit by a phone;
   dark violet room. Worried face, eyes on the screen.
2. Doorway: a father opens the door, green hallway light behind him. Caption
   (white): "HEY, STAN. WHAT ARE YOU DOING? ARE YOU WATCHING PORN?"
3. Boy, phone visible with an app screen, warm lamp glow. Caption (yellow):
   "NO. I'M DOING A LOOKSMAXXING APP."
4. Father close-up. Caption (white): "OH OK. IS THAT PORN?"

(The reference line is the sample used for planning; the acceptance fixture in
`examples/` may use neutral dialogue of the same shape.)

## "All English words and sentences"

Enumerating every word is impossible and not the goal. The requirement is **open
vocabulary**: any English text produces speech, mouth shapes and captions, and
anything the system cannot pronounce confidently is reported before rendering.

1. Text normalisation (numbers, abbreviations, punctuation, casing) to words.
2. A pronunciation lexicon with a documented permissive licence plus
   rule-based grapheme-to-phoneme fallback for unknown words (slang, names).
3. Per-project pronunciation overrides in the screenplay.
4. A pre-render report listing unknown or low-confidence words.
5. Phoneme to viseme mapping with timing provenance: manual, estimated or
   forced-aligned (never presented as forced alignment unless it is). Caption and
   mouth timing use the same sample clock as dialogue.
6. Speech synthesis stays Kokoro (offline, user-consented assets); acoustic
   quality is a listening decision, not a test result.

## Workstreams, owners and acceptance

| # | Workstream | Main areas | Acceptance (code-checkable) |
| --- | --- | --- | --- |
| 1 | Captions and text rendering | motion-render, motion-export, motion-script, NOTICE | Text drawn on the CPU raster path with a bundled font whose licence is recorded; speaker-coloured caption style; timing from dialogue; deterministic SVG/PNG/preview agreement; missing glyphs reported |
| 2 | Language and lip-sync data | motion-lipsync, motion-script | Normaliser, lexicon + fallback G2P, overrides, unknown-word report, phoneme to viseme track with provenance; tests incl. out-of-vocabulary words |
| 3 | Acting layer | motion-rig, motion-scene, motion-render | Part variants (sprite swap) driven by viseme tracks, eye gaze, blink, brows on custom rigs; deterministic across seeks; legacy rigs unchanged |
| 4 | Lighting and props | motion-art, motion-render | Light-source model (colour, radius, flicker) using existing blend modes and masks; props with an animated image surface (phone screen); preflight budgets |
| 5 | Cinematography and reference scene | motion-script rules, examples/, tests | Shot library (establishing, medium, close-up, reaction), cut rules on dialogue/pause, an original-art reference screenplay that renders end to end, golden-frame tests |

Dependencies: 5 integrates 1-4 and starts on existing features; 3 consumes the
viseme data model that 2 produces (agree the track format first); 1 and 4 are
independent.

## What code cannot prove

Visual style quality, comic timing, voice quality, perceived lip-sync, font and
lexicon licensing sign-off, and any physical Mac behaviour need human review or
external evidence. Tests establish determinism, bounds and correct computation
only. Lip-sync quality must be measured against annotated audio before it is
described as validated.

## Decisions taken as defaults (change them by editing this file)

- Original art in a flat cut-out style; no third-party assets.
- Text is drawn only as captions (not actor labels), on the CPU path first.
- Font: a permissively licensed (for example SIL OFL) bold condensed face,
  bundled only after its licence text is recorded in NOTICE.
- Export through the existing offline FFmpeg path; no cloud service.

## Increments

1. This plan and the reference screenplay skeleton.
2. Workstreams 1-4 in parallel, each as its own reviewed PR with real checks.
3. Workstream 5 integration: full reference scene, golden frames, timing report.
4. Human review pass (style, timing, voice, licences); only then any claim of
   "professional-grade" for the sample.
