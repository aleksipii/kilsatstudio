"""Portable release-orchestration tests; no Apple command or account is used."""
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import developer_release as release


TEAM = "ABC1234567"
IDENTITY = f"Developer ID Application: Example ({TEAM})"


class FakeAppleTools:
    """Model tool effects on a staged bundle, retaining every command for review."""

    def __init__(self, *, team=TEAM, status="Accepted", fail=None, split_details=False):
        self.team = team
        self.status = status
        self.fail = fail
        self.split_details = split_details
        self.calls = []
        self.entitlements = {}
        self.submission_bytes = None

    def __call__(self, command, **kwargs):
        command = [str(part) for part in command]
        self.calls.append(command)
        if not kwargs.get("check") or not 0 < kwargs.get("timeout", 0) <= 1260:
            raise AssertionError("Release tools must fail closed and have finite timeouts")
        if self.fail and self.fail(command):
            raise subprocess.CalledProcessError(1, command, stderr="injected tool failure")
        stdout = stderr = ""
        if command[0] == "codesign":
            if "--sign" in command:
                if "--entitlements" in command:
                    path = Path(command[command.index("--entitlements") + 1])
                    self.entitlements[command[-1]] = plistlib.loads(path.read_bytes())
            elif "--display" in command:
                # codesign normally writes details to stderr.
                if self.split_details:
                    stdout = f"TeamIdentifier={self.team}"
                    stderr = f"Authority={IDENTITY}\n"
                else:
                    stderr = f"Authority={IDENTITY}\nTeamIdentifier={self.team}\n"
        elif command[:3] == ["xcrun", "notarytool", "submit"]:
            self.submission_bytes = Path(command[3]).read_bytes()
            stdout = json.dumps({"id": "fake-submission-id", "status": self.status})
        elif command[:3] == ["xcrun", "stapler", "staple"]:
            (Path(command[-1]) / "Contents" / "stapled-ticket").write_bytes(b"fake accepted ticket")
        elif command[:3] == ["xcrun", "stapler", "validate"]:
            if not (Path(command[-1]) / "Contents" / "stapled-ticket").is_file():
                raise AssertionError("Cannot validate an unstapled app")
        elif command[0] == "spctl":
            if not (Path(command[-1]) / "Contents" / "stapled-ticket").is_file():
                raise AssertionError("Gatekeeper assessment must use the stapled candidate")
        elif command[0] == "ditto":
            source, destination = Path(command[-2]), Path(command[-1])
            base = source.parent if "--keepParent" in command else source
            with zipfile.ZipFile(destination, "w") as archive:
                for item in sorted(source.rglob("*")):
                    if item.is_file():
                        archive.write(item, item.relative_to(base).as_posix())
        else:
            raise AssertionError(f"Unexpected external command: {command}")
        return subprocess.CompletedProcess(command, 0, stdout, stderr)


class DeveloperReleaseTests(unittest.TestCase):
    def validate(self, root, **kw):
        args = dict(identity=IDENTITY, team=TEAM, profile="operator-profile",
                    output=root / "release.zip", report=root / "report.json", consent=True)
        args.update(kw)
        return release.validate(**args)

    def inputs(self, root):
        result = []
        for name in ("desktop", "gpu", "head"):
            path = root / name
            path.write_bytes(b"fake native binary")
            path.chmod(0o755)
            result.append(path)
        return result

    @staticmethod
    def stage(repo, binary, directory, commit, arch, profile, gpu, head):
        """Use fake native files while leaving run_release's orchestration real."""
        del repo, commit, arch, profile
        bundle = directory / "Kilsat Studio Preview.app"
        main = bundle / "Contents" / "MacOS" / "studio-desktop"
        main.parent.mkdir(parents=True)
        shutil.copy2(binary, main)
        helpers = bundle / "Contents" / "Resources" / "bin"
        helpers.mkdir(parents=True)
        shutil.copy2(gpu, helpers / "gpu_probe")
        shutil.copy2(head, helpers / "head_pose")
        (directory / "LICENSE").write_text("fake license", encoding="utf-8")
        return bundle

    def invoke(self, root, tools):
        binary, gpu, head = self.inputs(root)
        with patch.object(release.package_macos, "stage_distribution", side_effect=self.stage):
            return release.run_release(root, binary, gpu, head, "a" * 40,
                                       IDENTITY, TEAM, "operator-profile",
                                       root / "release.zip", root / "report.json", True, run=tools)

    def test_explicit_identity_team_consent_and_new_outputs_are_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.validate(root)
            for change in [dict(identity="-"), dict(team="OTHERTEAM0"), dict(profile=""),
                           dict(consent=False), dict(output=root / "x.txt")]:
                with self.subTest(change=change), self.assertRaises(ValueError):
                    self.validate(root, **change)
            (root / "release.zip").write_text("existing")
            with self.assertRaises(ValueError):
                self.validate(root)
            self.assertEqual((root / "release.zip").read_text(), "existing")

    def test_symlink_input_cannot_be_a_signing_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "binary").write_text("source")
            (root / "link").symlink_to(root / "binary")
            with self.assertRaises(ValueError):
                release.regular(root / "link", "binary")

    def test_platform_refuses_before_any_sign_or_notarization_command(self):
        with patch.object(release.sys, "platform", "linux"), patch.object(release.subprocess, "run") as run:
            with self.assertRaises(SystemExit):
                release.main(["--binary", "b", "--gpu-probe", "g", "--head-pose", "h",
                              "--output", "out.zip", "--report", "r.json", "--commit", "a" * 40,
                              "--identity", IDENTITY, "--team", TEAM,
                              "--notary-profile", "p", "--allow-notarization"])
            run.assert_not_called()

    def test_nested_signatures_precede_app_and_camera_entitlements_are_explicit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tools = FakeAppleTools()
            self.invoke(root, tools)
            signs = [command for command in tools.calls if command[0] == "codesign" and "--sign" in command]
            self.assertEqual([Path(command[-1]).name for command in signs],
                             ["gpu_probe", "head_pose", "Kilsat Studio Preview.app"])
            for command in signs:
                self.assertEqual(command[command.index("--sign") + 1], IDENTITY)
                self.assertEqual(command[command.index("--options") + 1], "runtime")
                self.assertIn("--timestamp", command)
            self.assertNotIn(signs[0][-1], tools.entitlements)
            self.assertEqual(tools.entitlements[signs[1][-1]], release.CAMERA_ENTITLEMENTS)
            self.assertEqual(tools.entitlements[signs[2][-1]], release.CAMERA_ENTITLEMENTS)
            for command in signs:
                at = tools.calls.index(command)
                self.assertEqual(tools.calls[at + 1], ["codesign", "--verify", "--strict", command[-1]])

    def test_signed_team_mismatch_stops_before_notary_upload(self):
        # A matching prefix is also a mismatch: fields must match completely.
        for actual in ("OTHERTEAM0", TEAM + "EXTRA", TEAM + "\nTeamIdentifier=OTHERTEAM0"):
            with self.subTest(actual=actual), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                tools = FakeAppleTools(team=actual)
                with self.assertRaisesRegex(ValueError, "identity/team"):
                    self.invoke(root, tools)
                self.assertFalse(any(command[:2] == ["xcrun", "notarytool"] for command in tools.calls))
                self.assertFalse((root / "release.zip").exists())
                self.assertFalse((root / "report.json").exists())

    def test_rejected_notarization_never_staples_or_creates_a_final_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tools = FakeAppleTools(status="Rejected")
            with self.assertRaisesRegex(ValueError, "not Accepted"):
                self.invoke(root, tools)
            self.assertTrue(any(command[:3] == ["xcrun", "notarytool", "submit"] for command in tools.calls))
            self.assertFalse(any(command[:2] == ["xcrun", "stapler"] for command in tools.calls))
            self.assertFalse(any(command[0] == "ditto" and "--sequesterRsrc" in command for command in tools.calls))
            self.assertFalse((root / "release.zip").exists())
            self.assertFalse((root / "report.json").exists())

    def test_success_staples_validates_and_hashes_only_the_final_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tools = FakeAppleTools(split_details=True)
            evidence = self.invoke(root, tools)
            staple = next(i for i, command in enumerate(tools.calls) if command[:3] == ["xcrun", "stapler", "staple"])
            validate = next(i for i, command in enumerate(tools.calls) if command[:3] == ["xcrun", "stapler", "validate"])
            deep_verify = next(i for i, command in enumerate(tools.calls) if command[:4] == ["codesign", "--verify", "--deep", "--strict"])
            assess = next(i for i, command in enumerate(tools.calls) if command[0] == "spctl")
            final_archive = next(i for i, command in enumerate(tools.calls) if command[0] == "ditto" and "--sequesterRsrc" in command)
            self.assertLess(staple, validate)
            self.assertLess(validate, deep_verify)
            self.assertLess(deep_verify, assess)
            self.assertLess(assess, final_archive)
            with zipfile.ZipFile(root / "release.zip") as archive:
                ticket = "Kilsat Studio Preview.app/Contents/stapled-ticket"
                self.assertEqual(archive.read(ticket), b"fake accepted ticket")
                self.assertIn("LICENSE", archive.namelist())
            final_bytes = (root / "release.zip").read_bytes()
            self.assertNotEqual(final_bytes, tools.submission_bytes)
            self.assertEqual(evidence["archive_sha256"], hashlib.sha256(final_bytes).hexdigest())
            self.assertEqual(json.loads((root / "report.json").read_text()), evidence)
            self.assertEqual(evidence["build_commit"], "a" * 40)
            self.assertTrue(evidence["stapled"])
            self.assertFalse(evidence["distribution_approved"])
            self.assertEqual(evidence["physical_mac_acceptance"], "not_granted")
            self.assertEqual(evidence["license_acceptance"], "not_granted")

    def test_staple_validation_or_final_verification_failures_do_not_publish(self):
        cases = {
            "staple": lambda command: command[:3] == ["xcrun", "stapler", "staple"],
            "ticket validation": lambda command: command[:3] == ["xcrun", "stapler", "validate"],
            "signed app verification": lambda command: command[:4] == ["codesign", "--verify", "--deep", "--strict"],
            "Gatekeeper": lambda command: command[0] == "spctl",
        }
        for name, fail in cases.items():
            with self.subTest(stage=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                tools = FakeAppleTools(fail=fail)
                with self.assertRaises(subprocess.CalledProcessError):
                    self.invoke(root, tools)
                self.assertFalse(any(command[0] == "ditto" and "--sequesterRsrc" in command for command in tools.calls))
                self.assertFalse((root / "release.zip").exists())
                self.assertFalse((root / "report.json").exists())
