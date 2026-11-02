#!/usr/bin/env bash
# Microsoft Store screenshots of the native app (docs/store/*.png): the README's scenes at 2×
# (the same demo data and fixed date), rendered offscreen by examples/snapshot.rs on the
# stand-in wallpaper. Needs what parity.mjs and parity-native.sh need.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
scenes=(store-month-light store-month-dark store-gantt store-settings-appearance)
PARITY_SCALE=2 node scripts/parity.mjs "${scenes[@]}"
cd native
exe="$(cargo build -q -p fluentcal-native --example snapshot --message-format=json \
  | grep -o '"executable":"[^"]*snapshot[^"]*"' | tail -1 \
  | sed -e 's/^"executable":"//' -e 's/"$//' -e 's#\\\\#/#g')"
mkdir -p "$root/docs/store"
for s in "${scenes[@]}"; do
  "$exe" ".parity/$s.json" ".parity/$s.native.png" > /dev/null
done
# Settings pages are rendered at their full height; the screenshot is the window (548 px).
python - "$root" <<'EOF'
import sys
from pathlib import Path
from PIL import Image
root = Path(sys.argv[1])
for s in ('store-month-light', 'store-month-dark', 'store-gantt', 'store-settings-appearance'):
    im = Image.open(root / 'native' / '.parity' / f'{s}.native.png').convert('RGB')
    if s == 'store-settings-appearance':
        im = im.crop((0, 0, im.width, 548 * 2))
    out = root / 'docs' / 'store' / f'{s[6:]}.png'
    im.save(out, optimize=True)
    print(out.name, im.size)
EOF
