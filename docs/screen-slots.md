# Screen slots: your own image or video on a phone

A phone (or tablet, laptop, TV, billboard) is not only a drawing. Its rig can
carry a named **screen slot**, and a scene can show any image source in that
slot over time.

## Structure

| Layer | Where | What it holds |
| --- | --- | --- |
| Slot geometry | `motion_rig::ScreenSlotDefinition` (`RigDefinition.screens`) | `id`, the glass `part`, a parallelogram `origin` + `x_axis` + `y_axis` in the glass part's joint space, and `overlays` (drawn UI hidden while content shows) |
| Catalog slot | `HandPropItem.screen` in `examples/stick-studio/hand-props.json` | `puhelin` → slot `phone` (28×51 item units), `tabletti` → slot `tablet`. Attaching the item copies the slot onto the hand; detaching removes it. A second phone in the other hand gets a hand-tagged slot name (`phone_l` / `phone_r`) |
| Content over time | `motion_script::ScreenCue` (`composition.screens`) | `actor`, `slot`, `at`, `source`, `fit`, `trim_start`, `trim_end`, `speed`, `loop`, `taps`, `brightness`, `glow`, `glow_radius` |
| Evaluation | `motion_scene` → `PreparedRig::apply_screen` | Sets the glass part's texture from the scene time, on the same joint transforms as every other part; `ScreenLight` becomes `EvaluatedTexture::brightness` and `::glow` |
| Fitting | `EvaluatedTexture::placement` | `cover` (default for screens), `contain`, `stretch`; one function used by both the SVG and the GPU renderer |
| Video frames | `motion_export::screen_video` | FFmpeg decode into a content-addressed PNG frame cache, proxy or full size, Display P3 → BT.709 |

The slot's aspect ratio is `|x_axis| / |y_axis|`. Rounded corners come from
the glass part's polygon (or its `smooth`): content is clipped to it.
Overlays hide the device's drawn UI while content or `off` is shown; the
phone's `reflection` part is deliberately not an overlay, so a faint glass
reflection stays over the picture.

## MotionScript

```yaml
scenes:
  - id: demo
    duration: 10
    events: []
    composition:
      screens:
        - actor: liisa
          slot: phone            # from the catalog phone in liisa's hand
          at: 2.0                # scene seconds
          source: {type: video, asset: assets/screens/oma-sovellus-demo.mov, fps: 30, duration: 12.5}
          fit: cover             # cover | contain | stretch
          trim_start: 0.5        # source seconds
          loop: true
          taps: [3.2]            # "tap at 3.2 s" in the recording
          brightness: 1.2        # picture multiplier, 0..4 (default 1)
          glow: 0.6              # light around the glass, 0..1 (default 0)
          glow_radius: 0.75      # reach in slot lengths, (0, 4] (default 0.75)
        - {actor: liisa, slot: phone, at: 8.0, source: {type: "off"}}
```

Sources: `{type: off}`, `{type: image, asset: <image asset id>}`,
`{type: sequence, frames: [<image asset ids>], fps}` and
`{type: video, asset: <project-relative .mp4/.mov/.m4v>, fps, duration}`.
A cue lasts until the next cue for the same actor and slot; before the first
cue the rig's own drawing shows. A finished non-looping source holds its last
frame. Trim, speed, loop and taps are rejected on stills and `off`. Unknown
slots, out-of-scene times, bad trims, unsorted taps and `..` paths are
validation errors; other video containers are `Unsupported`.

`holds: phone` / `phone.screen:` shorthand from the original idea is not part
of the schema: props are attached through the rig (desktop prop panel or
`attach_hand_prop`), and screen cues name the slot explicitly.

## Brightness and glow

`brightness` multiplies the picture's sRGB channels and clamps at white
(0 is a black picture, 2 doubles a mid grey). `glow` adds a soft light around
the glass: an ellipse centred on the slot that reaches `glow_radius` × the
slot's longer side beyond it on every side, screen-blended, with a smoothstep
falloff from the centre. Its colour is the picture's mean colour (a 16×16
sample grid of the whole decoded image, `ImageResource::average_rgb`)
normalised to full brightness; its opacity is `glow × brightness × mean luma`,
clamped to 1, so dark pictures glow little and a black one not at all. Each
video frame has its own mean, so the light follows the recording frame by
frame. `off` never glows, and brightness/glow on an `off` cue are validation
errors, as are non-finite or out-of-range values and `glow_radius` without
`glow`.

The glow is drawn behind the actor holding the device, after the background
and earlier actors, so in a dark scene it lights the surroundings; the device
itself, the hand and the face are drawn over it. Both renderers use the same
numbers: `TextureGlow::ellipse` and `TextureGlow::paint` in `motion-rig`. The
SVG path writes a radial gradient with seven smoothstep stops and an
`feComponentTransfer` filter for brightness (only when brightness ≠ 1, so older
scenes keep their bytes); the GPU command list uses the radial light command
(kind 24, screen blend, softness 1) mapped into the slot's rest space and
carries brightness on the textured command.

## Timing

The shown source time is a pure function of scene time:
`trim_start + (t - at) * speed`, wrapped over `[trim_start, trim_end)` when
looping, otherwise clamped to `trim_end`. The frame is
`floor(source_time * fps)`. Preview evaluates on the 24 kHz sample clock and
export on frame seconds; both call the same evaluator, and a test checks every
frame of a 5 s scene is identical both ways. `crates/motion-render/tests/g2_screen_seek.rs`
(KS-003d) puts the catalog phone with off → still → trimmed looping lit video →
held video cues into the G2 scene and checks that a scattered, backwards seek
on a fresh evaluator gives exactly the frame-by-frame playback's evaluated
scene and SVG for all 96 frames, on both clocks. Decoded video frames are keyed
`<asset>#frame=<n>` (`motion_script::video_frame_asset`);
`Composition::video_frames_at(t)` lists exactly the frames a scene time needs.

`ScreenCue::tap_times(until)` maps tap markers to scene times, repeating on
loops, for automatic direction to time a finger press to.

## Video decoding

`motion_export::screen_video::decode_screen_video` runs a caller-selected
absolute FFmpeg path (never a PATH search), with a timeout and cancellation,
writing `<cache>/<key>/000000.png…` and a manifest, renamed into place only
when complete. The key hashes the recipe version, source bytes, fps,
duration and proxy width, so preview proxies (`SCREEN_PROXY_WIDTH` = 540 px)
and full-size export frames are separate entries with identical frame
numbers. Stale (recipe or size changed) or incomplete entries are ignored.
It is a blocking call for a background job, not the UI thread. Decoded
frames are always bounded to 4096×4096 (aspect kept), and decoding is
stopped if an entry would exceed 4 GiB on disk.

`probe_color` reads the recording's tags with ffprobe: Display P3
(`smpte432`) is converted to BT.709 primaries with FFmpeg's `colorspace`
filter. DCI-P3 (`smpte431`, another white point) and HDR (BT.2020, HLG, PQ)
are rejected with an explanation instead of shown with wrong colours.
Untagged recordings are treated as BT.709.

Clean recordings of your own app come straight from the iOS simulator:
`xcrun simctl io booted recordVideo demo.mov` (no notifications or status
clutter).

## Validation

- `motion-rig`: slot validation, joint following, overlays, fit maths, catalog
  slot travels with the prop; a second phone gets a hand-tagged slot name;
  brightness/glow reach the texture (never on `off`), range checks, glow paint.
- `motion-script`: YAML parse/round-trip, trimmed double-speed hold, sequence
  loop, actionable validation errors, character removal drops its cues;
  brightness/glow defaults are omitted on save, round-trip and validate.
- `motion-render`: GPU glow and brightness commands match the CPU placement
  and paint (`screen_brightness_and_glow_match_the_cpu_placement`, `gpu`
  feature); G2 seek ≡ frame-by-frame with screen cues.
- `motion-export`: SVG pixels for brightness 2 and 0.5, glow outside the glass
  in a black scene that fades and ends at the ellipse, no glow for a black
  picture or `off`; the `rig-screen-brightness-glow` GPU coverage fixture
  (measured on Apple M1 / Metal, macOS 27.0.1, twice with identical numbers:
  max 58,57,37, RMS 1.32,1.29,1.20, 14.1% changed, interior RMS 0.20 — edge
  errors like the other textured rig fixtures; recorded, not an acceptance
  threshold). SVG pixels for drawn/contain/cover/stretch/off, export
  preflight for missing stills and undecoded video, Veikko's catalog phone
  playing a looping trimmed recording on the scene clock (preview ≡ export
  for every frame), tap times, decode cache keys, colour tags and FFmpeg
  arguments.
- `export_streams_decoded_video_frames_from_the_cache`: preflight passes with
  no frame in memory, exactly one frame loads per instant, and a deleted cache
  file fails preflight.
- `real_ffmpeg_screen_video_decodes_proxy_and_converts_p3` (ignored locally,
  run by `.github/workflows/screen-video.yml` on macOS with Homebrew FFmpeg)
  decodes synthetic BT.709 and P3-tagged clips and checks the P3 red rises in
  sRGB.

## Export: streamed video frames

`ArtContext.screen_videos` maps a cue's video asset to its decoded frame cache
(`DecodedScreenVideo`). Export no longer needs every frame in memory:
`ArtContext::images_at(scene, time)` reads only the frames that one exported
frame shows from disk and drops them afterwards. The working-set budget counts
one decoded frame per video. Export preflight checks that every frame an
exported frame will show exists in the cache (or in the image map), so a
missing frame fails before rendering starts.

`production_export` decodes each video cue once (ffprobe colour probe, then
`decode_screen_video` into `screen-video-cache/` beside the output). Review
renders (`--review-size`) use the 540 px proxy; final export decodes full size.

## Example episode

`examples/screen-demo/live-demo.kilsat` ("The Live Demo", English, Kokoro
voices): Aino holds a phone with a `phone` slot on her raised arm. A close-up
plays `assets/kilsat-app-demo.mp4`, an original illustrative mileage-app
recording marked EXAMPLE, and the later shots hold its last frame. Regenerate
with `python3 scripts/generate_screen_demo.py --ffmpeg /opt/homebrew/bin/ffmpeg`
and render with `production_export --retime-dialogue` (see the example README).

## Open work

- Desktop (done in `apps/studio-desktop`, `screen_cues.rs` and
  `screen_video_worker.rs`): screen-cue editor (add/edit/remove, inline
  validation, undo), video import into `assets/screens/` with ffprobe, and a
  bounded background proxy decode feeding the preview's
  `ArtContext.screen_videos`. Not exercised by hand in the GUI yet. Still open:
  desktop **Export video** and sequence export do not decode full-size video
  (preflight rejects it; use `production_export`); the preview shows a
  missing-frame error until the proxy decode finishes; no native file picker.
- Automatic direction does not yet read `tap_times` to place finger motion.
- Glow lights only what is behind the actor: it does not brighten the
  holder's own face or hand, and it is not cast by art-layer lights onto other
  layers. Its colour is the whole image's mean, not only the cropped (`cover`)
  region. Brightness and glow are per cue, not animated within a cue.
- P3 conversion is verified only by the CI test on synthetic clips, not on a
  real iPhone recording; HDR tone mapping is out of scope.
- Slots on laptops, TVs and billboards need those items in a catalog or rig;
  the structure supports them.
