# Multiplane parallax, aerial perspective and value structure (P05)

Status: implemented in `motion-render` (`src/parallax.rs`) and
`motion-library` (manifest/assembly). Opt-in: nothing changes unless a caller
passes a `Multiplane`. The desktop app and the export pipeline do not pass one
yet, so parallax is not visible in the UI.

## Data

- `motion-library` manifest, all optional and backward compatible:
  - `parallax` on an art layer and on a set/location part (the part wins);
  - `atmosphere: { color, strength }` on a location or set (the set wins).
- Defaults by depth: `far` 0.3, `mid` 0.6, `near` 1.0; the character plane is
  1.0. Factors are validated as finite and 0..=4, strength as 0..=1.
- `Assembled::parallax` (layer id → factor) and `Assembled::atmosphere` feed
  `motion_render::parallax::Multiplane::new`, which validates again at the
  render boundary (`ParallaxError`).

## Rules

- **Per-layer camera.** `layer_camera(camera, p)`: pan `x·p, y·p`, zoom
  `1 + (zoom − 1)·p` clamped to 0.05..=20. Factor 1.0 returns the camera
  unchanged bit for bit. Layers attached to an actor stay on the character plane.
- **One function, both renderers.** The SVG painter (`render_svg_with_multiplane`)
  wraps each non-unit layer in a compensating transform derived from
  `Multiplane::camera_for`; the GPU builder (`scene_commands::build_with_multiplane`)
  uses the same `camera_for` for its per-layer origin and scale.
- **Aerial perspective.** Haze amount `strength · (1 − min(p, 1))`. Vector
  colours are desaturated toward luma and blended toward the atmosphere colour on
  the CPU before either renderer sees the document. Image layers cannot be
  recoloured; they are listed in `Hazed::skipped` rather than silently treated
  as hazed.
- **Value structure.** `value_structure` compares the WCAG luminance-contrast
  ratio of background flat colours with that of the characters and reports
  `background_overpowers()`. It only warns; it is a staging heuristic, not a
  perceptual measurement, and does not measure image layers (`unmeasured`).
- **Determinism.** Nothing reads a clock; camera and time come from the
  evaluated scene, so a seek equals frame-by-frame playback.

## Tests

- `motion-render` `parallax::tests`: validation, identity camera, stationary
  camera / empty multiplane / zero atmosphere give byte-identical SVG, pan moves
  Far 0.3× and Mid 0.6× of Near, seek equals frame-by-frame (49 frames), haze and
  skipped images, value-structure warning.
- `motion-render` `scene_commands::tests::multiplane_uses_the_shared_layer_camera_and_is_identity_when_empty`
  (feature `gpu`): empty or unit multiplane gives identical commands; a 0.3 layer
  gives exactly the commands of a scene filmed by `layer_camera`; atmosphere uses
  the same hazed document.
- `motion-export` `gpu_parity_tests` (feature `gpu`): a default-run CPU check and
  the hardware test `gpu_parity_multiplane_within_shapes_tolerance`; numbers in
  [gpu-parity.md](gpu-parity.md#multiplane-parallax-p05).

## Limitations

- Not wired into `studio-desktop` or the export job; no MotionScript key yet.
- No per-layer blur (depth of field); haze is colour only.
- Image layers are moved by parallax but not hazed.
