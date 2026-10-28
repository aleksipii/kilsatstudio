#!/usr/bin/env python3
"""Explicit pinned Piper-fi test assets outside the repository; no automatic app install."""
import argparse,hashlib,importlib.metadata,json,os
from pathlib import Path
import sys,urllib.request
REVISION='5041a7a49ae5b3a0f30e6837769eb03fdbb231bd'
BASE=f'https://huggingface.co/rhasspy/piper-voices/resolve/{REVISION}/fi/fi_FI/harri/medium/'
FILES={'fi_FI-harri-medium.onnx':'a44167faa34caed940e4fcad139fcc35922266b2593bcebe77701774c0fb2389','fi_FI-harri-medium.onnx.json':'3f9c9f76f74adf1fbe7279e41eea17d6610757e45effd6808bbea6be74b8916d','MODEL_CARD':'a0a7fa3f046f1e582689db76f93099735530343a3126f3f40ed64380c419a0b2'}

def download(directory,piper,consent,opener=urllib.request.urlopen):
    if not consent:raise ValueError('explicit --allow-downloads consent is required')
    repo=Path(__file__).resolve().parents[1];directory=Path(directory).absolute();piper=Path(piper).absolute()
    if directory.resolve().is_relative_to(repo) or directory.exists():raise ValueError('voice directory must be new and outside the Git repository')
    if piper.is_symlink() or not piper.is_file() or not os.access(piper,os.X_OK):raise ValueError('select an installed regular Piper executable')
    directory.mkdir(parents=True)
    evidence={}
    try:
        for name,expected in FILES.items():
            target=directory/name;digest=hashlib.sha256();total=0
            with opener(BASE+name,timeout=90) as source,target.open('xb') as out:
                while chunk:=source.read(65536):
                    total+=len(chunk)
                    if total>128*1024*1024:raise ValueError('voice asset exceeds download budget')
                    digest.update(chunk);out.write(chunk)
            if digest.hexdigest()!=expected:raise ValueError(f'{name} SHA-256 mismatch')
            evidence[name]={'bytes':total,'sha256':expected,'url':BASE+name}
        versions={}
        for name in ['piper-tts','onnxruntime']:
            try:versions[name]=importlib.metadata.version(name)
            except importlib.metadata.PackageNotFoundError:versions[name]='not_observed_in_this_interpreter'
        profile={'executable':str(piper),'model_path':str(directory/'fi_FI-harri-medium.onnx'),'config_path':str(directory/'fi_FI-harri-medium.onnx.json'),'cache_directory':str(directory/'cache'),'preset':'piper_harri','model_version':'hf:'+REVISION,'environment_identity':json.dumps({'observed_package_versions':versions,'python':sys.version,'dependency_closure_verified':False},sort_keys=True),'timeout_seconds':60}
        (directory/'profile.json').write_text(json.dumps(profile,indent=2)+'\n')
        (directory/'download-evidence.json').write_text(json.dumps({'schema_version':1,'revision':REVISION,'files':evidence,'versions':versions,'weights_bundled':False,'dependency_closure_verified':False,'publication_approved':False},indent=2)+'\n')
        return directory/'profile.json'
    except BaseException:
        for name in [*FILES,'profile.json','download-evidence.json']:(directory/name).unlink(missing_ok=True)
        directory.rmdir();raise

def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--directory',type=Path,required=True);parser.add_argument('--piper',type=Path,required=True);parser.add_argument('--allow-downloads',action='store_true');args=parser.parse_args(argv)
    try:profile=download(args.directory,args.piper,args.allow_downloads)
    except (OSError,ValueError) as error:parser.exit(1,f'Finnish asset preparation failed: {error}\n')
    print(profile);return 0
if __name__=='__main__':raise SystemExit(main())
