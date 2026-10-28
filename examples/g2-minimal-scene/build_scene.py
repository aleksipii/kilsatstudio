"""Build examples/g2-minimal-scene/scene.kilsat from stick-studio sources (read-only).

Usage (from the repository root): python3 -I examples/g2-minimal-scene/build_scene.py .
Optional arguments override the close-up camera, Veikko's raised-arm pose and the
phone's grip placement as JSON.
The defaults reproduce the committed scene.kilsat byte for byte.
"""
import json, math, sys, os

root = sys.argv[1]
close = json.loads(sys.argv[2]) if len(sys.argv) > 2 else {"x": -285, "y": -108, "zoom": 2.4}
studio = json.load(open(os.path.join(root, "examples/stick-studio/stick-studio.kilsat")))
# Standalone character sheets: same rigs as the studio project, without its
# baked "heldprop_*" parts. The phone is attached below from the catalog.
chars = {}
for name, cid in [("veikko", "VEIKKO"), ("kipina", "KIPINA")]:
    sheet = json.load(open(os.path.join(root, "examples/stick-studio/characters/%s.kilsat" % name)))
    chars[cid] = json.loads(sheet["source"])["characters"][cid]
pose = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {"upper": -1.2, "fore": -2.2}
bg = json.load(open(os.path.join(root, "examples/stick-studio/backgrounds/pysakki.kilsat")))
bg_source = json.loads(bg["source"])

# Catalog phone in Veikko's right hand. PHONE_PLACEMENT is the hand-local grip
# placement; the parts are the catalog item's parts and grip_parts moved onto
# hand_r and appended, as motion_rig::props::attach_hand_prop does (same formula
# as GripPlacement::to_hand and grip_parts; checked against the Rust function by
# crates/motion-render/tests/g2_minimal_scene.rs). The raised forearm points up
# and back, so a half turn stands the phone upright in the hand.
PHONE_PLACEMENT = json.loads(sys.argv[4]) if len(sys.argv) > 4 else {
    "x": 0, "y": 0, "rotation_degrees": 180, "scale": 1}
ELLIPSE_SEGMENTS = 48  # motion_rig::ELLIPSE_SEGMENTS
catalog = json.load(open(os.path.join(root, "examples/stick-studio/hand-props.json")))
phone = next(i for i in catalog["items"] if i["id"] == "puhelin")

def held_phone(pl=PHONE_PLACEMENT):
    angle = pl["rotation_degrees"] * (math.pi / 180.0)
    sin, cos = math.sin(angle), math.cos(angle)
    gx, gy = phone["grip"]
    def to_hand(p):
        x = (p[0] - gx) * pl["scale"]
        y = (p[1] - gy) * pl["scale"]
        return [pl["x"] + cos * x - sin * y, pl["y"] + sin * x + cos * y]
    out = []
    for part in phone["parts"] + phone["grip_parts"]:
        part = dict(part)
        part["id"] = "heldprop_r_%s_%s" % (phone["id"], part["id"])
        part["joint"] = "hand_r"
        e = part.get("ellipse")
        if e is not None:
            if pl["rotation_degrees"] == 0 or e["rx"] == e["ry"]:
                cx, cy = to_hand([e["cx"], e["cy"]])
                part["ellipse"] = {"cx": cx, "cy": cy, "rx": e["rx"] * pl["scale"], "ry": e["ry"] * pl["scale"]}
            else:
                part["points"] = [to_hand([e["cx"] + e["rx"] * math.cos(math.tau * i / ELLIPSE_SEGMENTS),
                                           e["cy"] + e["ry"] * math.sin(math.tau * i / ELLIPSE_SEGMENTS)])
                                  for i in range(ELLIPSE_SEGMENTS)]
                del part["ellipse"]
        else:
            part["points"] = [to_hand(p) for p in part["points"]]
        if part.get("outline"):
            part["outline"] = dict(part["outline"], width=part["outline"]["width"] * pl["scale"])
        out.append(part)
    return out

def character(cid):
    c = dict(chars[cid])
    c.pop("voice", None)  # no speech in this scene
    if cid == "VEIKKO":
        rig = dict(c["rig"])
        rig["parts"] = rig["parts"] + held_phone()
        c["rig"] = rig
    return c

SCENE = "g2_min"
smooth = "smoothstep"
source = {
    "project": {"title": "G2 pienin kohtaus", "width": 1920, "height": 1080, "fps": 24},
    "characters": {"VEIKKO": character("VEIKKO"), "KIPINA": character("KIPINA")},
    "scenes": [{
        "id": SCENE,
        "duration": 4,
        "composition": {
            "background_color": bg_source["scenes"][0]["composition"]["background_color"],
            "camera": [
                {"at": 0, "x": 0, "y": 0, "zoom": 1},
                {"at": 1.99, "x": 0, "y": 0, "zoom": 1},
                {"at": 2, **close},
                {"at": 4, **close},
            ],
            "layers": {"VEIKKO": {"z": 0, "visible": True}, "KIPINA": {"z": 0, "visible": True}},
            "actor_tracks": {
                "VEIKKO": [{"at": 0, "easing": smooth, "x": -300.0, "y": -12, "scale_x": 1.6, "scale_y": 1.6}],
                "KIPINA": [{"at": 0, "easing": smooth, "x": 300.0, "y": -12, "scale_x": 1.6, "scale_y": 1.6}],
            },
            "joint_tracks": {
                "VEIKKO": {
                    "upper_arm_r": [{"at": 0, "easing": smooth, "rotation": pose["upper"]}],
                    "forearm_r": [{"at": 0, "easing": smooth, "rotation": pose["fore"]}],
                },
            },
            "gaze_tracks": {"VEIKKO": [{"at": 0, "target": "KIPINA"}], "KIPINA": [{"at": 0, "target": "VEIKKO"}]},
        },
        "events": [],
    }],
}

layers = []
for layer in bg["art"]["layers"]:
    layer = dict(layer)
    layer["scene_id"] = SCENE
    layer["id"] = layer["id"].replace("tausta_tausta_", "g2_bg_")
    layers.append(layer)
# One foreground element: a bus-stop pole in front of the actors, at the left edge.
layers.append({
    "id": "g2_fg_pole", "name": "foreground pole", "scene_id": SCENE, "visible": True,
    "plane": "foreground", "opacity": 1.0, "attachment": None, "clip": None,
    "content": {"type": "rectangle", "rect": {"x": 120, "y": 0, "width": 26, "height": 1080},
                "color": {"r": 60, "g": 72, "b": 78, "a": 255}},
})

project = {"format_version": studio["format_version"], "source": json.dumps(source, ensure_ascii=False, separators=(",", ":")),
           "assets": [], "art": {"layers": layers}}
out = os.path.join(root, "examples/g2-minimal-scene/scene.kilsat")
os.makedirs(os.path.dirname(out), exist_ok=True)
with open(out, "w") as f:
    json.dump(project, f, ensure_ascii=False, indent=1)
    f.write("\n")
print(out, os.path.getsize(out))
