# G2 reference images (A08, KS-003c)

Scene: [`examples/g2-minimal-scene/scene.kilsat`](../../../examples/g2-minimal-scene/README.md)
(SHA-256 `8ba99dc39ed851d076f69370c83a35237b738aaa6a062ae1bba75366dcbd8f03`).
Base commit `9342979` plus branch `feat/a08-g2-phone`. VEIKKO holds the catalog phone
(`puhelin`, `examples/stick-studio/hand-props.json`) in the right hand, attached with
A05's `motion_rig::props` grip contract at placement x 0, y 0, rotation 180°,
scale 1.

| File | Frame | Shot |
| --- | --- | --- |
| `g2-wide-f024.png` | 24 (1.0 s) | wide, zoom 1 |
| `g2-close-f072.png` | 72 (3.0 s) | close-up, zoom 2.4 |

## Hashes

SVG, asserted by `reference_svgs_are_deterministic_and_match_recorded_hashes`
(`render_svg_with_art` at 960x540, no mouths, no images):

| Shot | SHA-256 |
| --- | --- |
| wide (1.0 s) | `6f6b2cd6ca918ac5c8969a25ba19f7de76d58952876bea58c154b561a7e4dde7` |
| close (3.0 s) | `5de41ad65eee7b6c9c5a690173b0984edc696605526097bd640c9bfac742c0f6` |

PNG (export pipeline). The two export frames are asserted on Apple Silicon macOS by
`crates/motion-export/tests/g2_export.rs` (values in `tests/support/g2.rs`); the
downscaled committed files are informational:

| File | Size | SHA-256 |
| --- | --- | --- |
| export `frame_000024.png` | 1920x1080 | `8bee3aa180d1041408a9e04e4d5fe2ef31215f2f47df040db8b7389a52edc658` |
| export `frame_000072.png` | 1920x1080 | `85c0e33ec5cf848f0c21bcf17c245eb929da4623d776b7868eb575a6bece7f23` |
| `g2-wide-f024.png` (committed) | 960x540 | `847911ed619328a88566a212308d2632d2bd5a6a415ef2d82e110df5e191d1c6` |
| `g2-close-f072.png` (committed) | 960x540 | `85cd6082a11bbe31036756871773f7f48dbf2ae0f8d8facfbcfad150415e7ec2` |

## Renderer and platform

- Renderer: CPU only. `motion-export` example `g2_export` (PNG mode, strict
  model-free audio): shared evaluator → `render_svg_with_art` → resvg 0.45.1 /
  tiny-skia 0.11.4. The earlier `render_scene` command produced the same frame bytes
  for the scene without the phone.
  The committed PNGs are those frames downscaled to 960x540 with macOS `sips -Z 960`.
- No GPU render of this scene was made; no CPU/GPU parity is claimed
  (see [render-contract.md § CPU and GPU](../../render-contract.md#cpu-and-gpu)).
- Machine: Apple Silicon (arm64), macOS 27.0.1 (26A434), rustc 1.99.1.
- Input audio: 4 s of silence on the 24 kHz scene clock from
  `motion_voice::model_free::prepare_scene_model_free` (the scene has no speech).

Reproduce:

```sh
python3 -I examples/g2-minimal-scene/build_scene.py .   # byte-identical scene.kilsat
cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR png
sips -Z 960 OUT_DIR/frame_000024.png --out g2-wide-f024.png
sips -Z 960 OUT_DIR/frame_000072.png --out g2-close-f072.png
```

What the images show (by eye, not a perceptual review): both figures stand on the
pavement with feet visible in the wide shot; the foreground pole draws over the
background shelter; VEIKKO holds the phone upright at the side of the head in both
shots, the forearm runs into the hand and the fingers wrap the lower part of the
phone; the close-up keeps the face, the hand and the whole phone inside the frame.
The phone overlaps the right edge of the cap brim. The framing and depth claims rest on the tests, not on these images.
