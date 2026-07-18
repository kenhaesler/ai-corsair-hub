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
        Title = "Connect the GUI to the Windows Service over IPC"
        Labels = "enhancement"
        Body = @"
## Description

``apps/service`` now runs the fan-control core under the Windows Service Control Manager. The GUI can still own hardware directly, and a shared named mutex rejects concurrent ownership. Add IPC so the service can remain the single hardware owner while the GUI is open.

## Architecture

- Service binary in ``apps/service/`` remains the single hardware owner
- GUI app (``apps/gui/``) connects via an authenticated local named pipe
- Service handles device discovery, fan control, RGB rendering, sensor polling, and persistence
- GUI handles display, configuration, and user interaction

## Considerations

- Service needs to run as LocalSystem or a service account with USB HID access
- LHM integration may need adjustment (LHM has its own service mode)
- Config changes from GUI need to be sent to service and persisted
- Preserve the ownership mutex as a fail-safe around direct HID access
- Graceful handoff: if the service is not running, the GUI should fall back to direct hardware control

## Acceptance Criteria

- [x] ``apps/service`` installs and runs as a native Windows Service
- [x] Fan control runs without GUI open
- [x] Concurrent direct hardware ownership is rejected before HID discovery
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
        Title = "Extend release version automation"
        Labels = "enhancement,developer-experience"
        Body = @"
## Problem

Release versions are synchronized across the Cargo workspace, Cargo.lock, Tauri config,
both npm packages, and both npm lockfiles by ``scripts/bump-version.ps1``. The release
workflows verify the metadata and tag with ``scripts/check-version.ps1``.

The remaining work is optional release ergonomics.

## Improvements

- [x] Update and verify ``Cargo.lock`` after each bump
- [x] Validate release metadata and tag consistency in CI
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
