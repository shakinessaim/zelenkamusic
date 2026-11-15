$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$projectRoot = Split-Path -Parent $PSScriptRoot
$iconPath = Join-Path $projectRoot 'assets\icons\zelenka.ico'
$source = [System.Drawing.Image]::FromFile((Join-Path $projectRoot 'assets\images\logo.jpg'))
$iconStream = [System.IO.MemoryStream]::new()
$writer = [System.IO.BinaryWriter]::new($iconStream)
$images = @()
foreach ($size in @(16, 32, 48, 64, 128, 256)) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.ScaleTransform($size / 256.0, $size / 256.0)
    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $path.AddArc(0, 0, 104, 104, 180, 90)
    $path.AddArc(152, 0, 104, 104, 270, 90)
    $path.AddArc(152, 152, 104, 104, 0, 90)
    $path.AddArc(0, 152, 104, 104, 90, 90)
    $path.CloseFigure()
    $graphics.SetClip($path)
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.DrawImage($source, 0, 0, 256, 256)
    $memory = [System.IO.MemoryStream]::new()
    $bitmap.Save($memory, [System.Drawing.Imaging.ImageFormat]::Png)
    $images += [PSCustomObject]@{ Size = $size; Data = $memory.ToArray() }
    $memory.Dispose(); $path.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$images.Count)
$offset = 6 + 16 * $images.Count
foreach ($entry in $images) {
    $dimension = if ($entry.Size -eq 256) { 0 } else { $entry.Size }
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension); $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32); $writer.Write([uint32]$entry.Data.Length); $writer.Write([uint32]$offset)
    $offset += $entry.Data.Length
}
foreach ($entry in $images) { $writer.Write([byte[]]$entry.Data) }
$writer.Flush()
$iconBytes = $iconStream.ToArray()
if (-not (Test-Path -LiteralPath $iconPath) -or [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($iconPath)) -ne [Convert]::ToBase64String($iconBytes)) {
    [System.IO.File]::WriteAllBytes($iconPath, $iconBytes)
}
$writer.Dispose(); $source.Dispose()
Write-Output "Created $iconPath"
