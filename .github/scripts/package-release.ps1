param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path,
    [string]$TargetTriple = "x86_64-pc-windows-msvc",
    [string]$ArchLabel = "x64",
    [string]$InstallerArchitecture = "x64compatible"
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

function Export-EmbeddedAppIcon {
    param(
        [string]$ExePath,
        [string]$OutputPath
    )

    Add-Type -AssemblyName System.Drawing

    $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($ExePath)
    if ($null -eq $icon) {
        throw "Failed to extract embedded icon from $ExePath"
    }
    try {
        $fileStream = [System.IO.File]::Create($OutputPath)
        try {
            $icon.Save($fileStream)
        } finally {
            $fileStream.Dispose()
        }
    } finally {
        $icon.Dispose()
    }
}

$cargoToml = Join-Path $RepoRoot "Cargo.toml"
$version = Get-PackageVersion -CargoTomlPath $cargoToml
$releaseDir = Join-Path $RepoRoot ("target\{0}\release" -f $TargetTriple)

Write-Host "Building release binary..."
Push-Location $RepoRoot
try {
    & cargo build --release --locked --target $TargetTriple
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed for $TargetTriple."
    }
} finally {
    Pop-Location
}

$exePath = Join-Path $releaseDir "shadowsync.exe"
if (-not (Test-Path -LiteralPath $exePath)) {
    throw "Release executable not found at $exePath. Build the project first."
}

$iconPath = Join-Path $releaseDir "shadowsync.ico"
Export-EmbeddedAppIcon -ExePath $exePath -OutputPath $iconPath

$portableRoot = Join-Path $releaseDir "portable"
if (Test-Path -LiteralPath $portableRoot) {
    Remove-Item -LiteralPath $portableRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $portableRoot -Force | Out-Null
Copy-Item -LiteralPath $exePath -Destination (Join-Path $portableRoot "shadowsync.exe") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "README.md") -Destination (Join-Path $portableRoot "README.md") -Force
Copy-Item -LiteralPath (Join-Path $RepoRoot "config.example.json") -Destination (Join-Path $portableRoot "config.example.json") -Force

$portableZip = Join-Path $releaseDir ("shadowsync-windows-{0}-portable-v{1}.zip" -f $ArchLabel, $version)
if (Test-Path -LiteralPath $portableZip) {
    Remove-Item -LiteralPath $portableZip -Force
}
Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath $portableZip -Force

$iscc = Find-Iscc
$installerScript = Join-Path $RepoRoot ".github\installer\shadowsync.iss"
$installerBase = "shadowsync-windows-$ArchLabel-setup-v$version"
& $iscc "/DAppVersion=$version" "/DSourceExe=$exePath" "/DAppIcon=$iconPath" "/DOutputDir=$releaseDir" "/DOutputBase=$installerBase" "/DArchitecturesAllowed=$InstallerArchitecture" "/DArchitecturesInstallIn64BitMode=$InstallerArchitecture" $installerScript
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
