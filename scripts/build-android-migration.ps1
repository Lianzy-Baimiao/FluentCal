param(
    [Parameter(Mandatory = $true)][string]$PreviousApk,
    [string]$LegacyKeystore = (Join-Path $env:USERPROFILE '.android\debug.keystore')
)
# One-time bridge from our original debug key to the permanent release key.
# Android 9+ verifies the v3 proof-of-rotation and retains the application's UID/data.
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$localTools = Join-Path $env:USERPROFILE 'tools\fluentcal-android'
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = (Get-ChildItem (Join-Path $localTools 'ms-java') -Directory | Select-Object -First 1).FullName }
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $localTools 'sdk' }
$apksigner = Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\apksigner.bat'
$verifier = Join-Path $PSScriptRoot 'VerifyAndroidUpgrade.java'
$signingDir = Join-Path $localTools 'signing'
$lineage = Join-Path $signingDir 'debug-to-release.lineage'
$previous = (Resolve-Path -LiteralPath $PreviousApk).Path
$version = [regex]::Match([IO.File]::ReadAllText((Join-Path $repoRoot 'src\mobile\version.ts')), 'MOBILE_VERSION = "([0-9.]+)"').Groups[1].Value
$release = Join-Path $repoRoot "release\FluentCal-Android-$version.apk"
$output = Join-Path $repoRoot "release\FluentCal-Android-$version-legacy-upgrade.apk"
if (-not (Test-Path -LiteralPath $release)) { throw 'Build the normal release APK first.' }
if (-not (Test-Path -LiteralPath $LegacyKeystore)) { throw 'The original debug signing key is required; do not generate a replacement.' }
try {
    if (-not $env:FLUENTCAL_ANDROID_KEYSTORE) {
        $env:FLUENTCAL_ANDROID_KEYSTORE = Join-Path $signingDir 'fluentcal-release.jks'
        $secure = ConvertTo-SecureString ([IO.File]::ReadAllText((Join-Path $signingDir 'password.dpapi')))
        $cred = New-Object System.Management.Automation.PSCredential('fluentcal', $secure)
        $env:FLUENTCAL_ANDROID_STORE_PASSWORD = $cred.GetNetworkCredential().Password
    }
    if (-not $env:FLUENTCAL_ANDROID_STORE_PASSWORD) { throw 'Release signing password is required.' }
    if (-not (Test-Path -LiteralPath $lineage)) {
        & $apksigner rotate --out $lineage --old-signer --ks $LegacyKeystore --ks-key-alias androiddebugkey --ks-pass pass:android --set-installed-data true --set-rollback false --new-signer --ks $env:FLUENTCAL_ANDROID_KEYSTORE --ks-key-alias fluentcal --ks-pass env:FLUENTCAL_ANDROID_STORE_PASSWORD
        if ($LASTEXITCODE -ne 0) { throw 'Could not create signing-key rotation proof.' }
    }
    & $apksigner sign --ks $LegacyKeystore --ks-key-alias androiddebugkey --ks-pass pass:android --next-signer --ks $env:FLUENTCAL_ANDROID_KEYSTORE --ks-key-alias fluentcal --ks-pass env:FLUENTCAL_ANDROID_STORE_PASSWORD --lineage $lineage --rotation-min-sdk-version 28 --v4-signing-enabled false --debuggable-apk-permitted false --out $output $release
    if ($LASTEXITCODE -ne 0) { throw 'Could not sign migration APK.' }
    & $apksigner verify --verbose $output
    if ($LASTEXITCODE -ne 0) { throw 'Migration APK verification failed.' }
    $java = Join-Path $env:JAVA_HOME 'bin\java.exe'
    $classpath = Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\lib\apksigner.jar'
    & $java -cp $classpath $verifier $previous $output
    if ($LASTEXITCODE -ne 0) { throw 'Old APK cannot upgrade to the migration APK.' }
    & $java -cp $classpath $verifier $output $release
    if ($LASTEXITCODE -ne 0) { throw 'Migration APK cannot upgrade to the normal release signing key.' }
    $checksum = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$output.sha256", "$checksum  $([IO.Path]::GetFileName($output))`n")
    Write-Output "Android 9+ migration APK: $output"
    Write-Output "SHA256: $checksum"
} finally { $env:FLUENTCAL_ANDROID_STORE_PASSWORD = $null }
