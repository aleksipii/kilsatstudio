"""Original, editable vector scenery for the Kilsat stick-figure cast.

Backgrounds use a 1920 x 1080 canvas and a floor at y=960. Props use their
declared local bounds. Consumers deep-copy layers and replace TEMPLATE scene IDs
and layer IDs when placing them. There are no fonts, linked images or downloads.
"""

from math import cos, pi, sin


INK = (43, 49, 48, 255)
PAPER = (241, 236, 223, 255)
TEAL = (65, 112, 111, 255)
TEAL_LIGHT = (157, 194, 185, 255)
RUST = (170, 90, 61, 255)
AMBER = (221, 175, 91, 255)
METAL = (185, 199, 197, 255)
WOOD = (161, 125, 86, 255)
NIGHT = (131, 153, 164, 255)


def _layer(name, content, plane="foreground"):
    return {
        "id": name,
        "name": name.replace("_", " "),
        "scene_id": "TEMPLATE",
        "visible": True,
        "plane": plane,
        "opacity": 1.0,
        "attachment": None,
        "clip": None,
        "content": content,
    }


def _color(value):
    return dict(zip(("r", "g", "b", "a"), value))


def _rect(name, x, y, width, height, color, plane="foreground"):
    return _layer(name, {
        "type": "rectangle",
        "rect": dict(x=x, y=y, width=width, height=height),
        "color": _color(color),
    }, plane)


def _ellipse(name, x, y, width, height, color, plane="foreground"):
    return _layer(name, {
        "type": "ellipse",
        "rect": dict(x=x, y=y, width=width, height=height),
        "color": _color(color),
    }, plane)


def _stroke(name, points, width, color=INK, plane="foreground"):
    return _layer(name, {
        "type": "stroke",
        "points": [dict(x=x, y=y) for x, y in points],
        "width": width,
        "color": _color(color),
    }, plane)


def _panel(name, x, y, width, height, color, border=7, plane="foreground"):
    return [
        _rect(name + "_outline", x, y, width, height, INK, plane),
        _rect(name + "_fill", x + border, y + border,
              width - 2 * border, height - 2 * border, color, plane),
    ]


def _disc(name, x, y, width, height, color, border=7, plane="foreground"):
    return [
        _ellipse(name + "_outline", x, y, width, height, INK, plane),
        _ellipse(name + "_fill", x + border, y + border,
                 width - 2 * border, height - 2 * border, color, plane),
    ]


def _clip(layer, x, y, width, height):
    layer["clip"] = dict(x=x, y=y, width=width, height=height)
    return layer


def _backgrounds():
    bg = "background"
    office = [
        _rect("toimisto_wall", 0, 0, 1920, 1080, PAPER, bg),
        _rect("toimisto_floor", 0, 810, 1920, 270, (203, 190, 165, 255), bg),
        _rect("toimisto_skirting", 0, 798, 1920, 12, (103, 131, 120, 255), bg),
        *_panel("toimisto_window", 70, 180, 330, 400, TEAL_LIGHT, 12, bg),
        _rect("toimisto_skyline_far", 92, 414, 130, 140, (124, 159, 154, 255), bg),
        _rect("toimisto_skyline_near", 251, 365, 124, 189, (99, 139, 136, 255), bg),
        _rect("toimisto_mullion_v", 229, 192, 12, 376, INK, bg),
        _rect("toimisto_mullion_h", 82, 370, 306, 12, INK, bg),
        *_disc("toimisto_clock", 1632, 160, 170, 170, PAPER, 9, bg),
        _stroke("toimisto_clock_hour", [(1717, 245), (1680, 226)], 8, INK, bg),
        _stroke("toimisto_clock_minute", [(1717, 245), (1717, 187)], 6, INK, bg),
        _ellipse("toimisto_clock_pin", 1710, 238, 14, 14, RUST, bg),
        *_panel("toimisto_door", 1660, 490, 180, 458, (130, 155, 145, 255), 10, bg),
        _rect("toimisto_door_glass", 1692, 529, 116, 158, (191, 209, 189, 255), bg),
        _stroke("toimisto_door_handle", [(1684, 740), (1714, 740)], 9, INK, bg),
        _rect("toimisto_side_desk", 70, 755, 325, 28, INK, bg),
        _rect("toimisto_side_desk_front", 85, 783, 295, 49, WOOD, bg),
        _rect("toimisto_desk_leg_left", 94, 832, 17, 116, INK, bg),
        _rect("toimisto_desk_leg_right", 353, 832, 17, 116, INK, bg),
    ]
    bus_stop = [
        _rect("pysakki_night", 0, 0, 1920, 1080, NIGHT, bg),
        _rect("pysakki_pavement", 0, 810, 1920, 270, (150, 167, 167, 255), bg),
        _rect("pysakki_curb", 0, 1020, 1920, 10, (94, 117, 120, 255), bg),
        _rect("pysakki_road_mark", 870, 1037, 490, 12, (203, 194, 158, 255), bg),
        _rect("pysakki_distant_roof", 0, 402, 370, 72, (35, 57, 63, 255), bg),
        _rect("pysakki_distant_house", 36, 474, 280, 260, (35, 57, 63, 255), bg),
        _rect("pysakki_distant_window", 92, 517, 49, 61, AMBER, bg),
        _rect("pysakki_distant_window_two", 207, 517, 49, 61, (146, 137, 92, 255), bg),
        _ellipse("pysakki_lamp_glow", 47, 111, 347, 480, (238, 211, 134, 24), bg),
        _stroke("pysakki_lamp_post", [(185, 942), (185, 141), (303, 141)], 15, INK, bg),
        _rect("pysakki_lamp_shade", 266, 130, 90, 23, INK, bg),
        _ellipse("pysakki_lamp_light", 274, 149, 74, 17, AMBER, bg),
        _rect("pysakki_bench_back", 59, 723, 324, 22, WOOD, bg),
        _rect("pysakki_bench_back_lower", 59, 765, 324, 22, WOOD, bg),
        _rect("pysakki_bench_seat", 48, 826, 346, 22, INK, bg),
        _stroke("pysakki_bench_legs", [(86, 938), (105, 848), (340, 848), (359, 938)], 13, INK, bg),
        _stroke("pysakki_sign_post", [(1736, 938), (1736, 387)], 13, METAL, bg),
        *_panel("pysakki_sign", 1656, 278, 160, 184, TEAL_LIGHT, 9, bg),
        _rect("pysakki_bus_icon", 1686, 309, 100, 91, INK, bg),
        _rect("pysakki_bus_windshield", 1695, 322, 82, 37, PAPER, bg),
        _ellipse("pysakki_bus_wheel_left", 1697, 389, 19, 27, INK, bg),
        _ellipse("pysakki_bus_wheel_right", 1757, 389, 19, 27, INK, bg),
        _ellipse("pysakki_star_left", 731, 177, 7, 7, PAPER, bg),
        _ellipse("pysakki_star_right", 1315, 116, 9, 9, PAPER, bg),
    ]
    graveyard = [
        _rect("hautausmaa_sky", 0, 0, 1920, 1080, (135, 155, 162, 255), bg),
        _rect("hautausmaa_ground", 0, 810, 1920, 270, (155, 170, 150, 255), bg),
        _stroke("hautausmaa_horizon", [(0, 814), (490, 814), (770, 806), (1210, 814), (1914, 814)], 8, TEAL, bg),
        _ellipse("hautausmaa_moon_halo", 1425, 86, 282, 282, (215, 224, 188, 16), bg),
        _ellipse("hautausmaa_moon", 1470, 131, 192, 192, (218, 224, 197, 255), bg),
        _ellipse("hautausmaa_moon_crater", 1508, 170, 37, 37, (197, 208, 182, 255), bg),
        _ellipse("hautausmaa_moon_crater_small", 1572, 242, 21, 21, (197, 208, 182, 255), bg),
        _stroke("hautausmaa_tree_trunk", [(1739, 941), (1714, 720), (1730, 481), (1692, 344)], 28, INK, bg),
        _stroke("hautausmaa_tree_left", [(1720, 677), (1643, 596), (1585, 492), (1516, 454)], 18, INK, bg),
        _stroke("hautausmaa_tree_left_twig", [(1643, 596), (1599, 635), (1554, 640)], 9, INK, bg),
        _stroke("hautausmaa_tree_right", [(1727, 533), (1814, 460), (1848, 361)], 16, INK, bg),
        _stroke("hautausmaa_tree_right_twig", [(1814, 460), (1880, 448)], 8, INK, bg),
        _stroke("hautausmaa_tree_top_twig", [(1692, 344), (1712, 289), (1750, 264)], 9, INK, bg),
        *_disc("hautausmaa_stone_round", 76, 668, 145, 163, METAL, 8, bg),
        *_panel("hautausmaa_stone_base", 76, 744, 145, 205, METAL, 8, bg),
        _stroke("hautausmaa_stone_engraving", [(116, 834), (180, 834)], 6, TEAL, bg),
        *_panel("hautausmaa_stone_square", 268, 781, 111, 168, (144, 165, 160, 255), 8, bg),
        _stroke("hautausmaa_stone_small_engraving", [(298, 835), (349, 835)], 5, TEAL, bg),
        _stroke("hautausmaa_grass_left", [(37, 946), (24, 909), (49, 929), (63, 904)], 7, TEAL, bg),
        _stroke("hautausmaa_grass_right", [(1815, 947), (1804, 909), (1828, 931), (1847, 902)], 7, TEAL, bg),
        _ellipse("hautausmaa_star", 633, 186, 8, 8, PAPER, bg),
    ]
    cafe = [
        _rect("kahvila_wall", 0, 0, 1920, 1080, (237, 222, 201, 255), bg),
        _rect("kahvila_floor", 0, 810, 1920, 270, (201, 170, 133, 255), bg),
        _rect("kahvila_skirting", 0, 798, 1920, 12, (161, 105, 81, 255), bg),
        *_panel("kahvila_counter", 42, 686, 332, 262, RUST, 9, bg),
        _rect("kahvila_counter_panel", 72, 733, 271, 176, (149, 76, 55, 255), bg),
        _rect("kahvila_counter_top", 30, 663, 356, 30, INK, bg),
        *_panel("kahvila_machine", 86, 486, 230, 177, METAL, 8, bg),
        _rect("kahvila_machine_front", 108, 524, 185, 67, INK, bg),
        _ellipse("kahvila_machine_dial", 254, 504, 17, 17, RUST, bg),
        _stroke("kahvila_machine_spout", [(172, 579), (172, 613), (204, 613)], 10, METAL, bg),
        _rect("kahvila_machine_cup", 179, 625, 47, 26, PAPER, bg),
        *_panel("kahvila_window", 1555, 227, 286, 391, TEAL_LIGHT, 11, bg),
        _rect("kahvila_window_mullion", 1691, 238, 12, 369, INK, bg),
        _rect("kahvila_window_sill", 1539, 618, 316, 17, INK, bg),
        _stroke("kahvila_pendant_left_cable", [(555, 0), (555, 178)], 6, INK, bg),
        _ellipse("kahvila_pendant_left_shade", 497, 167, 116, 55, RUST, bg),
        _ellipse("kahvila_pendant_left_bulb", 529, 213, 52, 13, AMBER, bg),
        _stroke("kahvila_pendant_right_cable", [(1198, 0), (1198, 178)], 6, INK, bg),
        _ellipse("kahvila_pendant_right_shade", 1140, 167, 116, 55, RUST, bg),
        _ellipse("kahvila_pendant_right_bulb", 1172, 213, 52, 13, AMBER, bg),
        _rect("kahvila_window_street", 1566, 512, 264, 95, (127, 165, 154, 255), bg),
        _stroke("kahvila_window_reflection", [(1755, 270), (1797, 312), (1797, 348)], 7, (211, 225, 203, 255), bg),
    ]
    # Receding seams and edge furniture establish a floor plane without filling
    # the acting area. Foot contact remains y=960, inside the floor, not its edge.
    for prefix, layers, tone in (
        ("toimisto", office, (183, 173, 151, 255)),
        ("kahvila", cafe, (181, 150, 118, 255)),
    ):
        floor_index = next(i for i, layer in enumerate(layers)
                           if layer["id"] == prefix + "_skirting")
        seams = [
            _stroke(prefix + "_floor_seam_" + str(i),
                    [(840 + i * 60, 810), (-260 + i * 480, 1080)], 2, tone, bg)
            for i in range(5)
        ]
        seams.extend([
            _stroke(prefix + "_floor_cross_0", [(0, 866), (1920, 866)], 2, tone, bg),
            _stroke(prefix + "_floor_cross_1", [(0, 957), (1920, 957)], 2, tone, bg),
        ])
        layers[floor_index:floor_index] = seams
    office.extend([
        *_panel("toimisto_pinboard", 755, 225, 345, 180, (201, 177, 137, 255), 8, bg),
        _rect("toimisto_pinboard_note_one", 780, 252, 88, 111, PAPER, bg),
        _rect("toimisto_pinboard_note_two", 899, 265, 144, 78, (219, 226, 198, 255), bg),
        _ellipse("toimisto_pinboard_pin", 817, 255, 9, 9, RUST, bg),
        _ellipse("toimisto_pinboard_pin_two", 962, 270, 9, 9, RUST, bg),
        _stroke("toimisto_note_rule", [(793, 286), (854, 286)], 3, (130, 146, 134, 255), bg),
        _stroke("toimisto_note_rule_two", [(793, 305), (840, 305)], 3, (130, 146, 134, 255), bg),
        _rect("toimisto_desk_books_0", 112, 727, 100, 15, RUST, bg),
        _rect("toimisto_desk_books_1", 106, 742, 116, 13, PAPER, bg),
        _stroke("toimisto_plant_stem", [(310, 723), (303, 650)], 5, TEAL, bg),
        _ellipse("toimisto_plant_leaf_left", 260, 658, 48, 19, (100, 143, 122, 255), bg),
        _ellipse("toimisto_plant_leaf_right", 304, 630, 43, 19, (100, 143, 122, 255), bg),
        _rect("toimisto_plant_pot", 280, 710, 55, 43, RUST, bg),
    ])
    bus_stop.extend([
        _stroke("pysakki_rear_curb", [(0, 811), (1920, 811)], 4, (114, 138, 141, 255), bg),
        _stroke("pysakki_paving_seam_0", [(420, 812), (100, 1018)], 2, (129, 149, 151, 255), bg),
        _stroke("pysakki_paving_seam_1", [(1420, 812), (1840, 1018)], 2, (129, 149, 151, 255), bg),
        _stroke("pysakki_paving_cross", [(0, 900), (1920, 900)], 2, (129, 149, 151, 255), bg),
        _rect("pysakki_road", 0, 1030, 1920, 50, (104, 126, 132, 255), bg),
        _rect("pysakki_road_dash_0", 150, 1057, 220, 7, (218, 220, 198, 255), bg),
        _rect("pysakki_road_dash_1", 780, 1057, 220, 7, (218, 220, 198, 255), bg),
        _rect("pysakki_road_dash_2", 1410, 1057, 220, 7, (218, 220, 198, 255), bg),
        *_panel("pysakki_timetable", 1662, 530, 150, 170, (226, 228, 211, 255), 5, bg),
        _stroke("pysakki_timetable_rule_0", [(1680, 564), (1789, 564)], 4, TEAL, bg),
        _stroke("pysakki_timetable_rule_1", [(1680, 594), (1773, 594)], 3, TEAL, bg),
        _stroke("pysakki_timetable_rule_2", [(1680, 621), (1789, 621)], 3, TEAL, bg),
        _stroke("pysakki_timetable_rule_3", [(1680, 648), (1773, 648)], 3, TEAL, bg),
    ])
    # A low distant fence adds depth; its quiet colors stay behind dark actors.
    fence = [_stroke("hautausmaa_fence_rail", [(390, 743), (1450, 743)], 4,
                     (112, 139, 139, 255), bg)]
    fence.extend(_stroke("hautausmaa_fence_post_" + str(i),
                         [(430 + i * 95, 715), (430 + i * 95, 809)], 4,
                         (112, 139, 139, 255), bg) for i in range(11))
    graveyard[4:4] = fence
    graveyard.extend([
        _stroke("hautausmaa_path_left", [(905, 814), (520, 1080)], 3,
                (136, 155, 137, 255), bg),
        _stroke("hautausmaa_path_right", [(1000, 814), (1450, 1080)], 3,
                (136, 155, 137, 255), bg),
        _ellipse("hautausmaa_moss_left", 86, 930, 128, 17, (104, 134, 117, 255), bg),
        _ellipse("hautausmaa_moss_right", 1590, 948, 195, 13, (125, 148, 127, 255), bg),
    ])
    cafe.extend([
        _rect("kahvila_shelf", 96, 389, 235, 12, WOOD, bg),
        _rect("kahvila_cup_shelf_0", 119, 356, 30, 31, PAPER, bg),
        _rect("kahvila_cup_shelf_1", 175, 356, 30, 31, PAPER, bg),
        _rect("kahvila_cup_shelf_2", 231, 356, 30, 31, PAPER, bg),
        _rect("kahvila_table_leg", 1715, 789, 13, 160, INK, bg),
        _stroke("kahvila_table_feet", [(1650, 949), (1721, 923), (1791, 949)], 9, INK, bg),
        _ellipse("kahvila_table_top_outline", 1575, 744, 284, 61, INK, bg),
        _ellipse("kahvila_table_top", 1583, 749, 268, 45, WOOD, bg),
        _rect("kahvila_table_cup", 1674, 721, 33, 32, PAPER, bg),
        _ellipse("kahvila_table_coffee", 1674, 716, 33, 10, (111, 76, 55, 255), bg),
        _stroke("kahvila_table_cup_handle", [(1707, 727), (1719, 727), (1719, 742), (1707, 742)], 4, PAPER, bg),
        _stroke("kahvila_chair_back", [(1854, 708), (1883, 708), (1883, 840)], 9, INK, bg),
        _stroke("kahvila_chair_base", [(1816, 946), (1828, 842), (1882, 842), (1896, 946)], 9, INK, bg),
        _rect("kahvila_chair_seat", 1807, 829, 84, 16, RUST, bg),
    ])
    return {
        "toimisto": dict(name="Loputtoman palaverin toimisto", color="#f1ecdf", layers=office),
        "pysakki": dict(name="Yövuoron pysäkki", color="#8399a4", layers=bus_stop),
        "hautausmaa": dict(name="Kuunsirpin hautausmaa", color="#879ba2", layers=graveyard),
        "kahvila": dict(name="Viimeisen kahvin kahvila", color="#eddec9", layers=cafe),
    }


def _props():
    def prop(name, width, height, layers):
        return dict(name=name, width=width, height=height, layers=layers)

    return {
        "viikate": prop("Noutajan viikate", 310, 640, [
            _stroke("viikate_staff_outline", [(80, 610), (80, 82)], 20),
            _stroke("viikate_staff_wood", [(80, 608), (80, 83)], 11, WOOD),
            _stroke("viikate_blade_outline", [(81, 106), (108, 68), (162, 42), (219, 42), (272, 65)], 31),
            _stroke("viikate_blade_metal", [(81, 106), (108, 68), (162, 42), (219, 42), (272, 65)], 20, METAL),
            _stroke("viikate_blade_tip", [(260, 61), (290, 89), (277, 66)], 7, METAL),
            _rect("viikate_collar", 66, 112, 28, 22, INK),
            _rect("viikate_collar_band", 69, 116, 22, 6, AMBER),
        ]),
        "kahvikuppi": prop("Viimeinen kahvikuppi", 230, 220, [
            *_disc("kahvikuppi_handle", 143, 76, 70, 82, PAPER, 10),
            _ellipse("kahvikuppi_base_outline", 30, 143, 143, 55, INK),
            _ellipse("kahvikuppi_base_fill", 38, 142, 127, 47, PAPER),
            *_panel("kahvikuppi_body", 30, 76, 143, 100, PAPER, 8),
            _ellipse("kahvikuppi_rim_outline", 30, 58, 143, 46, INK),
            _ellipse("kahvikuppi_coffee", 39, 67, 125, 28, (111, 76, 55, 255)),
            _stroke("kahvikuppi_steam_left", [(72, 44), (63, 28), (71, 10)], 6, TEAL),
            _stroke("kahvikuppi_steam_right", [(117, 42), (127, 26), (118, 10)], 6, TEAL),
            _stroke("kahvikuppi_glaze", [(52, 115), (52, 151)], 7, AMBER),
        ]),
        "toimistotuoli": prop("Palaverituoli", 250, 350, [
            *_panel("toimistotuoli_back", 52, 20, 146, 160, TEAL, 9),
            _rect("toimistotuoli_back_inset", 72, 39, 106, 119, (81, 133, 127, 255)),
            _stroke("toimistotuoli_support", [(125, 173), (125, 305)], 15),
            *_panel("toimistotuoli_seat", 25, 194, 200, 39, TEAL, 8),
            _stroke("toimistotuoli_arm_left", [(32, 197), (32, 164), (60, 164)], 11),
            _stroke("toimistotuoli_arm_right", [(218, 197), (218, 164), (190, 164)], 11),
            _stroke("toimistotuoli_base", [(41, 318), (125, 297), (208, 318)], 12),
            _ellipse("toimistotuoli_wheel_left", 26, 316, 30, 22, INK),
            _ellipse("toimistotuoli_wheel_right", 193, 316, 30, 22, INK),
        ]),
        "tyopoyta": prop("Työpöytä", 520, 340, [
            _rect("tyopoyta_leg_left", 43, 66, 23, 258, INK),
            _rect("tyopoyta_leg_right", 455, 66, 23, 258, INK),
            _rect("tyopoyta_foot_left", 27, 296, 55, 17, INK),
            _rect("tyopoyta_foot_right", 439, 296, 55, 17, INK),
            *_panel("tyopoyta_drawer", 321, 72, 134, 117, WOOD, 8),
            _stroke("tyopoyta_handle", [(362, 113), (414, 113)], 8),
            *_panel("tyopoyta_top", 14, 29, 492, 49, WOOD, 8),
        ]),
        "kannettava": prop("Kannettava tietokone", 340, 250, [
            *_panel("kannettava_lid", 43, 21, 254, 171, METAL, 9),
            _rect("kannettava_screen", 60, 39, 220, 136, TEAL),
            _stroke("kannettava_screen_line", [(84, 78), (145, 78), (145, 95), (211, 95)], 7, TEAL_LIGHT),
            _ellipse("kannettava_camera", 166, 26, 8, 8, INK),
            *_panel("kannettava_keyboard", 15, 190, 310, 39, METAL, 7),
            _stroke("kannettava_trackpad", [(132, 209), (207, 209)], 6, TEAL),
        ]),
        "puhelin": prop("Puhelin", 160, 270, [
            *_panel("puhelin_case", 25, 13, 110, 242, TEAL, 8),
            _rect("puhelin_screen", 39, 42, 82, 165, PAPER),
            _stroke("puhelin_speaker", [(61, 29), (99, 29)], 5),
            _ellipse("puhelin_home", 72, 222, 16, 16, INK),
            _rect("puhelin_message", 49, 64, 61, 28, TEAL_LIGHT),
            _rect("puhelin_message_two", 58, 104, 52, 21, AMBER),
            _stroke("puhelin_notification", [(54, 158), (67, 171), (99, 139)], 8, RUST),
            _rect("puhelin_side_button", 135, 68, 5, 35, INK),
        ]),
        "ajopaivakirja": prop("Ajopäiväkirja", 250, 310, [
            *_panel("ajopaivakirja_cover", 25, 22, 203, 267, RUST, 9),
            _rect("ajopaivakirja_spine", 32, 31, 26, 249, (126, 69, 53, 255)),
            _rect("ajopaivakirja_label", 76, 66, 119, 100, PAPER),
            _stroke("ajopaivakirja_road", [(100, 147), (123, 83), (147, 83), (171, 147)], 8, TEAL),
            _stroke("ajopaivakirja_road_dashes", [(135, 107), (135, 132)], 5, AMBER),
            _stroke("ajopaivakirja_binding_top", [(16, 77), (48, 77)], 9),
            _stroke("ajopaivakirja_binding_bottom", [(16, 236), (48, 236)], 9),
            _rect("ajopaivakirja_bookmark", 178, 248, 22, 52, AMBER),
        ]),
        "kello": prop("Myöhässä oleva kello", 230, 230, [
            *_disc("kello_face", 15, 15, 200, 200, PAPER, 10),
            _stroke("kello_hour", [(115, 115), (78, 91)], 9),
            _stroke("kello_minute", [(115, 115), (115, 49)], 7),
            _ellipse("kello_pin", 107, 107, 16, 16, RUST),
            _stroke("kello_tick_top", [(115, 32), (115, 41)], 6, TEAL),
            _stroke("kello_tick_right", [(190, 115), (199, 115)], 6, TEAL),
            _stroke("kello_tick_bottom", [(115, 190), (115, 199)], 6, TEAL),
        ]),
        "liikennemerkki": prop("Yövuoron pysäkkimerkki", 220, 540, [
            _stroke("liikennemerkki_post", [(110, 508), (110, 164)], 16, METAL),
            *_panel("liikennemerkki_sign", 29, 21, 162, 183, TEAL_LIGHT, 9),
            _rect("liikennemerkki_bus", 57, 51, 106, 96, INK),
            _rect("liikennemerkki_window", 67, 64, 86, 38, PAPER),
            _ellipse("liikennemerkki_wheel_left", 68, 138, 22, 29, INK),
            _ellipse("liikennemerkki_wheel_right", 131, 138, 22, 29, INK),
        ]),
        "paketti": prop("Salaperäinen paketti", 300, 260, [
            *_panel("paketti_box", 21, 41, 258, 193, WOOD, 9),
            _rect("paketti_lid", 30, 50, 240, 38, (189, 151, 106, 255)),
            _rect("paketti_tape", 127, 50, 36, 175, AMBER),
            _stroke("paketti_seam", [(31, 91), (269, 91)], 7),
            _rect("paketti_label", 181, 124, 64, 67, PAPER),
            _stroke("paketti_arrow", [(194, 158), (213, 138), (232, 158)], 6, TEAL),
            _stroke("paketti_arrow_stem", [(213, 139), (213, 179)], 6, TEAL),
            _stroke("paketti_scuff", [(47, 194), (79, 210)], 6, RUST),
        ]),
        "tyokaluavain": prop("Kipinän kiintoavain", 150, 390, [
            _stroke("tyokaluavain_handle_outline", [(75, 324), (75, 124)], 43),
            _stroke("tyokaluavain_handle_metal", [(75, 324), (75, 124)], 29, METAL),
            _stroke("tyokaluavain_jaw_outline", [(30, 36), (27, 86), (49, 113), (101, 113), (123, 86), (120, 36)], 28),
            _stroke("tyokaluavain_jaw_metal", [(30, 36), (27, 86), (49, 113), (101, 113), (123, 86), (120, 36)], 16, METAL),
            _stroke("tyokaluavain_ring", [(75 + 28 * cos(i * pi / 16), 332 + 28 * sin(i * pi / 16)) for i in range(33)], 15),
            _stroke("tyokaluavain_ring_metal", [(75 + 28 * cos(i * pi / 16), 332 + 28 * sin(i * pi / 16)) for i in range(33)], 7, METAL),
        ]),
        "sateenvarjo": prop("Yövuoron sateenvarjo", 380, 450, [
            _stroke("sateenvarjo_shaft", [(190, 44), (190, 374), (204, 405), (228, 411), (247, 391)], 14),
            _clip(_ellipse("sateenvarjo_canopy_outline", 20, 35, 340, 207, INK), 20, 35, 340, 137),
            _clip(_ellipse("sateenvarjo_canopy_fill", 29, 44, 322, 189, TEAL), 29, 44, 322, 122),
            _stroke("sateenvarjo_handle_grip", [(203, 404), (228, 411), (247, 391)], 8, WOOD),
            _stroke("sateenvarjo_left_rib", [(190, 47), (132, 89), (108, 160)], 7, INK),
            _stroke("sateenvarjo_right_rib", [(190, 47), (248, 89), (272, 160)], 7, INK),
            _stroke("sateenvarjo_rim", [(25, 157), (108, 174), (190, 157), (272, 174), (355, 157)], 12, INK),
            _stroke("sateenvarjo_tip", [(190, 25), (190, 45)], 9, INK),
        ]),
        "auto": prop("Veikon pieni auto", 650, 330, [
            *_panel("auto_cabin", 173, 42, 285, 159, TEAL, 11),
            _rect("auto_window_left", 195, 63, 110, 89, TEAL_LIGHT),
            _rect("auto_window_right", 321, 63, 114, 89, TEAL_LIGHT),
            *_panel("auto_body", 36, 150, 578, 125, TEAL, 11),
            _rect("auto_headlamp", 564, 177, 38, 31, AMBER),
            _rect("auto_tail_light", 48, 178, 24, 30, RUST),
            *_disc("auto_wheel_left", 116, 223, 105, 91, METAL, 21),
            *_disc("auto_wheel_right", 433, 223, 105, 91, METAL, 21),
        ]),
        "vuoronumerolaite": prop("Ikuisen jonon vuoronumerolaite", 240, 380, [
            *_panel("vuoronumerolaite_stand", 95, 217, 50, 118, METAL, 8),
            _rect("vuoronumerolaite_foot", 49, 327, 142, 22, INK),
            *_panel("vuoronumerolaite_body", 36, 25, 168, 218, RUST, 10),
            _rect("vuoronumerolaite_display", 61, 53, 118, 55, INK),
            _stroke("vuoronumerolaite_display_digit", [(91, 67), (91, 94), (105, 94), (105, 67), (91, 67)], 5, AMBER),
            _stroke("vuoronumerolaite_display_second", [(132, 67), (147, 67), (147, 80), (132, 94)], 5, AMBER),
            _ellipse("vuoronumerolaite_button", 104, 127, 32, 32, AMBER),
            _rect("vuoronumerolaite_slot", 65, 184, 111, 17, INK),
            _rect("vuoronumerolaite_ticket", 91, 190, 57, 76, PAPER),
            _stroke("vuoronumerolaite_ticket_mark", [(105, 215), (133, 215)], 6, TEAL),
        ]),
    }


BACKGROUNDS = _backgrounds()
PROPS = _props()


if __name__ == "__main__":
    for catalog in (BACKGROUNDS, PROPS):
        for key, value in catalog.items():
            print(f"{key}: {value['name']} ({len(value['layers'])} editable layers)")
