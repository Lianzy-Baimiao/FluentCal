#!/usr/bin/env bash
# Builds the Microsoft Store package (MSIX, unsigned: the Store signs it):
#   scripts/build-msix.sh [identity-name publisher publisher-display-name [version]]
# The version defaults to the Cargo version with .0 appended. To replace changed content,
# increase one of the first three fields (e.g. 0.3.1.0). Microsoft Store reserves the
# fourth field: it MUST be zero, and each field must be in the range 0..65535.
# The identity comes from Partner Center after the app name is reserved; without it, test
# placeholders are used (fine for checking the package, not for upload).
# makeappx comes from Microsoft's SDK BuildTools NuGet package (no SDK install), cached in
# ~/tools/sdk-buildtools. Run scripts/build-native.sh first (it builds fluentcal.exe).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
name="${1:-FluentCal.Test}"
publisher="${2:-CN=FluentCal Test}"
pubname="${3:-FluentCal}"
cd "$root/native"
version="${4:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1).0}"
invalid_version() {
  echo "Invalid Microsoft Store package version: $version (expected x.y.z.0; each field 0..65535)" >&2
  exit 1
}
[[ "$version" =~ ^[0-9]{1,5}\.[0-9]{1,5}\.[0-9]{1,5}\.0$ ]] || invalid_version
IFS=. read -r -a version_fields <<< "$version"
for field in "${version_fields[@]}"; do
  (( 10#$field <= 65535 )) || invalid_version
done

target="$(sed -n 's/^target-dir *= *"\(.*\)"/\1/p' .cargo/config.toml | sed 's#\\\\#/#g' | head -1)"
exe="${target:-target}/release/fluentcal.exe"
[ -f "$exe" ] || { echo "no $exe: run scripts/build-native.sh first"; exit 1; }

tools="$HOME/tools/sdk-buildtools"
makeappx="$(find "$tools" -path '*x64*' -name makeappx.exe 2>/dev/null | head -1 || true)"
if [ -z "$makeappx" ]; then
  mkdir -p "$tools"
  ver="$(curl -s -m 30 https://api.nuget.org/v3-flatcontainer/microsoft.windows.sdk.buildtools/index.json | python -c "import json,sys; print([v for v in json.load(sys.stdin)['versions'] if 'preview' not in v][-1])")"
  curl -sL -m 300 -o "$tools/buildtools.zip" "https://api.nuget.org/v3-flatcontainer/microsoft.windows.sdk.buildtools/$ver/microsoft.windows.sdk.buildtools.$ver.nupkg"
  (cd "$tools" && unzip -q -o buildtools.zip -d "$ver" && rm buildtools.zip)
  makeappx="$(find "$tools" -path '*x64*' -name makeappx.exe | head -1)"
fi

stage="$(dirname "$exe")/msix"
rm -rf "$stage"
mkdir -p "$stage/assets"
cp "$exe" "$stage/fluentcal.exe"
icons="$root/src-tauri/icons"
cp "$icons/StoreLogo.png" "$icons/Square44x44Logo.png" "$icons/Square150x150Logo.png" "$stage/assets/"
sed -e "s|\$IDENTITY_NAME|$name|" -e "s|\$PUBLISHER_NAME|$pubname|" -e "s|\$PUBLISHER|$publisher|" -e "s|\$VERSION|$version|" \
  app/package/AppxManifest.xml > "$stage/AppxManifest.xml"
out="$(dirname "$exe")/FluentCal_${version}_x64.msix"
MSYS2_ARG_CONV_EXCL='*' "$makeappx" pack /o /d "$(cygpath -w "$stage")" /p "$(cygpath -w "$out")"
ls -la "$out"
