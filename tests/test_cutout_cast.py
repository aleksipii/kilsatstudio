"""The committed cut-out cast matches its generator; no network or model needed."""
import importlib.util
import json
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
        source = json.loads(project["source"])
        faced = {event["actor"] for event in source["scenes"][0]["events"]
                 if event["type"] == "face"}
        self.assertEqual(faced, {"ILONA", "TOPI", "RAUNO"})
        self.assertNotIn("speak", {event["type"] for event in source["scenes"][0]["events"]})


if __name__ == "__main__":
    unittest.main()
