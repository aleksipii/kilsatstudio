#!/usr/bin/env python3
"""Generate Albus, the guitar-playing cat (examples/albus).

Albus is the user's own character: a grey cat with a rainbow headband, round
glasses, yellow eyes with slit pupils, a baggy white sweatshirt, white cargo
trousers, white trainers, a rainbow wristband and a white electric guitar on
a strap. This script turns that drawing into an editable paper cut-out rig in
the same style as examples/cutout-cast, with:

* five expressions (neutral, happy, sad, surprised, angry) acted by eyelids,
  pupils, mouth, fangs and ears, plus blink, gaze and the 13 mouth shapes;
* reusable project-local joint clips (`rig_clips`) for Albus's own moves
  (strumming, a solo, headbanging, a foot on the amp, licking a paw,
  stretching);
* two scenes: `ilmeet` (expressions) and `liikkeet` (moves on a small stage).

The rig keeps the 15 joint ids of the other casts, so the shared gestures
(`wave`, `jump`, `bow`, ...) drive Albus too. Coordinates are rig units with
the hips at (0, 0) and y pointing down.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import generate_cutout_cast as cc  # noqa: E402  (shared paper cut-out helpers)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "examples/albus"
ACTOR = "ALBUS"

INK = cc.INK
HIDDEN = cc.HIDDEN
FUR = [150, 150, 154, 255]
FUR_DARK = [118, 118, 124, 255]
FUR_LIGHT = [204, 204, 206, 255]
EAR_PINK = [236, 164, 170, 255]
NOSE = [240, 128, 128, 255]
EYE = [236, 228, 112, 255]
CLOTH = [248, 247, 242, 255]
CLOTH_SHADE = [226, 225, 218, 255]
SHOE = [244, 244, 240, 255]
GUITAR = [252, 251, 246, 255]
NECK = [238, 222, 186, 255]
STRAP = [70, 70, 76, 255]
MOUTH = [70, 30, 38, 255]
# The headband from top to bottom, as in the drawing.
RAINBOW = ["#2E8FC8", "#8A4FB8", "#3DBB5A", "#F2D33C", "#F08A2E", "#E2364E"]
WRISTBAND = ["#F4E070", "#7CCB78", "#78A8E6"]

BUILD = dict(body=74, top=30, hem=34, head=(56, 50), leg=30, mouth=21)
# The top of the head is a superellipse: a broad cat forehead the headband
# wraps round, as in the drawing. The jaw stays elliptical.
CROWN = 2.8
UPPER_ARM = 26
FOREARM = 20


def _rgba(hex_color):
    return cc._rgba(hex_color)


def _rotated(points, angle, origin):
    c, s = math.cos(angle), math.sin(angle)
    ox, oy = origin
    return [[round(ox + u * c - v * s, 2), round(oy + u * s + v * c, 2)] for u, v in points]


def _thin(path, width):
    """A closed polygon for a stroke along `path` (a short open polyline)."""
    out, back = [], []
    for i, (x, y) in enumerate(path):
        a = path[max(i - 1, 0)]
        b = path[min(i + 1, len(path) - 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy) or 1.0
        nx, ny = -dy / length * width / 2, dx / length * width / 2
        out.append([round(x + nx, 2), round(y + ny, 2)])
        back.append([round(x - nx, 2), round(y - ny, 2)])
    return out + back[::-1]


def _joints():
    body, top = BUILD["body"], BUILD["top"]
    joints = [
        {"id": "root", "parent": None},
        {"id": "torso", "parent": "root", "y": -4},
        {"id": "head", "parent": "torso", "y": -(body + BUILD["head"][1] - 6)},
    ]
    for side, sign, out, bend in (("l", -1, 0.3, 0.1), ("r", 1, 0.26, 0.14)):
        joints.extend([
            {"id": "upper_arm_" + side, "parent": "torso", "x": sign * (top + 1),
             "y": -body + 11, "rotation": -sign * out},
            {"id": "forearm_" + side, "parent": "upper_arm_" + side, "y": UPPER_ARM,
             "rotation": sign * bend},
            {"id": "hand_" + side, "parent": "forearm_" + side, "y": FOREARM},
        ])
    for side, sign in (("l", -1), ("r", 1)):
        joints.extend([
            {"id": "thigh_" + side, "parent": "root", "x": sign * 14, "y": 2},
            {"id": "shin_" + side, "parent": "thigh_" + side, "y": BUILD["leg"]},
            {"id": "foot_" + side, "parent": "shin_" + side, "y": 8},
        ])
    return joints


# ---------------------------------------------------------------- body ------

def _legs():
    parts = []
    leg = BUILD["leg"]
    for side, sign in (("l", -1), ("r", 1)):
        # Baggy cargo trousers: one piece bending at the knee.
        parts.append(cc._tube("leg_" + side, "thigh_" + side, "shin_" + side, leg, -8, leg + 8,
                              13, 12, CLOTH))
        # A cargo pocket on the outer side of each thigh.
        x0, x1 = sorted((sign * 4, sign * 13.5))
        pocket = cc._polygon("cargo_pocket_" + side, "thigh_" + side,
                             cc._soft(cc._box(x0, 4, x1, 16), 2), CLOTH_SHADE, 1.6, 2)
        sole = cc._ellipse("sole_" + side, "foot_" + side, sign * 4, 6, 17, 4, [200, 200, 196, 255],
                           2.2)
        shoe = cc._ellipse("shoe_" + side, "foot_" + side, sign * 4, 2, 16, 7.5, SHOE)
        for piece, cy in ((sole, 6), (shoe, 2)):
            ry = piece["ellipse"]["ry"]
            rx = piece["ellipse"]["rx"]
            piece["views"] = {
                "three_quarter": {"ellipse": {"cx": sign * 2 + 4, "cy": cy, "rx": rx - 1, "ry": ry}},
                "side": {"ellipse": {"cx": 7, "cy": cy, "rx": rx - 2, "ry": ry}},
            }
        parts.extend([pocket, sole, shoe])
    return parts


def _torso():
    body = cc._torso(BUILD, CLOTH)
    hem = cc._hem_band("sweater_hem", BUILD, CLOTH_SHADE, 7)
    # A loose crew neck under the chin.
    neck = cc._polygon("sweater_neck", "torso",
                       cc._arc(0, -BUILD["body"] + 2, 20, 8, 0, 180, 8), CLOTH_SHADE, 1.8, 0)
    neck["views"] = cc._body_views(neck, HIDDEN)
    return [body, hem, neck]


# Guitar frame: origin at the body centre on the torso, the u axis along the
# neck toward the headstock (up and to the figure's left = screen right).
GUITAR_AT = (-8.0, -22.0)
GUITAR_ANGLE = math.radians(-38)


def _guitar():
    def g(points):
        return _rotated(points, GUITAR_ANGLE, GUITAR_AT)

    body_shape = [[-26, -4], [-24, -13], [-16, -18], [-6, -15], [4, -14], [12, -21], [22, -21],
                  [19, -12], [12, -6], [12, 5], [18, 11], [20, 18], [11, 18], [2, 13], [-6, 17],
                  [-17, 18], [-24, 12], [-27, 4]]
    strap = cc._polygon("guitar_strap", "torso",
                        [[-6, -70], [2, -70], [-14, -34], [-21, -34]], STRAP, 1.6, 0)
    body = cc._polygon("guitar_body", "torso", g(body_shape), GUITAR, 2.6, 2)
    guard = cc._polygon("guitar_pickguard", "torso",
                        g([[-18, -8], [-10, -12], [6, -10], [8, 4], [-2, 10], [-14, 12], [-20, 4]]),
                        [232, 232, 226, 255], 1.4, 2)
    neck = cc._polygon("guitar_neck", "torso", g(cc._box(14, -3.6, 84, 3.6)), NECK, 1.8, 0)
    head = cc._polygon("guitar_headstock", "torso",
                       g([[82, -4.5], [92, -8], [104, -7], [106, -1], [100, 5], [82, 4.5]]),
                       GUITAR, 2.0, 2)
    parts = [strap, body, guard, neck, head]
    for k, u in enumerate((22, 32, 42, 52, 62, 72)):
        parts.append(cc._polygon(f"guitar_fret_{k}", "torso", g(cc._box(u, -3.4, u + 1, 3.4)),
                                 INK, 0, 0))
    for k, u in enumerate((-12, -2)):
        parts.append(cc._polygon(f"guitar_pickup_{k}", "torso", g(cc._box(u, -5, u + 2.6, 5)),
                                 [60, 60, 64, 255], 0, 0))
    bridge = cc._polygon("guitar_bridge", "torso", g(cc._box(-20, -5.5, -16, 5.5)),
                         [150, 150, 156, 255], 1.0, 0)
    parts.append(bridge)
    for k, u in enumerate((88, 95, 102)):
        x, y = g([[u, -9]])[0]
        parts.append(cc._ellipse(f"guitar_peg_{k}", "torso", x, y, 2.4, 2.4, INK, 0))
    # From behind only the strap and a little of the headstock would show;
    # keep it simple and hide the front of the guitar.
    for part in parts:
        part["views"] = {"back": HIDDEN}
    strap["views"] = {"back": {"points": [[-6, -70], [2, -70], [24, -24], [16, -24]]}}
    return parts


def _arms():
    parts = []
    for side, sign in (("l", -1), ("r", 1)):
        sleeve = cc._tube("arm_" + side, "upper_arm_" + side, "forearm_" + side, UPPER_ARM, 0,
                          UPPER_ARM + FOREARM - 1, 9.5, 8.2, CLOTH, cap=True)
        cuff = cc._polygon("cuff_" + side, "forearm_" + side,
                           cc._soft(cc._box(-8.6, FOREARM - 9, 8.6, FOREARM - 2), 2.5),
                           CLOTH_SHADE, 2.2)
        parts.extend([sleeve, cuff])
        if side == "l":
            # The rainbow wristband just above the paw.
            for k, colour in enumerate(WRISTBAND):
                y0 = FOREARM - 9 + k * 7 / 3
                parts.append(cc._polygon(f"wristband_{k}", "forearm_l",
                                         cc._box(-8.6, round(y0, 2), 8.6, round(y0 + 7 / 3, 2)),
                                         _rgba(colour), 0, 0))
            parts[-3]["outline"] = cc._outline(2.2)
        parts.extend([
            cc._ellipse("thumb_" + side, "hand_" + side, -sign * 8.5, 1, 4.4, 5.6, FUR, 2.2),
            cc._ellipse("hand_" + side, "hand_" + side, 0, 6, 9, 10, FUR),
        ])
        # Toe beans: little grooves on the paw.
        for k, dx in enumerate((-3.5, 0.5, 4.5)):
            parts.append(cc._polygon(f"paw_line_{side}{k}", "hand_" + side,
                                     cc._box(dx - 0.6, 10, dx + 0.6, 15), INK, 0, 0))
    return parts


# ---------------------------------------------------------------- head ------

EYE_X = 21
EYE_Y = -8
EYE_R = 14
LID_R = 11.8


def _arc_points(cx, cy, r, a0, a1, steps):
    return [[round(cx + r * math.cos(math.radians(a0 + (a1 - a0) * k / steps)), 2),
             round(cy + r * math.sin(math.radians(a0 + (a1 - a0) * k / steps)), 2)]
            for k in range(steps + 1)]


def _upper_lid(cx, cy, r, y_inner, y_outer, side):
    """The part of the eye above a straight lid edge; y values are relative to
    the eye centre. `side` 'l' has its outer corner at -x."""
    y_left, y_right = (y_outer, y_inner) if side == "l" else (y_inner, y_outer)
    a_left = math.degrees(math.atan2(y_left, -math.sqrt(r * r - y_left * y_left)))
    a_right = math.degrees(math.atan2(y_right, math.sqrt(r * r - y_right * y_right)))
    if a_left < 0:
        a_left += 360
    if a_right < 0:
        a_right += 360
    # Go over the top (270 degrees) from the left end to the right end.
    if a_right < a_left:
        a_right += 360
    return _arc_points(cx, cy, r, a_left, a_right, 10)


def _happy_lid(cx, cy, r):
    """Smiling eyes: the lower lid rises into an arch (a cat's content squint)."""
    return _arc_points(cx, cy, r, 15, 165, 8) + [[round(cx - r * 0.6, 2), round(cy - 1, 2)],
                                                  [cx, round(cy - 5, 2)],
                                                  [round(cx + r * 0.6, 2), round(cy - 1, 2)]]


def _ear(sign, kind):
    """Ear triangles (outer and pink inner) per expression."""
    shapes = {
        "base": ([[-50, -16], [-46, -84], [-12, -44]], [[-44, -26], [-42, -72], [-22, -44]]),
        "perk": ([[-48, -18], [-46, -92], [-14, -46]], [[-42, -28], [-42, -80], [-24, -46]]),
        "flat": ([[-52, -14], [-80, -56], [-20, -42]], [[-48, -24], [-70, -50], [-28, -40]]),
        "down": ([[-50, -14], [-72, -66], [-16, -42]], [[-46, -24], [-64, -58], [-26, -42]]),
    }
    outer, inner = shapes[kind]
    # Shapes are drawn for the left ear (-x); the right ear mirrors them.
    return ([[round(-sign * x, 2), y] for x, y in outer],
            [[round(-sign * x, 2), y] for x, y in inner])


def _head():
    rx, ry = BUILD["head"]
    mouth_y = BUILD["mouth"]
    behind, face_parts, features = [], [], []
    # Ears sit behind the face. They are expression parts (ears flatten when
    # angry, droop when sad, perk up when surprised) and draw explicit views so
    # they stay visible from behind instead of being treated as face features.
    for side, sign in (("l", -1), ("r", 1)):
        outer, inner = _ear(sign, "base")
        ear = cc._polygon("ear_" + side, "head", outer, FUR, 2.6, 2)
        inner_ear = cc._polygon("ear_inner_" + side, "head", inner, EAR_PINK, 0, 2)
        for part, index in ((ear, 0), (inner_ear, 1)):
            part["variants"] = {kind: {"points": _ear(sign, kind)[index]}
                                for kind in ("perk", "flat", "down")}
        ear["views"] = {"three_quarter": {"points": cc._scale_x(outer, 0.9, 4)},
                        "side": {"points": cc._scale_x(outer, 0.75, 10)},
                        "back": {"points": outer}}
        inner_ear["views"] = {"three_quarter": {"points": cc._scale_x(inner, 0.9, 4)},
                              "side": {"points": cc._scale_x(inner, 0.75, 10)},
                              "back": HIDDEN}
        behind.extend([ear, inner_ear])

    # A broad-browed head with fluffy tufts on the cheeks.
    face_points = []
    n = 32  # 64 points after rounding (the validator's per-polygon limit)
    for k in range(n):
        a = 2 * math.pi * k / n
        deg = math.degrees(a) % 360
        c, s = math.cos(a), math.sin(a)
        cheek = (15 < deg < 70) or (110 < deg < 165)
        r_scale = 1.07 if cheek and k % 2 == 0 else 1.0
        if s < 0:  # crown: superellipse
            c = math.copysign(abs(c) ** (2 / CROWN), c)
            s = -abs(s) ** (2 / CROWN)
        face_points.append([round(rx * r_scale * c, 2),
                            round(ry * r_scale * s * (1.04 if s > 0 else 1), 2)])
    face = cc._polygon("face", "head", face_points, FUR, 2.8, 2)
    muzzle = cc._ellipse("muzzle", "head", 0, mouth_y - 4, 22, 13, FUR_LIGHT, 0)
    face_parts.extend([face, muzzle])

    # Fur stripes on the forehead peek out above the headband.
    over_head = []
    band_top, band_bottom = -38, -17
    def band_half(y):
        """Half width of the band at `y`: the crown's width plus fabric."""
        return rx * (1 - abs(y / ry) ** CROWN) ** (1 / CROWN) + 3

    def band_row(y, inset, reverse=False):
        """One edge of a stripe, sagging slightly in the middle as it wraps."""
        half = band_half(y) - inset
        row = [[round(x * half, 2), round(y + 2.2 * (1 - x * x), 2)]
               for x in (-1, -0.5, 0, 0.5, 1)]
        return row[::-1] if reverse else row

    base_band = cc._polygon("headband", "head",
                            band_row(band_top, 0) + band_row(band_bottom, 0, True),
                            _rgba(RAINBOW[0]), 2.6, 0)
    over_head.append(base_band)
    step = (band_bottom - band_top) / len(RAINBOW)
    for k, colour in enumerate(RAINBOW[1:], start=1):
        y0 = band_top + k * step
        y1 = band_top + (k + 1) * step - (0.9 if k == len(RAINBOW) - 1 else 0)
        over_head.append(cc._polygon(f"headband_{k}", "head",
                                     band_row(y0, 0.9) + band_row(y1, 0.9, True),
                                     _rgba(colour), 0, 0))

    # Eyes behind glasses: the thick black outline of the yellow eye is the
    # round frame. Lids (face-coloured) and pupils draw inside it.
    for side, sign in (("l", -1), ("r", 1)):
        cx = sign * EYE_X
        eye = cc._ellipse("eye_" + side, "head", cx, EYE_Y, EYE_R, EYE_R, EYE, 0)
        eye["outline"] = {"color": list(INK), "width": 4.4}
        pupil = cc._polygon("pupil_" + side, "head",
                            [[cx, EYE_Y - 7], [cx + 2.6, EYE_Y], [cx, EYE_Y + 7], [cx - 2.6, EYE_Y]],
                            INK, 0, 0)
        pupil["variants"] = {
            "blink": HIDDEN.copy(),
            "wide": {"ellipse": {"cx": cx, "cy": EYE_Y, "rx": 6, "ry": 7}},
            "slit": {"points": [[cx, EYE_Y - 8], [cx + 1.2, EYE_Y], [cx, EYE_Y + 8],
                                [cx - 1.2, EYE_Y]]},
            "hidden": HIDDEN.copy(),
        }
        lid = cc._polygon("lid_" + side, "head", _upper_lid(cx, EYE_Y, LID_R, -6, -6, side),
                          [0, 0, 0, 0], 0, 0)
        lid["variants"] = {
            "blink": {"points": _arc_points(cx, EYE_Y, LID_R, 0, 360, 24)[:-1],
                      "color": list(FUR)},
            "happy": {"points": _happy_lid(cx, EYE_Y, LID_R), "color": list(FUR)},
            "sad": {"points": _upper_lid(cx, EYE_Y, LID_R, -7, 2, side), "color": list(FUR)},
            "angry": {"points": _upper_lid(cx, EYE_Y, LID_R, 1, -6, side), "color": list(FUR)},
        }
        lash = cc._polygon("lash_" + side, "head",
                           _thin([[cx - 9, EYE_Y + 1], [cx, EYE_Y + 3], [cx + 9, EYE_Y + 1]], 2.2),
                           [0, 0, 0, 0], 0, 0)
        lash["variants"] = {"blink": {"color": list(INK)},
                            "happy": {"points": _thin([[cx - 8, EYE_Y + 2], [cx, EYE_Y - 4],
                                                       [cx + 8, EYE_Y + 2]], 2.2),
                                      "color": list(INK)}}
        temple = cc._polygon("glasses_temple_" + side, "head",
                             cc._box(min(sign * (EYE_X + EYE_R), sign * (rx - 1)), EYE_Y - 2,
                                     max(sign * (EYE_X + EYE_R), sign * (rx - 1)), EYE_Y + 0.6),
                             INK, 0, 0)
        face_parts.extend([temple, eye, pupil, lid, lash])
    bridge = cc._polygon("glasses_bridge", "head",
                         _thin([[-EYE_X + EYE_R - 1, EYE_Y - 2], [0, EYE_Y - 5],
                                [EYE_X - EYE_R + 1, EYE_Y - 2]], 2.4), INK, 0, 0)
    heart = cc._polygon("forehead_heart", "head",
                        [[0, -14], [-3, -17], [-1.5, -19], [0, -18], [1.5, -19], [3, -17]],
                        [120, 120, 126, 255], 0, 0)
    nose = cc._polygon("nose", "head", [[-5, 6], [5, 6], [0, 11.5]], NOSE, 1.6, 0)
    philtrum = cc._polygon("philtrum", "head", cc._box(-0.7, 11, 0.7, mouth_y - 2), INK, 0, 0)
    whiskers = []
    for side, sign in (("l", -1), ("r", 1)):
        for k, (dy, tilt) in enumerate(((0, -5), (5, 0), (10, 5))):
            whiskers.append(cc._polygon(
                f"whisker_{side}{k}", "head",
                _thin([[sign * 22, mouth_y - 6 + dy], [sign * 60, mouth_y - 6 + dy + tilt]], 1.1),
                INK, 0, 0))
    face_parts.extend([bridge, heart, nose, philtrum] + whiskers)

    # The cat mouth: a small 'w' at rest, the shared mouth shapes for speech.
    y = mouth_y
    mouth = cc._polygon("mouth", "head",
                        _thin([[-11, y - 2], [-5.5, y + 2.5], [0, y - 1], [5.5, y + 2.5],
                               [11, y - 2]], 2.4), MOUTH, 0, 0)
    sizes = {
        "rest": (9, 1.6), "closed": (9, 1.0), "teeth_lip": (8, 3), "tongue_teeth": (8, 4.6),
        "tongue_up": (9.5, 5.5), "sibilant": (10, 2.4), "sh": (7, 5.5), "r": (7.5, 6),
        "a": (9.5, 10), "e": (11, 4.6), "i": (12, 2.8), "o": (6.5, 9), "u": (4.6, 6.4),
    }
    mouth["variants"] = {name: {"ellipse": {"cx": 0, "cy": y + 2, "rx": a, "ry": b}}
                         for name, (a, b) in sizes.items()}
    # At rest the 'w' line reads better than a tiny ellipse.
    mouth["variants"]["rest"] = {"points": mouth["points"]}
    mouth["variants"]["teeth_lip"].update(color=list(cc.WHITE), outline=cc._outline(1.6))
    mouth["variants"]["smile"] = {
        "points": [[-13, y - 3], [-6, y + 1], [0, y - 1], [6, y + 1], [13, y - 3], [8, y + 6],
                   [0, y + 9], [-8, y + 6]]}
    mouth["variants"]["frown"] = {
        "points": _thin([[-10, y + 5], [-5, y + 1], [0, y], [5, y + 1], [10, y + 5]], 2.4)}
    mouth["variants"]["hiss"] = {
        "points": [[-12, y + 1], [-6, y - 2], [0, y], [6, y - 2], [12, y + 1], [8, y + 8],
                   [0, y + 10], [-8, y + 8]]}
    fangs = cc._polygon("fangs", "head",
                        [[-7, y - 1], [7, y - 1], [7, y + 0.5], [5.5, y + 0.5], [4.5, y + 5],
                         [3.5, y + 0.5], [-3.5, y + 0.5], [-4.5, y + 5], [-5.5, y + 0.5],
                         [-7, y + 0.5]],
                        [0, 0, 0, 0], 0, 0)
    fangs["variants"] = {"angry": {"color": list(cc.WHITE)}}
    face_parts.extend([mouth, fangs])
    return behind, face_parts[:2], over_head, face_parts[2:]


def _turn_views(parts, center, half):
    """Explicit turned drawings for face details that are not acting parts, so
    they turn in step with the rig's own feature turn."""
    for part in parts:
        if "views" in part:
            continue
        views = {}
        for name, squash in cc.TURNS.items():
            shift = (1.0 - squash) * half
            if "ellipse" in part:
                e = part["ellipse"]
                views[name] = {"ellipse": {"cx": round(center + (e["cx"] - center) * squash + shift, 2),
                                           "cy": e["cy"], "rx": round(e["rx"] * squash, 2),
                                           "ry": e["ry"]}}
            else:
                views[name] = {"points": [[round(center + (x - center) * squash + shift, 2), y]
                                          for x, y in part["points"]]}
        views["back"] = HIDDEN
        part["views"] = views


EXPRESSIONS = {
    "happy": [("lid_l", "happy"), ("lid_r", "happy"), ("lash_l", "happy"), ("lash_r", "happy"),
              ("pupil_l", "hidden"), ("pupil_r", "hidden"), ("mouth", "smile")],
    "sad": [("lid_l", "sad"), ("lid_r", "sad"), ("mouth", "frown"),
            ("ear_l", "down"), ("ear_r", "down"), ("ear_inner_l", "down"), ("ear_inner_r", "down")],
    "surprised": [("pupil_l", "wide"), ("pupil_r", "wide"), ("mouth", "o"),
                  ("ear_l", "perk"), ("ear_r", "perk"), ("ear_inner_l", "perk"),
                  ("ear_inner_r", "perk")],
    "angry": [("lid_l", "angry"), ("lid_r", "angry"), ("pupil_l", "slit"), ("pupil_r", "slit"),
              ("mouth", "hiss"), ("fangs", "angry"),
              ("ear_l", "flat"), ("ear_r", "flat"), ("ear_inner_l", "flat"),
              ("ear_inner_r", "flat")],
}

ARM_LIMITS = {"upper_arm_l": {"min_rotation": -2.95, "max_rotation": 2.3},
              "upper_arm_r": {"min_rotation": -2.3, "max_rotation": 2.95},
              "forearm_l": {"min_rotation": -2.9, "max_rotation": 2.9},
              "forearm_r": {"min_rotation": -2.9, "max_rotation": 2.9}}


def character():
    behind, face, over_head, details = _head()
    body = _legs() + _torso() + _guitar()
    arms = _arms()
    parts = body + behind + face + over_head + details + arms
    # The rig turns its acting parts about their joint-local centre and half
    # width at rest; give the other face details the same turn.
    acting = {p for entries in EXPRESSIONS.values() for p, _ in entries}
    acting |= {"mouth", "pupil_l", "pupil_r", "lid_l", "lid_r", "lash_l", "lash_r"}
    xs = []
    for part in parts:
        if part["id"] in acting:
            if "ellipse" in part:
                e = part["ellipse"]
                xs += [e["cx"] - e["rx"], e["cx"] + e["rx"]]
            else:
                xs += [x for x, _ in part["points"]]
    center, half = (min(xs) + max(xs)) / 2, (max(xs) - min(xs)) / 2
    _turn_views([p for p in face[1:] + details if p["id"] not in acting], center, half)
    rig = {
        "version": 1, "id": "albus_cutout_original",
        "joints": _joints(), "parts": parts, "joint_limits": ARM_LIMITS,
        "visemes": {"part": "mouth", "map": {name: name for name in cc.VISEME_NAMES}},
        "expressions": {emotion: [{"part": part, "variant": variant} for part, variant in entries]
                        for emotion, entries in EXPRESSIONS.items()},
        "eyes": {"pupils": ["pupil_l", "pupil_r"], "travel": 4.0,
                 "blink": {"parts": ["lid_l", "lid_r", "pupil_l", "pupil_r", "lash_l", "lash_r"],
                           "variant": "blink", "interval": 3.4, "duration": 0.12}},
    }
    return {"asset": "characters/albus-cutout-original", "default_pose": "idle",
            "timing": {"on": 2, "turn": "snap"}, "rig": rig}


def check(albus):
    """Reuse the cut-out cast's structural checks."""
    saved = cc.CAST
    cc.CAST = [{"id": ACTOR}]
    try:
        cc.check({ACTOR: albus})
    finally:
        cc.CAST = saved
    # The validator also caps every polygon at 64 points after rounding.
    for part in albus["rig"]["parts"]:
        for shape in [part, *part.get("variants", {}).values(), *part.get("views", {}).values()]:
            if "points" in shape:
                smooth = shape.get("smooth", part.get("smooth", 0))
                assert len(shape["points"]) * max(smooth, 1) <= 64, part["id"]


# --------------------------------------------------------------- moves ------

def _k(at, easing="smoothstep", **values):
    key = {"at": round(at, 4), "easing": easing}
    key.update({k: round(v, 4) for k, v in values.items()})
    return key


# Offsets from the relaxed rest pose that put the paws on the guitar: the
# figure's right paw (screen left) over the strings, the left one on the neck.
STRUM = {"upper_arm_l": -0.3, "forearm_l": -0.85}
FRET = {"upper_arm_r": -0.1, "forearm_r": -2.62}
PLAY = {**STRUM, **FRET}


def _hold_pose(pose, start, end, ease_in=0.3):
    """Tracks that move into `pose`, hold it and come back to rest."""
    return {joint: [_k(start), _k(start + ease_in, rotation=value),
                    _k(end - ease_in, rotation=value), _k(end)]
            for joint, value in pose.items()}


def _strum_keys(pose_value, start, end, beat, depth, lead=0.3):
    keys = [_k(start), _k(start + lead, rotation=pose_value)]
    t, k = start + lead + beat, 0
    while t < end - lead - 1e-6:
        keys.append(_k(t, "linear", rotation=pose_value + (depth if k % 2 == 0 else 0)))
        t += beat
        k += 1
    keys.append(_k(end - lead, rotation=pose_value))
    keys.append(_k(end))
    return keys


def clip_soitto():
    """Strumming with a nodding head: Albus's signature idle."""
    d = 4.0
    tracks = _hold_pose(FRET, 0, d)
    tracks["forearm_l"] = _strum_keys(PLAY["forearm_l"], 0, d, 0.25, 0.22)
    tracks["upper_arm_l"] = _strum_keys(PLAY["upper_arm_l"], 0, d, 0.25, 0.06)
    tracks["head"] = [_k(0)] + [_k(0.5 * i, rotation=0.06 if i % 2 else -0.02, y=2 if i % 2 else 0)
                                for i in range(1, 8)] + [_k(d)]
    tracks["torso"] = [_k(0)] + [_k(0.5 * i, y=1.5 if i % 2 else 0) for i in range(1, 8)] + [_k(d)]
    return {"duration": d, "joint_tracks": tracks}


def clip_soolo():
    """A fast solo: leaning back, paw sliding up the neck, head thrown back."""
    d = 3.0
    tracks = {
        "upper_arm_r": [_k(0), _k(0.3, rotation=FRET["upper_arm_r"]),
                        _k(1.4, rotation=FRET["upper_arm_r"] - 0.25),
                        _k(2.6, rotation=FRET["upper_arm_r"] + 0.05), _k(d)],
        "forearm_r": [_k(0), _k(0.3, rotation=FRET["forearm_r"]),
                      _k(1.4, rotation=FRET["forearm_r"] - 0.2),
                      _k(2.6, rotation=FRET["forearm_r"] + 0.1), _k(d)],
        "forearm_l": _strum_keys(PLAY["forearm_l"], 0, d, 1 / 12, 0.18),
        "upper_arm_l": [_k(0), _k(0.3, rotation=PLAY["upper_arm_l"]),
                        _k(d - 0.3, rotation=PLAY["upper_arm_l"]), _k(d)],
        "torso": [_k(0), _k(0.6, rotation=-0.12, y=1), _k(2.4, rotation=-0.14, y=1), _k(d)],
        "head": [_k(0), _k(0.6, rotation=0.18), _k(2.4, rotation=0.2), _k(d)],
    }
    return {"duration": d, "joint_tracks": tracks}


def clip_headbang():
    """Headbanging on twos: the head drops on every beat."""
    d = 2.5
    tracks = _hold_pose(PLAY, 0, d, 0.25)
    head, torso = [_k(0, "hold")], [_k(0)]
    t, k = 0.25, 0
    while t < d - 0.3:
        down = k % 2 == 0
        head.append(_k(t, "hold", y=7 if down else -2, rotation=0.05 if down else -0.05))
        torso.append(_k(t, "linear", y=3 if down else 0))
        t += 0.25
        k += 1
    head.append(_k(d - 0.25, "hold"))
    head.append(_k(d, "hold"))
    torso.append(_k(d))
    tracks["head"], tracks["torso"] = head, torso
    return {"duration": d, "joint_tracks": tracks}


# Thigh lifted a little above horizontal, shin hanging straight down.
LEG_UP = 1.75


def clip_jalka_vahvistimelle():
    """The pose from the drawing: right foot up on the amp, rocking to the beat."""
    d = 3.5
    tracks = _hold_pose(PLAY, 0, d, 0.4)
    tracks["thigh_r"] = [_k(0), _k(0.5, rotation=-LEG_UP), _k(d - 0.5, rotation=-LEG_UP), _k(d)]
    tracks["shin_r"] = [_k(0), _k(0.5, rotation=LEG_UP), _k(d - 0.5, rotation=LEG_UP), _k(d)]
    tracks["torso"] = [_k(0), _k(0.5, rotation=-0.05)] + [
        _k(0.5 + 0.5 * i, rotation=-0.05 + (0.05 if i % 2 else 0), y=1 if i % 2 else 0)
        for i in range(1, 5)] + [_k(d - 0.5, rotation=-0.05), _k(d)]
    return {"duration": d, "joint_tracks": tracks}


def clip_tassun_nuolaisu():
    """Licking a paw, as cats do between songs."""
    d = 2.5
    up = {"upper_arm_l": -2.75, "forearm_l": 0.25}
    tracks = _hold_pose(up, 0, d, 0.45)
    tracks["forearm_l"] = [_k(0), _k(0.45, rotation=0.25)] + [
        _k(0.45 + 0.3 * i, rotation=0.25 + (0.12 if i % 2 else 0)) for i in range(1, 5)] + [
        _k(d - 0.45, rotation=0.25), _k(d)]
    tracks["head"] = [_k(0), _k(0.45, rotation=-0.14, y=2), _k(d - 0.45, rotation=-0.14, y=2), _k(d)]
    return {"duration": d, "joint_tracks": tracks}


def clip_venyttely():
    """A big cat stretch: arms up and out, body long, then a little shake."""
    d = 3.0
    pose = {"upper_arm_l": 1.85, "forearm_l": 0.45, "upper_arm_r": -1.9, "forearm_r": -0.5}
    tracks = _hold_pose(pose, 0, d, 0.7)
    tracks["torso"] = [_k(0), _k(0.3, scale_y=0.95, y=1), _k(0.9, scale_y=1.06, y=-2),
                       _k(2.2, scale_y=1.06, y=-2), _k(2.5, scale_y=0.98), _k(d)]
    tracks["head"] = [_k(0), _k(0.9, rotation=-0.08), _k(2.2, rotation=0.06), _k(2.4, rotation=-0.06),
                      _k(2.6, rotation=0.04), _k(d)]
    return {"duration": d, "joint_tracks": tracks}


CLIPS = {
    "soitto": clip_soitto,
    "soolo": clip_soolo,
    "headbang": clip_headbang,
    "jalka_vahvistimelle": clip_jalka_vahvistimelle,
    "tassun_nuolaisu": clip_tassun_nuolaisu,
    "venyttely": clip_venyttely,
}


def place(clips, placements):
    """Joint tracks for `placements` [(clip name, start seconds)], in order."""
    tracks = {}
    for name, start in placements:
        for joint, keys in clips[name]["joint_tracks"].items():
            out = tracks.setdefault(joint, [])
            for key in keys:
                key = dict(key, at=round(key["at"] + start, 4))
                if out and key["at"] <= out[-1]["at"] + 1e-6:
                    if all(key.get(f, 0) == 0 for f in ("rotation", "x", "y")) and \
                            key.get("scale_y", 1) == 1:
                        continue
                    raise ValueError(f"{name} overlaps the previous move on {joint}")
                out.append(key)
    return tracks


# -------------------------------------------------------------- scenes ------

SCALE = 2.4
GROUND_Y = 84
ACTOR_X = -40
# Measured from renders: scene y = 757.6 + actor y + rig y * SCALE, and the
# actor x is relative to the frame centre.
ROOT_SCREEN_Y = 757.6 + GROUND_Y


def amp_box():
    """Where the amp stands so the raised right shoe lands on its top."""
    leg = BUILD["leg"]
    hip_x, hip_y = 14, 2
    knee_x = hip_x + leg * math.sin(LEG_UP)
    knee_y = hip_y + leg * math.cos(LEG_UP)
    shoe_bottom = knee_y + 8 + 2 + 7.5  # foot joint, shoe centre, shoe radius
    rest_bottom = hip_y + leg + 8 + 2 + 7.5
    shoe_x = knee_x + 4
    top = ROOT_SCREEN_Y + shoe_bottom * SCALE - 2
    floor = ROOT_SCREEN_Y + rest_bottom * SCALE
    centre = 960 + ACTOR_X + shoe_x * SCALE
    width = 116
    return round(centre - width / 2), round(top), width, round(floor - top)


def _layer(scene_id, name, kind, rect, hex_color):
    return cc._layer(scene_id, name, kind, rect, hex_color)


def stage(scene_id):
    """An original little rehearsal stage with Albus's amp."""
    rect, oval = "rectangle", "ellipse"
    ax, top, w, h = amp_box()
    r = round(h * 0.2)  # speaker radius
    sx = ax + w // 2 - r
    return [
        _layer(scene_id, "wall", rect, (0, 0, 1920, 1080), "#3B3350"),
        _layer(scene_id, "spot", oval, (560, -120, 800, 1020), "#574B72"),
        _layer(scene_id, "spot_core", oval, (680, 40, 560, 820), "#6A5B88"),
        _layer(scene_id, "floor", rect, (0, 800, 1920, 280), "#8A6A4E"),
        _layer(scene_id, "floor_edge", rect, (0, 792, 1920, 14), "#6E523C"),
        _layer(scene_id, "amp_outline", rect, (ax - 5, top - 5, w + 10, h + 10), "#16141A"),
        _layer(scene_id, "amp", rect, (ax, top, w, h), "#F4F2EC"),
        _layer(scene_id, "amp_grille_l", rect, (ax + w // 4, top, 4, h), "#16141A"),
        _layer(scene_id, "amp_grille_r", rect, (ax + 3 * w // 4 - 4, top, 4, h), "#16141A"),
        _layer(scene_id, "amp_speaker_top_ring", oval, (sx - 3, top + h // 4 - r - 3,
                                                        2 * r + 6, 2 * r + 6), "#16141A"),
        _layer(scene_id, "amp_speaker_top", oval, (sx, top + h // 4 - r, 2 * r, 2 * r), "#F4F2EC"),
        _layer(scene_id, "amp_speaker_low_ring", oval, (sx - 3, top + 3 * h // 4 - r - 3,
                                                        2 * r + 6, 2 * r + 6), "#16141A"),
        _layer(scene_id, "amp_speaker_low", oval, (sx, top + 3 * h // 4 - r, 2 * r, 2 * r),
               "#F4F2EC"),
    ]


def moves_scene(clips):
    x = ACTOR_X
    placements = [("soitto", 0.0), ("soolo", 4.2), ("headbang", 7.4), ("jalka_vahvistimelle", 10.1),
                  ("tassun_nuolaisu", 16.4), ("venyttely", 19.1)]
    composition = {
        "background_color": "#3B3350",
        "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
        "layers": {ACTOR: {"z": 0, "visible": True}},
        "actor_tracks": {ACTOR: [cc._key(0, x=x, y=GROUND_Y, scale_x=SCALE, scale_y=SCALE)]},
        "joint_tracks": {ACTOR: place(clips, placements)},
    }
    events = [
        {"type": "emotion", "actor": ACTOR, "at": 0, "value": "happy"},
        {"type": "emotion", "actor": ACTOR, "at": 4.2, "value": "surprised"},
        {"type": "emotion", "actor": ACTOR, "at": 5.0, "value": "happy"},
        {"type": "emotion", "actor": ACTOR, "at": 7.4, "value": "angry"},
        {"type": "emotion", "actor": ACTOR, "at": 10.1, "value": "happy"},
        # Shared gestures from the library drive the same joints.
        {"type": "gesture", "actor": ACTOR, "at": 13.8, "value": "jump", "duration": 1.0},
        {"type": "emotion", "actor": ACTOR, "at": 13.8, "value": "surprised"},
        {"type": "gesture", "actor": ACTOR, "at": 15.0, "value": "wave", "duration": 1.3},
        {"type": "emotion", "actor": ACTOR, "at": 15.0, "value": "happy"},
        {"type": "emotion", "actor": ACTOR, "at": 16.4, "value": "neutral"},
        {"type": "emotion", "actor": ACTOR, "at": 19.1, "value": "surprised"},
        {"type": "emotion", "actor": ACTOR, "at": 20.0, "value": "happy"},
        {"type": "gesture", "actor": ACTOR, "at": 22.3, "value": "bow", "duration": 1.6},
        {"type": "emotion", "actor": ACTOR, "at": 22.3, "value": "neutral"},
        {"type": "emotion", "actor": ACTOR, "at": 24.0, "value": "happy"},
    ]
    return {"id": "liikkeet", "duration": 25, "composition": composition, "events": events}


EXPRESSION_STEPS = [
    (0.0, "neutral", "front"), (2.0, "happy", "front"), (4.0, "sad", "front"),
    (6.0, "surprised", "front"), (8.0, "angry", "front"), (10.0, "happy", "three_quarter_left"),
    (12.0, "neutral", "right"), (13.5, "neutral", "back"), (15.0, "happy", "front"),
]


def expressions_scene():
    composition = {
        "background_color": "#F4EFE4",
        "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
        "layers": {ACTOR: {"z": 0, "visible": True}},
        "actor_tracks": {ACTOR: [cc._key(0, x=0, y=140, scale_x=3.4, scale_y=3.4)]},
        # The head tilts a little with each feeling.
        "joint_tracks": {ACTOR: {
            "head": [_k(0), _k(2.2, rotation=0.06), _k(4.2, rotation=-0.08, y=3),
                     _k(6.1, y=-4), _k(8.2, rotation=0.0, y=2), _k(10.2), _k(17)],
        }},
        "gaze_tracks": {ACTOR: [{"at": 0, "x": 960, "y": 540}, {"at": 1.0, "x": 300, "y": 300},
                                {"at": 1.6, "x": 1600, "y": 300}, {"at": 2.0, "x": 960, "y": 540},
                                {"at": 4.0, "x": 960, "y": 1000}, {"at": 6.0, "x": 960, "y": 540}]},
    }
    events = []
    for at, emotion, face in EXPRESSION_STEPS:
        events.append({"type": "emotion", "actor": ACTOR, "at": at, "value": emotion})
        events.append({"type": "face", "actor": ACTOR, "at": at, "value": face})
    return {"id": "ilmeet", "duration": 17, "composition": composition, "events": events}


def portrait():
    composition = {
        "background_color": "#F4EFE4",
        "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
        "layers": {ACTOR: {"z": 0, "visible": True}},
        "actor_tracks": {ACTOR: [cc._key(0, x=0, y=200, scale_x=2.6, scale_y=2.6)]},
    }
    return {"id": "hahmo", "duration": 4, "composition": composition, "events": []}


def project(title, albus, scenes, width, height, layers, clips):
    source = {"project": {"title": title, "width": width, "height": height, "fps": 24},
              "rig_clips": clips, "characters": {ACTOR: albus}, "scenes": scenes}
    return {"format_version": 2,
            "source": json.dumps(source, ensure_ascii=False, separators=(",", ":")),
            "assets": [], "art": {"layers": layers}}


def generate(destination=DEFAULT):
    destination = Path(destination)
    (destination / "characters").mkdir(parents=True, exist_ok=True)
    albus = character()
    check(albus)
    clips = {name: make() for name, make in CLIPS.items()}
    written = {}
    files = {
        "albus.kilsat": project("Albus", albus, [moves_scene(clips), expressions_scene()],
                                1920, 1080, stage("liikkeet"), clips),
        "characters/albus.kilsat": project("Albus", albus, [portrait()], 768, 1024, [], clips),
    }
    for relative, value in files.items():
        data = cc.encoded(value)
        (destination / relative).write_bytes(data)
        written[relative] = hashlib.sha256(data).hexdigest()
    return written


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(generate(args.output), indent=2))
