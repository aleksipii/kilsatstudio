"""Synthetic artifact-bound corpus collection; no external binary/model execution."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[1]


def load(path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


reporter = load(REPO / 'scripts/release_license_report.py')
fixture = load(REPO / 'tests/test_package_audit.py')


class ReleaseLicenseReportTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixture.PackageAuditTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.stage(helper=True)
        self.archive = self.fixture.write_zip().resolve()
        self.root = self.fixture.root.resolve()
        self.audit = self.fixture.collect()
        self.audit_path = self.root / 'audit.json'
        self.build_path = self.root / 'build.json'
        self.index_path = self.root / 'index.json'
        self.corpus = self.root / 'corpus'
        self.corpus.mkdir()
        self.write(self.audit_path, self.audit)
        self.build = dict(schema_version=1, distribution_approved=False, physical_device_acceptance='not_granted',
                          artifact=self.audit['artifact'], inputs={'package_audit': reporter.identity(self.audit_path.read_bytes())},
                          native_observations=[dict(path=x['path'], sha256=x['sha256']) for x in self.audit['components']],
                          graphs=[dict(root=name, packages=[dict(id='pkg:' + name, name=name, version='1', source='registry:synthetic')])
                                  for name in ('studio-desktop', 'motion-render')])
        self.write(self.build_path, self.build)
        self.index = dict(schema_version=1, artifact=self.audit['artifact'], build_evidence=reporter.identity(self.build_path.read_bytes()), components=[dict(id='desktop', name='studio-desktop', version='1', source='registry:synthetic',
                         revision='synthetic-revision', boundary='bundled', graph_refs=[dict(graph='studio-desktop', id='pkg:studio-desktop')])], evidence=[], reviews=[])
        self.write(self.index_path, self.index)

    def write(self, path, value):
        path.write_text(json.dumps(value))

    def collect(self):
        self.write(self.index_path, self.index)
        return reporter.collect(self.archive, self.audit_path, self.build_path, self.index_path, self.corpus, REPO)

    def add_text(self):
        data = b'SYNTHETIC fixture only. No third party license represented.'
        (self.corpus / 'LICENSE.txt').write_bytes(data)
        self.index['evidence'] = [dict(id='text', component='desktop', path='LICENSE.txt', source='registry:synthetic', kind='license', **reporter.identity(data))]

    def test_determinism_explicit_gaps_and_no_approval(self):
        first = self.collect()
        self.assertEqual(first, self.collect())
        self.assertFalse(first['distribution_approved'])
        self.assertEqual(first['physical_device_acceptance'], 'not_granted')
        self.assertIn('graph-candidate-unmapped', {x['code'] for x in first['unresolved']})

    def test_changed_archive_audit_build_are_rejected(self):
        self.build['inputs']['package_audit']['sha256'] = '0' * 64
        self.write(self.build_path, self.build)
        with self.assertRaisesRegex(ValueError, 'unbound'):
            self.collect()
        self.archive.write_bytes(b'not zip')
        with self.assertRaisesRegex(ValueError, 'revalidated'):
            self.collect()

    def test_corpus_hash_symlink_traversal_and_hardlink_rejected(self):
        self.add_text()
        self.collect()
        self.index['evidence'][0]['path'] = '../outside'
        with self.assertRaisesRegex(ValueError, 'relative'):
            self.collect()
        self.index['evidence'][0]['path'] = 'LICENSE.txt'
        original = self.corpus / 'LICENSE.txt'
        original.rename(self.corpus / 'real')
        original.symlink_to('real')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.collect()
        original.unlink()
        original.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'hash/size'):
            self.collect()
        original.unlink()
        os.link(self.index_path, original)
        self.index['evidence'][0].update(reporter.identity(original.read_bytes()))
        with self.assertRaises(ValueError):
            self.collect()

    def test_duplicate_unknown_boundary_and_same_name_different_source(self):
        self.index['components'].append(dict(self.index['components'][0]))
        with self.assertRaisesRegex(ValueError, 'duplicate component'):
            self.collect()
        self.index['components'].pop()
        self.index['components'][0]['source'] = 'registry:other'
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            self.collect()

    def test_review_binds_component_text_and_all_obligation_categories(self):
        self.add_text()
        component = self.index['components'][0]
        self.index['reviews'] = [dict(component='desktop', reviewer='Fixture reviewer', date='2026-10-19', rationale='Synthetic validation',
               declared_expression='MIT OR Apache-2.0', component_sha256=reporter.identity(json.dumps(component, sort_keys=True, separators=(',', ':')).encode())['sha256'],
               evidence=[dict(id='text', sha256=self.index['evidence'][0]['sha256'])], obligations=[])]
        result = self.collect()
        codes = {x['code'] for x in result['unresolved']}
        self.assertIn('license-alternative-unselected', codes)
        self.assertIn('obligation-category-missing', codes)
        self.index['reviews'][0]['evidence'][0]['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'stale/dangling'):
            self.collect()

    def test_prepared_notice_is_not_packaged(self):
        self.test_review_binds_component_text_and_all_obligation_categories()
        review = self.index['reviews'][0]
        review['evidence'][0]['sha256'] = self.index['evidence'][0]['sha256']
        review['obligations'] = [dict(category='notice-copyright', status='satisfied', rationale='Fixture', evidence=['text'],
                                     destination='THIRD-PARTY.txt', sha256=self.index['evidence'][0]['sha256'])]
        self.assertIn('notice-not-packaged', {x['code'] for x in self.collect()['unresolved']})

    def test_bounds_shapes_and_no_subprocess_network(self):
        with patch('subprocess.run', side_effect=AssertionError('must not execute')), patch('socket.socket', side_effect=AssertionError('must not network')):
            self.collect()
        self.index['components'] = [None]
        with self.assertRaisesRegex(ValueError, 'bounded'):
            self.collect()
        with self.assertRaisesRegex(ValueError, 'byte budget'):
            reporter.regular(self.archive, 1)

    def test_missing_font_native_license_notice_source_remain_gaps(self):
        self.index['components'].append(dict(id='font', parent='desktop', name='font-fixture', version='1', source=None,
                                             revision=None, boundary='bundled'))
        result = self.collect()
        font = {x['code'] for x in result['unresolved'] if x['component'] == 'font'}
        self.assertTrue({'upstream-identity-missing', 'license-text-missing', 'provenance-evidence-missing',
                         'font-data-native-inventory-unreviewed', 'review-missing'} <= font)

    def test_corpus_binding_schema_and_duplicate_json_keys(self):
        self.index['build_evidence'] = dict(sha256='0' * 64, size_bytes=1)
        with self.assertRaisesRegex(ValueError, 'bind exact artifact'):
            self.collect()
        self.index_path.write_text('{"schema_version":1,"schema_version":1}')
        with self.assertRaisesRegex(ValueError, 'duplicate JSON'):
            reporter.document(self.index_path)
        self.index_path.write_text('{"schema_version":true}')
        with self.assertRaisesRegex(ValueError, 'schema version'):
            reporter.document(self.index_path)

    def test_external_classification_cannot_hide_graph_candidate(self):
        self.index['components'][0]['boundary'] = 'external'
        with self.assertRaisesRegex(ValueError, 'cannot be excluded'):
            self.collect()
        self.index['components'][0]['graph_refs'] = []
        result = self.collect()
        self.assertEqual(sum(x['code'] == 'graph-candidate-unmapped' for x in result['unresolved']), 2)

    def test_unknown_subcomponent_cycle_and_member_binding(self):
        self.index['components'][0]['parent'] = 'unknown'
        with self.assertRaisesRegex(ValueError, 'parent'):
            self.collect()
        del self.index['components'][0]['parent']
        self.index['components'][0]['members'] = [dict(path='LICENSE', sha256='0' * 64)]
        with self.assertRaisesRegex(ValueError, 'member differs'):
            self.collect()

    def test_unlisted_special_and_oversized_corpus_files_rejected(self):
        if hasattr(os, 'mkfifo'):
            os.mkfifo(self.corpus / 'fifo')
            with self.assertRaisesRegex(ValueError, 'special corpus'):
                self.collect()
            (self.corpus / 'fifo').unlink()
        (self.corpus / 'oversized').write_bytes(b'x' * (reporter.MAX_TEXT + 1))
        with self.assertRaisesRegex(ValueError, 'byte budget'):
            self.collect()

    def test_missing_reviewer_rationale_and_stale_component_review(self):
        self.test_review_binds_component_text_and_all_obligation_categories()
        review = self.index['reviews'][0]
        review['evidence'][0]['sha256'] = self.index['evidence'][0]['sha256']
        del review['reviewer']
        with self.assertRaisesRegex(ValueError, 'reviewer'):
            self.collect()
        review['reviewer'] = 'Fixture reviewer'
        self.index['components'][0]['revision'] = 'changed-revision'
        with self.assertRaisesRegex(ValueError, 'stale component'):
            self.collect()

    def test_atomic_new_output_and_cli_exits(self):
        output = self.root / 'output.json'
        arguments = ['--archive', str(self.archive), '--audit', str(self.audit_path), '--build-evidence', str(self.build_path),
                     '--corpus-index', str(self.index_path), '--corpus-root', str(self.corpus), '--repo', str(REPO), '--output', str(output), '--strict']
        self.assertEqual(reporter.main(arguments), 1)
        before = output.read_bytes()
        self.assertEqual(reporter.main(arguments), 2)
        self.assertEqual(output.read_bytes(), before)
        output.unlink()
        with patch.object(reporter.os, 'link', side_effect=OSError('fixture publication failure')):
            self.assertEqual(reporter.main(arguments), 2)
        self.assertFalse(output.exists())
        self.assertEqual(list(self.root.glob('.release-evidence-*')), [])


if __name__ == '__main__':
    unittest.main()
