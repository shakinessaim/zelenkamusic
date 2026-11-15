param(
    [string]$SdkPath,
    [string]$JavaHome,
    [int]$CompileSdk = 35,
    [string]$BuildToolsVersion,
    [switch]$SkipTests,
    [switch]$Install,
    [string]$DeviceSerial,
    [string]$KeystorePath,
    [string]$KeyAlias = 'zelenka-local'
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Android-Tools.ps1"
$projectRoot = Split-Path -Parent $PSScriptRoot
$javaRoot = Get-ZelenkaJavaHome -Requested $JavaHome
$sdkRoot = Get-ZelenkaAndroidSdk -Requested $SdkPath

$platform = Join-Path $sdkRoot "platforms\android-$CompileSdk\android.jar"
if (-not (Test-Path -LiteralPath $platform)) {
    $available = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'platforms') -Directory |
        Where-Object {
            $_.Name -match '^android-\d+(?:\.\d+)?$' -and
            [int]($_.Name -replace '^android-(\d+).*$', '$1') -ge $CompileSdk -and
            (Test-Path -LiteralPath (Join-Path $_.FullName 'android.jar'))
        } |
        Sort-Object {
            $apiVersion = $_.Name -replace '^android-', ''
            if ($apiVersion -notmatch '\.') { $apiVersion += '.0' }
            [version]$apiVersion
        } -Descending | Select-Object -First 1
    if (-not $available) { throw "Install Android SDK Platform $CompileSdk in Android Studio > SDK Manager." }
    $platform = Join-Path $available.FullName 'android.jar'
}
$toolsRoot = Join-Path $sdkRoot 'build-tools'
if ($BuildToolsVersion) { $buildTools = Join-Path $toolsRoot $BuildToolsVersion }
else {
    $available = Get-ChildItem -LiteralPath $toolsRoot -Directory |
        Where-Object {
            $_.Name -match '^\d+\.\d+\.\d+$' -and [version]$_.Name -ge [version]'35.0.0' -and
            (Test-Path -LiteralPath (Join-Path $_.FullName 'aapt2.exe')) -and
            (Test-Path -LiteralPath (Join-Path $_.FullName 'zipalign.exe')) -and
            (Test-Path -LiteralPath (Join-Path $_.FullName 'lib\d8.jar')) -and
            (Test-Path -LiteralPath (Join-Path $_.FullName 'lib\apksigner.jar'))
        } |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if (-not $available) { throw 'Install Android SDK Build-Tools 35.0.0 (or newer) in Android Studio > SDK Manager > SDK Tools.' }
    $buildTools = $available.FullName
}
foreach ($required in @('aapt2.exe', 'zipalign.exe', 'lib\d8.jar', 'lib\apksigner.jar')) {
    if (-not (Test-Path -LiteralPath (Join-Path $buildTools $required))) { throw "Missing Android Build-Tools file: $required" }
}
Write-Output "Java: $javaRoot"
Write-Output "Android platform: $platform"
Write-Output "Android Build-Tools: $buildTools"
if (-not $SkipTests) { & "$PSScriptRoot\Test-Android.ps1" -JavaHome $javaRoot }

# A fresh build directory avoids packaging stale Java classes or resources.
$build = Join-Path $projectRoot ('android\build\apk-' + [Guid]::NewGuid().ToString('N'))
$resources = Join-Path $build 'res'
$classes = Join-Path $build 'classes'
$generated = Join-Path $build 'generated'
$dex = Join-Path $build 'dex'
foreach ($folder in @($build, $resources, $classes, $generated, $dex)) {
    New-Item -ItemType Directory -Path $folder -Force | Out-Null
}
# Stage the platform jar with the other build inputs.
$platformCopy = Join-Path $build 'android.jar'
Copy-Item -LiteralPath $platform -Destination $platformCopy
$platform = $platformCopy
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
Copy-Item -Path (Join-Path $projectRoot 'android\app\src\main\res\*') -Destination $resources -Recurse -Force
$logoFolder = Join-Path $resources 'drawable-nodpi'
New-Item -ItemType Directory -Path $logoFolder -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'assets\images\logo.jpg') -Destination (Join-Path $logoFolder 'logo.jpg')

# AGP uses namespace in app/build.gradle. The direct SDK build needs a manifest package.
$manifest = New-Object System.Xml.XmlDocument
$manifest.Load((Join-Path $projectRoot 'android\app\src\main\AndroidManifest.xml'))
$manifest.DocumentElement.SetAttribute('package', 'com.zelenka.music')
$manifestPath = Join-Path $build 'AndroidManifest.xml'
$manifest.Save($manifestPath)

$compiledResources = Join-Path $build 'resources.zip'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $buildTools 'aapt2.exe') -Arguments @(
    'compile', '--dir', $resources, '-o', $compiledResources
)
$unsigned = Join-Path $build 'unsigned.apk'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $buildTools 'aapt2.exe') -Arguments @(
    'link', '-o', $unsigned, '-I', $platform, '--manifest', $manifestPath,
    '--java', $generated, '--min-sdk-version', '26', '--target-sdk-version', '35',
    '--version-code', '102', '--version-name', '0.1.2', $compiledResources
)
$javaSources = @(
    Get-ChildItem -LiteralPath (Join-Path $projectRoot 'android\app\src\main\java') -Filter '*.java' -Recurse
    Get-ChildItem -LiteralPath $generated -Filter '*.java' -Recurse
) | ForEach-Object { $_.FullName }
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments (@(
    (Join-Path $PSScriptRoot 'CompileAndroid.java'), $platform, $classes
) + [string[]]$javaSources)
$classesJar = Join-Path $build 'classes.jar'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\jar.exe') -Arguments @('-cf', $classesJar, '-C', $classes, '.')
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments @(
    '-cp', (Join-Path $buildTools 'lib\d8.jar'), 'com.android.tools.r8.D8', '--release',
    '--min-api', '26', '--lib', $platform, '--output', $dex, $classesJar
)
$archive = [System.IO.Compression.ZipFile]::Open($unsigned, [System.IO.Compression.ZipArchiveMode]::Update)
try {
    foreach ($file in Get-ChildItem -LiteralPath $dex -Filter '*.dex') {
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $archive, $file.FullName, $file.Name, [System.IO.Compression.CompressionLevel]::Optimal
        ) | Out-Null
    }
} finally { $archive.Dispose() }
$aligned = Join-Path $build 'aligned.apk'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $buildTools 'zipalign.exe') -Arguments @('-f', '-p', '4', $unsigned, $aligned)

$customKey = -not [string]::IsNullOrEmpty($KeystorePath)
if ($customKey) {
    if (-not (Test-Path -LiteralPath $KeystorePath)) { throw "Signing keystore not found: $KeystorePath" }
    if (-not $env:ZELENKA_KEYSTORE_PASSWORD) { throw 'Set ZELENKA_KEYSTORE_PASSWORD for the supplied signing keystore.' }
    $keyArguments = @('--ks-pass', 'env:ZELENKA_KEYSTORE_PASSWORD')
    if ($env:ZELENKA_KEY_PASSWORD) { $keyArguments += @('--key-pass', 'env:ZELENKA_KEY_PASSWORD') }
} else {
    $KeystorePath = Join-Path $projectRoot '.tools\android-signing\zelenka-local.jks'
    New-Item -ItemType Directory -Path (Split-Path -Parent $KeystorePath) -Force | Out-Null
    $KeyAlias = 'zelenka-local'
    if (-not (Test-Path -LiteralPath $KeystorePath)) {
        Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\keytool.exe') -Arguments @(
            '-genkeypair', '-noprompt', '-keystore', $KeystorePath, '-storetype', 'JKS',
            '-storepass', 'android', '-keypass', 'android', '-alias', $KeyAlias,
            '-keyalg', 'RSA', '-keysize', '2048', '-validity', '10000',
            '-dname', 'CN=ZelenkaMusic Local Build, O=ZelenkaMusic, C=RU'
        )
    }
    $keyArguments = @('--ks-pass', 'pass:android', '--key-pass', 'pass:android')
    Write-Output 'Signing with the persistent local development key in .tools\android-signing.'
}
$dist = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$apk = Join-Path $dist 'ZelenkaMusic-Android-0.1.2.apk'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments (@(
    '-jar', (Join-Path $buildTools 'lib\apksigner.jar'), 'sign', '--ks', $KeystorePath,
    '--ks-key-alias', $KeyAlias, '--out', $apk
) + $keyArguments + @($aligned))
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments @(
    '-jar', (Join-Path $buildTools 'lib\apksigner.jar'), 'verify', '--verbose', $apk
)
Invoke-ZelenkaAndroidTool -Tool (Join-Path $buildTools 'zipalign.exe') -Arguments @('-c', '4', $apk)
$hash = (Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash
[System.IO.File]::WriteAllText("$apk.sha256", "$hash  ZelenkaMusic-Android-0.1.2.apk`n")
Copy-Item -LiteralPath (Join-Path $projectRoot 'android\INSTALL.txt') -Destination (Join-Path $dist 'ZelenkaMusic-Android-INSTALL.txt') -Force
Write-Output "APK installer: $apk"
Write-Output "SHA-256: $hash"

if ($Install) {
    $adb = Join-Path $sdkRoot 'platform-tools\adb.exe'
    if (-not (Test-Path -LiteralPath $adb)) { throw 'APK built, but Android SDK Platform-Tools are missing. Install them to use -Install.' }
    $adbArguments = @()
    if ($DeviceSerial) { $adbArguments += @('-s', $DeviceSerial) }
    Invoke-ZelenkaAndroidTool -Tool $adb -Arguments ($adbArguments + @('install', '-r', $apk))
}
