# Captions and text rendering

Workstream 1 of the [professional animation plan](professional-animation-plan.md).
Captions are derived from `speak` events and drawn on the CPU raster path through
the shared SVG renderer, so scene preview, sequence preview, SVG export, PNG
export and video export (which consumes the PNG frames) produce the same pixels.

## Screenplay

Captions are opt-in: a project without `captions` draws none (existing projects
are unchanged). `captions: {}` turns them on with the defaults below, and
`enabled: false` keeps the style but draws nothing.

```yaml
project:
  title: Doorway
  fps: 24
  width: 1920
  height: 1080
  captions:
    enabled: true          # false disables captions
    font_size: 64          # project pixels
    fill: "#ffffff"        # default fill
    speaker_fill:          # explicit per-speaker colours (character ids)
      BOY: "#ffd400"
    outline: "#000000"
    outline_width: 6       # visible outline thickness in pixels; 0 = none
    bottom_margin: 0.08    # fraction of height below the last line
    max_width: 0.8         # fraction of width; words wrap greedily
    line_height: 1.15      # multiple of font_size
    uppercase: true
```

Validation (`motion-script`): all numbers finite and in range, colours `#rrggbb`,
`speaker_fill` keys must be existing characters, unknown fields are rejected.
There is no per-event override yet; the speaker and text come from the event.

## Timing

A cue starts at the speak event's `at`. When the prepared scene manifest holds the
matching dialogue clip (same actor, same 24 kHz placement sample) the cue ends
with that clip's audio, on the same sample clock as dialogue and mouths. With no
prepared audio the end is *estimated* (0.4 s plus 0.06 s per character, at least
one second) and the cue is marked `estimated`; this is a display estimate and not
alignment. Intervals are half-open. Overlapping cues stack upward, latest lowest.

## Rendering

`motion-render::captions` lays text out with the metrics of the bundled font
(advance widths, no kerning, so wrapping is slightly conservative and identical
on every machine) and appends `<text>` elements to the scene SVG inside the same
letterboxed transform and opacity as the scene (so scene fades and crossfades
apply). Outlines use `paint-order="stroke"` so the fill stays on top.

`motion-export` rasterizes with resvg `0.45.1` built with its `text` feature and
an explicit `fontdb` containing only the bundled font; system fonts are never
scanned and every generic family maps to the bundled font.

Errors, never silent omission (reported before export/preview starts, and again at
render time): characters missing from the font (listed with code points), a word
wider than the line, over-long text, or a caption block taller than the frame.
Only the glyph coverage of the bundled font is supported (Latin as shipped in
Anton; not CJK, Arabic, emoji and so on).

## Font and licence

Anton Regular, SIL Open Font License 1.1, bundled unmodified at
`crates/motion-render/assets/fonts/Anton-Regular.ttf` (about 170 KB, SHA-256
`a4ba3a92...ab947cab`). Source: the `ofl/anton` directory of the google/fonts
repository (project: googlefonts/AntonFont). The licence text, copyright line and
source URL are recorded in the repository `NOTICE`, which the macOS packager
copies to `Contents/Resources/NOTICE` and the package audit verifies. This is a
record of the font only; it is not a full third-party licence audit (see
[final-license-audit](workstreams/final-license-audit.md)).

## Actor labels and the GPU path

Actor name labels are still not drawn: their `<text>` is now `display="none"`
(resvg would otherwise draw it now that text is enabled). Other SVG consumers
that honour `display` also hide them. The experimental GPU path has no caption
input and draws no text; use the CPU path for projects with captions.

## Desktop caption choice (user-level override)

The desktop inspector has a labelled **Captions** control: *Follow screenplay*
(default), *Show*, *Hide*. It is an override layered on top of
`project.captions`, never a rewrite of the screenplay.

- `motion_script::CaptionMode` (`Follow` / `Show` / `Hide`) is resolved in one place,
  `motion-export` `captions::style_for`, which builds the caption plan for every
  consumer: `ScenePreview::new_with_captions`, `SequencePreview::render_frame_captions`,
  `export_scene_with_art_captions` / `export_png_scene_with_art_captions` /
  `export_mp4_scene_with_art_captions`, and `export_{png,svg,mp4}_sequence_captions`.
  The older entry points delegate with `Follow`, so existing callers are unchanged.
- `Follow`: exactly the screenplay (no style or `enabled: false` draws nothing).
  `Hide`: nothing is drawn. `Show`: draws with the screenplay's style, even when it
  says `enabled: false`.
- `Show` needs a `project.captions` style; there is no hidden default. Without one,
  or when text cannot be laid out (missing glyph, word wider than the line), the
  preview or export fails with the real error (`captions: ... project.captions ...`
  or the glyph list). The scene preview mirrors caption errors to the status line,
  the sequence preview shows them in its status, and export reports them as the
  export error. Nothing is silently dropped. `Hide` never fails on undrawable text.
- The scene preview, sequence preview and the export workers all receive the same
  mode from the same app field; changing it starts a new preview snapshot.
- Not screenplay text and not in undo history: changing the choice adds no undo
  entry and leaves the source untouched.
- Persistence: `.kilsat` project documents carry an optional `caption_mode` field
  (`"show"` / `"hide"`), written only when it is not `Follow`, so existing projects
  keep their bytes and load unchanged. A document that stores the field is not
  readable by a build without this field (strict `deny_unknown_fields`). Plain
  `.yaml` screenplays have nowhere to store it, so the choice is session-only
  there. The saved value is read on open and written on save to `.kilsat`.
- Actor labels stay undrawn and the GPU path is untouched (it still draws no text).

Tests: `motion-script` (resolution table), `motion-project` (optional, byte-compatible
field, round-trip, bad value rejected), `motion-export` `caption_tests.rs` (Show,
Hide, Follow over absent/disabled/enabled styles change pixels as expected; scene
preview, sequence preview, scene PNG/SVG and sequence PNG/SVG exports agree per
mode; Show without a style and missing glyphs are errors), `studio-desktop`
(export worker and sequence worker tasks receive and apply the mode; headless egui
draw of the control; override leaves source and history alone). Not exercised: the
control by hand in a running window, and save/reopen of the field through the
desktop's file dialogs (the field is tested at the `motion-project` layer).

## What the tests establish and what they do not

Headless tests (`crates/motion-export/src/caption_tests.rs`, `motion-render`
caption unit tests, `motion-script` style tests) show: text changes pixels,
per-speaker and default fill colours, outline colour, enable/disable, opt-in,
deterministic wrapping and ink within the line width and safe margins, estimated
versus audio-derived timing, identical bytes across SVG-to-PNG, scene preview and
sequence preview, a font database holding one font, and missing-glyph errors.
They do not establish typographic quality, readability, kerning-accurate widths,
or behaviour on a physical Mac; no desktop UI was exercised.

### Optional caption backing

`project.captions.background: '#15121f'` adds an opaque full-width strip behind
active captions on the shared SVG path. It is fixed in screen space, so camera
moves cannot put the text back on a blanket or prop. The strip includes padding
for glyph outlines and extends to the bottom edge. It is absent during silence;
omitting `background` preserves transparent captions. Colours must be `#rrggbb`.
