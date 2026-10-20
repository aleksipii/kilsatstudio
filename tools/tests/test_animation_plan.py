"""Negative contract fixtures; these do not test animation quality or runtime."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("check_animation_plan", ROOT / "tools/check_animation_plan.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class DesignContractTests(unittest.TestCase):
    def setUp(self):
        self.plan = json.loads((ROOT / "docs/automation/plan.json").read_text(encoding="utf-8"))
        self.intent = json.loads((ROOT / "examples/automation/pilot-intent.json").read_text(encoding="utf-8"))
        self.source = (ROOT / "examples/automation/pilot-fi.txt").read_bytes()

    def check(self):
        return checker.validate(self.plan, self.intent, self.source)

    def reject(self, text):
        with self.assertRaisesRegex(checker.PlanError, text):
            self.check()

    def test_valid_pilot_is_not_reported_as_rendered_or_measured(self):
        self.assertEqual(self.check(), {"status": "design_fixture_valid", "agents": 10,
            "duration_s": 50, "shots": 8, "animation_rendered": False, "sla_measured": False})
        checker.check_reports(self.plan)

    def test_source_change_invalidates_spans(self):
        self.source += "ä".encode()
        self.reject("stale source hash")

    def test_dialogue_rewrite_rejected(self):
        self.intent["beats"][0]["text"] = "Uusi repliikki"
        self.reject("dialogue changed")

    def test_unknown_speaker_rejected(self):
        self.intent["beats"][0]["actor"] = "THIRD"
        self.reject("unknown speaker")

    def test_nonfinite_or_bool_time_rejected(self):
        for value in (float("nan"), float("inf"), True, -1):
            with self.subTest(value=value):
                self.intent["beats"][0]["end_s"] = value
                self.reject("beat outside")

    def test_overlapping_speaker_rejected(self):
        extra = copy.deepcopy(self.intent["beats"][0])
        extra["id"] = "overlap"
        self.intent["beats"].append(extra)
        self.reject("overlapping actor")

    def test_shot_gap_and_excess_shots_rejected(self):
        self.intent["shot_proposal"][1]["start_s"] += 1
        self.reject("without gaps")
        self.setUp()
        self.intent["shot_proposal"] *= 3
        self.reject("sixteen final shots")

    def test_deadline_budgets_and_truthful_status(self):
        self.plan["stage_caps_s"]["render"] += 1
        self.reject("total 300")
        self.setUp()
        self.plan["sla_measured"] = True
        self.reject("no measured SLA")

    def test_missing_agent_and_duplicate_agent_rejected(self):
        self.plan["agents"].pop()
        self.reject("ten agent reports")
        self.setUp()
        self.plan["agents"][1]["id"] = 1
        self.reject("IDs must")

    def test_source_span_mismatch(self):
        self.intent["beats"][0]["source_span"]["start_byte"] += 1
        self.reject("source span text mismatch")

    def test_utf8_character_boundary(self):
        self.intent["beats"][0]["source_span"]["start_byte"] = self.source.index("ä".encode()) + 1
        self.reject("splits a UTF-8")

    def test_unknown_or_missing_provenance(self):
        self.intent["beats"][0].pop("provenance")
        self.reject("provenance")
        self.setUp()
        self.intent["beats"][0]["provenance"]["rule_id"] = "INVENTED"
        self.reject("provenance")

    def test_insert_time_must_match_shot(self):
        self.intent["app_insert"].update(start_s=5, end_s=10)
        self.reject("insert and shot interval differ")

    def test_duplicate_shot_id(self):
        self.intent["shot_proposal"][1]["id"] = self.intent["shot_proposal"][0]["id"]
        self.reject("duplicate shot ID")

    def test_shot_cannot_reference_other_story_scene(self):
        self.intent["scenes"][0]["duration_s"] = 49
        self.intent["scenes"].append({"id": "other", "duration_s": 1})
        self.intent["shot_proposal"][0]["scene"] = "other"
        self.reject("outside referenced story scene")

    def test_cross_speaker_overlap_requires_future_explicit_support(self):
        self.intent["beats"][1]["start_s"] = 7
        self.reject("overlapping speakers")


if __name__ == "__main__":
    unittest.main()
