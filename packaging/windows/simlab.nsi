Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef STAGE
  !error "STAGE is required"
!endif
!ifndef WEBVIEW2
  !error "WEBVIEW2 must point to the verified offline standalone installer"
!endif
!ifndef OUTPUT
  !error "OUTPUT is required"
!endif
!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef CHECKSCRIPT
  !error "CHECKSCRIPT is required"
!endif

Name "Simulator All-in-One ${VERSION}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\SimLab"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
Var NewDir
Var BackupDir
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Simulator All-in-One requires 64-bit Windows." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
FunctionEnd

Function DetectWebView2
  SetRegView 32
  ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
FunctionEnd

Function AssertNotRunning
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_ROOT", t "$INSTDIR") i .r0'
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_MODE", t "active") i .r0'
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\check-payload.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Close all Simulator All-in-One windows and retry. / 请关闭全部窗口后重试。检查失败：$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
FunctionEnd

Function AssertOwnedTarget
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_ROOT", t "$INSTDIR") i .r0'
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_MODE", t "target") i .r0'
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\check-payload.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Unrecognized files in the target directory were preserved. / 目标目录含未识别的文件，已保留：$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
FunctionEnd

Section "Simulator All-in-One (all eight simulators)" SEC_MAIN
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=check-payload.ps1 "${CHECKSCRIPT}"
  Call AssertNotRunning
  Call AssertOwnedTarget
  Call DetectWebView2
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    SetOutPath "$PLUGINSDIR"
    File /oname=WebView2Runtime.exe "${WEBVIEW2}"
    ExecWait '"$PLUGINSDIR\WebView2Runtime.exe" /silent /install' $1
    ${If} $1 != 0
    ${AndIf} $1 != 3010
      MessageBox MB_ICONSTOP "Offline WebView2 installation failed ($1). Simulator All-in-One was not installed." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
    Call DetectWebView2
    ${If} $0 == ""
    ${OrIf} $0 == "0.0.0.0"
      MessageBox MB_ICONSTOP "WebView2 is still unavailable. Simulator All-in-One was not installed." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
  System::Call 'kernel32::GetCurrentProcessId() i .r8'
  StrCpy $NewDir "$INSTDIR.new.$8"
  StrCpy $BackupDir "$INSTDIR.previous"
  IfFileExists "$NewDir\*" 0 new_directory_available
    MessageBox MB_ICONSTOP "A staging directory already exists; inspect it before retrying. / 安装预备目录已存在，请检查：$NewDir" /SD IDOK
    SetErrorLevel 1
    Abort
new_directory_available:
  IfFileExists "$BackupDir\*" 0 backup_directory_available
    MessageBox MB_ICONSTOP "A previous installation backup still exists; inspect it first. / 上次安装的备份目录仍存在，请先检查：$BackupDir" /SD IDOK
    SetErrorLevel 1
    Abort
backup_directory_available:
  ClearErrors
  SetOutPath "$NewDir"
  File /r "${STAGE}\*"
  WriteUninstaller "$NewDir\Uninstall.exe"
  FileOpen $0 "$NewDir\.simlab-owned" w
  FileWrite $0 "SimLab-Desktop cloud.carldai.simlab"
  FileClose $0
  ${If} ${Errors}
    SetOutPath "$TEMP"
    RMDir /r "$NewDir"
    MessageBox MB_ICONSTOP "Suite preparation failed; the existing installation is unchanged. / 套件准备失败，原版本保持不变。" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_ROOT", t "$NewDir") i .r0'
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_MODE", t "verify") i .r0'
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\check-payload.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    SetOutPath "$TEMP"
    RMDir /r "$NewDir"
    MessageBox MB_ICONSTOP "Suite verification failed; the existing installation is unchanged. / 套件校验失败，原版本保持不变：$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  ; Repeat just before swapping, since extraction can take a while.
  Call AssertNotRunning
  Call AssertOwnedTarget
  SetOutPath "$TEMP"
  ClearErrors
  IfFileExists "$INSTDIR\*" 0 old_directory_moved
    Rename "$INSTDIR" "$BackupDir"
old_directory_moved:
  ${If} ${Errors}
    RMDir /r "$NewDir"
    MessageBox MB_ICONSTOP "Could not prepare the existing installation; setup cancelled. / 无法预备原版本，安装已取消。" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  ClearErrors
  Rename "$NewDir" "$INSTDIR"
  ${If} ${Errors}
    Rename "$BackupDir" "$INSTDIR"
    RMDir /r "$NewDir"
    MessageBox MB_ICONSTOP "Could not replace the suite; recovery was attempted. Inspect: / 无法切换套件，已尝试恢复原版本。请检查：$INSTDIR" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  RMDir /r "$BackupDir"
  Delete "$SMPROGRAMS\SimLab\SimLab.lnk"
  RMDir "$SMPROGRAMS\SimLab"
  CreateDirectory "$SMPROGRAMS\Simulator All-in-One"
  CreateShortcut "$SMPROGRAMS\Simulator All-in-One\Simulator All-in-One.lnk" "$INSTDIR\SAIO.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SimLab" "DisplayName" "Simulator All-in-One"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SimLab" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SimLab" "UninstallString" '"$INSTDIR\Uninstall.exe"'
SectionEnd

Section "Uninstall"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=check-payload.ps1 "${CHECKSCRIPT}"
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_ROOT", t "$INSTDIR") i .r0'
  System::Call 'kernel32::SetEnvironmentVariable(t "SIMLAB_CHECK_MODE", t "owned") i .r0'
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\check-payload.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Ownership or active-window checks failed; all files were preserved. / 无法确认目录归属或仍有窗口运行，已保留全部文件：$1" /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  Delete "$SMPROGRAMS\Simulator All-in-One\Simulator All-in-One.lnk"
  RMDir "$SMPROGRAMS\Simulator All-in-One"
  Delete "$SMPROGRAMS\SimLab\SimLab.lnk"
  RMDir "$SMPROGRAMS\SimLab"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SimLab"
  ; Preserve user configuration/projects and the shared WebView2 runtime.
SectionEnd
