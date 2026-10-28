#!/usr/bin/env python3
"""Generate the prop manifests of the Kilsat Studio asset library.

Writes `examples/stick-studio/library/props/<id>.json` (manifest v1, ORK-LIB-01):

* migrated props: every existing `examples/stick-studio/props/*.kilsat` project as
  `{"type": "rig_project", "path", "sha256"}` with bounds measured from its geometry;
* original art props drawn here from `motion_art::Content` rectangles, ellipses and
  strokes (no filled polygons), coloured through palette slots.

Deterministic: Python 3 standard library only, no network, sorted inputs, rounded
numbers, fixed key order, `indent=2` and a trailing newline. Re-running produces
identical bytes. Only stale `prop.*.json` files in the props directory are removed.

Usage (from the repository root):
    python3 scripts/generate_library_props.py           # write manifests
    python3 scripts/generate_library_props.py --check   # verify files are up to date

Scale of the art props: 1920x1080 canvas pixels at actor scale 1. The stick-studio
characters measure 379-422 px from head top to sole at scale 1, with the hip
142 px and the knee ~72 px above the sole (rest hand ~181 px, shoulder ~292 px).
Seats sit 72-86 px above the floor, tables ~118 px, kitchen counters ~145 px and
doors 440 px.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKAGE = ROOT / "examples" / "stick-studio"
OUT_DIR = PACKAGE / "library" / "props"
RIG_DIR = PACKAGE / "props"

SCHEMA_VERSION = 1
LICENSE = {
    "license": "MIT",
    "author": "Kilsat Studio",
    "origin": "original",
    "redistribution": "allowed",
}
ID_RE = re.compile(r"^prop\.[a-z0-9]+(?:_[a-z0-9]+)*\.[a-z0-9]+(?:_[a-z0-9]+)*_v[1-9][0-9]*$")
TAG_RE = re.compile(r"^[a-z0-9_äö]+$")
LAYER_RE = re.compile(r"^[a-z0-9_]+$")
VARIANT_RE = re.compile(r"^[a-z0-9_]+$")
LINE_LIMIT = 8.0  # outlines stay thinner than the 9 px stick limb
EPS = 1e-6


def num(v: float) -> float | int:
    """Round to 3 decimals; whole numbers serialise as integers."""
    r = round(float(v), 3)
    if r == 0:
        return 0
    return int(r) if r == int(r) else r


def floor3(v: float) -> float:
    return math.floor(v * 1000 + 1e-9) / 1000


def ceil3(v: float) -> float:
    return math.ceil(v * 1000 - 1e-9) / 1000


# ---------------------------------------------------------------------------
# Palette (restrained tones from docs/stick-studio-art-direction.md).
# ---------------------------------------------------------------------------
COLORS: dict[str, tuple[int, int, int] | tuple[int, int, int, int]] = {
    "line": (36, 40, 45),
    "paper": (249, 247, 239),
    "white": (234, 232, 224),
    "wood": (178, 140, 100),
    "wood_dark": (128, 94, 66),
    "wood_light": (212, 184, 144),
    "fabric": (126, 146, 150),
    "fabric_dark": (96, 114, 120),
    "cushion": (196, 180, 152),
    "metal": (170, 178, 182),
    "metal_dark": (106, 114, 120),
    "glass": (190, 214, 218),
    "glass_clear": (190, 214, 218, 150),
    "screen": (58, 66, 74),
    "screen_glow": (174, 207, 210),
    "accent": (221, 175, 91),
    "accent_red": (183, 98, 87),
    "blue": (94, 145, 174),
    "leaf": (118, 150, 106),
    "leaf_dark": (78, 108, 84),
    "pot": (176, 118, 90),
    "stone": (156, 156, 150),
    "stone_dark": (106, 106, 102),
    "bark": (112, 88, 68),
    "board": (66, 90, 80),
    "chalk": (232, 232, 220),
    "snow": (240, 243, 245),
    "light": (246, 228, 168),
    "fire": (222, 140, 84),
    "rug": (172, 124, 104),
    "rug_pattern": (226, 208, 172),
    "coffee": (111, 76, 55),
}
DUSK = (52, 60, 92)


def evening(palette: dict) -> dict:
    """Derived evening palette: surfaces shift toward dusk blue; lamps stay warm."""
    out = {}
    for slot, c in palette.items():
        if slot == "line":
            continue
        if slot in ("light", "fire"):
            out[slot] = {"r": 250, "g": 212, "b": 138, "a": c["a"]} if slot == "light" else dict(c)
            continue
        out[slot] = {
            "r": round(c["r"] * 0.62 + DUSK[0] * 0.38),
            "g": round(c["g"] * 0.62 + DUSK[1] * 0.38),
            "b": round(c["b"] * 0.62 + DUSK[2] * 0.38),
            "a": c["a"],
        }
    return out


def color(rgb) -> dict:
    r, g, b = rgb[:3]
    a = rgb[3] if len(rgb) == 4 else 255
    return {"r": r, "g": g, "b": b, "a": a}


def pt(x, y) -> dict:
    return {"x": num(x), "y": num(y)}


def rect(x, y, w, h) -> dict:
    return {"x": num(x), "y": num(y), "width": num(w), "height": num(h)}


# ---------------------------------------------------------------------------
# Art builder
# ---------------------------------------------------------------------------
class Art:
    """Collects art layers for one prop; every layer paints from a palette slot."""

    def __init__(self, w: float, h: float, colors: dict | None = None):
        self.w = w
        self.h = h
        self.colors = dict(COLORS)
        if colors:
            self.colors.update(colors)
        self.layers: list[dict] = []
        self.used: list[str] = []
        self.anchors: list[dict] = []
        self.F = h - 1.5  # bottom edge of an outlined (3 px) shape that touches the floor

    def _slot(self, slot: str) -> dict:
        if slot not in self.colors:
            raise KeyError(f"unknown palette slot {slot}")
        if slot not in self.used:
            self.used.append(slot)
        return color(self.colors[slot])

    def _add(self, lid: str, slot: str, content: dict, depth: str | None = None):
        layer = {"id": lid}
        if depth:
            layer["depth"] = depth
        layer["paint"] = slot
        layer["content"] = content
        self.layers.append(layer)

    def line(self, lid, pts, width=3.0, slot="line"):
        c = self._slot(slot)
        self._add(lid, slot, {"type": "stroke", "points": [pt(x, y) for x, y in pts], "color": c, "width": num(width)})

    def box(self, lid, x, y, w, h, slot, lw=3.0):
        """Filled rectangle with a dark outline stroke (lw=0: no outline)."""
        c = self._slot(slot)
        self._add(lid, slot, {"type": "rectangle", "rect": rect(x, y, w, h), "color": c})
        if lw > 0:
            self.line(lid + "_line", [(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)], lw)

    def oval(self, lid, cx, cy, rx, ry, slot, lw=3.0, n=32):
        c = self._slot(slot)
        self._add(lid, slot, {"type": "ellipse", "rect": rect(cx - rx, cy - ry, 2 * rx, 2 * ry), "color": c})
        if lw > 0:
            pts = [(cx + rx * math.cos(2 * math.pi * i / n), cy + ry * math.sin(2 * math.pi * i / n)) for i in range(n)]
            pts.append(pts[0])
            self.line(lid + "_line", pts, lw)

    # anchors ---------------------------------------------------------------
    def ground(self, aid="floor"):
        self.anchors.append({"type": "ground", "id": aid, "at": pt(0, self.h), "width": num(self.w)})

    def seat(self, x, y, aid="seat"):
        self.anchors.append({"type": "seat", "id": aid, "at": pt(x, y)})

    def grip(self, x, y, aid="grip"):
        self.anchors.append({"type": "grip", "id": aid, "at": pt(x, y)})

    def stand(self, x, y, aid="stand"):
        self.anchors.append({"type": "stand", "id": aid, "at": pt(x, y)})

    def screen(self, x, y, w, h, aid="screen"):
        self.anchors.append({"type": "screen", "id": aid, "rect": rect(x, y, w, h)})

    def doorway(self, x, y, w, h, aid="doorway"):
        self.anchors.append({"type": "doorway", "id": aid, "rect": rect(x, y, w, h)})

    def palette(self) -> dict:
        return {s: color(self.colors[s]) for s in sorted(self.used)}


ART: list[dict] = []


def prop(pid, name, w, h, tags, depth="mid", colors=None, variants=()):
    """Decorator registering an art prop drawn by the decorated function."""

    def wrap(fn):
        ART.append({"id": pid, "name": name, "w": w, "h": h, "tags": list(tags), "depth": depth,
                    "colors": colors, "variants": list(variants), "draw": fn})
        return fn

    return wrap


def legs(b: Art, xs, top, width=10, slot="wood_dark"):
    for i, x in enumerate(xs):
        b.box(f"leg_{i}", x, top, width, b.F - top, slot)


# ---------------------------------------------------------------------------
# Furniture
# ---------------------------------------------------------------------------
@prop("prop.furniture.chair_wood_v1", "Puutuoli", 90, 200, ["tuoli", "chair", "puu", "wood", "huonekalu", "furniture"],
      variants=["evening", ("painted", {"wood": (174, 196, 196), "wood_dark": (110, 136, 140)})])
def _(b):
    legs(b, [10, 72], 130, 8)
    b.box("post_left", 10, 6, 8, 120, "wood_dark")
    b.box("post_right", 72, 6, 8, 120, "wood_dark")
    b.box("top_rail", 6, 6, 78, 18, "wood")
    for i, x in enumerate((26, 42, 58)):
        b.box(f"slat_{i}", x, 24, 6, 80, "wood_light", 2)
    b.box("seat_board", 4, 122, 82, 12, "wood")
    b.seat(45, 122)
    b.ground()


@prop("prop.furniture.office_chair_v1", "Toimistotuoli", 110, 210, ["tuoli", "chair", "toimisto", "office", "huonekalu", "furniture"],
      variants=["evening"])
def _(b):
    b.box("back", 20, 6, 70, 92, "fabric_dark")
    b.box("back_pad", 28, 14, 54, 76, "fabric", 2)
    b.box("back_post", 51, 98, 8, 28, "metal_dark")
    b.box("lift", 51, 140, 8, 40, "metal_dark")
    b.box("seat_cushion", 8, 124, 94, 18, "fabric")
    b.line("arm_left", [(14, 124), (14, 104), (32, 104)], 5)
    b.line("arm_right", [(96, 124), (96, 104), (78, 104)], 5)
    b.line("base", [(16, 194), (55, 180), (94, 194)], 6)
    b.oval("wheel_left", 16, 201, 7.5, 7.5, "metal_dark")
    b.oval("wheel_right", 94, 201, 7.5, 7.5, "metal_dark")
    b.seat(55, 124)
    b.ground()


@prop("prop.furniture.stool_v1", "Jakkara", 70, 80, ["jakkara", "stool", "tuoli", "chair", "huonekalu", "furniture"])
def _(b):
    b.line("leg_left", [(18, 14), (8, 77)], 6, "wood_dark")
    b.line("leg_right", [(52, 14), (62, 77)], 6, "wood_dark")
    b.line("rung", [(13, 50), (57, 50)], 5, "wood_dark")
    b.box("seat_top", 4, 4, 62, 12, "wood")
    b.seat(35, 4)
    b.ground()


@prop("prop.furniture.sofa_v1", "Sohva", 340, 160, ["sohva", "sofa", "olohuone", "living_room", "huonekalu", "furniture"],
      variants=["evening", ("mustard", {"fabric": (204, 172, 112), "fabric_dark": (160, 130, 82)})])
def _(b):
    b.box("foot_left", 24, 140, 12, b.F - 140, "wood_dark")
    b.box("foot_right", 304, 140, 12, b.F - 140, "wood_dark")
    b.box("back", 22, 12, 296, 80, "fabric")
    b.box("frame", 8, 96, 324, 44, "fabric_dark")
    b.box("cushion_left", 44, 78, 124, 26, "cushion")
    b.box("cushion_right", 172, 78, 124, 26, "cushion")
    b.box("arm_left", 4, 52, 40, 88, "fabric_dark")
    b.box("arm_right", 296, 52, 40, 88, "fabric_dark")
    b.seat(106, 78, "seat_left")
    b.seat(234, 78, "seat_right")
    b.ground()


@prop("prop.furniture.armchair_v1", "Nojatuoli", 170, 160, ["nojatuoli", "armchair", "tuoli", "chair", "olohuone", "living_room", "furniture"],
      variants=["evening", ("green", {"fabric": (132, 156, 128), "fabric_dark": (96, 120, 96)})])
def _(b):
    b.box("foot_left", 22, 140, 12, b.F - 140, "wood_dark")
    b.box("foot_right", 136, 140, 12, b.F - 140, "wood_dark")
    b.box("back", 24, 8, 122, 84, "fabric")
    b.box("frame", 8, 96, 154, 44, "fabric_dark")
    b.box("cushion", 42, 78, 86, 26, "cushion")
    b.box("arm_left", 4, 50, 38, 90, "fabric_dark")
    b.box("arm_right", 128, 50, 38, 90, "fabric_dark")
    b.seat(85, 78)
    b.ground()


@prop("prop.furniture.bench_v1", "Penkki", 240, 80, ["penkki", "bench", "puu", "wood", "huonekalu", "furniture"])
def _(b):
    legs(b, [22, 204], 16, 14)
    b.box("stretcher", 36, 50, 168, 8, "wood_dark", 2)
    b.box("plank", 4, 4, 232, 14, "wood")
    b.seat(120, 4)
    b.ground()


@prop("prop.furniture.table_dining_v1", "Ruokapöytä", 280, 118, ["pöytä", "table", "ruokapöytä", "dining_table", "keittiö", "kitchen", "furniture"],
      variants=["evening"])
def _(b):
    legs(b, [18, 250], 28, 12)
    b.box("apron", 14, 16, 252, 14, "wood_dark")
    b.box("top", 4, 4, 272, 14, "wood")
    b.stand(140, 4, "surface")
    b.ground()


@prop("prop.furniture.desk_v1", "Kirjoituspöytä", 250, 118, ["pöytä", "desk", "kirjoituspöytä", "toimisto", "office", "furniture"],
      variants=["evening"])
def _(b):
    b.box("leg_left", 14, 18, 12, b.F - 18, "wood_dark")
    b.box("drawers", 150, 18, 88, b.F - 18, "wood")
    for i in range(3):
        y = 26 + i * 30
        b.box(f"drawer_{i}", 158, y, 72, 24, "wood_light", 2)
        b.line(f"handle_{i}", [(184, y + 12), (204, y + 12)], 4)
    b.box("top", 4, 4, 242, 14, "wood")
    b.stand(80, 4, "surface")
    b.ground()


@prop("prop.furniture.coffee_table_v1", "Sohvapöytä", 190, 66, ["pöytä", "table", "sohvapöytä", "coffee_table", "olohuone", "living_room", "furniture"])
def _(b):
    legs(b, [14, 166], 16, 10)
    b.box("shelf", 20, 42, 150, 8, "wood_dark", 2)
    b.box("top", 4, 4, 182, 12, "wood")
    b.stand(95, 4, "surface")
    b.ground()


@prop("prop.furniture.bed_v1", "Sänky", 340, 140, ["sänky", "bed", "makuuhuone", "bedroom", "furniture"],
      variants=["evening"])
def _(b):
    b.box("headboard", 4, 6, 28, b.F - 6, "wood")
    b.box("footboard", 308, 56, 28, b.F - 56, "wood")
    b.box("frame", 32, 92, 276, 20, "wood_dark")
    b.box("mattress", 32, 66, 276, 26, "white")
    b.oval("pillow", 70, 60, 32, 12, "paper")
    b.box("blanket", 108, 58, 200, 38, "blue")
    b.line("blanket_fold", [(118, 70), (300, 70)], 3)
    b.seat(200, 58)
    b.ground()


@prop("prop.furniture.bookshelf_v1", "Kirjahylly", 150, 310, ["kirjahylly", "bookshelf", "hylly", "shelf", "kirja", "book", "furniture"],
      variants=["evening"])
def _(b):
    b.box("case", 4, 4, 142, b.F - 4, "wood")
    b.box("back", 14, 14, 122, 270, "wood_dark", 0)
    books = ["blue", "accent_red", "paper", "accent", "leaf", "blue", "cushion"]
    for s in range(4):
        y0 = 14 + s * 70
        b.box(f"shelf_{s}", 10, y0 + 62, 130, 8, "wood", 2)
        x = 16
        for i in range(5):
            bw = 14 + ((s * 5 + i) * 7) % 10
            bh = 40 + ((s * 3 + i) * 11) % 18
            b.box(f"book_{s}_{i}", x, y0 + 62 - bh, bw, bh, books[(s * 2 + i) % len(books)], 2)
            x += bw + 4
    b.box("plinth", 4, 298, 142, b.F - 298, "wood_dark")
    b.ground()


@prop("prop.furniture.cabinet_v1", "Kaappi", 130, 150, ["kaappi", "cabinet", "säilytys", "storage", "furniture"])
def _(b):
    b.box("body", 6, 12, 118, b.F - 22, "wood")
    b.box("door_left", 14, 20, 49, 104, "wood_light", 2)
    b.box("door_right", 67, 20, 49, 104, "wood_light", 2)
    b.oval("knob_left", 56, 72, 3.5, 3.5, "metal_dark", 2)
    b.oval("knob_right", 74, 72, 3.5, 3.5, "metal_dark", 2)
    b.box("top", 2, 4, 126, 10, "wood_dark")
    b.box("plinth", 10, 138, 110, b.F - 138, "wood_dark")
    b.stand(65, 4, "surface")
    b.ground()


@prop("prop.furniture.counter_v1", "Keittiötaso", 280, 145, ["työtaso", "counter", "keittiö", "kitchen", "kaappi", "cabinet", "furniture"])
def _(b):
    b.box("body", 8, 18, 264, 112, "white")
    for i in range(3):
        x = 16 + i * 86
        b.box(f"door_{i}", x, 26, 78, 98, "paper", 2)
        b.line(f"handle_{i}", [(x + 39 - 10, 36), (x + 39 + 10, 36)], 4)
    b.box("kick", 14, 130, 252, b.F - 130, "metal_dark")
    b.box("top", 2, 4, 276, 14, "wood_light")
    b.stand(140, 4, "surface")
    b.ground()


# ---------------------------------------------------------------------------
# Kitchen
# ---------------------------------------------------------------------------
@prop("prop.kitchen.fridge_v1", "Jääkaappi", 120, 280, ["jääkaappi", "fridge", "keittiö", "kitchen", "kodinkone", "appliance"])
def _(b):
    b.box("body", 6, 4, 108, 264, "white")
    b.line("split", [(6, 92), (114, 92)], 3)
    b.line("handle_top", [(98, 30), (98, 76)], 5, "metal_dark")
    b.line("handle_bottom", [(98, 106), (98, 170)], 5, "metal_dark")
    b.box("note", 24, 120, 26, 30, "paper", 2)
    b.box("foot_left", 14, 268, 14, b.F - 268, "metal_dark")
    b.box("foot_right", 92, 268, 14, b.F - 268, "metal_dark")
    b.ground()


@prop("prop.kitchen.stove_v1", "Liesi", 120, 145, ["liesi", "stove", "uuni", "oven", "keittiö", "kitchen", "appliance"])
def _(b):
    b.box("body", 6, 14, 108, 120, "white")
    b.box("cooktop", 4, 4, 112, 10, "screen")
    for i, x in enumerate((24, 48, 72, 96)):
        b.oval(f"knob_{i}", x, 26, 5, 5, "metal_dark", 2)
    b.box("oven_door", 16, 40, 88, 82, "white")
    b.box("oven_window", 26, 56, 68, 46, "screen", 2)
    b.line("oven_handle", [(28, 48), (92, 48)], 4, "metal_dark")
    b.box("kick", 10, 134, 100, b.F - 134, "metal_dark")
    b.ground()


@prop("prop.kitchen.sink_v1", "Tiskiallas", 160, 145, ["tiskiallas", "sink", "hana", "tap", "keittiö", "kitchen"])
def _(b):
    b.line("faucet", [(80, 40), (80, 8), (104, 8), (104, 20)], 6, "metal_dark")
    b.box("body", 8, 52, 144, 80, "white")
    b.box("door_left", 16, 60, 62, 64, "paper", 2)
    b.box("door_right", 82, 60, 62, 64, "paper", 2)
    b.box("top", 2, 40, 156, 14, "wood_light")
    b.box("basin", 40, 42, 80, 6, "metal", 2)
    b.box("kick", 14, 132, 132, b.F - 132, "metal_dark")
    b.ground()


@prop("prop.kitchen.pot_v1", "Kattila", 120, 76, ["kattila", "pot", "keittiö", "kitchen", "ruoka", "cooking"])
def _(b):
    b.line("handle_left", [(18, 30), (4, 30)], 6, "metal_dark")
    b.line("handle_right", [(102, 30), (116, 30)], 6, "metal_dark")
    b.box("body", 18, 22, 84, 52.5, "metal")
    b.oval("lid", 60, 18, 44, 6, "metal")
    b.oval("lid_knob", 60, 9, 8, 4, "metal_dark", 2)
    b.grip(10, 30)
    b.ground("base")


@prop("prop.kitchen.toaster_v1", "Leivänpaahdin", 96, 72, ["leivänpaahdin", "toaster", "keittiö", "kitchen", "aamiainen", "breakfast"])
def _(b):
    b.oval("toast_left", 34, 14, 12, 10, "wood_light", 2)
    b.oval("toast_right", 62, 14, 12, 10, "wood_light", 2)
    b.box("body", 8, 16, 80, 50, "metal")
    b.line("slot_left", [(22, 18), (46, 18)], 4)
    b.line("slot_right", [(50, 18), (74, 18)], 4)
    b.line("lever", [(88, 32), (88, 46)], 5, "metal_dark")
    b.box("foot", 14, 66, 68, b.F - 66, "metal_dark")
    b.ground("base")


@prop("prop.kitchen.coffee_maker_v1", "Kahvinkeitin", 90, 136, ["kahvinkeitin", "coffee_maker", "kahvi", "coffee", "keittiö", "kitchen"])
def _(b):
    b.box("tower", 52, 6, 32, b.F - 6, "screen")
    b.box("head", 10, 6, 74, 26, "screen")
    b.box("jug", 14, 66, 40, 56, "glass_clear")
    b.box("brew", 16, 92, 36, 28, "coffee", 0)
    b.line("jug_handle", [(54, 76), (62, 76), (62, 110), (54, 110)], 4)
    b.box("plate", 6, 122, 78, b.F - 122, "metal_dark")
    b.oval("lamp", 68, 50, 4, 4, "accent_red", 2)
    b.ground("base")


@prop("prop.kitchen.dish_cabinet_v1", "Astiakaappi", 160, 300, ["astiakaappi", "dish_cabinet", "kaappi", "cabinet", "keittiö", "kitchen"],
      variants=["evening"])
def _(b):
    b.box("upper", 8, 4, 144, 140, "wood")
    for i, x in enumerate((16, 82)):
        b.box(f"glass_{i}", x, 12, 62, 124, "glass", 2)
        for j, y in enumerate((44, 96)):
            b.oval(f"plate_{i}_{j}", x + 31, y, 20, 20, "paper", 2)
        b.line(f"shelf_{i}", [(x, 70), (x + 62, 70)], 3)
    b.box("counter", 4, 144, 152, 12, "wood_dark")
    b.box("lower", 8, 156, 144, 128, "wood")
    b.box("door_left", 16, 164, 62, 112, "wood_light", 2)
    b.box("door_right", 82, 164, 62, 112, "wood_light", 2)
    b.oval("knob_left", 70, 220, 3.5, 3.5, "metal_dark", 2)
    b.oval("knob_right", 90, 220, 3.5, 3.5, "metal_dark", 2)
    b.box("plinth", 12, 284, 136, b.F - 284, "wood_dark")
    b.ground()


@prop("prop.kitchen.kettle_v1", "Vedenkeitin", 84, 84, ["vedenkeitin", "kettle", "keittiö", "kitchen", "tee", "tea"])
def _(b):
    b.line("handle", [(56, 20), (72, 20), (78, 34), (78, 56), (64, 60)], 6)
    b.line("spout", [(14, 44), (6, 30)], 6, "metal_dark")
    b.box("body", 14, 20, 50, 52, "white")
    b.box("lid", 22, 12, 34, 8, "metal", 2)
    b.box("base", 10, 72, 58, b.F - 72, "metal_dark")
    b.grip(78, 40)
    b.ground("base")


@prop("prop.kitchen.microwave_v1", "Mikroaaltouuni", 140, 84, ["mikro", "microwave", "keittiö", "kitchen", "kodinkone", "appliance"])
def _(b):
    b.box("body", 4, 4, 132, 72, "white")
    b.box("window", 14, 14, 82, 52, "screen", 2)
    b.box("panel", 104, 14, 24, 52, "metal", 2)
    b.box("display", 108, 18, 16, 8, "screen_glow", 2)
    for i in range(3):
        b.oval(f"button_{i}", 116, 36 + i * 9, 3, 3, "metal_dark", 1.5)
    b.box("foot_left", 12, 76, 14, b.F - 76, "metal_dark")
    b.box("foot_right", 114, 76, 14, b.F - 76, "metal_dark")
    b.screen(108, 18, 16, 8, "display")
    b.ground("base")


@prop("prop.kitchen.frying_pan_v1", "Paistinpannu", 160, 40, ["paistinpannu", "frying_pan", "pannu", "pan", "keittiö", "kitchen"])
def _(b):
    b.line("handle", [(98, 18), (154, 10)], 7, "wood_dark")
    b.box("pan", 6, 12, 92, 18, "metal_dark")
    b.oval("rim", 52, 12, 46, 5, "screen", 2)
    b.grip(140, 12)


@prop("prop.kitchen.coffee_pot_v1", "Kahvipannu", 110, 110, ["kahvipannu", "coffee_pot", "pannu", "kahvi", "coffee", "suomi", "finland"])
def _(b):
    b.line("spout", [(30, 70), (14, 46), (6, 40)], 7, "metal_dark")
    b.line("handle", [(78, 40), (100, 46), (102, 80), (80, 90)], 6, "line")
    b.box("body", 26, 34, 56, 70, "metal")
    b.box("band", 26, 52, 56, 6, "metal_dark", 0)
    b.oval("lid", 54, 32, 24, 6, "metal")
    b.oval("lid_knob", 54, 22, 6, 4, "wood_dark", 2)
    b.box("bottom", 22, 104, 64, b.F - 104, "metal_dark")
    b.grip(101, 62)
    b.ground("base")


# ---------------------------------------------------------------------------
# Electronics
# ---------------------------------------------------------------------------
@prop("prop.electronics.tv_v1", "Televisio", 220, 150, ["televisio", "tv", "näyttö", "screen", "olohuone", "living_room", "electronics"],
      variants=["evening", ("on", {"screen": (174, 207, 210)})])
def _(b):
    b.line("stand_left", [(60, 130), (44, 147)], 6, "metal_dark")
    b.line("stand_right", [(160, 130), (176, 147)], 6, "metal_dark")
    b.box("body", 4, 4, 212, 128, "metal_dark")
    b.box("panel", 12, 12, 196, 108, "screen", 2)
    b.oval("led", 110, 126, 2.5, 2.5, "accent_red", 0)
    b.screen(12, 12, 196, 108)
    b.ground("base")


@prop("prop.electronics.monitor_v1", "Tietokoneen näyttö", 120, 112, ["näyttö", "monitor", "tietokone", "computer", "toimisto", "office", "electronics"],
      variants=[("on", {"screen": (174, 207, 210)})])
def _(b):
    b.box("neck", 54, 76, 12, 22, "metal_dark")
    b.box("foot", 28, 98, 64, b.F - 98, "metal_dark")
    b.box("body", 4, 4, 112, 72, "screen")
    b.box("panel", 10, 10, 100, 60, "blue", 2)
    b.box("window", 18, 18, 48, 30, "paper", 2)
    b.screen(10, 10, 100, 60)
    b.ground("base")


@prop("prop.electronics.radio_v1", "Radio", 120, 84, ["radio", "musiikki", "music", "olohuone", "living_room", "electronics"])
def _(b):
    b.line("handle", [(24, 24), (30, 6), (90, 6), (96, 24)], 5)
    b.box("body", 6, 24, 108, 54, "wood")
    b.oval("speaker", 40, 51, 20, 20, "cushion")
    b.box("dial", 70, 34, 34, 14, "light", 2)
    b.oval("knob_0", 78, 62, 5, 5, "metal_dark", 2)
    b.oval("knob_1", 96, 62, 5, 5, "metal_dark", 2)
    b.box("foot", 14, 78, 92, b.F - 78, "wood_dark")
    b.grip(60, 6)
    b.ground("base")


# ---------------------------------------------------------------------------
# Decor and living room
# ---------------------------------------------------------------------------
def leafy(b: Art, cx, cy, s):
    """Overlapping leaf ovals of a house plant around (cx, cy) at scale s."""
    spots = [(-0.6, 0.2, "leaf_dark"), (0.6, 0.1, "leaf_dark"), (-0.4, -0.6, "leaf"), (0.45, -0.55, "leaf"),
             (0, -1.0, "leaf_dark"), (0, 0.1, "leaf")]
    for i, (dx, dy, slot) in enumerate(spots):
        b.oval(f"leaf_{i}", cx + dx * s, cy + dy * s, 0.55 * s, 0.38 * s, slot)


@prop("prop.decor.plant_pot_v1", "Ruukkukasvi", 90, 150, ["kasvi", "plant", "ruukku", "pot", "viherkasvi", "decor"])
def _(b):
    b.line("stem", [(45, 96), (45, 50)], 4, "leaf_dark")
    leafy(b, 45, 58, 34)
    b.box("pot", 24, 98, 42, b.F - 98, "pot")
    b.box("rim", 18, 90, 54, 12, "pot")
    b.ground()


@prop("prop.decor.plant_large_near_v1", "Iso viherkasvi (etuala)", 190, 330, ["kasvi", "plant", "ruukku", "pot", "etuala", "foreground", "decor"],
      depth="near", variants=["evening"])
def _(b):
    for i, (x1, y1) in enumerate(((95, 120), (60, 150), (130, 140))):
        b.line(f"stem_{i}", [(95, 252), (x1, y1)], 5, "leaf_dark")
    leafy(b, 95, 120, 78)
    b.oval("leaf_low_left", 40, 200, 36, 18, "leaf")
    b.oval("leaf_low_right", 150, 196, 36, 18, "leaf_dark")
    b.box("pot", 52, 262, 86, b.F - 262, "pot")
    b.box("rim", 44, 248, 102, 16, "pot")
    b.ground()


@prop("prop.decor.floor_lamp_v1", "Jalkalamppu", 100, 330, ["lamppu", "lamp", "jalkalamppu", "floor_lamp", "valo", "light", "decor"],
      variants=["evening"])
def _(b):
    b.line("pole", [(50, 70), (50, 318)], 5, "metal_dark")
    b.oval("glow", 50, 76, 30, 10, "light", 0)
    b.box("shade", 18, 6, 64, 66, "light")
    b.line("shade_seam", [(18, 22), (82, 22)], 2)
    b.oval("base", 50, 322, 32, 6.5, "metal_dark")
    b.ground()


@prop("prop.decor.picture_frame_v1", "Taulu", 110, 86, ["taulu", "picture", "kehys", "frame", "seinä", "wall", "decor"])
def _(b):
    b.box("frame", 4, 4, 102, 78, "wood_dark")
    b.box("mat", 12, 12, 86, 62, "paper", 2)
    b.box("sky", 20, 20, 70, 46, "glass", 0)
    b.oval("sun", 72, 32, 7, 7, "accent", 0)
    b.oval("hill", 44, 56, 24, 9, "leaf", 0)
    b.box("ground_band", 20, 56, 70, 10, "leaf_dark", 0)


@prop("prop.decor.rug_v1", "Matto", 320, 44, ["matto", "rug", "lattia", "floor", "olohuone", "living_room", "decor"],
      variants=[("blue", {"rug": (120, 146, 164), "rug_pattern": (214, 222, 220)})])
def _(b):
    b.oval("rug", 160, 22, 156, 20.5, "rug")
    b.oval("pattern", 160, 22, 118, 13, "rug_pattern", 2)
    b.oval("center", 160, 22, 70, 7, "rug", 2)
    b.stand(160, 22)
    b.ground()


@prop("prop.decor.wall_clock_v1", "Seinäkello", 84, 84, ["kello", "clock", "seinäkello", "wall_clock", "aika", "time", "decor"])
def _(b):
    b.oval("rim", 42, 42, 38, 38, "wood_dark")
    b.oval("face", 42, 42, 31, 31, "paper", 2)
    for i in range(12):
        a = 2 * math.pi * i / 12
        r0 = 24 if i % 3 else 21
        b.line(f"tick_{i}", [(42 + r0 * math.cos(a), 42 + r0 * math.sin(a)), (42 + 28 * math.cos(a), 42 + 28 * math.sin(a))], 2)
    b.line("hour_hand", [(42, 42), (42, 26)], 4)
    b.line("minute_hand", [(42, 42), (60, 50)], 3)
    b.oval("pin", 42, 42, 3, 3, "accent_red", 0)


@prop("prop.decor.coat_rack_v1", "Naulakko", 100, 330, ["naulakko", "coat_rack", "eteinen", "hallway", "takki", "coat", "decor"])
def _(b):
    b.line("pole", [(50, 20), (50, 318)], 7, "wood_dark")
    b.line("hook_left", [(50, 34), (24, 22), (20, 30)], 4, "wood_dark")
    b.line("hook_right", [(50, 34), (76, 22), (80, 30)], 4, "wood_dark")
    b.line("scarf", [(26, 30), (22, 70), (30, 112)], 7, "accent_red")
    b.oval("knob", 50, 16, 7, 7, "wood_dark", 2)
    b.line("foot_left", [(50, 312), (12, 327)], 6, "wood_dark")
    b.line("foot_right", [(50, 312), (88, 327)], 6, "wood_dark")
    b.ground()


@prop("prop.decor.trash_bin_v1", "Roskakori", 64, 78, ["roskakori", "trash_bin", "roska", "trash", "toimisto", "office", "decor"])
def _(b):
    b.box("bin", 10, 10, 44, b.F - 10, "metal")
    b.line("band", [(10, 24), (54, 24)], 2)
    b.box("rim", 6, 4, 52, 8, "metal_dark")
    b.ground()


@prop("prop.decor.vase_flowers_v1", "Kukkamaljakko", 70, 130, ["maljakko", "vase", "kukka", "flower", "olohuone", "living_room", "decor"])
def _(b):
    for i, (x, y) in enumerate(((20, 24), (35, 12), (50, 26))):
        b.line(f"stem_{i}", [(35, 76), (x, y)], 3, "leaf_dark")
    for i, (x, y, slot) in enumerate(((20, 24, "accent"), (35, 12, "accent_red"), (50, 26, "paper"))):
        b.oval(f"flower_{i}", x, y, 9, 9, slot, 2)
    b.oval("vase_body", 35, 108.5, 22, 20, "blue")
    b.box("vase_neck", 25, 72, 20, 18, "blue")
    b.ground("base")


@prop("prop.decor.curtains_v1", "Verhot", 240, 300, ["verho", "curtain", "ikkuna", "window", "olohuone", "living_room", "decor"],
      variants=["evening"])
def _(b):
    b.line("rod", [(4, 10), (236, 10)], 5, "metal_dark")
    for side, x0 in (("left", 10), ("right", 160)):
        b.box(f"panel_{side}", x0, 14, 70, 282, "fabric")
        for k in range(1, 4):
            b.line(f"fold_{side}_{k}", [(x0 + k * 17.5, 22), (x0 + k * 17.5, 288)], 2, "fabric_dark")
        b.line(f"tie_{side}", [(x0, 180), (x0 + 70, 180)], 6, "accent")


@prop("prop.decor.table_lamp_v1", "Pöytävalaisin", 80, 116, ["lamppu", "lamp", "pöytävalaisin", "table_lamp", "valo", "light", "decor"],
      variants=["evening"])
def _(b):
    b.line("stem", [(40, 50), (40, 100)], 5, "metal_dark")
    b.box("shade", 12, 6, 56, 46, "light")
    b.oval("base", 40, 106, 26, 8.5, "wood_dark")
    b.ground("base")


@prop("prop.decor.ryijy_v1", "Ryijy", 170, 240, ["ryijy", "rya_rug", "seinävaate", "tapestry", "suomi", "finland", "decor"],
      colors={"rug": (150, 96, 82), "rug_pattern": (224, 196, 140)})
def _(b):
    b.line("rod", [(4, 8), (166, 8)], 5, "wood_dark")
    b.box("weave", 14, 12, 142, 200, "rug")
    for i, (cx, cy, r) in enumerate(((85, 62, 34), (85, 150, 34))):
        b.oval(f"motif_{i}", cx, cy, r, r * 0.8, "rug_pattern", 2)
        b.oval(f"motif_core_{i}", cx, cy, r * 0.45, r * 0.35, "blue", 2)
    b.line("stripe", [(14, 106), (156, 106)], 6, "rug_pattern")
    for i in range(8):
        x = 22 + i * 18
        b.line(f"fringe_{i}", [(x, 212), (x, 236)], 3, "rug")


@prop("prop.decor.cushion_v1", "Tyyny", 80, 50, ["tyyny", "cushion", "sohva", "sofa", "olohuone", "living_room", "decor"],
      variants=[("blue", {"cushion": (148, 172, 186)})])
def _(b):
    b.oval("pillow", 40, 26, 36, 22.5, "cushion")
    b.oval("button", 40, 26, 3, 3, "fabric_dark", 1.5)
    b.ground("base")


# ---------------------------------------------------------------------------
# Living room furniture extras
# ---------------------------------------------------------------------------
@prop("prop.furniture.tv_stand_v1", "TV-taso", 250, 76, ["tv_taso", "tv_stand", "taso", "sideboard", "olohuone", "living_room", "furniture"])
def _(b):
    b.box("body", 6, 4, 238, 60, "wood")
    b.box("drawer_left", 14, 12, 106, 44, "wood_light", 2)
    b.box("open_shelf", 130, 12, 106, 44, "wood_dark", 2)
    b.line("pull", [(56, 34), (78, 34)], 4)
    b.box("foot_left", 16, 64, 12, b.F - 64, "wood_dark")
    b.box("foot_right", 222, 64, 12, b.F - 64, "wood_dark")
    b.stand(125, 4, "surface")
    b.ground()


@prop("prop.furniture.side_table_v1", "Pikkupöytä", 84, 104, ["pöytä", "table", "pikkupöytä", "side_table", "olohuone", "living_room", "furniture"])
def _(b):
    b.line("leg_left", [(22, 14), (12, 101.5)], 5, "wood_dark")
    b.line("leg_right", [(62, 14), (72, 101.5)], 5, "wood_dark")
    b.line("shelf", [(16, 66), (68, 66)], 5, "wood_dark")
    b.oval("top", 42, 10, 38, 6, "wood")
    b.stand(42, 4, "surface")
    b.ground()


# ---------------------------------------------------------------------------
# Architecture
# ---------------------------------------------------------------------------
@prop("prop.architecture.door_v1", "Ovi", 190, 440, ["ovi", "door", "kulku", "doorway", "seinä", "wall", "architecture"],
      variants=["evening", ("blue", {"wood_light": (150, 178, 190)})])
def _(b):
    b.box("frame", 4, 4, 182, b.F - 4, "wood_dark")
    b.box("leaf", 18, 18, 154, b.F - 18, "wood_light")
    b.box("panel_top", 36, 40, 118, 150, "wood_light", 2)
    b.box("panel_bottom", 36, 220, 118, 180, "wood_light", 2)
    b.oval("handle", 152, 214, 6, 6, "metal_dark", 2)
    b.doorway(18, 18, 154, 422)
    b.ground()


@prop("prop.architecture.window_v1", "Ikkuna", 200, 236, ["ikkuna", "window", "lasi", "glass", "seinä", "wall", "architecture"],
      variants=["evening"])
def _(b):
    b.box("frame", 8, 4, 184, 210, "white")
    b.box("pane_left", 20, 16, 74, 186, "glass", 2)
    b.box("pane_right", 106, 16, 74, 186, "glass", 2)
    b.line("crossbar_left", [(20, 92), (94, 92)], 4, "white")
    b.line("crossbar_right", [(106, 92), (180, 92)], 4, "white")
    b.line("glint", [(32, 40), (52, 30)], 3, "paper")
    b.box("sill", 2, 214, 196, 14, "wood")


# ---------------------------------------------------------------------------
# Sauna
# ---------------------------------------------------------------------------
@prop("prop.sauna.bench_v1", "Saunan lauteet", 300, 150, ["lauteet", "sauna_bench", "sauna", "puu", "wood", "suomi", "finland"],
      colors={"wood": (206, 168, 120), "wood_dark": (150, 112, 76)}, variants=["evening"])
def _(b):
    b.box("riser", 20, 16, 260, 74, "wood_dark")
    for i in range(4):
        b.line(f"riser_slat_{i}", [(20, 30 + i * 16), (280, 30 + i * 16)], 2)
    b.box("upper", 4, 4, 292, 14, "wood")
    b.box("step_front", 20, 104, 260, b.F - 104, "wood_dark")
    b.box("step", 4, 90, 292, 14, "wood")
    b.seat(150, 4)
    b.seat(150, 90, "seat_lower")
    b.ground()


@prop("prop.sauna.stove_v1", "Kiuas", 116, 156, ["kiuas", "sauna_stove", "sauna", "kivi", "stone", "suomi", "finland"],
      variants=["evening"])
def _(b):
    b.box("leg_left", 16, 140, 10, b.F - 140, "metal_dark")
    b.box("leg_right", 90, 140, 10, b.F - 140, "metal_dark")
    b.box("body", 8, 40, 100, 100, "screen")
    b.box("door", 30, 86, 56, 38, "metal_dark")
    b.box("fire", 38, 96, 40, 20, "fire", 2)
    b.line("door_handle", [(40, 92), (76, 92)], 3, "metal")
    for i, (cx, cy, slot) in enumerate(((26, 34, "stone"), (50, 30, "stone_dark"), (74, 32, "stone"), (94, 36, "stone_dark"),
                                        (38, 18, "stone_dark"), (62, 16, "stone"), (82, 22, "stone"))):
        b.oval(f"stone_{i}", cx, cy, 13, 10, slot, 2)
    b.ground()


@prop("prop.sauna.bucket_v1", "Kiulu", 72, 76, ["kiulu", "sauna_bucket", "sauna", "puu", "wood", "suomi", "finland"],
      colors={"wood": (206, 168, 120)})
def _(b):
    b.line("handle", [(14, 34), (14, 8), (58, 8), (58, 34)], 4, "metal_dark")
    b.box("bucket", 10, 30, 52, b.F - 30, "wood")
    for i in range(1, 4):
        b.line(f"stave_{i}", [(10 + i * 13, 32), (10 + i * 13, b.F - 2)], 1.5, "wood_dark")
    b.line("band_top", [(10, 40), (62, 40)], 4, "metal_dark")
    b.line("band_bottom", [(10, 64), (62, 64)], 4, "metal_dark")
    b.grip(36, 8)
    b.ground("base")


@prop("prop.sauna.ladle_v1", "Löylykauha", 40, 120, ["löylykauha", "sauna_ladle", "kauha", "ladle", "sauna", "suomi", "finland"],
      colors={"wood": (206, 168, 120)})
def _(b):
    b.line("handle", [(20, 6), (20, 92)], 6, "wood_dark")
    b.box("cup", 6, 90, 28, 26, "wood")
    b.grip(20, 30)


@prop("prop.sauna.vihta_v1", "Vihta", 80, 150, ["vihta", "vasta", "sauna_whisk", "koivu", "birch", "sauna", "suomi", "finland"])
def _(b):
    b.line("handle", [(40, 112), (40, 146)], 7, "wood_dark")
    for i, (cx, cy, slot) in enumerate(((40, 20, "leaf"), (24, 38, "leaf_dark"), (56, 40, "leaf"), (30, 62, "leaf"), (52, 66, "leaf_dark"),
                                        (40, 88, "leaf"), (40, 46, "leaf_dark"))):
        b.oval(f"leaves_{i}", cx, cy, 18, 16, slot, 2)
    b.line("binding", [(32, 112), (48, 112)], 5, "accent")
    b.grip(40, 132)


# ---------------------------------------------------------------------------
# Outdoor
# ---------------------------------------------------------------------------
@prop("prop.outdoor.street_lamp_v1", "Katuvalo", 130, 520, ["katuvalo", "street_lamp", "valo", "light", "katu", "street", "outdoor"],
      variants=["evening"])
def _(b):
    b.line("pole", [(40, 30), (40, 500)], 8, "metal_dark")
    b.line("arm", [(40, 30), (40, 16), (100, 16), (100, 26)], 6, "metal_dark")
    b.oval("glow", 100, 40, 22, 9, "light", 0)
    b.box("head", 78, 22, 44, 14, "metal_dark")
    b.box("base", 26, 494, 28, b.F - 494, "metal_dark")
    b.ground()


@prop("prop.outdoor.tree_v1", "Lehtipuu", 300, 560, ["puu", "tree", "lehtipuu", "koivu", "ulko", "outdoor", "luonto", "nature"],
      variants=["evening", ("autumn", {"leaf": (204, 160, 92), "leaf_dark": (170, 112, 78)})])
def _(b):
    b.box("trunk", 132, 300, 36, b.F - 300, "bark")
    b.line("branch_left", [(150, 340), (96, 260)], 7, "bark")
    b.line("branch_right", [(150, 320), (206, 250)], 7, "bark")
    for i, (cx, cy, rx, ry, slot) in enumerate(((150, 120, 100, 90, "leaf_dark"), (82, 210, 74, 66, "leaf"), (220, 200, 74, 66, "leaf"),
                                                (150, 240, 84, 60, "leaf_dark"), (150, 160, 70, 60, "leaf"))):
        b.oval(f"crown_{i}", cx, cy, rx, ry, slot, 4)
    b.ground()


@prop("prop.outdoor.bus_stop_shelter_v1", "Bussikatos", 480, 420, ["bussikatos", "bus_stop", "pysäkki", "katos", "shelter", "katu", "street", "outdoor"],
      variants=["evening"])
def _(b):
    b.box("back_glass", 30, 50, 420, 300, "glass_clear")
    b.box("ad_panel", 330, 80, 100, 160, "paper", 3)
    b.box("ad_picture", 342, 96, 76, 70, "blue", 2)
    b.line("post_left", [(26, 30), (26, 416)], 8, "metal_dark")
    b.line("post_right", [(454, 30), (454, 416)], 8, "metal_dark")
    b.box("roof", 4, 14, 472, 24, "metal_dark")
    b.box("bench", 80, 316, 220, 12, "wood")
    b.line("bench_leg_left", [(100, 328), (100, 417)], 6, "metal_dark")
    b.line("bench_leg_right", [(280, 328), (280, 417)], 6, "metal_dark")
    b.box("sign", 40, 60, 60, 24, "accent", 2)
    b.seat(190, 316)
    b.ground()


@prop("prop.outdoor.mailbox_v1", "Postilaatikko", 100, 180, ["postilaatikko", "mailbox", "posti", "mail", "piha", "yard", "outdoor"],
      colors={"accent": (204, 168, 96)}, variants=["evening"])
def _(b):
    b.box("post", 44, 70, 12, b.F - 70, "wood_dark")
    b.box("box", 8, 10, 84, 60, "accent")
    b.box("slot", 22, 22, 56, 8, "screen", 2)
    b.box("name_plate", 30, 42, 40, 14, "paper", 2)
    b.ground()


@prop("prop.outdoor.bicycle_v1", "Polkupyörä", 260, 150, ["polkupyörä", "bicycle", "pyörä", "bike", "katu", "street", "outdoor"])
def _(b):
    b.oval("wheel_back", 56, 103, 44, 44.5, "metal", 5)
    b.oval("hub_back", 56, 103, 4, 4, "metal_dark", 2)
    b.oval("wheel_front", 204, 103, 44, 44.5, "metal", 5)
    b.oval("hub_front", 204, 103, 4, 4, "metal_dark", 2)
    b.line("frame", [(56, 103), (110, 103), (182, 52), (100, 52), (56, 103)], 6, "blue")
    b.line("seat_tube", [(110, 103), (94, 36)], 6, "blue")
    b.line("fork", [(182, 52), (204, 103)], 6, "blue")
    b.line("handlebar", [(182, 52), (176, 28), (196, 26)], 5)
    b.box("saddle", 80, 30, 30, 8, "screen", 2)
    b.grip(196, 26)
    b.ground()


@prop("prop.outdoor.street_bin_v1", "Roska-astia", 84, 124, ["roska_astia", "street_bin", "roska", "trash", "katu", "street", "outdoor"])
def _(b):
    b.box("post", 36, 80, 12, b.F - 80, "metal_dark")
    b.box("bin", 8, 14, 68, 70, "leaf_dark")
    b.box("lid", 4, 6, 76, 12, "metal_dark")
    b.box("mouth", 20, 22, 44, 10, "screen", 2)
    b.ground()


@prop("prop.outdoor.fence_v1", "Aita", 300, 120, ["aita", "fence", "piha", "yard", "puu", "wood", "outdoor"],
      colors={"wood_light": (226, 220, 206)}, variants=["evening"])
def _(b):
    for i in range(10):
        x = 6 + i * 29
        b.box(f"picket_{i}", x, 10, 20, b.F - 10, "wood_light")
    b.line("rail_top", [(4, 36), (296, 36)], 7, "wood_dark")
    b.line("rail_bottom", [(4, 92), (296, 92)], 7, "wood_dark")
    b.ground()


@prop("prop.outdoor.bush_v1", "Pensas", 190, 116, ["pensas", "bush", "piha", "yard", "luonto", "nature", "outdoor"],
      variants=["evening"])
def _(b):
    for i, (cx, cy, rx, ry, slot) in enumerate(((50, 76, 46, 38, "leaf_dark"), (140, 76, 46, 38, "leaf_dark"),
                                                (95, 54, 56, 48, "leaf"), (60, 90, 40, 24, "leaf"), (130, 90, 40, 24, "leaf"))):
        b.oval(f"clump_{i}", cx, cy, rx, ry, slot)
    b.box("soil", 20, 108, 150, b.F - 108, "bark")
    b.ground()


@prop("prop.outdoor.spruce_v1", "Kuusi", 220, 440, ["kuusi", "spruce", "havupuu", "puu", "tree", "metsä", "forest", "suomi", "finland", "outdoor"],
      variants=["evening", ("snowy", {"leaf": (220, 228, 230)})])
def _(b):
    b.box("trunk", 98, 380, 24, b.F - 380, "bark")
    tiers = [(110, 340, 104, 38), (110, 280, 86, 34), (110, 220, 70, 30), (110, 164, 54, 26), (110, 112, 40, 22), (110, 66, 26, 18), (110, 30, 12, 24)]
    for i, (cx, cy, rx, ry) in enumerate(tiers):
        b.oval(f"tier_{i}", cx, cy, rx, ry, "leaf_dark" if i % 2 == 0 else "leaf")
    b.ground()


@prop("prop.outdoor.flagpole_v1", "Lipputanko", 150, 520, ["lipputanko", "flagpole", "lippu", "flag", "piha", "yard", "suomi", "finland", "outdoor"],
      colors={"blue": (64, 102, 160)})
def _(b):
    b.line("pole", [(20, 12), (20, 500)], 6, "metal_dark")
    b.oval("finial", 20, 8, 5, 5, "accent", 2)
    b.box("flag", 24, 24, 110, 68, "paper")
    b.line("cross_vertical", [(60, 26), (60, 90)], 8, "blue")
    b.line("cross_horizontal", [(26, 58), (132, 58)], 8, "blue")
    b.box("base", 8, 500, 24, b.F - 500, "stone")
    b.ground()


@prop("prop.decor.mirror_v1", "Peili", 90, 150, ["peili", "mirror", "eteinen", "hallway", "seinä", "wall", "decor"])
def _(b):
    b.oval("frame", 45, 75, 41, 71, "wood_dark")
    b.oval("glass", 45, 75, 33, 63, "glass", 2)
    b.line("glint_0", [(28, 40), (40, 28)], 3, "paper")
    b.line("glint_1", [(30, 54), (50, 34)], 2, "paper")


@prop("prop.outdoor.traffic_cone_v1", "Liikennekartio", 70, 96, ["liikennekartio", "traffic_cone", "kartio", "cone", "tietyö", "roadwork", "outdoor"])
def _(b):
    b.box("base", 4, 84, 62, b.F - 84, "screen")
    for i, (w, slot) in enumerate(((44, "fire"), (34, "paper"), (24, "fire"), (14, "fire"))):
        b.box(f"band_{i}", 35 - w / 2, 64 - i * 18, w, 20, slot, 2)
    b.grip(35, 12)
    b.ground()


@prop("prop.outdoor.snowman_v1", "Lumiukko", 160, 220, ["lumiukko", "snowman", "lumi", "snow", "talvi", "winter", "outdoor"])
def _(b):
    b.line("arm_left", [(52, 112), (14, 86)], 4, "bark")
    b.line("arm_right", [(108, 112), (146, 88)], 4, "bark")
    b.oval("bottom", 80, 174, 56, 44.5, "snow")
    b.oval("middle", 80, 112, 40, 32, "snow")
    b.oval("head", 80, 56, 28, 26, "snow")
    b.line("nose", [(84, 58), (104, 62)], 5, "fire")
    b.oval("eye_left", 72, 50, 3, 3, "line", 0)
    b.oval("eye_right", 90, 50, 3, 3, "line", 0)
    b.line("scarf", [(56, 84), (104, 84), (100, 104)], 7, "accent_red")
    b.box("hat", 60, 6, 40, 26, "screen", 2)
    b.ground()


# ---------------------------------------------------------------------------
# Winter (Finnish)
# ---------------------------------------------------------------------------
@prop("prop.winter.skis_v1", "Sukset", 60, 380, ["sukset", "skis", "hiihto", "skiing", "talvi", "winter", "suomi", "finland"],
      colors={"wood": (196, 150, 104)})
def _(b):
    for i, x in enumerate((10, 34)):
        b.box(f"ski_{i}", x, 20, 16, b.F - 20, "wood")
        b.oval(f"tip_{i}", x + 8, 20, 8, 14, "wood")
        b.box(f"binding_{i}", x + 2, 210, 12, 24, "screen", 2)
    b.grip(30, 120)
    b.ground()


@prop("prop.winter.snow_shovel_v1", "Lumikola", 110, 280, ["lumikola", "snow_shovel", "lapio", "shovel", "lumi", "snow", "talvi", "winter", "suomi", "finland"])
def _(b):
    b.line("shaft", [(55, 20), (55, 210)], 7, "wood_dark")
    b.line("handle", [(40, 14), (70, 14)], 7, "line")
    b.box("blade", 8, 208, 94, b.F - 208, "blue")
    b.line("blade_rib", [(20, 222), (90, 222)], 3)
    b.grip(55, 14, "grip")
    b.grip(55, 140, "grip_low")
    b.ground()


@prop("prop.winter.sled_v1", "Pulkka", 170, 56, ["pulkka", "sled", "mäenlasku", "sledding", "talvi", "winter", "suomi", "finland"])
def _(b):
    b.line("rope", [(156, 24), (168, 8)], 3, "accent")
    b.oval("hull", 85, 36, 80, 18.5, "accent_red")
    b.oval("seat_dip", 80, 30, 50, 8, "screen", 2)
    b.seat(80, 26)
    b.ground()


@prop("prop.clothing.mittens_v1", "Rukkaset", 100, 66, ["rukkaset", "mittens", "lapaset", "talvi", "winter", "vaate", "clothing", "suomi", "finland"],
      colors={"fabric": (184, 92, 84)})
def _(b):
    for i, (x, side) in enumerate(((28, -1), (72, 1))):
        b.oval(f"thumb_{i}", x + side * 17, 32, 7, 11, "fabric")
        b.oval(f"hand_{i}", x, 26, 18, 22, "fabric")
        b.box(f"cuff_{i}", x - 16, 48, 32, 16.5, "paper")
        b.line(f"cuff_stripe_{i}", [(x - 12, 56), (x + 12, 56)], 3, "fabric")
    b.grip(72, 30)
    b.ground("base")


# ---------------------------------------------------------------------------
# Office
# ---------------------------------------------------------------------------
@prop("prop.office.printer_v1", "Tulostin", 150, 90, ["tulostin", "printer", "toimisto", "office", "paperi", "paper"])
def _(b):
    b.box("paper_out", 30, 4, 90, 18, "paper", 2)
    b.box("body", 6, 18, 138, 56, "white")
    b.box("tray", 20, 56, 110, 10, "metal", 2)
    b.box("panel", 108, 26, 28, 18, "screen", 2)
    b.box("foot", 12, 74, 126, b.F - 74, "metal_dark")
    b.screen(108, 26, 28, 18, "panel")
    b.ground("base")


@prop("prop.office.file_cabinet_v1", "Arkistokaappi", 96, 210, ["arkistokaappi", "file_cabinet", "kaappi", "cabinet", "toimisto", "office"])
def _(b):
    b.box("body", 6, 4, 84, b.F - 4, "metal")
    for i in range(4):
        y = 12 + i * 48
        b.box(f"drawer_{i}", 14, y, 68, 42, "white", 2)
        b.line(f"handle_{i}", [(38, y + 14), (58, y + 14)], 4, "metal_dark")
        b.box(f"label_{i}", 38, y + 22, 20, 8, "paper", 1.5)
    b.ground()


@prop("prop.office.water_cooler_v1", "Vesiautomaatti", 90, 260, ["vesiautomaatti", "water_cooler", "vesi", "water", "toimisto", "office"])
def _(b):
    b.box("bottle", 22, 6, 46, 76, "glass_clear")
    b.box("water", 26, 30, 38, 48, "blue", 0)
    b.box("neck", 36, 82, 18, 10, "glass")
    b.box("body", 10, 92, 70, b.F - 92, "white")
    b.box("tap_panel", 18, 110, 54, 30, "metal", 2)
    b.line("tap_cold", [(32, 140), (32, 150)], 5, "blue")
    b.line("tap_hot", [(58, 140), (58, 150)], 5, "accent_red")
    b.box("drip_tray", 18, 160, 54, 8, "metal_dark", 2)
    b.ground()


@prop("prop.office.whiteboard_v1", "Valkotaulu", 380, 240, ["valkotaulu", "whiteboard", "taulu", "board", "toimisto", "office", "palaveri", "meeting"])
def _(b):
    b.box("frame", 4, 4, 372, 212, "metal")
    b.box("surface", 14, 14, 352, 192, "paper", 2)
    b.line("chart_axis", [(40, 40), (40, 170), (200, 170)], 3, "blue")
    b.line("chart_trend", [(48, 160), (90, 130), (130, 140), (180, 70)], 4, "accent_red")
    b.line("note_0", [(240, 60), (330, 60)], 3, "blue")
    b.line("note_1", [(240, 90), (310, 90)], 3, "blue")
    b.line("note_2", [(240, 120), (320, 120)], 3, "blue")
    b.box("tray", 60, 216, 260, 10, "metal_dark")
    b.screen(14, 14, 352, 192)


@prop("prop.office.desk_lamp_v1", "Työvalaisin", 100, 110, ["työvalaisin", "desk_lamp", "lamppu", "lamp", "toimisto", "office"])
def _(b):
    b.line("arm_low", [(30, 100), (46, 50)], 5, "metal_dark")
    b.line("arm_high", [(46, 50), (78, 24)], 5, "metal_dark")
    b.oval("joint", 46, 50, 5, 5, "metal_dark", 2)
    b.oval("glow", 82, 46, 12, 5, "light", 0)
    b.box("head", 66, 14, 32, 22, "accent")
    b.box("base", 6, 98, 50, b.F - 98, "metal_dark")
    b.ground("base")


@prop("prop.office.clipboard_v1", "Kirjoitusalusta", 64, 84, ["kirjoitusalusta", "clipboard", "paperi", "paper", "toimisto", "office"])
def _(b):
    b.box("board", 4, 8, 56, 72, "wood")
    b.box("sheet", 10, 16, 44, 58, "paper", 2)
    for i in range(4):
        b.line(f"text_{i}", [(16, 30 + i * 10), (46 - (i % 2) * 8, 30 + i * 10)], 2, "blue")
    b.box("clip", 20, 4, 24, 10, "metal_dark", 2)
    b.grip(32, 44)


@prop("prop.office.newspaper_v1", "Sanomalehti", 76, 96, ["sanomalehti", "newspaper", "lehti", "paper", "uutiset", "news"])
def _(b):
    b.box("sheet", 4, 4, 68, 88, "paper")
    b.box("masthead", 10, 10, 56, 12, "screen", 0)
    b.box("photo", 10, 28, 26, 22, "blue", 2)
    for i in range(6):
        b.line(f"col_{i}", [(42, 30 + i * 9), (66, 30 + i * 9)], 2, "metal_dark")
    for i in range(3):
        b.line(f"body_{i}", [(10, 60 + i * 9), (66, 60 + i * 9)], 2, "metal_dark")
    b.line("fold", [(38, 4), (38, 92)], 1.5, "metal")
    b.grip(10, 48)


# ---------------------------------------------------------------------------
# Classroom, shop, travel
# ---------------------------------------------------------------------------
@prop("prop.classroom.blackboard_v1", "Liitutaulu", 420, 280, ["liitutaulu", "blackboard", "taulu", "board", "luokka", "classroom", "koulu", "school"],
      variants=["evening"])
def _(b):
    b.box("frame", 4, 4, 412, 252, "wood")
    b.box("board", 14, 14, 392, 232, "board", 2)
    b.line("chalk_line_0", [(40, 50), (180, 50)], 3, "chalk")
    b.line("chalk_line_1", [(40, 80), (150, 80)], 3, "chalk")
    b.line("chalk_sum", [(40, 120), (60, 140), (40, 160)], 3, "chalk")
    b.line("chalk_box", [(260, 60), (360, 60), (360, 150), (260, 150), (260, 60)], 3, "chalk")
    b.box("tray", 40, 256, 340, 10, "wood_dark")
    b.box("chalk", 300, 250, 20, 6, "chalk", 1.5)
    b.screen(14, 14, 392, 232)


@prop("prop.classroom.school_desk_v1", "Koulupulpetti", 160, 124, ["pulpetti", "school_desk", "pöytä", "desk", "luokka", "classroom", "koulu", "school"])
def _(b):
    b.line("leg_left", [(26, 20), (26, 121)], 6, "metal_dark")
    b.line("leg_right", [(134, 20), (134, 121)], 6, "metal_dark")
    b.box("book_tray", 22, 30, 116, 18, "metal", 2)
    b.box("top", 4, 6, 152, 14, "wood")
    b.stand(80, 6, "surface")
    b.ground()


@prop("prop.classroom.globe_v1", "Karttapallo", 90, 124, ["karttapallo", "globe", "maapallo", "earth", "luokka", "classroom", "koulu", "school"])
def _(b):
    b.line("meridian", [(45, 10), (68, 17), (83, 38), (83, 64), (70, 84)], 4, "metal_dark")
    b.oval("sphere", 45, 52, 36, 36, "blue")
    b.oval("land_0", 34, 42, 14, 10, "leaf", 2)
    b.oval("land_1", 58, 66, 12, 9, "leaf", 2)
    b.line("stem", [(45, 88), (45, 110)], 6, "metal_dark")
    b.box("base", 20, 110, 50, b.F - 110, "wood_dark")
    b.ground("base")


@prop("prop.shop.shelf_v1", "Kaupan hylly", 210, 270, ["hylly", "shelf", "kauppa", "shop", "myymälä", "store"],
      variants=["evening"])
def _(b):
    b.box("back", 10, 4, 190, 250, "metal")
    products = ["accent_red", "accent", "blue", "paper", "leaf", "cushion"]
    for s in range(4):
        y = 64 + s * 60
        b.box(f"shelf_{s}", 4, y, 202, 8, "metal_dark", 2)
        x = 18
        k = 0
        while x < 180:
            pw = 18 + ((s * 7 + k * 5) % 3) * 6
            ph = 24 + ((s + k) % 3) * 8
            b.box(f"item_{s}_{k}", x, y - ph, pw, ph, products[(s + k) % len(products)], 2)
            x += pw + 8
            k += 1
    b.box("plinth", 10, 254, 190, b.F - 254, "metal_dark")
    b.ground()


@prop("prop.shop.cash_register_v1", "Kassakone", 140, 110, ["kassakone", "cash_register", "kassa", "checkout", "kauppa", "shop"])
def _(b):
    b.box("display_post", 98, 26, 10, 26, "metal_dark")
    b.box("display", 82, 6, 52, 22, "screen")
    b.box("display_text", 88, 12, 40, 10, "screen_glow", 0)
    b.box("body", 6, 52, 128, 36, "metal")
    for i in range(4):
        for j in range(2):
            b.box(f"key_{i}_{j}", 14 + i * 14, 58 + j * 12, 10, 8, "white", 1.5)
    b.box("drawer", 4, 88, 132, b.F - 88, "metal_dark")
    b.screen(88, 12, 40, 10)
    b.ground("base")


@prop("prop.shop.shopping_cart_v1", "Ostoskärry", 180, 170, ["ostoskärry", "shopping_cart", "kärry", "cart", "kauppa", "shop"])
def _(b):
    b.line("handle", [(170, 30), (150, 30)], 7, "accent_red")
    b.box("basket", 30, 34, 120, 70, "glass_clear")
    for i in range(1, 6):
        b.line(f"wire_{i}", [(30 + i * 20, 34), (30 + i * 20, 104)], 2, "metal_dark")
    b.line("basket_rim", [(30, 34), (150, 34), (150, 104), (30, 104), (30, 34)], 4, "metal_dark")
    b.line("chassis", [(150, 104), (150, 150), (30, 150), (40, 104)], 5, "metal_dark")
    b.oval("wheel_back", 40, 159.5, 9, 9, "screen")
    b.oval("wheel_front", 140, 159.5, 9, 9, "screen")
    b.grip(166, 30)
    b.ground()


@prop("prop.shop.shopping_bag_v1", "Ostoskassi", 80, 104, ["ostoskassi", "shopping_bag", "kassi", "bag", "kauppa", "shop"])
def _(b):
    b.line("handle", [(24, 38), (24, 8), (56, 8), (56, 38)], 4, "line")
    b.box("bag", 8, 34, 64, 68.5, "paper")
    b.oval("logo", 40, 64, 14, 14, "leaf", 2)
    b.oval("bread", 26, 34, 10, 8, "wood_light", 2)
    b.grip(40, 8)
    b.ground("base")


@prop("prop.travel.suitcase_v1", "Matkalaukku", 120, 170, ["matkalaukku", "suitcase", "laukku", "luggage", "matka", "travel"],
      variants=[("red", {"blue": (176, 104, 94)})])
def _(b):
    b.line("handle", [(40, 34), (40, 6), (80, 6), (80, 34)], 6, "metal_dark")
    b.box("shell", 10, 32, 100, 120, "blue")
    for i in range(1, 4):
        b.line(f"rib_{i}", [(10 + i * 25, 40), (10 + i * 25, 144)], 2, "fabric_dark")
    b.box("tag", 82, 44, 16, 22, "paper", 2)
    b.oval("wheel_left", 26, 160, 8, 8.5, "screen")
    b.oval("wheel_right", 94, 160, 8, 8.5, "screen")
    b.grip(60, 6)
    b.ground()


# ---------------------------------------------------------------------------
# Migrated rig-project props (examples/stick-studio/props/*.kilsat)
# ---------------------------------------------------------------------------
# file stem -> (id, tags, anchor recipe). Hand props use the `grip` joint origin
# (0, 0): hand-props.json shares the rig's local coordinates (identity transform).
MIGRATED = {
    "ajopaivakirja": ("prop.document.driving_log_v1", ["ajopäiväkirja", "logbook", "kirja", "book", "paperi", "paper", "auto", "car"],
                      [("grip_layer", "ajopaivakirja spine")]),
    "auto": ("prop.vehicle.small_car_v1", ["auto", "car", "ajoneuvo", "vehicle", "katu", "street", "outdoor"], [("ground",)]),
    "avain": ("prop.key.door_key_v1", ["avain", "key", "ovi", "door", "käsiesine", "hand_prop"], [("grip0",)]),
    "kahvikuppi": ("prop.drink.coffee_cup_v1", ["kahvikuppi", "coffee_cup", "kahvi", "coffee", "kuppi", "cup", "käsiesine", "hand_prop"], [("grip0",)]),
    "kannettava": ("prop.electronics.laptop_v1", ["kannettava", "laptop", "tietokone", "computer", "näyttö", "screen", "toimisto", "office"],
                   [("screen_layer", "kannettava screen"), ("ground", "base")]),
    "kello": ("prop.clock.late_clock_v1", ["kello", "clock", "aika", "time", "myöhässä", "late"], []),
    "kirja": ("prop.book.hardcover_v1", ["kirja", "book", "lukeminen", "reading", "käsiesine", "hand_prop"], [("grip0",)]),
    "kyna": ("prop.office.pen_v1", ["kynä", "pen", "kirjoitus", "writing", "toimisto", "office", "käsiesine", "hand_prop"], [("grip0",)]),
    "liikennemerkki": ("prop.street.bus_stop_sign_v1", ["liikennemerkki", "traffic_sign", "pysäkki", "bus_stop", "katu", "street", "outdoor"], [("ground",)]),
    "paketti": ("prop.parcel.mystery_box_v1", ["paketti", "parcel", "laatikko", "box", "posti", "mail"],
                [("grip_layer", "paketti box outline"), ("ground", "base")]),
    "puhelin": ("prop.phone.smartphone_front_v1", ["puhelin", "phone", "älypuhelin", "smartphone", "näyttö", "screen", "käsiesine", "hand_prop"],
                [("grip0",), ("screen_part", "screen")]),
    "puhelin_takana": ("prop.phone.smartphone_back_v1", ["puhelin", "phone", "älypuhelin", "smartphone", "takaosa", "back", "käsiesine", "hand_prop"],
                       [("grip0",)]),
    "puhelin_vino": ("prop.phone.smartphone_angled_v1", ["puhelin", "phone", "älypuhelin", "smartphone", "viisto", "angled", "käsiesine", "hand_prop"],
                     [("grip0",), ("screen_part", "screen")]),
    "sateenvarjo": ("prop.umbrella.night_umbrella_v1", ["sateenvarjo", "umbrella", "sade", "rain", "käsiesine", "hand_prop"],
                    [("grip_stroke", "sateenvarjo handle grip")]),
    "tabletti": ("prop.electronics.tablet_v1", ["tabletti", "tablet", "näyttö", "screen", "käsiesine", "hand_prop", "electronics"],
                 [("grip0",), ("screen_part", "screen")]),
    "toimistotuoli": ("prop.furniture.meeting_chair_v1", ["tuoli", "chair", "palaveri", "meeting", "toimisto", "office", "furniture"],
                      [("seat_layer", "toimistotuoli seat fill"), ("ground",)]),
    "tyokaluavain": ("prop.tool.wrench_v1", ["kiintoavain", "wrench", "työkalu", "tool", "käsiesine", "hand_prop"], [("grip0",)]),
    "tyopoyta": ("prop.furniture.work_desk_v1", ["työpöytä", "desk", "pöytä", "table", "toimisto", "office", "furniture"],
                 [("stand_layer", "tyopoyta top fill"), ("ground",)]),
    "viikate": ("prop.tool.scythe_v1", ["viikate", "scythe", "työkalu", "tool", "noutaja", "reaper"], [("grip_stroke_frac", "viikate staff outline", 0.35)]),
    "vuoronumerolaite": ("prop.shop.queue_ticket_machine_v1", ["vuoronumero", "queue_ticket", "jono", "queue", "kauppa", "shop", "näyttö", "screen"],
                         [("screen_layer", "vuoronumerolaite display"), ("ground",)]),
}


def outline_pad(outline: dict | None) -> float:
    if not outline:
        return 0.0
    w = float(outline.get("width", 0))
    return w / 2 if outline.get("mode", "over") == "over" else w


def kilsat_geometry(data: dict):
    """Return (kind, bounds_box, lookup) for a .kilsat project.

    Rig props: extent of all rig parts in rig-local coordinates (polygon points,
    ellipse cx±rx/cy±ry), padded by outline widths. Art-layer props (no character
    rig): extent of the art layers in document coordinates."""
    source = json.loads(data["source"])
    chars = source.get("characters") or {}
    xs: list[float] = []
    ys: list[float] = []
    lookup: dict[str, dict] = {}
    if chars:
        if len(chars) != 1:
            raise ValueError("expected exactly one character rig")
        rig = next(iter(chars.values()))["rig"]
        for part in rig["parts"]:
            pad = outline_pad(part.get("outline"))
            if "points" in part:
                pxs = [p[0] for p in part["points"]]
                pys = [p[1] for p in part["points"]]
            elif "ellipse" in part:
                e = part["ellipse"]
                pxs = [e["cx"] - e["rx"], e["cx"] + e["rx"]]
                pys = [e["cy"] - e["ry"], e["cy"] + e["ry"]]
            else:
                raise ValueError(f"unsupported rig part {part['id']}")
            box = (min(pxs), min(pys), max(pxs), max(pys))
            lookup[part["id"]] = {"box": box}
            xs += [box[0] - pad, box[2] + pad]
            ys += [box[1] - pad, box[3] + pad]
        kind = "rig"
    else:
        for layer in data["art"]["layers"]:
            c = layer["content"]
            if c["type"] in ("rectangle", "ellipse"):
                r = c["rect"]
                box = (r["x"], r["y"], r["x"] + r["width"], r["y"] + r["height"])
                xs += [box[0], box[2]]
                ys += [box[1], box[3]]
            elif c["type"] == "stroke":
                hw = c["width"] / 2
                px = [p["x"] for p in c["points"]]
                py = [p["y"] for p in c["points"]]
                box = (min(px), min(py), max(px), max(py))
                xs += [box[0] - hw, box[2] + hw]
                ys += [box[1] - hw, box[3] + hw]
            else:
                raise ValueError(f"unsupported art content {c['type']}")
            lookup[layer["name"]] = {"box": box, "content": c}
        kind = "art"
    bx, by = floor3(min(xs)), floor3(min(ys))
    bounds = (bx, by, ceil3(max(xs)) - bx, ceil3(max(ys)) - by)
    return kind, bounds, lookup


def migrated_manifest(stem: str, path: Path) -> dict:
    raw = path.read_bytes()
    data = json.loads(raw)
    pid, tags, recipe = MIGRATED[stem]
    name = json.loads(data["source"])["project"]["title"]
    kind, (bx, by, bw, bh), lookup = kilsat_geometry(data)
    anchors = []
    for step in recipe:
        op = step[0]
        if op == "grip0":
            anchors.append({"type": "grip", "id": "grip", "at": pt(0, 0)})
        elif op == "ground":
            aid = step[1] if len(step) > 1 else "floor"
            anchors.append({"type": "ground", "id": aid, "at": pt(bx, by + bh), "width": num(bw)})
        elif op in ("screen_part", "screen_layer"):
            x0, y0, x1, y1 = lookup[step[1]]["box"]
            anchors.append({"type": "screen", "id": "screen", "rect": rect(x0, y0, x1 - x0, y1 - y0)})
        elif op == "grip_layer":
            x0, y0, x1, y1 = lookup[step[1]]["box"]
            anchors.append({"type": "grip", "id": "grip", "at": pt((x0 + x1) / 2, (y0 + y1) / 2)})
        elif op == "grip_stroke":
            pts = lookup[step[1]]["content"]["points"]
            mid = pts[len(pts) // 2]
            anchors.append({"type": "grip", "id": "grip", "at": pt(mid["x"], mid["y"])})
        elif op == "grip_stroke_frac":
            a, b2 = lookup[step[1]]["content"]["points"][:2]
            f = step[2]
            anchors.append({"type": "grip", "id": "grip", "at": pt(a["x"] + (b2["x"] - a["x"]) * f, a["y"] + (b2["y"] - a["y"]) * f)})
        elif op == "seat_layer":
            x0, y0, x1, _ = lookup[step[1]]["box"]
            anchors.append({"type": "seat", "id": "seat", "at": pt((x0 + x1) / 2, y0)})
        elif op == "stand_layer":
            x0, y0, x1, _ = lookup[step[1]]["box"]
            anchors.append({"type": "stand", "id": "surface", "at": pt((x0 + x1) / 2, y0)})
        else:
            raise ValueError(op)
    m = {
        "schema_version": SCHEMA_VERSION,
        "id": pid,
        "kind": "prop",
        "name": name,
        "bounds": rect(bx, by, bw, bh),
        "depth": "mid",
        "tags": tags,
        "license": dict(LICENSE),
        "geometry": {"type": "rig_project", "path": f"props/{path.name}", "sha256": hashlib.sha256(raw).hexdigest()},
    }
    if anchors:
        m["anchors"] = anchors
    return m


def art_manifest(spec: dict) -> dict:
    b = Art(spec["w"], spec["h"], spec["colors"])
    spec["draw"](b)
    palette = b.palette()
    m = {
        "schema_version": SCHEMA_VERSION,
        "id": spec["id"],
        "kind": "prop",
        "name": spec["name"],
        "bounds": rect(0, 0, spec["w"], spec["h"]),
        "depth": spec["depth"],
        "tags": spec["tags"],
        "license": dict(LICENSE),
        "palette": palette,
        "geometry": {"type": "art", "layers": b.layers},
    }
    if b.anchors:
        m["anchors"] = b.anchors
    variants = []
    for v in spec["variants"]:
        if v == "evening":
            variants.append({"name": "evening", "palette": evening(palette)})
        else:
            vname, over = v
            variants.append({"name": vname, "palette": {k: color(c) for k, c in sorted(over.items())}})
    if variants:
        m["variants"] = variants
    return m


# ---------------------------------------------------------------------------
# Self-check (a light mirror of the ORK-LIB-01 rules; motion-library is authoritative)
# ---------------------------------------------------------------------------
def content_box(c: dict):
    if c["type"] in ("rectangle", "ellipse"):
        r = c["rect"]
        return r["x"], r["y"], r["x"] + r["width"], r["y"] + r["height"]
    hw = c["width"] / 2
    xs = [p["x"] for p in c["points"]]
    ys = [p["y"] for p in c["points"]]
    return min(xs) - hw, min(ys) - hw, max(xs) + hw, max(ys) + hw


def finite(*vals) -> bool:
    return all(isinstance(v, (int, float)) and math.isfinite(v) for v in vals)


def check(m: dict) -> list[str]:
    errs = []
    pid = m["id"]
    if not ID_RE.match(pid):
        errs.append("id does not match prop.<category>.<name>_v<N>")
    if m["kind"] != "prop" or m["schema_version"] != 1 or not m["name"]:
        errs.append("kind/schema_version/name")
    tags = m["tags"]
    if not tags or len(set(tags)) != len(tags) or not all(TAG_RE.match(t) for t in tags):
        errs.append(f"tags invalid: {tags}")
    bd = m["bounds"]
    bx, by, bw, bh = bd["x"], bd["y"], bd["width"], bd["height"]
    if not finite(bx, by, bw, bh) or bw <= 0 or bh <= 0:
        errs.append("bounds not finite/positive")

    def inside(x, y):
        return finite(x, y) and bx - EPS <= x <= bx + bw + EPS and by - EPS <= y <= by + bh + EPS

    ids = set()
    for a in m.get("anchors", []):
        if a["id"] in ids:
            errs.append(f"duplicate anchor {a['id']}")
        ids.add(a["id"])
        if "at" in a and not inside(a["at"]["x"], a["at"]["y"]):
            errs.append(f"anchor {a['id']} outside bounds")
        if a["type"] == "ground" and not (inside(a["at"]["x"] + a["width"], a["at"]["y"]) and a["width"] > 0):
            errs.append(f"ground {a['id']} width outside bounds")
        if "rect" in a:
            r = a["rect"]
            if not (inside(r["x"], r["y"]) and inside(r["x"] + r["width"], r["y"] + r["height"]) and r["width"] > 0 and r["height"] > 0):
                errs.append(f"anchor {a['id']} rect outside bounds")
    g = m["geometry"]
    if g["type"] == "rig_project":
        p = PACKAGE / g["path"]
        if ".." in g["path"] or g["path"].startswith("/") or not p.is_file():
            errs.append("rig_project path invalid")
        elif hashlib.sha256(p.read_bytes()).hexdigest() != g["sha256"]:
            errs.append("rig_project sha256 mismatch")
        if "palette" in m or "variants" in m:
            errs.append("rig_project carries palette/variants")
    elif g["type"] == "art":
        pal = m.get("palette", {})
        lids = set()
        bottom = 0.0
        for layer in g["layers"]:
            lid = layer["id"]
            if lid in lids or not LAYER_RE.match(lid):
                errs.append(f"layer id {lid}")
            lids.add(lid)
            slot = layer.get("paint")
            if slot is not None and slot not in pal:
                errs.append(f"layer {lid} paint {slot} missing from palette")
            c = layer["content"]
            if slot is not None and c["color"] != pal[slot]:
                errs.append(f"layer {lid} colour differs from palette slot")
            if c["type"] == "stroke" and c["width"] > LINE_LIMIT:
                errs.append(f"layer {lid} stroke wider than {LINE_LIMIT}")
            x0, y0, x1, y1 = content_box(c)
            if not (inside(x0, y0) and inside(x1, y1)):
                errs.append(f"layer {lid} outside bounds ({x0:.2f},{y0:.2f})-({x1:.2f},{y1:.2f})")
            bottom = max(bottom, y1)
        for a in m.get("anchors", []):
            if a["type"] == "ground" and abs(bottom - (by + bh)) > 0.01:
                errs.append(f"grounded prop does not touch its ground line (bottom {bottom:.3f})")
        unused = set(pal) - {layer.get("paint") for layer in g["layers"]}
        if unused:
            errs.append(f"unused palette slots {sorted(unused)}")
        names = set()
        for v in m.get("variants", []):
            if not VARIANT_RE.match(v["name"]) or v["name"] in names:
                errs.append(f"variant name {v['name']}")
            names.add(v["name"])
            extra = set(v.get("palette", {})) - set(pal)
            if extra:
                errs.append(f"variant {v['name']} adds unknown slots {sorted(extra)}")
    else:
        errs.append("unexpected geometry type")
    if m["license"] != LICENSE:
        errs.append("license")
    return [f"{pid}: {e}" for e in errs]


def build() -> dict[str, dict]:
    manifests: dict[str, dict] = {}
    stems = sorted(p.stem for p in RIG_DIR.glob("*.kilsat"))
    missing = set(stems) ^ set(MIGRATED)
    if missing:
        raise SystemExit(f"rig prop list out of date: {sorted(missing)}")
    for stem in stems:
        m = migrated_manifest(stem, RIG_DIR / f"{stem}.kilsat")
        manifests[m["id"]] = m
    for spec in ART:
        if spec["id"] in manifests:
            raise SystemExit(f"duplicate id {spec['id']}")
        manifests[spec["id"]] = art_manifest(spec)
    return manifests


def render(m: dict) -> str:
    return json.dumps(m, indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str]) -> int:
    check_only = "--check" in argv
    manifests = build()
    errors = [e for m in manifests.values() for e in check(m)]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    expected = {f"{pid}.json": render(m) for pid, m in sorted(manifests.items())}
    stale = sorted(p.name for p in OUT_DIR.glob("prop.*.json") if p.name not in expected) if OUT_DIR.is_dir() else []
    if check_only:
        bad = [n for n, text in expected.items() if not (OUT_DIR / n).is_file() or (OUT_DIR / n).read_text(encoding="utf-8") != text]
        if bad or stale:
            print(f"out of date: {bad + stale}", file=sys.stderr)
            return 1
        print(f"ok: {len(expected)} prop manifests up to date")
        return 0
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for n in stale:
        (OUT_DIR / n).unlink()
    for n, text in expected.items():
        p = OUT_DIR / n
        if not p.is_file() or p.read_text(encoding="utf-8") != text:
            p.write_text(text, encoding="utf-8")
    rig = sum(1 for m in manifests.values() if m["geometry"]["type"] == "rig_project")
    anchors: dict[str, int] = {}
    for m in manifests.values():
        for a in m.get("anchors", []):
            anchors[a["type"]] = anchors.get(a["type"], 0) + 1
    print(f"wrote {len(expected)} prop manifests ({rig} rig_project, {len(expected) - rig} art); "
          f"removed {len(stale)} stale; anchors {dict(sorted(anchors.items()))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
