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
        self.assertEqual(len(result['unresolved']), 9 + len(mac.SCENARIOS['launch-project']))
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
        self.assertEqual(sum('scenario' in x for x in result['unresolved']), sum(map(len, mac.SCENARIOS.values())))
        self.assertEqual(result['physical_device_acceptance'], 'not_granted')

    def test_complete_scenarios_still_cannot_grant_acceptance(self):
        data = b'Synthetic detailed operator observations, not actual device evidence'
        (self.root / 'report.txt').write_bytes(data)
        self.value['observations'] = [dict(
            gate=gate, status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
            evidence=[dict(path='report.txt', **mac.identity(data))],
            scenarios=[dict(scenario=scenario, status='pass', steps='Fixture procedure',
                            expected='Fixture expectation', actual='Fixture observation',
                            evidence_paths=['report.txt']) for scenario in mac.SCENARIOS[gate]])
            for gate in mac.GATES]
        result = self.collect()
        self.assertEqual({x['gate'] for x in result['unresolved']}, {'authority-review', 'build-attestation'})
        self.assertFalse(result['distribution_approved'])
        self.assertEqual(result['physical_device_acceptance'], 'not_granted')

    def test_scenario_evidence_and_contradictions_rejected(self):
        data = b'Synthetic observation'
        (self.root / 'report.txt').write_bytes(data)
        scenario = dict(scenario='launch', status='pass', steps='Fixture', expected='Fixture',
                        actual='Fixture', evidence_paths=['unretained.txt'])
        gate = dict(gate='launch-project', status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
                    evidence=[dict(path='report.txt', **mac.identity(data))], scenarios=[scenario])
        self.value['observations'] = [gate]
        with self.assertRaisesRegex(ValueError, 'unique retained'):
            self.collect()
        scenario['evidence_paths'] = []
        with self.assertRaisesRegex(ValueError, 'tested scenario requires'):
            self.collect()
        scenario['evidence_paths'] = ['report.txt']
        scenario['status'] = 'fail'
        with self.assertRaisesRegex(ValueError, 'passing gate'):
            self.collect()
        gate['status'] = 'not_tested'
        with self.assertRaisesRegex(ValueError, 'untested gate'):
            self.collect()
        gate['status'] = 'fail'
        result = self.collect()
        self.assertIn(dict(gate='launch-project', scenario='launch', reason='fail'), result['unresolved'])
        scenario['scenario'] = 'invented'
        with self.assertRaisesRegex(ValueError, 'unknown/duplicate device scenario'):
            self.collect()

    def test_duplicate_scenarios_and_duplicate_paths_rejected(self):
        data = b'Synthetic observation'
        (self.root / 'report.txt').write_bytes(data)
        scenario = dict(scenario='launch', status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
                        evidence_paths=['report.txt'])
        gate = dict(gate='launch-project', status='pass', steps='Fixture', expected='Fixture', actual='Fixture',
                    evidence=[dict(path='report.txt', **mac.identity(data))], scenarios=[scenario, scenario])
        self.value['observations'] = [gate]
        with self.assertRaisesRegex(ValueError, 'unknown/duplicate device scenario'):
            self.collect()
        gate['scenarios'] = [scenario]
        scenario['evidence_paths'] = ['report.txt', 'report.txt']
        with self.assertRaisesRegex(ValueError, 'unique retained'):
            self.collect()

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
