Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WordFunc.nsh"
!include "WinVer.nsh"
!include "x64.nsh"
!include "Sections.nsh"

!ifndef APP_VERSION
  !error "Use tools/build-windows-installer.ps1"
!endif
!define APP_ID "dev.oicha.writer"
!define APP_KEY "Software\Oicha"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Oicha"
!define MARKER "Oicha-8c8c69a3-7cc9-4f10-88da-25b2597bc46a"
Name "Oicha"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Oicha"
InstallDirRegKey HKCU "${APP_KEY}" "InstallDir"
RequestExecutionLevel user
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2
ManifestSupportedOS Win10
CRCCheck force
SetCompressor /SOLID lzma
SetCompressorDictSize 32
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Oicha"
VIAddVersionKey /LANG=1033 "CompanyName" "Oicha contributors"
VIAddVersionKey /LANG=1033 "FileDescription" "Oicha Windows Installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Copyright 2026 Oicha contributors"
!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${WIZARD_BITMAP}"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${WIZARD_BITMAP}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\Oicha.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Mở Oicha"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Vietnamese"

Var SetupMutex
Var OldDesktop
Var OldMenu
Var OldAssociation

!macro AcquireMutex
  UserInfo::GetName
  Pop $0
  System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Local\Oicha.Setup.$0") p .r1 ?e'
  Pop $0
  StrCpy $SetupMutex $1
  ${If} $1 = 0
    SetErrorLevel 2
    Abort "Không thể khóa phiên cài đặt Oicha."
  ${EndIf}
  ${If} $0 = 183
    SetErrorLevel 2
    Abort "Một bộ cài/gỡ Oicha khác đang chạy."
  ${EndIf}
!macroend

!macro CheckWritable Prefix
Function ${Prefix}CheckWritable
  IfFileExists "$INSTDIR\Oicha.exe" 0 ready
retry:
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\Oicha.exe", i 0x40000000, i 7, p 0, i 3, i 0, p 0) p .r0'
  ${If} $0 = -1
    IfSilent blocked
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Hãy đóng Oicha để lưu bản thảo, rồi nhấn Thử lại. Nếu app đã đóng, hãy kiểm tra quyền ghi thư mục cài đặt." IDRETRY retry
blocked:
    SetErrorLevel 2
    Abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
ready:
FunctionEnd
!macroend
!insertmacro CheckWritable ""
!insertmacro CheckWritable "un."

Section "Oicha (bắt buộc)" Core
  SectionIn RO
  Call CheckWritable
  SetOutPath "$INSTDIR"
  ClearErrors
  File /oname=Oicha.exe.installing "${PAYLOAD_DIR}\Oicha.exe"
  ${If} ${Errors}
    SetErrorLevel 2
    Abort "Không thể ghi app mới."
  ${EndIf}
  !include "${PAYLOAD_INCLUDE}"
  ${If} ${Errors}
    Delete "$INSTDIR\Oicha.exe.installing"
    SetErrorLevel 2
    Abort "Không thể ghi đủ thành phần cài đặt; app trước được giữ lại."
  ${EndIf}
  SetOutPath "$INSTDIR"
  ClearErrors
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  ${If} ${Errors}
    Delete "$INSTDIR\Oicha.exe.installing"
    SetErrorLevel 2
    Abort "Không thể tạo trình gỡ cài đặt."
  ${EndIf}
  ClearErrors
  IfFileExists "$INSTDIR\Oicha.exe" 0 replace
  Delete "$INSTDIR\Oicha.exe.previous"
  Rename "$INSTDIR\Oicha.exe" "$INSTDIR\Oicha.exe.previous"
  ${If} ${Errors}
    Delete "$INSTDIR\Oicha.exe.installing"
    SetErrorLevel 2
    Abort "Không thể thay app đang cài."
  ${EndIf}
replace:
  ClearErrors
  Rename "$INSTDIR\Oicha.exe.installing" "$INSTDIR\Oicha.exe"
  ${If} ${Errors}
    Rename "$INSTDIR\Oicha.exe.previous" "$INSTDIR\Oicha.exe"
    SetErrorLevel 2
    Abort "Không thể hoàn tất cập nhật; app trước được giữ lại."
  ${EndIf}
  Delete "$INSTDIR\Oicha.exe.previous"
  WriteRegStr HKCU "${APP_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${APP_KEY}" "Version" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Oicha"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Oicha contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\Oicha.exe,0"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" ${INSTALLED_KIB}
SectionEnd

Section "Shortcut trong Start Menu" StartMenu
  CreateDirectory "$SMPROGRAMS\Oicha"
  CreateShortcut "$SMPROGRAMS\Oicha\Oicha.lnk" "$INSTDIR\Oicha.exe" "" "$INSTDIR\Oicha.exe" 0 SW_SHOWNORMAL "" "Oicha — Không gian viết tiểu thuyết"
  WriteRegDWORD HKCU "${APP_KEY}" "StartMenu" 1
SectionEnd
Section /o "Shortcut trên Desktop" Desktop
  CreateShortcut "$DESKTOP\Oicha.lnk" "$INSTDIR\Oicha.exe" "" "$INSTDIR\Oicha.exe" 0 SW_SHOWNORMAL "" "Oicha — Không gian viết tiểu thuyết"
  WriteRegDWORD HKCU "${APP_KEY}" "Desktop" 1
SectionEnd
Section /o "Thêm Oicha vào Open with cho file .oicha" Association
  WriteRegStr HKCU "Software\Classes\Oicha.Manuscript" "" "Oicha Manuscript"
  WriteRegStr HKCU "Software\Classes\Oicha.Manuscript\DefaultIcon" "" '$\"$INSTDIR\Oicha.exe$\",0'
  WriteRegStr HKCU "Software\Classes\Oicha.Manuscript\shell\open\command" "" '$\"$INSTDIR\Oicha.exe$\" $\"%1$\"'
  WriteRegStr HKCU "Software\Classes\.oicha\OpenWithProgids" "Oicha.Manuscript" ""
  WriteRegDWORD HKCU "${APP_KEY}" "Association" 1
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd

Section "-Finalize"
  SectionGetFlags ${Desktop} $0
  IntOp $0 $0 & ${SF_SELECTED}
  ${If} $0 = 0
  ${AndIf} $OldDesktop = 1
    Delete "$DESKTOP\Oicha.lnk"
  ${EndIf}
  WriteRegDWORD HKCU "${APP_KEY}" "Desktop" $0
  SectionGetFlags ${StartMenu} $0
  IntOp $0 $0 & ${SF_SELECTED}
  ${If} $0 = 0
  ${AndIf} $OldMenu = 1
    Delete "$SMPROGRAMS\Oicha\Oicha.lnk"
    RMDir "$SMPROGRAMS\Oicha"
  ${EndIf}
  WriteRegDWORD HKCU "${APP_KEY}" "StartMenu" $0
  SectionGetFlags ${Association} $0
  IntOp $0 $0 & ${SF_SELECTED}
  ${If} $0 = 0
  ${AndIf} $OldAssociation = 1
    DeleteRegValue HKCU "Software\Classes\.oicha\OpenWithProgids" "Oicha.Manuscript"
    DeleteRegKey /ifempty HKCU "Software\Classes\.oicha\OpenWithProgids"
    DeleteRegKey /ifempty HKCU "Software\Classes\.oicha"
    DeleteRegKey HKCU "Software\Classes\Oicha.Manuscript"
    System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ${EndIf}
  WriteRegDWORD HKCU "${APP_KEY}" "Association" $0
SectionEnd

Function .onInit
  ${IfNot} ${IsNativeAMD64}
    SetErrorLevel 2
    Abort "Bộ cài này dành cho Windows x64."
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 < 19041
    SetErrorLevel 2
    Abort "Oicha cần Windows 10 phiên bản 2004 trở lên, hoặc Windows 11."
  ${EndIf}
  !insertmacro AcquireMutex
  ReadRegStr $0 HKCU "${APP_KEY}" "Version"
  ${If} $0 != ""
    ${VersionCompare} $0 "${APP_VERSION}" $1
    ${If} $1 = 1
      SetErrorLevel 2
      Abort "Một phiên bản Oicha mới hơn đã được cài."
    ${EndIf}
  ${EndIf}
  ; Refuse unknown nonempty destinations; never overwrite another app's files.
  IfFileExists "$INSTDIR\*.*" 0 empty
  ClearErrors
  FileOpen $0 "$INSTDIR\oicha-install-id.txt" r
  IfErrors invalid
  FileRead $0 $1
  FileClose $0
  ${If} $1 != "${MARKER}"
invalid:
    SetErrorLevel 2
    Abort "Thư mục đích không phải cài đặt Oicha được quản lý."
  ${EndIf}
empty:
  ReadRegDWORD $OldDesktop HKCU "${APP_KEY}" "Desktop"
  ReadRegDWORD $OldMenu HKCU "${APP_KEY}" "StartMenu"
  ReadRegDWORD $OldAssociation HKCU "${APP_KEY}" "Association"
  ReadRegStr $0 HKCU "${APP_KEY}" "Version"
  ${If} $0 != ""
  ${AndIf} $OldMenu != 1
    SectionSetFlags ${StartMenu} 0
  ${EndIf}
  ${If} $OldDesktop = 1
    SectionSetFlags ${Desktop} 1
  ${EndIf}
  ${If} $OldAssociation = 1
    SectionSetFlags ${Association} 1
  ${EndIf}
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/NOSTARTMENU" $1
  ${IfNot} ${Errors}
    SectionSetFlags ${StartMenu} 0
  ${EndIf}
  ClearErrors
  ${GetOptions} $0 "/DESKTOP" $1
  ${IfNot} ${Errors}
    SectionSetFlags ${Desktop} 1
  ${EndIf}
  ClearErrors
  ${GetOptions} $0 "/ASSOC" $1
  ${IfNot} ${Errors}
    SectionSetFlags ${Association} 1
  ${EndIf}
FunctionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext current
  !insertmacro AcquireMutex
  ClearErrors
  FileOpen $0 "$INSTDIR\oicha-install-id.txt" r
  IfErrors invalid
  FileRead $0 $1
  FileClose $0
  ${If} $1 != "${MARKER}"
invalid:
    SetErrorLevel 2
    Abort "Không xác nhận được thư mục cài đặt Oicha."
  ${EndIf}
  Call un.CheckWritable
FunctionEnd

Section "Uninstall"
  Call un.CheckWritable
  ClearErrors
  Delete "$INSTDIR\Oicha.exe"
  ${If} ${Errors}
    SetErrorLevel 2
    Abort "Không thể gỡ app đang được sử dụng. Bản thảo và đăng ký cài đặt vẫn được giữ lại."
  ${EndIf}
  ReadRegStr $0 HKCU "${APP_KEY}" "InstallDir"
  ${If} $0 == $INSTDIR
    ReadRegDWORD $0 HKCU "${APP_KEY}" "Desktop"
    ${If} $0 = 1
      Delete "$DESKTOP\Oicha.lnk"
    ${EndIf}
    ReadRegDWORD $0 HKCU "${APP_KEY}" "StartMenu"
    ${If} $0 = 1
      Delete "$SMPROGRAMS\Oicha\Oicha.lnk"
      RMDir "$SMPROGRAMS\Oicha"
    ${EndIf}
    DeleteRegValue HKCU "Software\Classes\.oicha\OpenWithProgids" "Oicha.Manuscript"
    DeleteRegKey /ifempty HKCU "Software\Classes\.oicha\OpenWithProgids"
    DeleteRegKey /ifempty HKCU "Software\Classes\.oicha"
    DeleteRegKey HKCU "Software\Classes\Oicha.Manuscript"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
    DeleteRegKey HKCU "${APP_KEY}"
    System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ${EndIf}
  !include "${UNINSTALL_INCLUDE}"
  Delete "$INSTDIR\Oicha.exe.installing"
  Delete "$INSTDIR\Oicha.exe.previous"
  Delete "$INSTDIR\Uninstall.exe"
  ; Intentionally nonrecursive: preserve user-added files and every manuscript.
  RMDir "$INSTDIR"
SectionEnd
