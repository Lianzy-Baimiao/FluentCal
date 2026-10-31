; FluentCal installer for the GitHub download (the Store gets an MSIX). Per user, no admin:
; the same folder, uninstall key and Run value as 0.2's Tauri installer, so installing this
; over 0.2 upgrades it in place. Data (%APPDATA%\com.fluentcal.desktop) is never touched.
; Built by scripts/build-native.sh: makensis /DVERSION=x.y.z /DEXE=<path> /DOUT=<setup.exe>

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define NAME "FluentCal"
!define UNINST "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NAME}"
!define MSGCLASS "com.fluentcal.desktop-sic"

Name "${NAME} 桌面日历"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\${NAME}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${NAME} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=2052 "ProductName" "${NAME}"
VIAddVersionKey /LANG=2052 "FileDescription" "${NAME} 安装程序"
VIAddVersionKey /LANG=2052 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "ProductVersion" "${VERSION}"

!define MUI_ICON "..\res\icon.ico"
!define MUI_UNICON "..\res\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\fluentcal.exe"
!define MUI_FINISHPAGE_RUN_TEXT "启动 FluentCal"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"

; Asks a running FluentCal (0.2 or this one) to exit, then makes sure it has.
!macro CloseRunning
  FindWindow $0 "${MSGCLASS}" ""
  StrCmp $0 0 +3
    SendMessage $0 ${WM_CLOSE} 0 0 /TIMEOUT=3000
    Sleep 1500
  nsExec::Exec 'taskkill /IM fluentcal.exe /F'
  Pop $0
  Sleep 300
!macroend

Section "FluentCal"
  !insertmacro CloseRunning
  SetOutPath "$INSTDIR"
  File "/oname=fluentcal.exe" "${EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${NAME}.lnk" "$INSTDIR\fluentcal.exe"
  WriteRegStr HKCU "${UNINST}" "DisplayName" "${NAME}"
  WriteRegStr HKCU "${UNINST}" "DisplayIcon" '"$INSTDIR\fluentcal.exe"'
  WriteRegStr HKCU "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}" "Publisher" "${NAME}"
  WriteRegStr HKCU "${UNINST}" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${UNINST}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST}" "MainBinaryName" "fluentcal.exe"
  WriteRegDWORD HKCU "${UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINST}" "EstimatedSize" $0
SectionEnd

Section "Uninstall"
  !insertmacro CloseRunning
  Delete "$INSTDIR\fluentcal.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${NAME}.lnk"
  DeleteRegKey HKCU "${UNINST}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${NAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${NAME}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\com.fluentcal.desktop"
SectionEnd
