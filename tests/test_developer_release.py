import importlib.util
from pathlib import Path
import sys,tempfile,unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import developer_release as release

class DeveloperReleaseTests(unittest.TestCase):
 def validate(self,root,**kw):
  args=dict(identity='Developer ID Application: Example (ABC1234567)',team='ABC1234567',profile='operator-profile',output=root/'release.zip',report=root/'report.json',consent=True);args.update(kw);return release.validate(**args)
 def test_explicit_identity_team_consent_and_new_outputs_are_required(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);self.validate(root)
   for change in [dict(identity='-'),dict(team='OTHERTEAM0'),dict(profile=''),dict(consent=False),dict(output=root/'x.txt')]:
    with self.assertRaises(ValueError):self.validate(root,**change)
   (root/'release.zip').write_text('existing')
   with self.assertRaises(ValueError):self.validate(root)
   self.assertEqual((root/'release.zip').read_text(),'existing')
 def test_symlink_input_cannot_be_a_signing_candidate(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);(root/'binary').write_text('source');(root/'link').symlink_to(root/'binary')
   with self.assertRaises(ValueError):release.regular(root/'link','binary')
 def test_platform_refuses_before_any_sign_or_notarization_command(self):
  with patch.object(release.sys,'platform','linux'),patch.object(release.subprocess,'run') as run:
   with self.assertRaises(SystemExit):release.main(['--binary','b','--gpu-probe','g','--head-pose','h','--output','out.zip','--report','r.json','--commit','a'*40,'--identity','Developer ID Application: Example (ABC1234567)','--team','ABC1234567','--notary-profile','p','--allow-notarization'])
   run.assert_not_called()
