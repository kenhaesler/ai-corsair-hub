<#
.SYNOPSIS
    Creates GitHub issues for Phase 6 deferred work items.
.NOTES
    Requires: gh auth login (run once first)
    Usage: powershell scripts\create-issues.ps1
#>

$ErrorActionPreference = 'Stop'

$issues = @(
    @{
        Title = "Add code signing certificate to eliminate SmartScreen warning"
        Labels = "enhancement,installer"
        Body = @"
## Problem

Without a code signing certificate, Windows SmartScreen shows a warning on first run:
"Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting."

Users must click "More info" then "Run anyway" to proceed. This hurts adoption.

## Options

1. **SignPath Foundation** — Free for open-source projects. Requires approval.
2. **Certum Open Source Code Signing** — ~$70/yr. Faster to obtain.
3. **EV code signing** — $200-400/yr. Immediate SmartScreen reputation. Hardware token required.

## Interim

Document "Click More info -> Run anyway" in README and installer release notes.

## Acceptance Criteria

- [ ] Obtain a code signing certificate
- [ ] Add certificate thumbprint to `apps/gui/tauri.conf.json` bundle.windows.certificateThumbprint
- [ ] CI signs the installer during `tauri-action` build
- [ ] Installer runs without SmartScreen warning
"@
    },
    @{
        Title = "Add auto-start on boot option"
        Labels = "enhancement"
        Body = @"
## Description

Users should be able to configure Corsair Hub to start automatically when Windows boots, so fan control is always active.

## Approaches

1. **NSIS installer option** — Add a checkbox during install to create a registry key at
   ``HKCU\Software\Microsoft\Windows\CurrentVersion\Run``
2. **In-app settings toggle** — Add a toggle in Settings that creates/removes the registry entry
3. **Both** — Install checkbox sets default, app toggle allows changing later

## Acceptance Criteria

- [ ] Toggle in Settings > General: "Start with Windows"
- [ ] Creates/removes registry Run key pointing to Corsair Hub exe
- [ ] App starts minimized to system tray on auto-start (no main window flash)
- [ ] Works with both per-machine and per-user installs
"@
    },
    @{
        Title = "Implement Windows Service daemon (apps/service)"
        Labels = "enhancement"
        Body = @"
## Description

``apps/service`` is currently a stub. A proper Windows Service would allow fan control to run without a logged-in user session (e.g., after reboot before login, or on headless/remote machines).

## Architecture

- Service binary in ``apps/service/`` manages the hardware control loop
- GUI app (``apps/gui/``) becomes a frontend that connects to the service via IPC (named pipe or localhost TCP)
- Service handles: device discovery, fan control, RGB rendering, sensor polling
- GUI handles: display, configuration, user interaction

## Considerations

- Service needs to run as LocalSystem or a service account with USB HID access
- LHM integration may need adjustment (LHM has its own service mode)
- Config changes from GUI need to be sent to service and persisted
- Graceful handoff: if service isn't running, GUI should fall back to direct hardware control

## Acceptance Criteria

- [ ] ``apps/service`` builds as a Windows Service (``sc create`` compatible)
- [ ] Fan control runs without GUI open
- [ ] GUI connects to service for status and configuration
- [ ] Fallback to direct mode when service is unavailable
"@
    },
    @{
        Title = "Publish to Scoop and WinGet package managers"
        Labels = "enhancement,installer"
        Body = @"
## Description

After v1.0, publish Corsair Hub to Windows package managers so users can install with:
- ``winget install corsair-hub``
- ``scoop install corsair-hub``

## Tasks

### WinGet
- [ ] Create manifest YAML (publisher, version, installer URL, hash)
- [ ] Submit PR to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs)
- [ ] Set up automation to submit manifest updates on new releases

### Scoop
- [ ] Create a Scoop bucket or submit to ``extras`` bucket
- [ ] Manifest includes: version, url, hash, shortcuts
- [ ] Auto-update via GitHub Actions on release

## Prerequisites

- Stable release (v1.0+)
- Code signing certificate (reduces SmartScreen issues for winget installs)
"@
    },
    @{
        Title = "Add version bump script for release automation"
        Labels = "enhancement,developer-experience"
        Body = @"
## Problem

Version lives in 4 files that must stay in sync:
1. ``Cargo.toml`` -> ``[workspace.package] version``
2. ``apps/gui/tauri.conf.json`` -> ``version``
3. ``apps/gui/package.json`` -> ``version``
4. ``apps/gui/ui/package.json`` -> ``version``

A basic ``scripts/bump-version.ps1`` was added but could be improved.

## Improvements

- [ ] Add a ``Cargo.lock`` update step (``cargo check`` after bump)
- [ ] Validate that the new version is greater than the current version
- [ ] Optionally create the git commit and tag (``--commit`` flag)
- [ ] Add a cross-platform shell script (``scripts/bump-version.sh``) for CI
- [ ] Consider using ``cargo-release`` or a similar tool for the full workflow
"@
    }
)

foreach ($issue in $issues) {
    $labelArg = if ($issue.Labels) { "--label `"$($issue.Labels)`"" } else { "" }
    Write-Host "Creating issue: $($issue.Title)" -ForegroundColor Yellow

    # Write body to temp file to avoid escaping issues
    $tmpBody = [System.IO.Path]::GetTempFileName()
    $issue.Body | Set-Content $tmpBody -Encoding UTF8

    gh issue create --title $issue.Title --body-file $tmpBody --label $issue.Labels.Split(',')
    Remove-Item $tmpBody

    Write-Host "  Done" -ForegroundColor Green
}

Write-Host "`nAll issues created!" -ForegroundColor Cyan
