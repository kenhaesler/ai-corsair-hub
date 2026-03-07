# Scan for Corsair USB devices (VID 0x1B1C)
Write-Host "`n=== Corsair USB Devices ===" -ForegroundColor Cyan
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_1B1C*' } | Format-Table -Property InstanceId, FriendlyName, Status -AutoSize -Wrap

Write-Host "`n=== Detailed USB Device Info ===" -ForegroundColor Cyan
Get-PnpDevice | Where-Object { $_.InstanceId -like '*VID_1B1C*' } | ForEach-Object {
    $device = $_
    Write-Host "`nDevice: $($device.FriendlyName)" -ForegroundColor Green
    Write-Host "  Instance ID: $($device.InstanceId)"
    Write-Host "  Status: $($device.Status)"
    Write-Host "  Class: $($device.Class)"

    # Extract VID and PID
    if ($device.InstanceId -match 'VID_([0-9A-F]+)&PID_([0-9A-F]+)') {
        Write-Host "  VID: 0x$($Matches[1])" -ForegroundColor Yellow
        Write-Host "  PID: 0x$($Matches[2])" -ForegroundColor Yellow
    }
}

Write-Host "`n=== All HID Devices (for reference) ===" -ForegroundColor Cyan
Get-PnpDevice -Class HIDClass -Status OK | Format-Table -Property InstanceId, FriendlyName -AutoSize -Wrap
