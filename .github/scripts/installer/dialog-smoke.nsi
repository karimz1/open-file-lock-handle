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
!include "${HOOKS}"
Page directory
Page instfiles
Section
  SetAutoClose true
  StrCpy $R1 "OFLH Desktop is running"
  StrCpy $R2 "Close the application to continue, or cancel to keep it running."
  !insertmacro OFLH_RUNNING_DIALOG "OFLH Desktop"
  FileOpen $0 "${RESULT_OUTPUT}" w
  FileWrite $0 "$R4"
  FileClose $0
SectionEnd
