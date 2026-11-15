param([string]$JavaHome)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Android-Tools.ps1"
$projectRoot = Split-Path -Parent $PSScriptRoot
$javaRoot = Get-ZelenkaJavaHome -Requested $JavaHome
$testOutput = Join-Path $projectRoot ('android\build\queue-tests-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testOutput -Force | Out-Null
$queueSource = Join-Path $projectRoot 'android\app\src\main\java\com\zelenka\music\PlaybackQueue.java'
$testSource = Join-Path $projectRoot 'android\tests\PlaybackQueueTest.java'
$lyricsSource = Join-Path $projectRoot 'android\app\src\main\java\com\zelenka\music\Lyrics.java'
$pathSource = Join-Path $projectRoot 'android\app\src\main\java\com\zelenka\music\RepositoryPath.java'
$lyricsTests = Join-Path $projectRoot 'android\tests\LyricsTest.java'
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\javac.exe') -Arguments @(
    '--release', '8', '-encoding', 'UTF-8', '-d', $testOutput, $queueSource, $testSource, $lyricsSource, $pathSource, $lyricsTests
)
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments @(
    '-cp', $testOutput, 'com.zelenka.music.LyricsTest'
)
Invoke-ZelenkaAndroidTool -Tool (Join-Path $javaRoot 'bin\java.exe') -Arguments @(
    '-cp', $testOutput, 'com.zelenka.music.PlaybackQueueTest'
)
