<#
.SYNOPSIS
    Verify that release metadata uses one version and optionally matches a tag.
.PARAMETER Tag
    Git tag to validate. Release tags must be exactly "v<workspace version>".
.EXAMPLE
    powershell scripts\check-version.ps1 -Tag v0.2.0
#>

param(
    [string]$Tag
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot -Parent
$cargoManifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
$workspaceMatch = [regex]::Match(
    $cargoManifest,
    '(?ms)^\[workspace\.package\]\s*.*?^version\s*=\s*"([^"]+)"'
)

if (-not $workspaceMatch.Success) {
    throw 'Could not read [workspace.package].version from Cargo.toml'
}

$expected = $workspaceMatch.Groups[1].Value
$metadata = [ordered]@{
    'Cargo.toml [workspace.package]' = $expected
}

function Add-JsonVersion {
    param(
        [Parameter(Mandatory)]
        [string]$RelativePath
    )

    $path = Join-Path $root $RelativePath
    $json = Get-Content $path -Raw | ConvertFrom-Json
    $script:metadata[$RelativePath] = [string]$json.version

}

function Add-LockfileVersions {
    param(
        [Parameter(Mandatory)]
        [string]$RelativePath
    )

    # Windows PowerShell 5.1 cannot ConvertFrom-Json an object with the empty
    # package key used by npm lockfiles, so read the first two root versions.
    $path = Join-Path $root $RelativePath
    $matches = [regex]::Matches(
        (Get-Content $path -Raw),
        '(?m)^\s*"version"\s*:\s*"([^"]+)"'
    )
    if ($matches.Count -lt 2) {
        throw "Could not read root package versions from $RelativePath"
    }

    $script:metadata[$RelativePath] = $matches[0].Groups[1].Value
    $script:metadata["$RelativePath packages['']"] = $matches[1].Groups[1].Value
}

Add-JsonVersion 'apps\gui\tauri.conf.json'
Add-JsonVersion 'apps\gui\package.json'
Add-LockfileVersions 'apps\gui\package-lock.json'
Add-JsonVersion 'apps\gui\ui\package.json'
Add-LockfileVersions 'apps\gui\ui\package-lock.json'

$mismatches = @(
    $metadata.GetEnumerator() |
        Where-Object { $_.Value -ne $expected } |
        ForEach-Object { "  $($_.Key): $($_.Value) (expected $expected)" }
)

if ($mismatches.Count -gt 0) {
    throw "Version metadata is inconsistent:`n$($mismatches -join "`n")"
}

# A locked metadata read fails when Cargo.lock does not reflect the manifests,
# including workspace package version changes.
Push-Location $root
try {
    & cargo metadata --locked --no-deps --format-version 1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'Cargo.lock is not synchronized with the workspace manifests'
    }
} finally {
    Pop-Location
}

if ($Tag) {
    $expectedTag = "v$expected"
    if ($Tag -ne $expectedTag) {
        throw "Release tag '$Tag' does not match workspace version '$expectedTag'"
    }
}

Write-Host "Version metadata is consistent: $expected" -ForegroundColor Green
if ($Tag) {
    Write-Host "Release tag matches: $Tag" -ForegroundColor Green
}
