#!/bin/sh
# Build the shared Rust animation core (crates/motion-ffi) for iPad devices and
# the iPad simulator, and wrap it as KoetaCore.xcframework for the iPad app.
#
# Usage: scripts/build_ipad_core.sh [debug|release]
# Writes apps/studio-ipad/Build/KoetaCore.xcframework (not committed).
# Minimum iPadOS 16: every iPad that runs it has a 64-bit Apple chip.
set -eu

profile=${1:-release}
case "$profile" in
    release) cargo_profile=--release ;;
    debug) cargo_profile= ;;
    *) echo "profile must be debug or release: $profile" >&2; exit 2 ;;
esac
repo=$(cd "$(dirname "$0")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-"$repo/target"}
output="$repo/apps/studio-ipad/Build"
export IPHONEOS_DEPLOYMENT_TARGET=16.0

for triple in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo build --manifest-path "$repo/Cargo.toml" --locked $cargo_profile -p motion-ffi --lib --target "$triple"
done

headers="$output/include"
rm -rf "$output/KoetaCore.xcframework" "$headers"
mkdir -p "$headers"
cp "$repo/crates/motion-ffi/include/koeta.h" "$headers/"
cat > "$headers/module.modulemap" <<'EOF'
module KoetaCore {
    header "koeta.h"
    link "motion_ffi"
    export *
}
EOF
xcodebuild -create-xcframework \
    -library "$target_dir/aarch64-apple-ios/$profile/libmotion_ffi.a" -headers "$headers" \
    -library "$target_dir/aarch64-apple-ios-sim/$profile/libmotion_ffi.a" -headers "$headers" \
    -output "$output/KoetaCore.xcframework"
echo "$output/KoetaCore.xcframework"
