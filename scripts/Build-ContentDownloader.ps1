$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$output = Join-Path $projectRoot 'dist\ZelenkaMusic-Content.exe'
New-Item -ItemType Directory -Path (Join-Path $projectRoot 'dist') -Force | Out-Null
$compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
& $compiler /nologo /codepage:65001 /optimize+ /target:winexe /platform:x64 "/out:$output" "/win32icon:$projectRoot\assets\icons\zelenka.ico" /reference:System.Windows.Forms.dll,System.Drawing.dll,System.Web.Extensions.dll "$projectRoot\installer\DownloadContent.cs" "$projectRoot\installer\RepositoryContent.cs"
if ($LASTEXITCODE -ne 0) { throw 'Content downloader compilation failed.' }
