#!/usr/bin/env bash
# Builds the native release: the web view's pages (vite build → dist/, embedded in the exe),
# fluentcal.exe, and the GitHub installer FluentCal_<version>_x64-setup.exe next to the exe.
# Needs the native toolchain on PATH (native/.cargo/config.toml) and NSIS (makensis; Tauri's
# copy in %LOCALAPPDATA%\tauri\NSIS works).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
npx vite build --logLevel warn
cd native
exe="$(cargo build --release -q -p fluentcal-native --bin fluentcal --message-format=json \
  | grep -o '"executable":"[^"]*fluentcal\.exe"' | tail -1 \
  | sed -e 's/^"executable":"//' -e 's/"$//' -e 's#\\\\#/#g')"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
makensis="$(command -v makensis || echo "$LOCALAPPDATA/tauri/NSIS/makensis.exe")"
out="$(dirname "$exe")/FluentCal_${version}_x64-setup.exe"
"$makensis" -V2 "-DVERSION=$version" "-DEXE=$(cygpath -w "$exe")" "-DOUT=$(cygpath -w "$out")" "$(cygpath -w app/installer/fluentcal.nsi)"
ls -la "$exe" "$out"
