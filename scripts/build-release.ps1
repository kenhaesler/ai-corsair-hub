<#
.SYNOPSIS
    Build the NSIS installer for Corsair Hub.
.DESCRIPTION
    Builds the Windows service, then runs `npx tauri build` from apps/gui to
    produce the installer in target/release/bundle/nsis/.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'check-version.ps1')

Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo build --locked --release --bin corsair-service
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to build corsair-service'
    }
} finally {
    Pop-Location
}

Push-Location (Join-Path $PSScriptRoot '..\apps\gui')
try {
    npx tauri build --config tauri.release.conf.json
} finally {
    Pop-Location
}
