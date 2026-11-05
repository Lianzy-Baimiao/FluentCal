param([switch]$Release, [string]$PreviousApk)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $repoRoot
try {
    $localTools = Join-Path $env:USERPROFILE 'tools\fluentcal-android'
    if (-not $env:JAVA_HOME -and (Test-Path (Join-Path $localTools 'ms-java'))) {
        $jdk = Get-ChildItem (Join-Path $localTools 'ms-java') -Directory | Select-Object -First 1
        if ($jdk) { $env:JAVA_HOME = $jdk.FullName }
    }
    if (-not $env:ANDROID_HOME -and (Test-Path (Join-Path $localTools 'sdk'))) { $env:ANDROID_HOME = Join-Path $localTools 'sdk' }
    if (-not $env:JAVA_HOME -or -not (Test-Path (Join-Path $env:JAVA_HOME 'bin\java.exe'))) { throw 'Set JAVA_HOME to a JDK 21 installation.' }
    if (-not $env:ANDROID_HOME) { throw 'Set ANDROID_HOME to the Android SDK (platforms;android-36 and build-tools;36.0.0).' }
    $mobileVersion = [regex]::Match([IO.File]::ReadAllText((Join-Path $repoRoot 'src\mobile\version.ts')), 'MOBILE_VERSION = "([0-9.]+)"').Groups[1].Value
    if (-not $mobileVersion) { throw 'Cannot read Android version.' }
    if ($Release -and -not $env:FLUENTCAL_ANDROID_KEYSTORE) {
        $signingDir = Join-Path $localTools 'signing'
        $keystore = Join-Path $signingDir 'fluentcal-release.jks'
        $secretFile = Join-Path $signingDir 'password.dpapi'
        if ((Test-Path -LiteralPath $keystore) -and (Test-Path -LiteralPath $secretFile)) {
            $secure = ConvertTo-SecureString ([IO.File]::ReadAllText($secretFile))
            $cred = New-Object System.Management.Automation.PSCredential('fluentcal', $secure)
            $env:FLUENTCAL_ANDROID_KEYSTORE = $keystore
            $env:FLUENTCAL_ANDROID_STORE_PASSWORD = $cred.GetNetworkCredential().Password
        }
    }
    if ($Release -and (-not $env:FLUENTCAL_ANDROID_KEYSTORE -or -not $env:FLUENTCAL_ANDROID_STORE_PASSWORD)) { throw 'Release needs FLUENTCAL_ANDROID_KEYSTORE and FLUENTCAL_ANDROID_STORE_PASSWORD. Keep this signing key for all future updates.' }
    & npm.cmd run android:sync
    if ($LASTEXITCODE -ne 0) { throw 'Web build / Capacitor sync failed.' }
    Push-Location (Join-Path $repoRoot 'android')
    try {
        $taskName = if ($Release) { ':app:assembleRelease' } else { ':app:assembleDebug' }
        & .\gradlew.bat $taskName --console=plain
        if ($LASTEXITCODE -ne 0) { throw 'Android build failed.' }
    } finally { Pop-Location }
    $variant = if ($Release) { 'release' } else { 'debug' }
    $apkName = if ($Release) { 'app-release.apk' } else { 'app-debug.apk' }
    $outputName = if ($Release) { "FluentCal-Android-$mobileVersion.apk" } else { "FluentCal-Android-$mobileVersion-debug.apk" }
    $output = Join-Path $repoRoot "release\$outputName"
    New-Item -ItemType Directory -Force (Join-Path $repoRoot 'release') | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot "android\app\build\outputs\apk\$variant\$apkName") -Destination $output
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($output)
    try { $checksum = ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $sha.Dispose() }
    [System.IO.File]::WriteAllText("$output.sha256", "$checksum  $([System.IO.Path]::GetFileName($output))`n")
    Write-Output "SHA256: $checksum"
    Write-Output "APK: $output"
    if ($Release) {
        & (Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\apksigner.bat') verify --verbose $output
        if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed.' }
        if ($PreviousApk) {
            & (Join-Path $env:JAVA_HOME 'bin\java.exe') -cp (Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\lib\apksigner.jar') (Join-Path $PSScriptRoot 'VerifyAndroidUpgrade.java') $PreviousApk $output 24
            if ($LASTEXITCODE -ne 0) { throw 'Update signing compatibility verification failed.' }
        }
    }
} finally { $env:FLUENTCAL_ANDROID_STORE_PASSWORD = $null; Pop-Location }
