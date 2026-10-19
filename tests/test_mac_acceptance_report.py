"""Human claims are kept distinct from final acceptance."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('mac_acceptance_report', Path(__file__).resolve().parents[1] / 'scripts/mac_acceptance_report.py')
mac = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mac)


class MacAcceptanceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.archive = self.root / 'preview.zip'
        self.archive.write_bytes(b'synthetic archive identity')
        self.observations = self.root / 'observations.json'
        self.value = dict(schema_version=1, artifact=mac.identity(self.archive.read_bytes()), operator='Synthetic tester', utc_date='2026-10-19',
                          mac_model='Synthetic Mac', chip='arm64 fixture', macos='fixture only', build_commit='a' * 40, observations=[])

    def collect(self):
        self.observations.write_text(json.dumps(self.value))
        return mac.collect(self.archive, self.observations, self.root)

    def test_all_missing_gates_are_open_no_device_approval(self):
        result = self.collect()
        self.assertEqual(len(result['unresolved']), 10)
        self.assertEqual(result['physical_device_acceptance'], 'not_granted')
        self.assertFalse(result['distribution_approved'])
        self.assertEqual(result, self.collect())

    def test_tested_gate_requires_exact_retained_bytes(self):
        item = dict(gate='launch-project', status='pass', steps='Synthetic steps', expected='Fixture result', actual='Fixture result', evidence=[])
        self.value['observations'] = [item]
        with self.assertRaisesRegex(ValueError, 'retained evidence'):
            self.collect()
        (self.root / 'capture.txt').write_bytes(b'synthetic observation')
        item['evidence'] = [dict(path='capture.txt', **mac.identity(b'synthetic observation'))]
        result = self.collect()
        self.assertEqual(len(result['unresolved']), 9)
        self.assertEqual(result['physical_device_acceptance'], 'not_granted')
        (self.root / 'capture.txt').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'digest/size'):
            self.collect()

    def test_stale_build_unknown_gate_and_duplicate_rejected(self):
        self.value['artifact']['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'current archive'):
            self.collect()
        self.value['artifact'] = mac.identity(self.archive.read_bytes())
        self.value['observations'] = [dict(gate='invented')]
        with self.assertRaisesRegex(ValueError, 'unknown/duplicate'):
            self.collect()

    def test_archive_and_observation_aliases_cannot_be_gate_evidence(self):
        self.value['observations'] = [dict(gate='launch-project', status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
                                         evidence=[dict(path=self.archive.name, **self.value['artifact'])])]
        with self.assertRaisesRegex(ValueError, 'aliases'):
            self.collect()

    def test_all_pass_claims_still_require_authority_and_build_binding(self):
        data = b'Synthetic reviewed observations'
        (self.root / 'report.txt').write_bytes(data)
        self.value['observations'] = [dict(gate=gate, status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
                                         evidence=[dict(path='report.txt', **mac.identity(data))]) for gate in mac.GATES]
        result = self.collect()
        self.assertEqual({x['gate'] for x in result['unresolved']}, {'authority-review', 'build-attestation'})
        self.assertEqual(result['physical_device_acceptance'], 'not_granted')

    def test_cli_strict_and_preserves_report(self):
        self.collect()
        output = self.root / 'report.json'
        args = ['--archive', str(self.archive), '--observations', str(self.observations), '--evidence-root', str(self.root), '--output', str(output), '--strict']
        with patch('subprocess.run', side_effect=AssertionError('execute')), patch('socket.socket', side_effect=AssertionError('network')):
            self.assertEqual(mac.main(args), 1)
        before = output.read_bytes()
        self.assertEqual(mac.main(args), 2)
        self.assertEqual(before, output.read_bytes())


if __name__ == '__main__':
    unittest.main()
