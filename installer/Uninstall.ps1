$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$answer = [System.Windows.Forms.MessageBox]::Show('Удалить ZelenkaMusic? Музыка, плейлисты и настройки будут сохранены.', 'ZelenkaMusic', 'YesNo', 'Question')
if ($answer -ne [System.Windows.Forms.DialogResult]::Yes) { exit }
$installRoot = [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\ZelenkaMusic'))
if ([System.IO.Path]::GetFullPath($PSScriptRoot).TrimEnd('\') -ne $installRoot.TrimEnd('\')) { throw 'Программа удаления находится вне папки установки ZelenkaMusic.' }
try {
    foreach ($name in @('ZelenkaMusic.exe','README.md')) {
        $file = Join-Path $installRoot $name
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force }
    }
    $menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'ZelenkaMusic'
    foreach ($name in @('ZelenkaMusic.lnk','Папка музыки.lnk','Music folder.lnk','Тексты песен.lnk')) {
        $shortcut = Join-Path $menu $name
        if (Test-Path -LiteralPath $shortcut) { Remove-Item -LiteralPath $shortcut -Force }
    }
    if ((Test-Path -LiteralPath $menu) -and @(Get-ChildItem -LiteralPath $menu -Force).Count -eq 0) { Remove-Item -LiteralPath $menu }
    Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\ZelenkaMusic' -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $installRoot 'Uninstall.ps1') -Force
    [System.Windows.Forms.MessageBox]::Show('ZelenkaMusic удалён. Музыка, плейлисты и настройки сохранены.', 'ZelenkaMusic', 'OK', 'Information') | Out-Null
} catch {
    [System.Windows.Forms.MessageBox]::Show("Закройте ZelenkaMusic и попробуйте ещё раз.`n`n$($_.Exception.Message)", 'Ошибка удаления', 'OK', 'Error') | Out-Null
    exit 1
}
