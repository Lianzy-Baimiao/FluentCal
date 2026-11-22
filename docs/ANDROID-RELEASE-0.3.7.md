# FluentCal Android 0.3.7

## Which APK to download

- New installs and existing 0.3.0+ release installs: `FluentCal-Android-0.3.7.apk`. Install over the previous release to keep app data.
- Existing 0.1.0 / 0.2.0 debug installs that report a signature mismatch, Android 9+: install `FluentCal-Android-0.3.7-legacy-upgrade.apk` once, then install the regular release APK. Do not uninstall first.
- Android 7/8 cannot use signing-key rotation. Export JSON, uninstall the old debug app, install the regular release and import the backup.

Both packages contain the same app and use versionCode 10. The migration APK adds the legacy signing lineage for Android 9+ only.

## What's new

- Task colours support RGBA opacity and remain intact when unrelated task fields are edited or synchronized.
- Gantt bars and the read-only browser view display the same transparent task colours as desktop.
- The mobile app retains offline storage, reminders, group filtering, recurrence, and automatic WebDAV / GitHub / Gitee sync.

The interface remains in Simplified Chinese. Requires Android 7.0+ and Android System WebView 120+.

## Verification

Android browser E2E passes. The release APK is non-debuggable and signed with the regular release key; certificate compatibility passes on API 24, 27, 28, 32, 33 and 36. The migration APK verifies the old debug → migration → release signing chain on Android 9+.

- Release APK SHA-256: `cf190574b6e1422ae80bf7e3a5f68c86f5abc6443eca605c23437bbe0c170ca4`
- Migration APK SHA-256: `c74352fc8fd5a4b1f1c7198030bb628b2b7c876e91d8252669ec560333b0f729`
