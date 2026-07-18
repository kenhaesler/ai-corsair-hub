<#
.SYNOPSIS
    Bump version across all package and lockfile metadata that must stay in sync.
.PARAMETER Version
    The new version string (e.g. "0.2.0").
.EXAMPLE
    powershell scripts\bump-version.ps1 -Version 0.2.0
#>

param(
    [Parameter(Mandatory)]
    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$')]
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [string]$Content
    )

    [System.IO.File]::WriteAllText($Path, $Content, $script:utf8NoBom)
}

$files = @(
    @{
        Path    = Join-Path $root 'Cargo.toml'
        Pattern = '(?ms)^\[workspace\.package\]\s*.*?^version\s*=\s*"([^"]+)"'
        Type    = 'toml-workspace'
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
        Path    = Join-Path $root 'apps\gui\package-lock.json'
        Pattern = 'version'
        Type    = 'json-lock'
    },
    @{
        Path    = Join-Path $root 'apps\gui\ui\package.json'
        Pattern = 'version'
        Type    = 'json'
    },
    @{
        Path    = Join-Path $root 'apps\gui\ui\package-lock.json'
        Pattern = 'version'
        Type    = 'json-lock'
    }
)

foreach ($f in $files) {
    $path = $f.Path
    if (-not (Test-Path $path)) {
        Write-Warning "Not found: $path"
        continue
    }

    if ($f.Type -eq 'json') {
        $content = Get-Content $path -Raw
        $json = $content | ConvertFrom-Json
        $old = $json.version
        $match = [regex]::Match(
            $content,
            '(?m)^\s*"version"\s*:\s*"([^"]+)"'
        )
        if (-not $match.Success) {
            throw "Could not read top-level version from $path"
        }

        $value = $match.Groups[1]
        $content = $content.Remove($value.Index, $value.Length).Insert($value.Index, $Version)
        Write-Utf8NoBom $path $content
        Write-Host "  $path : $old -> $Version" -ForegroundColor Green
    }
    elseif ($f.Type -eq 'json-lock') {
        $content = Get-Content $path -Raw
        $matches = [regex]::Matches(
            $content,
            '(?m)^\s*"version"\s*:\s*"([^"]+)"'
        )
        if ($matches.Count -lt 2) {
            throw "Could not read root package versions from $path"
        }

        $old = $matches[0].Groups[1].Value
        for ($i = 1; $i -ge 0; $i--) {
            $value = $matches[$i].Groups[1]
            $content = $content.Remove($value.Index, $value.Length).Insert($value.Index, $Version)
        }
        Write-Utf8NoBom $path $content
        Write-Host "  $path : $old -> $Version" -ForegroundColor Green
    }
    elseif ($f.Type -eq 'toml-workspace') {
        $content = Get-Content $path -Raw
        $match = [regex]::Match($content, $f.Pattern)
        if (-not $match.Success) {
            throw "Could not read [workspace.package].version from $path"
        }

        $value = $match.Groups[1]
        $old = $value.Value
        $content = $content.Remove($value.Index, $value.Length).Insert($value.Index, $Version)
        Write-Utf8NoBom $path $content
        Write-Host "  $path : $old -> $Version" -ForegroundColor Green
    }
}

# Let Cargo update the workspace package versions recorded in Cargo.lock without
# refreshing unrelated dependencies.
Push-Location $root
try {
    & cargo metadata --no-deps --format-version 1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed while updating Cargo.lock"
    }
} finally {
    Pop-Location
}

& (Join-Path $PSScriptRoot 'check-version.ps1')

Write-Host "`nVersion bumped to $Version" -ForegroundColor Cyan
Write-Host "Next steps:"
Write-Host "  git commit -am 'release: v$Version'"
Write-Host "  git tag v$Version"
Write-Host "  git push origin HEAD"
Write-Host "  git push origin v$Version"
