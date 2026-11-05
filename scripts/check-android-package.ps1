param([Parameter(Mandatory = $true)][string]$Apk)
$ErrorActionPreference = 'Stop'
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:USERPROFILE 'tools\fluentcal-android\sdk' }
$aapt = Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\aapt.exe'
$badging = (& $aapt dump badging $Apk) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect APK.' }
if ($badging -notmatch "package: name='com.fluentcal.mobile'") { throw 'Unexpected Android application ID.' }
if ($badging -match 'application-debuggable') { throw 'Release APK must not enable debugging.' }
$tree = (& $aapt dump xmltree $Apk AndroidManifest.xml) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect packaged manifest.' }
$capture = [regex]::Match($tree, '(?s)android:name[^\r\n]+="com\.journeyapps\.barcodescanner\.CaptureActivity".*?(?=\n\s*E:|\z)').Value
if ($capture -notmatch 'android:screenOrientation[^\r\n]+=\(type 0x10\)0x1\b') { throw 'Scanner is not portrait in packaged manifest.' }
if ($capture -notmatch 'android:exported[^\r\n]+=\(type 0x12\)0x0\b') { throw 'Scanner must not be exported.' }
Write-Output 'PASS packaged APK: expected application ID, debugging disabled, private portrait scanner'
