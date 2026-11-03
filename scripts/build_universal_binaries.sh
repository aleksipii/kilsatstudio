#!/bin/sh
# Build the desktop app and its helpers as universal (arm64 + x86_64) executables,
# so one package runs on Apple Silicon and Intel Macs with macOS 13 or newer.
#
# Usage: scripts/build_universal_binaries.sh OUTPUT_DIRECTORY [debug|release]
# Writes OUTPUT_DIRECTORY/studio-desktop, gpu_probe and head_pose. Each one is
# checked to contain exactly both slices and to link only system libraries.
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 OUTPUT_DIRECTORY [debug|release]" >&2
    exit 2
fi
output=$1
profile=${2:-release}
case "$profile" in
    release) cargo_profile=--release ;;
    debug) cargo_profile= ;;
    *) echo "profile must be debug or release: $profile" >&2; exit 2 ;;
esac
repo=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
minimum=13.0
export MACOSX_DEPLOYMENT_TARGET=$minimum
mkdir -p "$output"

for triple in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --manifest-path "$repo/Cargo.toml" --locked $cargo_profile -p studio-desktop --target "$triple"
    cargo build --manifest-path "$repo/Cargo.toml" --locked $cargo_profile -p motion-render --features gpu \
        --example gpu_probe --target "$triple"
done
for swift_arch in arm64 x86_64; do
    swiftc -swift-version 5 -O -target "$swift_arch-apple-macos$minimum" "$repo/services/head-pose/main.swift" \
        -o "$output/head_pose-$swift_arch" -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
        -Xlinker "$repo/services/head-pose/Info.plist"
done

lipo -create "$target_dir/aarch64-apple-darwin/$profile/studio-desktop" \
    "$target_dir/x86_64-apple-darwin/$profile/studio-desktop" -output "$output/studio-desktop"
lipo -create "$target_dir/aarch64-apple-darwin/$profile/examples/gpu_probe" \
    "$target_dir/x86_64-apple-darwin/$profile/examples/gpu_probe" -output "$output/gpu_probe"
lipo -create "$output/head_pose-arm64" "$output/head_pose-x86_64" -output "$output/head_pose"
rm "$output/head_pose-arm64" "$output/head_pose-x86_64"

for executable in "$output/studio-desktop" "$output/gpu_probe" "$output/head_pose"; do
    archs=$(lipo -archs "$executable")
    test "$archs" = "x86_64 arm64" || { echo "expected x86_64 and arm64 in $executable, got: $archs" >&2; exit 1; }
    for arch in x86_64 arm64; do
        otool -arch "$arch" -L "$executable" | tail -n +2 | while read -r library _; do
            case "$library" in
                /System/Library/*|/usr/lib/*) ;;
                *) echo "unexpected non-system dependency in $executable ($arch): $library" >&2; exit 1 ;;
            esac
        done
    done
done
echo "$output"
