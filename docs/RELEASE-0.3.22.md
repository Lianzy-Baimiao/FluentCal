# FluentCal 0.3.22

FluentCal 0.3.22 completes the full-alpha colour workflow and GitHub Issues #17–#21 improvements.

## What's new

- Preserve RGBA colours for tasks, groups, day marks, calendar backgrounds, the accent, exports and data sync across Windows, Android and the read-only web view.
- Choose calendar fonts, adaptive or custom text colours, task-title colour behaviour and text-outline width. Title-bar icons adapt to calendar text contrast.
- Select countdown contents independently: manually marked tasks, groups (including ungrouped tasks), statutory holidays, other festivals and solar terms. Same-day observances share one countdown row.
- Set a display colour for ungrouped tasks. Task-editor group options display colour and task counts; list labels remain optional.
- Keep group administration on its dedicated settings page, make long task notes scroll, and size the countdown filter control to 20px beside the 22px countdown pills.
- Preserve ungrouped tasks when deleting selected groups. Locate targets with a short flashing highlight that becomes static when reduced motion is enabled; each new jump replaces the previous target.

## Downloads

Install `FluentCal_0.3.22_x64-setup.exe` to upgrade while retaining existing calendar data, or use the standalone `FluentCal-Desktop-0.3.22.exe`. Both include SHA-256 checksum files.

Microsoft Store package version is 0.3.22.0. Store availability follows Microsoft's certification and publication process.

## Verification

Rust workspace tests (including 50 desktop E2E, 15 editor and 11 settings tests), 231 front-end tests, TypeScript build, Android browser flows, web Issue regressions and 1,175 translation messages pass. Windows and Android packages pass signature and identity checks. The local Windows upgrade preserves all 93 tasks and the original data SHA-256.

## SHA-256

- Installer: `64716e9f3e10f480d7370edafa0f4cb9e869ef98a1dc64a5ecb8ad41a6b28e31`
- Standalone EXE: `979cd0ac19099c5a44bddb7ed531ab808291aca0def3e40f51e8dc258f9f52de`
- Microsoft Store MSIX: `ce1993aea8955b14a6600195178eeea753a7055471e29d97887bbd3babfa5f10`
