<#
.SYNOPSIS
    Build the NSIS installer for Corsair Hub.
.DESCRIPTION
    Runs `npx tauri build` from apps/gui to produce the installer
    in target/release/bundle/nsis/.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Push-Location (Join-Path $PSScriptRoot '..\apps\gui')
try {
    npx tauri build
} finally {
    Pop-Location
}
