; Windows Explorer context menu entries for the OFLH Desktop NSIS installer.
;
; Tauri includes this file near the top of its installer template: before its
; own defines (PRODUCTNAME, MANUPRODUCTKEY, MAINBINARYNAME, ...), before the
; first page, and before MUI_LANGUAGE. That order shapes this file:
;   - Language strings use numeric LANGIDs; ${LANG_*} is not defined yet.
;   - Functions here use only the defines from this file. Template defines are
;     used only inside the NSIS_HOOK_* macros, which Tauri expands later inside
;     its Install and Uninstall sections.
;   - MUI_PAGE_CUSTOMFUNCTION_SHOW/LEAVE defined here are consumed by the
;     template's first page, the welcome page, which gains a checkbox.
;
; The template has no components page, so the option is a checkbox on the
; welcome page, checked by default. Passive, silent, and updater installs skip
; that page and keep the stored choice (on when never chosen). `/NOCONTEXTMENU`
; on the installer command line turns the entries off.
;
; Entries are written to SHCTX\Software\Classes, which follows the install
; mode: HKCU for the default per-user install, HKLM for a per-machine install.
; Explorer resolves HKCU over HKLM for the current user.

!include LogicLib.nsh
!include nsDialogs.nsh
!include FileFunc.nsh

!define OFLH_CONTEXT_MENU_VERB "oflh-desktop.inspect"
!define OFLH_CONTEXT_MENU_PREFERENCE "ExplorerContextMenu"
!define OFLH_SHCNE_ASSOCCHANGED 0x08000000
!define OFLH_SHCNF_IDLIST 0

; Installer languages are chosen from the Windows display language (see
; bundle.windows.nsis.languages in tauri.conf.json), the same language source
; the desktop app uses by default. Labels are written in that language, with
; English as the fallback for every other display language.
; 1033 = English, 1031 = German, 2052 = Simplified Chinese.
LangString oflhContextMenuOption 1033 "Add $\"Inspect file$\" and $\"Inspect folder$\" to the Explorer context menu"
LangString oflhContextMenuOption 1031 "$\"Datei untersuchen$\" und $\"Ordner untersuchen$\" zum Explorer-Kontextmenü hinzufügen"
LangString oflhContextMenuOption 2052 "将“检查文件”和“检查文件夹”添加到资源管理器右键菜单"
LangString oflhInspectFile 1033 "Inspect file"
LangString oflhInspectFile 1031 "Datei untersuchen"
LangString oflhInspectFile 2052 "检查文件"
LangString oflhInspectFolder 1033 "Inspect folder"
LangString oflhInspectFolder 1031 "Ordner untersuchen"
LangString oflhInspectFolder 2052 "检查文件夹"

; "" until the welcome page is left; then "1" (checked) or "0" (unchecked).
Var OflhContextMenuChoice
Var OflhContextMenuCheckbox

!define MUI_PAGE_CUSTOMFUNCTION_SHOW OflhWelcomeShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE OflhWelcomeLeave

; MUI2 welcome page layout in dialog units: the page is 193u high, the text
; (one 28u title above it) sits at 120u 45u 195u 130u and would reach the bottom edge. The text is
; shortened to 95u (the longest default text, German, needs about 72u) and the
; checkbox gets a 30u, three-line slot below it, ending 18u above the edge.
; Dialog units scale with the font and DPI, so nothing is clipped at 100-200%.
!define OFLH_WELCOME_TEXT_HEIGHT 95
!define OFLH_CHECKBOX_TOP 145
!define OFLH_CHECKBOX_HEIGHT 30

Function OflhWelcomeShow
  ; $mui.WelcomePage.* are declared after this file is included, so find the
  ; welcome text at runtime: the page's third static control, after the
  ; sidebar bitmap and the title.
  FindWindow $0 "#32770" "" $HWNDPARENT
  FindWindow $6 "STATIC" "" $0
  FindWindow $6 "STATIC" "" $0 $6
  FindWindow $6 "STATIC" "" $0 $6
  ; Shorten it: map dialog units to pixels for SetWindowPos.
  System::Call "*(i 120, i 45, i 195, i ${OFLH_WELCOME_TEXT_HEIGHT}) p .r1"
  System::Call "user32::MapDialogRect(p r0, p r1)"
  System::Call "*$1(i .r2, i .r3, i .r4, i .r5)"
  System::Free $1
  ; SWP_NOZORDER | SWP_NOACTIVATE
  System::Call "user32::SetWindowPos(p r6, p 0, i r2, i r3, i r4, i r5, i 0x14)"

  ; nsDialogs checkboxes wrap (BS_MULTILINE); German takes two lines.
  ${NSD_CreateCheckbox} 120u ${OFLH_CHECKBOX_TOP}u 195u ${OFLH_CHECKBOX_HEIGHT}u "$(oflhContextMenuOption)"
  Pop $OflhContextMenuCheckbox
  SetCtlColors $OflhContextMenuCheckbox "" "FFFFFF"
  ${If} $OflhContextMenuChoice != "0"
    ${NSD_Check} $OflhContextMenuCheckbox
  ${EndIf}
FunctionEnd

Function OflhWelcomeLeave
  ${NSD_GetState} $OflhContextMenuCheckbox $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $OflhContextMenuChoice "1"
  ${Else}
    StrCpy $OflhContextMenuChoice "0"
  ${EndIf}
FunctionEnd

!macro OFLH_DELETE_CONTEXT_MENU
  DeleteRegKey SHCTX "Software\Classes\*\shell\${OFLH_CONTEXT_MENU_VERB}"
  DeleteRegKey SHCTX "Software\Classes\Directory\shell\${OFLH_CONTEXT_MENU_VERB}"
  DeleteRegKey SHCTX "Software\Classes\Directory\Background\shell\${OFLH_CONTEXT_MENU_VERB}"
  DeleteRegKey SHCTX "Software\Classes\Drive\shell\${OFLH_CONTEXT_MENU_VERB}"
!macroend

; Explorer passes the selection as %1 and the folder behind a background click
; as %V. Both are quoted for spaces; the app repairs the `"C:\"` drive-root
; quoting quirk when it parses `--inspect`.
!macro OFLH_WRITE_CONTEXT_MENU_VERB CLASS LABEL PLACEHOLDER
  WriteRegStr SHCTX "Software\Classes\${CLASS}\shell\${OFLH_CONTEXT_MENU_VERB}" "MUIVerb" "${LABEL}"
  WriteRegStr SHCTX "Software\Classes\${CLASS}\shell\${OFLH_CONTEXT_MENU_VERB}" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  ; One target at a time, like drag and drop: hide the entry for multi-selections.
  WriteRegStr SHCTX "Software\Classes\${CLASS}\shell\${OFLH_CONTEXT_MENU_VERB}" "MultiSelectModel" "Single"
  WriteRegStr SHCTX "Software\Classes\${CLASS}\shell\${OFLH_CONTEXT_MENU_VERB}\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --inspect $\"${PLACEHOLDER}$\""
!macroend

!macro OFLH_REFRESH_SHELL
  System::Call 'shell32::SHChangeNotify(i ${OFLH_SHCNE_ASSOCCHANGED}, i ${OFLH_SHCNF_IDLIST}, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  ${If} $OflhContextMenuChoice == ""
    ; Welcome page skipped (passive, silent, or updater install): keep the
    ; previous choice, defaulting to on.
    ClearErrors
    ReadRegDWORD $0 SHCTX "${MANUPRODUCTKEY}" "${OFLH_CONTEXT_MENU_PREFERENCE}"
    ${If} ${Errors}
      StrCpy $OflhContextMenuChoice "1"
    ${Else}
      StrCpy $OflhContextMenuChoice $0
    ${EndIf}
  ${EndIf}
  ClearErrors
  ${GetOptions} $CMDLINE "/NOCONTEXTMENU" $0
  ${IfNot} ${Errors}
    StrCpy $OflhContextMenuChoice "0"
  ${EndIf}

  ; Rewrite from scratch so labels follow the current display language and the
  ; command follows the current install directory.
  !insertmacro OFLH_DELETE_CONTEXT_MENU
  ${If} $OflhContextMenuChoice == "1"
    !insertmacro OFLH_WRITE_CONTEXT_MENU_VERB "*" "$(oflhInspectFile)" "%1"
    !insertmacro OFLH_WRITE_CONTEXT_MENU_VERB "Directory" "$(oflhInspectFolder)" "%1"
    !insertmacro OFLH_WRITE_CONTEXT_MENU_VERB "Directory\Background" "$(oflhInspectFolder)" "%V"
    !insertmacro OFLH_WRITE_CONTEXT_MENU_VERB "Drive" "$(oflhInspectFolder)" "%1"
    WriteRegDWORD SHCTX "${MANUPRODUCTKEY}" "${OFLH_CONTEXT_MENU_PREFERENCE}" 1
  ${Else}
    WriteRegDWORD SHCTX "${MANUPRODUCTKEY}" "${OFLH_CONTEXT_MENU_PREFERENCE}" 0
  ${EndIf}
  !insertmacro OFLH_REFRESH_SHELL
  Pop $0
!macroend

; Runs after the template removed the app, so cancelling its "app is running"
; prompt leaves the entries in place.
!macro NSIS_HOOK_POSTUNINSTALL
  ; Updates keep the entries; the new installer rewrites them afterwards.
  ${If} $UpdateMode <> 1
    !insertmacro OFLH_DELETE_CONTEXT_MENU
    DeleteRegValue SHCTX "${MANUPRODUCTKEY}" "${OFLH_CONTEXT_MENU_PREFERENCE}"
    !insertmacro OFLH_REFRESH_SHELL
  ${EndIf}
!macroend
