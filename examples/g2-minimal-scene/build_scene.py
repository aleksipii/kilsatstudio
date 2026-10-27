"""Build examples/g2-minimal-scene/scene.kilsat from stick-studio sources (read-only).

Usage (from the repository root): python3 -I examples/g2-minimal-scene/build_scene.py .
Optional arguments override the close-up camera and Veikko's raised-arm pose as JSON.
The defaults reproduce the committed scene.kilsat byte for byte.
"""
import json, sys, os

root = sys.argv[1]
close = json.loads(sys.argv[2]) if len(sys.argv) > 2 else {"x": -285, "y": -108, "zoom": 2.4}
studio = json.load(open(os.path.join(root, "examples/stick-studio/stick-studio.kilsat")))
# Standalone character sheets: same rigs as the studio project, without the
# baked "heldprop_*" parts (the phone waits for A05's grip binding, KS-003a).
chars = {}
for name, cid in [("veikko", "VEIKKO"), ("kipina", "KIPINA")]:
    sheet = json.load(open(os.path.join(root, "examples/stick-studio/characters/%s.kilsat" % name)))
    chars[cid] = json.loads(sheet["source"])["characters"][cid]
pose = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {"upper": -1.2, "fore": -2.2}
bg = json.load(open(os.path.join(root, "examples/stick-studio/backgrounds/pysakki.kilsat")))
bg_source = json.loads(bg["source"])

def character(cid):
    c = dict(chars[cid])
    c.pop("voice", None)  # no speech in this scene
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
