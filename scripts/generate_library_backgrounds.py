#!/usr/bin/env python3
"""Generate the nature, rural, leisure, fantasy and sci-fi backgrounds of the library.

Writes `examples/stick-studio/library/backgrounds/<id>.json` (manifest v1,
ORK-LIB-01) for the backgrounds listed in BACKGROUNDS. The older backgrounds
belong to `crates/motion-library/examples/generate_stick_library.rs`, which also
kit-bashes these backgrounds into locations and sets.

Original geometry from rectangles, ellipses and strokes on a 1920x1080 canvas at
actor scale 1. Sky, distant scenery and the ground are `far`; nearer scenery
behind the actors is `mid`. Every background passes the stick-studio style rule
(`motion_art::style::check_background`): large opaque surfaces are muted and
light enough for the dark limbs (contrast >= 4.5, saturation <= 0.35) and
strokes are at most 18 px. `check()` mirrors that rule; the Rust tests assemble
every location that uses these backgrounds and run the real check.

Usage (from the repository root):
    python3 scripts/generate_library_backgrounds.py           # write manifests
    python3 scripts/generate_library_backgrounds.py --check   # verify files are up to date
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "examples" / "stick-studio" / "library" / "backgrounds"
W, H = 1920.0, 1080.0
LICENSE = {"license": "MIT", "author": "Koeta Studio", "origin": "original", "redistribution": "allowed"}
INK = (43, 49, 48)
LIMB_INK = (36, 40, 45)
MAX_LINE = 18.0


def num(v):
    r = round(float(v), 2)
    return float(r)


def col(c):
    r, g, b = c[:3]
    return {"r": r, "g": g, "b": b, "a": c[3] if len(c) == 4 else 255}


class Bg:
    def __init__(self):
        self.layers = []
        self.anchors = []
        self.n = {}

    def _id(self, name):
        k = self.n.get(name, 0)
        self.n[name] = k + 1
        return name if k == 0 else f"{name}_{k}"

    def _add(self, name, depth, content):
        self.layers.append({"id": self._id(name), "depth": depth, "content": content})

    def rect(self, name, x, y, w, h, c, depth="far"):
        self._add(name, depth, {"type": "rectangle", "rect": {"x": num(x), "y": num(y), "width": num(w), "height": num(h)}, "color": col(c)})

    def oval(self, name, cx, cy, rx, ry, c, depth="far"):
        self._add(name, depth, {"type": "ellipse", "rect": {"x": num(cx - rx), "y": num(cy - ry), "width": num(2 * rx), "height": num(2 * ry)}, "color": col(c)})

    def line(self, name, pts, width, c, depth="far"):
        self._add(name, depth, {"type": "stroke", "points": [{"x": num(x), "y": num(y)} for x, y in pts], "color": col(c), "width": num(width)})

    def triangle(self, name, apex_x, apex_y, base_y, half, c, depth="far", step=12.0):
        """Filled triangle from stacked horizontal strokes (no polygon content)."""
        rows = max(2, math.ceil((base_y - apex_y) / step))
        for i in range(rows):
            t = (i + 0.5) / rows
            y = apex_y + t * (base_y - apex_y)
            hw = max(half * t - step / 2, 0.5)
            self.line(name, [(apex_x - hw, y), (apex_x + hw, y)], step + 1, c, depth)

    def spruce(self, name, x, base_y, height, c, trunk=(112, 88, 68), depth="far"):
        self.line(name + "_trunk", [(x, base_y), (x, base_y - height * 0.2)], min(18, max(4, height * 0.06)), trunk, depth)
        for k, (top, bottom, half) in enumerate(((0.0, 0.45, 0.22), (0.25, 0.7, 0.3), (0.45, 0.88, 0.38))):
            self.triangle(f"{name}_tier{k}", x, base_y - height * (1 - top), base_y - height * (1 - bottom), height * half, c, depth, step=min(12.0, height * 0.05))

    def leafy_tree(self, name, x, base_y, height, c, dark, trunk=(112, 88, 68), depth="far"):
        self.line(name + "_trunk", [(x, base_y), (x, base_y - height * 0.55)], min(18, height * 0.07), trunk, depth)
        r = height * 0.28
        self.oval(name + "_crown_a", x - r * 0.55, base_y - height * 0.62, r * 0.8, r * 0.7, dark, depth)
        self.oval(name + "_crown_b", x + r * 0.55, base_y - height * 0.6, r * 0.8, r * 0.7, dark, depth)
        self.oval(name + "_crown_c", x, base_y - height * 0.78, r, r * 0.8, c, depth)

    def cloud(self, name, x, y, s, c=(240, 240, 236)):
        for k, (dx, dy, rx, ry) in enumerate(((0, 0, 1.0, 0.45), (-0.7, 0.12, 0.6, 0.35), (0.75, 0.1, 0.65, 0.35), (0.1, -0.3, 0.55, 0.4))):
            self.oval(f"{name}_{k}", x + dx * s, y + dy * s, rx * s, ry * s, c)

    def ground(self, y):
        self.anchors.append({"type": "ground", "id": "ground", "at": {"x": 0.0, "y": num(y)}, "width": W})

    def stand(self, aid, x, y):
        self.anchors.append({"type": "stand", "id": aid, "at": {"x": num(x), "y": num(y)}})

    def seat(self, aid, x, y):
        self.anchors.append({"type": "seat", "id": aid, "at": {"x": num(x), "y": num(y)}})

    def doorway(self, aid, x, y, w, h):
        self.anchors.append({"type": "doorway", "id": aid, "rect": {"x": num(x), "y": num(y), "width": num(w), "height": num(h)}})

    def screen(self, aid, x, y, w, h):
        self.anchors.append({"type": "screen", "id": aid, "rect": {"x": num(x), "y": num(y), "width": num(w), "height": num(h)}})


BACKGROUNDS = {}


def background(bid, name, tags):
    def wrap(fn):
        BACKGROUNDS[bid] = (name, tags, fn)
        return fn
    return wrap


SKY = (186, 206, 214)
GRASS = (170, 190, 150)
GRASS_FAR = (150, 172, 140)
SPRUCE = (104, 132, 112)
SPRUCE_FAR = (138, 160, 146)
LEAF = (132, 160, 120)
LEAF_DARK = (110, 138, 104)


# ---------------------------------------------------------------- nature
@background("background.nature.forest_v1", "Metsä", ["outdoor", "day", "nature", "luonto", "forest", "metsä", "trees", "puut"])
def _(b):
    b.rect("sky", 0, 0, W, H, (196, 212, 210))
    for i in range(9):
        b.spruce(f"far_spruce_{i}", 80 + i * 225, 790, 330 + (i * 47) % 90, SPRUCE_FAR)
    b.rect("forest_floor", 0, 780, W, 300, (176, 188, 150))
    b.rect("path", 820, 780, 300, 300, (204, 194, 168))
    for i, (x, h) in enumerate(((140, 560), (520, 610), (1380, 590), (1760, 640))):
        b.spruce(f"spruce_{i}", x, 900, h, SPRUCE, depth="mid")
    for i, x in enumerate((360, 1180, 1580)):
        b.oval(f"bush_{i}", x, 905, 90, 40, LEAF_DARK, "mid")
    b.oval("rock", 1020, 930, 70, 30, (164, 162, 156), "mid")
    b.ground(950)
    b.seat("rock", 1020, 905)
    b.stand("path", 970, 950)


@background("background.nature.lake_shore_v1", "Järvenranta", ["outdoor", "day", "summer", "kesä", "nature", "luonto", "lake", "järvi", "shore", "ranta"])
def _(b):
    b.rect("sky", 0, 0, W, H, (196, 214, 222))
    b.cloud("cloud_a", 420, 160, 90)
    b.cloud("cloud_b", 1420, 120, 70)
    b.oval("far_shore", 960, 560, 1100, 70, (150, 172, 152))
    for i in range(12):
        b.spruce(f"shore_spruce_{i}", 60 + i * 165, 560, 120 + (i * 31) % 50, SPRUCE_FAR)
    b.rect("lake", 0, 560, W, 240, (176, 200, 210))
    for i in range(6):
        y = 600 + i * 34
        b.line(f"ripple_{i}", [(200 + (i * 280) % 1300, y), (330 + (i * 280) % 1300, y)], 3, (206, 222, 228))
    b.rect("beach", 0, 790, W, 290, (214, 204, 176))
    b.rect("dock", 1260, 740, 420, 22, (170, 140, 110), "mid")
    for i, x in enumerate((1280, 1440, 1600, 1660)):
        b.line(f"dock_post_{i}", [(x, 760), (x, 820)], 12, (130, 104, 84), "mid")
    b.oval("reeds_base", 260, 812, 140, 22, LEAF_DARK, "mid")
    for i in range(9):
        x = 160 + i * 24
        b.line(f"reed_{i}", [(x, 812), (x + (i % 3 - 1) * 8, 720 + (i * 17) % 40)], 5, LEAF_DARK, "mid")
    b.ground(950)
    b.stand("dock", 1600, 740)
    b.stand("water_edge", 900, 950)


@background("background.nature.beach_v1", "Hiekkaranta", ["outdoor", "day", "summer", "kesä", "nature", "luonto", "beach", "ranta", "sea", "meri"])
def _(b):
    b.rect("sky", 0, 0, W, H, (200, 220, 228))
    b.oval("sun", 1560, 170, 70, 70, (246, 230, 180))
    b.cloud("cloud", 600, 190, 80)
    b.rect("sea", 0, 520, W, 260, (168, 198, 208))
    b.rect("sea_far", 0, 520, W, 40, (150, 184, 198))
    for i in range(8):
        y = 580 + (i % 4) * 46
        x = 100 + i * 230
        b.line(f"wave_{i}", [(x, y), (x + 40, y - 8), (x + 80, y)], 4, (220, 232, 236))
    b.oval("surf", 960, 790, 1200, 22, (232, 236, 232))
    b.rect("sand", 0, 790, W, 290, (226, 212, 180))
    b.oval("umbrella_shade", 420, 930, 160, 18, (200, 186, 156), "mid")
    b.line("umbrella_pole", [(420, 940), (420, 690)], 8, (120, 110, 100), "mid")
    b.triangle("umbrella", 420, 630, 700, 180, (214, 160, 140), "mid", step=8)
    b.oval("umbrella_tip", 420, 628, 8, 8, (120, 110, 100), "mid")
    b.rect("towel", 560, 900, 220, 40, (150, 182, 196), "mid")
    b.oval("sandcastle", 1400, 920, 70, 34, (206, 186, 144), "mid")
    b.rect("tower", 1380, 860, 40, 50, (206, 186, 144), "mid")
    b.ground(950)
    b.seat("towel", 670, 900)
    b.stand("water", 1000, 950)


@background("background.nature.mountains_v1", "Tunturi", ["outdoor", "day", "nature", "luonto", "mountains", "tunturi", "vuori", "hiking", "retki"])
def _(b):
    b.rect("sky", 0, 0, W, H, (198, 214, 224))
    b.triangle("far_peak_a", 420, 260, 700, 520, (170, 182, 190))
    b.triangle("far_peak_b", 1300, 200, 700, 620, (176, 188, 196))
    b.triangle("snow_a", 420, 260, 360, 118, (238, 240, 242))
    b.triangle("snow_b", 1300, 200, 320, 150, (238, 240, 242))
    b.triangle("near_fell", 900, 430, 760, 900, (160, 170, 150))
    b.rect("plateau", 0, 740, W, 340, (188, 194, 164))
    for i in range(7):
        b.oval(f"heath_{i}", 140 + i * 280, 800 + (i % 2) * 20, 110, 24, (168, 172, 138))
    b.oval("cairn_base", 1500, 900, 70, 30, (160, 158, 152), "mid")
    b.oval("cairn_mid", 1500, 862, 50, 26, (172, 170, 164), "mid")
    b.oval("cairn_top", 1500, 830, 30, 18, (160, 158, 152), "mid")
    b.line("trail", [(200, 1080), (520, 900), (900, 820), (1260, 780)], 16, (210, 202, 182))
    b.ground(950)
    b.stand("cairn", 1500, 950)
    b.stand("trail", 700, 950)


@background("background.nature.meadow_v1", "Kukkaniitty", ["outdoor", "day", "summer", "kesä", "nature", "luonto", "meadow", "niitty", "flowers", "kukat"])
def _(b):
    b.rect("sky", 0, 0, W, H, (196, 216, 222))
    b.cloud("cloud_a", 380, 180, 80)
    b.cloud("cloud_b", 1300, 140, 100)
    b.oval("hill_far", 500, 700, 800, 180, GRASS_FAR)
    b.oval("hill_far_b", 1500, 720, 700, 160, (160, 180, 146))
    b.rect("meadow", 0, 760, W, 320, (180, 198, 150))
    colors = [(226, 170, 180), (238, 236, 226), (230, 200, 120), (190, 170, 210)]
    for i in range(48):
        x = 30 + (i * 397) % 1880
        y = 800 + (i * 131) % 240
        b.oval(f"flower_{i}", x, y, 7, 7, colors[i % 4], "mid" if y > 900 else "far")
    b.leafy_tree("birch", 1640, 880, 520, LEAF, LEAF_DARK, trunk=(230, 228, 220), depth="mid")
    b.oval("birch_shade", 1640, 900, 160, 20, (160, 180, 136), "mid")
    b.ground(950)
    b.stand("tree", 1560, 950)
    b.seat("grass", 900, 940)


@background("background.nature.winter_forest_v1", "Talvimetsä", ["outdoor", "day", "winter", "talvi", "snow", "lumi", "nature", "luonto", "forest", "metsä"])
def _(b):
    b.rect("sky", 0, 0, W, H, (206, 216, 224))
    for i in range(10):
        x = 50 + i * 205
        b.spruce(f"far_spruce_{i}", x, 760, 300 + (i * 41) % 80, (150, 170, 166))
        b.triangle(f"far_snow_{i}", x, 760 - (300 + (i * 41) % 80), 760 - (300 + (i * 41) % 80) * 0.75, 30, (236, 240, 242), step=10)
    b.rect("snowfield", 0, 750, W, 330, (234, 238, 240))
    for i in range(6):
        b.oval(f"drift_{i}", 160 + i * 330, 800 + (i % 2) * 30, 180, 26, (222, 228, 234))
    for i, (x, h) in enumerate(((200, 600), (1700, 640))):
        b.spruce(f"spruce_{i}", x, 910, h, SPRUCE, depth="mid")
        for k in range(3):
            b.oval(f"spruce_{i}_snow_{k}", x, 910 - h * (0.55 + 0.2 * k), h * (0.2 - 0.05 * k), 12, (240, 243, 245), "mid")
    for i in range(30):
        b.oval(f"snowflake_{i}", (i * 263) % 1900 + 10, (i * 157) % 680 + 20, 5, 5, (246, 248, 250))
    b.rect("ski_track_a", 600, 900, 900, 6, (210, 218, 226))
    b.rect("ski_track_b", 600, 920, 900, 6, (210, 218, 226))
    b.ground(950)
    b.stand("clearing", 960, 950)


@background("background.nature.autumn_park_v1", "Syyspuisto", ["outdoor", "day", "autumn", "syksy", "park", "puisto", "nature", "luonto", "leaves", "lehdet"])
def _(b):
    b.rect("sky", 0, 0, W, H, (210, 214, 208))
    b.rect("lawn", 0, 780, W, 300, (190, 186, 150))
    b.rect("path", 0, 900, W, 90, (206, 196, 178))
    autumn = [((210, 160, 104), (186, 134, 90)), ((198, 120, 92), (170, 104, 84)), ((214, 184, 112), (186, 160, 100))]
    for i, x in enumerate((180, 560, 1360, 1740)):
        c, d = autumn[i % 3]
        b.leafy_tree(f"tree_{i}", x, 860, 520 + (i * 37) % 80, c, d, depth="far")
    for i in range(40):
        c, _ = autumn[i % 3]
        b.oval(f"leaf_{i}", (i * 211) % 1900 + 10, 800 + (i * 89) % 180, 9, 5, c)
    b.rect("bench_seat", 860, 830, 220, 16, (150, 120, 92), "mid")
    b.rect("bench_back", 860, 780, 220, 14, (150, 120, 92), "mid")
    b.line("bench_leg_a", [(880, 846), (880, 900)], 8, (90, 90, 90), "mid")
    b.line("bench_leg_b", [(1060, 846), (1060, 900)], 8, (90, 90, 90), "mid")
    b.line("lamp_post", [(1560, 950), (1560, 520)], 12, (90, 96, 100), "mid")
    b.oval("lamp", 1560, 510, 26, 30, (240, 226, 180), "mid")
    b.ground(950)
    b.seat("bench", 970, 830)
    b.stand("lamp", 1500, 950)


@background("background.nature.river_bridge_v1", "Joki ja silta", ["outdoor", "day", "nature", "luonto", "river", "joki", "bridge", "silta"])
def _(b):
    b.rect("sky", 0, 0, W, H, (198, 214, 220))
    b.cloud("cloud", 1500, 160, 80)
    for i in range(10):
        b.leafy_tree(f"far_tree_{i}", 80 + i * 200, 640, 220 + (i * 29) % 60, (150, 172, 146), (138, 160, 136))
    b.rect("bank_far", 0, 620, W, 80, (168, 186, 150))
    b.rect("river", 0, 690, W, 160, (172, 198, 206))
    for i in range(7):
        y = 710 + (i % 3) * 40
        b.line(f"flow_{i}", [(120 + i * 260, y), (220 + i * 260, y)], 4, (204, 222, 228))
    b.rect("bank", 0, 840, W, 240, (178, 192, 150))
    b.oval("arch", 960, 700, 330, 120, (170, 160, 150), "mid")
    b.oval("arch_hole", 960, 740, 250, 90, (172, 198, 206), "mid")
    b.rect("deck", 560, 560, 800, 40, (176, 150, 120), "mid")
    for i in range(11):
        x = 580 + i * 76
        b.line(f"rail_post_{i}", [(x, 560), (x, 500)], 8, (130, 104, 84), "mid")
    b.line("rail", [(570, 500), (1350, 500)], 10, (130, 104, 84), "mid")
    b.ground(950)
    b.stand("bridge", 960, 560)
    b.stand("bank", 300, 950)


# ---------------------------------------------------------------- rural and leisure
@background("background.rural.farm_v1", "Maatila", ["outdoor", "day", "farm", "maatila", "countryside", "maaseutu", "barn", "lato"])
def _(b):
    b.rect("sky", 0, 0, W, H, (200, 216, 222))
    b.cloud("cloud", 500, 150, 90)
    b.oval("hill", 960, 640, 1200, 140, (168, 186, 146))
    for i in range(8):
        y = 600 + i * 22
        b.line(f"field_row_{i}", [(1100, y), (1900, y + 8)], 8, (196, 184, 130))
    b.triangle("barn_roof", 600, 300, 470, 300, (120, 112, 112))
    b.rect("barn", 330, 460, 540, 300, (196, 140, 124))
    b.rect("barn_door", 520, 560, 160, 200, (150, 104, 92))
    b.line("barn_x_a", [(520, 560), (680, 760)], 8, (232, 228, 220))
    b.line("barn_x_b", [(680, 560), (520, 760)], 8, (232, 228, 220))
    b.rect("silo", 900, 360, 110, 400, (196, 198, 196))
    b.oval("silo_top", 955, 360, 55, 40, (176, 178, 176))
    b.rect("yard", 0, 760, W, 320, (196, 188, 158))
    for i in range(10):
        x = 1100 + i * 80
        b.line(f"fence_post_{i}", [(x, 900), (x, 800)], 10, (150, 126, 100), "mid")
    b.line("fence_rail_a", [(1090, 830), (1830, 830)], 8, (150, 126, 100), "mid")
    b.line("fence_rail_b", [(1090, 870), (1830, 870)], 8, (150, 126, 100), "mid")
    b.ground(950)
    b.doorway("barn_door", 520, 560, 160, 200)
    b.stand("fence", 1300, 950)


@background("background.outdoor.playground_v1", "Leikkipuisto", ["outdoor", "day", "playground", "leikkipuisto", "children", "lapset", "park", "puisto"])
def _(b):
    b.rect("sky", 0, 0, W, H, (198, 216, 224))
    b.cloud("cloud", 1500, 160, 90)
    for i in range(6):
        b.leafy_tree(f"tree_{i}", 120 + i * 340, 760, 300 + (i * 23) % 60, (150, 176, 140), (136, 160, 128))
    b.rect("lawn", 0, 740, W, 340, (178, 196, 150))
    b.oval("sandbox_sand", 1500, 930, 220, 36, (222, 206, 170), "mid")
    b.rect("sandbox_edge", 1280, 930, 440, 18, (176, 140, 104), "mid")
    b.line("swing_post_a", [(240, 950), (300, 520)], 14, (110, 130, 150), "mid")
    b.line("swing_post_b", [(660, 950), (600, 520)], 14, (110, 130, 150), "mid")
    b.line("swing_bar", [(280, 520), (620, 520)], 14, (110, 130, 150), "mid")
    for i, x in enumerate((360, 540)):
        b.line(f"swing_chain_{i}a", [(x - 30, 520), (x - 30, 820)], 4, (90, 96, 100), "mid")
        b.line(f"swing_chain_{i}b", [(x + 30, 520), (x + 30, 820)], 4, (90, 96, 100), "mid")
        b.rect(f"swing_seat_{i}", x - 40, 818, 80, 14, (200, 120, 100), "mid")
    b.line("slide_ladder", [(860, 950), (880, 600)], 12, (110, 130, 150), "mid")
    b.rect("slide_top", 870, 590, 110, 18, (110, 130, 150), "mid")
    b.line("slide", [(980, 600), (1160, 930), (1220, 940)], 18, (230, 190, 110), "mid")
    b.ground(950)
    b.seat("swing", 360, 818)
    b.stand("slide_top", 925, 590)
    b.stand("sandbox", 1500, 950)


@background("background.outdoor.harbour_v1", "Satama", ["outdoor", "day", "harbour", "satama", "sea", "meri", "boats", "veneet"])
def _(b):
    b.rect("sky", 0, 0, W, H, (198, 212, 220))
    b.cloud("cloud", 700, 150, 80)
    b.rect("sea", 0, 560, W, 260, (164, 192, 204))
    for i in range(6):
        y = 600 + (i % 3) * 50
        b.line(f"wave_{i}", [(200 + i * 300, y), (290 + i * 300, y)], 4, (200, 218, 226))
    b.rect("ship_hull", 1120, 420, 600, 120, (176, 110, 100))
    b.rect("ship_deck", 1180, 340, 360, 80, (226, 226, 220))
    b.rect("ship_bridge", 1420, 260, 120, 80, (226, 226, 220))
    b.line("ship_mast", [(1300, 340), (1300, 200)], 8, (90, 96, 100))
    b.rect("ship_waterline", 1120, 530, 600, 30, (110, 120, 130))
    b.line("crane_mast", [(300, 790), (300, 260)], 18, (200, 170, 100))
    b.line("crane_jib", [(300, 270), (700, 270)], 14, (200, 170, 100))
    b.line("crane_cable", [(640, 270), (640, 480)], 4, (90, 96, 100))
    b.rect("crane_load", 600, 480, 80, 60, (120, 150, 170))
    b.rect("quay", 0, 790, W, 290, (196, 196, 190))
    b.rect("quay_edge", 0, 790, W, 20, (150, 150, 146))
    for i, x in enumerate((420, 1100, 1700)):
        b.rect(f"bollard_{i}", x - 18, 830, 36, 40, (90, 96, 100), "mid")
        b.oval(f"bollard_top_{i}", x, 830, 26, 10, (90, 96, 100), "mid")
    b.ground(950)
    b.seat("bollard", 1100, 822)
    b.stand("quay_edge", 760, 950)


@background("background.outdoor.train_platform_v1", "Rautatieasema", ["outdoor", "day", "train", "juna", "station", "asema", "travel", "matka"])
def _(b):
    b.rect("sky", 0, 0, W, H, (200, 210, 216))
    b.rect("station_wall", 0, 300, W, 360, (214, 206, 192))
    for i in range(5):
        b.rect(f"station_window_{i}", 120 + i * 380, 360, 200, 160, (186, 206, 214))
    b.rect("roof", 0, 240, W, 60, (150, 156, 160))
    for i in range(6):
        b.line(f"roof_pillar_{i}", [(80 + i * 352, 300), (80 + i * 352, 800)], 16, (130, 136, 140))
    b.rect("clock_body", 900, 120, 120, 120, (236, 234, 226))
    b.line("clock_hand_a", [(960, 180), (960, 140)], 6, INK)
    b.line("clock_hand_b", [(960, 180), (990, 190)], 5, INK)
    b.rect("train", 0, 560, W, 200, (190, 204, 214))
    b.rect("train_stripe", 0, 640, W, 24, (176, 120, 108))
    for i in range(8):
        b.rect(f"train_window_{i}", 60 + i * 240, 590, 150, 40, (150, 172, 184))
    b.rect("train_door", 1500, 580, 110, 170, (166, 180, 190))
    b.rect("tracks", 0, 760, W, 30, (140, 136, 130))
    b.rect("platform", 0, 790, W, 290, (204, 202, 196))
    b.rect("safety_line", 0, 805, W, 12, (230, 206, 120))
    b.rect("board", 640, 430, 240, 70, (70, 80, 90), "mid")
    b.rect("bench_seat", 260, 880, 220, 16, (150, 120, 92), "mid")
    b.line("bench_leg_a", [(280, 896), (280, 948)], 8, (90, 96, 100), "mid")
    b.line("bench_leg_b", [(460, 896), (460, 948)], 8, (90, 96, 100), "mid")
    b.ground(950)
    b.doorway("train_door", 1500, 580, 110, 170)
    b.screen("departures", 640, 430, 240, 70)
    b.seat("bench", 370, 880)


# ---------------------------------------------------------------- fantasy and sci-fi
@background("background.fantasy.castle_courtyard_v1", "Linnan piha", ["outdoor", "day", "fantasy", "fantasia", "castle", "linna", "medieval", "keskiaika"])
def _(b):
    b.rect("sky", 0, 0, W, H, (204, 214, 222))
    stone, stone_dark = (196, 192, 184), (170, 166, 158)
    b.rect("wall", 0, 380, W, 420, stone)
    for i in range(20):
        b.rect(f"merlon_{i}", i * 100, 340, 60, 50, stone)
    for i, x in enumerate((160, 1640)):
        b.rect(f"tower_{i}", x - 120, 160, 240, 640, stone_dark)
        b.triangle(f"tower_roof_{i}", x, 40, 170, 150, (160, 112, 108))
        b.rect(f"tower_window_{i}", x - 24, 300, 48, 90, (110, 116, 120))
        b.line(f"flag_pole_{i}", [(x, 40), (x, -10)], 6, (90, 90, 90))
    for row in range(5):
        for k in range(12):
            x = 300 + k * 110 + (55 if row % 2 else 0)
            b.rect(f"brick_{row}_{k}", x, 420 + row * 70, 90, 6, stone_dark)
    b.oval("gate_arch", 960, 540, 150, 140, (120, 104, 92))
    b.rect("gate", 810, 540, 300, 260, (120, 104, 92))
    for i in range(6):
        x = 840 + i * 48
        b.line(f"portcullis_{i}", [(x, 440), (x, 800)], 6, (90, 92, 96))
    b.rect("courtyard", 0, 800, W, 280, (204, 196, 178))
    b.oval("well", 1460, 900, 110, 40, stone_dark, "mid")
    b.rect("well_body", 1350, 860, 220, 40, stone_dark, "mid")
    b.line("well_post_a", [(1370, 860), (1370, 720)], 12, (130, 104, 84), "mid")
    b.line("well_post_b", [(1550, 860), (1550, 720)], 12, (130, 104, 84), "mid")
    b.line("well_beam", [(1360, 720), (1560, 720)], 12, (130, 104, 84), "mid")
    b.ground(950)
    b.doorway("gate", 810, 540, 300, 260)
    b.stand("well", 1300, 950)


@background("background.scifi.space_station_v1", "Avaruusasema", ["interior", "scifi", "space", "avaruus", "station", "asema", "future", "tulevaisuus"])
def _(b):
    b.rect("wall", 0, 0, W, H, (206, 212, 218))
    b.rect("ceiling", 0, 0, W, 120, (180, 188, 196))
    for i in range(8):
        b.rect(f"ceiling_light_{i}", 60 + i * 240, 100, 160, 14, (236, 240, 230))
    b.oval("window", 960, 400, 360, 210, (150, 160, 180))
    b.oval("planet", 1100, 440, 120, 120, (190, 170, 150))
    b.oval("planet_ring", 1100, 440, 190, 24, (210, 196, 176))
    for i in range(14):
        b.oval(f"star_{i}", 680 + (i * 157) % 560, 260 + (i * 97) % 280, 3, 3, (240, 240, 230))
    b.rect("window_frame_top", 600, 182, 720, 14, (150, 160, 170))
    b.rect("window_frame_bottom", 600, 604, 720, 14, (150, 160, 170))
    for i, x in enumerate((140, 1600)):
        b.rect(f"panel_{i}", x, 260, 180, 300, (186, 194, 202))
        b.rect(f"panel_screen_{i}", x + 20, 290, 140, 90, (120, 160, 170))
        for k in range(3):
            b.oval(f"panel_button_{i}_{k}", x + 40 + k * 50, 430, 12, 12, ((200, 120, 100), (220, 196, 120), (140, 180, 130))[k])
    b.rect("door", 1380, 360, 160, 440, (176, 186, 196))
    b.line("door_seam", [(1460, 360), (1460, 800)], 6, (140, 150, 160))
    b.rect("floor", 0, 800, W, 280, (190, 196, 202))
    for i in range(9):
        b.line(f"floor_seam_{i}", [(i * 240, 800), (i * 240 - 60, 1080)], 4, (170, 176, 184))
    b.ground(950)
    b.doorway("door", 1380, 360, 160, 440)
    b.screen("console_left", 160, 290, 140, 90)
    b.screen("console_right", 1620, 290, 140, 90)
    b.stand("window", 960, 950)


@background("background.scifi.alien_planet_v1", "Vieras planeetta", ["outdoor", "scifi", "space", "avaruus", "planet", "planeetta", "alien", "olio"])
def _(b):
    b.rect("sky", 0, 0, W, H, (214, 204, 222))
    b.oval("moon_big", 1500, 220, 160, 160, (226, 216, 200))
    b.oval("moon_crater", 1450, 200, 30, 24, (210, 200, 186))
    b.oval("moon_small", 380, 160, 50, 50, (200, 214, 222))
    b.oval("dune_far", 500, 720, 900, 160, (196, 184, 204))
    b.oval("dune_far_b", 1500, 740, 800, 150, (188, 178, 198))
    b.rect("ground", 0, 780, W, 300, (210, 196, 186))
    for i in range(6):
        b.oval(f"crater_{i}", 160 + i * 330, 840 + (i % 2) * 60, 90, 18, (190, 176, 168))
    for i, (x, h) in enumerate(((260, 300), (1700, 360))):
        b.line(f"plant_stem_{i}", [(x, 900), (x - 20, 900 - h)], 14, (140, 170, 150), "mid")
        b.oval(f"plant_bulb_{i}", x - 20, 900 - h, 44, 44, (160, 200, 170), "mid")
    b.rect("lander_body", 880, 640, 240, 160, (210, 214, 218), "mid")
    b.oval("lander_dome", 1000, 640, 120, 70, (186, 206, 218), "mid")
    b.line("lander_leg_a", [(900, 790), (840, 930)], 14, (150, 156, 160), "mid")
    b.line("lander_leg_b", [(1100, 790), (1160, 930)], 14, (150, 156, 160), "mid")
    b.rect("lander_hatch", 960, 700, 80, 100, (176, 184, 192), "mid")
    b.ground(950)
    b.doorway("hatch", 960, 700, 80, 100)
    b.stand("crater", 500, 950)


# ---------------------------------------------------------------- checks and output
def saturation(c):
    hi, lo = max(c[:3]), min(c[:3])
    return 0.0 if hi == 0 else (hi - lo) / hi


def luminance(c):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(v) for v in c[:3])
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def check(bid, m):
    """Mirror of motion_art::style::check_background (stick_studio, actor scale 1)."""
    errors = []
    limb = luminance(LIMB_INK)
    ids = set()
    for layer in m["geometry"]["layers"]:
        if layer["id"] in ids:
            errors.append(f"duplicate layer {layer['id']}")
        ids.add(layer["id"])
        k = layer["content"]
        c = (k["color"]["r"], k["color"]["g"], k["color"]["b"])
        if k["type"] == "stroke":
            if k["width"] > MAX_LINE:
                errors.append(f"{layer['id']}: stroke {k['width']} > {MAX_LINE}")
            continue
        r = k["rect"]
        w = min(r["x"] + r["width"], W) - max(r["x"], 0)
        h = min(r["y"] + r["height"], H) - max(r["y"], 0)
        if w <= 0 or h <= 0 or w * h / (W * H) < 0.20:
            continue
        if saturation(c) > 0.35:
            errors.append(f"{layer['id']}: saturated large surface")
        l = luminance(c)
        if (max(l, limb) + 0.05) / (min(l, limb) + 0.05) < 4.5:
            errors.append(f"{layer['id']}: low limb contrast")
    if len(m["geometry"]["layers"]) > 1024:
        errors.append("too many layers")
    return [f"{bid}: {e}" for e in errors]


def build():
    out = {}
    for bid, (name, tags, fn) in BACKGROUNDS.items():
        b = Bg()
        fn(b)
        out[bid] = {
            "schema_version": 1, "id": bid, "kind": "background", "name": name,
            "bounds": {"x": 0.0, "y": 0.0, "width": W, "height": H}, "depth": "far",
            "tags": tags, "license": LICENSE,
            "geometry": {"type": "art", "layers": b.layers}, "anchors": b.anchors,
        }
    return out


def main(argv):
    manifests = build()
    errors = [e for bid, m in manifests.items() for e in check(bid, m)]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    texts = {bid: json.dumps(m, indent=2, ensure_ascii=False) + "\n" for bid, m in manifests.items()}
    if "--check" in argv:
        bad = [bid for bid, t in texts.items() if not (OUT_DIR / f"{bid}.json").is_file()
               or (OUT_DIR / f"{bid}.json").read_text(encoding="utf-8") != t]
        if bad:
            print(f"out of date: {bad}", file=sys.stderr)
            return 1
        print(f"ok: {len(texts)} background manifests up to date")
        return 0
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for bid, t in texts.items():
        (OUT_DIR / f"{bid}.json").write_text(t, encoding="utf-8")
    print(f"wrote {len(texts)} background manifests, "
          f"{sum(len(m['geometry']['layers']) for m in manifests.values())} layers")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
