!include "MUI2.nsh"
!include "x64.nsh"

Unicode true
RequestExecutionLevel admin

!define PRODUCT_NAME "CyberMonitor Suite"
!define COMPANY_NAME "CyberDesktop"
!define MONITOR_EXE "cyber_monitor.exe"
!define HOST_EXE "cyber_monitor_host.exe"
!define INSTALL_SUBDIR "CyberDesktop\CyberMonitor"

!ifndef OUTPUT_DIR
    !define OUTPUT_DIR "."
!endif

!ifndef PRODUCT_VERSION
    !define PRODUCT_VERSION "0.0.0"
!endif

!ifndef INSTALLER_BASENAME
    !define INSTALLER_BASENAME "CyberMonitorSuite"
!endif

Name "${PRODUCT_NAME}"
OutFile "${OUTPUT_DIR}\${INSTALLER_BASENAME}_${PRODUCT_VERSION}_Setup.exe"
InstallDir "$PROGRAMFILES64\${INSTALL_SUBDIR}"

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"

Section "CyberMonitor" SecMain
    SectionIn RO
    SetOutPath "$INSTDIR"

    nsExec::ExecToLog 'taskkill /F /T /IM ${MONITOR_EXE}'
    nsExec::ExecToLog 'taskkill /F /T /IM ${HOST_EXE}'

    File "${OUTPUT_DIR}\app\app.7z"
    Nsis7z::ExtractWithCallback "$INSTDIR\app.7z" $R9
    Delete "$INSTDIR\app.7z"

    CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
    CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\CyberMonitor.lnk" "$INSTDIR\${MONITOR_EXE}"
    CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\CyberMonitor Host.lnk" "$INSTDIR\${HOST_EXE}"
    CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\卸载.lnk" "$INSTDIR\Uninstall.exe"

    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "Publisher" "${COMPANY_NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayVersion" "${PRODUCT_VERSION}"

    ExecShell "" "$INSTDIR\${MONITOR_EXE}"

    WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "开机自动启动" SecAutostart
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CyberMonitor" "$\"$INSTDIR\${MONITOR_EXE}$\" --startup"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CyberMonitorHost" "$\"$INSTDIR\${HOST_EXE}$\" --startup"
SectionEnd

Section "Uninstall"
    nsExec::ExecToLog 'taskkill /F /T /IM ${MONITOR_EXE}'
    nsExec::ExecToLog 'taskkill /F /T /IM ${HOST_EXE}'

    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CyberMonitor"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CyberMonitorHost"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"

    Delete "$SMPROGRAMS\${PRODUCT_NAME}\CyberMonitor.lnk"
    Delete "$SMPROGRAMS\${PRODUCT_NAME}\CyberMonitor Host.lnk"
    Delete "$SMPROGRAMS\${PRODUCT_NAME}\卸载.lnk"
    RMDir "$SMPROGRAMS\${PRODUCT_NAME}"

    Delete "$INSTDIR\${MONITOR_EXE}"
    Delete "$INSTDIR\${HOST_EXE}"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
SectionEnd
