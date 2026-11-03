#!/usr/bin/env bash
# Regression: Store-invalid versions must fail before invoking the SDK packager.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
for version in 0.3.0.1 0.3.0 0.3.0.0.0 0.3.1.0-preview 65536.0.0.0 0.65536.0.0 0.3.65536.0; do
  if output="$(bash "$root/scripts/build-msix.sh" FluentCal.Test 'CN=FluentCal Test' FluentCal "$version" 2>&1)"; then
    echo "FAIL: accepted invalid Store version $version" >&2
    exit 1
  fi
  if [[ "$output" != *"Invalid Microsoft Store package version:"* ]]; then
    echo "FAIL: $version failed after version validation: $output" >&2
    exit 1
  fi
  echo "PASS: rejected $version before packaging"
done
