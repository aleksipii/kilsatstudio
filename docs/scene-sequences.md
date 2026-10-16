# Scene sequence cuts and crossfades

Scenes remain ordered in MotionScript. The outgoing transition is stored in
`composition.transition`; omitted transitions are cuts.

```yaml
composition:
  transition: {type: crossfade, duration: 0.5}
```

Use `{type: cut}` for an explicit cut. The final scene must cut. Crossfade duration
must be finite, positive and round to at least one 24 kHz sample, strictly shorter
than both neighboring scenes. Incoming plus outgoing overlap cannot exceed the
scene's rounded length. Whole-document validation and settings Undo are retained.
The native Inspector authors these settings, but its preview remains scene-local.

## Timing and composition

`SequenceTimeline::compile` rounds scene and overlap durations to the nearest
sample at 24000 Hz. Scene B starts at A's end minus their overlap. Compilation
bounds scenes to 1024 and rejects zero rounded lengths and arithmetic overflow.
Sampling is stateless on `[0, total_samples)`. At a cut only the new scene
contributes. During an overlap of N samples starting at S, incoming weight is
`(sample-S)/N`, outgoing weight its complement. At S+N outgoing is absent.
Triple overlap is rejected.

Global frames use `floor(frame * 24000 / fps)` and exclude EOF. Each contribution
uses its own scene-local camera, artwork, fade and viseme clock, including current
rule-based speech motion. Whole opaque images are mixed by weighted 8-bit RGBA
channel sums, rounded once. This is encoded-color crossfading, not linear-light
color grading. Fading two stacked images separately is not equivalent.

`motion_voice::sequence::mix_sequence` uses the same weights, validates ordered
scene manifests and finite mono 24 kHz PCM, and preserves finite headroom like the
existing mixer. WAV output clips to its PCM range. Audio is not stretched or
regenerated. Output is bounded to ten minutes, source audio to twenty minutes,
with cancellation checks at stage and chunk boundaries.

## Headless PNG/WAV export

```sh
cargo run -p motion-export --example render_sequence -- examples/scene-sequence.yaml inputs.json /absolute/new-output
```

Supply existing local audio and scene manifests in scene order:

```json
[
  {"wav":"warm.wav","manifest":"warm.json"},
  {"wav":"cool.wav","manifest":"cool.json"}
]
```

Input paths resolve relative to `inputs.json`. Manifests use the existing
single-scene audio format, with exact scene IDs and rounded sample lengths.
The CLI bounds aggregate manifest bytes to 8 MiB and WAV bytes to 128 MiB before
retaining inputs. It performs no synthesis or download. The library
`export_png_sequence` also accepts an optional `ArtContext`; this CLI phase does
not resolve `.kilsat` image resources.

Output contains numbered `frame-000000.png` files, mixed `audio.wav`, and
`sequence.json` with dimensions, ordered scene spans, local/global sample indices
and contribution weights. Destination must not exist. Cancellation/errors clean
only the newly created output directory.

Export is sequential with at most two opaque raster images, at most 10000 frames,
16 million pixels per frame, one billion estimated pixel visits, 256 MiB output
and a conservative 256 MiB working-set estimate including retained source/mixed
PCM and artwork. Existing scene/art/keyframe limits also apply. These estimates
are not a process allocator cap; long/high-resolution work can require splitting.

## Still open

Full native sequence preview, global playback/seek, desktop sequence-export
controls, MP4/SVG sequence export, CLI `.kilsat` resource loading and shared
parallel sequence jobs remain open. Automated synthetic fixtures do not establish
physical Mac synchronization or real speech transition acceptance.
