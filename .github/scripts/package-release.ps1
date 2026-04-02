param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-PackageVersion {
    param([string]$CargoTomlPath)
    $manifest = Get-Content -LiteralPath $CargoTomlPath -Raw
    if ($manifest -match '(?s)\[package\].*?version\s*=\s*"([^"]+)"') {
        return $matches[1]
    }
    throw "Unable to determine package version from $CargoTomlPath"
}

function Find-Iscc {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
    )

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }

    throw "Inno Setup compiler not found. Install it in the workflow before packaging."
}

function New-AppIcon {
    param(
        [string]$OutputPath
    )

    Add-Type -AssemblyName System.Drawing

    $size = 256
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $background = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.Rectangle 0, 0, $size, $size),
        [System.Drawing.Color]::FromArgb(17, 58, 102),
        [System.Drawing.Color]::FromArgb(11, 106, 130),
        45
    )
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $radius = 48
    $path.AddArc(0, 0, $radius * 2, $radius * 2, 180, 90)
    $path.AddArc($size - ($radius * 2), 0, $radius * 2, $radius * 2, 270, 90)
    $path.AddArc($size - ($radius * 2), $size - ($radius * 2), $radius * 2, $radius * 2, 0, 90)
    $path.AddArc(0, $size - ($radius * 2), $radius * 2, $radius * 2, 90, 90)
    $path.CloseFigure()
    $graphics.FillPath($background, $path)

    $panelBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(30, 255, 255, 255))
    $graphics.FillPath($panelBrush, $path)

    $deviceBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(20, 61, 96))
    $devicePen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(234, 247, 255), 6)

    $body = New-Object System.Drawing.Rectangle 74, 92, 108, 92
    $graphics.FillRectangle($deviceBrush, $body)
    $graphics.DrawRectangle($devicePen, $body)
    $graphics.FillRectangle((New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(102, 214, 255))), 118, 112, 20, 28)
    $graphics.FillRectangle((New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(102, 214, 255))), 130, 140, 24, 10)

    $accentPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(214, 249, 255), 14)
    $accentPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $accentPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $graphics.DrawArc($accentPen, 62, 134, 132, 92, 210, 120)
    $graphics.DrawArc($accentPen, 62, 134, 132, 92, 30, 120)

    $arrowBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(102, 214, 255))
    $graphics.FillPolygon($arrowBrush, @(
        (New-Object System.Drawing.Point 174, 166),
        (New-Object System.Drawing.Point 204, 160),
        (New-Object System.Drawing.Point 184, 186)
    ))
    $graphics.FillPolygon($arrowBrush, @(
        (New-Object System.Drawing.Point 82, 166),
        (New-Object System.Drawing.Point 52, 160),
        (New-Object System.Drawing.Point 72, 186)
    ))

    $handle = $bitmap.GetHicon()
    try {
        $icon = [System.Drawing.Icon]::FromHandle($handle)
        $icon.Save($OutputPath)
    } finally {
        Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NativeMethods {
    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool DestroyIcon(IntPtr hIcon);
}
"@
        [void][NativeMethods]::DestroyIcon($handle)
        $graphics.Dispose()
        $background.Dispose()
        $panelBrush.Dispose()
        $deviceBrush.Dispose()
        $devicePen.Dispose()
        $accentPen.Dispose()
        $arrowBrush.Dispose()
        $path.Dispose()
        $bitmap.Dispose()
    }
}

$cargoToml = Join-Path $RepoRoot "Cargo.toml"
$version = Get-PackageVersion -CargoTomlPath $cargoToml
$releaseDir = Join-Path $RepoRoot "target\release"

Write-Host "Building release binary..."
Push-Location $RepoRoot
try {
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed."
    }
} finally {
    Pop-Location
}

$exePath = Join-Path $releaseDir "usb_mirror_sync.exe"
if (-not (Test-Path -LiteralPath $exePath)) {
    throw "Release executable not found at $exePath. Build the project first."
}

$iconPath = Join-Path $releaseDir "usb_mirror_sync.ico"
New-AppIcon -OutputPath $iconPath

$portableRoot = Join-Path $releaseDir "portable"
if (Test-Path -LiteralPath $portableRoot) {
    Remove-Item -LiteralPath $portableRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
Copy-Item -LiteralPath $exePath -Destination (Join-Path $portableRoot "usb_mirror_sync.exe") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "README.md") -Destination (Join-Path $portableRoot "README.md") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "config.example.json") -Destination (Join-Path $portableRoot "config.example.json") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "assets\setup_wizard.ps1") -Destination (Join-Path $portableRoot "setup_wizard.ps1") -Force

$portableZip = Join-Path $releaseDir ("usb_mirror_sync-portable-v{0}.zip" -f $version)
if (Test-Path -LiteralPath $portableZip) {
    Remove-Item -LiteralPath $portableZip -Force
}
Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath $portableZip -Force

$iscc = Find-Iscc
$installerScript = Join-Path $RepoRoot ".github\installer\usb_mirror_sync.iss"
$installerBase = "usb_mirror_sync-setup-v$version"
& $iscc "/DAppVersion=$version" "/DSourceExe=$exePath" "/DAppIcon=$iconPath" "/DOutputDir=$releaseDir" "/DOutputBase=$installerBase" $installerScript
if ($LASTEXITCODE -ne 0) {
    throw "Installer compilation failed."
}

$installerExe = Join-Path $releaseDir "$installerBase.exe"
if (-not (Test-Path -LiteralPath $installerExe)) {
    throw "Expected installer output was not found at $installerExe"
}

Write-Host "Packaged version $version"
Write-Host "Portable: $portableZip"
Write-Host "Installer: $installerExe"
