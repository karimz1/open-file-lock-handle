; Fast native validation of the exact dialog macro used by the full installer.
Unicode true
Name "OFLH dialog smoke test"
OutFile "${OUTPUT_EXE}"
RequestExecutionLevel user
; TaskDialogIndirect requires the Common Controls v6 activation context.
XPStyle on
InstallDir "$TEMP\oflh-dialog-smoke"
!include MUI2.nsh
!macro CheckIfAppIsRunning executablePath productName
!macroend
!include /CHARSET=UTF8 "${HOOKS}"
Page directory
Page instfiles
Section
  SetAutoClose true
  !insertmacro OFLH_RUNNING_DIALOG "OFLH Desktop" "$(oflhAppRunningTitle)" "Click OK to close it and continue."
  FileOpen $0 "${RESULT_OUTPUT}" w
  FileWrite $0 "$R4"
  FileClose $0
SectionEnd
