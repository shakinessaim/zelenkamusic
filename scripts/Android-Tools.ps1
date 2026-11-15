function Get-ZelenkaJavaHome {
    param([string]$Requested)
    $projectRoot = Split-Path -Parent $PSScriptRoot
    $candidates = @(
        $Requested,
        $env:JAVA_HOME,
        (Join-Path $env:ProgramFiles 'Android\Android Studio\jbr'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Android Studio\jbr'),
        (Join-Path $projectRoot '.tools\jdk')
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath (Join-Path $candidate 'bin\javac.exe'))) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    throw 'Java JDK not found. Install Android Studio (which includes Java), or pass -JavaHome with a JDK 17 or newer directory.'
}

function Get-ZelenkaAndroidSdk {
    param([string]$Requested)
    $projectRoot = Split-Path -Parent $PSScriptRoot
    $candidates = @(
        $Requested,
        $env:ANDROID_HOME,
        $env:ANDROID_SDK_ROOT,
        (Join-Path $env:LOCALAPPDATA 'Android\Sdk'),
        (Join-Path $projectRoot '.tools\android-sdk')
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath (Join-Path $candidate 'platforms'))) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    throw 'Android SDK not found. In Android Studio > SDK Manager, install Android SDK Platform 35 and Build-Tools 35.0.0, or pass -SdkPath.'
}

function Invoke-ZelenkaAndroidTool {
    param([string]$Tool, [string[]]$Arguments)
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Android build tool failed: $Tool (exit $LASTEXITCODE)" }
}
