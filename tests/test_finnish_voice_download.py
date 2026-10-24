import importlib.util,io,tempfile,unittest
from pathlib import Path
from unittest.mock import Mock,patch
PATH=Path(__file__).resolve().parents[1]/'tools/prepare_finnish_test_voice.py'
spec=importlib.util.spec_from_file_location('prepare_finnish_test_voice',PATH);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class VoiceDownloadTests(unittest.TestCase):
 def test_no_consent_means_no_network_or_files(self):
  opener=Mock()
  with tempfile.TemporaryDirectory(dir=PATH.parents[2]) as d:
   path=Path(d)/'voice'
   with self.assertRaises(ValueError):module.download(path,Path(d)/'piper',False,opener)
   self.assertFalse(path.exists());opener.assert_not_called()
 def test_hash_mismatch_cleans_only_new_voice_directory(self):
  with tempfile.TemporaryDirectory(dir=PATH.parents[2]) as d:
   root=Path(d);piper=root/'piper';piper.write_text('fixture');piper.chmod(0o755)
   with self.assertRaisesRegex(ValueError,'SHA-256 mismatch'):module.download(root/'voice',piper,True,lambda *a,**kw:io.BytesIO(b'wrong model'))
   self.assertFalse((root/'voice').exists());self.assertEqual(piper.read_text(),'fixture')
 def test_existing_directories_and_symlink_executables_are_not_replaced(self):
  with tempfile.TemporaryDirectory(dir=PATH.parents[2]) as d:
   root=Path(d);piper=root/'piper';piper.write_text('fixture');piper.chmod(0o755);link=root/'link';link.symlink_to(piper)
   with self.assertRaises(ValueError):module.download(root,piper,True)
   with self.assertRaises(ValueError):module.download(root/'voice',link,True)
   self.assertFalse((root/'voice').exists())
