import copy
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest import mock
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('build_stick_studio_pack', ROOT/'scripts/build_stick_studio_pack.py')
with mock.patch.object(sys,'path',[str(ROOT/'scripts'),*sys.path]):
    packer = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(packer)


def write(path, value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value),encoding='utf-8')


def fixture(root):
    source = root/'source'
    source.mkdir()
    (source/'README.md').write_text('Fixture-only guide',encoding='utf-8')
    catalog = {'schema_version':1, 'items':[
        {'id':identifier, 'name':identifier, 'width':12, 'height':20, 'grip':[0,0],
         'parts':[{'id':'case','joint':'grip','points':[[-6,-20],[6,-20],[6,0],[-6,0]],'color':[40,40,40,255]}],
         'grip_parts':[{'id':'finger','joint':'grip','ellipse':{'cx':0,'cy':0,'rx':4,'ry':3},'color':[40,40,40,255]}]}
        for identifier in packer.HAND_PROP_IDS]}
    write(source/'hand-props.json',catalog)
    roadmap = root/'roadmap.md'
    roadmap.write_text('Fixture-only acceptance gates',encoding='utf-8')
    starter = {'format_version':2,'assets':[],
               'source':json.dumps({'project':{'width':1920,'height':1080,'fps':24},
                                    'characters':{str(i):{} for i in range(6)},
                                    'scenes':[{'id':str(i),'duration':5,'events':[]} for i in range(6)]})}
    write(source/'stick-studio.kilsat',starter)
    manifest = {'schema_version':1,'pack_revision':2,'artistic_review':False,'physical_mac_review':False,
                'hand_props_catalog':'hand-props.json','hand_props_sha256':packer.file_hash(source/'hand-props.json'),
                'starter_project':'stick-studio.kilsat','starter_sha256':packer.file_hash(source/'stick-studio.kilsat'),
                'starter_source_bytes':len(starter['source'].encode()),'items':[]}
    for kind,count in packer.COUNTS.items():
        for number in range(count):
            identifier = f'{kind}_{number}'
            relative = f'{packer.DIRECTORIES[kind]}/{identifier}.kilsat'
            project = {'format_version':2,'assets':[],
                       'source':json.dumps({'project':{'width':512,'height':512},'scenes':[{'events':[]}]})}
            write(source/relative,project)
            manifest['items'].append({'id':identifier,'kind':kind,'project':relative,
                                      'transparent':kind!='background','export_width':512,'export_height':512,
                                      'project_sha256':packer.file_hash(source/relative)})
    write(source/'manifest.json',manifest)
    return source,roadmap,manifest


def rewrite_project(source, manifest, item, change):
    path = source/item['project']
    document = json.loads(path.read_text())
    screenplay = json.loads(document['source'])
    change(document,screenplay)
    document['source'] = json.dumps(screenplay)
    write(path,document)
    item['project_sha256'] = packer.file_hash(path)
    write(source/'manifest.json',manifest)


class StickPackTests(unittest.TestCase):
    def test_archive_publishes_completed_outputs_without_renderer_staging_trees(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root/'delivery'
            output.mkdir()
            for directory in ('images', 'video', 'projects', 'logs'):
                (output/directory).mkdir()
            for name in ('LICENSE', 'NOTICE', 'ALOITA-TAALTA.md', 'professional-studio-next.md',
                         'pack-report.json', 'render-inputs.json', 'HAHMOT-JA-ESINEET.png'):
                (output/name).write_bytes(b'checked fixture deliverable')
            (output/'video/sequence.mp4').write_bytes(b'completed and independently checked movie fixture')
            (output/'.kilsat-stream-6-1').mkdir()
            (output/'.kilsat-stream-6-1/sequence.mp4').write_bytes(b'incomplete renderer staging fixture')
            (output/'.kilsat-pipeline-input-9-1').mkdir()
            (output/'.kilsat-pipeline-input-9-1/source.kilsat').write_bytes(b'internal source snapshot fixture')
            (output/'video/.kilsat-stream-nested').mkdir()
            (output/'video/.kilsat-stream-nested/partial.mp4').write_bytes(b'incomplete nested fixture')
            archive = packer.archive_pack(output)
            with zipfile.ZipFile(archive) as delivery:
                self.assertEqual(delivery.read('video/sequence.mp4'),
                                 b'completed and independently checked movie fixture')
                self.assertEqual(len(delivery.namelist()),8)
                self.assertTrue(all(not any(part.startswith('.kilsat-') for part in Path(name).parts)
                                    for name in delivery.namelist()))
            archive.unlink()
            (output/'unvalidated-extra.mp4').write_bytes(b'unknown file must not be published')
            with self.assertRaisesRegex(ValueError,'unexpected root'):
                packer.archive_pack(output)
            self.assertFalse(archive.exists())

    def test_archive_rejects_wrong_root_kinds_and_symlinks_in_published_trees(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            output = root/'delivery'
            output.mkdir()
            for directory in ('images', 'video', 'projects', 'logs'):
                (output/directory).mkdir()
            for name in ('LICENSE', 'NOTICE', 'ALOITA-TAALTA.md', 'professional-studio-next.md',
                         'pack-report.json', 'render-inputs.json'):
                (output/name).write_bytes(b'checked fixture')
            (output/'video').rmdir()
            (output/'video').write_bytes(b'wrong root kind')
            with self.assertRaisesRegex(ValueError,'directory has the wrong'):
                packer.archive_pack(output)
            (output/'video').unlink()
            (output/'video').mkdir()
            (output/'LICENSE').unlink()
            (output/'LICENSE').mkdir()
            with self.assertRaisesRegex(ValueError,'file has the wrong'):
                packer.archive_pack(output)
            (output/'LICENSE').rmdir()
            (output/'LICENSE').write_bytes(b'checked fixture')
            (output/'video/link').symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError,'symbolic'):
                packer.archive_pack(output)
            self.assertFalse((root/'Kilsat-Tikku-ukkostudio.zip').exists())

    def test_all_sources_and_guides_are_hash_bound_without_quality_approval(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,manifest = fixture(Path(temporary).resolve())
            actual,identity = packer.source_identity(source,roadmap)
            self.assertEqual(actual,manifest)
            self.assertEqual(len(identity['source_sha256']),34)
            self.assertEqual(identity['pack_revision'],2)
            self.assertFalse(identity['artistic_review'])
            self.assertFalse(identity['physical_mac_review'])
            self.assertFalse(identity['publication_approved'])
            before = copy.deepcopy(identity)
            roadmap.write_text('Changed acceptance gates')
            self.assertNotEqual(before,packer.source_identity(source,roadmap)[1])
            (source/manifest['items'][0]['project']).write_text('changed geometry')
            with self.assertRaisesRegex(ValueError,'project changed'):
                packer.source_identity(source,roadmap)

    def test_manifest_rejects_future_schema_missing_members_and_false_approval(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,original = fixture(Path(temporary).resolve())
            for change in ('version','counts','approval','mac'):
                manifest = copy.deepcopy(original)
                if change == 'version': manifest['schema_version'] = 2
                if change == 'counts': manifest['items'].pop()
                if change == 'approval': manifest['artistic_review'] = True
                if change == 'mac': manifest['physical_mac_review'] = True
                write(source/'manifest.json',manifest)
                with self.assertRaises(ValueError):
                    packer.source_identity(source,roadmap)

    def test_pack_revision_must_explicitly_select_current_geometry(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,original = fixture(Path(temporary).resolve())
            for revision in (None, 0, 1, 3, True, '2'):
                manifest = copy.deepcopy(original)
                if revision is None:
                    manifest.pop('pack_revision')
                else:
                    manifest['pack_revision'] = revision
                write(source/'manifest.json',manifest)
                with self.assertRaisesRegex(ValueError,'revision-two'):
                    packer.source_identity(source,roadmap)

    def test_hand_catalog_bytes_are_hash_bound_and_its_path_is_fixed(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,manifest = fixture(Path(temporary).resolve())
            catalog_path = source/'hand-props.json'
            catalog = json.loads(catalog_path.read_text())
            catalog['items'][0]['parts'][0]['points'][0][0] = -7
            write(catalog_path,catalog)
            with self.assertRaisesRegex(ValueError,'catalog changed'):
                packer.source_identity(source,roadmap)
            manifest['hand_props_sha256'] = packer.file_hash(catalog_path)
            write(source/'manifest.json',manifest)
            self.assertIn('hand-props.json',packer.source_identity(source,roadmap)[1]['source_sha256'])
            manifest['hand_props_catalog'] = '../outside.json'
            write(source/'manifest.json',manifest)
            with self.assertRaisesRegex(ValueError,'catalog path'):
                packer.source_identity(source,roadmap)

    def test_rehashed_hand_catalog_rejects_external_and_unbounded_geometry(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,manifest = fixture(Path(temporary).resolve())
            path = source/'hand-props.json'
            original = json.loads(path.read_text())
            for invalid in ('future','ids','texture','skin','variants','smooth','ellipse','nan','vertices','grip'):
                catalog = copy.deepcopy(original)
                part = catalog['items'][0]['parts'][0]
                if invalid == 'future': catalog['schema_version'] = 2
                if invalid == 'ids': catalog['items'][0]['id'] = 'unlisted'
                if invalid == 'texture': part['texture'] = {'asset':'external.png'}
                if invalid == 'skin': part['skin'] = []
                if invalid == 'variants': part['variants'] = {}
                if invalid == 'smooth': part['smooth'] = 16
                if invalid == 'ellipse': part['ellipse'] = {'cx':0,'cy':0,'rx':0,'ry':3}
                if invalid == 'nan': part['points'][0][0] = float('nan')
                if invalid == 'vertices': part['points'] *= 100
                if invalid == 'grip': catalog['items'][0]['grip'] = [1,0]
                write(path,catalog)
                manifest['hand_props_sha256'] = packer.file_hash(path)
                write(source/'manifest.json',manifest)
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    packer.source_identity(source,roadmap)

    def test_identifiers_paths_dimensions_and_transparency_cannot_escape_the_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,original = fixture(Path(temporary).resolve())
            for change in ('traversal','absolute','duplicate','boolean','alpha'):
                manifest = copy.deepcopy(original)
                item = manifest['items'][0]
                if change == 'traversal': item['project'] = '../outside.kilsat'
                if change == 'absolute': item['project'] = '/outside.kilsat'
                if change == 'duplicate': item['id'] = manifest['items'][1]['id']
                if change == 'boolean': item['export_width'] = True
                if change == 'alpha': item['transparent'] = False
                write(source/'manifest.json',manifest)
                with self.assertRaises(ValueError):
                    packer.source_identity(source,roadmap)
            for relative in ('../outside','/absolute','characters//a','characters/./a','characters\\a'):
                with self.assertRaises(ValueError): packer.pack_path(source,relative)

    def test_symlink_source_parent_and_output_are_rejected_before_resolution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source,roadmap,manifest = fixture(root)
            path = source/manifest['items'][0]['project']
            path.unlink()
            path.symlink_to(roadmap)
            with self.assertRaisesRegex(ValueError,'symbolic'):
                packer.source_identity(source,roadmap)
            (root/'linked').symlink_to(root,target_is_directory=True)
            with self.assertRaisesRegex(ValueError,'symbolic'):
                packer.safe_path(root/'linked/new-output')

    def test_speech_external_assets_and_audio_cues_need_an_explicit_different_pipeline(self):
        for kind in ('speech','asset','cue','timing'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                source,roadmap,manifest = fixture(Path(temporary).resolve())
                def change(document,script):
                    if kind == 'speech': script['scenes'][0]['events'] = [{'type':'speak'}]
                    if kind == 'cue': script['scenes'][0]['composition'] = {'audio_cues':[{'asset':'tone.wav'}]}
                    if kind == 'asset': document['assets'] = [{'path':'unlisted.png'}]
                    if kind == 'timing': document['timing'] = {'line':'{}'}
                rewrite_project(source,manifest,manifest['items'][0],change)
                with self.assertRaises(ValueError): packer.source_identity(source,roadmap)

    def test_starter_duration_and_character_membership_are_checked_after_rehash(self):
        with tempfile.TemporaryDirectory() as temporary:
            source,roadmap,manifest = fixture(Path(temporary).resolve())
            path = source/'stick-studio.kilsat'
            original = json.loads(path.read_text())
            for change in ('duration','cast','profile'):
                document = copy.deepcopy(original)
                script = json.loads(document['source'])
                if change == 'duration': script['scenes'][0]['duration'] = 6
                if change == 'cast': script['characters'].pop('0')
                if change == 'profile': script['project']['fps'] = 30
                document['source'] = json.dumps(script)
                write(path,document)
                manifest['starter_sha256'] = packer.file_hash(path)
                manifest['starter_source_bytes'] = len(document['source'].encode())
                write(source/'manifest.json',manifest)
                with self.assertRaisesRegex(ValueError,'six characters'):
                    packer.source_identity(source,roadmap)

    def test_tool_identity_uses_resolved_bytes_and_detects_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            actual = root/'actual-tool'
            actual.write_bytes(b'fixture-only executable bytes')
            actual.chmod(0o700)
            alias = root/'tool'
            alias.symlink_to(actual)
            before = packer.tool_identity({'ffmpeg':alias})
            self.assertEqual(before['ffmpeg']['path'],str(actual))
            actual.write_bytes(b'changed fixture bytes')
            self.assertNotEqual(before,packer.tool_identity({'ffmpeg':alias}))
            with self.assertRaises(ValueError): packer.tool_identity({'ffmpeg':Path('relative')})

    def test_optional_catalog_has_a_hash_bound_png_header_and_bounded_dimensions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = root/'catalog.png'
            def header(width,height):
                return b'\x89PNG\r\n\x1a\n'+b'\0\0\0\rIHDR'+struct.pack('>II',width,height)
            # This fixture tests header/path boundaries only, not image rendering.
            path.write_bytes(header(1920,1080))
            first = packer.catalog_identity(path)
            self.assertEqual((first['width'],first['height']),(1920,1080))
            path.write_bytes(header(1920,1080)+b'changed-fixture')
            self.assertNotEqual(first['sha256'],packer.catalog_identity(path)['sha256'])
            for dimensions in ((0,100),(100,0),(4097,100),(100,4097)):
                path.write_bytes(header(*dimensions))
                with self.assertRaises(ValueError): packer.catalog_identity(path)
            path.write_bytes(b'not a PNG')
            with self.assertRaisesRegex(ValueError,'PNG'): packer.catalog_identity(path)
            with self.assertRaisesRegex(ValueError,'absolute'): packer.catalog_identity(Path('relative.png'))
            linked = root/'linked.png'
            linked.symlink_to(path)
            with self.assertRaisesRegex(ValueError,'symbolic'): packer.catalog_identity(linked)

    def test_video_receipt_and_probe_must_retain_full_hd_frame_and_sample_clocks(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary).resolve()
            pipeline = {'schema_version':1,'status':'completed','publication_approved':False,
                        'dialogue_audio':[],'private_system_preview_used':False,'review':False,
                        'streaming':{'width':1920,'height':1080,'fps':24,'frame_count':720,'total_samples':720_000}}
            write(directory/'pipeline-report.json',pipeline)
            with mock.patch.object(packer,'probe',return_value={'streams':[],'format':{'duration':'30'}}):
                with self.assertRaisesRegex(ValueError,'exactly one actual video'):
                    packer.validate_video(directory,Path('/ffmpeg'),Path('/ffprobe'),directory)
            for change in ('speech','frame','sample','review'):
                altered = copy.deepcopy(pipeline)
                if change == 'speech': altered['dialogue_audio'] = [{'text':'unrequested'}]
                if change == 'frame': altered['streaming']['frame_count'] = 719
                if change == 'sample': altered['streaming']['total_samples'] -= 1
                if change == 'review': altered['review'] = True
                write(directory/'pipeline-report.json',altered)
                with self.assertRaisesRegex(ValueError,'exact silent'):
                    packer.validate_video(directory,Path('/ffmpeg'),Path('/ffprobe'),directory)


if __name__ == '__main__':
    unittest.main()
