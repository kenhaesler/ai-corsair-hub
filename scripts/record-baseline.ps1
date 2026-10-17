<#
.SYNOPSIS
  Records a machine-local hardware baseline (PR 0 evidence) for ai-corsair-hub.

.DESCRIPTION
  Creates <OutDir>\<yyyyMMdd-HHmmss>\ and writes baseline.json plus copies of:
    * the working config.toml (with its SHA-256),
    * the newest led-manifest-*.json produced by the "LED markers" diagnostic.
  baseline.json records the git commit and dirty state, the GUI executable
  (path, size, version info, SHA-256), the config hash, Windows build and
  rustc version.

  Read-only with respect to the system: it does NOT talk to USB devices,
  start the app, or modify any file outside the new baseline folder.
  Windows PowerShell 5.1 compatible.

.PARAMETER ExePath
  GUI executable to fingerprint. Default: the newest of
  <repo>\target\release\corsair-hub-gui.exe and the installed app (looked up
  from productName in apps\gui\tauri.conf.json under %LOCALAPPDATA% and
  %ProgramFiles%). Required if none can be found.

.PARAMETER OutDir
  Root folder for baselines. Default: %LOCALAPPDATA%\corsair-hub\baselines.

.PARAMETER ConfigDir
  Folder holding config.toml and diagnostics\. Default: %APPDATA%\corsair-hub.

.EXAMPLE
  .\scripts\record-baseline.ps1
.EXAMPLE
  .\scripts\record-baseline.ps1 -ExePath "C:\path\to\corsair-hub-gui.exe"
#>
[CmdletBinding()]
param(
    [string]$ExePath,
    [string]$OutDir,
    [string]$ConfigDir
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $OutDir)    { $OutDir    = Join-Path $env:LOCALAPPDATA 'corsair-hub\baselines' }
if (-not $ConfigDir) { $ConfigDir = Join-Path $env:APPDATA 'corsair-hub' }

$warnings = New-Object System.Collections.Generic.List[string]
function Add-Warn([string]$msg) {
    $warnings.Add($msg)
    Write-Warning $msg
}

function Get-Sha256([string]$path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
}

# --- Locate the GUI executable ------------------------------------------------
if (-not $ExePath) {
    $candidates = New-Object System.Collections.Generic.List[string]
    $candidates.Add((Join-Path $repoRoot 'target\release\corsair-hub-gui.exe'))
    $productName = $null
    $tauriConf = Join-Path $repoRoot 'apps\gui\tauri.conf.json'
    if (Test-Path -LiteralPath $tauriConf) {
        try { $productName = (Get-Content -LiteralPath $tauriConf -Raw | ConvertFrom-Json).productName } catch { }
    }
    if ($productName) {
        $roots = @($env:LOCALAPPDATA, $env:ProgramFiles, ${env:ProgramFiles(x86)}) | Where-Object { $_ }
        foreach ($root in $roots) {
            foreach ($name in @($productName, 'corsair-hub-gui')) {
                $candidates.Add((Join-Path (Join-Path $root $productName) ($name + '.exe')))
                $candidates.Add((Join-Path (Join-Path (Join-Path $root 'Programs') $productName) ($name + '.exe')))
            }
        }
    }
    $found = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -Unique |
        ForEach-Object { Get-Item -LiteralPath $_ } | Sort-Object LastWriteTime -Descending
    if (-not $found) {
        throw "Could not find the GUI executable automatically. Pass -ExePath <path to corsair-hub-gui.exe>."
    }
    $ExePath = @($found)[0].FullName
}
if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "ExePath not found: $ExePath"
}

# --- Create the baseline folder (the only write location) --------------------
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$dest = Join-Path $OutDir $stamp
if (Test-Path -LiteralPath $dest) { throw "Baseline folder already exists: $dest" }
New-Item -ItemType Directory -Path $dest | Out-Null

# --- Git ---------------------------------------------------------------------
$git = @{ commit = $null; branch = $null; dirty = $null; status = @() }
try {
    $git.commit = (& git -C $repoRoot rev-parse HEAD).Trim()
    $git.branch = (& git -C $repoRoot rev-parse --abbrev-ref HEAD).Trim()
    $porcelain = @(& git -C $repoRoot status --porcelain)
    $git.dirty = ($porcelain.Count -gt 0)
    $git.status = $porcelain
    if ($git.dirty) { Add-Warn 'Working tree is dirty: the recorded commit does not fully describe the built source.' }
} catch {
    Add-Warn "git information unavailable: $($_.Exception.Message)"
}

# --- Executable --------------------------------------------------------------
$exeItem = Get-Item -LiteralPath $ExePath
$exeInfo = [ordered]@{
    path            = $exeItem.FullName
    size_bytes      = $exeItem.Length
    last_write_time = $exeItem.LastWriteTime.ToString('o')
    file_version    = $exeItem.VersionInfo.FileVersion
    product_version = $exeItem.VersionInfo.ProductVersion
    product_name    = $exeItem.VersionInfo.ProductName
    sha256          = Get-Sha256 $exeItem.FullName
}

# --- Config copy -------------------------------------------------------------
$configInfo = [ordered]@{ source = $null; copied_as = $null; sha256 = $null }
$configSrc = Join-Path $ConfigDir 'config.toml'
if (Test-Path -LiteralPath $configSrc) {
    Copy-Item -LiteralPath $configSrc -Destination (Join-Path $dest 'config.toml')
    $configInfo.source = $configSrc
    $configInfo.copied_as = 'config.toml'
    $configInfo.sha256 = Get-Sha256 (Join-Path $dest 'config.toml')
} else {
    Add-Warn "config.toml not found at $configSrc (nothing to back up)."
}

# --- Newest LED manifest -----------------------------------------------------
$manifestInfo = [ordered]@{ source = $null; copied_as = $null; sha256 = $null }
$diagDir = Join-Path $ConfigDir 'diagnostics'
$manifest = $null
if (Test-Path -LiteralPath $diagDir) {
    $manifest = Get-ChildItem -LiteralPath $diagDir -Filter 'led-manifest-*.json' -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
}
if ($manifest) {
    Copy-Item -LiteralPath $manifest.FullName -Destination (Join-Path $dest $manifest.Name)
    $manifestInfo.source = $manifest.FullName
    $manifestInfo.copied_as = $manifest.Name
    $manifestInfo.sha256 = Get-Sha256 (Join-Path $dest $manifest.Name)
} else {
    Add-Warn "No led-manifest-*.json found in $diagDir - run the LED marker diagnostic first (Devices > LED markers)."
}

# --- Toolchain / OS ----------------------------------------------------------
$rustc = $null
try {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path -LiteralPath $cargoBin) { $env:PATH = $cargoBin + ';' + $env:PATH }
    $rustc = ((& rustc -V) | Out-String).Trim()
} catch {
    Add-Warn 'rustc not found on PATH.'
}
$osInfo = [ordered]@{ version = [Environment]::OSVersion.VersionString }
try {
    $cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $osInfo.product_name = $cv.ProductName
    $osInfo.display_version = $cv.DisplayVersion
    $osInfo.build = ('{0}.{1}' -f $cv.CurrentBuild, $cv.UBR)
} catch {
    Add-Warn 'Could not read the Windows build from the registry.'
}

# --- baseline.json -----------------------------------------------------------
$baseline = [ordered]@{
    recorded_at_local = (Get-Date).ToString('o')
    recorded_by_script = 'scripts/record-baseline.ps1'
    git          = $git
    executable   = $exeInfo
    config       = $configInfo
    led_manifest = $manifestInfo
    windows      = $osInfo
    rustc        = $rustc
    physical_verification = 'PENDING - markers/photos must be checked by the operator (see docs/hardware-baseline.md)'
    warnings     = @($warnings)
}
$json = $baseline | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText((Join-Path $dest 'baseline.json'), $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host ''
Write-Host 'Baseline recorded in:'
Write-Host $dest
