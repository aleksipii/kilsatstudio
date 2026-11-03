#!/bin/sh
# Build, package and audit the Koeta Studio release app for every Mac with
# macOS 13 or newer: one universal app for Apple Silicon and Intel.
#
# Usage: scripts/build_release_app.sh OUTPUT_DIRECTORY [SPEECH_BUNDLE_DIRECTORY]
# Produces OUTPUT_DIRECTORY/Koeta-Studio-1.0.0-universal.zip and an audit report.
# With SPEECH_BUNDLE_DIRECTORY (built by scripts/build_speech_bundle.py) the app
# ships Kokoro, Piper and Rhubarb ready to use, with no download or setup.
# The app is ad-hoc signed; Developer ID signing and notarization are a
# separate, explicit step (scripts/developer_release.py, docs/mac-production-release.md).
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 OUTPUT_DIRECTORY [SPEECH_BUNDLE_DIRECTORY]" >&2
    exit 2
fi
speech_args=
if [ "$#" -eq 2 ]; then
    speech=$(cd "$2" && pwd)
    test -f "$speech/speech-bundle.json" || { echo "not a speech bundle: $2" >&2; exit 2; }
    speech_args="--speech-bundle $speech"
fi
repo=$(cd "$(dirname "$0")/.." && pwd)
output=$1
if [ -e "$output" ]; then
    echo "output directory must not exist: $output" >&2
    exit 2
fi
commit=$(git -C "$repo" rev-parse HEAD)
version=$(python3 -c "import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.VERSION)" "$repo/scripts/package_macos.py")

mkdir -p "$output"
"$repo/scripts/build_universal_binaries.sh" "$output/build" release
binary="$output/build/studio-desktop"
helper="$output/build/gpu_probe"
for executable in "$binary" "$helper"; do
    strip -x "$executable"
done

archive="$output/Koeta-Studio-$version-universal.zip"
python3 "$repo/scripts/package_macos.py" --repo "$repo" --binary "$binary" --gpu-probe "$helper" \
    --head-pose "$output/build/head_pose" --output "$archive" --commit "$commit" --arch universal --profile release \
    ${speech_args:+--speech-bundle "$speech"}
python3 "$repo/scripts/package_audit.py" --archive "$archive" --repo "$repo" --commit "$commit" \
    --target universal-apple-darwin --profile release --output "$output/package-audit.json" --require-signature-metadata \
    ${speech_args:+--speech-bundle "$speech"}
shasum -a 256 "$archive" > "$archive.sha256"
echo "$archive"
