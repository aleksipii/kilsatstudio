#!/bin/sh
# Build a self-contained FFmpeg (ffmpeg + ffprobe) for Contents/Resources/ffmpeg.
#
# Static FFmpeg 9.0.2 with libx264 (H.264), the native AAC and ProRes encoders
# and Apple VideoToolbox/AudioToolbox. It links only macOS system libraries, so
# it runs on any Apple Silicon Mac with macOS 13 or newer without Homebrew.
#
# Usage: scripts/build_ffmpeg_bundle.sh OUTPUT_DIRECTORY [DOWNLOAD_CACHE]
# Needs: Xcode command-line tools, pkg-config (brew install pkgconf), network
# access for the two pinned source archives (or a cache that holds them).
#
# Licence: the result is GPL-2.0-or-later (FFmpeg with --enable-gpl and x264).
# The licence texts, exact sources and configure lines are written next to the
# binaries; distributing the app publicly also requires offering these sources.
set -eu

FFMPEG_VERSION=9.0.2
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
# Same SHA-256 as Homebrew's ffmpeg formula for this release.
FFMPEG_SHA256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
# x264 stable branch, the revision Homebrew pins as r3222.
X264_COMMIT=b35605ace3ddf7c1a5d67a2eb553f034aef41d55
X264_URL="https://code.videolan.org/videolan/x264/-/archive/$X264_COMMIT/x264-$X264_COMMIT.tar.bz2"
X264_SHA256=6eeb82934e69fd51e043bd8c5b0d152839638d1ce7aa4eea65a3fedcf83ff224
MIN_MACOS=13.0

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 OUTPUT_DIRECTORY [DOWNLOAD_CACHE]" >&2
    exit 2
fi
case "$1" in /*) ;; *) echo "OUTPUT_DIRECTORY must be absolute" >&2; exit 2 ;; esac
output=$1
if [ -e "$output" ]; then
    echo "output directory must not exist: $output" >&2
    exit 2
fi
if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
    echo "build on an Apple Silicon Mac (the bundle is arm64)" >&2
    exit 2
fi
command -v pkg-config >/dev/null || { echo "pkg-config is required (brew install pkgconf)" >&2; exit 2; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cache=${2:-$work/cache}
mkdir -p "$cache"

fetch() { # url sha256 file
    if [ ! -f "$cache/$3" ]; then
        curl --proto '=https' --tlsv1.2 -fsSL -o "$cache/$3.part" "$1"
        mv "$cache/$3.part" "$cache/$3"
    fi
    echo "$2  $cache/$3" | shasum -a 256 -c - >/dev/null || {
        echo "checksum mismatch: $3" >&2
        exit 1
    }
}
fetch "$FFMPEG_URL" "$FFMPEG_SHA256" "ffmpeg-$FFMPEG_VERSION.tar.xz"
fetch "$X264_URL" "$X264_SHA256" "x264-$X264_COMMIT.tar.bz2"

prefix=$work/prefix
jobs=$(sysctl -n hw.ncpu)
export MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"

tar -xjf "$cache/x264-$X264_COMMIT.tar.bz2" -C "$work"
x264_configure="--prefix=$prefix --enable-static --disable-cli --disable-opencl --enable-pic"
(cd "$work/x264-$X264_COMMIT" && ./configure $x264_configure >/dev/null && make -j"$jobs" >/dev/null && make install >/dev/null)

tar -xJf "$cache/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$work"
ffmpeg_configure="--prefix=$prefix --pkg-config-flags=--static --enable-gpl --enable-libx264 \
--disable-shared --enable-static --disable-autodetect --enable-videotoolbox --enable-audiotoolbox \
--disable-ffplay --disable-doc --disable-network --disable-debug \
--extra-cflags=-mmacosx-version-min=$MIN_MACOS --extra-ldflags=-mmacosx-version-min=$MIN_MACOS"
(cd "$work/ffmpeg-$FFMPEG_VERSION" && ./configure $ffmpeg_configure >"$work/configure.log" 2>&1 \
    || { tail -30 "$work/configure.log" >&2; exit 1; })
(cd "$work/ffmpeg-$FFMPEG_VERSION" && make -j"$jobs" >/dev/null && make install >/dev/null)

mkdir -p "$output/bin" "$output/licenses"
for tool in ffmpeg ffprobe; do
    cp "$prefix/bin/$tool" "$output/bin/$tool"
    strip -x "$output/bin/$tool"
    # Refuse anything that links outside the operating system.
    if otool -L "$output/bin/$tool" | tail -n +2 | awk '{print $1}' \
        | grep -v -E '^(/usr/lib/|/System/Library/)'; then
        echo "$tool links a non-system library" >&2
        exit 1
    fi
    codesign --force --sign - "$output/bin/$tool"
done
"$output/bin/ffmpeg" -hide_banner -encoders 2>/dev/null | grep -q ' libx264 ' || { echo "libx264 missing" >&2; exit 1; }
"$output/bin/ffmpeg" -hide_banner -encoders 2>/dev/null | grep -q ' prores_ks ' || { echo "prores_ks missing" >&2; exit 1; }

cp "$work/ffmpeg-$FFMPEG_VERSION/COPYING.GPLv2" "$output/licenses/FFmpeg-COPYING.GPLv2"
cp "$work/ffmpeg-$FFMPEG_VERSION/LICENSE.md" "$output/licenses/FFmpeg-LICENSE.md"
cp "$work/x264-$X264_COMMIT/COPYING" "$output/licenses/x264-COPYING"
cat >"$output/SOURCES.txt" <<EOF
FFmpeg for Koeta Studio (GPL-2.0-or-later as built)

FFmpeg $FFMPEG_VERSION
  source: $FFMPEG_URL
  sha256: $FFMPEG_SHA256
  configure: $(echo "$ffmpeg_configure" | sed "s|$prefix|PREFIX|g")
x264 (stable, $X264_COMMIT)
  source: $X264_URL
  sha256: $X264_SHA256
  configure: $(echo "$x264_configure" | sed "s|$prefix|PREFIX|g")

Built by scripts/build_ffmpeg_bundle.sh in the Koeta Studio repository with
MACOSX_DEPLOYMENT_TARGET=$MIN_MACOS on $(sw_vers -productVersion) arm64. No source
changes. The corresponding source is the two archives above.
EOF
cat >"$output/ffmpeg-bundle.json" <<EOF
{
  "schema_version": 1,
  "ffmpeg": "bin/ffmpeg",
  "ffprobe": "bin/ffprobe",
  "ffmpeg_version": "$FFMPEG_VERSION",
  "x264_commit": "$X264_COMMIT",
  "arch": "arm64",
  "license": "GPL-2.0-or-later"
}
EOF
echo "$output"
