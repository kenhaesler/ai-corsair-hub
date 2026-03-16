#Requires -RunAsAdministrator
<#
.SYNOPSIS
    One-command developer setup for ai-corsair-hub.
.DESCRIPTION
    Installs Rust, Node.js, MSVC Build Tools, and LibreHardwareMonitor via winget,
    then runs npm ci for all frontend dependencies.
.NOTES
    Run as Administrator: powershell -ExecutionPolicy Bypass scripts\dev-setup.ps1
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Write-Host "`n=== Corsair Hub Developer Setup ===" -ForegroundColor Cyan

# --- Install toolchain via winget ---

$packages = @(
    @{ Id = 'Rustlang.Rustup';                          Name = 'Rust (rustup)' },
    @{ Id = 'OpenJS.NodeJS.LTS';                         Name = 'Node.js LTS' },
    @{ Id = 'Microsoft.VisualStudio.2022.BuildTools';    Name = 'MSVC Build Tools 2022' },
    @{ Id = 'LibreHardwareMonitor.LibreHardwareMonitor'; Name = 'LibreHardwareMonitor' }
)

foreach ($pkg in $packages) {
    Write-Host "`nInstalling $($pkg.Name)..." -ForegroundColor Yellow
    winget install $pkg.Id --silent --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
        # -1978335189 = already installed
        Write-Warning "$($pkg.Name) install returned exit code $LASTEXITCODE"
    }
}

# Refresh PATH for current session
$env:Path = [System.Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
            [System.Environment]::GetEnvironmentVariable('Path', 'User')

# --- Install MSVC C++ workload if Build Tools was just installed ---
Write-Host "`nEnsuring MSVC C++ workload is available..." -ForegroundColor Yellow
$vsInstaller = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
if (Test-Path $vsInstaller) {
    & $vsInstaller modify --installPath "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools" `
        --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --norestart 2>$null
}

# --- Install npm dependencies ---
Write-Host "`nInstalling Tauri CLI dependencies..." -ForegroundColor Yellow
Push-Location (Join-Path $PSScriptRoot '..\apps\gui')
npm ci
Pop-Location

Write-Host "`nInstalling frontend dependencies..." -ForegroundColor Yellow
Push-Location (Join-Path $PSScriptRoot '..\apps\gui\ui')
npm ci
Pop-Location

# --- Done ---
Write-Host "`n=== Setup Complete ===" -ForegroundColor Green
Write-Host @"

Quick reference:
  Build:    cd apps/gui && npm run build
  Dev:      cd apps/gui && npm run dev
  Bundle:   cd apps/gui && npx tauri build
  Test:     cargo test
  Scanner:  cargo run --bin corsair-scanner

"@ -ForegroundColor Cyan
