#!/usr/bin/env python3
"""Generate the character library of the stick-studio pack.

Writes, deterministically and without network access:

* `examples/stick-studio/cast/<slug>.kilsat`: one editable character project per
  library character (MotionScript rig, 768x1024 still scene, shared rig clips);
* `examples/stick-studio/library/characters/<id>.json`: a `character` manifest
  (ORK-LIB-01) per new character and per existing stick-studio character, with
  `rig_project` geometry, measured bounds and casting data.

Every character is original geometry built on the same 15-joint interface,
13 visemes, four expressions, blink and five hand shapes as
`scripts/stick_characters.py`, so the shared rig clips and acting presets work
on all of them. Age group, build and height change joint lengths; clothing,
hair, headwear, face details and hand items are layered polygons and ellipses,
with per-view drawings where the face turn needs them.

Usage (from the repository root):
    python3 scripts/generate_character_library.py           # write files
    python3 scripts/generate_character_library.py --check   # verify files are up to date
"""
from __future__ import annotations

import copy
import hashlib
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from stick_characters import (  # noqa: E402
    CAST as STICK_CAST, HAND_SHAPES, HEAD_HALF, HIDDEN, INK, JOINT_IDS, METAL, PAPER, TORSO_HALF,
    VISEME_NAMES, _brim, _capsule, _ellipse, _face, _hand_variants, _head_back, _polygon,
    _rgba, _turn_views,
)
from generate_stick_studio import clips  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PACK = ROOT / "examples" / "stick-studio"
OUT_DIR = PACK / "library" / "characters"
LICENSE = {"license": "MIT", "author": "Koeta Studio", "origin": "original",
           "redistribution": "allowed"}
ACTING = ("calm", "lively", "nervous", "proud", "tired", "stoic")

# Proportions per age group: leg, torso and arm length factors, shoulder and hip
# half-widths, limb radius factor.
AGES = {
    "child": dict(leg=0.55, torso=0.62, arm=0.62, shoulder=15, hip=9, limb=0.85),
    "teen": dict(leg=0.86, torso=0.88, arm=0.88, shoulder=19, hip=12, limb=0.95),
    "adult": dict(leg=1.0, torso=1.0, arm=1.0, shoulder=22, hip=13, limb=1.0),
    "elder": dict(leg=0.94, torso=0.96, arm=0.97, shoulder=21, hip=13, limb=0.95),
}
BUILDS = {
    "slim": dict(shoulder=-2, hip=-1, limb=0.9, belly=0),
    "average": dict(shoulder=0, hip=0, limb=1.0, belly=0),
    "broad": dict(shoulder=7, hip=3, limb=1.15, belly=0),
    "round": dict(shoulder=3, hip=4, limb=1.1, belly=14),
}
HAIR_COLORS = {
    "black": "#2E2B2A", "brown": "#6B4A36", "auburn": "#94523A", "blond": "#D9B865",
    "red": "#B5603A", "grey": "#8E9194", "white": "#DCDCD6", "pink": "#D88AA6",
}


def rgba(value):
    return _rgba(value) if isinstance(value, str) else list(value)


def shade(color, factor):
    """Darker (factor < 1) or lighter (factor > 1) version of an RGBA colour."""
    if factor <= 1:
        rgb = [round(c * factor) for c in color[:3]]
    else:
        rgb = [round(c + (255 - c) * (factor - 1)) for c in color[:3]]
    return rgb + [color[3]]


def rnd(v):
    r = round(float(v), 2)
    return int(r) if r == int(r) else r


def pts(points):
    return [[rnd(x), rnd(y)] for x, y in points]


def poly(name, joint, points, color, smooth=2, width=1.4):
    return _polygon(name, joint, pts(points), rgba(color), smooth, width)


def oval(name, joint, cx, cy, rx, ry, color, width=1.2):
    return _ellipse(name, joint, rnd(cx), rnd(cy), rnd(rx), rnd(ry), rgba(color), width)


def cap(name, joint, start, end, radius, color, width=1.2):
    return _capsule(name, joint, (rnd(start[0]), rnd(start[1])), (rnd(end[0]), rnd(end[1])),
                    rnd(radius), rgba(color), width)


def rounded_rect(x0, y0, x1, y1, r=4, steps=3):
    """Rectangle with rounded corners as a plain polygon."""
    out = []
    for cx, cy, start in ((x1 - r, y0 + r, -90), (x1 - r, y1 - r, 0), (x0 + r, y1 - r, 90), (x0 + r, y0 + r, 180)):
        for k in range(steps + 1):
            a = math.radians(start + 90 * k / steps)
            out.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return out


def densify(points, step=28.0):
    """Split long edges so corner rounding only softens the corners."""
    out = []
    for i, a in enumerate(points):
        b = points[(i + 1) % len(points)]
        n = max(1, math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / step))
        out += [(a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n) for k in range(n)]
    assert len(out) <= 64, len(out)
    return out


def bumpy(cx, cy, rx, ry, bumps, depth, start=0.0, steps=48):
    """Closed outline with `bumps` round lobes (curly hair, chef's hat)."""
    out = []
    for k in range(steps):
        a = 2 * math.pi * k / steps
        f = 1 + depth * abs(math.sin(bumps * (a + start) / 2))
        out.append((cx + rx * f * math.cos(a), cy + ry * f * math.sin(a)))
    return out


# ---------------------------------------------------------------- the body
class Body:
    """Joint lengths and widths for one character (rig units, hips at 0,0, y down)."""

    def __init__(self, age, build, tall=1.0):
        a, b = AGES[age], BUILDS[build]
        self.L, self.T, self.A = a["leg"] * tall, a["torso"] * tall, a["arm"] * tall
        self.S = a["shoulder"] + b["shoulder"]
        self.hip = a["hip"] + b["hip"]
        self.r = a["limb"] * b["limb"]
        self.belly = b["belly"]
        self.thigh, self.shin = 70 * self.L, 62 * self.L
        self.upper, self.fore = 58 * self.A, 54 * self.A
        self.shoulder_y = -65 * self.T
        self.waist = 85 * self.T  # the hips in torso space
        self.neck = max(45 * self.T, 40)  # keeps a child's head clear of the shoulders

    def joints(self):
        joints = [
            {"id": "root", "parent": None},
            {"id": "torso", "parent": "root", "y": rnd(-self.waist)},
            {"id": "head", "parent": "torso", "y": rnd(self.shoulder_y - self.neck)},
        ]
        for side, sign in (("l", -1), ("r", 1)):
            joints += [
                {"id": "upper_arm_" + side, "parent": "torso", "x": sign * self.S,
                 "y": rnd(self.shoulder_y), "rotation": -sign * 0.16},
                {"id": "forearm_" + side, "parent": "upper_arm_" + side, "y": rnd(self.upper)},
                {"id": "hand_" + side, "parent": "forearm_" + side, "y": rnd(self.fore)},
            ]
        for side, sign in (("l", -1), ("r", 1)):
            joints += [
                {"id": "thigh_" + side, "parent": "root", "x": sign * self.hip, "rotation": -sign * 0.055},
                {"id": "shin_" + side, "parent": "thigh_" + side, "y": rnd(self.thigh)},
                {"id": "foot_" + side, "parent": "shin_" + side, "y": rnd(self.shin)},
            ]
        return joints

    def legs(self):
        parts = []
        for side, sign in (("l", -1), ("r", 1)):
            parts += [
                cap("thigh_" + side, "thigh_" + side, (0, 0), (0, self.thigh), 4.5 * self.r, INK, 0),
                cap("shin_" + side, "shin_" + side, (0, 0), (0, self.shin), 4 * self.r, INK, 0),
                oval("foot_" + side, "foot_" + side, sign * 5, 4, 15 * max(self.L, 0.7), 6, INK, 0),
            ]
        return parts

    def spine(self):
        S, r, sy = self.S, self.r, self.shoulder_y
        return [
            cap("hip_bridge", "root", (-self.hip, 0), (self.hip, 0), 4.5 * r, INK, 0),
            cap("spine_base", "root", (0, 0), (0, -self.waist), 5 * r, INK, 0),
            cap("torso", "torso", (0, 0), (0, sy), 5 * r, INK, 0),
            cap("shoulder_bridge", "torso", (-S, sy), (S, sy), 4.5 * r, INK, 0),
            cap("neck", "torso", (0, sy), (0, sy - self.neck), 4.5 * r, INK, 0),
        ]

    def arms(self):
        parts = []
        for side in ("l", "r"):
            parts += [
                cap("upper_arm_" + side, "upper_arm_" + side, (0, 0), (0, self.upper), 4.5 * self.r, INK, 0),
                cap("forearm_" + side, "forearm_" + side, (0, 0), (0, self.fore), 4 * self.r, INK, 0),
                dict(_ellipse("hand_" + side, "hand_" + side, 0, 2, 6, 8, INK), variants=_hand_variants(side)),
            ]
        return parts


# ---------------------------------------------------------------- clothing
def top_outline(b: Body, length, flare=6, chest=5):
    """Shirt/coat outline in torso space from the shoulders to `length` below the hips."""
    S, y0, w, belly = b.S, b.shoulder_y - 3, b.waist, b.belly
    hem, hem_x = w + length, b.hip + flare
    points = [(-(S + chest), y0), (S + chest, y0), (S + 3, b.shoulder_y + 26)]
    if belly:
        points += [(S + belly, w * 0.55), (hem_x + belly * 0.5, w * 0.92)]
    points += [(hem_x, hem), (-hem_x, hem)]
    if belly:
        points += [(-(hem_x + belly * 0.5), w * 0.92), (-(S + belly), w * 0.55)]
    points += [(-(S + 3), b.shoulder_y + 26)]
    return densify(points, 40.0)


def tops(b: Body, kind, color, extra):
    """Torso garments: (parts behind the body, garment, garment details)."""
    c = rgba(color)
    dark, light = shade(c, 0.78), shade(c, 1.35)
    behind, garment, details = [], [], []
    hem = {"tshirt": 6, "shirt": 6, "sweater": 8, "hoodie": 10, "jacket": 14, "suit": 16,
           "coat": 62 * b.L, "labcoat": 70 * b.L, "robe": 120 * b.L, "dress_top": -b.waist * 0.25,
           "tunic": 34 * b.L, "uniform": 14, "scrubs": 10, "overalls": 6, "cardigan": 18,
           "smock": 40 * b.L, "jumpsuit": 6, "sport": -4, "cape_suit": 4}[kind]
    flare = 22 if kind in ("coat", "labcoat", "robe", "smock") else 10 if kind in ("tunic", "cardigan") else 6
    garment.append(poly("top", "torso", top_outline(b, hem, flare), c))
    y0, w = b.shoulder_y, b.waist
    if kind in ("jacket", "suit", "coat", "labcoat", "cardigan", "uniform"):
        inner = PAPER if kind in ("suit", "uniform") else [214, 220, 222, 255] if kind == "labcoat" else shade(c, 0.88)
        details += [
            poly("top_inner", "torso", [(-9, y0 + 2), (9, y0 + 2), (3, w * 0.55), (-3, w * 0.55)], inner, 0, 1),
            poly("lapel_l", "torso", [(-15, y0 - 1), (-2, y0 + 4), (-4, y0 + 36), (-14, y0 + 22)], dark, 0, 1),
            poly("lapel_r", "torso", [(2, y0 + 4), (15, y0 - 1), (14, y0 + 22), (4, y0 + 36)], dark, 0, 1),
        ]
        for i in range(4 if kind == "coat" else 3):
            details.append(oval(f"button_{i}", "torso", 0, y0 + 46 + i * 18 * b.T, 2.4, 2.4, dark, 0.8))
    if kind == "hoodie":
        behind.append(oval("hood", "torso", 0, y0 - 10, b.S + 8, 16, dark, 1.4))
        details += [poly("pocket", "torso", [(-14, w * 0.62), (14, w * 0.62), (18, w * 0.92), (-18, w * 0.92)], dark, 0, 1),
                    poly("hood_cord_l", "torso", [(-6, y0 + 4), (-4, y0 + 4), (-4, y0 + 26), (-6, y0 + 26)], PAPER, 0, 0),
                    poly("hood_cord_r", "torso", [(4, y0 + 4), (6, y0 + 4), (6, y0 + 26), (4, y0 + 26)], PAPER, 0, 0)]
    if kind in ("tshirt", "sport") and extra.get("stripes"):
        for i in range(2):
            y = y0 + 22 + i * 22 * b.T
            details.append(poly(f"stripe_{i}", "torso", [(-(b.S + 2), y), (b.S + 2, y), (b.S + 2, y + 7), (-(b.S + 2), y + 7)], light, 0, 0))
    if kind == "sweater":
        details += [poly("knit_band", "torso", [(-(b.S + 3), y0 + 30), (b.S + 3, y0 + 30), (b.S + 3, y0 + 40), (-(b.S + 3), y0 + 40)], light, 0, 0),
                    poly("collar", "torso", [(-10, y0 - 2), (10, y0 - 2), (7, y0 + 6), (-7, y0 + 6)], dark, 0, 1)]
    if kind == "scrubs":
        details += [poly("v_neck", "torso", [(-8, y0 - 1), (8, y0 - 1), (0, y0 + 16)], dark, 0, 1),
                    poly("chest_pocket", "torso", [(-16, y0 + 24), (-5, y0 + 24), (-5, y0 + 36), (-16, y0 + 36)], dark, 0, 1)]
    if kind == "overalls":
        details += [poly("bib", "torso", [(-13, y0 + 20), (13, y0 + 20), (14, w), (-14, w)], dark, 0, 1.2),
                    poly("strap_l", "torso", [(-15, y0 - 1), (-10, y0 - 1), (-9, y0 + 21), (-13, y0 + 21)], dark, 0, 1),
                    poly("strap_r", "torso", [(10, y0 - 1), (15, y0 - 1), (13, y0 + 21), (9, y0 + 21)], dark, 0, 1),
                    oval("bib_button_l", "torso", -10, y0 + 24, 2.2, 2.2, METAL, 0.6),
                    oval("bib_button_r", "torso", 10, y0 + 24, 2.2, 2.2, METAL, 0.6)]
    if kind == "jumpsuit":
        details += [poly("zip", "torso", [(-1.5, y0 + 1), (1.5, y0 + 1), (1.5, w), (-1.5, w)], dark, 0, 0),
                    poly("chest_patch", "torso", [(6, y0 + 18), (16, y0 + 18), (16, y0 + 28), (6, y0 + 28)], light, 0, 0.8)]
    if kind == "robe":
        details.append(poly("robe_sash", "torso", [(-(b.S + 2), w - 6), (b.S + 2, w - 6), (b.S + 2, w + 4), (-(b.S + 2), w + 4)], dark, 0, 1))
    if extra.get("emblem"):
        details.append(oval("emblem", "torso", 0, y0 + 32, 9, 9, extra["emblem"], 1.2))
    if kind == "dress_top":
        details.append(poly("neckline", "torso", [(-9, y0 - 1), (9, y0 - 1), (0, y0 + 10)], shade(c, 1.25), 0, 0.8))
    if kind == "smock":
        details += [oval("paint_1", "torso", -10, w * 0.5, 4, 3, "#D27A54", 0),
                    oval("paint_2", "torso", 8, w * 0.7, 3, 4, "#5E91AE", 0),
                    oval("paint_3", "torso", -3, w * 0.95, 4, 3, "#DDAF5B", 0)]
    return behind, garment, details


def sleeves(b: Body, length, color):
    """Short, half or long sleeves over the arm sticks (after the arms)."""
    parts = []
    reach = {"short": 0.42, "half": 0.75, "long": 1.0}[length]
    for side in ("l", "r"):
        parts.append(cap("sleeve_" + side, "upper_arm_" + side, (0, -2), (0, b.upper * reach), 4.5 * b.r + 3.2, color))
        if length == "long":
            parts.append(cap("cuff_" + side, "forearm_" + side, (0, 0), (0, b.fore * 0.82), 4 * b.r + 2.8, color))
    return parts


def bottoms(b: Body, kind, color):
    """Trousers, shorts and skirts (after the leg sticks, before the spine)."""
    parts = []
    if kind in ("trousers", "shorts"):
        reach = 1.0 if kind == "trousers" else 0.62
        parts.append(cap("pants_seat", "root", (-b.hip, 0), (b.hip, 0), 4.5 * b.r + 4, color))
        for side in ("l", "r"):
            parts.append(cap("pants_thigh_" + side, "thigh_" + side, (0, -2), (0, b.thigh * reach), 4.5 * b.r + 3.4, color))
            if kind == "trousers":
                parts.append(cap("pants_shin_" + side, "shin_" + side, (0, 0), (0, b.shin * 0.9), 4 * b.r + 3, color))
    else:
        hem = b.thigh * (0.75 if kind == "skirt" else 1.6)
        top = b.hip + 9
        spread = top + 20 + hem * 0.12
        parts.append(poly("skirt", "root", densify([(-top, -14), (top, -14), (spread, hem), (-spread, hem)], 24.0), color, 2))
    return parts


# ---------------------------------------------------------------- hair
HAIR_TOPS = {
    "short": [(-41, -12), (-38, -30), (-23, -42), (0, -46), (23, -42), (38, -30), (41, -12), (31, -23), (13, -30), (-7, -27), (-27, -24)],
    "crop": [(-40, -16), (-34, -33), (-17, -43), (0, -45), (17, -43), (34, -33), (40, -16), (28, -28), (0, -34), (-28, -28)],
    "side_part": [(-42, -6), (-39, -29), (-24, -43), (0, -47), (24, -43), (39, -29), (42, -10), (34, -20), (16, -31), (-12, -24), (-31, -18)],
    "bob": [(-45, 16), (-46, -10), (-38, -33), (-20, -45), (0, -47), (20, -45), (38, -33), (46, -10), (45, 16), (36, 16), (33, -12), (18, -26), (0, -29), (-18, -26), (-33, -12), (-36, 16)],
    "spiky": [(-41, -10), (-44, -32), (-32, -36), (-30, -52), (-14, -44), (-6, -60), (6, -45), (18, -58), (24, -42), (40, -46), (37, -30), (44, -14), (30, -24), (10, -30), (-12, -27), (-28, -22)],
    "fringe": [(-42, -4), (-40, -28), (-24, -43), (0, -47), (24, -43), (40, -28), (42, -4), (34, -16), (22, -20), (8, -16), (-6, -20), (-20, -16), (-32, -18)],
    "wavy": [(-44, 4), (-44, -22), (-34, -38), (-14, -47), (8, -48), (28, -42), (42, -28), (45, -4), (36, -12), (30, -26), (14, -30), (-4, -26), (-18, -30), (-32, -18), (-36, 2)],
}


def hair_parts(style, color):
    """(behind-head parts, front parts) for a hairstyle."""
    c = rgba(HAIR_COLORS.get(color, color))
    behind, front = [], []
    top = {"long": "fringe", "ponytail": "short", "bun": "short", "top_bun": "short",
           "pigtails": "fringe", "curly": None, "afro": None, "bald": None}.get(style, style)
    if style == "long":
        behind.append(poly("hair_back", "head", [(-48, -12), (-44, -36), (-24, -50), (0, -53), (24, -50), (44, -36), (48, -12), (50, 40), (40, 66), (20, 58), (0, 50), (-20, 58), (-40, 66), (-50, 40)], c))
    elif style == "ponytail":
        behind.append(poly("ponytail", "head", [(30, -34), (46, -38), (60, -20), (64, 12), (56, 44), (46, 28), (44, 4), (36, -14)], c))
    elif style == "bun":
        behind.append(oval("bun", "head", 36, -28, 17, 18, c, 1.6))
    elif style == "top_bun":
        behind.append(oval("bun", "head", 0, -50, 18, 15, c, 1.6))
    elif style == "pigtails":
        behind += [poly("pigtail_l", "head", [(-38, -14), (-54, -6), (-62, 18), (-56, 36), (-46, 18), (-40, 2)], c),
                   poly("pigtail_r", "head", [(38, -14), (54, -6), (62, 18), (56, 36), (46, 18), (40, 2)], c)]
    elif style == "curly":
        behind.append(poly("curls", "head", bumpy(0, -8, 50, 50, 9, 0.11), c, 0))
        front.append(poly("hair_top", "head", bumpy(0, -38, 36, 12, 5, 0.25, 0.3, 32), c, 0))
    elif style == "afro":
        behind.append(poly("afro", "head", bumpy(0, -14, 60, 58, 11, 0.08), c, 0))
        front.append(poly("hair_top", "head", [(-40, -18), (-30, -36), (-12, -44), (12, -44), (30, -36), (40, -18), (24, -28), (0, -32), (-24, -28)], c))
    elif style == "bald":
        front += [poly("hair_side_l", "head", [(-43, -8), (-38, -20), (-32, -12), (-34, 4)], c, 2, 1),
                  poly("hair_side_r", "head", [(43, -8), (38, -20), (32, -12), (34, 4)], c, 2, 1)]
    if top:
        front.append(poly("hair_top", "head", HAIR_TOPS[top], c))
    return behind, front


FULL_BACK = {"long", "curly", "afro", "pigtails", "wavy"}


def hair_views(parts, cover_back=True, nape=24.0):
    """Hair follows the head turn: the top shifts back, the back of the head is covered."""
    for part in parts:
        pid = part["id"]
        if pid == "hair_top":
            views = {name: {"points": [[rnd(x * s + d), y] for x, y in part["points"]]}
                     for name, s, d in (("three_quarter", 0.92, -5), ("side", 0.86, -11))}
            views["back"] = {"points": _head_back(nape=nape)} if cover_back else HIDDEN
            part["views"] = views
        elif pid == "hair_side_r":
            part["views"] = {"side": HIDDEN}
        elif pid == "hair_side_l":
            part["views"] = {"side": {"points": [[rnd(x * 0.9 + 18), y] for x, y in part["points"]]}}
        elif pid in ("ponytail", "pigtail_l", "pigtail_r"):
            part["views"] = {name: {"points": [[rnd(x * s - d), y] for x, y in part["points"]]}
                             for name, s, d in (("three_quarter", 0.9, 10), ("side", 0.8, 22))}
        elif pid == "bun" and part["ellipse"]["cx"] != 0:
            e = part["ellipse"]
            part["views"] = {"three_quarter": {"ellipse": dict(e, cx=-24)},
                             "side": {"ellipse": dict(e, cx=-40, cy=e["cy"] + 4)},
                             "back": {"ellipse": dict(e, cx=0, cy=-48)}}


# ---------------------------------------------------------------- headwear
def headwear(kind, color, detail=None):
    """Hats and helmets on the head joint: (behind parts, front parts)."""
    c = rgba(color)
    dark, light = shade(c, 0.75), shade(c, 1.3)
    behind, front = [], []
    if kind == "cap":
        front += [oval("cap_crown", "head", -2, -36, 43, 19, c, 1.6),
                  poly("cap_brim", "head", [(-43, -34), (28, -34), (52, -25), (48, -20), (-40, -23)], dark, 2, 1.3)]
    elif kind == "beanie":
        front += [oval("beanie_crown", "head", 0, -40, 43, 22, c, 1.6),
                  poly("beanie_band", "head", [(-44, -35), (44, -35), (43, -22), (-43, -22)], dark, 2, 1.4),
                  oval("beanie_pom", "head", 0, -64, 9, 8, light, 1.2)]
    elif kind == "hardhat":
        front += [oval("hat_dome", "head", 0, -38, 45, 25, c, 1.6),
                  poly("hat_rim", "head", [(-54, -31), (54, -31), (54, -24), (-54, -24)], dark, 0, 1.4),
                  poly("hat_ridge", "head", [(-4, -62), (4, -62), (4, -32), (-4, -32)], light, 0, 0.8)]
    elif kind == "chef":
        front += [poly("chef_puff", "head", bumpy(0, -76, 38, 24, 5, 0.18, 0.6, 32), c, 0),
                  poly("chef_band", "head", [(-34, -58), (34, -58), (36, -30), (-36, -30)], c, 0)]
    elif kind in ("police", "pilot"):
        front += [poly("cap_top", "head", [(-48, -60), (48, -60), (40, -36), (-40, -36)], c, 2, 1.5),
                  poly("cap_band", "head", [(-41, -40), (41, -40), (42, -30), (-42, -30)], dark, 0, 1.2),
                  poly("cap_peak", "head", [(-34, -31), (34, -31), (40, -24), (-40, -24)], "#2B3034", 0, 1),
                  oval("cap_badge", "head", 0, -48, 6, 6, detail or "#DDAF5B", 1)]
    elif kind == "sunhat":
        front += [oval("hat_brim", "head", 0, -32, 68, 11, c, 1.5),
                  oval("hat_crown", "head", 0, -46, 35, 19, c, 1.5),
                  poly("hat_ribbon", "head", [(-34, -40), (34, -40), (34, -33), (-34, -33)], detail or "#B76257", 0, 0.8)]
    elif kind == "beret":
        front += [oval("beret", "head", -6, -40, 47, 15, c, 1.5),
                  oval("beret_nub", "head", -2, -56, 4, 4, dark, 1)]
    elif kind == "flatcap":
        front += [poly("flatcap", "head", [(-44, -28), (-40, -44), (-14, -54), (20, -52), (44, -38), (54, -28)], c, 2, 1.4),
                  poly("flatcap_brim", "head", [(10, -31), (56, -31), (58, -25), (12, -25)], dark, 0, 1)]
    elif kind == "firehelmet":
        behind.append(poly("helmet_tail", "head", [(-48, -30), (48, -30), (58, -12), (-58, -12)], dark, 2, 1.4))
        front += [oval("helmet_dome", "head", 0, -40, 46, 27, c, 1.6),
                  poly("helmet_rim", "head", [(-52, -30), (52, -30), (52, -23), (-52, -23)], dark, 0, 1.2),
                  poly("helmet_shield", "head", [(-9, -62), (9, -62), (11, -40), (0, -34), (-11, -40)], detail or "#DDAF5B", 0, 1)]
    elif kind == "headscarf":
        front.append(poly("scarf_head", "head", [(-46, 16), (-48, -12), (-40, -36), (-20, -50), (0, -53), (20, -50), (40, -36), (48, -12), (46, 16), (38, 14), (36, -10), (24, -28), (0, -32), (-24, -28), (-36, -10), (-38, 14)], c))
        behind.append(poly("scarf_knot", "head", [(-14, 38), (14, 38), (20, 56), (0, 50), (-20, 56)], dark, 2, 1.2))
    elif kind == "wizard":
        front += [oval("hat_brim", "head", 0, -32, 62, 10, dark, 1.5),
                  poly("hat_cone", "head", [(-42, -36), (42, -36), (16, -96), (-2, -136), (-8, -100)], c, 2, 1.5),
                  oval("hat_star_1", "head", -10, -66, 4, 4, "#E8D58A", 0),
                  oval("hat_star_2", "head", 12, -50, 3, 3, "#E8D58A", 0)]
    elif kind == "knight":
        behind.append(oval("helmet_back", "head", 0, -4, 50, 52, c, 1.8))
        front += [poly("helmet_top", "head", [(-48, -4), (-46, -30), (-30, -48), (0, -54), (30, -48), (46, -30), (48, -4), (38, -8), (36, -24), (0, -30), (-36, -24), (-38, -8)], c, 2, 1.6),
                  poly("helmet_crest", "head", [(-4, -54), (4, -54), (5, -30), (-5, -30)], light, 0, 1),
                  poly("plume", "head", [(-2, -54), (10, -76), (32, -86), (22, -66), (8, -52)], detail or "#B76257", 2, 1.2)]
    elif kind == "tricorn":
        front += [poly("tricorn", "head", [(-60, -30), (-36, -60), (0, -50), (36, -60), (60, -30), (30, -36), (0, -30), (-30, -36)], c, 2, 1.5),
                  oval("tricorn_badge", "head", 0, -44, 6, 6, detail or "#E7E2D2", 1)]
    elif kind == "headband":
        front.append(poly("headband", "head", [(-43, -28), (43, -28), (43, -19), (-43, -19)], c, 0, 1))
    elif kind == "headphones":
        front += [poly("phones_band", "head", [(-46, -6), (-44, -34), (-26, -52), (0, -57), (26, -52), (44, -34), (46, -6), (40, -6), (38, -30), (22, -46), (0, -50), (-22, -46), (-38, -30), (-40, -6)], dark, 0, 1),
                  oval("phones_l", "head", -44, -2, 9, 14, c, 1.2),
                  oval("phones_r", "head", 44, -2, 9, 14, c, 1.2)]
    elif kind == "antennae":
        for side, sign in (("l", -1), ("r", 1)):
            front += [cap("antenna_" + side, "head", (sign * 14, -38), (sign * 28, -72), 2.2, dark, 0.8),
                      oval("antenna_tip_" + side, "head", sign * 28, -74, 6, 6, light, 1)]
    elif kind == "robot":
        front += [cap("antenna", "head", (0, -40), (0, -66), 2.4, METAL, 0.8),
                  oval("antenna_tip", "head", 0, -68, 6, 6, detail or "#D27A54", 1),
                  poly("bolt_l", "head", rounded_rect(-50, -10, -40, 10, 3), METAL, 0, 1),
                  poly("bolt_r", "head", rounded_rect(40, -10, 50, 10, 3), METAL, 0, 1)]
    else:
        raise KeyError(kind)
    return behind, front


def headwear_views(parts):
    for part in parts:
        pid = part["id"]
        if pid == "cap_brim":
            part["views"] = _brim(part["points"], 14)
        elif pid in ("cap_badge", "helmet_shield", "tricorn_badge"):
            part["views"] = _turn_views(part, HEAD_HALF, HIDDEN)
        elif pid in ("phones_l", "phones_r"):
            e = part["ellipse"]
            part["views"] = {"side": {"ellipse": dict(e, cx=-6, rx=10)} if pid == "phones_r" else HIDDEN,
                             "three_quarter": {"ellipse": dict(e, cx=rnd(e["cx"] * 0.8 - 4))}}


# ---------------------------------------------------------------- face details
def face_details(items, hair_color):
    """(parts right after `face`, parts before `mouth`, parts over the face)."""
    under, before_mouth, over = [], [], []
    hc = rgba(HAIR_COLORS.get(hair_color, hair_color))
    lens = [193, 216, 220, 36]
    for item in items:
        if item == "glasses":
            over += [oval("glasses_l", "head", -16, -3, 12, 12, lens, 1.4),
                     oval("glasses_r", "head", 16, -3, 12, 12, lens, 1.4),
                     poly("glasses_bridge", "head", [(-4, -5), (4, -5), (4, -2), (-4, -2)], INK, 0, 0)]
        elif item == "square_glasses":
            over += [poly("glasses_l", "head", rounded_rect(-28, -12, -5, 6, 3), lens, 0, 1.6),
                     poly("glasses_r", "head", rounded_rect(5, -12, 28, 6, 3), lens, 0, 1.6),
                     poly("glasses_bridge", "head", [(-5, -5), (5, -5), (5, -2), (-5, -2)], INK, 0, 0)]
        elif item == "sunglasses":
            over += [poly("shades_l", "head", rounded_rect(-30, -12, -4, 5, 5), "#2B3034", 0, 1),
                     poly("shades_r", "head", rounded_rect(4, -12, 30, 5, 5), "#2B3034", 0, 1),
                     poly("shades_bridge", "head", [(-5, -9), (5, -9), (5, -6), (-5, -6)], INK, 0, 0)]
        elif item == "goggles":
            over += [oval("goggle_l", "head", -15, -29, 11, 9, [177, 199, 202, 255], 1.6),
                     oval("goggle_r", "head", 15, -29, 11, 9, [177, 199, 202, 255], 1.6)]
        elif item == "mustache":
            before_mouth.append(poly("mustache", "head", [(-16, 11), (-6, 7), (0, 9), (6, 7), (16, 11), (13, 16), (4, 13), (0, 14), (-4, 13), (-13, 16)], hc, 2, 1))
        elif item in ("beard", "long_beard"):
            chin = [(-24, 64), (0, 92), (24, 64)] if item == "long_beard" else [(-22, 43), (0, 51), (22, 43)]
            before_mouth.append(poly("beard", "head", [(-41, -2), (-37, 24)] + chin + [(37, 24), (41, -2), (32, 10), (20, 30), (0, 35), (-20, 30), (-32, 10)], hc, 2, 1.2))
        elif item == "stubble":
            before_mouth.append(poly("stubble", "head", [(-38, 8), (-32, 28), (-16, 40), (0, 43), (16, 40), (32, 28), (38, 8), (28, 18), (14, 31), (0, 33), (-14, 31), (-28, 18)], shade(hc, 1.5)[:3] + [110], 2, 0))
        elif item == "freckles":
            for side, sign in (("l", -1), ("r", 1)):
                for k, (dx, dy) in enumerate(((20, 9), (26, 12), (24, 5))):
                    over.append(oval(f"freckle_{side}{k}", "head", sign * dx, dy, 1.5, 1.5, "#B98A6A", 0))
        elif item == "blush":
            over += [oval("blush_l", "head", -25, 10, 7, 4, [226, 150, 150, 120], 0),
                     oval("blush_r", "head", 25, 10, 7, 4, [226, 150, 150, 120], 0)]
        elif item == "mask":
            under.append(poly("mask", "head", [(-36, -16), (-6, -14), (0, -8), (6, -14), (36, -16), (38, 6), (6, 6), (0, 2), (-6, 6), (-38, 6)], "#2B3034", 2, 1))
        elif item == "eyepatch":
            over += [poly("eyepatch_strap", "head", [(-42, -26), (30, 2), (30, 6), (-42, -22)], INK, 0, 0),
                     oval("eyepatch", "head", 16, -3, 11, 11, "#2B3034", 1)]
        elif item == "earrings":
            over += [oval("earring_l", "head", -41, 10, 3, 3, "#DDAF5B", 0.8),
                     oval("earring_r", "head", 41, 10, 3, 3, "#DDAF5B", 0.8)]
        else:
            raise KeyError(item)
    return under, before_mouth, over


FACE_TURNED = ("glasses", "shades", "goggle_", "mustache", "beard", "stubble", "freckle", "blush",
               "mask", "eyepatch")


def face_views(parts):
    for part in parts:
        pid = part["id"]
        if pid.startswith(FACE_TURNED):
            part["views"] = _turn_views(part, HEAD_HALF, HIDDEN)
        elif pid == "earring_r":
            part["views"] = {"side": HIDDEN, "three_quarter": HIDDEN}
        elif pid == "earring_l":
            part["views"] = {"side": {"ellipse": dict(part["ellipse"], cx=-6)},
                             "three_quarter": {"ellipse": dict(part["ellipse"], cx=-28)}}


# ---------------------------------------------------------------- torso extras
def torso_extras(b: Body, items):
    """(parts behind the body, parts on the front of the torso)."""
    behind, front = [], []
    y0, w = b.shoulder_y, b.waist
    for item, c in items.items():
        if item == "tie":
            front.append(poly("tie", "torso", [(-5, y0 + 4), (0, y0 + 1), (5, y0 + 4), (8, y0 + 50), (0, y0 + 60), (-8, y0 + 50)], c, 0, 1.1))
        elif item == "scarf":
            front += [poly("scarf", "torso", [(-19, y0 - 4), (19, y0 - 4), (16, y0 + 10), (-17, y0 + 10)], c, 2, 1.2),
                      poly("scarf_tail", "torso", [(5, y0 + 6), (16, y0 + 9), (24, y0 + 40), (13, y0 + 43)], c, 2, 1)]
        elif item == "badge":
            front.append(poly("badge", "torso", [(-15, y0 + 18), (-7, y0 + 16), (-6, y0 + 26), (-11, y0 + 31), (-16, y0 + 26)], c, 0, 1))
        elif item == "name_tag":
            front.append(poly("name_tag", "torso", rounded_rect(5, y0 + 20, 19, y0 + 28, 1.5), c, 0, 0.8))
        elif item == "stethoscope":
            front += [poly("stethoscope", "torso", [(-12, y0 - 1), (-9, y0 - 1), (-6, y0 + 30), (0, y0 + 36), (6, y0 + 30), (9, y0 - 1), (12, y0 - 1), (9, y0 + 32), (0, y0 + 41), (-9, y0 + 32)], "#3A4A55", 0, 0),
                      oval("stethoscope_head", "torso", 0, y0 + 44, 4.5, 4.5, METAL, 1)]
        elif item == "necklace":
            front += [oval(f"bead_{k}", "torso", 14 * math.cos(math.radians(a)), y0 + 4 + 12 * math.sin(math.radians(a)), 2.4, 2.4, c, 0.6)
                      for k, a in enumerate(range(20, 170, 25))]
        elif item == "medal":
            front += [poly("medal_ribbon", "torso", [(-5, y0 + 1), (5, y0 + 1), (3, y0 + 30), (-3, y0 + 30)], "#5E91AE", 0, 0.8),
                      oval("medal", "torso", 0, y0 + 36, 7, 7, c, 1.1)]
        elif item == "hivis":
            for k in range(2):
                y = y0 + 34 + k * 26 * b.T
                front.append(poly(f"reflector_{k}", "torso", [(-(b.S + 3), y), (b.S + 3, y), (b.S + 3, y + 6), (-(b.S + 3), y + 6)], c, 0, 0))
        elif item == "vest":
            front += [poly("vest_l", "torso", [(-(b.S - 1), y0 + 4), (-5, y0 + 7), (-5, w - 2), (-(b.hip + 9), w - 4)], c, 2, 1.2),
                      poly("vest_r", "torso", [(5, y0 + 7), (b.S - 1, y0 + 4), (b.hip + 9, w - 4), (5, w - 2)], c, 2, 1.2)]
        elif item == "apron":
            front += [poly("apron", "torso", [(-15, y0 + 12), (15, y0 + 12), (16, w - 6), (b.hip + 10, w + 46 * b.L), (-(b.hip + 10), w + 46 * b.L), (-16, w - 6)], c, 0, 1.2),
                      poly("apron_pocket", "torso", [(-10, w + 8), (10, w + 8), (10, w + 24), (-10, w + 24)], shade(rgba(c), 0.85), 0, 1)]
        elif item == "belt":
            front += [poly("belt", "torso", [(-(b.hip + 9), w - 8), (b.hip + 9, w - 8), (b.hip + 9, w - 1), (-(b.hip + 9), w - 1)], c, 0, 1),
                      poly("buckle", "torso", rounded_rect(-5, w - 9, 5, w, 1.5), "#DDAF5B", 0, 0.8)]
        elif item == "backpack":
            behind.append(poly("backpack", "torso", rounded_rect(-(b.S + 9), y0 + 4, b.S + 9, w * 0.62, 8), c, 0, 1.4))
            front += [poly("pack_strap_l", "torso", [(-15, y0 - 1), (-9, y0 - 1), (-11, y0 + 44), (-17, y0 + 44)], shade(rgba(c), 0.75), 0, 0.8),
                      poly("pack_strap_r", "torso", [(9, y0 - 1), (15, y0 - 1), (17, y0 + 44), (11, y0 + 44)], shade(rgba(c), 0.75), 0, 0.8)]
        elif item == "mailbag":
            front += [poly("bag_strap", "torso", [(-25, y0), (-18, y0 - 3), (25, w * 0.55), (18, w * 0.55 + 4)], "#4A5459", 0, 0),
                      poly("mailbag", "torso", rounded_rect(8, w * 0.48, 44, w * 0.95, 4), c, 0, 1.4),
                      poly("mailbag_flap", "torso", [(8, w * 0.48), (44, w * 0.48), (40, w * 0.66), (12, w * 0.66)], shade(rgba(c), 0.8), 0, 1)]
        elif item == "cape":
            behind.append(poly("cape", "torso", [(-(b.S + 4), y0 - 4), (b.S + 4, y0 - 4), (b.S + 30, w + 100 * b.L), (0, w + 92 * b.L), (-(b.S + 30), w + 100 * b.L)], c, 2, 1.5))
        elif item == "lightning":
            front.append(poly("chest_bolt", "torso", [(-2, y0 + 16), (9, y0 + 16), (3, y0 + 30), (10, y0 + 30), (-6, y0 + 52), (-1, y0 + 35), (-8, y0 + 35)], c, 0, 1))
        elif item == "panel":
            front += [poly("chest_panel", "torso", rounded_rect(-14, y0 + 16, 14, y0 + 40, 3), shade(rgba(c), 0.8), 0, 1),
                      oval("panel_light_1", "torso", -7, y0 + 28, 3, 3, "#D27A54", 0),
                      oval("panel_light_2", "torso", 0, y0 + 28, 3, 3, "#DDAF5B", 0),
                      oval("panel_light_3", "torso", 7, y0 + 28, 3, 3, "#9CA95F", 0)]
        else:
            raise KeyError(item)
    return behind, front


TORSO_FRONT = {"tie", "badge", "name_tag", "stethoscope", "stethoscope_head", "medal", "medal_ribbon",
               "chest_bolt", "chest_panel", "panel_light_1", "panel_light_2", "panel_light_3", "emblem",
               "lapel_l", "lapel_r", "top_inner", "v_neck", "chest_pocket", "zip", "chest_patch",
               "pocket", "hood_cord_l", "hood_cord_r", "apron", "apron_pocket", "neckline", "buckle",
               "bib_button_l", "bib_button_r", "paint_1", "paint_2", "paint_3"}
TORSO_TURNED = {"scarf_tail", "pack_strap_l", "pack_strap_r", "vest_l", "vest_r", "bag_strap",
                "mailbag", "mailbag_flap", "bib", "strap_l", "strap_r"}


def torso_views(parts):
    for part in parts:
        pid = part["id"]
        if pid in TORSO_FRONT or pid.startswith(("button_", "bead_")):
            part["views"] = _turn_views(part, TORSO_HALF, HIDDEN)
        elif pid in TORSO_TURNED:
            part["views"] = _turn_views(part, TORSO_HALF)


# ---------------------------------------------------------------- hand items
def hand_item(kind):
    """Items held in a hand (drawn last). +y runs from the wrist toward the fingertips."""
    if kind == "cane":
        return [cap("cane_staff", "hand_r", (16, 4), (16, 176), 3, "#675B58", 1),
                poly("cane_handle", "hand_r", [(-5, -8), (13, -8), (22, -1), (20, 11), (14, 11), (14, 2), (10, -1), (-5, -1)], "#675B58", 0, 1)]
    if kind == "briefcase":
        return [poly("case_handle", "hand_r", [(-9, 2), (9, 2), (9, 12), (-9, 12)], INK, 0, 0),
                poly("briefcase", "hand_r", rounded_rect(-26, 10, 26, 48, 4), "#6B4A36", 0, 1.5),
                poly("case_clasp", "hand_r", [(-4, 10), (4, 10), (4, 18), (-4, 18)], "#DDAF5B", 0, 0.8)]
    if kind == "handbag":
        return [poly("bag_handle", "hand_l", [(-12, 0), (-8, -2), (-6, 14), (6, 14), (8, -2), (12, 0), (10, 18), (-10, 18)], INK, 0, 0),
                poly("handbag", "hand_l", [(-18, 14), (18, 14), (24, 44), (-24, 44)], "#9F7E9B", 2, 1.4)]
    if kind == "staff":
        return [cap("staff", "hand_r", (0, 190), (0, -150), 3.6, "#7A5C44"),
                oval("staff_orb", "hand_r", 0, -160, 12, 12, "#A8C6D8", 1.4)]
    if kind == "sword":
        return [poly("sword_guard", "hand_r", [(-16, 8), (16, 8), (16, 14), (-16, 14)], "#DDAF5B", 0, 1),
                poly("sword_blade", "hand_r", [(-4, 14), (4, 14), (4, 116), (0, 128), (-4, 116)], METAL, 0, 1.2),
                poly("sword_pommel", "hand_r", [(-3, -12), (3, -12), (3, 8), (-3, 8)], "#6B4A36", 0, 0.8)]
    if kind == "book":
        return [poly("book", "hand_l", rounded_rect(-32, -18, -4, 22, 2), "#B76257", 0, 1.4),
                poly("book_pages", "hand_l", [(-30, -16), (-28, -16), (-28, 20), (-30, 20)], PAPER, 0, 0)]
    if kind == "notepad":
        return [poly("notepad", "hand_l", rounded_rect(-30, -22, -4, 12, 2), PAPER, 0, 1.3),
                poly("notepad_spiral", "hand_l", [(-30, -24), (-4, -24), (-4, -19), (-30, -19)], METAL, 0, 0.6),
                poly("pencil", "hand_r", [(-2, -26), (2, -26), (2, 10), (0, 15), (-2, 10)], "#DDAF5B", 0, 0.8)]
    if kind == "broom":
        return [cap("broom_stick", "hand_r", (0, -70), (0, 150), 2.6, "#9A7656", 1),
                poly("broom_head", "hand_r", [(-6, 146), (6, 146), (20, 186), (-20, 186)], "#C9A55B", 0, 1.2)]
    if kind == "ladle":
        return [cap("ladle_handle", "hand_r", (0, -10), (0, 58), 2.2, METAL, 0.8),
                oval("ladle_bowl", "hand_r", 0, 64, 10, 7, METAL, 1.2)]
    if kind == "palette":
        return [oval("palette", "hand_l", -18, 4, 22, 16, "#D4B890", 1.3),
                oval("dab_1", "hand_l", -28, -2, 4, 3, "#B76257", 0),
                oval("dab_2", "hand_l", -18, -8, 4, 3, "#5E91AE", 0),
                oval("dab_3", "hand_l", -8, -4, 4, 3, "#DDAF5B", 0),
                cap("brush", "hand_r", (0, -30), (0, 16), 1.8, "#7A5C44", 0.6)]
    if kind == "flask":
        return [poly("flask", "hand_l", [(-6, -24), (6, -24), (6, -8), (18, 18), (-18, 18), (-6, -8)], [190, 214, 218, 200], 0, 1.3),
                poly("flask_liquid", "hand_l", [(-11, 4), (11, 4), (17, 16), (-17, 16)], "#9CA95F", 0, 0)]
    if kind == "pitchfork":
        return [cap("fork_handle", "hand_r", (0, -40), (0, 150), 2.6, "#9A7656", 1),
                poly("fork_head", "hand_r", [(-16, 146), (16, 146), (16, 152), (-16, 152)], METAL, 0, 0.8)] + [
                poly(f"tine_{k}", "hand_r", [(x - 1.6, 150), (x + 1.6, 150), (x + 1.6, 180), (x, 184), (x - 1.6, 180)], METAL, 0, 0.6)
                for k, x in enumerate((-14, 0, 14))]
    if kind == "ball":
        return [oval("ball", "hand_l", 0, 18, 15, 15, "#D27A54", 1.4),
                poly("ball_seam", "hand_l", [(-15, 17), (15, 17), (15, 19), (-15, 19)], INK, 0, 0)]
    if kind == "skateboard":
        return [poly("skateboard", "hand_r", rounded_rect(-9, -40, 9, 70, 8), "#5E91AE", 0, 1.4),
                oval("skate_wheel_1", "hand_r", 11, -26, 4, 4, PAPER, 1),
                oval("skate_wheel_2", "hand_r", 11, 56, 4, 4, PAPER, 1)]
    if kind == "wrench":
        return [poly("wrench", "hand_r", [(-3, -10), (3, -10), (3, 40), (9, 44), (9, 56), (4, 52), (-4, 52), (-9, 56), (-9, 44), (-3, 40)], METAL, 0, 1)]
    if kind == "flashlight":
        return [cap("flashlight", "hand_r", (0, -6), (0, 34), 4, "#3A4248", 1),
                oval("flashlight_lens", "hand_r", 0, 38, 5, 3, "#F6E4A8", 0.8)]
    if kind == "gadget":
        return [poly("gadget", "hand_r", rounded_rect(-6, -4, 6, 30, 3), "#7A8E9A", 0, 1),
                oval("gadget_tip", "hand_r", 0, 34, 5, 5, "#9CD3B6", 0.8)]
    if kind == "letter":
        return [poly("letter", "hand_l", rounded_rect(-34, -10, -4, 12, 1.5), PAPER, 0, 1.2),
                poly("letter_flap", "hand_l", [(-34, -10), (-4, -10), (-19, 2)], shade(PAPER, 0.9), 0, 0.8)]
    if kind == "phone":
        return [poly("phone", "hand_l", rounded_rect(-12, -22, 4, 6, 3), "#2B3034", 0, 1),
                poly("phone_screen", "hand_l", rounded_rect(-10, -19, 2, 2, 1.5), "#AECFD2", 0, 0)]
    if kind == "microphone":
        return [cap("mic_handle", "hand_r", (0, -6), (0, 22), 3.2, INK, 0.6),
                oval("mic_head", "hand_r", 0, -12, 7, 8, METAL, 1.1)]
    if kind == "teddy":
        return [oval("teddy_body", "hand_l", 0, 24, 13, 15, "#B08A64", 1.2),
                oval("teddy_head", "hand_l", 0, 2, 10, 10, "#B08A64", 1.2),
                oval("teddy_ear_l", "hand_l", -8, -6, 4, 4, "#8E6C4C", 0.8),
                oval("teddy_ear_r", "hand_l", 8, -6, 4, 4, "#8E6C4C", 0.8)]
    raise KeyError(kind)


# ---------------------------------------------------------------- catalogue
def C(**kw):
    defaults = dict(tall=1.0, hat=None, face=[], torso_items={}, hands=[], face_color=None, face_shape=None)
    return {**defaults, **kw}


CATALOG = [
    # Children
    C(slug="aava", name="Aava", category="family", role="Utelias koululainen", role_en="Curious schoolgirl",
      age="child", build="average", acting="lively", hair=("pigtails", "auburn"), face=["freckles"],
      top=("tshirt", "#D27A54", "short", {}), bottom=("skirt", "#3F5C7A"), torso_items={"backpack": "#DDAF5B"},
      tags=["koulu", "school"]),
    C(slug="eetu", name="Eetu", category="family", role="Vauhdikas pikkuveli", role_en="Energetic little brother",
      age="child", build="average", acting="lively", hair=("spiky", "blond"), hat=("cap", "#5E91AE"),
      top=("tshirt", "#9CA95F", "short", {"stripes": True}), bottom=("shorts", "#4A5A6A"), hands=["ball"],
      tags=["leikki", "play"]),
    C(slug="lumi", name="Lumi", category="family", role="Pieni ja ujo", role_en="Small and shy",
      age="child", build="round", tall=0.92, acting="nervous", hair=("top_bun", "black"), face=["blush"],
      top=("dress_top", "#C68AA8", "short", {}), bottom=("skirt", "#C68AA8"), hands=["teddy"],
      tags=["taapero", "toddler"]),
    C(slug="onni", name="Onni", category="family", role="Kirjatoukka", role_en="Bookworm",
      age="child", build="slim", acting="nervous", hair=("side_part", "brown"), face=["glasses"],
      top=("hoodie", "#6E8C6A", "long", {}), bottom=("trousers", "#4A5A6A"), hands=["book"],
      tags=["kirja", "book", "koulu", "school"]),
    # Teens
    C(slug="iida", name="Iida", category="family", role="Musiikkia rakastava nuori", role_en="Music-loving teen",
      age="teen", build="slim", acting="tired", hair=("long", "red"), hat=("headphones", "#B76257"),
      top=("hoodie", "#5B6B8A", "long", {}), bottom=("trousers", "#3F5C7A"), hands=["phone"],
      tags=["musiikki", "music"]),
    C(slug="rasmus", name="Rasmus", category="family", role="Rento rullalautailija", role_en="Laid-back skateboarder",
      age="teen", build="slim", acting="tired", hair=("fringe", "brown"), hat=("beanie", "#D27A54"),
      top=("tshirt", "#4E6E7E", "short", {}), bottom=("shorts", "#6B6B5E"), hands=["skateboard"],
      tags=["rullalauta", "skateboard"]),
    C(slug="nella", name="Nella", category="sport", role="Kilpajuoksija", role_en="Track runner",
      age="teen", build="slim", acting="lively", hair=("ponytail", "blond"), hat=("headband", "#B76257"),
      top=("sport", "#5E91AE", "short", {}), bottom=("shorts", "#2E3E52"),
      tags=["urheilu", "sport", "juoksu", "running"]),
    # Professions
    C(slug="helmi", name="Tohtori Helmi", category="profession", role="Rauhallinen lääkäri", role_en="Calm doctor",
      age="adult", build="slim", acting="calm", hair=("bun", "brown"), face=["glasses"],
      top=("labcoat", "#E7E9E4", "long", {}), bottom=("trousers", "#5B6B7A"),
      torso_items={"stethoscope": None, "name_tag": "#5E91AE"},
      tags=["lääkäri", "doctor", "sairaala", "hospital", "terveys", "health"]),
    C(slug="makela", name="Konstaapeli Mäkelä", category="profession", role="Tarkka poliisi", role_en="Precise police officer",
      age="adult", build="broad", acting="stoic", hair=("crop", "black"), hat=("police", "#2E3E52", "#DDAF5B"), face=["mustache"],
      top=("uniform", "#34465A", "long", {}), bottom=("trousers", "#2E3E52"),
      torso_items={"badge": "#DDAF5B", "belt": "#2B3034"}, hands=["flashlight"],
      tags=["poliisi", "police", "virkapuku", "uniform"]),
    C(slug="jere", name="Palomies Jere", category="profession", role="Rohkea palomies", role_en="Brave firefighter",
      age="adult", build="broad", tall=1.04, acting="proud", hair=("short", "brown"), hat=("firehelmet", "#B5473A", "#DDAF5B"), face=["stubble"],
      top=("jacket", "#9C8A62", "long", {}), bottom=("trousers", "#9C8A62"), torso_items={"hivis": "#E8D58A"},
      tags=["palomies", "firefighter", "pelastus", "rescue"]),
    C(slug="ville", name="Kokki Ville", category="profession", role="Innokas kokki", role_en="Enthusiastic chef",
      age="adult", build="round", acting="lively", hair=("short", "black"), hat=("chef", "#F2F0E8"), face=["mustache"],
      top=("jacket", "#EDEDE6", "long", {}), bottom=("trousers", "#3A3E44"), torso_items={"apron": "#D9D6CC"}, hands=["ladle"],
      tags=["kokki", "chef", "keittiö", "kitchen", "ravintola", "restaurant"]),
    C(slug="aino", name="Opettaja Aino", category="profession", role="Kärsivällinen opettaja", role_en="Patient teacher",
      age="adult", build="average", acting="calm", hair=("bob", "auburn"), face=["glasses", "earrings"],
      top=("cardigan", "#7E9A86", "long", {}), bottom=("long_skirt", "#5B4E66"), hands=["book"],
      tags=["opettaja", "teacher", "koulu", "school"]),
    C(slug="satu", name="Hoitaja Satu", category="profession", role="Reipas sairaanhoitaja", role_en="Brisk nurse",
      age="adult", build="average", acting="lively", hair=("ponytail", "black"),
      top=("scrubs", "#6FA3A0", "short", {}), bottom=("trousers", "#6FA3A0"), torso_items={"name_tag": PAPER},
      tags=["hoitaja", "nurse", "sairaala", "hospital", "terveys", "health"]),
    C(slug="pena", name="Rakentaja Pena", category="profession", role="Jykevä rakennusmies", role_en="Sturdy builder",
      age="adult", build="broad", tall=1.05, acting="proud", hair=("short", "brown"), hat=("hardhat", "#E0B44C"), face=["beard"],
      top=("tshirt", "#8E8E86", "short", {}), bottom=("trousers", "#4E6680"),
      torso_items={"vest": "#E3A04A", "hivis": "#EEEDE2"}, hands=["wrench"],
      tags=["rakentaja", "builder", "työmaa", "construction"]),
    C(slug="leena", name="Postinkantaja Leena", category="profession", role="Aina ajoissa", role_en="Always on time",
      age="adult", build="slim", acting="nervous", hair=("ponytail", "blond"), hat=("cap", "#3F5C7A"),
      top=("jacket", "#4E6E8E", "long", {}), bottom=("trousers", "#2E3E52"), torso_items={"mailbag": "#B08A64"}, hands=["letter"],
      tags=["posti", "mail", "kirje", "letter"]),
    C(slug="kaisa", name="Myyjä Kaisa", category="profession", role="Ystävällinen kauppias", role_en="Friendly shop clerk",
      age="adult", build="round", acting="calm", hair=("curly", "brown"),
      top=("shirt", "#D9C9A8", "half", {}), bottom=("trousers", "#4A4E56"), torso_items={"apron": "#4E7E6A", "name_tag": PAPER},
      tags=["myyjä", "clerk", "kauppa", "shop"]),
    C(slug="harri", name="Liikemies Harri", category="profession", role="Kiireinen liikemies", role_en="Busy businessman",
      age="adult", build="average", tall=1.04, acting="proud", hair=("side_part", "black"),
      top=("suit", "#5A6068", "long", {}), bottom=("trousers", "#5A6068"), torso_items={"tie": "#7A3F48"}, hands=["briefcase"],
      tags=["toimisto", "office", "liike", "business"]),
    C(slug="marjatta", name="Johtaja Marjatta", category="profession", role="Määrätietoinen johtaja", role_en="Decisive executive",
      age="adult", build="slim", acting="proud", hair=("bob", "grey"), face=["earrings"],
      top=("suit", "#3F4A66", "long", {}), bottom=("skirt", "#3F4A66"), torso_items={"necklace": "#DDAF5B"},
      tags=["toimisto", "office", "johtaja", "executive"]),
    C(slug="toivo", name="Maanviljelijä Toivo", category="profession", role="Vakaa maanviljelijä", role_en="Steady farmer",
      age="adult", build="broad", acting="calm", hair=("short", "grey"), hat=("sunhat", "#C9A55B", "#7A5C44"), face=["beard"],
      top=("overalls", "#4E6680", "short", {}), bottom=("trousers", "#4E6680"), hands=["pitchfork"],
      tags=["maatila", "farm", "maaseutu", "countryside"]),
    C(slug="ronja", name="Taiteilija Ronja", category="profession", role="Haaveileva taiteilija", role_en="Dreamy artist",
      age="adult", build="slim", acting="lively", hair=("wavy", "pink"), hat=("beret", "#3A3E5A"),
      top=("smock", "#D8D0BC", "half", {}), bottom=("trousers", "#4A5A6A"), hands=["palette"],
      tags=["taide", "art", "maalari", "painter"]),
    C(slug="kalle", name="Muusikko Kalle", category="profession", role="Viileä laulaja", role_en="Cool singer",
      age="adult", build="slim", tall=1.04, acting="stoic", hair=("long", "black"), face=["sunglasses", "stubble"],
      top=("jacket", "#2E3034", "long", {}), bottom=("trousers", "#3A4A62"), hands=["microphone"],
      tags=["musiikki", "music", "laulaja", "singer"]),
    C(slug="veera", name="Tutkija Veera", category="profession", role="Hajamielinen tutkija", role_en="Absent-minded scientist",
      age="adult", build="average", acting="nervous", hair=("curly", "auburn"), face=["goggles"],
      top=("labcoat", "#E7E9E4", "long", {}), bottom=("trousers", "#5E6E5E"), hands=["flask"],
      tags=["tiede", "science", "laboratorio", "lab"]),
    C(slug="antti", name="Lentäjä Antti", category="profession", role="Varma lentäjä", role_en="Confident pilot",
      age="adult", build="average", tall=1.05, acting="proud", hair=("crop", "blond"), hat=("pilot", "#2E3E52", "#DDAF5B"), face=["sunglasses"],
      top=("uniform", "#2E3E52", "long", {}), bottom=("trousers", "#2E3E52"), torso_items={"tie": "#2B3034", "badge": "#DDAF5B"},
      tags=["lentäjä", "pilot", "lento", "flight"]),
    C(slug="jari", name="Kuljettaja Jari", category="profession", role="Väsynyt bussikuski", role_en="Tired bus driver",
      age="adult", build="round", acting="tired", hair=("bald", "brown"), hat=("cap", "#4E6E7E"), face=["mustache"],
      top=("shirt", "#9AB0BC", "short", {}), bottom=("trousers", "#3A4A5A"), torso_items={"vest": "#4E6E7E"},
      tags=["kuljettaja", "driver", "bussi", "bus"]),
    C(slug="riitta", name="Siivooja Riitta", category="profession", role="Tehokas siivooja", role_en="Efficient cleaner",
      age="adult", build="average", acting="lively", hair=("short", "brown"), hat=("headscarf", "#6E8CA8"),
      top=("jumpsuit", "#7E8E9E", "half", {}), bottom=("trousers", "#7E8E9E"), hands=["broom"],
      tags=["siivous", "cleaning", "huolto", "maintenance"]),
    C(slug="emil", name="Toimittaja Emil", category="profession", role="Utelias toimittaja", role_en="Inquisitive reporter",
      age="adult", build="slim", acting="nervous", hair=("wavy", "brown"), face=["square_glasses"],
      top=("sweater", "#A86E5A", "long", {}), bottom=("trousers", "#4A5462"), hands=["notepad"],
      tags=["toimittaja", "reporter", "uutiset", "news"]),
    C(slug="mira", name="Urheilija Mira", category="sport", role="Sinnikäs urheilija", role_en="Determined athlete",
      age="adult", build="average", acting="lively", hair=("ponytail", "black"), hat=("headband", "#9CA95F"),
      top=("sport", "#D27A54", "short", {"stripes": True}), bottom=("shorts", "#2E3E52"), torso_items={"medal": "#DDAF5B"},
      tags=["urheilu", "sport", "valmentaja", "coach"]),
    C(slug="mikko", name="Isä Mikko", category="family", role="Leppoisa isä", role_en="Easy-going dad",
      age="adult", build="round", acting="tired", hair=("short", "brown"), face=["beard"],
      top=("sweater", "#6E7E9A", "long", {}), bottom=("trousers", "#3F5C7A"),
      tags=["isä", "dad", "koti", "home"]),
    C(slug="sanna", name="Äiti Sanna", category="family", role="Järjestelmällinen äiti", role_en="Organised mum",
      age="adult", build="average", acting="calm", hair=("long", "brown"),
      top=("cardigan", "#B98A6A", "long", {}), bottom=("trousers", "#4A5A6A"), hands=["phone"],
      tags=["äiti", "mum", "koti", "home"]),
    C(slug="tuuli", name="Tuuli", category="family", role="Iloinen naapuri", role_en="Cheerful neighbour",
      age="adult", build="broad", acting="lively", hair=("afro", "black"), face=["earrings"],
      top=("tshirt", "#DDAF5B", "short", {}), bottom=("trousers", "#4E6E7E"),
      tags=["naapuri", "neighbour", "arki", "everyday"]),
    # Elders
    C(slug="erkki", name="Ukki Erkki", category="family", role="Tarinoita kertova ukki", role_en="Storytelling grandpa",
      age="elder", build="slim", acting="calm", hair=("bald", "white"), face=["glasses", "mustache"],
      top=("cardigan", "#8A7A62", "long", {}), bottom=("trousers", "#5A5A54"), hands=["cane"],
      tags=["isoisä", "grandpa"]),
    C(slug="hilkka", name="Mummo Hilkka", category="family", role="Napakka mummo", role_en="Brisk grandma",
      age="elder", build="round", tall=0.95, acting="proud", hair=("curly", "white"), face=["glasses"],
      top=("dress_top", "#7E6A8E", "half", {}), bottom=("long_skirt", "#7E6A8E"), torso_items={"necklace": "#E7E2D2"}, hands=["handbag"],
      tags=["isoäiti", "grandma"]),
    C(slug="aatu", name="Vaari Aatu", category="family", role="Harvasanainen vaari", role_en="Taciturn grandad",
      age="elder", build="average", acting="stoic", hair=("short", "grey"), hat=("flatcap", "#6B6458"), face=["stubble"],
      top=("coat", "#5A5E52", "long", {}), bottom=("trousers", "#44463E"), torso_items={"scarf": "#8E4A3A"},
      tags=["isoisä", "grandpa", "talvi", "winter"]),
    # Fantasy and science fiction
    C(slug="tahtisilma", name="Velho Tähtisilmä", category="fantasy", role="Unelias velho", role_en="Sleepy wizard",
      age="elder", build="slim", tall=1.04, acting="stoic", hair=("long", "white"), hat=("wizard", "#4A4E7A"), face=["long_beard"],
      top=("robe", "#4A4E7A", "long", {}), bottom=("trousers", "#3A3E5A"), hands=["staff"],
      tags=["velho", "wizard", "taikuus", "magic"]),
    C(slug="rautio", name="Ritari Rautio", category="fantasy", role="Ylpeä ritari", role_en="Proud knight",
      age="adult", build="broad", tall=1.03, acting="proud", hair=("crop", "brown"), hat=("knight", "#AAB2B6", "#B76257"),
      top=("tunic", "#5E7EA0", "long", {"emblem": "#DDAF5B"}), bottom=("trousers", "#8A9296"), torso_items={"belt": "#6B4A36"}, hands=["sword"],
      tags=["ritari", "knight", "linna", "castle"]),
    C(slug="ankkuri", name="Kapteeni Ankkuri", category="fantasy", role="Äänekäs merikapteeni", role_en="Loud sea captain",
      age="adult", build="round", acting="proud", hair=("long", "black"), hat=("tricorn", "#2E3034", "#E7E2D2"), face=["eyepatch", "beard"],
      top=("coat", "#7A2F34", "long", {}), bottom=("trousers", "#3A3226"), torso_items={"belt": "#3A3226"},
      tags=["merirosvo", "pirate", "laiva", "ship"]),
    C(slug="salama", name="Salama", category="fantasy", role="Aloitteleva supersankari", role_en="Rookie superhero",
      age="adult", build="broad", acting="proud", hair=("short", "black"), face=["mask"],
      top=("cape_suit", "#3F5C9A", "long", {}), bottom=("trousers", "#3F5C9A"),
      torso_items={"cape": "#B5473A", "lightning": "#E8C34A", "belt": "#E8C34A"},
      tags=["sankari", "hero", "supersankari", "superhero"]),
    C(slug="ruuvi", name="Robotti Ruuvi", category="scifi", role="Avulias robotti", role_en="Helpful robot",
      age="adult", build="broad", acting="calm", hair=("bald", "#AAB2B6"), hat=("robot", METAL, "#D27A54"),
      face_color=[214, 222, 226, 255], face_shape="square",
      top=("jumpsuit", "#9AA6AE", "long", {}), bottom=("trousers", "#7A868E"), torso_items={"panel": "#9AA6AE"},
      tags=["robotti", "robot", "kone", "machine"]),
    C(slug="kuu", name="Avaruusolio Kuu", category="scifi", role="Ihmettelevä avaruusolio", role_en="Wondering alien",
      age="teen", build="slim", acting="lively", hair=("bald", "#9CBF8A"), hat=("antennae", "#8FB47E"),
      face_color=[214, 236, 204, 255],
      top=("jumpsuit", "#6E6E9A", "long", {}), bottom=("trousers", "#6E6E9A"), hands=["gadget"],
      tags=["avaruus", "space", "olio", "alien"]),
]

EXPRESSIONS = {
    "happy": [("brow_l", "happy"), ("brow_r", "happy"), ("mouth", "smile")],
    "sad": [("brow_l", "worried"), ("brow_r", "worried"), ("mouth", "frown")],
    "surprised": [("brow_l", "raised"), ("brow_r", "raised"), ("mouth", "o")],
    "angry": [("brow_l", "angry"), ("brow_r", "angry"), ("mouth", "closed")],
}
# Headwear that covers the whole head from behind, so the hair need not.
COVERING_HATS = {"knight", "wizard", "chef", "headscarf", "firehelmet", "hardhat", "police", "pilot", "beanie"}


def build_rig(spec, index):
    b = Body(spec["age"], spec["build"], spec["tall"])
    kind, color, sleeve, extra = spec["top"]
    top_behind, garment, top_details = tops(b, kind, color, extra)
    torso_behind, torso_front = torso_extras(b, spec["torso_items"])
    hair_behind, hair_front = hair_parts(*spec["hair"])
    hat_behind, hat_front = headwear(*spec["hat"]) if spec["hat"] else ([], [])
    under, before_mouth, over = face_details(spec["face"], spec["hair"][1])
    face = _face()
    if spec["face_color"]:
        face[0]["color"] = list(spec["face_color"])
    if spec["face_shape"] == "square":
        face[0] = poly("face", "head", rounded_rect(-41, -41, 41, 41, 14, 4), face[0]["color"], 0, 2.2)
    face = face[:1] + under + face[1:-1] + before_mouth + face[-1:]
    hands = [part for item in spec["hands"] for part in hand_item(item)]
    parts = (torso_behind + top_behind + b.legs() + bottoms(b, *spec["bottom"]) + b.spine()
             + garment + top_details + torso_front + b.arms() + sleeves(b, sleeve, color)
             + hat_behind + hair_behind + face + over + hair_front + hat_front + hands)
    covered = bool(spec["hat"]) and spec["hat"][0] in COVERING_HATS
    hair_views(parts, cover_back=not covered, nape=44.0 if spec["hair"][0] in FULL_BACK else 24.0)
    headwear_views(parts)
    face_views(parts)
    torso_views(parts)
    rig = {
        "version": 1, "id": spec["slug"] + "_stick_library",
        "joints": b.joints(), "parts": parts,
        "visemes": {"part": "mouth", "map": {name: name for name in VISEME_NAMES}},
        "expressions": {emotion: [{"part": p, "variant": v} for p, v in entries]
                        for emotion, entries in EXPRESSIONS.items()},
        "hands": {side: {"part": "hand_" + side[0], "shapes": {n: n for n in HAND_SHAPES}}
                  for side in ("left", "right")},
        "eyes": {"pupils": ["pupil_l", "pupil_r"], "travel": 3.4,
                 "blink": {"parts": ["eye_l", "eye_r", "pupil_l", "pupil_r"], "variant": "blink",
                           "interval": round(3.1 + (index % 9) * 0.13, 2), "duration": 0.12}},
    }
    return {"asset": "characters/" + spec["slug"] + "-stick-library", "default_pose": "idle",
            "rig": rig, "acting": spec["acting"]}


def _cross(a, b, c):
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def _on_segment(a, b, p):
    return (_cross(a, b, p) == 0 and min(a[0], b[0]) <= p[0] <= max(a[0], b[0])
            and min(a[1], b[1]) <= p[1] <= max(a[1], b[1]))


def _intersects(a, b, c, d):
    def opposite(x, y):
        return (x < 0 < y) or (x > 0 > y)
    return ((opposite(_cross(a, b, c), _cross(a, b, d)) and opposite(_cross(c, d, a), _cross(c, d, b)))
            or _on_segment(a, b, c) or _on_segment(a, b, d) or _on_segment(c, d, a) or _on_segment(c, d, b))


def simple_polygon(points):
    """Port of motion-rig's `simple_polygon`: no repeated, retraced or crossing edges."""
    n = len(points)
    for i in range(n):
        a, b, c = points[i], points[(i + 1) % n], points[(i + 2) % n]
        if a == b:
            return False
        if _cross(a, b, c) == 0 and (a[0] - b[0]) * (c[0] - b[0]) + (a[1] - b[1]) * (c[1] - b[1]) > 0:
            return False
        for j in range(i + 1, n):
            if j == i + 1 or (i == 0 and j == n - 1):
                continue
            if _intersects(a, b, points[j], points[(j + 1) % n]):
                return False
    return True


def round_corners(points, samples):
    """Port of motion-rig's `round_corners` (quadratic corner rounding)."""
    if samples == 0:
        return points
    n, out = len(points), []
    for i in range(n):
        prev, p, nxt = points[i - 1], points[i], points[(i + 1) % n]
        a = ((prev[0] + p[0]) / 2, (prev[1] + p[1]) / 2)
        b = ((p[0] + nxt[0]) / 2, (p[1] + nxt[1]) / 2)
        for k in range(samples):
            t = k / samples
            u = 1 - t
            out.append([u * u * a[0] + 2 * u * t * p[0] + t * t * b[0],
                        u * u * a[1] + 2 * u * t * p[1] + t * t * b[1]])
    return out


def check_rig(identifier, rig):
    """Structural checks mirroring motion-rig's definition rules (polygon simplicity,
    rounding, point budgets); the native validation also runs in the Rust tests."""
    joints = [j["id"] for j in rig["joints"]]
    assert tuple(joints) == JOINT_IDS, identifier
    parts, expanded = {}, 0
    for part in rig["parts"]:
        pid = part["id"]
        assert pid not in parts, (identifier, pid)
        parts[pid] = part
        assert part["joint"] in joints, (identifier, pid)
        assert len(part.get("variants", {})) <= 16
        assert set(part.get("views", {})) <= {"three_quarter", "side", "back"}, (identifier, pid)
        base = None
        for name, shape in [("base", part), *part.get("variants", {}).items(), *part.get("views", {}).items()]:
            assert not ("ellipse" in shape and "points" in shape), (identifier, pid, name)
            smooth = shape.get("smooth", part.get("smooth", 0))
            assert smooth == 0 or 2 <= smooth <= 16, (identifier, pid, name, "smooth")
            if "ellipse" not in shape and "points" not in shape:
                control = base
            elif "ellipse" in shape:
                e = shape["ellipse"]
                assert all(math.isfinite(v) for v in e.values()) and e["rx"] > 0 and e["ry"] > 0, (identifier, pid)
                control = [[e["cx"] + e["rx"] * math.cos(2 * math.pi * i / 48),
                            e["cy"] + e["ry"] * math.sin(2 * math.pi * i / 48)] for i in range(48)]
            else:
                control = shape["points"]
                assert 3 <= len(control) <= 64, (identifier, pid, name, len(control))
                assert simple_polygon(control), (identifier, pid, name, "not simple")
                area = sum(a[0] * c[1] - c[0] * a[1] for a, c in zip(control, control[1:] + control[:1]))
                assert abs(area) > 1e-6, (identifier, pid, name)
            if base is None:
                base = control
            assert len(control) * max(smooth, 1) <= 512, (identifier, pid, name)
            if smooth:
                assert simple_polygon(round_corners(control, smooth)), (identifier, pid, name, "rounded not simple")
            expanded += len(control) * max(smooth, 1)
    assert len(parts) <= 256, (identifier, len(parts))
    assert expanded <= 16_384, (identifier, expanded)
    for entries in rig["expressions"].values():
        for entry in entries:
            assert entry["variant"] in parts[entry["part"]]["variants"]


def world_transforms(joints):
    out = {}
    for j in joints:
        tx, ty, rot = j.get("x", 0), j.get("y", 0), j.get("rotation", 0)
        if j["parent"] is None:
            out[j["id"]] = (0.0, 0.0, rot)
        else:
            px, py, prot = out[j["parent"]]
            c, s = math.cos(prot), math.sin(prot)
            out[j["id"]] = (px + tx * c - ty * s, py + tx * s + ty * c, prot + rot)
    return out


def measure(rig):
    """Rest-pose bounds of the front view and head-top-to-sole height."""
    world = world_transforms(rig["joints"])
    xs, ys = [], []
    for part in rig["parts"]:
        if part.get("color", [0, 0, 0, 255])[3] == 0:
            continue
        jx, jy, rot = world[part["joint"]]
        c, s = math.cos(rot), math.sin(rot)
        pad = part.get("outline", {}).get("width", 0) + 1
        if "ellipse" in part:
            e = part["ellipse"]
            local = [(e["cx"] + dx * e["rx"], e["cy"] + dy * e["ry"]) for dx, dy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        else:
            local = part["points"]
        for x, y in local:
            wx, wy = jx + x * c - y * s, jy + x * s + y * c
            xs += [wx - pad, wx + pad]
            ys += [wy - pad, wy + pad]
    head_top = world["head"][1] - 42
    sole = max(world["foot_l"][1], world["foot_r"][1]) + 10
    x0, y0 = math.floor(min(xs)), math.floor(min(ys))
    return ({"x": x0, "y": y0, "width": math.ceil(max(xs)) - x0, "height": math.ceil(max(ys)) - y0},
            round(sole - head_top, 1))


def project_bytes(name, identifier, declaration):
    key = lambda at, **v: dict(at=at, easing="smoothstep", **v)  # noqa: E731
    shot = {
        "id": "hahmo", "duration": 5, "events": [],
        "composition": {
            "background_color": "#f4f1e9",
            "camera": [{"at": 0, "x": 0, "y": 0, "zoom": 1}],
            "layers": {identifier: {"z": 0, "visible": True}},
            "actor_tracks": {identifier: [key(0, x=0.0, y=-20, scale_x=1.8, scale_y=1.8)]},
            "joint_tracks": {identifier: {"head": [key(0), key(1.5, rotation=-0.05), key(5)]}},
        },
    }
    source = {"project": {"title": name, "width": 768, "height": 1024, "fps": 24},
              "characters": {identifier: declaration}, "scenes": [shot], "rig_clips": clips()}
    document = {"format_version": 2,
                "source": json.dumps(source, ensure_ascii=False, separators=(",", ":")),
                "assets": [], "art": {"layers": []}}
    return (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def manifest_bytes(asset_id, name, path, data, rig, info, tags):
    bounds, height = measure(rig)
    info = dict(info, height=height)
    unique = list(dict.fromkeys(tags))
    assert len(unique) <= 32, asset_id
    m = {
        "schema_version": 1, "id": asset_id, "kind": "character", "name": name,
        "bounds": bounds, "depth": "mid", "tags": unique, "license": LICENSE,
        "geometry": {"type": "rig_project", "path": path, "sha256": hashlib.sha256(data).hexdigest()},
        "character": info,
    }
    return (json.dumps(m, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


AGE_TAGS = {"child": ["lapsi", "child"], "teen": ["nuori", "teen"], "adult": ["aikuinen", "adult"],
            "elder": ["vanhus", "elder"]}
EXISTING = {  # the six stick-studio characters already in characters/*.kilsat
    "NOUTAJA": ("adult", "slim", "Friendly, laconic reaper", ["viikatemies", "reaper", "huumori", "humour"]),
    "VEIKKO": ("adult", "average", "Relatable commuter", ["työmatka", "commuter", "arki", "everyday"]),
    "PASMA": ("adult", "average", "Pompous boss", ["pomo", "boss", "toimisto", "office"]),
    "KIPINA": ("adult", "average", "Curious inventor", ["keksijä", "inventor", "korjaaja", "fixer"]),
    "KIIRE": ("adult", "slim", "Courier always on the move", ["lähetti", "courier", "kiire", "hurry"]),
    "SISU": ("elder", "slim", "Resourceful, calm grandmother", ["mummo", "grandma"]),
}


def build():
    """Return {path relative to the pack: bytes} for every generated file."""
    files = {}
    for index, spec in enumerate(CATALOG):
        identifier = spec["slug"].upper()
        assert spec["acting"] in ACTING, spec["slug"]
        declaration = build_rig(spec, index)
        check_rig(identifier, declaration["rig"])
        path = f"cast/{spec['slug']}.kilsat"
        data = project_bytes(spec["name"], identifier, declaration)
        files[path] = data
        info = {"id": identifier, "role": f"{spec['role']} / {spec['role_en']}", "age": spec["age"],
                "build": spec["build"], "acting": spec["acting"]}
        tags = spec["tags"] + AGE_TAGS[spec["age"]] + [spec["category"], spec["build"], spec["acting"], "tikku_ukko", "stick"]
        asset_id = f"character.{spec['category']}.{spec['slug']}_v1"
        files[f"library/characters/{asset_id}.json"] = manifest_bytes(
            asset_id, spec["name"], path, data, declaration["rig"], info, tags)
    for cast in STICK_CAST:
        identifier = cast["id"]
        path = f"characters/{identifier.lower()}.kilsat"
        data = (PACK / path).read_bytes()
        declaration = json.loads(json.loads(data)["source"])["characters"][identifier]
        age, build_, role_en, tags = EXISTING[identifier]
        info = {"id": identifier, "role": f"{cast['role']} / {role_en}", "age": age, "build": build_,
                "acting": declaration["acting"]}
        tags = tags + AGE_TAGS[age] + ["stick_studio", build_, declaration["acting"], "tikku_ukko", "stick"]
        asset_id = f"character.stick_studio.{identifier.lower()}_v1"
        files[f"library/characters/{asset_id}.json"] = manifest_bytes(
            asset_id, cast["name"], path, data, declaration["rig"], info, tags)
    return files


def main(argv):
    files = build()
    stale = [p for d, pattern in ((PACK / "cast", "*.kilsat"), (OUT_DIR, "character.*.json")) if d.exists()
             for p in sorted(d.glob(pattern)) if str(p.relative_to(PACK)) not in files]
    if "--check" in argv:
        bad = [p for p, d in files.items() if not (PACK / p).exists() or (PACK / p).read_bytes() != d]
        bad += [str(p.relative_to(PACK)) for p in stale]
        if bad:
            print("out of date: " + ", ".join(sorted(bad)), file=sys.stderr)
            return 1
        print(f"up to date: {len(files)} files")
        return 0
    for path in stale:
        path.unlink()
    for relative, data in files.items():
        target = PACK / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists() or target.read_bytes() != data:
            target.write_bytes(data)
    print(json.dumps({"characters": len(CATALOG) + len(STICK_CAST), "new": len(CATALOG), "files": len(files)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
