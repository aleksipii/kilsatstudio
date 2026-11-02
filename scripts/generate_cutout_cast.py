#!/usr/bin/env python3
"""Generate the original paper cut-out cast (examples/cutout-cast).

Three project-owned characters in a construction-paper style: a big round
head sitting straight on a small body, stubby mitten arms and legs, flat
colours and a dark outline round every rigid piece. Nothing bends: each piece
is a separate rigid part, mouths and eyes are swapped drawings, and head turns
swap per-view drawings (`views`).

The rigs use the same 15 joint ids as the stick-studio cast, so gestures,
gaits and joint clips that name those joints also drive these figures.
Coordinates are rig units with the hips at (0, 0) and y pointing down.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "examples/cutout-cast"

INK = [22, 20, 24, 255]
WHITE = [252, 250, 244, 255]
HIDDEN = {"color": [0, 0, 0, 0]}
JOINT_IDS = (
    "root", "torso", "head",
    "upper_arm_l", "forearm_l", "hand_l",
    "upper_arm_r", "forearm_r", "hand_r",
    "thigh_l", "shin_l", "foot_l",
    "thigh_r", "shin_r", "foot_r",
)
VISEME_NAMES = (
    "rest", "closed", "teeth_lip", "tongue_teeth", "tongue_up",
    "sibilant", "sh", "r", "a", "e", "i", "o", "u",
)
# Squash of the facial features per view (motion_rig::FaceView::feature_squash).
TURNS = {"three_quarter": 0.8, "side": 0.55}

CAST = [
    {"id": "ILONA", "name": "Ilona", "role": "Sanavalmis koululainen, joka keksii aina suunnitelman",
     "accent": "#2AA096"},
    {"id": "TOPI", "name": "Topi", "role": "Innokas luokkakaveri, joka sanoo ensin ja ajattelee sitten",
     "accent": "#ECB034"},
    {"id": "RAUNO", "name": "Rauno", "role": "Koulun talonmies, joka on nähnyt kaiken kahdesti",
     "accent": "#46648C"},
]

# Proportions per character: body height, half widths at the shoulder and hem,
# head radii, leg drop and the mouth's height on the face.
BUILD = {
    "ILONA": dict(body=62, top=25, hem=32, head=(56, 54), leg=16, mouth=27, skin=[255, 214, 172, 255]),
    "TOPI": dict(body=62, top=30, hem=38, head=(62, 54), leg=14, mouth=27, skin=[246, 198, 158, 255]),
    "RAUNO": dict(body=86, top=30, hem=34, head=(56, 58), leg=22, mouth=34, skin=[238, 192, 156, 255]),
}


def _outline(width=2.4):
    return {"color": list(INK), "width": width, "mode": "behind"}


def _polygon(name, joint, points, color, width=2.4, smooth=2):
    part = {"id": name, "joint": joint, "points": [list(p) for p in points], "color": list(color)}
    if smooth:
        part["smooth"] = smooth
    if width:
        part["outline"] = _outline(width)
    return part


def _ellipse(name, joint, cx, cy, rx, ry, color, width=2.4):
    part = {"id": name, "joint": joint,
            "ellipse": {"cx": cx, "cy": cy, "rx": rx, "ry": ry}, "color": list(color)}
    if width:
        part["outline"] = _outline(width)
    return part


def _box(x0, y0, x1, y1):
    return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]


def _scale_x(points, factor, shift=0.0):
    return [[round(x * factor + shift, 1), y] for x, y in points]


def _turned(part, squash, half):
    """The part turned like the facial features: squash about x=0, shift to +x."""
    shift = (1.0 - squash) * half
    if "ellipse" in part:
        e = part["ellipse"]
        return {"ellipse": {"cx": round(e["cx"] * squash + shift, 1), "cy": e["cy"],
                            "rx": round(e["rx"] * squash, 1), "ry": e["ry"]}}
    return {"points": _scale_x(part["points"], squash, shift)}


def _face_views(part, half, back=None):
    views = {name: _turned(part, squash, half) for name, squash in TURNS.items()}
    views["back"] = HIDDEN if back is None else back
    return views


def _body_views(part, back=None):
    """Bodies read flatter from the side: narrow the piece about its centre."""
    views = {"three_quarter": {"points": _scale_x(part["points"], 0.88)},
             "side": {"points": _scale_x(part["points"], 0.7)}}
    if back is not None:
        views["back"] = back
    return views


def _arc(cx, cy, rx, ry, start, end, steps):
    """Points along an ellipse arc, angles in degrees (0 = +x, 90 = down)."""
    return [[round(cx + rx * math.cos(math.radians(start + (end - start) * k / steps)), 1),
             round(cy + ry * math.sin(math.radians(start + (end - start) * k / steps)), 1)]
            for k in range(steps + 1)]


def _joints(build):
    body, top = build["body"], build["top"]
    joints = [
        {"id": "root", "parent": None},
        {"id": "torso", "parent": "root", "y": -4},
        {"id": "head", "parent": "torso", "y": -(body + build["head"][1] - 6)},
    ]
    for side, sign in (("l", -1), ("r", 1)):
        joints.extend([
            {"id": "upper_arm_" + side, "parent": "torso", "x": sign * (top - 2), "y": -body + 12,
             "rotation": -sign * 0.42},
            {"id": "forearm_" + side, "parent": "upper_arm_" + side, "y": 18},
            {"id": "hand_" + side, "parent": "forearm_" + side, "y": 14},
        ])
    for side, sign in (("l", -1), ("r", 1)):
        joints.extend([
            {"id": "thigh_" + side, "parent": "root", "x": sign * 13, "y": 2},
            {"id": "shin_" + side, "parent": "thigh_" + side, "y": build["leg"]},
            {"id": "foot_" + side, "parent": "shin_" + side, "y": 6},
        ])
    return joints


def _tube(name, upper, lower, bend, top, bottom, half_top, half_bottom, color):
    """One seamless piece from `top` to `bottom` (upper-joint local y) skinned
    to two joints, so it bends smoothly at `bend` (the lower joint) with no
    seam. Weights blend linearly over the rows either side of the bend."""
    rows = sorted({y for y in (top, (top + bend) / 2, bend - 6, bend, bend + 6,
                               (bend + bottom) / 2, bottom) if top <= y <= bottom})
    def half(y):
        return half_top + (half_bottom - half_top) * (y - top) / (bottom - top)
    def weight(y):  # share of the lower joint
        return min(max((y - (bend - 8)) / 16, 0.0), 1.0)
    left = [[-round(half(y), 2), round(y, 2)] for y in rows]
    right = [[round(half(y), 2), round(y, 2)] for y in reversed(rows)]
    points = left + right
    skin = []
    for _, y in points:
        w = round(weight(y), 4)
        if w <= 0:
            skin.append([{"joint": upper, "weight": 1}])
        elif w >= 1:
            skin.append([{"joint": lower, "weight": 1}])
        else:
            skin.append([{"joint": upper, "weight": round(1 - w, 4)},
                         {"joint": lower, "weight": w}])
    part = _polygon(name, upper, points, color)
    part["skin"] = skin
    return part


def _legs(build, pants, shoes):
    parts = []
    leg = build["leg"]
    for side, sign in (("l", -1), ("r", 1)):
        parts.extend([
            # Thigh and shin share one trouser leg that bends at the knee.
            _tube("leg_" + side, "thigh_" + side, "shin_" + side, leg, -8, leg + 6, 11, 10.5,
                  pants),
            _ellipse("shoe_" + side, "foot_" + side, sign * 4, 3, 16, 7.5, shoes),
        ])
    return parts


def _torso(build, shirt):
    body, top, hem = build["body"], build["top"], build["hem"]
    points = [[-top, -body], [top, -body], [hem, 8], [-hem, 8]]
    part = _polygon("body", "torso", points, shirt, width=2.6)
    part["views"] = _body_views(part)
    return part


def _arms(sleeve, hand):
    parts = []
    for side in ("l", "r"):
        parts.extend([
            # One sleeve from shoulder to wrist, bending at the elbow (y=18).
            _tube("arm_" + side, "upper_arm_" + side, "forearm_" + side, 18, -8, 33, 8, 7,
                  sleeve),
            _ellipse("hand_" + side, "hand_" + side, 0, 6, 8.5, 9.5, hand),
        ])
    return parts


def _face(build, brow=3.2):
    rx, ry = build["head"]
    mouth_y = build["mouth"]
    parts = [_ellipse("face", "head", 0, 0, rx, ry, build["skin"], 2.8)]
    for side, cx in (("l", -18), ("r", 18)):
        eye = _ellipse("eye_" + side, "head", cx, -10, 15, 19, WHITE, 2.2)
        eye["variants"] = {"blink": {"ellipse": {"cx": cx, "cy": -7, "rx": 15, "ry": 2.2},
                                     "color": list(INK)}}
        pupil = _ellipse("pupil_" + side, "head", cx + (5 if side == "l" else -5), -8, 3.4, 3.8,
                         INK, 0)
        pupil["variants"] = {"blink": HIDDEN.copy()}
        x0, x1 = cx - 12, cx + 12
        base = -36
        b = brow
        sad = (base + 2, base - 5) if side == "l" else (base - 5, base + 2)
        angry = (base - 5, base + 3) if side == "l" else (base + 3, base - 5)
        browpart = _polygon("brow_" + side, "head", _box(x0, base, x1, base + b), INK, 0, 0)
        browpart["variants"] = {
            "happy": {"points": [[x0, base - 2], [cx, base - 6], [x1, base - 2],
                                 [x1, base - 2 + b], [cx, base - 6 + b], [x0, base - 2 + b]]},
            "raised": {"points": _box(x0, base - 8, x1, base - 8 + b)},
            "worried": {"points": [[x0, sad[0]], [x1, sad[1]], [x1, sad[1] + b], [x0, sad[0] + b]]},
            "angry": {"points": [[x0, angry[0]], [x1, angry[1]], [x1, angry[1] + b],
                                 [x0, angry[0] + b]]},
        }
        parts.extend([eye, pupil, browpart])
    mouth_color = [70, 26, 34, 255]
    mouth = _ellipse("mouth", "head", 0, mouth_y, 12, 1.8, mouth_color, 0)
    sizes = {
        "rest": (12, 1.8), "closed": (12, 1.0), "teeth_lip": (11, 3.6),
        "tongue_teeth": (11, 6), "tongue_up": (13, 7), "sibilant": (14, 3),
        "sh": (9.5, 7), "r": (10.5, 8), "a": (13, 13), "e": (15.5, 6),
        "i": (17, 3.6), "o": (8.5, 12), "u": (6, 8.5),
    }
    mouth["variants"] = {name: {"ellipse": {"cx": 0, "cy": mouth_y, "rx": rx_, "ry": ry_}}
                         for name, (rx_, ry_) in sizes.items()}
    mouth["variants"]["teeth_lip"].update(color=list(WHITE), outline=_outline(1.8))
    y = mouth_y
    mouth["variants"]["smile"] = {
        "points": [[-15, y - 5], [-7, y], [0, y + 1], [7, y], [15, y - 5], [9, y + 5], [0, y + 8],
                   [-9, y + 5]],
    }
    mouth["variants"]["frown"] = {
        "points": [[-13, y + 5], [-7, y], [0, y - 1], [7, y], [13, y + 5], [7, y + 3], [0, y + 2],
                   [-7, y + 3]],
    }
    parts.append(mouth)
    return parts


# The features span x = -33..33 (the eye ellipses), so the rig turns them by
# squashing about x=0 and shifting (1 - squash) * 33 toward +x.
FEATURE_HALF = 33.0


def _ilona(build):
    teal, hair, navy, red, yellow = (_rgba("#2AA096"), [92, 58, 40, 255], [52, 64, 110, 255],
                                     [204, 54, 52, 255], [250, 205, 60, 255])
    rx, ry = build["head"]
    legs = _legs(build, navy, red)
    body = _torso(build, teal)
    collar = _polygon("collar", "torso", [[-14, -62], [14, -62], [9, -52], [0, -48], [-9, -52]],
                      WHITE, 2)
    collar["views"] = _body_views(collar, HIDDEN)
    hair_back = _polygon("hair_back", "head", [[-64, 36], [-66, -10], [-56, -44], [-30, -60],
                                               [30, -60], [56, -44], [66, -10], [64, 36]],
                         hair, 2.6, 2)
    hair_back["views"] = {
        "three_quarter": {"points": [[-68, 36], [-70, -10], [-58, -46], [-30, -60], [26, -60],
                                     [50, -44], [56, -10], [52, 20]]},
        "side": {"points": [[-70, 38], [-72, -12], [-60, -46], [-30, -62], [16, -60], [40, -46],
                            [38, -20], [20, 0]]},
    }
    fringe = [[-54, -16], [-50, -36], [-34, -52], [0, -58], [34, -52], [50, -36], [54, -16],
              [44, -38], [30, -42], [14, -46], [-2, -42], [-18, -46], [-34, -42], [-46, -36]]
    bangs = _polygon("bangs", "head", fringe, hair, 2.4, 2)
    bangs["views"] = {
        "three_quarter": {"points": [[-58, 6], [-58, -26], [-44, -48], [-14, -58], [18, -56],
                                     [44, -42], [56, -18], [42, -34], [30, -44], [14, -46],
                                     [-2, -42], [-18, -38], [-34, -24], [-44, 6]]},
        "side": {"points": [[-58, 34], [-60, -20], [-46, -46], [-18, -58], [12, -58], [38, -48],
                            [54, -26], [38, -38], [18, -44], [0, -40], [-16, -28], [-26, -8],
                            [-34, 34]]},
        "back": {"points": [[-62, 34], [-62, -20], [-50, -46], [-24, -60], [24, -60], [50, -46],
                            [62, -20], [62, 34]]},
    }
    clip = _polygon("hair_clip", "head", [[-40, -50], [-31, -46], [-24, -52], [-28, -42],
                                          [-22, -34], [-32, -38], [-40, -32], [-38, -42],
                                          [-46, -46], [-38, -46]], yellow, 1.6, 0)
    clip["views"] = {"three_quarter": {"points": _scale_x(clip["points"], 0.8, -6)},
                     "side": {"points": _scale_x(clip["points"], 0.6, -10)},
                     "back": HIDDEN}
    return legs + [body, collar], [hair_back], [bangs, clip], []


def _topi(build):
    mustard, pocket, brown, sneaker = (_rgba("#ECB034"), [210, 146, 34, 255], [120, 86, 60, 255],
                                       [236, 236, 230, 255])
    purple, ginger = [118, 78, 188, 255], [204, 108, 48, 255]
    legs = _legs(build, brown, sneaker)
    body = _torso(build, mustard)
    pouch = _polygon("hoodie_pocket", "torso", [[-22, -18], [22, -18], [27, 2], [-27, 2]], pocket, 2)
    pouch["views"] = _body_views(pouch, HIDDEN)
    strings = []
    for side, x in (("l", -7), ("r", 7)):
        cord = _polygon("drawstring_" + side, "torso", _box(x - 1.5, -60, x + 1.5, -40), WHITE, 1.2, 0)
        cord["views"] = _body_views(cord, HIDDEN)
        strings.append(cord)
    tufts = []
    for side, sign in (("l", -1), ("r", 1)):
        tuft = _polygon("hair_tuft_" + side, "head",
                        [[sign * 50, -40], [sign * 66, -32], [sign * 58, -24], [sign * 66, -16],
                         [sign * 54, -12], [sign * 46, -26]], ginger, 2, 0)
        tufts.append(tuft)
    tufts[0]["views"] = {"side": {"points": [[-50, -40], [-70, -30], [-62, -20], [-70, -10],
                                             [-56, -4], [-46, -22]]}}
    tufts[1]["views"] = {"side": HIDDEN, "back": {"points": tufts[1]["points"]}}
    # The cap sits high so the brows stay readable just under the brim.
    crown_points = _arc(0, -47, 60, 32, 180, 360, 12)
    crown = _polygon("cap_crown", "head", crown_points, purple, 2.6, 2)
    band = _polygon("cap_button", "head", _box(-5, -83, 5, -77), purple, 1.6, 0)
    brim = _polygon("cap_brim", "head", [[-52, -49], [52, -49], [60, -43], [-60, -43]], purple,
                    2.4, 0)
    brim["views"] = {
        "three_quarter": {"points": [[-36, -49], [56, -49], [80, -43], [-30, -43]]},
        "side": {"points": [[10, -49], [60, -49], [96, -42], [10, -43]]},
        "back": HIDDEN,
    }
    freckles = []
    for side, sign in (("l", -1), ("r", 1)):
        for k, (dx, dy) in enumerate(((0, 0), (7, 4), (-6, 5))):
            dot = _ellipse(f"freckle_{side}{k}", "head", sign * (30 + dx), 12 + dy, 1.8, 1.8,
                           [190, 110, 80, 255], 0)
            dot["views"] = _face_views(dot, FEATURE_HALF)
            freckles.append(dot)
    return legs + [body, pouch] + strings, tufts, [crown, band, brim], freckles


def _rauno(build):
    shirt, overalls, strap, boots = ([196, 62, 50, 255], _rgba("#46648C"), [56, 82, 118, 255],
                                     [86, 58, 40, 255])
    hair = [112, 84, 62, 255]
    legs = _legs(build, overalls, boots)
    body = _torso(build, shirt)
    body_h = build["body"]
    bib = _polygon("overall_bib", "torso", [[-20, -50], [20, -50], [22, -20], [33, -16],
                                            [34, 8], [-34, 8], [-33, -16], [-22, -20]], overalls, 2.4, 0)
    bib["views"] = _body_views(bib)
    straps = []
    for side, sign in (("l", -1), ("r", 1)):
        s = _polygon("strap_" + side, "torso",
                     [[sign * 16, -50], [sign * 22, -50], [sign * 27, -body_h], [sign * 19, -body_h]],
                     strap, 1.6, 0)
        s["views"] = _body_views(s)
        straps.append(s)
    pocket = _polygon("bib_pocket", "torso", _box(-11, -42, 11, -26), strap, 1.6, 0)
    pocket["views"] = _body_views(pocket, HIDDEN)
    buttons = []
    for side, sign in (("l", -1), ("r", 1)):
        b = _ellipse("bib_button_" + side, "torso", sign * 16, -46, 3, 3, [230, 196, 70, 255], 1.2)
        b["views"] = {
            "three_quarter": {"ellipse": {"cx": sign * 14, "cy": -46, "rx": 3, "ry": 3}},
            "side": {"ellipse": {"cx": sign * 11, "cy": -46, "rx": 3, "ry": 3}},
            "back": HIDDEN}
        buttons.append(b)
    sides = []
    for side, sign in (("l", -1), ("r", 1)):
        tuft = _polygon("side_hair_" + side, "head",
                        [[sign * 48, -30], [sign * 60, -24], [sign * 62, 0], [sign * 54, 12],
                         [sign * 46, 0]], hair, 2.2, 2)
        sides.append(tuft)
    sides[1]["views"] = {"side": HIDDEN}
    sides[0]["views"] = {"three_quarter": {"points": [[-48, -30], [-62, -22], [-64, 4], [-54, 16],
                                                      [-44, 2]]},
                         "side": {"points": [[-36, -36], [-60, -26], [-62, 6], [-50, 20], [-34, 4]]}}
    # Seen from behind, the fringe of hair round the back of the bald head.
    # Hidden (transparent) in every other view.
    back_patch = _polygon("hair_back_patch", "head", [[-50, -6], [-30, 0], [0, 4], [30, 0],
                                                       [50, -6], [54, 18], [30, 30], [0, 34],
                                                       [-30, 30], [-54, 18]], [0, 0, 0, 0], 0, 2)
    back_patch["views"] = {"back": {"color": list(hair)}}
    mouth_y = build["mouth"]
    moustache = _polygon("moustache", "head",
                         [[-26, mouth_y - 4], [-16, mouth_y - 14], [0, mouth_y - 11],
                          [16, mouth_y - 14], [26, mouth_y - 4], [14, mouth_y - 6],
                          [0, mouth_y - 4], [-14, mouth_y - 6]], hair, 2, 2)
    moustache["views"] = _face_views(moustache, FEATURE_HALF)
    return legs + [body, bib] + straps + [pocket] + buttons, [], sides, [moustache, back_patch]


def _rgba(hex_color):
    return [int(hex_color[i:i + 2], 16) for i in (1, 3, 5)] + [255]


SPECIFIC = {"ILONA": _ilona, "TOPI": _topi, "RAUNO": _rauno}
SLEEVES = {"ILONA": "#2AA096", "TOPI": "#ECB034", "RAUNO": "#C43E32"}


def characters():
    """Return fresh MotionScript character declarations keyed by CAST's ids."""
    result = {}
    for index, cast in enumerate(CAST):
        identifier = cast["id"]
        build = BUILD[identifier]
        body, behind_head, over_head, on_face = SPECIFIC[identifier](build)
        face = _face(build, brow=4.6 if identifier == "RAUNO" else 3.2)
        arms = _arms(_rgba(SLEEVES[identifier]), build["skin"])
        # Paper order: legs and body, then arms in front, then the head with
        # its hair/hat over the face shape but under eyes, brows and mouth.
        parts = body + arms + behind_head + face[:1] + over_head + face[1:] + on_face
        expressions = {
            "happy": [("brow_l", "happy"), ("brow_r", "happy"), ("mouth", "smile")],
            "sad": [("brow_l", "worried"), ("brow_r", "worried"), ("mouth", "frown")],
            "surprised": [("brow_l", "raised"), ("brow_r", "raised"), ("mouth", "o")],
            "angry": [("brow_l", "angry"), ("brow_r", "angry"), ("mouth", "closed")],
        }
        result[identifier] = {
            "asset": "characters/" + identifier.lower() + "-cutout-original",
            "default_pose": "idle",
            # Paper cut-outs move on twos and swap drawings when they turn.
            "timing": {"on": 2, "turn": "snap"},
            "rig": {
                "version": 1, "id": identifier.lower() + "_cutout_original",
                "joints": _joints(build), "parts": parts,
                "visemes": {"part": "mouth", "map": {name: name for name in VISEME_NAMES}},
                "expressions": {emotion: [{"part": part, "variant": variant}
                                          for part, variant in entries]
                                for emotion, entries in expressions.items()},
                "eyes": {"pupils": ["pupil_l", "pupil_r"], "travel": 4.0,
                         "blink": {"parts": ["eye_l", "eye_r", "pupil_l", "pupil_r"],
                                   "variant": "blink", "interval": 3.1 + index * 0.37,
                                   "duration": 0.12}},
            },
        }
    return result


def _crosses(a, b, c, d):
    def orient(p, q, r):
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    return (orient(a, b, c) * orient(a, b, d) < 0) and (orient(c, d, a) * orient(c, d, b) < 0)


def _simple(points):
    """No two non-adjacent edges cross (the rig validator rejects such shapes)."""
    edges = list(zip(points, points[1:] + points[:1]))
    n = len(edges)
    for (a, b), (_, c) in zip(edges, edges[1:] + edges[:1]):
        # A spike that runs straight back along its own edge retraces it.
        cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
        dot = (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1])
        if abs(cross) < 1e-9 and dot < 0:
            return False
    for i in range(n):
        for j in range(i + 2, n):
            if i == 0 and j == n - 1:
                continue
            if _crosses(*edges[i], *edges[j]):
                return False
    return True


def _rounded(points, samples):
    """motion_rig::round_corners: a quadratic between edge midpoints per corner."""
    if not samples:
        return points
    out = []
    n = len(points)
    for i in range(n):
        prev, p, nxt = points[i - 1], points[i], points[(i + 1) % n]
        a = [(prev[0] + p[0]) / 2, (prev[1] + p[1]) / 2]
        b = [(p[0] + nxt[0]) / 2, (p[1] + nxt[1]) / 2]
        for k in range(samples):
            t = k / samples
            u = 1 - t
            out.append([u * u * a[j] + 2 * u * t * p[j] + t * t * b[j] for j in (0, 1)])
    return out


def check(definitions):
    """Structural checks; Rust tests validate and render the generated rigs."""
    assert list(definitions) == [cast["id"] for cast in CAST]
    for identifier, character in definitions.items():
        rig = character["rig"]
        assert tuple(joint["id"] for joint in rig["joints"]) == JOINT_IDS
        seen = set()
        for joint in rig["joints"]:
            assert joint["parent"] is None or joint["parent"] in seen
            seen.add(joint["id"])
        parts = {part["id"]: part for part in rig["parts"]}
        assert len(parts) == len(rig["parts"]) <= 256, identifier
        for part in parts.values():
            assert part["joint"] in seen
            assert set(part.get("views", {})) <= {"three_quarter", "side", "back"}, part["id"]
            for shape in [part, *part.get("variants", {}).values(), *part.get("views", {}).values()]:
                assert not ("ellipse" in shape and "points" in shape)
                if "points" in shape:
                    points = shape["points"]
                    assert 3 <= len(points) <= 64, (identifier, part["id"])
                    area = sum(a[0] * b[1] - b[0] * a[1]
                               for a, b in zip(points, points[1:] + points[:1]))
                    assert abs(area) > 1e-9, (identifier, part["id"])
                    smooth = shape.get("smooth", part.get("smooth", 0))
                    assert smooth == 0 or 2 <= smooth <= 16, (identifier, part["id"])
                    assert _simple(_rounded(points, smooth)), (identifier, part["id"], points)
        for variant in rig["visemes"]["map"].values():
            assert variant in parts["mouth"]["variants"]
        for entries in rig["expressions"].values():
            for entry in entries:
                assert entry["variant"] in parts[entry["part"]]["variants"]


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def _color(hex_color, alpha=255):
    r, g, b, _ = _rgba(hex_color)
    return {"r": r, "g": g, "b": b, "a": alpha}


def _layer(scene_id, name, kind, rect, hex_color):
    return {"id": f"{scene_id}_{name}", "name": name, "scene_id": scene_id, "visible": True,
            "plane": "background", "opacity": 1.0, "attachment": None, "clip": None,
            "content": {"type": kind, "rect": dict(zip(("x", "y", "width", "height"), rect)),
                        "color": _color(hex_color)}}


def street(scene_id):
    """An original paper-collage street: sky, two houses, a fence and a pavement."""
    rect, oval = "rectangle", "ellipse"
    return [
        _layer(scene_id, "sky", rect, (0, 0, 1920, 1080), "#A9D8EE"),
        _layer(scene_id, "hill_far", oval, (-300, 520, 1500, 420), "#B9D7A8"),
        _layer(scene_id, "hill_near", oval, (900, 560, 1400, 380), "#A6CB93"),
        _layer(scene_id, "house_a", rect, (180, 400, 420, 340), "#E9D3A4"),
        _layer(scene_id, "house_a_roof", rect, (150, 360, 480, 60), "#9C5A48"),
        _layer(scene_id, "house_a_door", rect, (350, 590, 90, 150), "#6F8FA8"),
        _layer(scene_id, "house_a_window", rect, (230, 470, 90, 80), "#F7F2E2"),
        _layer(scene_id, "house_b", rect, (1340, 430, 380, 310), "#C9B3D6"),
        _layer(scene_id, "house_b_roof", rect, (1310, 390, 440, 60), "#5E6E8C"),
        _layer(scene_id, "house_b_window", rect, (1420, 500, 100, 80), "#F7F2E2"),
        _layer(scene_id, "house_b_window_2", rect, (1580, 500, 100, 80), "#F7F2E2"),
        _layer(scene_id, "fence", rect, (0, 700, 1920, 40), "#F3EEE2"),
        _layer(scene_id, "lawn", rect, (0, 740, 1920, 120), "#8DBF7A"),
        _layer(scene_id, "pavement", rect, (0, 860, 1920, 220), "#C8C4BC"),
        _layer(scene_id, "kerb", rect, (0, 850, 1920, 18), "#A8A49C"),
    ]


SCALE = 2.6
# Root (hip) height that puts the shoes on the pavement at this scale.
GROUND_Y = 150


def _key(at, **values):
    return dict(at=at, easing="linear", **values)


def demo_scene():
    duration = 8
    xs = {"ILONA": -520, "TOPI": 0, "RAUNO": 520}
    composition = {
        "background_color": "#A9D8EE",
        # Shots are framed on ones; the actors themselves are on twos.
        "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
        "layers": {actor: {"z": index, "visible": True} for index, actor in enumerate(xs)},
        # Longer legs stand lower on the hips: lift the root so every shoe
        # meets the pavement.
        "actor_tracks": {actor: [_key(0, x=x, y=GROUND_Y - (BUILD[actor]["leg"] - 16) * SCALE,
                                      scale_x=SCALE, scale_y=SCALE)]
                         for actor, x in xs.items()},
        "joint_tracks": {
            # A paper bob: the head pops between two drawings.
            "TOPI": {"head": [{"at": 0, "easing": "hold"},
                              {"at": 3.0, "rotation": 0.12, "easing": "hold"},
                              {"at": 3.5, "easing": "hold"},
                              {"at": 4.0, "rotation": 0.12, "easing": "hold"},
                              {"at": 4.5, "easing": "hold"}]},
        },
    }
    events = [
        {"type": "emotion", "actor": "ILONA", "at": 0, "value": "happy"},
        {"type": "gesture", "actor": "ILONA", "at": 0.5, "value": "wave", "duration": 2},
        {"type": "face", "actor": "TOPI", "at": 1.0, "value": "left"},
        {"type": "face", "actor": "RAUNO", "at": 1.5, "value": "three_quarter_left"},
        {"type": "emotion", "actor": "TOPI", "at": 2.5, "value": "surprised"},
        {"type": "face", "actor": "TOPI", "at": 2.5, "value": "front"},
        {"type": "emotion", "actor": "RAUNO", "at": 3.0, "value": "angry"},
        {"type": "face", "actor": "ILONA", "at": 4.0, "value": "right"},
        {"type": "emotion", "actor": "ILONA", "at": 4.0, "value": "surprised"},
        {"type": "face", "actor": "RAUNO", "at": 5.0, "value": "back"},
        {"type": "emotion", "actor": "TOPI", "at": 5.0, "value": "sad"},
        {"type": "face", "actor": "ILONA", "at": 6.0, "value": "front"},
        {"type": "emotion", "actor": "ILONA", "at": 6.0, "value": "happy"},
        {"type": "face", "actor": "RAUNO", "at": 6.5, "value": "front"},
        {"type": "emotion", "actor": "RAUNO", "at": 6.5, "value": "neutral"},
    ]
    return {"id": "katu", "duration": duration, "composition": composition, "events": events}


def portrait(actor):
    composition = {
        "background_color": "#F4EFE4",
        "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
        "layers": {actor: {"z": 0, "visible": True}},
        "actor_tracks": {actor: [_key(0, x=0, y=220, scale_x=3.0, scale_y=3.0)]},
    }
    return {"id": "hahmo", "duration": 4, "composition": composition, "events": []}


def project(title, cast, scenes, width, height, layers):
    source = {"project": {"title": title, "width": width, "height": height, "fps": 24},
              "characters": cast, "scenes": scenes}
    return {"format_version": 2,
            "source": json.dumps(source, ensure_ascii=False, separators=(",", ":")),
            "assets": [], "art": {"layers": layers}}


def generate(destination=DEFAULT):
    destination = Path(destination)
    (destination / "characters").mkdir(parents=True, exist_ok=True)
    cast = characters()
    check(cast)
    written = {}
    demo = encoded(project("Paperikatu", cast, [demo_scene()], 1920, 1080, street("katu")))
    (destination / "cutout-cast.kilsat").write_bytes(demo)
    written["cutout-cast.kilsat"] = hashlib.sha256(demo).hexdigest()
    for meta in CAST:
        actor = meta["id"]
        data = encoded(project(meta["name"], {actor: cast[actor]}, [portrait(actor)], 768, 1024, []))
        relative = f"characters/{actor.lower()}.kilsat"
        (destination / relative).write_bytes(data)
        written[relative] = hashlib.sha256(data).hexdigest()
    return written


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(generate(args.output), indent=2))
