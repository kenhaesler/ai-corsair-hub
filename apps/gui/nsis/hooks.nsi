; Corsair Hub NSIS installer hooks
; Auto-installs LibreHardwareMonitor for CPU temperature monitoring

!macro NSIS_HOOK_POSTINSTALL
  ; Check if LibreHardwareMonitor is already installed
  IfFileExists "$PROGRAMFILES64\LibreHardwareMonitor\LibreHardwareMonitor.exe" lhm_skip
  IfFileExists "$PROGRAMFILES\LibreHardwareMonitor\LibreHardwareMonitor.exe" lhm_skip

  DetailPrint "Installing LibreHardwareMonitor for CPU temperature monitoring..."
  ExecWait 'cmd /c winget install LibreHardwareMonitor.LibreHardwareMonitor --silent --accept-package-agreements --accept-source-agreements' $0
  DetailPrint "LibreHardwareMonitor install exit code: $0"

  lhm_skip:
!macroend
