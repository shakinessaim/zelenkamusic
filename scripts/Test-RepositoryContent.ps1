$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path $projectRoot ('target\verification\repository-download-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$output = Join-Path $testRoot 'RepositoryContentTest.exe'
$compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
& $compiler /nologo /codepage:65001 /target:exe "/out:$output" /reference:System.Web.Extensions.dll "$projectRoot\tests\RepositoryContentTest.cs" "$projectRoot\installer\RepositoryContent.cs"
if ($LASTEXITCODE -ne 0) { throw 'Repository download test compilation failed.' }
& $output $testRoot
if ($LASTEXITCODE -ne 0) { throw 'Repository download tests failed.' }
