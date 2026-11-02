#!/usr/bin/env python3
"""Original, project-owned stick figures for the native MotionScript rig format.

The declarations contain editable geometry, joint attachments and facial swaps;
they do not run inference or create procedural gestures. Coordinates are in rig
units, with hips at (0, 0), the face at (0, -195), and feet around y=140.
All six use the same 15-joint interface so authored FK clips can be shared.
"""
from __future__ import annotations

import json
import math


INK = [36, 40, 45, 255]
PAPER = [249, 247, 239, 255]
METAL = [185, 197, 201, 255]
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
CAST = [
    {"id": "NOUTAJA", "name": "Noutaja", "role": "Ystävällinen, lakoninen viikatemies", "accent": "#9A93AE"},
    {"id": "VEIKKO", "name": "Veikko", "role": "Samaistuttava työmatkalainen", "accent": "#5E91AE"},
    {"id": "PASMA", "name": "Päsmä", "role": "Mahtipontinen pomo ja sääntöjen vartija", "accent": "#B76257"},
    {"id": "KIPINA", "name": "Kipinä", "role": "Utelias keksijä ja korjaaja", "accent": "#CE8A47"},
    {"id": "KIIRE", "name": "Kiire", "role": "Aina liikkeellä oleva lähetti", "accent": "#9CA95F"},
    {"id": "SISU", "name": "Sisu", "role": "Neuvokas, rauhallinen mummo", "accent": "#9F7E9B"},
]
# Each character's acting profile (MotionScript `acting:` preset): how it
# stands, how big its gestures are, how much its feelings show and its tempo.
ACTING = {
    "NOUTAJA": "stoic",   # laconic: still, contained
    "VEIKKO": "tired",    # the commuter: slumped, small and slow
    "PASMA": "proud",     # the boss: chest out, measured
    "KIPINA": "lively",   # the inventor: big, quick and open
    "KIIRE": "nervous",   # the courier: hunched, quick, every feeling shows
    "SISU": "calm",       # the grandmother: upright and unhurried
}
HAND_SHAPES = ("fist", "open", "point", "grip", "thumbs_up")


def _rgba(hex_color):
    return [int(hex_color[i:i + 2], 16) for i in (1, 3, 5)] + [255]


def _outline(width=2.0, color=None):
    return {"color": list(INK if color is None else color), "width": width, "mode": "behind"}


def _polygon(name, joint, points, color, smooth=2, width=0):
    part = {"id": name, "joint": joint, "points": points, "color": list(color)}
    if smooth:
        part["smooth"] = smooth
    if width:
        part["outline"] = _outline(width)
    return part


def _ellipse(name, joint, cx, cy, rx, ry, color, width=0):
    part = {"id": name, "joint": joint,
            "ellipse": {"cx": cx, "cy": cy, "rx": rx, "ry": ry}, "color": list(color)}
    if width:
        part["outline"] = _outline(width)
    return part


def _joints(reaper=False):
    joints = [
        {"id": "root", "parent": None},
        {"id": "torso", "parent": "root", "y": -85},
        {"id": "head", "parent": "torso", "y": -110},
    ]
    for side, sign in (("l", -1), ("r", 1)):
        joints.extend([
            {"id": "upper_arm_" + side, "parent": "torso", "x": sign * (43 if reaper else 22),
             "y": -65, "rotation": -sign * 0.16},
            {"id": "forearm_" + side, "parent": "upper_arm_" + side, "y": 58},
            {"id": "hand_" + side, "parent": "forearm_" + side, "y": 54},
        ])
    for side, sign in (("l", -1), ("r", 1)):
        joints.extend([
            {"id": "thigh_" + side, "parent": "root", "x": sign * 13, "rotation": -sign * 0.055},
            {"id": "shin_" + side, "parent": "thigh_" + side, "y": 70},
            {"id": "foot_" + side, "parent": "shin_" + side, "y": 62},
        ])
    return joints


def _capsule(name, joint, start, end, radius, color=INK, width=0):
    """Keep constant-width sticks joined at their actual rotation pivots.

    Spline-rounding a long rectangle pulls its ends inward and narrows them.
    These explicit semicircular caps extend one radius beyond both pivots, so
    adjacent segments overlap even when an elbow, knee or shoulder is bent.
    """
    angle = math.atan2(end[1] - start[1], end[0] - start[0])
    points = []
    for center, first in ((start, angle + math.pi / 2),
                          (end, angle - math.pi / 2)):
        for step in range(7):
            theta = first + math.pi * step / 6
            points.append([round(center[0] + radius * math.cos(theta), 4),
                           round(center[1] + radius * math.sin(theta), 4)])
    return _polygon(name, joint, points, color, smooth=0, width=width)


def _body(reaper=False):
    # Each stick ends at the next joint, with an overlapping round cap. Bridges
    # join the offset hips/shoulders to the spine instead of floating beside it.
    parts = []
    for side, sign in (("l", -1), ("r", 1)):
        parts.extend([
            _capsule("thigh_" + side, "thigh_" + side, (0, 0), (0, 70), 4.5),
            _capsule("shin_" + side, "shin_" + side, (0, 0), (0, 62), 4),
            _ellipse("foot_" + side, "foot_" + side, sign * 5, 4, 15, 6, INK),
        ])
    shoulder = 43 if reaper else 22
    parts.extend([
        _capsule("hip_bridge", "root", (-13, 0), (13, 0), 4.5),
        # Lower spine stays with the hips. Upper spine bends at the torso joint
        # (0,-85), keeping the two pieces connected while the torso rotates.
        _capsule("spine_base", "root", (0, 0), (0, -85), 5),
        _capsule("torso", "torso", (0, 0), (0, -65), 5),
        _capsule("shoulder_bridge", "torso", (-shoulder, -65), (shoulder, -65), 4.5),
        _capsule("neck", "torso", (0, -65), (0, -110), 4.5),
    ])
    for side in ("l", "r"):
        parts.extend([
            _capsule("upper_arm_" + side, "upper_arm_" + side, (0, 0), (0, 58), 4.5),
            _capsule("forearm_" + side, "forearm_" + side, (0, 0), (0, 54), 4),
            dict(_ellipse("hand_" + side, "hand_" + side, 0, 2, 6, 8, INK),
                 variants=_hand_variants(side)),
        ])
    return parts


def _star(bumps, base, center=(0.0, 3.0)):
    """A star-shaped hand outline around `center` in hand-joint space.

    `bumps` are (angle in degrees, length, half-width in degrees): a finger
    block, a single finger or a thumb sticking out of a round palm of radius
    `base`. +y (90 degrees) runs from the wrist toward the fingertips. Each
    bump has a flat, rounded end (profile sqrt(1 - u^4)) sampled at seven
    angles; the palm is sampled every 45 degrees. Star-shaped about its
    centre, so the outline never crosses itself.
    """
    angles = {float(a) for a in range(0, 360, 45)}
    for angle, _, half in bumps:
        angles.update((angle + f * half) % 360 for f in (-1, -0.75, -0.4, 0, 0.4, 0.75, 1))
    points = []
    for degrees in sorted(angles):
        radius = base
        for angle, length, half in bumps:
            u = ((degrees - angle + 180) % 360 - 180) / half
            if abs(u) < 1:
                radius = max(radius, base + length * math.sqrt(1 - u ** 4))
        theta = math.radians(degrees)
        points.append([round(center[0] + radius * math.cos(theta), 1),
                       round(center[1] + radius * math.sin(theta), 1)])
    return points


def _hand_variants(side):
    """Drawn hand shapes in the cartoon mitten style: the fingers as one block,
    the thumb apart. The thumb sits on the side toward the body's centre."""
    inward = 1 if side == "l" else -1  # +x for the left hand, -x for the right
    thumb = 90 - inward * 60
    return {
        "fist": {"ellipse": {"cx": 0, "cy": 3, "rx": 7.5, "ry": 7}},
        "grip": {"ellipse": {"cx": 0, "cy": 2.5, "rx": 7.8, "ry": 6.4}},
        "open": {"points": _star([(90, 7.5, 40), (thumb - inward * 25, 6, 16)], 6.2), "smooth": 2},
        "point": {"points": _star([(90 + inward * 8, 10, 11), (thumb, 3, 18)], 6.6), "smooth": 2},
        "thumbs_up": {"points": _star([(90 + inward * 10, 7.5, 14)], 7.0), "smooth": 2},
    }


def _face():
    parts = [_ellipse("face", "head", 0, 0, 42, 42, PAPER, 2.2)]
    for side, cx in (("l", -16), ("r", 16)):
        eye = _ellipse("eye_" + side, "head", cx, -3, 8, 9, PAPER)
        eye["variants"] = {"blink": {"ellipse": {"cx": cx, "cy": -3, "rx": 8, "ry": 1.1}, "color": INK.copy()}}
        pupil = _ellipse("pupil_" + side, "head", cx, -2, 3.1, 4.2, INK)
        pupil["variants"] = {"blink": {"color": [0, 0, 0, 0]}}
        x0, x1 = cx - 9, cx + 9
        brow = _polygon("brow_" + side, "head", [[x0, -21], [x1, -21], [x1, -18], [x0, -18]], INK)
        sad_y = (-20, -27) if side == "l" else (-27, -20)
        angry_y = (-27, -19) if side == "l" else (-19, -27)
        brow["variants"] = {
            "happy": {"points": [[x0, -25], [cx, -28], [x1, -25], [x1, -22], [cx, -25], [x0, -22]]},
            "raised": {"points": [[x0, -31], [x1, -31], [x1, -28], [x0, -28]]},
            "worried": {"points": [[x0, sad_y[0]], [x1, sad_y[1]], [x1, sad_y[1] + 3], [x0, sad_y[0] + 3]]},
            "angry": {"points": [[x0, angry_y[0]], [x1, angry_y[1]], [x1, angry_y[1] + 3], [x0, angry_y[0] + 3]]},
        }
        parts.extend([eye, pupil, brow])
    mouth = _ellipse("mouth", "head", 0, 19, 10, 1.6, INK)
    sizes = {
        "rest": (10, 1.6), "closed": (10, 0.9), "teeth_lip": (9, 3),
        "tongue_teeth": (9, 5), "tongue_up": (11, 6), "sibilant": (12, 2.5),
        "sh": (8, 6), "r": (9, 7), "a": (11, 11), "e": (13, 5),
        "i": (14, 3), "o": (7, 10), "u": (5, 7),
    }
    mouth["variants"] = {name: {"ellipse": {"cx": 0, "cy": 19, "rx": rx, "ry": ry}}
                         for name, (rx, ry) in sizes.items()}
    mouth["variants"]["teeth_lip"].update(color=PAPER.copy(), outline=_outline(1.6))
    mouth["variants"]["smile"] = {
        "points": [[-12, 15], [-6, 19], [0, 20], [6, 19], [12, 15], [7, 22], [0, 24], [-7, 22]],
        "smooth": 2,
    }
    mouth["variants"]["frown"] = {
        "points": [[-11, 23], [-6, 19], [0, 18], [6, 19], [11, 23], [6, 22], [0, 21], [-6, 22]],
        "smooth": 2,
    }
    parts.append(mouth)
    return parts


def _accessories(identifier, accent):
    """Return clothing, behind-face parts and front-face/hand accessories."""
    clothes, behind, front = [], [], []
    if identifier == "NOUTAJA":
        clothes = [
            _polygon("cloak", "torso", [[-42, -67], [42, -67], [57, 66], [36, 79], [15, 70], [0, 82], [-19, 71], [-53, 82]], [29, 32, 38, 255], 3, 2.2),
            _polygon("cloak_hem", "torso", [[-48, 65], [-19, 67], [0, 75], [16, 64], [36, 72], [51, 63], [50, 69], [36, 79], [15, 71], [0, 82], [-19, 72], [-49, 79]], [55, 58, 65, 255], 2),
            _ellipse("clasp", "torso", 0, -45, 5, 5, accent, 1),
        ]
        behind = [
            _ellipse("hood", "head", 0, -5, 54, 64, [29, 32, 38, 255], 2.2),
            _ellipse("hood_seam", "head", 0, 0, 47, 51, [55, 58, 65, 255]),
        ]
        front = [
            _capsule("scythe_staff", "hand_r", (0, 177), (9, -225), 3.5, [70, 65, 65, 255], 1.2),
            _polygon("scythe_blade", "hand_r", [[6, -225], [48, -236], [100, -228], [145, -207], [167, -179], [128, -195], [90, -205], [47, -211], [11, -208]], METAL, 0, 2),
            _polygon("scythe_grip", "hand_r", [[-5, -10], [9, -10], [8, 13], [-6, 13]], accent, 2, 1),
        ]
    elif identifier == "VEIKKO":
        clothes = [
            _polygon("satchel_strap", "torso", [[-25, -65], [-18, -68], [25, 43], [18, 47]], [74, 84, 89, 255], 2),
            _polygon("satchel", "torso", [[9, 18], [39, 18], [41, 57], [7, 57]], accent, 3, 1.6),
            _polygon("satchel_flap", "torso", [[7, 18], [41, 18], [38, 33], [11, 33]], [75, 116, 141, 255], 2, 1),
        ]
        front = [
            _ellipse("cap_crown", "head", -2, -36, 43, 19, accent, 1.8),
            _polygon("cap_brim", "head", [[-43, -34], [28, -34], [52, -25], [48, -20], [-40, -23]], accent, 2, 1.4),
        ]
    elif identifier == "PASMA":
        clothes = [
            _polygon("collar_l", "torso", [[-20, -63], [0, -54], [-11, -40]], PAPER, 2, 1),
            _polygon("collar_r", "torso", [[0, -54], [20, -63], [11, -40]], PAPER, 2, 1),
            _polygon("tie", "torso", [[-6, -51], [0, -57], [6, -51], [9, -5], [0, 7], [-9, -5]], accent, 2, 1.2),
        ]
        front = [
            _polygon("clipboard", "hand_l", [[-54, -36], [-7, -36], [-7, 24], [-54, 24]], [143, 127, 109, 255], 2, 1.5),
            _polygon("clipboard_paper", "hand_l", [[-49, -30], [-12, -30], [-12, 18], [-49, 18]], PAPER, 2),
            _polygon("clipboard_clip", "hand_l", [[-40, -39], [-21, -39], [-21, -29], [-40, -29]], METAL, 2, 1),
            _polygon("clipboard_line_1", "hand_l", [[-44, -16], [-17, -16], [-17, -13], [-44, -13]], [116, 125, 128, 255], 2),
            _polygon("clipboard_line_2", "hand_l", [[-44, -5], [-23, -5], [-23, -2], [-44, -2]], [116, 125, 128, 255], 2),
        ]
    elif identifier == "KIPINA":
        clothes = [
            _polygon("tool_belt", "torso", [[-17, 37], [17, 37], [17, 50], [-17, 50]], [106, 99, 88, 255], 2, 1),
            _polygon("spanner", "torso", [[8, 38], [12, 8], [5, 0], [6, -8], [11, -10], [11, -3], [17, -2], [19, -8], [23, -4], [22, 4], [17, 10], [15, 39]], METAL, 2, 1),
        ]
        front = [
            _ellipse("beanie_crown", "head", 0, -40, 43, 21, accent, 1.6),
            _polygon("beanie_band", "head", [[-44, -35], [44, -35], [43, -22], [-43, -22]], accent, 2, 1.5),
            _ellipse("goggle_l", "head", -15, -29, 11, 9, [177, 199, 202, 255], 2),
            _ellipse("goggle_r", "head", 15, -29, 11, 9, [177, 199, 202, 255], 2),
            _polygon("goggle_bridge", "head", [[-5, -31], [5, -31], [5, -27], [-5, -27]], [89, 95, 101, 255], 2),
        ]
    elif identifier == "KIIRE":
        clothes = [
            _polygon("vest_l", "torso", [[-20, -61], [-5, -58], [-5, -7], [-22, -9]], accent, 2, 1.2),
            _polygon("vest_r", "torso", [[5, -58], [20, -61], [22, -9], [5, -7]], accent, 2, 1.2),
            _polygon("vest_zip", "torso", [[-2, -56], [2, -56], [2, -8], [-2, -8]], PAPER, 2),
        ]
        front = [
            _ellipse("courier_cap", "head", -3, -37, 42, 17, accent, 1.6),
            _polygon("courier_brim", "head", [[-40, -32], [32, -32], [49, -24], [46, -20], [-37, -24]], accent, 2, 1.2),
            _polygon("parcel", "hand_r", [[2, -28], [47, -28], [47, 22], [2, 22]], [180, 176, 155, 255], 2, 1.6),
            _polygon("parcel_band", "hand_r", [[19, -28], [29, -28], [29, 22], [19, 22]], accent, 2),
        ]
    elif identifier == "SISU":
        clothes = [
            _polygon("scarf", "torso", [[-18, -68], [18, -68], [15, -54], [-17, -54]], accent, 2, 1.2),
            _polygon("scarf_tail", "torso", [[5, -59], [16, -56], [24, -26], [13, -23]], accent, 2, 1),
        ]
        behind = [_ellipse("bun", "head", 35, -29, 18, 19, [142, 145, 148, 255], 1.8)]
        front = [
            _polygon("hair_cap", "head", [[-38, -18], [-33, -34], [-15, -43], [10, -43], [31, -34], [38, -17], [24, -23], [10, -31], [-4, -27], [-21, -32]], [142, 145, 148, 255], 2, 1),
            # Nonzero lens alpha keeps native outlines visible; pupils remain
            # readable through the lightly tinted, transparent lenses.
            _ellipse("glasses_l", "head", -16, -3, 12, 13, [193, 216, 220, 36], 1.4),
            _ellipse("glasses_r", "head", 16, -3, 12, 13, [193, 216, 220, 36], 1.4),
            _polygon("glasses_bridge", "head", [[-4, -5], [4, -5], [4, -2], [-4, -2]], INK, 2),
            _capsule("cane_staff", "hand_r", (17, 4), (17, 180), 3, [103, 91, 88, 255], 1),
            _polygon("cane_handle", "hand_r", [[-5, -8], [13, -8], [22, -1], [20, 11], [14, 11], [14, 2], [10, -1], [-5, -1]], [103, 91, 88, 255], 0, 1),
        ]
    # The hand ellipse reaches x=+-6. Move the clipboard's right edge inside it;
    # keep the courier parcel's near edge over the palm after spline rounding.
    shift = 6 if identifier == "PASMA" else -3 if identifier == "KIIRE" else 0
    if shift:
        for part in front:
            if part["id"].startswith(("clipboard", "parcel")):
                part["points"] = [[x + shift, y] for x, y in part["points"]]
                # Straight box edges retain their intended grip silhouette.
                part.pop("smooth", None)
    return clothes, behind, front


# Facial features (eyes, brows, mouth) span x = -25..25 on the head; the rig
# turns them by squashing toward x = 0 and shifting toward +x. Accessories that
# sit on the face use the same turn so they stay on the eyes. The torso uses a
# narrower spread for ties, collars and vests.
TURNS = {"three_quarter": 0.8, "side": 0.55}
HEAD_HALF, TORSO_HALF = 25.0, 20.0
HIDDEN = {"color": [0, 0, 0, 0]}


def _turned(part, squash, half):
    """The part's shape turned like the facial features (squash about x=0)."""
    shift = (1.0 - squash) * half
    if "ellipse" in part:
        e = part["ellipse"]
        return {"ellipse": {"cx": round(e["cx"] * squash + shift, 1), "cy": e["cy"],
                            "rx": round(e["rx"] * squash, 1), "ry": e["ry"]}}
    return {"points": [[round(x * squash + shift, 1), y] for x, y in part["points"]]}


def _turn_views(part, half, back=None):
    views = {name: _turned(part, squash, half) for name, squash in TURNS.items()}
    if back is not None:
        views["back"] = back
    return views


def _head_back(top=-43.0, nape=22.0):
    """Hair covering the back of the head (radius 43 about the head centre)."""
    points = []
    for k in range(20):
        a = 2 * math.pi * k / 20
        points.append([round(43 * math.sin(a)), round(max(-43 * math.cos(a), top)) if -43 * math.cos(a) < nape else nape])
    return points


def _brim(front, side_reach):
    """Cap brim drawn longer toward the facing side as the head turns."""
    (x0, y0), (x1, y1), (x2, y2), (x3, y3), (x4, y4) = front
    def at(reach, narrow):
        return [[x0 + narrow, y0], [x1, y1], [x2 + reach, y2], [x3 + reach, y3], [x4 + narrow, y4]]
    return {"three_quarter": {"points": at(side_reach * 0.45, 3)},
            "side": {"points": at(side_reach, 9)},
            "back": HIDDEN}


def _views(identifier, parts):
    """Per-view drawings for accessories the automatic face turn does not move.

    Without them, glasses, goggles and ties stay where the front view put them:
    on the back of the head from behind, and off the eyes in profile.
    """
    by_id = {part["id"]: part for part in parts}
    def head(name, back=HIDDEN):
        by_id[name]["views"] = _turn_views(by_id[name], HEAD_HALF, back)
    def torso(name, back=HIDDEN):
        by_id[name]["views"] = _turn_views(by_id[name], TORSO_HALF, back)
    def only_back(name, back=HIDDEN):
        by_id[name]["views"] = {"back": back}
    if identifier == "NOUTAJA":
        head("hood_seam")
        only_back("face")  # the hood covers the back of the head
        torso("clasp")
    elif identifier == "VEIKKO":
        by_id["cap_brim"]["views"] = _brim(by_id["cap_brim"]["points"], 14)
    elif identifier == "PASMA":
        torso("tie")
        torso("collar_l", {"points": [[-20, -63], [20, -63], [17, -56], [-17, -56]]})
        torso("collar_r")
    elif identifier == "KIPINA":
        for name in ("goggle_l", "goggle_r"):
            head(name)
        head("goggle_bridge", {"points": [[-44, -32], [44, -32], [44, -26], [-44, -26]]})
        torso("spanner")
    elif identifier == "KIIRE":
        torso("vest_l", {"points": [[-20, -61], [20, -61], [22, -9], [-22, -9]]})
        torso("vest_r")
        torso("vest_zip")
        by_id["courier_brim"]["views"] = _brim(by_id["courier_brim"]["points"], 14)
    elif identifier == "SISU":
        for name in ("glasses_l", "glasses_r", "glasses_bridge"):
            head(name)
        # Hair is drawn per view: in profile it covers the back of the head
        # (toward -x) and leaves the turned face clear.
        by_id["hair_cap"]["views"] = {
            "three_quarter": {"points": [[-40, -8], [-37, -27], [-22, -40], [0, -44], [22, -39],
                                         [36, -25], [24, -27], [11, -32], [-5, -28], [-19, -24],
                                         [-29, -10]]},
            "side": {"points": [[-42, 6], [-41, -18], [-32, -35], [-14, -43], [8, -43], [26, -36],
                                [33, -26], [19, -30], [4, -33], [-11, -27], [-23, -14], [-29, 6]]},
            "back": {"points": _head_back()},
        }
        by_id["bun"]["views"] = {
            "three_quarter": {"ellipse": {"cx": -24, "cy": -31, "rx": 17, "ry": 19}},
            "side": {"ellipse": {"cx": -40, "cy": -24, "rx": 16, "ry": 19}},
            "back": {"ellipse": {"cx": 0, "cy": -50, "rx": 18, "ry": 15}},
        }
        torso("scarf_tail")


def characters():
    """Return fresh MotionScript character declarations keyed by CAST's ids."""
    result = {}
    for cast in CAST:
        identifier = cast["id"]
        clothes, behind, front = _accessories(identifier, _rgba(cast["accent"]))
        body = _body(identifier == "NOUTAJA")
        parts = body[:11] + clothes + body[11:] + behind + _face() + front
        _views(identifier, parts)
        expressions = {
            "happy": [("brow_l", "happy"), ("brow_r", "happy"), ("mouth", "smile")],
            "sad": [("brow_l", "worried"), ("brow_r", "worried"), ("mouth", "frown")],
            "surprised": [("brow_l", "raised"), ("brow_r", "raised"), ("mouth", "o")],
            "angry": [("brow_l", "angry"), ("brow_r", "angry"), ("mouth", "closed")],
        }
        result[identifier] = {
            "asset": "characters/" + identifier.lower() + "-stick-original",
            "default_pose": "idle",
            "rig": {
                "version": 1, "id": identifier.lower() + "_stick_original",
                "joints": _joints(identifier == "NOUTAJA"), "parts": parts,
                "visemes": {"part": "mouth", "map": {name: name for name in VISEME_NAMES}},
                "expressions": {emotion: [{"part": part, "variant": variant} for part, variant in entries]
                                for emotion, entries in expressions.items()},
                "hands": {side: {"part": "hand_" + side[0],
                                 "shapes": {name: name for name in HAND_SHAPES}}
                          for side in ("left", "right")},
                "eyes": {"pupils": ["pupil_l", "pupil_r"], "travel": 3.4,
                         "blink": {"parts": ["eye_l", "eye_r", "pupil_l", "pupil_r"],
                                   "variant": "blink", "interval": 3.4 + CAST.index(cast) * 0.17,
                                   "duration": 0.12}},
            },
            "voice": {"provider": "piper", "preset": "piper_harri", "language": "fi-fi", "speed": 1.0},
            "acting": ACTING[identifier],
        }
    return result


def _check():
    """Check declaration bounds and references without claiming native acceptance."""
    definitions = characters()
    size = len(json.dumps(definitions, ensure_ascii=False, separators=(",", ":")).encode())
    # Per-view accessory drawings added ~1.3 KB and the five drawn hand shapes
    # ~12 KB; the starter project keeps its own 100 KB check.
    assert size < 76_000, size
    assert list(definitions) == [cast["id"] for cast in CAST]
    counts = {}
    for identifier, character in definitions.items():
        rig = character["rig"]
        assert tuple(joint["id"] for joint in rig["joints"]) == JOINT_IDS
        parents = set()
        for joint in rig["joints"]:
            assert joint["parent"] is None or joint["parent"] in parents
            parents.add(joint["id"])
        parts = {part["id"]: part for part in rig["parts"]}
        assert len(parts) == len(rig["parts"]) <= 256
        for part in parts.values():
            assert part["joint"] in parents
            assert len(part.get("variants", {})) <= 16
            assert set(part.get("views", {})) <= {"three_quarter", "side", "back"}
            for shape in [part, *part.get("variants", {}).values(), *part.get("views", {}).values()]:
                assert not ("ellipse" in shape and "points" in shape)
                if "ellipse" in shape:
                    ellipse = shape["ellipse"]
                    assert all(math.isfinite(value) for value in ellipse.values())
                    assert ellipse["rx"] > 0 and ellipse["ry"] > 0
                if "points" in shape:
                    points = shape["points"]
                    assert 3 <= len(points) <= 64
                    assert all(math.isfinite(value) for point in points for value in point)
                    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(points, points[1:] + points[:1]))
                    assert abs(area) > 1e-9, (identifier, part["id"])
        assert tuple(rig["visemes"]["map"]) == VISEME_NAMES
        for name, variant in rig["visemes"]["map"].items():
            assert variant in parts[rig["visemes"]["part"]]["variants"], name
        assert set(rig["expressions"]) == {"happy", "sad", "surprised", "angry"}
        for entries in rig["expressions"].values():
            for entry in entries:
                assert entry["variant"] in parts[entry["part"]]["variants"]
        for side, hand in rig["hands"].items():
            assert set(hand["shapes"]) == set(HAND_SHAPES), side
            for variant in hand["shapes"].values():
                assert variant in parts[hand["part"]]["variants"], (side, variant)
        assert character["acting"] in ("calm", "lively", "nervous", "proud", "tired", "stoic")
        for name in rig["eyes"]["blink"]["parts"]:
            assert "blink" in parts[name]["variants"]
        counts[identifier] = len(parts)
    drawn = {identifier: sum(1 for part in character["rig"]["parts"] if part.get("views"))
             for identifier, character in definitions.items()}
    assert all(drawn.values()), drawn
    print(json.dumps({"characters": len(definitions), "joints_each": len(JOINT_IDS),
                      "parts": counts, "parts_with_views": drawn,
                      "compact_character_bytes": size}, ensure_ascii=False))


if __name__ == "__main__":
    _check()
