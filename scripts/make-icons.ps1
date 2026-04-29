Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$iconsDir = Join-Path $root "app\icons"
$uiDir = Join-Path $root "app\ui"
if (-not (Test-Path $iconsDir)) {
    New-Item -ItemType Directory -Path $iconsDir | Out-Null
}
if (-not (Test-Path $uiDir)) {
    New-Item -ItemType Directory -Path $uiDir | Out-Null
}

function New-AppIcon {
    param([string]$OutPath, [int]$Size)

    $bmp = New-Object System.Drawing.Bitmap $Size, $Size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

    $bg = [System.Drawing.Color]::FromArgb(255, 31, 105, 211)
    $bgBrush = New-Object System.Drawing.SolidBrush $bg
    $g.FillRectangle($bgBrush, 0, 0, $Size, $Size)

    try {
        $font = New-Object System.Drawing.Font "Segoe UI", ($Size * 0.62), [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel
    } catch {
        $font = New-Object System.Drawing.Font "Arial", ($Size * 0.62), [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel
    }

    $textBrush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $rect = New-Object System.Drawing.RectangleF 0, ($Size * -0.04), $Size, $Size
    $g.DrawString("S", $font, $textBrush, $rect, $sf)

    $g.Dispose()
    $bgBrush.Dispose()
    $textBrush.Dispose()
    $font.Dispose()

    $bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "Wrote $OutPath ($Size x $Size)"
}

New-AppIcon (Join-Path $iconsDir "32x32.png") 32
New-AppIcon (Join-Path $iconsDir "128x128.png") 128
New-AppIcon (Join-Path $iconsDir "128x128@2x.png") 256
New-AppIcon (Join-Path $iconsDir "icon.png") 256

$srcPng = Join-Path $iconsDir "icon.png"
$icoPath = Join-Path $iconsDir "icon.ico"

$bmp = [System.Drawing.Bitmap]::FromFile($srcPng)
$hicon = $bmp.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($hicon)
$fs = [System.IO.File]::Create($icoPath)
$icon.Save($fs)
$fs.Close()
$icon.Dispose()
$bmp.Dispose()
Write-Host "Wrote $icoPath"

$brandIconPath = Join-Path $uiDir "brand-icon.png"
Copy-Item -Force (Join-Path $iconsDir "128x128.png") $brandIconPath
Write-Host "Wrote $brandIconPath"
