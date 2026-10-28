import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('generate_kilsat_english', ROOT / 'scripts/generate_kilsat_english.py')
english = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(english)


class EnglishSourceTests(unittest.TestCase):
    def test_generated_projects_match_checked_sources_and_keep_finnish_prop(self):
        source = ROOT / 'examples/kilsat-english'
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary)
            manifest = english.generate(destination, source / 'stories.json')
            self.assertEqual(manifest['language'], 'en-us')
            self.assertFalse(manifest['publishable'])
            self.assertTrue(all(value is False for value in manifest['acceptance'].values()))
            for path in destination.rglob('*'):
                if path.is_file():
                    self.assertEqual(path.read_bytes(), (source / path.relative_to(destination)).read_bytes())
            for episode in manifest['episodes']:
                project = json.loads((destination / episode['project']).read_text())
                self.assertEqual(hashlib.sha256(project['source'].encode()).hexdigest(), episode['source_sha256'])
                script = json.loads(project['source'])
                self.assertEqual((script['project']['width'], script['project']['height'], script['project']['fps']), (1920, 1080, 24))
                self.assertTrue(script['project']['captions']['enabled'])
                self.assertEqual(len(script['scenes']), 10)
                for actor, preset in english.VOICES.items():
                    voice = script['characters'][actor]['voice']
                    self.assertEqual((voice['provider'], voice['language'], voice['preset']), ('kokoro', 'en-us', preset))
            self.assertEqual(english.original.example_screen(), (ROOT / 'examples/kilsat-season/assets/kilsat-example.png').read_bytes())
            self.assertNotEqual(english.original.example_screen('en-us'), english.original.example_screen())

    def test_wrong_language_membership_and_missing_kilsat_are_rejected(self):
        catalog = json.loads((ROOT / 'examples/kilsat-english/stories.json').read_text())
        for change in ('language', 'missing', 'kilsat'):
            invalid = copy.deepcopy(catalog)
            if change == 'language':
                invalid['language'] = 'fi-fi'
            elif change == 'missing':
                invalid['episodes'].pop()
            else:
                invalid['episodes'][0]['turns'][4] = 'An unrelated insert would misrepresent the pictures.'
            with self.assertRaises(ValueError):
                english.validate_catalog(invalid)


if __name__ == '__main__':
    unittest.main()
