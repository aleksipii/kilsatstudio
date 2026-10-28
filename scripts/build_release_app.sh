#!/bin/sh
# Build, package and audit the Kilsat Studio release app for Apple Silicon.
#
# Usage: scripts/build_release_app.sh OUTPUT_DIRECTORY
# Produces OUTPUT_DIRECTORY/Kilsat-Studio-1.0.0-arm64.zip and an audit report.
# The app is ad-hoc signed; Developer ID signing and notarization are a
# separate, explicit step (scripts/developer_release.py, docs/mac-production-release.md).
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 OUTPUT_DIRECTORY" >&2
    exit 2
fi
repo=$(cd "$(dirname "$0")/.." && pwd)
output=$1
if [ -e "$output" ]; then
    echo "output directory must not exist: $output" >&2
    exit 2
fi
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
triple=aarch64-apple-darwin
commit=$(git -C "$repo" rev-parse HEAD)
version=$(python3 -c "import importlib.util,sys;s=importlib.util.spec_from_file_location('p',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);print(m.VERSION)" "$repo/scripts/package_macos.py")

export MACOSX_DEPLOYMENT_TARGET=13.0
cargo build --manifest-path "$repo/Cargo.toml" --locked --release -p studio-desktop --target "$triple"
cargo build --manifest-path "$repo/Cargo.toml" --locked --release -p motion-render --features gpu --example gpu_probe --target "$triple"

mkdir -p "$output/build"
swiftc -swift-version 5 -O -target arm64-apple-macos13.0 "$repo/services/head-pose/main.swift" \
    -o "$output/build/head_pose" -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
    -Xlinker "$repo/services/head-pose/Info.plist"

binary="$output/build/studio-desktop"
helper="$output/build/gpu_probe"
cp "$target_dir/$triple/release/studio-desktop" "$binary"
cp "$target_dir/$triple/release/examples/gpu_probe" "$helper"
for executable in "$binary" "$helper"; do
    test "$(lipo -archs "$executable")" = arm64
    otool -L "$executable" | tail -n +2 | while read -r library _; do
        case "$library" in
            /System/Library/*|/usr/lib/*) ;;
            *) echo "unexpected non-system dependency in $executable: $library" >&2; exit 1 ;;
        esac
    done
    strip -x "$executable"
done

archive="$output/Kilsat-Studio-$version-arm64.zip"
python3 "$repo/scripts/package_macos.py" --repo "$repo" --binary "$binary" --gpu-probe "$helper" \
    --head-pose "$output/build/head_pose" --output "$archive" --commit "$commit" --arch arm64 --profile release
python3 "$repo/scripts/package_audit.py" --archive "$archive" --repo "$repo" --commit "$commit" \
    --target "$triple" --profile release --output "$output/package-audit.json" --require-signature-metadata
shasum -a 256 "$archive" > "$archive.sha256"
echo "$archive"
