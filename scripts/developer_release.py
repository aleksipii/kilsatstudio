#!/usr/bin/env python3
"""Explicit Developer ID/notarization staging; never substitutes ad-hoc signing.

Requires an operator-configured macOS signing identity and notarytool keychain
profile. It does not grant model, artwork, dependency or distribution rights.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import tempfile
import package_macos

CAMERA_ENTITLEMENTS = {"com.apple.security.device.camera": True}


def regular(path: Path, label: str) -> Path:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} must be a regular nonsymlink file")
    return path.absolute()


def validate(identity: str, team: str, profile: str, output: Path, report: Path, consent: bool):
    if not identity.startswith("Developer ID Application: ") or any(c in identity for c in "\r\n\x00"):
        raise ValueError("an explicit Developer ID Application signing identity is required")
    if not re.fullmatch(r"[A-Z0-9]{10}", team) or f"({team})" not in identity:
        raise ValueError("identity must name the requested ten-character Apple team")
    if not profile.strip() or len(profile)>200 or any(c in profile for c in "\r\n\x00"):
        raise ValueError("an existing notarytool keychain profile is required")
    if not consent:
        raise ValueError("--allow-notarization is required before uploading the app to Apple")
    if output.suffix != ".zip" or output.exists() or report.exists() or output.absolute()==report.absolute():
        raise ValueError("ZIP and JSON report must be distinct new output paths")


def run_release(repo: Path, binary: Path, gpu: Path, head: Path, commit: str, identity: str,
                team: str, profile: str, output: Path, report: Path, consent: bool,
                run=subprocess.run):
    validate(identity,team,profile,output,report,consent)
    if not re.fullmatch(r"[0-9a-f]{40}",commit):
        raise ValueError("commit must be an exact lowercase SHA")
    for path,label in [(binary,"desktop binary"),(gpu,"GPU helper"),(head,"camera helper")]:
        regular(path,label)
        if not os.access(path,os.X_OK):raise ValueError(f"{label} must be executable")
    output=output.absolute();report=report.absolute()
    if not output.parent.is_dir() or not report.parent.is_dir():
        raise ValueError("output and report parent directories must exist")
    with tempfile.TemporaryDirectory(prefix="kilsat-developer-release-") as temporary:
        staging=Path(temporary)/"distribution";staging.mkdir()
        bundle=package_macos.stage_distribution(repo,binary,staging,commit,"arm64","release",gpu,head)
        entitlement=Path(temporary)/"camera-entitlements.plist"
        entitlement.write_bytes(plistlib.dumps(CAMERA_ENTITLEMENTS))
        nested=[bundle/"Contents/Resources/bin/gpu_probe",bundle/"Contents/Resources/bin/head_pose"]
        for item in [*nested,bundle]:
            command=["codesign","--force","--sign",identity,"--options","runtime","--timestamp"]
            if item==nested[1] or item==bundle:command += ["--entitlements",str(entitlement)]
            run([*command,str(item)],check=True,capture_output=True,text=True,timeout=120)
            run(["codesign","--verify","--strict",str(item)],check=True,capture_output=True,text=True,timeout=60)
        details=run(["codesign","--display","--verbose=4",str(bundle)],check=True,capture_output=True,text=True,timeout=60)
        detail=details.stdout+details.stderr
        if f"TeamIdentifier={team}" not in detail or "Authority=Developer ID Application:" not in detail:
            raise ValueError("signed app identity/team verification failed")
        submission=Path(temporary)/"submission.zip"
        run(["ditto","-c","-k","--keepParent",str(bundle),str(submission)],check=True,capture_output=True,text=True,timeout=120)
        result=run(["xcrun","notarytool","submit",str(submission),"--keychain-profile",profile,"--wait","--timeout","20m","--output-format","json"],check=True,capture_output=True,text=True,timeout=1260)
        receipt=json.loads(result.stdout)
        if receipt.get("status")!="Accepted" or not receipt.get("id"):
            raise ValueError("Apple notarization was not Accepted; no release ZIP produced")
        run(["xcrun","stapler","staple",str(bundle)],check=True,capture_output=True,text=True,timeout=120)
        run(["xcrun","stapler","validate",str(bundle)],check=True,capture_output=True,text=True,timeout=60)
        run(["codesign","--verify","--deep","--strict",str(bundle)],check=True,capture_output=True,text=True,timeout=60)
        run(["spctl","--assess","--type","execute","--verbose=4",str(bundle)],check=True,capture_output=True,text=True,timeout=120)
        candidate=Path(temporary)/"final.zip"
        run(["ditto","-c","-k","--sequesterRsrc",str(staging),str(candidate)],check=True,capture_output=True,text=True,timeout=120)
        regular(candidate,"final stapled ZIP")
        digest_hash=hashlib.sha256()
        with candidate.open('rb') as source:
            while chunk:=source.read(1024*1024):digest_hash.update(chunk)
        digest=digest_hash.hexdigest()
        evidence={"schema_version":1,"build_commit":commit,"team":team,"signing":"developer_id_hardened_runtime","notarization":receipt,"stapled":True,"archive_sha256":digest,"distribution_approved":False,"physical_mac_acceptance":"not_granted","license_acceptance":"not_granted"}
        # Atomic no-overwrite publication, after every byte-changing signing stage.
        reserved=False
        try:
            with output.open("xb") as target,candidate.open("rb") as source:
                reserved=True
                while chunk:=source.read(1024*1024):target.write(chunk)
            with report.open("x") as target:json.dump(evidence,target,indent=2);target.write("\n")
        except BaseException:
            if reserved:output.unlink(missing_ok=True)
            raise
        return evidence


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo",type=Path,default=Path(__file__).resolve().parents[1])
    for key in ["binary","gpu-probe","head-pose","output","report"]:parser.add_argument("--"+key,type=Path,required=True)
    for key in ["commit","identity","team","notary-profile"]:parser.add_argument("--"+key,required=True)
    parser.add_argument("--allow-notarization",action="store_true")
    args=parser.parse_args(argv)
    if sys.platform!="darwin":parser.error("Developer ID signing/notarization requires an operator-configured Mac")
    try:
        result=run_release(args.repo,args.binary,args.gpu_probe,args.head_pose,args.commit,args.identity,args.team,args.notary_profile,args.output,args.report,args.allow_notarization)
    except (ValueError,OSError,subprocess.SubprocessError,json.JSONDecodeError) as error:
        parser.exit(1,f"Developer release failed: {error}\n")
    print(json.dumps(result));return 0

if __name__=="__main__":raise SystemExit(main())
