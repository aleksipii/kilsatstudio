# Screen slots: your own image or video on a phone

A phone (or tablet, laptop, TV, billboard) is not only a drawing. Its rig can
carry a named **screen slot**, and a scene can show any image source in that
slot over time.

## Structure

| Layer | Where | What it holds |
| --- | --- | --- |
| Slot geometry | `motion_rig::ScreenSlotDefinition` (`RigDefinition.screens`) | `id`, the glass `part`, a parallelogram `origin` + `x_axis` + `y_axis` in the glass part's joint space, and `overlays` (drawn UI hidden while content shows) |
| Catalog slot | `HandPropItem.screen` in `examples/stick-studio/hand-props.json` | `puhelin` → slot `phone` (28×51 item units), `tabletti` → slot `tablet`. Attaching the item copies the slot onto the hand; detaching removes it. A second phone in the other hand gets a hand-tagged slot name (`phone_l` / `phone_r`) |
| Content over time | `motion_script::ScreenCue` (`composition.screens`) | `actor`, `slot`, `at`, `source`, `fit`, `trim_start`, `trim_end`, `speed`, `loop`, `taps` |
| Evaluation | `motion_scene` → `PreparedRig::apply_screen` | Sets the glass part's texture from the scene time, on the same joint transforms as every other part |
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

## Timing

The shown source time is a pure function of scene time:
`trim_start + (t - at) * speed`, wrapped over `[trim_start, trim_end)` when
looping, otherwise clamped to `trim_end`. The frame is
`floor(source_time * fps)`. Preview evaluates on the 24 kHz sample clock and
export on frame seconds; both call the same evaluator, and a test checks every
frame of a 5 s scene is identical both ways. Decoded video frames are keyed
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
  slot travels with the prop; a second phone gets a hand-tagged slot name.
- `motion-script`: YAML parse/round-trip, trimmed double-speed hold, sequence
  loop, actionable validation errors, character removal drops its cues.
- `motion-export`: SVG pixels for drawn/contain/cover/stretch/off, export
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

- Desktop: no screen-cue editor or video import UI yet, and the preview does
  not yet schedule `decode_screen_video` or load frames per time; video cues
  render only where a caller has put decoded frames into the image map.
- Automatic direction does not yet read `tap_times` to place finger motion.
- Screen brightness and glow in dark scenes are not implemented (only the
  drawn reflection over the glass).
- P3 conversion is verified only by the CI test on synthetic clips, not on a
  real iPhone recording; HDR tone mapping is out of scope.
- Slots on laptops, TVs and billboards need those items in a catalog or rig;
  the structure supports them.
