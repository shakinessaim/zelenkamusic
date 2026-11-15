param([switch]$Installer, [switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    $env:PATH = "$cargoBin;$env:PATH"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install Rust from https://rustup.rs with the MSVC toolchain.' }
& "$PSScriptRoot\New-Icon.ps1"
if (-not $SkipTests) {
    & cargo test --lib --test window_restore
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    & cargo check --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'Compilation checks failed.' }
}
& cargo build --release --bin zelenka-music
if ($LASTEXITCODE -ne 0) { throw 'Release build failed. Install Visual Studio Build Tools with Desktop development with C++ and the Windows SDK.' }
$portable = Join-Path $projectRoot 'dist\ZelenkaMusic'
New-Item -ItemType Directory -Path (Join-Path $portable 'songs') -Force | Out-Null
Copy-Item -LiteralPath 'target\release\zelenka-music.exe' -Destination (Join-Path $portable 'ZelenkaMusic.exe')
Copy-Item -LiteralPath 'target\release\zelenka-music.exe' -Destination (Join-Path $projectRoot 'ZelenkaMusic.exe')
Copy-Item -LiteralPath 'README.md' -Destination $portable
if (Test-Path -LiteralPath 'VERIFICATION.md') { Copy-Item -LiteralPath 'VERIFICATION.md' -Destination $portable -Force }
Set-Content -LiteralPath (Join-Path $portable 'songs\PLACE-MUSIC-HERE.txt') -Encoding UTF8 -Value 'Поместите каждый альбом в отдельную папку с MP3 или FLAC. Можно добавить обложку cover.jpg, cover.png или cover.webp.'
Compress-Archive -Path $portable -DestinationPath (Join-Path $projectRoot 'dist\ZelenkaMusic-Windows-x64.zip') -Force
Write-Output "Portable app: $portable\ZelenkaMusic.exe"
Write-Output "Run with the existing library: $projectRoot\ZelenkaMusic.exe"
if ($Installer) {
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    $innoCompiler = if ($compiler) { $compiler.Source } else { Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe' }
    if (Test-Path -LiteralPath $innoCompiler) {
        & $innoCompiler 'installer\ZelenkaMusic.iss'
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    } else {
        & "$PSScriptRoot\Build-NativeInstaller.ps1"
    }
}
