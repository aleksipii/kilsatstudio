#!/bin/sh
# Run the g2_export example on the strict model-free path inside a macOS sandbox.
#
#   g2_model_free_run.sh BIN OUT_DIR svg|png
#   g2_model_free_run.sh BIN OUT_DIR mp4 FFMPEG
#
# BIN is the built example (target/release/examples/g2_export). The sandbox denies
# all network access and every process start except BIN itself and, for mp4, the
# resolved FFmpeg binary. HOME is a fresh empty directory and PATH points nowhere,
# so no model, recognizer, Python process or download can run. Afterwards HOME
# must still be empty and OUT_DIR must contain no model-like files. The JSON
# summary from the example goes to stdout. macOS only (sandbox-exec).
set -eu
[ "$#" -ge 3 ] || { echo "usage: $0 BIN OUT_DIR svg|png|mp4 [FFMPEG]" >&2; exit 2; }
bin=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
out=$2
format=$3
ffmpeg_arg=${4:-}
allow="(allow process-exec (literal \"$bin\"))"
set -- "$out" "$format"
if [ "$format" = mp4 ]; then
  [ -n "$ffmpeg_arg" ] || { echo "mp4 needs FFMPEG" >&2; exit 2; }
  ffmpeg=$(realpath "$ffmpeg_arg")
  allow="$allow(allow process-exec (literal \"$ffmpeg\"))"
  set -- "$@" --ffmpeg "$ffmpeg"
fi
profile="(version 1)(allow default)(deny network*)(deny process-exec)$allow"
home=$(mktemp -d)
env -i HOME="$home" PATH=/nonexistent KILSAT_COMMIT="${KILSAT_COMMIT:-}" \
  /usr/bin/sandbox-exec -p "$profile" "$bin" "$@"
leftover=$(find "$home" -mindepth 1 | head -n 1)
if [ -n "$leftover" ]; then
  echo "model-free run wrote under HOME: $leftover" >&2
  exit 1
fi
rmdir "$home"
models=$(find "$out" -type f \( -name '*.onnx' -o -name '*.pth' -o -name '*.pt' \
  -o -name '*.safetensors' -o -name '*.bin' -o -name '*.npz' \) | head -n 1)
if [ -n "$models" ]; then
  echo "model-like file in output: $models" >&2
  exit 1
fi
