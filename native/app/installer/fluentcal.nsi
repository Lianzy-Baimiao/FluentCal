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

Name "${NAME}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\${NAME}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${NAME} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=2052 "ProductName" "${NAME}"
VIAddVersionKey /LANG=2052 "FileDescription" "${NAME} Setup"
VIAddVersionKey /LANG=2052 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "ProductVersion" "${VERSION}"

!define MUI_ICON "..\res\icon.ico"
!define MUI_UNICON "..\res\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\fluentcal.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(LaunchFluentCal)"
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${UNINST}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "InstallerLanguage"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
; English is the fallback for unsupported Windows languages.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "TradChinese"
!insertmacro MUI_RESERVEFILE_LANGDLL
LangString LaunchFluentCal ${LANG_ENGLISH} "Launch FluentCal"
LangString LaunchFluentCal ${LANG_SIMPCHINESE} "启动 FluentCal"
LangString LaunchFluentCal ${LANG_TRADCHINESE} "啟動 FluentCal"

Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function un.onInit
  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd

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
  ; 0.2's WebView2 loader: this version has no WebView2, and RMDir would leave it behind.
  Delete "$INSTDIR\WebView2Loader.dll"
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
  Delete "$INSTDIR\WebView2Loader.dll"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${NAME}.lnk"
  DeleteRegKey HKCU "${UNINST}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${NAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${NAME}"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\com.fluentcal.desktop"
SectionEnd
