<#
.SYNOPSIS
    Bump version across all 4 locations that must stay in sync.
.PARAMETER Version
    The new version string (e.g. "0.2.0").
.EXAMPLE
    powershell scripts\bump-version.ps1 -Version 0.2.0
#>

param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent

$files = @(
    @{
        Path    = Join-Path $root 'Cargo.toml'
        Pattern = '(?<=^\s*version\s*=\s*")[^"]+(?=")'
        Type    = 'regex-first'
    },
    @{
        Path    = Join-Path $root 'apps\gui\tauri.conf.json'
        Pattern = 'version'
        Type    = 'json'
    },
    @{
        Path    = Join-Path $root 'apps\gui\package.json'
        Pattern = 'version'
        Type    = 'json'
    },
    @{
        Path    = Join-Path $root 'apps\gui\ui\package.json'
        Pattern = 'version'
        Type    = 'json'
    }
)

foreach ($f in $files) {
    $path = $f.Path
    if (-not (Test-Path $path)) {
        Write-Warning "Not found: $path"
        continue
    }

    if ($f.Type -eq 'json') {
        $json = Get-Content $path -Raw | ConvertFrom-Json
        $old = $json.version
        $json.version = $Version
        $json | ConvertTo-Json -Depth 20 | Set-Content $path -NoNewline
        Write-Host "  $path : $old -> $Version" -ForegroundColor Green
    }
    elseif ($f.Type -eq 'regex-first') {
        $content = Get-Content $path -Raw
        $old = [regex]::Match($content, $f.Pattern).Value
        # Replace only the first occurrence (workspace.package version)
        $content = [regex]::Replace($content, $f.Pattern, $Version, 1)
        Set-Content $path $content -NoNewline
        Write-Host "  $path : $old -> $Version" -ForegroundColor Green
    }
}

Write-Host "`nVersion bumped to $Version" -ForegroundColor Cyan
Write-Host "Next steps:"
Write-Host "  git commit -am 'release: v$Version'"
Write-Host "  git tag v$Version"
Write-Host "  git push origin main --tags"
