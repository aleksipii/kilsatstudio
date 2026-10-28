# G2 minimal scene (KS-003c)

`scene.kilsat` is the A08 test scene for gate G2: two stick figures (VEIKKO and
KIPINA) on the night bus-stop background, a wide shot and a hard cut to a
close-up of VEIKKO's face and raised right hand. 1920x1080, 24 fps, 4 s, silent.

| Time | Camera | Shot |
| --- | --- | --- |
| 0–1.99 s | x 0, y 0, zoom 1 | wide: both figures, feet on the pavement |
| 2.0–4 s | x −285, y −108, zoom 2.4 | close-up: face + right hand + phone |

The cut keys are 10 ms apart (1.99 s → 2.0 s), so no 24 fps frame samples the
transition: frames 0–47 are wide, 48–95 close (tested).

**Phone.** VEIKKO holds the catalog phone (`puhelin` in
`examples/stick-studio/hand-props.json`) in the right hand. The generator attaches it
the way A05's `motion_rig::props::attach_hand_prop` does: the item's parts and
finger parts are moved onto `hand_r` with grip placement x 0, y 0, rotation 180°,
scale 1 and appended to the rig, so they paint over the arm, hand and face. The
raised forearm points up and back, so the half turn stands the phone upright. The
rigs come from `examples/stick-studio/characters/*.kilsat`, which do not contain the
older `heldprop_*` parts baked into `stick-studio.kilsat`. The test
`phone_is_the_catalog_item_attached_to_veikkos_right_hand` rebuilds the parts with
`attach_hand_prop` and compares them with the scene file.

Sources (read-only, unchanged geometry): character rigs from
`examples/stick-studio/characters/veikko.kilsat` and `kipina.kilsat` (voice
metadata removed), background layers from
`examples/stick-studio/backgrounds/pysakki.kilsat` (scene ID and layer ID prefix
renamed), plus one foreground pole (`g2_fg_pole`) in front of the figures.
Regenerate byte for byte from the repository root:

```sh
python3 -I examples/g2-minimal-scene/build_scene.py .
```

Tests: `crates/motion-render/tests/g2_minimal_scene.rs`. Reference images and
hashes: [`docs/evidence/g2-render/`](../../docs/evidence/g2-render/README.md).
Contract: [`docs/render-contract.md`](../../docs/render-contract.md).
