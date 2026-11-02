"""Original, editable vector city for the Kilsat stick-figure cast.

Nine city sets (shopping street, residential street, market square, corner
shop, petrol station, park, rooftop, back alley, metro station) and a library
of city props (vehicles, street furniture and buildings). Every set is drawn
for the four camera angles of docs/camera-angles.md: front and reverse by hand,
high and low by moving the horizon (see `stick_scenery._with_angles`).

Sets reuse the prop drawings: a car parked in a street is the library car,
scaled, mirrored and renamed. Layers follow the stick-studio background style
(docs/stick-studio-art-direction.md § 3): large surfaces stay light and muted,
and no stroke is wider than two limb widths at actor scale 1.6 (28.8 px).
There are no fonts, linked images or downloads; shop lettering is abstract.
"""

from math import hypot

from stick_scenery import (
    AMBER, INK, METAL, PAPER, RUST, TEAL, TEAL_LIGHT, WOOD,
    _clip, _disc, _ellipse, _floor_seams, _panel, _rect, _stroke, _with_angles,
)

BG = "background"

# Large surfaces: light and muted so dark limbs stay readable.
SKY = (190, 214, 212, 255)
SKY_FAR = (166, 194, 194, 255)
DUSK = (226, 198, 172, 255)
NIGHT_CITY = (128, 150, 172, 255)
PAVEMENT = (200, 198, 186, 255)
PAVEMENT_SEAM = (178, 176, 164, 255)
SQUARE = (208, 200, 182, 255)
CONCRETE = (196, 194, 186, 255)
ROOF = (158, 160, 156, 255)
ALLEY = (152, 156, 154, 255)
GRASS = (152, 178, 132, 255)
SAND = (224, 208, 178, 255)
ROSE = (216, 186, 172, 255)
PLASTER = (178, 200, 190, 255)
STONE = (206, 202, 190, 255)
BRICK_LIGHT = (198, 158, 136, 255)
ALLEY_BRICK = (178, 152, 140, 255)
SHOP_WALL = (234, 230, 216, 255)
SHOP_FLOOR = (208, 206, 194, 255)
TILE = (214, 222, 214, 255)
STORE = (228, 216, 198, 255)
# Small accents may be stronger.
ASPHALT = (104, 114, 118, 255)
CURB = (168, 170, 162, 255)
LANE = (226, 224, 204, 255)
GLASS = TEAL_LIGHT
GLASS_DARK = (96, 128, 132, 255)
GLASS_SEE = (157, 194, 185, 150)
LEAF = (100, 143, 122, 255)
LEAF_LIGHT = (128, 166, 126, 255)
LEAF_DARK = (76, 116, 100, 255)
WATER = (128, 172, 178, 255)
RED = (184, 70, 60, 255)
RED_DARK = (140, 52, 46, 255)
BLUE = (70, 112, 156, 255)
CAR_BLUE = (94, 145, 174, 255)
TAXI = (228, 188, 88, 255)
WHITE = (240, 240, 232, 255)
GREEN = (84, 146, 104, 255)
ROOF_TILE = (150, 82, 64, 255)
DARK_WINDOW = (60, 84, 92, 255)
SKYLINE = (84, 100, 118, 255)
SKYLINE_TWO = (98, 114, 132, 255)
LAMP_GLOW = (238, 211, 134, 40)


def _darker(color, k=0.78):
    return tuple(round(c * k) for c in color[:3]) + (color[3],)


def _fill_points(polygon, width):
    """Zig-zag through a convex polygon so one stroke of `width` fills it.

    The polygon is inset by half the stroke width and scanned in rows closer
    than half the width, so the filled area keeps its edges and stays covered
    after the high/low angle stretches the floor by up to 1.63.
    """
    n = len(polygon)
    area = sum(polygon[i][0] * polygon[(i + 1) % n][1] - polygon[(i + 1) % n][0] * polygon[i][1]
               for i in range(n))
    sign = 1 if area > 0 else -1
    d = width / 2
    lines = []
    for i in range(n):
        (x1, y1), (x2, y2) = polygon[i], polygon[(i + 1) % n]
        length = hypot(x2 - x1, y2 - y1)
        # Inward normal for this winding.
        nx, ny = -(y2 - y1) / length * sign, (x2 - x1) / length * sign
        lines.append(((x1 + nx * d, y1 + ny * d), (x2 - x1, y2 - y1)))
    inset = []
    for i in range(n):
        (p, r), (q, s) = lines[i - 1], lines[i]
        cross = r[0] * s[1] - r[1] * s[0]
        t = ((q[0] - p[0]) * s[1] - (q[1] - p[1]) * s[0]) / cross
        inset.append((p[0] + t * r[0], p[1] + t * r[1]))
    top = min(y for _, y in inset)
    bottom = max(y for _, y in inset)
    rows = max(1, int((bottom - top) / (width * 0.45)) + 1)
    points = []
    for k in range(rows + 1):
        y = top + (bottom - top) * k / rows
        xs = []
        for i in range(n):
            (ax, ay), (bx, by) = inset[i], inset[(i + 1) % n]
            if (ay - y) * (by - y) <= 0 and ay != by:
                xs.append(ax + (y - ay) * (bx - ax) / (by - ay))
        if not xs:
            continue
        left, right = min(xs), max(xs)
        row = [(round(left, 1), round(y, 1)), (round(right, 1), round(y, 1))]
        points.extend(row if k % 2 == 0 else row[::-1])
    # Trace the inset outline last so the edges are straight, not scalloped.
    points.extend((round(x, 1), round(y, 1)) for x, y in inset + inset[:1])
    return points


class Paint:
    """Collects layers with one id prefix; every set or prop owns one."""

    def __init__(self, prefix, plane="foreground"):
        self.prefix, self.plane, self.layers = prefix, plane, []

    def _id(self, name):
        return f"{self.prefix}_{name}"

    def rect(self, name, x, y, w, h, color):
        self.layers.append(_rect(self._id(name), x, y, w, h, color, self.plane))

    def oval(self, name, x, y, w, h, color):
        self.layers.append(_ellipse(self._id(name), x, y, w, h, color, self.plane))

    def line(self, name, points, width, color=INK):
        self.layers.append(_stroke(self._id(name), points, width, color, self.plane))

    def panel(self, name, x, y, w, h, color, border=7):
        self.layers.extend(_panel(self._id(name), x, y, w, h, color, border, self.plane))

    def disc(self, name, x, y, w, h, color, border=7):
        self.layers.extend(_disc(self._id(name), x, y, w, h, color, border, self.plane))

    def poly(self, name, polygon, color, width=24):
        """A filled convex polygon (roofs, gables, awnings, paths)."""
        self.line(name, _fill_points(polygon, width), width, color)

    def clip(self, x, y, w, h):
        _clip(self.layers[-1], x, y, w, h)

    def use(self, key, name, x, ground, height, flip=False, recolor=None, lean=True):
        """Place library prop `key` with its left edge at `x` and its base on `ground`."""
        prop = CITY_PROPS[key]
        scale = height / prop["height"]
        recolor = recolor or {}
        for layer in prop["layers"]:
            suffix = layer["id"][len(key) + 1:]
            if lean and suffix in prop.get("detail", ()):
                continue
            copy = {**layer, "id": self._id(f"{name}_{suffix}"), "plane": self.plane,
                    "content": dict(layer["content"])}
            content = copy["content"]
            color = tuple(content["color"][k] for k in "rgba")
            if color in recolor:
                content["color"] = dict(zip("rgba", recolor[color]))
            if "rect" in content:
                content["rect"] = _place_rect(content["rect"], prop["width"], x, ground - height,
                                              scale, flip)
            if "points" in content:
                content["points"] = [_place_point(p, prop["width"], x, ground - height, scale, flip)
                                     for p in content["points"]]
                content["width"] = round(content["width"] * scale, 2)
            if layer.get("clip"):
                copy["clip"] = _place_rect(layer["clip"], prop["width"], x, ground - height,
                                           scale, flip)
            self.layers.append(copy)


def _place_rect(r, width, ox, oy, s, flip):
    x = width - r["x"] - r["width"] if flip else r["x"]
    return dict(x=round(ox + x * s, 2), y=round(oy + r["y"] * s, 2),
                width=round(r["width"] * s, 2), height=round(r["height"] * s, 2))


def _place_point(p, width, ox, oy, s, flip):
    x = width - p["x"] if flip else p["x"]
    return dict(x=round(ox + x * s, 2), y=round(oy + p["y"] * s, 2))


# ---------------------------------------------------------------------------
# City props. Local coordinates, base on y = height, vehicles face right.
# `detail` lists layer suffixes dropped when a set places the prop small.


def _prop(name, width, height, paint, detail=()):
    return dict(name=name, width=width, height=height, layers=paint.layers, detail=set(detail))


def _car(key, name, body, kind="sedan"):
    p = Paint(key)
    if kind == "taxi":
        p.panel("roof_sign", 286, 2, 74, 34, PAPER, 5)
    if kind == "police":
        p.rect("light_blue", 268, 10, 50, 22, BLUE)
        p.rect("light_red", 318, 10, 50, 22, RED)
    p.oval("cabin_outline", 150, 28, 330, 210, INK)
    p.clip(150, 28, 330, 104)
    p.oval("cabin_fill", 159, 37, 312, 192, body)
    p.clip(159, 37, 312, 95)
    p.oval("glass", 180, 55, 270, 160, GLASS)
    p.clip(180, 55, 270, 72)
    p.line("pillar", [(318, 58), (318, 128)], 12, body)
    p.rect("body_outline", 18, 124, 604, 112, INK)
    p.rect("body_fill", 26, 132, 588, 96, body)
    p.rect("sill", 26, 206, 588, 22, _darker(body))
    if kind == "taxi":
        p.rect("checker", 26, 172, 588, 10, INK)
    if kind == "police":
        p.rect("stripe", 26, 164, 588, 24, BLUE)
    p.line("door_seam", [(318, 136), (318, 204)], 4, INK)
    p.line("handle", [(334, 152), (362, 152)], 6, INK)
    p.rect("headlamp", 590, 146, 24, 22, AMBER)
    p.rect("tail_light", 26, 146, 16, 26, RUST)
    p.disc("wheel_back", 100, 192, 98, 98, METAL, 20)
    p.disc("wheel_front", 442, 192, 98, 98, METAL, 20)
    return _prop(name, 640, 290, p, ("door_seam", "handle", "sill"))


def _van(key, name, body, accent, ambulance=False):
    p = Paint(key)
    if ambulance:
        p.rect("light_red", 560, 74, 50, 22, RED)
        p.rect("light_blue", 610, 74, 50, 22, BLUE)
    p.panel("cargo", 16, 30, 474, 240, body, 8)
    p.panel("cab", 470, 96, 210, 174, body, 8)
    p.rect("glass", 496, 114, 162, 70, GLASS)
    p.line("pillar", [(560, 114), (560, 184)], 10, body)
    p.rect("stripe", 24, 168, 648, 26, accent)
    if ambulance:
        p.rect("cross_v", 214, 66, 36, 96, RED)
        p.rect("cross_h", 184, 96, 96, 36, RED)
    p.line("door_seam", [(300, 40), (300, 262)], 4, INK)
    p.rect("bumper", 12, 262, 672, 14, INK)
    p.rect("headlamp", 652, 204, 22, 24, AMBER)
    p.rect("tail_light", 22, 200, 16, 30, RUST)
    p.disc("wheel_back", 90, 232, 98, 98, METAL, 20)
    p.disc("wheel_front", 520, 232, 98, 98, METAL, 20)
    return _prop(name, 700, 330, p, ("door_seam", "tail_light"))


def _bus():
    p = Paint("linja_auto")
    p.panel("body", 10, 20, 1160, 320, TEAL, 10)
    p.rect("window_band", 28, 58, 1124, 128, INK)
    for i in range(5):
        p.rect(f"window_{i}", 40 + i * 192, 68, 172, 108, GLASS)
    p.rect("door", 1000, 68, 140, 252, GLASS_DARK)
    p.line("door_seam", [(1070, 76), (1070, 312)], 6, INK)
    p.rect("destination", 1004, 28, 132, 28, AMBER)
    p.rect("stripe", 20, 236, 970, 22, PAPER)
    p.rect("skirt", 20, 300, 1140, 34, _darker(TEAL))
    p.rect("headlamp", 1146, 262, 16, 30, AMBER)
    p.disc("wheel_back", 150, 290, 100, 100, METAL, 20)
    p.disc("wheel_front", 880, 290, 100, 100, METAL, 20)
    return _prop("Kaupunkibussi", 1180, 390, p, ("door_seam", "stripe", "headlamp"))


def _fire_truck():
    p = Paint("paloauto")
    p.rect("light_bar", 800, 24, 150, 18, AMBER)
    p.panel("cab", 760, 40, 230, 260, RED, 9)
    p.rect("windshield", 850, 62, 118, 92, GLASS)
    p.panel("body", 10, 140, 770, 160, RED, 9)
    p.rect("lockers", 40, 170, 700, 76, RED_DARK)
    p.line("locker_seams", [(270, 172), (270, 244)], 5, INK)
    p.line("locker_seams_two", [(500, 172), (500, 244)], 5, INK)
    p.line("ladder_rail_top", [(40, 104), (760, 104)], 9, METAL)
    p.line("ladder_rail_bottom", [(40, 134), (760, 134)], 9, METAL)
    p.line("ladder_rungs", [(40 + i * 40, 104 if i % 2 else 134) for i in range(19)], 6, METAL)
    p.rect("stripe", 18, 270, 964, 14, PAPER)
    p.rect("headlamp", 966, 220, 18, 30, AMBER)
    p.disc("wheel_back", 100, 282, 98, 98, METAL, 20)
    p.disc("wheel_middle", 420, 282, 98, 98, METAL, 20)
    p.disc("wheel_front", 820, 282, 98, 98, METAL, 20)
    return _prop("Paloauto", 1000, 380, p, ("locker_seams", "locker_seams_two", "headlamp"))


def _scooter():
    p = Paint("skootteri")
    p.rect("box", 34, 104, 74, 62, PAPER)
    p.line("frame", [(66, 222), (160, 222), (232, 150), (250, 74)], 22, RED)
    p.rect("seat", 82, 150, 118, 26, INK)
    p.line("handlebar", [(250, 74), (272, 56), (300, 56)], 9, INK)
    p.oval("headlamp", 254, 84, 32, 24, AMBER)
    p.disc("wheel_back", 26, 190, 80, 80, METAL, 16)
    p.disc("wheel_front", 214, 190, 80, 80, METAL, 16)
    return _prop("Skootteri", 320, 270, p)


def _traffic_light():
    p = Paint("liikennevalo")
    p.line("pole", [(100, 744), (100, 300)], 18, INK)
    p.panel("housing", 40, 20, 120, 300, (58, 66, 66, 255), 8)
    p.oval("red", 66, 44, 68, 68, (222, 76, 62, 255))
    p.oval("amber", 66, 124, 68, 68, (124, 108, 66, 255))
    p.oval("green", 66, 204, 68, 68, (64, 104, 84, 255))
    p.rect("button", 80, 430, 40, 60, AMBER)
    p.rect("base", 66, 740, 68, 20, INK)
    return _prop("Liikennevalo", 200, 760, p, ("button",))


def _hydrant():
    p = Paint("paloposti")
    p.oval("cap", 40, 8, 80, 44, RED_DARK)
    p.rect("body", 44, 30, 72, 164, RED)
    p.rect("nozzle_left", 14, 82, 32, 34, RED_DARK)
    p.rect("nozzle_right", 114, 82, 32, 34, RED_DARK)
    p.rect("band", 36, 58, 88, 14, METAL)
    p.rect("base", 28, 190, 104, 30, INK)
    return _prop("Paloposti", 160, 220, p)


def _parking_meter():
    p = Paint("pysakointimittari")
    p.line("post", [(60, 430), (60, 176)], 14, INK)
    p.panel("head", 18, 28, 84, 152, (96, 112, 116, 255), 6)
    p.rect("display", 32, 52, 56, 36, GLASS)
    p.rect("slot", 50, 108, 20, 8, INK)
    p.rect("card", 40, 134, 40, 10, AMBER)
    p.rect("base", 38, 424, 44, 16, INK)
    return _prop("Pysäköintimittari", 120, 440, p)


def _atm():
    p = Paint("pankkiautomaatti")
    p.panel("unit", 20, 20, 260, 440, STONE, 8)
    p.rect("sign", 40, 40, 220, 50, TEAL)
    p.line("sign_lettering", [(70, 66), (100, 58), (130, 72), (160, 58), (190, 72), (230, 62)], 6, PAPER)
    p.rect("screen", 70, 118, 160, 110, GLASS)
    p.rect("ledge", 40, 240, 220, 14, INK)
    p.rect("keypad", 96, 264, 96, 80, METAL)
    p.rect("card_slot", 208, 276, 40, 10, INK)
    p.rect("cash_slot", 80, 372, 140, 14, INK)
    return _prop("Pankkiautomaatti", 300, 480, p)


def _vending():
    p = Paint("juoma_automaatti")
    p.panel("body", 10, 10, 300, 580, RUST, 9)
    for i, color in enumerate((AMBER, PAPER, TEAL, (214, 120, 96, 255))):
        p.rect(f"row_{i}", 44, 70 + i * 96, 170, 50, color)
    p.rect("glass", 34, 40, 190, 420, GLASS_SEE)
    p.rect("buttons", 238, 40, 54, 200, INK)
    p.rect("coin", 248, 270, 34, 44, METAL)
    p.rect("flap", 40, 490, 180, 60, INK)
    return _prop("Juoma-automaatti", 320, 600, p)


def _tree():
    p = Paint("katupuu")
    p.line("trunk", [(190, 632), (192, 420), (180, 300)], 24, INK)
    p.line("branch", [(190, 430), (250, 330)], 14, INK)
    p.oval("crown_back", 40, 70, 300, 280, LEAF)
    p.oval("crown_left", 14, 180, 190, 170, LEAF_LIGHT)
    p.oval("crown_right", 186, 150, 180, 190, LEAF_DARK)
    p.oval("crown_top", 110, 24, 170, 150, LEAF_LIGHT)
    p.rect("grate", 116, 626, 148, 14, INK)
    return _prop("Katupuu", 380, 640, p, ("branch", "crown_top", "grate"))


def _bench():
    p = Paint("puistonpenkki")
    p.rect("back_top", 40, 30, 400, 26, WOOD)
    p.rect("back_low", 40, 74, 400, 26, WOOD)
    p.line("frame_left", [(70, 20), (70, 148), (46, 244)], 12, INK)
    p.line("frame_right", [(410, 20), (410, 148), (434, 244)], 12, INK)
    p.rect("seat", 24, 130, 432, 26, WOOD)
    p.line("leg_front", [(90, 156), (110, 244)], 10, INK)
    return _prop("Puistonpenkki", 480, 250, p, ("leg_front",))


def _crosswalk_sign():
    p = Paint("suojatiemerkki")
    p.line("post", [(110, 632), (110, 190)], 16, METAL)
    p.panel("plate", 30, 20, 160, 160, BLUE, 8)
    p.poly("triangle", [(52, 158), (110, 48), (168, 158)], PAPER, 14)
    p.oval("walker_head", 104, 78, 16, 16, INK)
    p.line("walker_body", [(110, 96), (106, 122), (92, 146)], 6, INK)
    p.line("walker_step", [(106, 122), (124, 146)], 6, INK)
    p.line("walker_arms", [(96, 112), (110, 102), (124, 110)], 5, INK)
    return _prop("Suojatiemerkki", 220, 640, p)


def _house(key="omakotitalo", name="Omakotitalo", wall=(214, 196, 160, 255), roof=ROOF_TILE,
           door=TEAL):
    p = Paint(key)
    p.rect("chimney", 470, 96, 54, 130, BRICK_LIGHT)
    p.panel("body", 60, 260, 600, 380, wall, 8)
    p.poly("gable", [(64, 266), (360, 66), (656, 266)], wall, 24)
    p.line("roof_outline", [(36, 282), (360, 50), (684, 282)], 28, INK)
    p.line("roof", [(40, 279), (360, 56), (680, 279)], 16, roof)
    p.disc("attic_window", 330, 150, 60, 60, GLASS, 6)
    p.panel("window_left", 116, 330, 150, 130, GLASS, 8)
    p.line("window_left_mullion", [(191, 338), (191, 452)], 6, INK)
    p.panel("window_right", 454, 330, 150, 130, GLASS, 8)
    p.line("window_right_mullion", [(529, 338), (529, 452)], 6, INK)
    p.panel("door", 306, 420, 110, 220, door, 8)
    p.oval("knob", 392, 528, 12, 12, AMBER)
    p.rect("step", 290, 626, 142, 14, STONE)
    return _prop(name, 720, 640, p, ("knob", "window_left_mullion", "window_right_mullion"))


def _apartments(key="kerrostalo", name="Kerrostalo", wall=STONE, lit=(1, 5, 6, 10, 13)):
    p = Paint(key)
    p.panel("body", 20, 40, 600, 960, wall, 9)
    p.rect("roof_edge", 8, 28, 624, 24, INK)
    for row in range(5):
        for col in range(3):
            i = row * 3 + col
            color = AMBER if i in lit else GLASS
            p.rect(f"window_{i}", 70 + col * 190, 96 + row * 156, 120, 104, color)
    p.rect("canopy", 236, 856, 168, 18, INK)
    p.panel("door", 262, 872, 116, 128, GLASS_DARK, 7)
    return _prop(name, 640, 1000, p, ("canopy", "roof_edge"))


def _corner_shop():
    p = Paint("kulmakauppa")
    p.rect("body", 20, 24, 680, 736, BRICK_LIGHT)
    p.rect("cornice", 8, 12, 704, 28, INK)
    for i in range(3):
        p.rect(f"upper_window_{i}", 74 + i * 214, 86, 148, 150, GLASS)
    p.panel("sign", 40, 290, 640, 74, AMBER, 7)
    p.line("sign_lettering", [(110, 330), (150, 312), (190, 342), (240, 312), (290, 340),
                              (340, 314), (390, 342), (440, 316), (490, 338)], 8, INK)
    p.rect("awning", 30, 376, 660, 64, RUST)
    for i in range(3):
        p.rect(f"awning_stripe_{i}", 96 + i * 220, 376, 64, 64, PAPER)
    p.panel("window", 56, 470, 390, 270, GLASS, 8)
    p.oval("fruit_one", 90, 680, 90, 50, AMBER)
    p.oval("fruit_two", 196, 684, 86, 46, (184, 96, 72, 255))
    p.oval("fruit_three", 300, 682, 100, 48, LEAF_LIGHT)
    p.panel("door", 486, 468, 170, 292, TEAL, 8)
    p.rect("door_glass", 512, 496, 118, 144, GLASS)
    return _prop("Kulmakauppa", 720, 760, p, ("fruit_three", "door_glass"))


def _tower():
    p = Paint("pilvenpiirtaja")
    p.line("spire", [(220, 96), (220, 6)], 10, INK)
    p.oval("beacon", 208, 0, 24, 24, RED)
    p.rect("crown", 78, 86, 284, 64, INK)
    p.panel("body", 40, 140, 360, 1260, (128, 156, 166, 255), 8)
    for i in range(4):
        p.line(f"glass_{i}", [(98 + i * 82, 176), (98 + i * 82, 1370)], 24, GLASS)
    p.rect("lobby", 120, 1300, 200, 100, AMBER)
    return _prop("Pilvenpiirtäjä", 440, 1400, p, ("beacon",))


def _fuel_pump():
    p = Paint("polttoainemittari")
    p.panel("body", 30, 40, 180, 430, PAPER, 8)
    p.rect("top", 38, 48, 164, 60, RED)
    p.rect("display", 60, 130, 120, 70, INK)
    p.line("digits", [(76, 150), (96, 150), (96, 180), (76, 180), (120, 180), (120, 150), (160, 150),
                      (160, 180)], 5, AMBER)
    p.line("hose", [(196, 262), (232, 340), (222, 420), (200, 446)], 8, INK)
    p.rect("holster", 180, 236, 40, 84, INK)
    p.rect("base", 18, 462, 204, 18, INK)
    return _prop("Polttoainemittari", 240, 480, p, ("digits",))


def _stall():
    p = Paint("torikoju")
    p.line("post_left", [(40, 556), (40, 150)], 12, INK)
    p.line("post_right", [(520, 556), (520, 150)], 12, INK)
    p.poly("roof", [(4, 96), (280, 20), (556, 96)], RUST, 20)
    p.rect("awning", 10, 92, 540, 90, PAPER)
    for i in range(3):
        p.rect(f"awning_stripe_{i}", 80 + i * 160, 92, 80, 90, RUST)
    p.line("awning_edge", [(10, 182), (550, 182)], 6, INK)
    p.rect("price", 210, 240, 140, 64, PAPER)
    p.line("price_mark", [(232, 272), (260, 258), (290, 284), (326, 262)], 6, INK)
    p.oval("goods_one", 74, 316, 120, 60, AMBER)
    p.oval("goods_two", 180, 312, 110, 64, (184, 96, 72, 255))
    p.oval("goods_three", 280, 318, 110, 58, LEAF_LIGHT)
    p.oval("goods_four", 376, 314, 110, 62, (218, 196, 120, 255))
    p.panel("counter", 56, 356, 448, 200, WOOD, 8)
    return _prop("Torikoju", 560, 560, p, ("price_mark", "goods_four"))


def _city_props():
    return {
        "henkiloauto": _car("henkiloauto", "Henkilöauto", CAR_BLUE),
        "taksi": _car("taksi", "Taksi", TAXI, "taxi"),
        "poliisiauto": _car("poliisiauto", "Poliisiauto", WHITE, "police"),
        "pakettiauto": _van("pakettiauto", "Pakettiauto", (196, 204, 198, 255), RUST),
        "ambulanssi": _van("ambulanssi", "Ambulanssi", WHITE, RED, ambulance=True),
        "linja_auto": _bus(),
        "paloauto": _fire_truck(),
        "skootteri": _scooter(),
        "liikennevalo": _traffic_light(),
        "paloposti": _hydrant(),
        "pysakointimittari": _parking_meter(),
        "pankkiautomaatti": _atm(),
        "juoma_automaatti": _vending(),
        "katupuu": _tree(),
        "puistonpenkki": _bench(),
        "suojatiemerkki": _crosswalk_sign(),
        "omakotitalo": _house(),
        "kerrostalo": _apartments(),
        "kulmakauppa": _corner_shop(),
        "pilvenpiirtaja": _tower(),
        "polttoainemittari": _fuel_pump(),
        "torikoju": _stall(),
    }


CITY_PROPS = _city_props()


# ---------------------------------------------------------------------------
# Shared set pieces.


def _near_street(p):
    """Pavement the actors stand on, kerb and the near edge of the road."""
    p.rect("pavement", 0, 810, 1920, 270, PAVEMENT)
    p.line("pavement_back", [(0, 812), (1920, 812)], 4, PAVEMENT_SEAM)
    p.line("paving_seam_0", [(500, 814), (180, 1018)], 2, PAVEMENT_SEAM)
    p.line("paving_seam_1", [(1420, 814), (1760, 1018)], 2, PAVEMENT_SEAM)
    p.line("paving_cross", [(0, 900), (1920, 900)], 2, PAVEMENT_SEAM)
    p.rect("kerb", 0, 1020, 1920, 12, CURB)
    p.rect("road", 0, 1032, 1920, 48, ASPHALT)


def _far_street(p, top=640, near=PAVEMENT):
    """Looking across the road: far pavement, two lanes, kerb and our side."""
    p.rect("far_pavement", 0, top, 1920, 26, PAVEMENT)
    p.rect("road", 0, top + 26, 1920, 800 - top - 26, ASPHALT)
    middle = (top + 26 + 800) / 2
    for i in range(4):
        p.rect(f"lane_{i}", 80 + i * 500, middle - 4, 240, 8, LANE)
    p.rect("kerb", 0, 798, 1920, 14, CURB)
    p.rect("near_ground", 0, 812, 1920, 268, near)
    p.line("near_seam_0", [(520, 814), (220, 1080)], 2, PAVEMENT_SEAM)
    p.line("near_seam_1", [(0, 920), (1920, 920)], 2, PAVEMENT_SEAM)


def _pickets(x0, x1, top, bottom, step):
    """A square wave: one stroke draws a row of pickets or railing bars."""
    points = []
    for i, x in enumerate(range(round(x0), round(x1) + 1, step)):
        points.extend([(x, top), (x, bottom)] if i % 2 == 0 else [(x, bottom), (x, top)])
    return points


def _shopfront(p, name, x, w, sign, awning, goods, door=TEAL):
    """A ground-floor shop between y=520 and the pavement at 810."""
    p.panel(name + "_sign", x, 520, w, 58, sign, 6)
    letters = [(x + 40 + i * (w - 80) / 8, 549 + (-9 if i % 2 else 9)) for i in range(9)]
    p.line(name + "_lettering", letters, 7, INK if sign in (AMBER, PAPER, SAND) else PAPER)
    p.rect(name + "_awning", x - 10, 584, w + 20, 50, awning)
    p.rect(name + "_awning_stripe", x + w / 2 - 25, 584, 50, 50, PAPER)
    window_w = w - 200
    p.rect(name + "_window", x + 20, 652, window_w, 140, GLASS)
    for i, (gw, gh, color) in enumerate(goods):
        gx = x + 40 + i * (window_w - 40) / max(len(goods), 1)
        p.oval(f"{name}_goods_{i}", gx, 792 - gh - 10, gw, gh, color)
    p.panel(name + "_door", x + w - 160, 650, 140, 160, door, 7)


def _building(p, name, x, w, top, wall, rows=2, lit=()):
    p.rect(name + "_wall", x, top, w, 810 - top, wall)
    p.rect(name + "_cornice", x, top - 12, w, 24, INK)
    cols = 3 if w >= 600 else 2
    span = (w - 80) / cols
    for row in range(rows):
        for col in range(cols):
            i = row * cols + col
            color = AMBER if i in lit else GLASS
            p.rect(f"{name}_window_{i}", x + 40 + col * span + span * 0.15, top + 50 + row * 150,
                   span * 0.7, 110, color)


def _lamp(p, name, x, ground, height, lit=False, flip=False):
    """A city street lamp: a post with an arm."""
    arm = -110 if flip else 110
    top = ground - height
    p.line(name + "_post", [(x, ground), (x, top), (x + arm, top)], 12, INK)
    p.rect(name + "_shade", x + arm - 40, top - 8, 80, 22, INK)
    if lit:
        p.oval(name + "_glow", x + arm - 170, top - 60, 340, 420, LAMP_GLOW)
    p.oval(name + "_light", x + arm - 32, top + 10, 64, 16, AMBER)


# ---------------------------------------------------------------------------
# Sets


def _keskusta():
    p = Paint("keskusta")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    p.rect("far_tower", 1480, 0, 170, 260, SKY_FAR)
    _building(p, "bakery", 0, 640, 140, SAND, lit=(4,))
    _shopfront(p, "bakery", 40, 560, AMBER, RUST, [(90, 46, WOOD)])
    _building(p, "boutique", 640, 640, 70, ROSE, lit=(1,))
    _shopfront(p, "boutique", 680, 560, TEAL, TEAL,
               [(60, 110, PAPER)], door=RUST)
    _building(p, "pharmacy", 1280, 640, 180, PLASTER)
    _shopfront(p, "pharmacy", 1320, 560, PAPER, GREEN,
               [(50, 60, GREEN)], door=GREEN)
    _near_street(p)
    p.use("katupuu", "tree", -110, 1006, 600)
    p.use("liikennevalo", "traffic_light", 1770, 1012, 660)
    p.use("paloposti", "hydrant", 1220, 1000, 110)
    return p.layers


def _keskusta_reverse():
    p = Paint("keskusta_rev")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    # The bank: a stone facade with columns under a pediment.
    p.rect("bank_wall", 0, 300, 620, 340, STONE)
    p.poly("bank_pediment", [(0, 304), (310, 190), (620, 304)], STONE)
    p.line("bank_pediment_edge", [(-10, 306), (310, 184), (630, 306)], 10, INK)
    p.rect("bank_frieze", 0, 300, 620, 18, INK)
    for i in range(4):
        p.line(f"bank_column_{i}", [(110 + i * 133, 330), (110 + i * 133, 620)], 26, (232, 228, 218, 255))
    p.rect("bank_door", 270, 500, 80, 140, INK)
    p.rect("bank_steps", 220, 626, 180, 14, PAVEMENT_SEAM)
    # The cinema: a lit marquee and two posters.
    p.rect("cinema_wall", 620, 150, 680, 490, (186, 140, 134, 255))
    p.rect("cinema_cornice", 620, 138, 680, 24, INK)
    p.panel("cinema_sign", 920, 190, 90, 220, RUST, 7)
    p.line("cinema_sign_lettering", [(950, 220), (980, 250), (950, 280), (980, 310), (950, 340), (980, 370)], 7, AMBER)
    p.panel("cinema_marquee", 680, 420, 560, 90, AMBER, 9)
    p.line("cinema_marquee_lettering", [(730, 465), (790, 450), (850, 478), (910, 450), (970, 478),
                                        (1030, 450), (1090, 478), (1170, 455)], 7, INK)
    p.panel("cinema_poster_left", 690, 530, 110, 110, TEAL, 6)
    p.panel("cinema_poster_right", 1120, 530, 110, 110, RUST, 6)
    p.rect("cinema_doors", 860, 540, 200, 100, GLASS_DARK)
    _building(p, "flats", 1300, 620, 180, SAND, rows=2, lit=(2, 3))
    _far_street(p)
    p.use("linja_auto", "bus", 1080, 736, 150, flip=True)
    p.use("taksi", "taxi", 140, 796, 140)
    _lamp(p, "lamp", 1840, 840, 520, flip=True)
    return p.layers


def _asuinalue():
    p = Paint("asuinalue")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    p.oval("cloud_left", 220, 90, 260, 70, (226, 234, 230, 255))
    p.oval("cloud_right", 1300, 60, 320, 80, (226, 234, 230, 255))
    p.rect("lawn", 0, 690, 1920, 124, GRASS)
    p.rect("driveway", 660, 700, 360, 114, PAVEMENT_SEAM)
    p.use("omakotitalo", "house_yellow", 40, 760, 470)
    p.use("omakotitalo", "house_teal", 1140, 768, 500,
          recolor={(214, 196, 160, 255): PLASTER, ROOF_TILE: (92, 104, 110, 255), TEAL: RUST})
    p.use("henkiloauto", "car", 690, 798, 140)
    p.use("katupuu", "maple", 980, 806, 560,
          recolor={LEAF: (198, 150, 84, 255), LEAF_LIGHT: (214, 176, 96, 255), LEAF_DARK: (168, 112, 70, 255)})
    for side, (x0, x1) in (("left", (0, 640)), ("right", (1040, 1920))):
        p.line(f"fence_{side}_pickets", _pickets(x0 + 10, x1, 742, 806, 36), 8, PAPER)
        p.line(f"fence_{side}_rail", [(x0, 772), (x1, 772)], 8, PAPER)
    p.line("mailbox_post", [(1080, 806), (1080, 700)], 10, INK)
    p.rect("mailbox", 1040, 660, 90, 50, TEAL)
    p.rect("mailbox_flag", 1122, 640, 10, 40, RED)
    _near_street(p)
    return p.layers


def _asuinalue_reverse():
    p = Paint("asuinalue_rev")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    p.rect("lawn", 0, 540, 1920, 110, GRASS)
    p.use("kerrostalo", "block_left", 60, 640, 600)
    p.use("kerrostalo", "block_right", 1460, 640, 540,
          recolor={STONE: ROSE, AMBER: GLASS})
    # A small playground between the blocks.
    p.line("swing_frame_left", [(760, 640), (810, 460), (860, 640)], 10, INK)
    p.line("swing_frame_right", [(1020, 640), (1070, 460), (1120, 640)], 10, INK)
    p.line("swing_bar", [(810, 460), (1070, 460)], 10, INK)
    p.line("swing_ropes", [(880, 462), (880, 590), (940, 590), (940, 462)], 4, INK)
    p.rect("swing_seat", 870, 586, 80, 12, RUST)
    p.line("slide_ladder", [(1180, 640), (1200, 520), (1220, 640)], 8, INK)
    p.line("slide", [(1210, 524), (1290, 560), (1340, 636)], 16, AMBER)
    p.use("katupuu", "tree", 1250, 650, 340)
    _far_street(p, top=650)
    p.use("pakettiauto", "van", 980, 800, 150, flip=True)
    return p.layers


def _tori():
    p = Paint("tori")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    # The town hall with its clock tower.
    p.rect("hall_tower", 860, 70, 200, 260, SAND)
    p.poly("hall_tower_roof", [(846, 80), (960, 0), (1074, 80)], (110, 128, 120, 255), 18)
    p.line("hall_tower_roof_edge", [(838, 86), (960, -6), (1082, 86)], 8, INK)
    p.disc("hall_clock", 910, 120, 100, 100, PAPER, 7)
    p.line("hall_clock_hands", [(960, 140), (960, 170), (985, 182)], 6, INK)
    p.rect("hall_body", 540, 300, 840, 400, SAND)
    p.rect("hall_cornice", 530, 288, 860, 24, INK)
    for i in range(5):
        if i != 2:
            p.rect(f"hall_window_{i}", 590 + i * 160, 380, 90, 150, GLASS)
    p.oval("hall_arch", 900, 520, 120, 90, INK)
    p.rect("hall_door", 900, 565, 120, 135, INK)
    p.rect("hall_steps", 850, 690, 220, 16, PAVEMENT_SEAM)
    for side, x in (("left", 600), ("right", 1300)):
        p.line(f"flag_{side}_pole", [(x, 300), (x, 170)], 6, INK)
        p.rect(f"flag_{side}", x + 3, 172, 70, 44, BLUE if side == "left" else PAPER)
    p.rect("square", 0, 700, 1920, 380, SQUARE)
    p.line("square_seam_0", [(700, 702), (260, 1080)], 2, PAVEMENT_SEAM)
    p.line("square_seam_1", [(1220, 702), (1660, 1080)], 2, PAVEMENT_SEAM)
    p.line("square_cross_0", [(0, 780), (1920, 780)], 2, PAVEMENT_SEAM)
    p.line("square_cross_1", [(0, 900), (1920, 900)], 2, PAVEMENT_SEAM)
    p.use("torikoju", "stall_left", 30, 840, 440)
    p.use("torikoju", "stall_right", 1450, 840, 440, recolor={RUST: TEAL})
    _lamp(p, "lamp_left", 500, 830, 420)
    _lamp(p, "lamp_right", 1420, 830, 420, flip=True)
    p.oval("pigeon_body", 620, 900, 36, 20, METAL)
    p.oval("pigeon_head", 648, 892, 14, 14, METAL)
    return p.layers


def _tori_reverse():
    p = Paint("tori_rev")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    p.rect("store", 0, 170, 1920, 540, STORE)
    p.rect("store_cornice", 0, 160, 1920, 24, INK)
    for i in range(6):
        p.rect(f"store_upper_{i}", 90 + i * 300, 220, 180, 90, GLASS)
    p.panel("store_sign", 660, 328, 600, 80, TEAL, 8)
    p.line("store_sign_lettering", [(720, 368), (790, 352), (860, 384), (930, 352), (1000, 384),
                                    (1070, 352), (1140, 384), (1200, 360)], 8, PAPER)
    p.rect("store_window_band", 40, 440, 1840, 270, (168, 160, 148, 255))
    for i, x in enumerate((60, 480, 1100, 1500)):
        p.rect(f"store_window_{i}", x, 456, 360, 254, GLASS)
    p.line("store_mannequin", [(230, 700), (230, 560)], 8, INK)
    p.poly("store_dress", [(190, 640), (230, 560), (270, 640)], RUST, 14)
    p.oval("store_mannequin_head", 216, 530, 28, 28, INK)
    p.rect("store_entrance", 860, 456, 200, 254, GLASS_DARK)
    p.line("store_revolving", [(960, 460), (960, 706)], 8, METAL)
    # The tram passing in front of the store.
    p.line("tram_wire", [(0, 300), (1920, 300)], 3, INK)
    p.line("tram_pantograph", [(560, 470), (620, 400), (560, 330), (640, 302)], 6, INK)
    p.panel("tram_body", 160, 470, 1000, 260, (214, 196, 128, 255), 9)
    p.rect("tram_window_band", 176, 506, 968, 110, INK)
    for i in range(5):
        p.rect(f"tram_window_{i}", 188 + i * 192, 516, 172, 90, GLASS)
    p.rect("tram_stripe", 170, 640, 980, 20, TEAL)
    p.rect("tram_skirt", 170, 700, 980, 26, INK)
    p.rect("rail_bed", 0, 724, 1920, 66, PAVEMENT_SEAM)
    p.line("rail_far", [(0, 742), (1920, 742)], 6, METAL)
    p.line("rail_near", [(0, 770), (1920, 770)], 6, METAL)
    p.rect("square", 0, 790, 1920, 290, SQUARE)
    p.line("square_seam", [(900, 792), (520, 1080)], 2, PAVEMENT_SEAM)
    p.line("square_cross", [(0, 900), (1920, 900)], 2, PAVEMENT_SEAM)
    p.panel("statue_plinth", 1560, 560, 160, 260, STONE, 8)
    p.oval("statue_head", 1624, 420, 34, 34, (92, 120, 112, 255))
    p.line("statue_body", [(1641, 452), (1641, 510), (1620, 562)], 12, (92, 120, 112, 255))
    p.line("statue_arm", [(1641, 470), (1690, 430), (1700, 400)], 10, (92, 120, 112, 255))
    p.line("statue_leg", [(1641, 510), (1662, 562)], 12, (92, 120, 112, 255))
    return p.layers


def _lahikauppa():
    p = Paint("lahikauppa")
    p.rect("wall", 0, 0, 1920, 1080, SHOP_WALL)
    p.rect("floor", 0, 810, 1920, 270, SHOP_FLOOR)
    p.layers.extend(_tagless_seams("lahikauppa", (188, 186, 174, 255)))
    p.rect("skirting", 0, 798, 1920, 12, TEAL)
    product_colors = [(AMBER, (184, 96, 72, 255)), (LEAF_LIGHT, PAPER), ((150, 104, 120, 255), TEAL)]
    for side, x in (("left", 70), ("right", 1150)):
        p.panel(f"shelf_{side}", x, 330, 700, 480, (176, 186, 184, 255), 8)
        for row in range(3):
            y = 380 + row * 140
            left, right = product_colors[(row + (side == "right")) % 3]
            p.rect(f"shelf_{side}_goods_{row}_a", x + 24, y, 320, 84, left)
            p.rect(f"shelf_{side}_goods_{row}_b", x + 356, y, 320, 84, right)
            p.rect(f"shelf_{side}_board_{row}", x + 8, y + 84, 684, 12, INK)
        p.rect(f"shelf_{side}_price_strip", x + 8, 330 + 8, 684, 18, PAPER)
    p.panel("fridge", 790, 300, 340, 510, METAL, 8)
    p.rect("fridge_light", 806, 314, 308, 20, PAPER)
    for i, color in enumerate((AMBER, WHITE, (184, 96, 72, 255), LEAF_LIGHT)):
        p.line(f"fridge_bottles_{i}", [(826, 400 + i * 100), (1094, 400 + i * 100)], 24, color)
    p.rect("fridge_door_left", 806, 342, 150, 452, GLASS_SEE)
    p.rect("fridge_door_right", 964, 342, 150, 452, GLASS_SEE)
    p.line("fridge_handles", [(946, 520), (946, 600)], 6, INK)
    p.line("fridge_handle_right", [(974, 520), (974, 600)], 6, INK)
    p.line("sale_cables", [(1330, 0), (1330, 130)], 3, INK)
    p.disc("sale_badge", 1260, 120, 140, 140, AMBER, 8)
    p.line("sale_lettering", [(1290, 192), (1312, 170), (1334, 196), (1356, 170), (1372, 190)], 7, INK)
    p.line("aisle_cables", [(880, 0), (880, 150), (1040, 150), (1040, 0)], 3, INK)
    p.panel("aisle_sign", 840, 150, 240, 80, TEAL, 7)
    p.line("aisle_lettering", [(880, 190), (920, 178), (960, 200), (1000, 178), (1040, 196)], 7, PAPER)
    p.rect("ceiling_light_left", 160, 46, 480, 20, PAPER)
    p.rect("ceiling_light_right", 1280, 46, 480, 20, PAPER)
    return p.layers


def _tagless_seams(prefix, tone):
    seams = _floor_seams(prefix, tone)
    for layer in seams:
        layer["plane"] = BG
    return seams


def _lahikauppa_reverse():
    p = Paint("lahikauppa_rev")
    p.rect("wall", 0, 0, 1920, 1080, SHOP_WALL)
    p.rect("floor", 0, 810, 1920, 270, SHOP_FLOOR)
    p.layers.extend(_tagless_seams("lahikauppa_rev", (188, 186, 174, 255)))
    p.rect("skirting", 0, 798, 1920, 12, TEAL)
    # The entrance: sliding doors with the street outside.
    p.rect("outside_sky", 1100, 380, 500, 200, SKY)
    p.rect("outside_house", 1100, 470, 500, 250, SAND)
    p.rect("outside_house_window", 1180, 520, 110, 90, GLASS)
    p.rect("outside_pavement", 1100, 720, 500, 90, PAVEMENT)
    p.rect("door_frame", 1084, 364, 532, 16, INK)
    p.rect("door_left", 1100, 380, 250, 430, GLASS_SEE)
    p.rect("door_right", 1350, 380, 250, 430, GLASS_SEE)
    p.line("door_frames", [(1092, 810), (1092, 372), (1608, 372), (1608, 810)], 14, INK)
    p.line("door_gap", [(1350, 380), (1350, 810)], 6, INK)
    p.panel("open_sign", 1400, 280, 160, 60, GREEN, 6)
    p.rect("doormat", 1110, 812, 480, 20, INK)
    # Checkout.
    p.panel("counter", 90, 620, 720, 190, (180, 186, 182, 255), 8)
    p.rect("belt", 104, 596, 480, 28, INK)
    p.rect("belt_goods", 180, 566, 70, 36, AMBER)
    p.rect("belt_goods_two", 290, 572, 90, 30, (184, 96, 72, 255))
    p.panel("register", 620, 500, 160, 120, METAL, 7)
    p.rect("register_screen", 640, 460, 110, 50, GLASS)
    p.rect("card_terminal", 560, 560, 40, 50, INK)
    p.panel("candy_rack", 40, 660, 40, 150, RUST, 5)
    p.panel("poster", 260, 200, 260, 180, PAPER, 7)
    p.oval("poster_sun", 300, 230, 70, 70, AMBER)
    p.line("poster_wave", [(290, 340), (350, 310), (410, 340), (480, 310)], 8, TEAL)
    p.disc("clock", 820, 180, 110, 110, PAPER, 7)
    p.line("clock_hands", [(875, 205), (875, 235), (900, 248)], 6, INK)
    for i, x in enumerate((1660, 1760)):
        p.line(f"cart_{i}_basket", [(x, 700), (x + 20, 820), (x + 120, 820), (x + 140, 700), (x, 700)], 8, METAL)
        p.line(f"cart_{i}_handle", [(x + 140, 700), (x + 170, 680)], 8, RED)
        p.line(f"cart_{i}_wheels", [(x + 20, 820), (x + 30, 870), (x + 120, 870), (x + 112, 820)], 6, INK)
    return p.layers


def _huoltoasema():
    p = Paint("huoltoasema")
    p.rect("sky", 0, 0, 1920, 1080, DUSK)
    p.oval("sun", 1500, 520, 170, 170, (238, 186, 110, 255))
    p.rect("forecourt", 0, 812, 1920, 268, CONCRETE)
    p.line("forecourt_seam", [(0, 930), (1920, 930)], 2, PAVEMENT_SEAM)
    p.line("forecourt_seam_two", [(960, 814), (960, 1080)], 2, PAVEMENT_SEAM)
    p.oval("oil_stain", 1240, 980, 240, 40, (176, 174, 168, 255))
    p.rect("shop_wall", 1120, 420, 800, 390, PAPER)
    p.rect("shop_roof", 1104, 404, 832, 24, INK)
    p.rect("shop_sign", 1180, 444, 500, 60, TEAL)
    p.line("shop_lettering", [(1220, 476), (1270, 462), (1320, 490), (1370, 462), (1420, 490),
                              (1470, 462), (1520, 488), (1620, 470)], 7, PAPER)
    p.rect("shop_window", 1160, 540, 520, 200, AMBER)
    p.line("shop_window_shelves", [(1180, 640), (1660, 640)], 6, (198, 150, 84, 255))
    p.panel("shop_door", 1720, 560, 140, 250, GLASS_DARK, 7)
    p.use("juoma_automaatti", "vending", 1060, 810, 300)
    p.line("canopy_pillar_left", [(270, 812), (270, 300)], 26, METAL)
    p.line("canopy_pillar_right", [(1000, 812), (1000, 300)], 26, METAL)
    p.panel("canopy", 200, 230, 1000, 80, PAPER, 8)
    p.rect("canopy_stripe", 208, 268, 984, 20, RED)
    for i in range(3):
        p.rect(f"canopy_light_{i}", 330 + i * 300, 310, 120, 12, AMBER)
    p.rect("island", 270, 798, 700, 16, CURB)
    p.use("polttoainemittari", "pump_left", 290, 806, 320)
    p.use("polttoainemittari", "pump_right", 760, 806, 320)
    p.use("henkiloauto", "car", 440, 812, 150)
    p.line("price_post", [(80, 812), (80, 440)], 16, INK)
    p.panel("price_board", 0, 200, 160, 250, PAPER, 7)
    p.rect("price_logo", 10, 210, 140, 60, RED)
    for i in range(3):
        p.line(f"price_digits_{i}", [(30, 310 + i * 46), (60, 300 + i * 46), (90, 318 + i * 46),
                                     (130, 302 + i * 46)], 6, INK)
    return p.layers


def _huoltoasema_reverse():
    p = Paint("huoltoasema_rev")
    p.rect("sky", 0, 0, 1920, 1080, DUSK)
    p.rect("hills", 0, 560, 1920, 100, (176, 178, 150, 255))
    p.use("katupuu", "tree_left", 860, 664, 300)
    p.use("katupuu", "tree_right", 1180, 664, 260)
    _far_street(p, top=660, near=CONCRETE)
    p.use("pakettiauto", "van", 1080, 790, 130)
    # The car wash.
    p.rect("wash_wall", 40, 360, 720, 452, (214, 214, 204, 255))
    p.rect("wash_roof", 24, 344, 752, 26, INK)
    p.rect("wash_bay", 140, 470, 420, 342, (110, 124, 128, 255))
    p.oval("wash_brush_left", 170, 500, 110, 300, CAR_BLUE)
    p.oval("wash_brush_right", 420, 500, 110, 300, CAR_BLUE)
    p.line("wash_bubbles", [(300, 600), (320, 580), (340, 610), (360, 585), (380, 612)], 10, PAPER)
    p.panel("wash_sign", 140, 390, 420, 60, TEAL, 6)
    p.line("wash_lettering", [(180, 420), (240, 406), (300, 432), (360, 406), (420, 430), (510, 414)], 6, PAPER)
    p.line("air_post", [(1580, 812), (1580, 620)], 14, INK)
    p.panel("air_unit", 1540, 560, 80, 110, RED, 6)
    p.line("air_hose", [(1600, 640), (1650, 700), (1630, 780)], 6, INK)
    p.use("paloposti", "hydrant", 1720, 812, 120)
    return p.layers


def _puisto():
    p = Paint("puisto")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    for i, (x, w, top) in enumerate([(140, 160, 300), (330, 120, 220), (1380, 150, 260), (1560, 190, 340)]):
        p.rect(f"skyline_{i}", x, top, w, 520 - top, SKY_FAR)
    p.use("pilvenpiirtaja", "tower", 1180, 560, 520, recolor={(128, 156, 166, 255): SKY_FAR,
                                                               GLASS: (186, 210, 206, 255)})
    for i, (x, w) in enumerate([(-60, 520), (380, 600), (900, 560), (1380, 640)]):
        p.oval(f"treeline_{i}", x, 470, w, 260, LEAF if i % 2 else LEAF_DARK)
    p.rect("grass", 0, 700, 1920, 380, GRASS)
    p.poly("path", [(820, 700), (1100, 700), (1560, 1080), (360, 1080)], (220, 206, 176, 255), 12)
    p.oval("pond_edge", 40, 850, 560, 170, (118, 150, 120, 255))
    p.oval("pond", 60, 862, 520, 146, WATER)
    for i, x in enumerate((200, 380)):
        p.oval(f"duck_{i}_body", x, 912, 60, 30, PAPER)
        p.oval(f"duck_{i}_head", x + 42, 896, 24, 24, LEAF_DARK if i else PAPER)
    p.use("katupuu", "oak_left", 520, 760, 520)
    p.use("katupuu", "oak_right", 1500, 790, 580)
    p.use("puistonpenkki", "bench", 1180, 780, 130)
    _lamp(p, "lamp", 780, 760, 380)
    for i, (x, color) in enumerate([(1660, (214, 120, 96, 255)), (1740, AMBER), (1820, PAPER)]):
        p.oval(f"flowers_{i}", x, 900, 90, 50, color)
    return p.layers


def _puisto_reverse():
    p = Paint("puisto_rev")
    p.rect("sky", 0, 0, 1920, 1080, SKY)
    p.oval("hill", -200, 560, 2400, 360, (170, 192, 150, 255))
    for i, (x, w) in enumerate([(-80, 540), (1400, 600)]):
        p.oval(f"treeline_{i}", x, 440, w, 300, LEAF_DARK)
    p.rect("grass", 0, 700, 1920, 380, GRASS)
    p.poly("path", [(860, 700), (1060, 700), (1300, 1080), (580, 1080)], (220, 206, 176, 255), 12)
    # Playground.
    p.line("swing_frame_left", [(140, 780), (220, 470), (300, 780)], 12, INK)
    p.line("swing_frame_right", [(560, 780), (640, 470), (720, 780)], 12, INK)
    p.line("swing_bar", [(220, 470), (640, 470)], 12, INK)
    p.line("swing_ropes", [(330, 474), (330, 660), (420, 660), (420, 474)], 5, INK)
    p.rect("swing_seat", 316, 654, 118, 16, RUST)
    p.line("swing_ropes_two", [(480, 474), (500, 650), (580, 640), (570, 474)], 5, INK)
    p.rect("sandbox", 160, 790, 300, 50, WOOD)
    p.rect("sand", 176, 790, 268, 20, (232, 214, 170, 255))
    p.line("slide_ladder", [(1400, 790), (1430, 560), (1460, 790)], 10, INK)
    p.rect("slide_platform", 1410, 552, 120, 18, INK)
    p.line("slide", [(1520, 560), (1640, 640), (1720, 780), (1780, 790)], 22, AMBER)
    p.use("torikoju", "ice_cream", 1060, 790, 330,
          recolor={RUST: (204, 140, 150, 255), (184, 96, 72, 255): PAPER})
    p.use("katupuu", "tree", 1720, 760, 480)
    return p.layers


def _katto():
    p = Paint("katto")
    p.rect("sky", 0, 0, 1920, 1080, NIGHT_CITY)
    p.oval("moon_halo", 1440, 40, 260, 260, (215, 224, 188, 20))
    p.oval("moon", 1490, 90, 160, 160, (218, 224, 197, 255))
    for i, (x, y) in enumerate([(300, 120), (720, 60), (1120, 160), (1820, 260)]):
        p.oval(f"star_{i}", x, y, 8, 8, PAPER)
    towers = [(0, 220, 420), (240, 200, 300), (460, 260, 470), (760, 180, 380), (980, 280, 260),
              (1300, 240, 430), (1560, 200, 360), (1780, 160, 450)]
    for i, (x, w, top) in enumerate(towers):
        p.rect(f"tower_{i}", x, top, w, 780 - top, SKYLINE if i % 2 else SKYLINE_TWO)
    for i, (x, top, bottom) in enumerate([(60, 470, 560), (120, 620, 700), (520, 340, 470), (600, 540, 600),
                                          (820, 450, 640), (1040, 300, 380), (1360, 500, 600),
                                          (1440, 640, 700), (1620, 420, 520), (1840, 520, 640)]):
        p.line(f"lit_{i}", [(x, top), (x, bottom)], 12, AMBER)
    p.use("pilvenpiirtaja", "spire", 1060, 780, 560,
          recolor={(128, 156, 166, 255): SKYLINE_TWO, GLASS: (110, 128, 144, 255)}, lean=False)
    p.rect("parapet", 0, 760, 1920, 56, (178, 176, 168, 255))
    p.line("parapet_coping", [(0, 760), (1920, 760)], 10, INK)
    p.rect("roof", 0, 816, 1920, 264, ROOF)
    p.line("roof_seam", [(0, 930), (1920, 930)], 2, (140, 142, 138, 255))
    p.line("roof_seam_two", [(700, 818), (420, 1080)], 2, (140, 142, 138, 255))
    p.oval("puddle", 1080, 1000, 320, 34, (172, 186, 196, 255))
    # Water tank on legs.
    p.line("tank_legs", [(80, 880), (100, 640), (280, 640), (300, 880)], 14, INK)
    p.line("tank_brace", [(100, 760), (280, 700)], 8, INK)
    p.rect("tank", 70, 420, 240, 230, WOOD)
    p.line("tank_band_top", [(70, 480), (310, 480)], 6, INK)
    p.line("tank_band_bottom", [(70, 590), (310, 590)], 6, INK)
    p.poly("tank_roof", [(60, 426), (190, 350), (320, 426)], (110, 92, 72, 255), 16)
    p.line("antenna", [(1780, 880), (1780, 300)], 10, METAL)
    p.line("antenna_bars", [(1740, 380), (1820, 380), (1780, 380), (1780, 440), (1730, 440), (1830, 440)], 6, METAL)
    p.oval("antenna_light", 1768, 286, 24, 24, RED)
    p.panel("vent", 1480, 760, 220, 120, METAL, 7)
    p.disc("vent_fan", 1540, 780, 80, 80, (110, 124, 128, 255), 6)
    return p.layers


def _katto_reverse():
    p = Paint("katto_rev")
    p.rect("sky", 0, 0, 1920, 1080, NIGHT_CITY)
    for i, (x, y) in enumerate([(200, 80), (640, 150), (1000, 60), (1500, 120), (1760, 200)]):
        p.oval(f"star_{i}", x, y, 8, 8, PAPER)
    p.rect("river", 0, 610, 1920, 150, (112, 136, 156, 255))
    p.line("bridge_arch", [(620, 640), (820, 470), (1040, 420), (1260, 470), (1460, 640)], 12, INK)
    p.line("bridge_deck", [(400, 600), (1700, 600)], 14, INK)
    p.line("bridge_lights", [(420, 588), (1680, 588)], 4, AMBER)
    p.line("bridge_hangers", [(760, 512), (760, 600), (900, 600), (900, 444), (1040, 422), (1040, 600),
                              (1180, 600), (1180, 444), (1320, 512), (1320, 600)], 4, INK)
    towers = [(0, 300, 380), (300, 200, 470), (1600, 320, 340)]
    for i, (x, w, top) in enumerate(towers):
        p.rect(f"tower_{i}", x, top, w, 780 - top, SKYLINE if i % 2 else SKYLINE_TWO)
    for i, (x, top, bottom) in enumerate([(80, 440, 560), (220, 600, 700), (380, 520, 640),
                                          (1680, 400, 520), (1820, 560, 680)]):
        p.line(f"lit_{i}", [(x, top), (x, bottom)], 12, AMBER)
    p.rect("parapet", 0, 760, 1920, 56, (178, 176, 168, 255))
    p.line("parapet_coping", [(0, 760), (1920, 760)], 10, INK)
    p.rect("roof", 0, 816, 1920, 264, ROOF)
    p.line("roof_seam", [(0, 940), (1920, 940)], 2, (140, 142, 138, 255))
    # The stairwell hut with its door lamp.
    p.rect("hut", 80, 400, 460, 480, (184, 180, 170, 255))
    p.rect("hut_roof", 60, 384, 500, 30, INK)
    p.panel("hut_door", 240, 600, 150, 280, TEAL, 8)
    p.oval("hut_knob", 362, 740, 14, 14, AMBER)
    p.oval("hut_glow", 170, 440, 290, 300, LAMP_GLOW)
    p.rect("hut_lamp", 286, 540, 58, 28, AMBER)
    # A billboard on the next roof.
    p.line("billboard_legs", [(1200, 760), (1240, 560), (1280, 760), (1320, 560), (1360, 760),
                              (1400, 560), (1440, 760), (1480, 560), (1520, 760)], 8, INK)
    p.panel("billboard", 1140, 300, 440, 270, TEAL, 9)
    p.oval("billboard_sun", 1200, 340, 110, 110, AMBER)
    p.line("billboard_wave", [(1180, 500), (1260, 460), (1340, 500), (1420, 460), (1540, 500)], 12, PAPER)
    p.rect("chimney", 1700, 560, 90, 260, BRICK_LIGHT)
    p.rect("chimney_cap", 1688, 548, 114, 20, INK)
    return p.layers


def _kuja():
    p = Paint("kuja")
    p.rect("wall", 0, 0, 1920, 1080, ALLEY_BRICK)
    for i, y in enumerate((140, 330, 560)):
        p.line(f"brick_course_{i}", [(0, y), (1920, y)], 3, (160, 134, 122, 255))
    for i, (x, y) in enumerate([(120, 200), (760, 420), (1640, 260)]):
        p.rect(f"brick_patch_{i}", x, y, 120, 44, (164, 136, 124, 255))
    p.rect("ground", 0, 810, 1920, 270, ALLEY)
    p.line("ground_drain", [(960, 812), (900, 1080)], 4, (128, 132, 130, 255))
    p.panel("window", 640, 110, 210, 170, AMBER, 9)
    p.line("window_mullion", [(745, 118), (745, 272)], 7, INK)
    p.line("pipe", [(920, 0), (920, 700), (980, 790)], 14, METAL)
    p.oval("door_glow", 300, 360, 340, 380, LAMP_GLOW)
    p.rect("door_lamp", 430, 450, 70, 26, AMBER)
    p.panel("door", 360, 500, 220, 310, (88, 118, 116, 255), 9)
    p.rect("door_step", 340, 800, 260, 16, INK)
    # Fire escape.
    p.rect("escape_platform_top", 1180, 230, 560, 16, INK)
    p.line("escape_rail_top", _pickets(1180, 1740, 160, 230, 40), 5, INK)
    p.rect("escape_platform_low", 1180, 500, 560, 16, INK)
    p.line("escape_rail_low", _pickets(1180, 1740, 430, 500, 40), 5, INK)
    p.line("escape_stairs", [(1260, 500), (1640, 246)], 14, INK)
    p.line("escape_ladder", [(1660, 516), (1660, 760)], 8, INK)
    p.line("escape_ladder_right", [(1700, 516), (1700, 760)], 8, INK)
    # Dumpster, bags, crates and the alley cat.
    p.panel("dumpster", 40, 690, 320, 200, TEAL, 9)
    p.rect("dumpster_lid", 30, 674, 340, 24, INK)
    p.oval("dumpster_wheel_left", 70, 880, 34, 34, INK)
    p.oval("dumpster_wheel_right", 296, 880, 34, 34, INK)
    p.oval("bag", 340, 820, 130, 100, (78, 86, 90, 255))
    p.oval("bag_two", 420, 850, 110, 80, (98, 104, 104, 255))
    p.panel("crate", 1460, 780, 180, 140, WOOD, 8)
    p.line("crate_slat", [(1468, 850), (1632, 850)], 6, INK)
    p.panel("crate_top", 1490, 660, 140, 120, WOOD, 8)
    p.oval("cat_body", 1520, 610, 80, 56, INK)
    p.oval("cat_head", 1580, 586, 44, 40, INK)
    p.line("cat_ears", [(1586, 594), (1590, 576), (1600, 590), (1612, 576), (1618, 594)], 5, INK)
    p.line("cat_tail", [(1524, 646), (1490, 640), (1478, 600)], 7, INK)
    p.oval("puddle", 900, 980, 340, 40, (176, 186, 192, 255))
    return p.layers


def _kuja_reverse():
    p = Paint("kuja_rev")
    p.rect("street_sky", 0, 0, 1920, 1080, NIGHT_CITY)
    p.rect("street_house", 640, 180, 640, 460, (160, 150, 150, 255))
    p.rect("street_shop", 700, 420, 240, 160, AMBER)
    p.rect("street_shop_two", 1000, 420, 220, 160, (214, 160, 140, 255))
    p.rect("street_windows", 720, 240, 500, 80, (110, 124, 128, 255))
    p.rect("street_road", 640, 640, 640, 170, ASPHALT)
    p.rect("street_kerb", 640, 630, 640, 12, CURB)
    p.use("taksi", "taxi", 760, 770, 120, flip=True)
    _lamp(p, "street_lamp", 1240, 640, 400, lit=True, flip=True)
    p.rect("wall_left", 0, 0, 660, 820, ALLEY_BRICK)
    p.rect("wall_right", 1260, 0, 660, 820, ALLEY_BRICK)
    p.line("wall_left_edge", [(660, 0), (660, 820)], 12, (150, 126, 116, 255))
    p.line("wall_right_edge", [(1260, 0), (1260, 820)], 12, (150, 126, 116, 255))
    for i, y in enumerate((200, 480)):
        p.line(f"brick_course_left_{i}", [(0, y), (650, y)], 3, (160, 134, 122, 255))
        p.line(f"brick_course_right_{i}", [(1270, y), (1920, y)], 3, (160, 134, 122, 255))
    p.panel("poster", 220, 320, 240, 300, PAPER, 8)
    p.oval("poster_mask", 290, 370, 100, 70, RUST)
    p.line("poster_lines", [(260, 500), (420, 500), (420, 540), (260, 540)], 8, TEAL)
    p.line("drainpipe", [(1500, 0), (1500, 790)], 14, METAL)
    p.rect("ground", 0, 810, 1920, 270, ALLEY)
    p.line("ground_drain", [(960, 812), (1010, 1080)], 4, (128, 132, 130, 255))
    p.oval("reflection", 760, 900, 400, 46, (190, 186, 160, 255))
    p.panel("bin", 1640, 660, 150, 230, METAL, 8)
    p.rect("bin_lid", 1626, 646, 178, 22, INK)
    return p.layers


def _metroasema():
    p = Paint("metroasema")
    p.rect("wall", 0, 0, 1920, 1080, TILE)
    for i, y in enumerate((150, 260)):
        p.line(f"tile_row_{i}", [(0, y), (1920, y)], 2, (190, 200, 192, 255))
    p.rect("light_strip", 0, 30, 1920, 16, PAPER)
    p.panel("station_sign", 700, 90, 520, 100, TEAL, 8)
    p.line("station_lettering", [(760, 140), (830, 122), (900, 156), (970, 122), (1040, 156), (1160, 132)], 8, PAPER)
    p.line("clock_cable", [(330, 0), (330, 70)], 4, INK)
    p.disc("clock", 270, 70, 120, 120, PAPER, 8)
    p.line("clock_hands", [(330, 98), (330, 130), (356, 142)], 6, INK)
    # The train waiting at the platform.
    p.rect("train", -20, 360, 1960, 440, (214, 212, 202, 255))
    p.rect("train_roof", -20, 344, 1960, 24, INK)
    p.rect("train_stripe", -20, 660, 1960, 34, AMBER)
    for i, x in enumerate((60, 520, 1300, 1700)):
        p.rect(f"train_window_{i}", x, 430, 200 if i in (0, 3) else 300, 150, GLASS_DARK)
    for i, x in enumerate((870, 1060)):
        p.panel(f"train_door_{i}", x, 410, 180, 390, METAL, 7)
        p.rect(f"train_door_{i}_glass", x + 34, 440, 112, 150, GLASS_DARK)
    p.rect("train_destination", 860, 372, 380, 30, AMBER)
    p.rect("train_skirt", -20, 760, 1960, 40, (110, 120, 120, 255))
    p.rect("platform_edge", 0, 800, 1920, 22, AMBER)
    p.rect("platform", 0, 822, 1920, 258, PAVEMENT)
    p.line("platform_seam", [(0, 920), (1920, 920)], 2, PAVEMENT_SEAM)
    p.line("platform_seam_two", [(560, 824), (300, 1080)], 2, PAVEMENT_SEAM)
    p.line("platform_seam_three", [(1360, 824), (1620, 1080)], 2, PAVEMENT_SEAM)
    return p.layers


def _metroasema_reverse():
    p = Paint("metroasema_rev")
    p.rect("wall", 0, 0, 1920, 1080, TILE)
    p.line("tile_row", [(0, 150), (1920, 150)], 2, (190, 200, 192, 255))
    p.rect("light_strip", 0, 30, 1920, 16, PAPER)
    p.panel("map", 160, 180, 620, 340, PAPER, 8)
    p.line("map_line_red", [(200, 440), (360, 300), (560, 300), (740, 220)], 10, RED)
    p.line("map_line_blue", [(200, 260), (400, 380), (740, 400)], 10, BLUE)
    p.line("map_line_green", [(460, 200), (460, 500)], 10, GREEN)
    for i, (x, y) in enumerate([(348, 288), (448, 288), (448, 368), (588, 286)]):
        p.oval(f"map_stop_{i}", x, y, 24, 24, INK)
    for i, x in enumerate((880, 1020)):
        p.panel(f"ticket_machine_{i}", x, 520, 120, 290, TEAL, 7)
        p.rect(f"ticket_machine_{i}_screen", x + 20, 560, 80, 60, GLASS)
    # Escalator up to the street.
    p.rect("escalator_opening", 1560, 80, 360, 220, (118, 136, 140, 255))
    p.rect("escalator_daylight", 1600, 100, 320, 120, SKY)
    steps = []
    for i in range(16):
        x, y = 1200 + i * 40, 790 - i * 34
        steps.extend([(x, y), (x + 40, y), (x + 40, y - 34)])
    p.line("escalator_side", [(1180, 810), (1840, 250)], 24, (140, 150, 150, 255))
    p.line("escalator_steps", steps, 8, METAL)
    p.line("escalator_rail", [(1170, 690), (1830, 130)], 10, INK)
    p.line("escalator_posts", [(1190, 800), (1180, 684), (1830, 130), (1840, 250)], 6, INK)
    p.panel("exit_sign", 1240, 110, 260, 80, GREEN, 7)
    p.line("exit_arrow", [(1300, 150), (1440, 150), (1410, 126), (1440, 150), (1410, 174)], 8, PAPER)
    p.rect("floor", 0, 810, 1920, 270, PAVEMENT)
    p.line("floor_seam", [(0, 920), (1920, 920)], 2, PAVEMENT_SEAM)
    p.line("floor_seam_two", [(700, 812), (460, 1080)], 2, PAVEMENT_SEAM)
    p.use("puistonpenkki", "bench", 220, 860, 140, recolor={WOOD: METAL})
    return p.layers


def _sets():
    sets = {
        "keskusta": dict(name="Kauppakatu", color="#bed6d4", layers=_keskusta(),
                         reverse=_keskusta_reverse(), ceiling=None),
        "asuinalue": dict(name="Vaahterakuja", color="#bed6d4", layers=_asuinalue(),
                          reverse=_asuinalue_reverse(), ceiling=None),
        "tori": dict(name="Kauppatori", color="#bed6d4", layers=_tori(),
                     reverse=_tori_reverse(), ceiling=None),
        "lahikauppa": dict(name="Kulmakaupan käytävä", color="#eae6d8", layers=_lahikauppa(),
                           reverse=_lahikauppa_reverse(), ceiling=(214, 214, 202, 255)),
        "huoltoasema": dict(name="Iltahuoltoasema", color="#e2c6ac", layers=_huoltoasema(),
                            reverse=_huoltoasema_reverse(), ceiling=None),
        "puisto": dict(name="Kaupunginpuisto", color="#bed6d4", layers=_puisto(),
                       reverse=_puisto_reverse(), ceiling=None),
        "katto": dict(name="Kattojen yllä", color="#8096ac", layers=_katto(),
                      reverse=_katto_reverse(), ceiling=None),
        "kuja": dict(name="Takakuja", color="#b2988c", layers=_kuja(),
                     reverse=_kuja_reverse(), ceiling=None),
        "metroasema": dict(name="Metroasema", color="#d6ded6", layers=_metroasema(),
                           reverse=_metroasema_reverse(), ceiling=(196, 204, 198, 255)),
    }
    for spec in sets.values():
        for key in ("layers", "reverse"):
            for layer in spec[key]:
                layer["plane"] = BG
            ids = [layer["id"] for layer in spec[key]]
            duplicates = {i for i in ids if ids.count(i) > 1}
            if duplicates:
                raise ValueError(f"duplicate layer ids: {sorted(duplicates)}")
    return {name: _with_angles(name, spec) for name, spec in sets.items()}


CITY_SETS = _sets()


if __name__ == "__main__":
    from collections import Counter
    for key, value in CITY_SETS.items():
        counts = Counter(layer["angle"] for layer in value["layers"])
        print(f"{key}: {value['name']} {dict(counts)}")
    for key, value in CITY_PROPS.items():
        print(f"{key}: {value['name']} ({len(value['layers'])} layers)")
