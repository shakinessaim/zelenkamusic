# Builds a small per-user installer with the .NET Framework included in Windows 10/11.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$stage = Join-Path $projectRoot 'dist\installer-files'
$exe = Join-Path $projectRoot 'dist\ZelenkaMusic\ZelenkaMusic.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'Build the portable application first with scripts\Build.ps1.' }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\Uninstall.ps1') -Destination $stage
$payload = Join-Path $stage 'payload.zip'
Compress-Archive -LiteralPath @((Join-Path $stage 'ZelenkaMusic.exe'), (Join-Path $stage 'README.md'), (Join-Path $stage 'Uninstall.ps1')) -DestinationPath $payload -Force
$output = Join-Path $projectRoot 'dist\ZelenkaMusic-Setup-0.1.2.exe'
$compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (-not (Test-Path -LiteralPath $compiler)) { throw 'The Windows .NET Framework compiler is unavailable. Install Inno Setup 6 and run scripts\Build.ps1 -Installer.' }
& $compiler /nologo /codepage:65001 /optimize+ /target:winexe /platform:x64 "/out:$output" "/win32icon:$projectRoot\assets\icons\zelenka.ico" "/win32manifest:$projectRoot\assets\windows.manifest" /reference:System.Windows.Forms.dll,System.Drawing.dll,System.IO.Compression.dll,System.IO.Compression.FileSystem.dll,Microsoft.CSharp.dll "/resource:$payload,Payload" "$projectRoot\installer\Setup.cs"
if ($LASTEXITCODE -ne 0) { throw 'Windows installer compilation failed.' }
Write-Output "Windows installer: $output"
