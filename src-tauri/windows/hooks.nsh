; Remove the autostart entry written by tauri-plugin-autostart, so uninstalling does not
; leave a Run key pointing at a deleted executable.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FluentCal"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "FluentCal"
!macroend
