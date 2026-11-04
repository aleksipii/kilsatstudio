"""More original art props for the asset library (registered by generate_library_props.py).

Categories: food and tableware, music, sport, health care, bathroom, bedroom and
household, garden and farm, toys, workshop tools, camping and nature, containers,
lighting, animals, vehicles and small buildings. Same conventions as the props in
`generate_library_props.py`: 1920x1080 canvas pixels at actor scale 1 (about 170
px per metre), bounds at the origin, floor contact at the bottom edge with a
`ground` anchor, hand props with a `grip` anchor, strokes at most 8 px, and every
layer painted from a palette slot.
"""
from __future__ import annotations

import math

NEW_COLORS = {
    "fruit_red": (190, 92, 80),
    "fruit_yellow": (226, 196, 108),
    "fruit_green": (150, 176, 98),
    "bread": (204, 160, 108),
    "crust": (160, 112, 70),
    "cream": (240, 228, 206),
    "berry": (176, 86, 110),
    "cheese": (230, 200, 120),
    "tomato": (196, 98, 76),
    "porcelain": (238, 238, 232),
    "water": (150, 192, 210),
    "drum_red": (168, 76, 70),
    "brass": (204, 170, 92),
    "ivory": (240, 236, 222),
    "key_black": (44, 46, 50),
    "grass": (124, 156, 100),
    "net": (226, 226, 220),
    "court": (196, 120, 84),
    "medical": (208, 222, 222),
    "cross_red": (190, 80, 74),
    "tile": (214, 224, 226),
    "towel": (150, 182, 196),
    "hay": (214, 184, 112),
    "soil": (120, 92, 70),
    "petal_pink": (220, 150, 170),
    "petal_white": (238, 236, 226),
    "toy_red": (204, 96, 80),
    "toy_blue": (92, 140, 188),
    "toy_yellow": (230, 196, 90),
    "toy_green": (120, 170, 110),
    "fur": (176, 136, 98),
    "fur_dark": (120, 92, 70),
    "fur_grey": (150, 150, 146),
    "feather": (110, 150, 180),
    "tent": (196, 128, 84),
    "tent_dark": (150, 96, 66),
    "flame": (236, 168, 84),
    "flame_core": (248, 220, 140),
    "rock": (146, 144, 138),
    "rock_dark": (110, 108, 104),
    "cardboard": (196, 160, 114),
    "barn_red": (160, 78, 66),
    "roof": (90, 92, 98),
    "tractor": (110, 150, 100),
    "boat": (176, 128, 90),
    "sail": (236, 232, 218),
    "lighthouse": (232, 228, 218),
    "wax": (240, 230, 200),
    "mushroom": (190, 84, 72),
}


def register(g):
    """Register the props on the generate_library_props module `g`."""
    g.COLORS.update(NEW_COLORS)
    legs = g.legs

    def prop(pid, name, w, h, tags, **kw):
        """`g.prop`, then fit a grounded prop's height to its drawn bottom edge so
        the floor contact is exactly the bottom of the bounds."""
        def wrap(fn):
            def draw(b):
                fn(b)
                ground = [a for a in b.anchors if a["type"] == "ground"]
                if ground:
                    bottom = g.num(g.ceil3(max(g.content_box(layer["content"])[3] for layer in b.layers)))
                    spec["h"] = bottom
                    for a in ground:
                        a["at"] = g.pt(0, bottom)
            g.prop(pid, name, w, h, tags, **kw)(draw)
            spec = g.ART[-1]
            return fn
        return wrap

    # ------------------------------------------------------------ food and tableware
    @prop("prop.food.apple_v1", "Omena", 36, 40, ["omena", "apple", "hedelmä", "fruit", "ruoka", "food", "käsiesine", "hand_prop"])
    def _(b):
        b.oval("apple", 18, 23, 15, 14, "fruit_red", 2)
        b.line("stem", [(18, 9), (20, 3)], 3, "bark")
        b.oval("leaf", 25, 6, 5, 2.5, "leaf", 0)
        b.grip(18, 23)

    @prop("prop.food.banana_v1", "Banaani", 60, 30, ["banaani", "banana", "hedelmä", "fruit", "ruoka", "food", "käsiesine", "hand_prop"])
    def _(b):
        b.line("banana", [(6, 8), (14, 18), (28, 23), (44, 21), (54, 12)], 8, "fruit_yellow")
        b.line("tip", [(4, 6), (8, 10)], 4, "crust")
        b.grip(30, 22)

    @prop("prop.food.bread_loaf_v1", "Leipä", 110, 50, ["leipä", "bread", "ruisleipä", "leipomo", "bakery", "ruoka", "food"])
    def _(b):
        b.oval("loaf", 55, 26, 51, 21.5, "bread")
        for i in range(4):
            b.line(f"cut_{i}", [(28 + i * 18, 14), (36 + i * 18, 26)], 3, "crust")
        b.ground()

    @prop("prop.food.cake_v1", "Kakku", 120, 100, ["kakku", "cake", "juhla", "party", "synttärit", "birthday", "ruoka", "food"])
    def _(b):
        b.box("plate", 4, 90, 112, b.F - 90, "porcelain", 2)
        b.box("cake", 14, 44, 92, 46, "cream")
        b.box("filling", 14, 64, 92, 6, "berry", 0)
        for i in range(5):
            b.oval(f"berry_{i}", 24 + i * 18, 42, 5, 5, "berry", 2)
        b.line("candle", [(60, 14), (60, 36)], 6, "wax")
        b.oval("flame", 60, 8, 4, 6, "flame", 0)
        b.stand(60, 44, "surface")
        b.ground()

    @prop("prop.food.pizza_v1", "Pizza", 120, 24, ["pizza", "ruoka", "food", "ravintola", "restaurant"])
    def _(b):
        b.oval("crust", 60, 12, 56, 10.5, "crust", 2)
        b.oval("cheese", 60, 11, 48, 7, "cheese", 0)
        for i, x in enumerate((36, 58, 80)):
            b.oval(f"tomato_{i}", x, 11, 6, 3, "tomato", 0)
        b.ground()

    @prop("prop.food.sandwich_v1", "Voileipä", 70, 34, ["voileipä", "sandwich", "eväs", "lunch", "ruoka", "food", "käsiesine", "hand_prop"])
    def _(b):
        b.box("bottom", 4, 22, 62, 9, "bread", 2)
        b.box("lettuce", 2, 18, 66, 5, "leaf", 0)
        b.box("cheese", 8, 14, 54, 5, "cheese", 0)
        b.box("top", 4, 4, 62, 10, "bread", 2)
        b.grip(35, 18)

    @prop("prop.food.ice_cream_v1", "Jäätelötötterö", 30, 70, ["jäätelö", "ice_cream", "kesä", "summer", "herkku", "treat", "käsiesine", "hand_prop"])
    def _(b):
        b.line("cone", [(15, 66), (15, 34)], 8, "bread")
        b.line("cone_left", [(7, 32), (14, 62)], 3, "crust")
        b.line("cone_right", [(23, 32), (16, 62)], 3, "crust")
        b.oval("scoop", 15, 22, 12, 12, "petal_pink", 2)
        b.oval("scoop_top", 15, 10, 8, 7, "cream", 2)
        b.grip(15, 52)

    @prop("prop.food.fruit_bowl_v1", "Hedelmäkulho", 120, 64, ["hedelmä", "fruit", "kulho", "bowl", "keittiö", "kitchen", "ruoka", "food"])
    def _(b):
        b.oval("apple_a", 40, 26, 14, 13, "fruit_red", 2)
        b.oval("orange", 64, 22, 15, 14, "accent", 2)
        b.oval("apple_b", 86, 28, 13, 12, "fruit_green", 2)
        b.line("banana", [(26, 30), (50, 40), (80, 38), (98, 30)], 8, "fruit_yellow")
        b.oval("bowl", 60, 46, 56, 16.5, "porcelain")
        b.ground()

    @prop("prop.tableware.plate_v1", "Lautanen", 90, 14, ["lautanen", "plate", "astia", "dish", "keittiö", "kitchen"])
    def _(b):
        b.oval("plate", 45, 7, 42, 5.5, "porcelain", 2)
        b.oval("well", 45, 6, 26, 2.5, "white", 0)
        b.ground()

    @prop("prop.tableware.bowl_v1", "Kulho", 70, 34, ["kulho", "bowl", "astia", "dish", "keitto", "soup", "keittiö", "kitchen"])
    def _(b):
        b.oval("soup", 35, 10, 30, 5, "accent", 0)
        b.box("bowl_body", 8, 10, 54, 16, "blue", 2)
        b.oval("bowl_base", 35, 26, 27, 6.5, "blue")
        b.ground()

    @prop("prop.tableware.water_glass_v1", "Vesilasi", 30, 50, ["lasi", "glass", "vesi", "water", "juoma", "drink", "käsiesine", "hand_prop"])
    def _(b):
        b.box("water", 6, 18, 18, b.F - 18, "water", 0)
        b.box("glass", 4, 4, 22, b.F - 4, "glass_clear", 2)
        b.grip(15, 26)
        b.ground()

    @prop("prop.drink.water_bottle_v1", "Vesipullo", 30, 80, ["pullo", "bottle", "vesi", "water", "juoma", "drink", "käsiesine", "hand_prop"])
    def _(b):
        b.box("cap", 10, 3, 10, 9, "blue", 2)
        b.box("bottle", 4, 12, 22, b.F - 12, "glass_clear", 2)
        b.box("label", 4, 38, 22, 16, "paper", 0)
        b.grip(15, 50)
        b.ground()

    @prop("prop.drink.milk_carton_v1", "Maitotölkki", 44, 90, ["maito", "milk", "tölkki", "carton", "aamiainen", "breakfast", "keittiö", "kitchen"])
    def _(b):
        b.box("carton", 4, 20, 36, b.F - 20, "white")
        b.line("roof_left", [(4, 21), (22, 6)], 3)
        b.line("roof_right", [(40, 21), (22, 6)], 3)
        b.box("band", 4, 46, 36, 16, "blue", 0)
        b.grip(22, 56)
        b.ground()

    # ------------------------------------------------------------ music
    @prop("prop.music.acoustic_guitar_v1", "Akustinen kitara", 90, 240, ["kitara", "guitar", "soitin", "instrument", "musiikki", "music"])
    def _(b):
        b.box("neck", 39, 22, 12, 110, "wood_dark", 2)
        b.box("head", 35, 4, 20, 22, "wood_dark", 2)
        b.oval("body_upper", 45, 140, 30, 28, "wood")
        b.oval("body_lower", 45, 194, 41, 40.5, "wood")
        b.oval("sound_hole", 45, 158, 11, 11, "screen", 2)
        b.box("bridge", 33, 196, 24, 6, "wood_dark", 0)
        for i, x in enumerate((41, 45, 49)):
            b.line(f"string_{i}", [(x, 20), (x, 198)], 1, "metal")
        b.grip(45, 70)
        b.ground()

    @prop("prop.music.upright_piano_v1", "Piano", 300, 260, ["piano", "soitin", "instrument", "musiikki", "music", "olohuone", "living_room"])
    def _(b):
        b.box("case", 8, 6, 284, 150, "wood_dark")
        b.box("lid", 4, 4, 292, 12, "wood")
        b.box("music_stand", 90, 40, 120, 40, "wood", 2)
        b.box("keybed", 4, 150, 292, 30, "wood_dark")
        b.box("keys", 14, 150, 272, 18, "ivory", 2)
        for i in range(17):
            if i % 7 not in (2, 6):
                b.box(f"black_{i}", 26 + i * 15.5, 150, 8, 10, "key_black", 0)
        b.box("lower", 16, 180, 268, b.F - 180, "wood_dark")
        b.box("pedal_board", 120, 238, 60, 8, "brass", 2)
        b.stand(150, 180, "keys")
        b.ground()

    @prop("prop.music.drum_kit_v1", "Rumpusetti", 280, 230, ["rummut", "drums", "rumpusetti", "drum_kit", "soitin", "instrument", "bändi", "band", "musiikki", "music"])
    def _(b):
        b.line("cymbal_stand", [(36, 40), (36, 226)], 4, "metal_dark")
        b.oval("cymbal", 36, 40, 32, 6, "brass", 2)
        b.line("hihat_stand", [(250, 76), (250, 226)], 4, "metal_dark")
        b.oval("hihat", 250, 76, 26, 5, "brass", 2)
        b.oval("tom_left", 108, 96, 26, 18, "drum_red")
        b.oval("tom_right", 172, 96, 26, 18, "drum_red")
        b.oval("bass_drum", 140, 166, 62, 61.5, "drum_red")
        b.oval("bass_head", 140, 166, 46, 46, "ivory", 2)
        b.box("snare", 200, 130, 60, 26, "metal", 2)
        b.ground()

    @prop("prop.music.microphone_stand_v1", "Mikrofoniteline", 90, 330, ["mikrofoni", "microphone", "teline", "stand", "laulu", "singing", "lava", "stage", "musiikki", "music"])
    def _(b):
        b.line("pole", [(45, 60), (45, 320)], 5, "metal_dark")
        b.line("boom", [(45, 64), (70, 30)], 4, "metal_dark")
        b.oval("mic", 74, 22, 8, 10, "metal", 2)
        b.line("leg_left", [(45, 314), (8, 326)], 5, "metal_dark")
        b.line("leg_right", [(45, 314), (82, 326)], 5, "metal_dark")
        b.ground()

    @prop("prop.music.speaker_v1", "Kaiutin", 110, 170, ["kaiutin", "speaker", "vahvistin", "amplifier", "ääni", "sound", "musiikki", "music"])
    def _(b):
        b.box("cabinet", 6, 6, 98, b.F - 6, "screen")
        b.oval("woofer", 55, 110, 34, 34, "metal_dark", 2)
        b.oval("woofer_cap", 55, 110, 12, 12, "screen", 2)
        b.oval("tweeter", 55, 40, 14, 14, "metal_dark", 2)
        b.ground()

    @prop("prop.music.trumpet_v1", "Trumpetti", 100, 40, ["trumpetti", "trumpet", "soitin", "instrument", "torvi", "brass", "musiikki", "music", "käsiesine", "hand_prop"])
    def _(b):
        b.line("tube", [(6, 20), (70, 20)], 6, "brass")
        b.line("loop", [(30, 20), (30, 32), (60, 32), (60, 20)], 4, "brass")
        for i, x in enumerate((38, 46, 54)):
            b.line(f"valve_{i}", [(x, 8), (x, 20)], 4, "brass")
        b.oval("bell", 82, 20, 14, 16, "brass", 2)
        b.grip(46, 26)

    # ------------------------------------------------------------ sport
    @prop("prop.sport.football_v1", "Jalkapallo", 44, 44, ["jalkapallo", "football", "pallo", "ball", "urheilu", "sport"])
    def _(b):
        b.oval("ball", 22, 22, 19.5, 19.5, "white")
        b.oval("patch", 22, 22, 7, 7, "screen", 0)
        for i in range(5):
            a = -math.pi / 2 + 2 * math.pi * i / 5
            b.line(f"seam_{i}", [(22 + 7 * math.cos(a), 22 + 7 * math.sin(a)), (22 + 16 * math.cos(a), 22 + 16 * math.sin(a))], 2)
        b.grip(22, 22)
        b.ground()

    @prop("prop.sport.basketball_v1", "Koripallo", 48, 48, ["koripallo", "basketball", "pallo", "ball", "urheilu", "sport"])
    def _(b):
        b.oval("ball", 24, 24, 21.5, 21.5, "court")
        b.line("seam_h", [(4, 24), (44, 24)], 2)
        b.line("seam_v", [(24, 4), (24, 44)], 2)
        b.line("seam_l", [(12, 8), (18, 24), (12, 40)], 2)
        b.line("seam_r", [(36, 8), (30, 24), (36, 40)], 2)
        b.grip(24, 24)
        b.ground()

    @prop("prop.sport.tennis_racket_v1", "Tennismaila", 50, 130, ["tennis", "maila", "racket", "urheilu", "sport", "käsiesine", "hand_prop"])
    def _(b):
        b.oval("frame", 25, 32, 21, 28, "accent_red")
        b.oval("strings", 25, 32, 16, 23, "net", 0)
        for i in range(-2, 3):
            b.line(f"string_v{i + 2}", [(25 + i * 6, 12), (25 + i * 6, 52)], 1, "metal_dark")
            b.line(f"string_h{i + 2}", [(11, 32 + i * 8), (39, 32 + i * 8)], 1, "metal_dark")
        b.line("throat", [(25, 60), (25, 80)], 5, "accent_red")
        b.line("handle", [(25, 80), (25, 126)], 7, "screen")
        b.grip(25, 104)

    @prop("prop.sport.hockey_stick_v1", "Jääkiekkomaila", 90, 260, ["jääkiekko", "hockey", "maila", "stick", "talvi", "winter", "urheilu", "sport", "käsiesine", "hand_prop"])
    def _(b):
        b.line("shaft", [(70, 4), (40, 236)], 7, "wood")
        b.line("tape_top", [(69.5, 8), (66, 34)], 7, "screen")
        b.line("blade", [(40, 234), (8, 254)], 8, "wood_dark")
        b.grip(66, 40)
        b.ground()

    @prop("prop.sport.dumbbell_v1", "Käsipaino", 80, 34, ["käsipaino", "dumbbell", "kuntosali", "gym", "urheilu", "sport", "käsiesine", "hand_prop"])
    def _(b):
        b.line("bar", [(14, 17), (66, 17)], 6, "metal")
        b.box("weight_left", 6, 3, 16, b.F - 3, "screen")
        b.box("weight_right", 58, 3, 16, b.F - 3, "screen")
        b.grip(40, 17)
        b.ground()

    @prop("prop.sport.football_goal_v1", "Jalkapallomaali", 420, 230, ["maali", "goal", "jalkapallo", "football", "kenttä", "field", "urheilu", "sport", "outdoor"])
    def _(b):
        b.box("net", 14, 14, 392, b.F - 14, "net", 0)
        for i in range(1, 12):
            b.line(f"net_v{i}", [(14 + i * 32.6, 14), (14 + i * 32.6, 226)], 1, "metal")
        for i in range(1, 7):
            b.line(f"net_h{i}", [(14, 14 + i * 30), (406, 14 + i * 30)], 1, "metal")
        b.line("post_left", [(10, 10), (10, 226)], 8, "white")
        b.line("post_right", [(410, 10), (410, 226)], 8, "white")
        b.line("crossbar", [(6, 10), (414, 10)], 8, "white")
        b.doorway(18, 18, 384, 208, "goal_mouth")
        b.ground()

    @prop("prop.sport.basketball_hoop_v1", "Koripallokori", 160, 460, ["kori", "hoop", "koripallo", "basketball", "piha", "yard", "urheilu", "sport", "outdoor"])
    def _(b):
        b.line("pole", [(40, 80), (40, 452)], 8, "metal_dark")
        b.line("arm", [(40, 100), (80, 100)], 6, "metal_dark")
        b.box("board", 70, 6, 84, 110, "white")
        b.box("target", 98, 52, 28, 24, "white", 3)
        b.oval("rim", 112, 104, 20, 4, "court", 3)
        for i, x in enumerate((96, 106, 118, 128)):
            b.line(f"net_{i}", [(x, 106), (106 + (x - 112) * 0.5 + 6, 136)], 2, "net")
        b.box("base", 16, 440, 48, b.F - 440, "screen")
        b.ground()

    @prop("prop.sport.trophy_v1", "Pokaali", 70, 100, ["pokaali", "trophy", "voitto", "win", "palkinto", "prize", "urheilu", "sport"])
    def _(b):
        b.oval("handle_left", 14, 30, 10, 14, "brass", 3)
        b.oval("handle_right", 56, 30, 10, 14, "brass", 3)
        b.oval("cup", 35, 28, 22, 24, "brass")
        b.box("stem", 30, 50, 10, 22, "brass", 2)
        b.box("base", 14, 72, 42, b.F - 72, "wood_dark")
        b.box("plaque", 22, 80, 26, 8, "paper", 0)
        b.grip(35, 60)
        b.ground()

    @prop("prop.sport.skateboard_v1", "Rullalauta", 160, 34, ["rullalauta", "skateboard", "nuoret", "youth", "urheilu", "sport"])
    def _(b):
        b.line("deck", [(8, 8), (22, 14), (138, 14), (152, 8)], 8, "toy_blue")
        b.box("truck_left", 30, 18, 20, 4, "metal_dark", 0)
        b.box("truck_right", 110, 18, 20, 4, "metal_dark", 0)
        b.oval("wheel_left", 40, 26.5, 7, 6, "ivory")
        b.oval("wheel_right", 120, 26.5, 7, 6, "ivory")
        b.stand(80, 10, "deck")
        b.ground()

    # ------------------------------------------------------------ health care
    @prop("prop.medical.hospital_bed_v1", "Sairaalasänky", 360, 190, ["sairaala", "hospital", "sänky", "bed", "potilas", "patient", "terveys", "health"])
    def _(b):
        b.line("head_frame", [(16, 186), (16, 20), (60, 20), (60, 110)], 6, "metal")
        b.line("foot_frame", [(340, 186), (340, 80), (304, 80), (304, 110)], 6, "metal")
        b.box("frame", 16, 106, 328, 18, "metal_dark")
        b.box("mattress", 22, 80, 316, 28, "white")
        b.oval("pillow", 64, 74, 34, 12, "paper")
        b.box("sheet", 110, 72, 228, 36, "medical")
        b.line("rail", [(150, 64), (290, 64)], 5, "metal")
        b.line("rail_post_a", [(170, 64), (170, 80)], 4, "metal")
        b.line("rail_post_b", [(270, 64), (270, 80)], 4, "metal")
        b.oval("wheel_left", 40, 178, 9.5, 9.5, "screen")
        b.oval("wheel_right", 320, 178, 9.5, 9.5, "screen")
        b.line("leg_left", [(40, 124), (40, 168)], 5, "metal")
        b.line("leg_right", [(320, 124), (320, 168)], 5, "metal")
        b.seat(200, 80)
        b.ground()

    @prop("prop.medical.wheelchair_v1", "Pyörätuoli", 150, 170, ["pyörätuoli", "wheelchair", "sairaala", "hospital", "esteettömyys", "accessibility", "terveys", "health"])
    def _(b):
        b.oval("wheel", 66, 112, 54.5, 54.5, "metal_dark")
        b.oval("wheel_inner", 66, 112, 44, 44, "medical", 2)
        b.oval("hub", 66, 112, 6, 6, "metal", 2)
        b.line("back", [(30, 20), (30, 100)], 6, "metal")
        b.box("backrest", 22, 24, 20, 56, "fabric_dark", 2)
        b.box("seat_pad", 26, 84, 84, 12, "fabric_dark")
        b.line("handle", [(30, 20), (14, 18)], 6, "screen")
        b.line("front_post", [(108, 96), (126, 150)], 5, "metal")
        b.line("footrest", [(116, 150), (140, 150)], 5, "metal")
        b.oval("caster", 128, 160, 8, 8.5, "screen")
        b.seat(66, 84)
        b.ground()

    @prop("prop.medical.first_aid_kit_v1", "Ensiapulaukku", 90, 66, ["ensiapu", "first_aid", "laukku", "kit", "terveys", "health", "käsiesine", "hand_prop"])
    def _(b):
        b.line("handle", [(32, 14), (32, 4), (58, 4), (58, 14)], 5, "screen")
        b.box("case", 4, 14, 82, b.F - 14, "white")
        b.box("cross_v", 38, 24, 14, 32, "cross_red", 0)
        b.box("cross_h", 29, 33, 32, 14, "cross_red", 0)
        b.grip(45, 4)
        b.ground()

    @prop("prop.medical.crutch_v1", "Kainalosauva", 40, 260, ["kainalosauva", "crutch", "terveys", "health", "loukkaantunut", "injured", "käsiesine", "hand_prop"])
    def _(b):
        b.line("pad", [(6, 8), (34, 8)], 8, "screen")
        b.line("rail_left", [(10, 10), (18, 150)], 4, "metal")
        b.line("rail_right", [(30, 10), (22, 150)], 4, "metal")
        b.line("hand_grip", [(12, 96), (28, 96)], 6, "screen")
        b.line("shaft", [(20, 148), (20, 252)], 5, "metal")
        b.line("tip", [(20, 250), (20, 256)], 8, "screen")
        b.grip(20, 96)
        b.ground()

    @prop("prop.medical.iv_stand_v1", "Tippateline", 90, 340, ["tippa", "iv_drip", "teline", "stand", "sairaala", "hospital", "terveys", "health"])
    def _(b):
        b.line("pole", [(45, 12), (45, 326)], 5, "metal")
        b.line("hook_bar", [(20, 14), (70, 14)], 4, "metal")
        b.box("bag", 52, 22, 26, 40, "glass_clear", 2)
        b.line("tube", [(65, 62), (65, 140), (56, 170)], 2, "glass")
        b.line("leg_left", [(45, 318), (8, 328)], 5, "metal")
        b.line("leg_right", [(45, 318), (82, 328)], 5, "metal")
        b.oval("caster_left", 10, 332, 5, 5.5, "screen")
        b.oval("caster_right", 80, 332, 5, 5.5, "screen")
        b.ground()

    @prop("prop.medical.medicine_bottle_v1", "Lääkepurkki", 30, 44, ["lääke", "medicine", "purkki", "bottle", "apteekki", "pharmacy", "terveys", "health", "käsiesine", "hand_prop"])
    def _(b):
        b.box("cap", 6, 3, 18, 9, "white", 2)
        b.box("bottle", 4, 12, 22, b.F - 12, "accent")
        b.box("label", 4, 20, 22, 12, "paper", 0)
        b.grip(15, 26)
        b.ground()

    # ------------------------------------------------------------ bathroom
    @prop("prop.bathroom.toilet_v1", "WC-istuin", 110, 150, ["wc", "toilet", "vessa", "kylpyhuone", "bathroom"])
    def _(b):
        b.box("tank", 10, 6, 50, 72, "porcelain")
        b.box("flush", 26, 14, 18, 6, "metal", 0)
        b.box("bowl_upper", 10, 76, 92, 14, "porcelain")
        b.oval("bowl", 56, 98, 46, 18, "porcelain")
        b.box("pedestal", 40, 108, 32, b.F - 108, "porcelain")
        b.seat(56, 78)
        b.ground()

    @prop("prop.bathroom.bathtub_v1", "Kylpyamme", 340, 130, ["kylpyamme", "bathtub", "kylpy", "bath", "kylpyhuone", "bathroom"])
    def _(b):
        b.line("tap", [(310, 18), (310, 6), (290, 6), (290, 16)], 5, "metal")
        b.oval("foam_a", 120, 26, 34, 12, "white", 2)
        b.oval("foam_b", 170, 22, 30, 12, "white", 2)
        b.box("tub", 8, 24, 324, 86, "porcelain")
        b.box("rim", 4, 24, 332, 12, "white")
        b.line("foot_left", [(36, 110), (30, 126)], 8, "metal")
        b.line("foot_right", [(304, 110), (310, 126)], 8, "metal")
        b.seat(170, 40)
        b.ground()

    @prop("prop.bathroom.washbasin_v1", "Pesuallas", 120, 300, ["pesuallas", "washbasin", "lavuaari", "sink", "peili", "mirror", "kylpyhuone", "bathroom"])
    def _(b):
        b.box("mirror_frame", 20, 6, 80, 96, "wood")
        b.box("mirror", 28, 14, 64, 80, "glass", 2)
        b.line("tap", [(60, 140), (60, 128), (72, 128)], 5, "metal")
        b.box("basin", 8, 142, 104, 30, "porcelain")
        b.box("pedestal", 44, 172, 32, b.F - 172, "porcelain")
        b.stand(60, 142, "basin")
        b.ground()

    @prop("prop.bathroom.shower_v1", "Suihku", 140, 420, ["suihku", "shower", "kylpyhuone", "bathroom", "sauna"])
    def _(b):
        b.box("tray", 4, 396, 132, b.F - 396, "porcelain")
        b.box("curtain", 10, 40, 30, 356, "towel")
        for i in range(3):
            b.line(f"fold_{i}", [(16 + i * 8, 46), (16 + i * 8, 390)], 2, "fabric_dark")
        b.line("rail", [(6, 36), (134, 36)], 5, "metal")
        b.line("pipe", [(120, 60), (120, 200)], 5, "metal")
        b.line("head_arm", [(120, 60), (96, 60)], 5, "metal")
        b.oval("head", 90, 66, 14, 6, "metal", 2)
        b.box("mixer", 110, 200, 20, 14, "metal", 2)
        b.stand(80, 396, "tray")
        b.ground()

    @prop("prop.bathroom.towel_rack_v1", "Pyyheteline", 90, 230, ["pyyhe", "towel", "teline", "rack", "kylpyhuone", "bathroom"])
    def _(b):
        b.line("post_left", [(12, 6), (12, 222)], 5, "metal")
        b.line("post_right", [(78, 6), (78, 222)], 5, "metal")
        b.line("bar", [(12, 40), (78, 40)], 5, "metal")
        b.box("towel", 18, 38, 54, 110, "towel")
        b.box("towel_stripe", 18, 120, 54, 8, "white", 0)
        b.line("foot_left", [(4, 226), (22, 226)], 6, "metal")
        b.line("foot_right", [(68, 226), (86, 226)], 6, "metal")
        b.ground()

    @prop("prop.bathroom.toothbrush_v1", "Hammasharja", 20, 80, ["hammasharja", "toothbrush", "aamu", "morning", "kylpyhuone", "bathroom", "käsiesine", "hand_prop"])
    def _(b):
        b.line("handle", [(10, 22), (10, 76)], 6, "toy_blue")
        b.box("bristles", 4, 4, 9, 18, "white", 2)
        b.grip(10, 56)

    # ------------------------------------------------------------ bedroom and household
    @prop("prop.household.wardrobe_v1", "Vaatekaappi", 180, 360, ["vaatekaappi", "wardrobe", "kaappi", "cabinet", "makuuhuone", "bedroom", "furniture"])
    def _(b):
        b.box("body", 6, 10, 168, 330, "wood")
        b.box("door_left", 14, 20, 74, 310, "wood_light", 2)
        b.box("door_right", 92, 20, 74, 310, "wood_light", 2)
        b.line("handle_left", [(80, 160), (80, 190)], 4, "metal_dark")
        b.line("handle_right", [(100, 160), (100, 190)], 4, "metal_dark")
        b.box("top", 2, 4, 176, 10, "wood_dark")
        b.box("plinth", 10, 340, 160, b.F - 340, "wood_dark")
        b.doorway(14, 20, 152, 310, "doors")
        b.ground()

    @prop("prop.household.dresser_v1", "Lipasto", 170, 150, ["lipasto", "dresser", "laatikosto", "drawers", "makuuhuone", "bedroom", "furniture"])
    def _(b):
        b.box("body", 6, 12, 158, 126, "wood")
        for i in range(3):
            b.box(f"drawer_{i}", 14, 20 + i * 38, 142, 32, "wood_light", 2)
            b.oval(f"knob_{i}", 85, 36 + i * 38, 4, 4, "metal_dark", 2)
        b.box("top", 2, 4, 166, 10, "wood_dark")
        legs(b, [12, 150], 138, 8)
        b.stand(85, 4, "surface")
        b.ground()

    @prop("prop.household.nightstand_v1", "Yöpöytä", 80, 100, ["yöpöytä", "nightstand", "makuuhuone", "bedroom", "furniture"])
    def _(b):
        b.box("body", 6, 12, 68, 66, "wood")
        b.box("drawer", 12, 20, 56, 24, "wood_light", 2)
        b.oval("knob", 40, 32, 3.5, 3.5, "metal_dark", 2)
        b.box("top", 2, 4, 76, 10, "wood_dark")
        legs(b, [8, 64], 78, 8)
        b.stand(40, 4, "surface")
        b.ground()

    @prop("prop.household.alarm_clock_v1", "Herätyskello", 60, 64, ["herätyskello", "alarm_clock", "kello", "clock", "aamu", "morning", "makuuhuone", "bedroom"])
    def _(b):
        b.oval("bell_left", 14, 12, 9, 8, "accent_red", 2)
        b.oval("bell_right", 46, 12, 9, 8, "accent_red", 2)
        b.oval("body", 30, 34, 24, 24, "accent_red")
        b.oval("face", 30, 34, 18, 18, "paper", 2)
        b.line("hand_h", [(30, 34), (30, 22)], 3)
        b.line("hand_m", [(30, 34), (40, 38)], 2)
        b.line("foot_left", [(16, 54), (10, 61)], 5, "screen")
        b.line("foot_right", [(44, 54), (50, 61)], 5, "screen")
        b.ground()

    @prop("prop.household.laundry_basket_v1", "Pyykkikori", 110, 90, ["pyykki", "laundry", "kori", "basket", "kotityö", "chores", "household"])
    def _(b):
        b.oval("shirt", 40, 16, 22, 12, "blue", 2)
        b.oval("sock", 74, 18, 16, 9, "accent", 2)
        b.box("basket", 8, 18, 94, b.F - 18, "wood_light")
        for i in range(1, 6):
            b.line(f"weave_{i}", [(8 + i * 15.7, 22), (8 + i * 15.7, 84)], 2, "wood_dark")
        b.box("rim", 4, 14, 102, 10, "wood")
        b.ground()

    @prop("prop.household.washing_machine_v1", "Pesukone", 130, 160, ["pesukone", "washing_machine", "pyykki", "laundry", "kodinkone", "appliance", "kylpyhuone", "bathroom"])
    def _(b):
        b.box("body", 6, 6, 118, b.F - 6, "white")
        b.box("panel", 6, 6, 118, 26, "metal", 3)
        b.oval("dial", 100, 19, 7, 7, "metal_dark", 2)
        b.box("display", 24, 13, 40, 12, "screen", 0)
        b.oval("door", 65, 92, 42, 42, "metal")
        b.oval("glass", 65, 92, 32, 32, "glass", 2)
        b.oval("water", 65, 102, 26, 18, "water", 0)
        b.stand(65, 6, "top")
        b.ground()

    @prop("prop.household.vacuum_cleaner_v1", "Pölynimuri", 150, 110, ["imuri", "vacuum", "siivous", "cleaning", "kotityö", "chores", "household"])
    def _(b):
        b.line("hose", [(70, 52), (100, 20), (128, 14)], 7, "screen")
        b.line("pipe", [(128, 14), (144, 90)], 6, "metal")
        b.box("nozzle", 120, 92, 28, b.F - 92, "screen")
        b.oval("body", 46, 72, 40, 34.5, "accent_red")
        b.oval("wheel", 46, 96, 12, 11, "screen")
        b.ground()

    @prop("prop.household.ironing_board_v1", "Silityslauta", 260, 150, ["silitys", "ironing", "lauta", "board", "silitysrauta", "iron", "kotityö", "chores", "household"])
    def _(b):
        b.line("leg_a", [(60, 24), (200, 146)], 5, "metal")
        b.line("leg_b", [(200, 24), (60, 146)], 5, "metal")
        b.oval("board_end", 236, 18, 20, 10, "towel")
        b.box("board", 8, 8, 228, 20, "towel")
        b.box("iron", 150, 0, 44, 8, "metal_dark", 0)
        b.stand(120, 8, "surface")
        b.ground()

    # ------------------------------------------------------------ garden and farm
    @prop("prop.garden.wheelbarrow_v1", "Kottikärryt", 230, 110, ["kottikärryt", "wheelbarrow", "puutarha", "garden", "piha", "yard", "outdoor"])
    def _(b):
        b.line("handle", [(8, 30), (150, 60)], 6, "wood_dark")
        b.box("tray", 70, 20, 130, 48, "tractor")
        b.box("soil", 74, 14, 122, 10, "soil", 0)
        b.line("leg", [(80, 68), (72, 106)], 6, "metal_dark")
        b.oval("wheel", 196, 86, 21, 21.5, "screen")
        b.oval("hub", 196, 86, 5, 5, "metal", 2)
        b.grip(8, 30)
        b.ground()

    @prop("prop.garden.watering_can_v1", "Kastelukannu", 90, 70, ["kastelukannu", "watering_can", "kastelu", "watering", "puutarha", "garden", "käsiesine", "hand_prop"])
    def _(b):
        b.line("spout", [(60, 46), (84, 16)], 6, "tractor")
        b.oval("rose", 84, 14, 5, 6, "tractor", 2)
        b.line("handle", [(18, 24), (30, 6), (52, 6), (58, 24)], 5, "tractor")
        b.box("can", 12, 24, 52, b.F - 24, "tractor")
        b.grip(40, 6)
        b.ground()

    @prop("prop.garden.rake_v1", "Harava", 70, 300, ["harava", "rake", "syksy", "autumn", "lehdet", "leaves", "puutarha", "garden", "käsiesine", "hand_prop"])
    def _(b):
        b.line("shaft", [(35, 4), (35, 270)], 6, "wood")
        b.line("head", [(6, 272), (64, 272)], 6, "metal_dark")
        for i in range(7):
            x = 9 + i * 8.6
            b.line(f"tine_{i}", [(x, 272), (x, 294)], 3, "metal_dark")
        b.grip(35, 60)
        b.ground()

    @prop("prop.garden.spade_v1", "Lapio", 50, 290, ["lapio", "spade", "kaivaa", "dig", "puutarha", "garden", "käsiesine", "hand_prop"])
    def _(b):
        b.line("grip_bar", [(12, 6), (38, 6)], 6, "screen")
        b.line("shaft", [(25, 6), (25, 210)], 6, "wood")
        b.box("blade", 8, 206, 34, b.F - 206, "metal")
        b.grip(25, 40)
        b.ground()

    @prop("prop.garden.flower_bed_v1", "Kukkapenkki", 260, 90, ["kukka", "flower", "kukkapenkki", "flower_bed", "kesä", "summer", "puutarha", "garden", "outdoor"])
    def _(b):
        flowers = ["petal_pink", "accent", "petal_white", "accent_red", "petal_pink", "accent", "petal_white"]
        for i, slot in enumerate(flowers):
            x = 26 + i * 35
            top = 18 + (i * 13) % 20
            b.line(f"stem_{i}", [(x, 60), (x, top)], 3, "leaf_dark")
            b.oval(f"leaf_{i}", x + 7, top + 24, 7, 3.5, "leaf", 0)
            b.oval(f"bloom_{i}", x, top, 9, 9, slot, 2)
            b.oval(f"centre_{i}", x, top, 3, 3, "accent", 0)
        b.box("soil", 6, 58, 248, b.F - 58, "soil")
        b.ground()

    @prop("prop.garden.garden_gnome_v1", "Puutarhatonttu", 60, 110, ["tonttu", "gnome", "puutarha", "garden", "koriste", "decor", "outdoor"])
    def _(b):
        b.line("hat", [(30, 6), (16, 40), (44, 40), (30, 6)], 8, "accent_red")
        b.oval("face", 30, 50, 12, 11, "cream", 2)
        b.oval("beard", 30, 62, 13, 10, "white", 2)
        b.box("body", 14, 66, 32, 30, "toy_blue")
        b.box("boots", 12, 96, 36, b.F - 96, "screen")
        b.ground()

    @prop("prop.farm.haystack_v1", "Heinäpaali", 160, 110, ["heinä", "hay", "paali", "bale", "maatila", "farm", "maaseutu", "countryside", "outdoor"])
    def _(b):
        b.box("bale", 6, 10, 148, b.F - 10, "hay")
        for i in range(1, 8):
            b.line(f"straw_{i}", [(6 + i * 18.5, 14), (10 + i * 18.5, 104)], 2, "crust")
        b.line("twine_a", [(50, 10), (50, 108)], 3, "accent_red")
        b.line("twine_b", [(110, 10), (110, 108)], 3, "accent_red")
        b.seat(80, 10)
        b.ground()

    @prop("prop.farm.scarecrow_v1", "Variksenpelätin", 180, 330, ["variksenpelätin", "scarecrow", "pelto", "field", "maatila", "farm", "syksy", "autumn", "outdoor"])
    def _(b):
        b.line("post", [(90, 60), (90, 322)], 7, "wood_dark")
        b.line("arm_bar", [(20, 110), (160, 110)], 6, "wood_dark")
        b.box("shirt", 54, 96, 72, 90, "fabric")
        b.box("patch", 60, 140, 20, 18, "accent", 2)
        b.line("sleeve_left", [(54, 108), (20, 112)], 8, "fabric")
        b.line("sleeve_right", [(126, 108), (160, 112)], 8, "fabric")
        b.line("straw_left", [(16, 112), (8, 124)], 3, "hay")
        b.line("straw_right", [(164, 112), (172, 124)], 3, "hay")
        b.oval("head", 90, 70, 22, 22, "cardboard")
        b.oval("eye_left", 82, 66, 3, 3, "screen", 0)
        b.oval("eye_right", 98, 66, 3, 3, "screen", 0)
        b.box("hat_brim", 58, 44, 64, 6, "crust", 2)
        b.box("hat_crown", 72, 22, 36, 24, "crust", 2)
        b.ground()

    @prop("prop.farm.fence_wood_v1", "Puuaita", 300, 120, ["aita", "fence", "puu", "wood", "maatila", "farm", "piha", "yard", "outdoor"])
    def _(b):
        b.box("rail_top", 4, 30, 292, 12, "wood_light", 2)
        b.box("rail_low", 4, 74, 292, 12, "wood_light", 2)
        for i in range(5):
            b.box(f"post_{i}", 8 + i * 69, 10, 14, b.F - 10, "wood")
        b.ground()

    @prop("prop.farm.milk_churn_v1", "Maitotonkka", 60, 100, ["maitotonkka", "milk_churn", "maito", "milk", "maatila", "farm", "maaseutu", "countryside"])
    def _(b):
        b.box("lid", 20, 4, 20, 10, "metal_dark", 2)
        b.box("neck", 18, 14, 24, 18, "metal")
        b.box("body", 6, 30, 48, b.F - 30, "metal")
        b.line("band", [(6, 60), (54, 60)], 3, "metal_dark")
        b.grip(30, 8)
        b.ground()

    # ------------------------------------------------------------ toys
    @prop("prop.toy.teddy_bear_v1", "Nalle", 70, 90, ["nalle", "teddy_bear", "lelu", "toy", "lapsi", "child", "käsiesine", "hand_prop"])
    def _(b):
        b.oval("ear_left", 20, 10, 8, 8, "fur", 2)
        b.oval("ear_right", 50, 10, 8, 8, "fur", 2)
        b.oval("head", 35, 24, 18, 17, "fur", 2)
        b.oval("snout", 35, 30, 7, 5, "cream", 0)
        b.oval("nose", 35, 28, 2.5, 2, "screen", 0)
        b.oval("eye_left", 28, 20, 2, 2, "screen", 0)
        b.oval("eye_right", 42, 20, 2, 2, "screen", 0)
        b.oval("body", 35, 62, 20, 22, "fur", 2)
        b.oval("arm_left", 14, 58, 7, 12, "fur", 2)
        b.oval("arm_right", 56, 58, 7, 12, "fur", 2)
        b.oval("foot_left", 24, 80, 9, 7, "fur", 2)
        b.oval("foot_right", 46, 80, 9, 7, "fur", 2)
        b.grip(35, 50)
        b.ground()

    @prop("prop.toy.toy_blocks_v1", "Rakennuspalikat", 120, 100, ["palikat", "blocks", "lelu", "toy", "lastenhuone", "kids_room", "lapsi", "child"])
    def _(b):
        b.box("block_a", 6, 60, 36, b.F - 60, "toy_red")
        b.box("block_b", 44, 60, 36, b.F - 60, "toy_blue")
        b.box("block_c", 82, 60, 32, b.F - 60, "toy_yellow")
        b.box("block_d", 26, 24, 36, 36, "toy_green")
        b.box("block_e", 64, 24, 32, 36, "toy_red")
        b.line("roof_left", [(38, 22), (50, 6)], 6, "toy_blue")
        b.line("roof_right", [(62, 22), (50, 6)], 6, "toy_blue")
        b.ground()

    @prop("prop.toy.kite_v1", "Leija", 120, 220, ["leija", "kite", "tuuli", "wind", "kesä", "summer", "lelu", "toy", "outdoor"])
    def _(b):
        b.line("left_top", [(60, 6), (14, 60)], 6, "toy_red")
        b.line("right_top", [(60, 6), (106, 60)], 6, "toy_blue")
        b.line("left_bottom", [(14, 60), (60, 140)], 6, "toy_yellow")
        b.line("right_bottom", [(106, 60), (60, 140)], 6, "toy_green")
        b.line("spar_v", [(60, 6), (60, 140)], 3, "wood_dark")
        b.line("spar_h", [(14, 60), (106, 60)], 3, "wood_dark")
        b.line("tail", [(60, 140), (48, 170), (70, 194), (56, 216)], 2, "line")
        for i, (x, y) in enumerate(((52, 162), (64, 186), (58, 208))):
            b.oval(f"bow_{i}", x, y, 6, 3, "toy_red", 0)
        b.grip(56, 216)

    @prop("prop.toy.rubber_duck_v1", "Kumiankka", 50, 44, ["kumiankka", "rubber_duck", "kylpy", "bath", "lelu", "toy", "kylpyhuone", "bathroom"])
    def _(b):
        b.oval("body", 24, 32, 21, 10.5, "toy_yellow")
        b.oval("head", 32, 14, 11, 10, "toy_yellow")
        b.oval("beak", 44, 16, 5, 3, "accent_red", 0)
        b.oval("eye", 34, 11, 2, 2, "screen", 0)
        b.grip(24, 32)
        b.ground()

    @prop("prop.toy.toy_car_v1", "Leikkiauto", 90, 44, ["leikkiauto", "toy_car", "auto", "car", "lelu", "toy", "lapsi", "child"])
    def _(b):
        b.box("cabin", 24, 6, 40, 18, "toy_red")
        b.box("window", 30, 10, 28, 10, "glass", 0)
        b.box("body", 6, 20, 78, 14, "toy_red")
        b.oval("wheel_left", 24, 34, 8.5, 8.5, "screen")
        b.oval("wheel_right", 66, 34, 8.5, 8.5, "screen")
        b.grip(44, 14)
        b.ground()

    @prop("prop.toy.beach_ball_v1", "Rantapallo", 60, 60, ["rantapallo", "beach_ball", "pallo", "ball", "kesä", "summer", "lelu", "toy"])
    def _(b):
        b.oval("ball", 30, 30, 26.5, 26.5, "white")
        b.oval("panel_red", 20, 30, 8, 24, "toy_red", 0)
        b.oval("panel_blue", 40, 30, 8, 24, "toy_blue", 0)
        b.oval("cap", 30, 30, 5, 5, "toy_yellow", 2)
        b.grip(30, 30)
        b.ground()

    @prop("prop.toy.rocking_horse_v1", "Keinuhevonen", 170, 150, ["keinuhevonen", "rocking_horse", "lelu", "toy", "lastenhuone", "kids_room", "lapsi", "child"])
    def _(b):
        b.line("rocker", [(8, 134), (50, 146), (120, 146), (162, 134)], 6, "wood_dark")
        b.line("leg_front", [(126, 92), (136, 142)], 6, "wood")
        b.line("leg_back", [(44, 92), (34, 142)], 6, "wood")
        b.oval("body", 85, 84, 52, 18, "wood")
        b.line("neck", [(124, 76), (142, 30)], 8, "wood")
        b.oval("head", 148, 28, 16, 10, "wood")
        b.line("mane", [(128, 72), (136, 36), (142, 18)], 5, "accent_red")
        b.oval("saddle", 80, 68, 20, 7, "accent_red", 2)
        b.seat(80, 66)
        b.ground()

    # ------------------------------------------------------------ workshop
    @prop("prop.tool.hammer_v1", "Vasara", 60, 110, ["vasara", "hammer", "työkalu", "tool", "rakentaminen", "building", "käsiesine", "hand_prop"])
    def _(b):
        b.line("handle", [(30, 20), (30, 104)], 7, "wood")
        b.box("head", 6, 6, 48, 16, "metal_dark")
        b.grip(30, 80)

    @prop("prop.tool.saw_v1", "Käsisaha", 160, 60, ["saha", "saw", "työkalu", "tool", "puutyö", "woodwork", "käsiesine", "hand_prop"])
    def _(b):
        b.box("blade", 40, 14, 116, 26, "metal", 2)
        for i in range(10):
            x = 44 + i * 11
            b.line(f"tooth_{i}", [(x, 40), (x + 5, 46), (x + 10, 40)], 2, "metal_dark")
        b.box("handle", 6, 8, 38, 40, "wood")
        b.box("handle_hole", 16, 18, 18, 18, "white", 2)
        b.grip(25, 28)

    @prop("prop.tool.ladder_v1", "Tikkaat", 110, 420, ["tikkaat", "ladder", "työkalu", "tool", "korkealla", "high", "remontti", "renovation"])
    def _(b):
        b.line("rail_left", [(30, 4), (8, 414)], 7, "wood")
        b.line("rail_right", [(80, 4), (102, 414)], 7, "wood")
        for i in range(8):
            y = 40 + i * 48
            k = y / 410
            b.line(f"rung_{i}", [(30 - 22 * k, y), (80 + 22 * k, y)], 6, "wood_dark")
        b.stand(55, 136, "step")
        b.ground()

    @prop("prop.tool.toolbox_v1", "Työkalupakki", 120, 74, ["työkalupakki", "toolbox", "työkalu", "tool", "korjaus", "repair", "käsiesine", "hand_prop"])
    def _(b):
        b.line("handle", [(40, 16), (40, 4), (80, 4), (80, 16)], 5, "screen")
        b.box("box", 6, 16, 108, b.F - 16, "accent_red")
        b.box("lid_line", 6, 16, 108, 14, "accent_red", 3)
        b.box("latch", 54, 26, 12, 10, "metal", 2)
        b.grip(60, 4)
        b.ground()

    @prop("prop.tool.power_drill_v1", "Porakone", 80, 70, ["porakone", "drill", "työkalu", "tool", "remontti", "renovation", "käsiesine", "hand_prop"])
    def _(b):
        b.line("bit", [(62, 18), (78, 18)], 3, "metal")
        b.box("chuck", 52, 12, 12, 12, "metal_dark", 2)
        b.box("body", 6, 6, 48, 24, "toy_yellow")
        b.box("handle", 14, 28, 18, 30, "screen")
        b.box("battery", 8, 56, 30, 12, "screen", 2)
        b.grip(23, 42)

    @prop("prop.tool.workbench_v1", "Työpenkki", 300, 140, ["työpenkki", "workbench", "verstas", "workshop", "autotalli", "garage", "furniture"])
    def _(b):
        legs(b, [16, 270], 30, 14)
        b.box("shelf", 20, 100, 260, 10, "wood_dark", 2)
        b.box("box_a", 40, 78, 50, 22, "cardboard", 2)
        b.box("top", 4, 16, 292, 16, "wood")
        b.box("vise", 230, 2, 40, 14, "metal_dark", 2)
        b.stand(150, 16, "surface")
        b.ground()

    # ------------------------------------------------------------ camping and nature
    @prop("prop.camping.tent_v1", "Teltta", 300, 190, ["teltta", "tent", "retki", "camping", "metsä", "forest", "outdoor"])
    def _(b):
        b.line("left_wall", [(150, 10), (10, 184)], 8, "tent_dark")
        b.line("right_wall", [(150, 10), (290, 184)], 8, "tent_dark")
        for i in range(24):
            t = (i + 0.5) / 24
            y = 14 + t * 166
            half = 140 * t
            b.line(f"panel_{i}", [(150 - half + 4, y), (150 + half - 4, y)], 8, "tent")
        b.line("door_left", [(150, 60), (124, 184)], 4, "tent_dark")
        b.line("door_right", [(150, 60), (176, 184)], 4, "tent_dark")
        b.line("pole", [(150, 4), (150, 14)], 4, "metal_dark")
        b.doorway(128, 70, 44, 110, "door")
        b.ground()

    @prop("prop.camping.campfire_v1", "Nuotio", 140, 100, ["nuotio", "campfire", "tuli", "fire", "retki", "camping", "ilta", "evening", "outdoor"])
    def _(b):
        for i in range(8):
            a = math.pi * (0.05 + 0.9 * i / 7)
            b.oval(f"stone_{i}", 70 - 58 * math.cos(a), 88, 9, 8.5, "rock" if i % 2 else "rock_dark")
        b.line("log_a", [(28, 84), (110, 64)], 8, "bark")
        b.line("log_b", [(30, 64), (112, 84)], 8, "bark")
        b.oval("flame", 70, 46, 24, 34, "flame", 0)
        b.oval("flame_core", 70, 58, 12, 20, "flame_core", 0)
        b.ground()

    @prop("prop.camping.hiking_backpack_v1", "Rinkka", 80, 120, ["rinkka", "backpack", "reppu", "retki", "hiking", "matka", "travel", "käsiesine", "hand_prop"])
    def _(b):
        b.oval("bedroll", 40, 14, 30, 10, "accent_red", 2)
        b.box("pack", 10, 22, 60, b.F - 22, "tractor")
        b.box("flap", 10, 22, 60, 30, "leaf_dark", 2)
        b.box("pocket", 22, 74, 36, 30, "leaf_dark", 2)
        b.line("strap_left", [(18, 52), (18, 110)], 4, "screen")
        b.line("strap_right", [(62, 52), (62, 110)], 4, "screen")
        b.grip(40, 26)
        b.ground()

    @prop("prop.camping.lantern_v1", "Lyhty", 50, 90, ["lyhty", "lantern", "valo", "light", "retki", "camping", "ilta", "evening", "käsiesine", "hand_prop"])
    def _(b):
        b.line("bail", [(12, 22), (25, 4), (38, 22)], 3, "metal_dark")
        b.box("top", 8, 18, 34, 10, "metal_dark", 2)
        b.oval("glow", 25, 52, 12, 18, "light", 0)
        b.box("glass", 10, 28, 30, 46, "glass_clear", 2)
        b.box("base", 6, 74, 38, b.F - 74, "metal_dark")
        b.grip(25, 4)
        b.ground()

    @prop("prop.nature.log_v1", "Tukki", 220, 60, ["tukki", "log", "puu", "wood", "metsä", "forest", "istuin", "seat", "outdoor"])
    def _(b):
        b.box("log", 26, 6, 170, b.F - 6, "bark")
        b.line("grain_a", [(40, 22), (180, 22)], 2, "wood_dark")
        b.line("grain_b", [(50, 40), (170, 40)], 2, "wood_dark")
        b.oval("end", 196, 30, 22, 26.5, "wood_light")
        b.oval("ring", 196, 30, 12, 15, "wood", 2)
        b.oval("end_back", 26, 30, 20, 26.5, "bark")
        b.seat(110, 6)
        b.ground()

    @prop("prop.nature.stump_v1", "Kanto", 110, 70, ["kanto", "stump", "puu", "tree", "metsä", "forest", "istuin", "seat", "outdoor"])
    def _(b):
        b.box("trunk", 14, 14, 82, b.F - 14, "bark")
        b.line("root_left", [(16, 60), (4, 66)], 8, "bark")
        b.line("root_right", [(94, 60), (106, 66)], 8, "bark")
        b.oval("top", 55, 14, 41, 10, "wood_light")
        b.oval("ring", 55, 14, 22, 5, "wood", 2)
        b.seat(55, 12)
        b.ground()

    @prop("prop.nature.boulder_v1", "Kivi", 180, 110, ["kivi", "rock", "lohkare", "boulder", "metsä", "forest", "ranta", "shore", "outdoor"])
    def _(b):
        b.oval("rock_back", 110, 64, 64, 42.5, "rock_dark")
        b.oval("rock_front", 70, 72, 62, 34.5, "rock")
        b.oval("moss", 60, 46, 26, 7, "leaf", 0)
        b.seat(80, 40)
        b.ground()

    @prop("prop.nature.mushroom_v1", "Kärpässieni", 50, 60, ["sieni", "mushroom", "metsä", "forest", "syksy", "autumn", "outdoor"])
    def _(b):
        b.box("stem", 18, 26, 14, b.F - 26, "cream")
        b.oval("cap", 25, 22, 22, 15, "mushroom")
        for i, (x, y) in enumerate(((16, 16), (28, 12), (34, 22), (20, 26))):
            b.oval(f"spot_{i}", x, y, 3, 2.5, "white", 0)
        b.ground()

    @prop("prop.nature.flower_v1", "Kukka", 40, 90, ["kukka", "flower", "kesä", "summer", "niitty", "meadow", "outdoor", "käsiesine", "hand_prop"])
    def _(b):
        b.line("stem", [(20, 26), (20, 88)], 3, "leaf_dark")
        b.oval("leaf", 28, 62, 9, 4, "leaf", 0)
        for i in range(6):
            a = 2 * math.pi * i / 6
            b.oval(f"petal_{i}", 20 + 9 * math.cos(a), 18 + 9 * math.sin(a), 6, 6, "petal_white", 0)
        b.oval("centre", 20, 18, 5, 5, "accent", 2)
        b.grip(20, 70)
        b.ground()

    @prop("prop.nature.fishing_rod_v1", "Onki", 160, 240, ["onki", "fishing_rod", "kalastus", "fishing", "järvi", "lake", "kesä", "summer", "käsiesine", "hand_prop"])
    def _(b):
        b.line("rod", [(14, 230), (150, 10)], 4, "wood_dark")
        b.line("handle", [(14, 230), (34, 198)], 7, "screen")
        b.oval("reel", 36, 210, 7, 7, "metal", 2)
        b.line("line", [(150, 10), (154, 120)], 1, "line")
        b.oval("float", 154, 124, 4, 6, "accent_red", 0)
        b.grip(24, 214)

    @prop("prop.nature.fish_v1", "Kala", 80, 34, ["kala", "fish", "kalastus", "fishing", "järvi", "lake", "ruoka", "food", "käsiesine", "hand_prop"])
    def _(b):
        b.line("tail_top", [(16, 17), (4, 6)], 6, "feather")
        b.line("tail_bottom", [(16, 17), (4, 28)], 6, "feather")
        b.oval("body", 44, 17, 30, 12, "feather")
        b.oval("belly", 46, 21, 22, 5, "white", 0)
        b.oval("eye", 64, 14, 2.5, 2.5, "screen", 0)
        b.grip(44, 17)

    # ------------------------------------------------------------ containers and lighting
    @prop("prop.container.cardboard_box_v1", "Pahvilaatikko", 110, 90, ["laatikko", "box", "pahvi", "cardboard", "muutto", "moving", "varasto", "storage"])
    def _(b):
        b.box("box", 6, 14, 98, b.F - 14, "cardboard")
        b.box("flap_left", 6, 4, 46, 12, "cardboard", 2)
        b.box("flap_right", 58, 4, 46, 12, "cardboard", 2)
        b.box("tape", 48, 14, 14, 30, "wood_light", 0)
        b.stand(55, 14, "surface")
        b.ground()

    @prop("prop.container.wooden_crate_v1", "Puulaatikko", 120, 100, ["laatikko", "crate", "puu", "wood", "satama", "harbour", "varasto", "storage"])
    def _(b):
        b.box("crate", 6, 6, 108, b.F - 6, "wood_light")
        for i in range(1, 4):
            b.line(f"plank_{i}", [(6, 6 + i * 23), (114, 6 + i * 23)], 2, "wood_dark")
        b.line("brace", [(10, 94), (110, 10)], 6, "wood")
        b.seat(60, 6)
        b.ground()

    @prop("prop.container.barrel_v1", "Tynnyri", 100, 130, ["tynnyri", "barrel", "puu", "wood", "satama", "harbour", "varasto", "storage"])
    def _(b):
        b.oval("barrel", 50, 66, 44, 61.5, "wood")
        for i in range(1, 4):
            b.line(f"stave_{i}", [(10 + i * 20, 10), (10 + i * 20, 122)], 2, "wood_dark")
        b.line("hoop_top", [(10, 30), (90, 30)], 5, "metal_dark")
        b.line("hoop_bottom", [(10, 102), (90, 102)], 5, "metal_dark")
        b.ground()

    @prop("prop.lighting.candle_v1", "Kynttilä", 30, 70, ["kynttilä", "candle", "valo", "light", "ilta", "evening", "joulu", "christmas"])
    def _(b):
        b.oval("glow", 15, 12, 8, 10, "light", 0)
        b.oval("flame", 15, 12, 3.5, 7, "flame", 0)
        b.box("candle", 8, 22, 14, 36, "wax", 2)
        b.box("holder", 3, 58, 24, b.F - 58, "brass")
        b.grip(15, 62)
        b.ground()

    @prop("prop.lighting.flashlight_v1", "Taskulamppu", 70, 26, ["taskulamppu", "flashlight", "valo", "light", "pimeä", "dark", "käsiesine", "hand_prop"])
    def _(b):
        b.box("body", 4, 8, 44, 12, "screen")
        b.box("head", 46, 3, 18, 20, "screen", 2)
        b.box("lens", 62, 6, 4, 14, "light", 0)
        b.box("button", 20, 4, 8, 4, "accent_red", 0)
        b.grip(26, 14)

    @prop("prop.lighting.chandelier_v1", "Kattokruunu", 200, 160, ["kattokruunu", "chandelier", "valo", "light", "juhlasali", "hall", "decor"])
    def _(b):
        b.line("chain", [(100, 2), (100, 56)], 3, "brass")
        b.oval("hub", 100, 66, 16, 12, "brass", 2)
        for i, x in enumerate((20, 60, 140, 180)):
            b.line(f"arm_{i}", [(100, 70), (x, 98), (x, 112)], 4, "brass")
            b.box(f"candle_{i}", x - 5, 92, 10, 22, "wax", 2)
            b.oval(f"flame_{i}", x, 84, 4, 7, "flame", 0)
        b.oval("drop", 100, 130, 10, 16, "glass_clear", 2)

    # ------------------------------------------------------------ animals (static poses)
    @prop("prop.animal.dog_sitting_v1", "Koira", 140, 150, ["koira", "dog", "lemmikki", "pet", "eläin", "animal"])
    def _(b):
        b.line("tail", [(30, 124), (8, 104)], 8, "fur")
        b.oval("body", 60, 110, 38, 36.5, "fur")
        b.line("front_leg", [(84, 110), (90, 144)], 8, "fur")
        b.oval("paw", 92, 144, 10, 4.5, "fur_dark")
        b.oval("head", 96, 48, 30, 26, "fur")
        b.oval("ear", 78, 50, 10, 20, "fur_dark", 2)
        b.oval("snout", 120, 58, 14, 10, "cream", 2)
        b.oval("nose", 130, 52, 5, 4, "screen", 0)
        b.oval("eye", 104, 40, 3.5, 3.5, "screen", 0)
        b.line("collar", [(78, 72), (110, 78)], 6, "accent_red")
        b.ground()

    @prop("prop.animal.cat_sitting_v1", "Kissa", 100, 130, ["kissa", "cat", "lemmikki", "pet", "eläin", "animal"])
    def _(b):
        b.line("tail", [(70, 124), (92, 110), (94, 80)], 6, "fur_grey")
        b.oval("body", 50, 98, 30, 28.5, "fur_grey")
        b.oval("head", 50, 42, 24, 21, "fur_grey")
        b.line("ear_left", [(32, 30), (30, 10), (44, 24)], 5, "fur_grey")
        b.line("ear_right", [(68, 30), (70, 10), (56, 24)], 5, "fur_grey")
        b.oval("eye_left", 41, 40, 4, 5, "fruit_green", 0)
        b.oval("eye_right", 59, 40, 4, 5, "fruit_green", 0)
        b.oval("nose", 50, 50, 3, 2, "petal_pink", 0)
        b.line("whisker_left", [(44, 52), (24, 50)], 1, "line")
        b.line("whisker_right", [(56, 52), (76, 50)], 1, "line")
        b.ground()

    @prop("prop.animal.bird_v1", "Lintu", 60, 50, ["lintu", "bird", "tiainen", "tit", "eläin", "animal", "outdoor"])
    def _(b):
        b.line("tail", [(14, 26), (4, 34)], 6, "feather")
        b.oval("body", 28, 28, 18, 13, "feather")
        b.oval("belly", 32, 32, 11, 7, "fruit_yellow", 0)
        b.oval("head", 42, 16, 10, 9, "screen")
        b.oval("cheek", 44, 17, 5, 4, "white", 0)
        b.line("beak", [(51, 16), (57, 17)], 3, "accent")
        b.line("leg", [(30, 40), (30, 47)], 2, "line")
        b.ground()

    @prop("prop.animal.fishbowl_v1", "Akvaario", 100, 100, ["akvaario", "fishbowl", "kultakala", "goldfish", "lemmikki", "pet", "eläin", "animal"])
    def _(b):
        b.oval("water", 50, 56, 44, 40.5, "water")
        b.oval("fish", 44, 56, 12, 7, "accent", 0)
        b.line("fish_tail", [(32, 56), (26, 50), (26, 62), (32, 56)], 3, "accent")
        b.oval("plant", 66, 84, 8, 10, "leaf", 0)
        b.box("rim", 24, 12, 52, 8, "glass", 2)
        b.ground()

    # ------------------------------------------------------------ vehicles and buildings
    @prop("prop.vehicle.tractor_v1", "Traktori", 340, 240, ["traktori", "tractor", "maatila", "farm", "ajoneuvo", "vehicle", "maaseutu", "countryside", "outdoor"])
    def _(b):
        b.box("cab_roof", 30, 6, 120, 12, "tractor")
        b.line("cab_post_a", [(40, 18), (40, 110)], 6, "metal_dark")
        b.line("cab_post_b", [(140, 18), (140, 110)], 6, "metal_dark")
        b.box("cab_glass", 44, 22, 92, 80, "glass_clear", 2)
        b.box("hood", 140, 82, 170, 70, "tractor")
        b.line("exhaust", [(280, 82), (280, 30)], 7, "metal_dark")
        b.box("body", 30, 110, 280, 50, "tractor")
        b.oval("rear_wheel", 90, 174, 64, 64.5, "screen")
        b.oval("rear_hub", 90, 174, 24, 24, "fruit_yellow", 2)
        b.oval("front_wheel", 270, 202, 36, 36.5, "screen")
        b.oval("front_hub", 270, 202, 13, 13, "fruit_yellow", 2)
        b.seat(90, 96)
        b.ground()

    @prop("prop.vehicle.motorcycle_v1", "Moottoripyörä", 300, 170, ["moottoripyörä", "motorcycle", "ajoneuvo", "vehicle", "katu", "street", "outdoor"])
    def _(b):
        b.oval("rear_wheel", 60, 130, 38, 38.5, "screen")
        b.oval("rear_hub", 60, 130, 12, 12, "metal", 2)
        b.oval("front_wheel", 242, 130, 38, 38.5, "screen")
        b.oval("front_hub", 242, 130, 12, 12, "metal", 2)
        b.line("fork", [(242, 130), (214, 40)], 6, "metal")
        b.line("handlebar", [(200, 36), (228, 30)], 6, "screen")
        b.line("frame", [(60, 130), (120, 90), (210, 70)], 7, "metal_dark")
        b.oval("tank", 160, 72, 40, 18, "accent_red")
        b.box("seat_pad", 76, 66, 70, 14, "screen")
        b.box("engine", 110, 92, 60, 40, "metal", 2)
        b.seat(110, 66)
        b.ground()

    @prop("prop.vehicle.rowboat_v1", "Soutuvene", 360, 90, ["vene", "boat", "soutuvene", "rowboat", "järvi", "lake", "kesä", "summer", "outdoor"])
    def _(b):
        b.line("oar", [(120, 30), (60, 86)], 5, "wood_dark")
        for i in range(9):
            t = (i + 0.5) / 9
            y = 32 + t * 52
            b.line(f"hull_{i}", [(14 + 50 * t ** 1.5, y), (346 - 70 * t ** 1.5, y)], 8, "boat")
        b.line("hull_seam", [(24, 52), (330, 52)], 2, "wood_dark")
        b.box("gunwale", 10, 24, 340, 10, "wood_dark")
        b.box("thwart", 160, 28, 40, 8, "wood_light", 2)
        b.seat(180, 28)
        b.ground()

    @prop("prop.vehicle.sailboat_v1", "Purjevene", 300, 360, ["purjevene", "sailboat", "vene", "boat", "meri", "sea", "kesä", "summer", "outdoor"])
    def _(b):
        b.line("mast", [(150, 10), (150, 290)], 6, "wood_dark")
        for i in range(36):
            t = (i + 0.5) / 36
            y = 20 + t * 250
            b.line(f"sail_{i}", [(156, y), (156 + 120 * t, y)], 8, "sail")
        for i in range(28):
            t = (i + 0.5) / 28
            y = 70 + t * 200
            b.line(f"jib_{i}", [(144, y), (144 - 80 * t, y)], 8, "sail")
        b.line("flag", [(150, 12), (176, 18), (150, 24)], 3, "accent_red")
        b.box("hull", 30, 290, 240, b.F - 300, "boat")
        b.line("keel_line", [(30, 300), (270, 300)], 4, "wood_dark")
        b.box("hull_bottom", 60, 330, 180, b.F - 330, "boat")
        b.ground()

    @prop("prop.building.barn_v1", "Lato", 520, 420, ["lato", "barn", "maatila", "farm", "rakennus", "building", "maaseutu", "countryside", "outdoor"])
    def _(b):
        for i in range(20):
            t = (i + 0.5) / 20
            y = 16 + t * 130
            b.line(f"gable_{i}", [(260 - 250 * t + 4, y), (260 + 250 * t - 4, y)], 8, "barn_red")
        b.box("wall", 14, 150, 492, b.F - 150, "barn_red")
        b.line("roof_left", [(260, 8), (4, 154)], 8, "roof")
        b.line("roof_right", [(260, 8), (516, 154)], 8, "roof")
        b.box("door", 180, 230, 160, 186, "wood_dark")
        b.line("door_x1", [(180, 230), (340, 416)], 6, "white")
        b.line("door_x2", [(340, 230), (180, 416)], 6, "white")
        b.box("loft", 230, 80, 60, 50, "wood_dark", 3)
        b.doorway(184, 234, 152, 180, "door")
        b.ground()

    @prop("prop.building.summer_cottage_v1", "Kesämökki", 480, 330, ["mökki", "cottage", "kesämökki", "summer_cottage", "järvi", "lake", "rakennus", "building", "kesä", "summer", "outdoor"])
    def _(b):
        for i in range(16):
            t = (i + 0.5) / 16
            y = 12 + t * 110
            b.line(f"gable_{i}", [(240 - 230 * t + 4, y), (240 + 230 * t - 4, y)], 8, "barn_red")
        b.box("wall", 24, 120, 432, b.F - 120, "barn_red")
        b.line("roof_left", [(240, 6), (4, 126)], 8, "roof")
        b.line("roof_right", [(240, 6), (476, 126)], 8, "roof")
        b.box("door", 300, 180, 70, 146, "wood_light")
        b.oval("door_knob", 360, 260, 4, 4, "metal_dark", 2)
        b.box("window", 80, 170, 100, 80, "glass")
        b.line("window_cross_v", [(130, 170), (130, 250)], 4, "white")
        b.line("window_cross_h", [(80, 210), (180, 210)], 4, "white")
        b.box("window_trim", 74, 250, 112, 8, "white", 0)
        b.box("step", 290, 316, 90, b.F - 316, "stone")
        b.doorway(300, 180, 70, 146, "door")
        b.ground()

    @prop("prop.building.lighthouse_v1", "Majakka", 200, 560, ["majakka", "lighthouse", "meri", "sea", "ranta", "shore", "rakennus", "building", "outdoor"])
    def _(b):
        for i in range(56):
            t = (i + 0.5) / 56
            y = 130 + t * 400
            half = 40 + 40 * t
            slot = "accent_red" if (i // 7) % 2 else "lighthouse"
            b.line(f"tower_{i}", [(100 - half + 4, y), (100 + half - 4, y)], 8, slot)
        b.box("lantern_room", 60, 70, 80, 54, "glass")
        b.oval("light", 100, 97, 20, 20, "light", 0)
        b.box("gallery", 44, 120, 112, 12, "screen")
        b.line("roof_left", [(100, 12), (54, 70)], 8, "accent_red")
        b.line("roof_right", [(100, 12), (146, 70)], 8, "accent_red")
        b.box("roof_fill", 70, 40, 60, 30, "accent_red", 0)
        b.box("base", 10, 530, 180, b.F - 530, "stone")
        b.box("door", 84, 470, 32, 60, "wood_dark")
        b.ground()
