#!/usr/bin/env bash
# Renders the native side of each parity scene (written by `npm run parity`) and compares it
# with 0.2's rendering: scripts/parity-native.sh [scene ...]. Needs the native toolchain on PATH
# (see native/.cargo/config.toml) and Python with Pillow.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/native/.parity"
cd "$root/native"
# Build, and take the example's path from cargo's own report (the target dir is per machine).
exe="$(cargo build -q -p fluentcal-native --example snapshot --message-format=json \
  | grep -o '"executable":"[^"]*snapshot[^"]*"' | tail -1 \
  | sed -e 's/^"executable":"//' -e 's/"$//' -e 's#\\\\#/#g')"
scenes=("$@")
if [ ${#scenes[@]} -eq 0 ]; then
  for f in "$dir"/*.json; do scenes+=("$(basename "$f" .json)"); done
fi
for s in "${scenes[@]}"; do
  "$exe" "$dir/$s.json" "$dir/$s.native.png" > /dev/null
done
python "$root/scripts/parity-compare.py" "${scenes[@]}"
