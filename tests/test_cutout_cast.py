"""The committed cut-out cast matches its generator; no network or model needed."""
import importlib.util
import json
import math
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "generate_cutout_cast", ROOT / "scripts/generate_cutout_cast.py")
cast = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cast)
EXAMPLE = ROOT / "examples/cutout-cast"


class CutoutCastTest(unittest.TestCase):
    def test_committed_projects_match_the_generator(self):
        with tempfile.TemporaryDirectory() as directory:
            written = cast.generate(Path(directory))
            for relative in written:
                self.assertEqual((Path(directory) / relative).read_bytes(),
                                 (EXAMPLE / relative).read_bytes(),
                                 f"{relative} is stale; run scripts/generate_cutout_cast.py")

    def test_characters_share_the_stick_joint_interface_and_move_on_twos(self):
        definitions = cast.characters()
        self.assertEqual(list(definitions), ["ILONA", "TOPI", "RAUNO"])
        for identifier, character in definitions.items():
            self.assertEqual(character["timing"], {"on": 2, "turn": "snap"}, identifier)
            joints = tuple(joint["id"] for joint in character["rig"]["joints"])
            self.assertEqual(joints, cast.JOINT_IDS)
            self.assertEqual(tuple(character["rig"]["visemes"]["map"]), cast.VISEME_NAMES)

    def test_sleeves_and_trouser_legs_are_single_seamless_skinned_pieces(self):
        for identifier, character in cast.characters().items():
            parts = {part["id"]: part for part in character["rig"]["parts"]}
            for side in ("l", "r"):
                self.assertNotIn("upper_arm_" + side, parts, identifier)
                self.assertNotIn("forearm_" + side, parts, identifier)
                for name, upper, lower in (("arm_" + side, "upper_arm_" + side, "forearm_" + side),
                                           ("leg_" + side, "thigh_" + side, "shin_" + side)):
                    part = parts[name]
                    self.assertEqual(len(part["skin"]), len(part["points"]))
                    joints = {w["joint"] for weights in part["skin"] for w in weights}
                    self.assertEqual(joints, {upper, lower}, (identifier, name))
                    for weights in part["skin"]:
                        self.assertAlmostEqual(sum(w["weight"] for w in weights), 1.0)

    def test_arms_draw_last_with_round_shoulders_and_limited_reach(self):
        for identifier, character in cast.characters().items():
            rig = character["rig"]
            order = [part["id"] for part in rig["parts"]]
            self.assertEqual(order[-8:], ["arm_l", "cuff_l", "thumb_l", "hand_l",
                                          "arm_r", "cuff_r", "thumb_r", "hand_r"], identifier)
            self.assertEqual(rig["joint_limits"], cast.ARM_LIMITS)
            rest = {joint["id"]: joint.get("rotation", 0) for joint in rig["joints"]}
            for joint, limit in cast.ARM_LIMITS.items():
                self.assertTrue(limit["min_rotation"] <= rest[joint] <= limit["max_rotation"])
            # Not a mirror image at rest.
            self.assertNotEqual(rest["upper_arm_l"], -rest["upper_arm_r"])
            # The sleeve's shoulder end is a half circle round the pivot.
            arm = next(part for part in rig["parts"] if part["id"] == "arm_r")
            top = [p for p in arm["points"] if p[1] < 0]
            self.assertTrue(top)
            for x, y in top:
                self.assertAlmostEqual(math.hypot(x, y), 8, delta=0.05)

    def test_soft_corners_keep_the_corner_and_stay_simple(self):
        square = cast._soft(cast._box(0, 0, 20, 20), 4)
        self.assertEqual(len(square), 12)
        self.assertIn([20, 0], square)
        self.assertIn([16.0, 0.0], square)
        self.assertTrue(cast._simple(cast._rounded(square, 2)))

    def test_clothes_sit_where_they_belong(self):
        definitions = cast.characters()
        def ids(identifier):
            return [part["id"] for part in definitions[identifier]["rig"]["parts"]]
        ilona, topi, rauno = ids("ILONA"), ids("TOPI"), ids("RAUNO")
        # Bands and collars draw over the garment they belong to.
        self.assertLess(ilona.index("body"), ilona.index("sweater_hem"))
        self.assertLess(ilona.index("body"), ilona.index("collar"))
        self.assertLess(topi.index("body"), topi.index("hood"))
        self.assertLess(topi.index("hoodie_hem"), topi.index("hoodie_pocket"))
        # Rauno's shirt collar sits under the overall straps.
        self.assertLess(rauno.index("shirt_collar_l"), rauno.index("strap_l"))
        bib = next(p for p in definitions["RAUNO"]["rig"]["parts"] if p["id"] == "overall_bib")
        # The overalls cover the shirt hem and dip between the legs.
        hem = cast.BUILD["RAUNO"]["hem"]
        self.assertGreater(max(x for x, _ in bib["points"]), hem)
        self.assertIn([0, 7], bib["points"])

    def test_the_cap_is_fitted_to_the_head(self):
        rx, ry = cast.BUILD["TOPI"]["head"]
        parts = {p["id"]: p for p in cast.characters()["TOPI"]["rig"]["parts"]}
        crown = parts["cap_crown"]["points"]
        # The lower corners of the crown sit on the skull outline (fabric
        # thickness only), not out in the air beside the head.
        for x, y in (crown[0], crown[-1]):
            self.assertLess(math.hypot(x / rx, y / ry), 1.08)
        # Nothing of the cap reaches far past the head except the brim when turned.
        for name in ("cap_crown", "cap_brim"):
            reach = max(math.hypot(x / rx, y / ry) for x, y in parts[name]["points"])
            self.assertLess(reach, 1.25, name)

    def test_overalls_have_a_back(self):
        parts = {p["id"]: p for p in cast.characters()["RAUNO"]["rig"]["parts"]}
        # No bib from behind, and the straps cross to the opposite hip.
        self.assertGreaterEqual(min(y for _, y in parts["overall_bib"]["views"]["back"]["points"]),
                                -20)
        left = parts["strap_l"]["views"]["back"]["points"]
        self.assertLess(left[0][0], 0)
        self.assertGreater(left[2][0], 0)

    def test_check_rejects_shapes_the_rig_validator_would_refuse(self):
        def broken(change):
            definitions = cast.characters()
            change(definitions["ILONA"]["rig"]["parts"][0])
            return definitions
        bad = [
            # smooth 1 is not an accepted rounding value.
            lambda part: part.update(smooth=1),
            # A bow tie crosses itself.
            lambda part: part.update(points=[[0, 0], [10, 10], [10, 0], [0, 10]]),
            # A spike running straight back along its own edge.
            lambda part: part.update(points=[[0, 0], [10, 0], [20, 0], [5, 0], [0, 10]]),
        ]
        for change in bad:
            with self.assertRaises(AssertionError):
                cast.check(broken(change))
        cast.check(cast.characters())

    def test_demo_turns_every_character_and_stays_offline(self):
        project = json.loads((EXAMPLE / "cutout-cast.kilsat").read_text(encoding="utf-8"))
        self.assertEqual(project["assets"], [])
        source = cast.motion_source.loads(project["source"])
        faced = {event["actor"] for event in source["scenes"][0]["events"]
                 if event["type"] == "face"}
        self.assertEqual(faced, {"ILONA", "TOPI", "RAUNO"})
        self.assertNotIn("speak", {event["type"] for event in source["scenes"][0]["events"]})


if __name__ == "__main__":
    unittest.main()
