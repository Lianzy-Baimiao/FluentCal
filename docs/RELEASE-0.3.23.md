# FluentCal 0.3.23

This update fixes the crowded countdown strip and aligns new-install countdown defaults with GitHub Issue #18.

## What's new

- Keep the “Expand countdowns” button fully visible at the far right, even when many long countdown items are shown. The item strip uses the remaining width and clips overflow.
- New installs show manually marked tasks and ordinary festivals by default. Statutory holidays and solar terms start off; each source remains independently selectable. Existing saved source preferences are preserved.
- Multi-day statutory holidays count down from the first day only. A three-day New Year break appears once, rather than as three countdown entries.
- Upgrades preserve explicitly saved countdown-source preferences.

## Downloads

Install `FluentCal_0.3.23_x64-setup.exe` to upgrade while retaining existing calendar data, or use the standalone `FluentCal-Desktop-0.3.23.exe`. Both include SHA-256 checksum files.

The Microsoft Store package version is 0.3.23.0. Submission 18 (1152921505702472573) was submitted on 2026-11-22 and is in preprocessing; it is set to publish immediately after certification.

## Verification

The countdown-strip E2E covers fourteen long entries at 880px and 600px window widths and asserts the expand control stays fully visible at the right edge. New Year holiday counting is covered by a three-day 2027 holiday fixture and checked against the November 22 40-day countdown. Rust workspace tests, 233 front-end tests, TypeScript production build, translation checks, web Issue regressions and Android browser E2E pass. The local Windows upgrade passed and retained all 93 tasks.

## SHA-256

- Installer: `a0639e70175386f337a9fe7627d8ad1cd73dcde6a7011d21aa8296039b3c8542`
- Standalone EXE: `cdb64d5215716329a9e70005ad3a62eb230d67c496b57cb5bed91285926755cd`
- Microsoft Store MSIX: `2f1c5c972191de34ccd3914d19e6407e402347cac5f14731454e735a77197162`
