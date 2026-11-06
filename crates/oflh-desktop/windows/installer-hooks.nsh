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

!define OFLH_WEBSITE_LABEL "oflh.karimzouine.com"
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
; Replaces the stock welcome text, whose advice to close other applications
; to avoid a reboot does not apply: OFLH installs only into its own folder.
LangString oflhWelcomeText 1033 "This wizard installs OFLH Desktop. It shows you which process is locking a file, folder or port.$\r$\n$\r$\nClick Next to continue."
LangString oflhWelcomeText 1031 "Dieser Assistent installiert OFLH Desktop. Damit findest du heraus, welcher Prozess eine Datei, einen Ordner oder einen Port blockiert.$\r$\n$\r$\nKlicke auf Weiter, um fortzufahren."
LangString oflhWelcomeText 2052 "此向导将安装 OFLH Desktop。它可以告诉你是哪个进程占用了文件、文件夹或端口。$\r$\n$\r$\n单击“下一步”继续。"
LangString oflhFinishText 1033 "OFLH Desktop is installed.$\r$\n$\r$\nClick Finish to close the wizard."
LangString oflhFinishText 1031 "OFLH Desktop ist installiert.$\r$\n$\r$\nKlicke auf Fertig stellen, um den Assistenten zu schließen."
LangString oflhFinishText 2052 "OFLH Desktop 已安装。$\r$\n$\r$\n单击“完成”关闭向导。"
LangString oflhDirectoryText 1033 "Setup installs OFLH Desktop in the folder below. To use a different folder, click Browse.$\r$\n$\r$\nClick Next to continue."
LangString oflhDirectoryText 1031 "OFLH Desktop wird in den unten angegebenen Ordner installiert. Für einen anderen Ordner klicke auf Durchsuchen.$\r$\n$\r$\nKlicke auf Weiter, um fortzufahren."
LangString oflhDirectoryText 2052 "OFLH Desktop 将安装到下面的文件夹。如需使用其他文件夹，请单击“浏览”。$\r$\n$\r$\n单击“下一步”继续。"
LangString oflhUninstallText 1033 "OFLH Desktop will be removed from the folder below. Click Uninstall to start."
LangString oflhUninstallText 1031 "OFLH Desktop wird aus dem unten angegebenen Ordner entfernt. Klicke auf Deinstallieren, um zu starten."
LangString oflhUninstallText 2052 "OFLH Desktop 将从下面的文件夹中删除。单击“卸载”开始。"
LangString oflhRunApp 1033 "Start OFLH Desktop"
LangString oflhRunApp 1031 "OFLH Desktop starten"
LangString oflhRunApp 2052 "启动 OFLH Desktop"
; Running-app prompt: this heading, then appRunningOkKill as its one-line body.
LangString oflhAppRunningTitle 1033 "OFLH Desktop is still running."
LangString oflhAppRunningTitle 1031 "OFLH Desktop läuft noch."
LangString oflhAppRunningTitle 2052 "OFLH Desktop 仍在运行。"
LangString oflhInspectFile 1033 "Inspect file"
LangString oflhInspectFile 1031 "Datei untersuchen"
LangString oflhInspectFile 2052 "检查文件"
LangString oflhInspectFolder 1033 "Inspect folder"
LangString oflhInspectFolder 1031 "Ordner untersuchen"
LangString oflhInspectFolder 2052 "检查文件夹"

; "" until the welcome page is left; then "1" (checked) or "0" (unchecked).
Var OflhContextMenuChoice
Var OflhContextMenuCheckbox

; Modern UI 2 retains native controls, focus, keyboard access and DPI scaling.
!define MUI_BGCOLOR "F8FAFD"
!define MUI_TEXTCOLOR "1F2937"
Var OflhPanelStrip
Var OflhBrandLabel
Var OflhBrandTitleFont
Var OflhBrandSubtitleFont
Var OflhHeaderWordmark
Var OflhHeaderFont
!define MUI_CUSTOMFUNCTION_GUIINIT OflhGuiInit
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.OflhGuiInit

!define MUI_WELCOMEPAGE_TEXT "$(oflhWelcomeText)"
; Consumed later by the template's finish page; it sets MUI_FINISHPAGE_RUN.
; Consumed by the directory and uninstall confirm pages; replace NSIS's
; built-in texts, which are formal in German.
!define MUI_DIRECTORYPAGE_TEXT_TOP "$(oflhDirectoryText)"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "$(oflhUninstallText)"
!define MUI_FINISHPAGE_TEXT "$(oflhFinishText)"
!define MUI_FINISHPAGE_RUN_TEXT "$(oflhRunApp)"
!define MUI_PAGE_CUSTOMFUNCTION_SHOW OflhWelcomeShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE OflhWelcomeLeave

; MUI2 welcome page layout in dialog units: the page is 193u high, the text
; (one 28u title above it) sits at 120u 45u 195u 130u. The text is shortened
; into a padded white panel and the checkbox
; gets a 30u, three-line slot below it, well above the bottom edge.
; Dialog units scale with the font and DPI, so nothing is clipped at 100-200%.
!define OFLH_WELCOME_TEXT_HEIGHT 54
!define OFLH_CHECKBOX_TOP 123
!define OFLH_CHECKBOX_HEIGHT 30

Var OflhFrameTop
Var OflhFrameWidth
Var OflhFrameHeight
!macro OFLH_FRAME_STRIP X Y W H
  System::Call 'user32::CreateWindowExW(i 0, w "STATIC", w "", i 0x50000000, i ${X}, i ${Y}, i ${W}, i ${H}, p r0, p 0, p 0, p 0) p.s'
  Pop $OflhPanelStrip
  SetCtlColors $OflhPanelStrip "1F2937" "FFFFFF"
!macroend

; Draws a white panel around the control on top of the stack by adding four
; padding strips beside it. Nothing overlaps: sibling STATIC controls do not
; clip each other, so a card placed behind the text could paint over it.
; Padding is 8u x 6u, in dialog units so it scales with font and DPI.
!macro OFLH_FRAME_PANEL
  Exch $R0
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7
  System::Call 'user32::GetParent(p R0) p.r0'
  System::Call '*(i 0, i 0, i 0, i 0) p.r1'
  System::Call 'user32::GetWindowRect(p R0, p r1)'
  System::Call 'user32::MapWindowPoints(p 0, p r0, p r1, i 2)'
  System::Call '*$1(i.r2, i.r3, i.r4, i.r5)'
  System::Call '*$1(i 8, i 6, i 0, i 0)'
  System::Call 'user32::MapDialogRect(p r0, p r1)'
  System::Call '*$1(i.r6, i.r7)'
  System::Free $1
  ; $2..$5: left, top, right, bottom of the text; $6/$7: horizontal/vertical padding.
  IntOp $1 $2 - $6
  IntOp $OflhFrameWidth $4 - $2
  IntOp $OflhFrameWidth $OflhFrameWidth + $6
  IntOp $OflhFrameWidth $OflhFrameWidth + $6
  IntOp $OflhFrameTop $3 - $7
  !insertmacro OFLH_FRAME_STRIP $1 $OflhFrameTop $OflhFrameWidth $7
  !insertmacro OFLH_FRAME_STRIP $1 $5 $OflhFrameWidth $7
  IntOp $OflhFrameHeight $5 - $3
  !insertmacro OFLH_FRAME_STRIP $1 $3 $6 $OflhFrameHeight
  !insertmacro OFLH_FRAME_STRIP $4 $3 $6 $OflhFrameHeight
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
  Pop $R0
!macroend
Function OflhFramePanel
  !insertmacro OFLH_FRAME_PANEL
FunctionEnd
Function un.OflhFramePanel
  !insertmacro OFLH_FRAME_PANEL
FunctionEnd

; NSIS loads bitmaps with LoadImage at the control size, which shrinks them
; with nearest-neighbour sampling: the high-resolution logo comes out jagged.
; Reload the full-size BMP and shrink it with HALFTONE StretchBlt, which
; averages the source pixels, then hand the result to the control.
; Leaves the previous image to its owner (MUI frees it on page leave).
!macro OFLH_SMOOTH_IMAGE CONTROL PATH
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7
  Push $8
  Push $9
  StrCpy $0 ${CONTROL}
  System::Call '*(i 0, i 0, i 0, i 0) p.r1'
  System::Call 'user32::GetClientRect(p r0, p r1)'
  System::Call '*$1(i, i, i.r2, i.r3)'
  System::Free $1
  ; IMAGE_BITMAP, natural size, LR_LOADFROMFILE | LR_CREATEDIBSECTION
  System::Call 'user32::LoadImageW(p 0, w "${PATH}", i 0, i 0, i 0, i 0x2010) p.r4'
  ${If} $4 P<> 0
  ${AndIf} $2 > 0
  ${AndIf} $3 > 0
    ; BITMAP: bmType, bmWidth, bmHeight, ...
    ; BITMAP is 24 bytes in a 32-bit and 32 bytes in a 64-bit process.
    System::Alloc 32
    Pop $1
    !if ${NSIS_PTR_SIZE} = 8
      System::Call 'gdi32::GetObjectW(p r4, i 32, p r1)'
    !else
      System::Call 'gdi32::GetObjectW(p r4, i 24, p r1)'
    !endif
    System::Call '*$1(i, i.r5, i.r6)'
    System::Free $1
    System::Call 'user32::GetDC(p 0) p.r7'
    System::Call 'gdi32::CreateCompatibleDC(p r7) p.r8'
    System::Call 'gdi32::CreateCompatibleDC(p r7) p.r9'
    System::Call 'gdi32::CreateCompatibleBitmap(p r7, i r2, i r3) p.r1'
    System::Call 'user32::ReleaseDC(p 0, p r7)'
    System::Call 'gdi32::SelectObject(p r8, p r4) p.s'
    System::Call 'gdi32::SelectObject(p r9, p r1) p.s'
    ; HALFTONE = 4; HALFTONE needs the brush origin reset; SRCCOPY = 0xCC0020
    System::Call 'gdi32::SetStretchBltMode(p r9, i 4)'
    System::Call 'gdi32::SetBrushOrgEx(p r9, i 0, i 0, p 0)'
    System::Call 'gdi32::StretchBlt(p r9, i 0, i 0, i r2, i r3, p r8, i 0, i 0, i r5, i r6, i 0xCC0020)'
    Pop $7
    System::Call 'gdi32::SelectObject(p r9, p r7)'
    Pop $7
    System::Call 'gdi32::SelectObject(p r8, p r7)'
    System::Call 'gdi32::DeleteDC(p r8)'
    System::Call 'gdi32::DeleteDC(p r9)'
    System::Call 'gdi32::DeleteObject(p r4)'
    ; STM_SETIMAGE, IMAGE_BITMAP
    SendMessage $0 0x172 0 $1
  ${EndIf}
  Pop $9
  Pop $8
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

; Render wordmarks as native text: sharp at the actual DPI, exposed to UIA.
Function OflhBrandSidebar
  ${If} $OflhBrandTitleFont == ""
    CreateFont $OflhBrandTitleFont "Segoe UI" 24 700
    CreateFont $OflhBrandSubtitleFont "Segoe UI" 8 500
  ${EndIf}
  ${NSD_CreateLabel} 0u 98u 109u 26u "OFLH"
  Pop $OflhBrandLabel
  ${NSD_AddStyle} $OflhBrandLabel 0x201
  SendMessage $OflhBrandLabel ${WM_SETFONT} $OflhBrandTitleFont 1
  SetCtlColors $OflhBrandLabel "FFFFFF" "4264DB"
  ${NSD_CreateLabel} 0u 124u 109u 14u "Open File Lock Handle"
  Pop $OflhBrandLabel
  ${NSD_AddStyle} $OflhBrandLabel 0x201
  SendMessage $OflhBrandLabel ${WM_SETFONT} $OflhBrandSubtitleFont 1
  SetCtlColors $OflhBrandLabel "FFFFFF" "4264DB"
  ; Sibling STATIC controls do not clip each other, so the sidebar bitmap
  ; repainted over the wordmark. Find every SS_BITMAP control on the page,
  ; give it WS_CLIPSIBLINGS and move it to the bottom of the z-order.
  FindWindow $0 "#32770" "" $HWNDPARENT
  StrCpy $1 0
  ${Do}
    FindWindow $1 "STATIC" "" $0 $1
    ${If} $1 P= 0
      ${Break}
    ${EndIf}
    System::Call 'user32::GetWindowLongW(p r1, i ${GWL_STYLE}) i.r2'
    IntOp $2 $2 & 0x1F
    ${If} $2 = 0x0E
      !insertmacro OFLH_SMOOTH_IMAGE $1 "$PLUGINSDIR\modern-wizard.bmp"
      ${NSD_AddStyle} $1 ${WS_CLIPSIBLINGS}
      System::Call "user32::SetWindowPos(p r1, p 1, i 0, i 0, i 0, i 0, i 0x13)"
    ${EndIf}
  ${Loop}
FunctionEnd

!macro OFLH_HEADER_WORDMARK
  GetDlgItem $0 $HWNDPARENT 1046
  ${If} $0 P<> 0
    System::Call '*(i 0, i 0, i 0, i 0) p.r1'
    System::Call 'user32::GetWindowRect(p r0, p r1)'
    System::Call 'user32::MapWindowPoints(p 0, p $HWNDPARENT, p r1, i 2)'
    System::Call '*$1(i.r2, i.r3, i.r4, i.r5)'
    System::Free $1
    IntOp $4 $4 - $2
    IntOp $5 $5 - $3
    IntOp $6 $5 / 4
    IntOp $3 $3 + $6
    IntOp $4 $4 * 65
    IntOp $4 $4 / 100
    IntOp $5 $5 / 2
    System::Call 'user32::CreateWindowExW(i 0, w "STATIC", w "OFLH", i 0x50000201, i r2, i r3, i r4, i r5, p $HWNDPARENT, p 0, p 0, p 0) p.s'
    Pop $OflhHeaderWordmark
    CreateFont $OflhHeaderFont "Segoe UI" 13 700
    SendMessage $OflhHeaderWordmark ${WM_SETFONT} $OflhHeaderFont 1
    SetCtlColors $OflhHeaderWordmark "4264DB" "FFFFFF"
  ${EndIf}
!macroend
!macro OFLH_SMOOTH_HEADER_IMAGE
  GetDlgItem $0 $HWNDPARENT 1046
  ${If} $0 P<> 0
    !insertmacro OFLH_SMOOTH_IMAGE $0 "$PLUGINSDIR\modern-header.bmp"
  ${EndIf}
!macroend
; Branding line at the bottom of every page, installer and uninstaller alike:
; "OFLH Desktop <version>  ·  oflh.karimzouine.com", never NSIS's own text.
; The template's defines, VERSION included, do not exist yet here, so read the
; version Tauri stamps on the setup exe (the uninstaller carries the same).
!macro OFLH_BRANDING_LINE
  ClearErrors
  GetDLLVersion "$EXEPATH" $1 $2
  ${If} ${Errors}
    StrCpy $3 "$(^Name)"
  ${Else}
    IntOp $3 $1 >> 16
    IntOp $4 $1 & 0xFFFF
    IntOp $5 $2 >> 16
    StrCpy $3 "$(^Name) $3.$4.$5"
  ${EndIf}
  ; NSIS's own branding controls (1028, and MUI2's 1256 over it) stayed
  ; blank on Windows 11; hide both so NSIS's default can never show, and draw
  ; the line as a native label left of the Back button (ID 3).
  GetDlgItem $0 $HWNDPARENT 1028
  ShowWindow $0 ${SW_HIDE}
  GetDlgItem $0 $HWNDPARENT 1256
  ShowWindow $0 ${SW_HIDE}
  GetDlgItem $0 $HWNDPARENT 3
  System::Call '*(i 0, i 0, i 0, i 0) p.r1'
  System::Call 'user32::GetWindowRect(p r0, p r1)'
  System::Call 'user32::MapWindowPoints(p 0, p $HWNDPARENT, p r1, i 2)'
  System::Call '*$1(i.r4, i.r5, i, i.r6)'
  System::Call '*$1(i 7, i 0, i 0, i 0)'
  System::Call 'user32::MapDialogRect(p $HWNDPARENT, p r1)'
  System::Call '*$1(i.r2)'
  System::Free $1
  ; $2 margin, $4 Back left, $5 top, $6 bottom
  IntOp $4 $4 - $2
  IntOp $4 $4 - $2
  IntOp $6 $6 - $5
  ; WS_CHILD | WS_VISIBLE | SS_CENTERIMAGE | SS_NOPREFIX | SS_ENDELLIPSIS
  System::Call 'user32::CreateWindowExW(i 0, w "STATIC", w "$3  ·  ${OFLH_WEBSITE_LABEL}", i 0x50004280, i r2, i r5, i r4, i r6, p $HWNDPARENT, p 0, p 0, p 0) p.r0'
  SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
  SendMessage $0 ${WM_SETFONT} $1 1
  SetCtlColors $0 "6B7280" transparent
!macroend
!macro OFLH_GUI_INIT
  !insertmacro OFLH_SMOOTH_HEADER_IMAGE
  !insertmacro OFLH_HEADER_WORDMARK
  !insertmacro OFLH_BRANDING_LINE
!macroend
Function OflhGuiInit
  !insertmacro OFLH_GUI_INIT
FunctionEnd
Function un.OflhGuiInit
  !insertmacro OFLH_GUI_INIT
FunctionEnd

Function OflhFinishShow
  ShowWindow $OflhHeaderWordmark ${SW_HIDE}
  Call OflhBrandSidebar
FunctionEnd

Function OflhWelcomeShow
  ShowWindow $OflhHeaderWordmark ${SW_HIDE}
  ; $mui.WelcomePage.* are declared after this file is included, so find
  ; MUI's welcome text by its content, hide it, and draw our own copy inside
  ; a padded white card. Moving MUI's control in place left it painted under
  ; the card, because sibling STATIC controls do not clip each other.
  FindWindow $0 "#32770" "" $HWNDPARENT
  StrCpy $6 0
  ${Do}
    FindWindow $6 "STATIC" "" $0 $6
    ${If} $6 P= 0
      ${Break}
    ${EndIf}
    System::Call 'user32::GetWindowTextW(p r6, w .r7, i ${NSIS_MAX_STRLEN})'
    ${If} $7 == "$(oflhWelcomeText)"
      ShowWindow $6 ${SW_HIDE}
    ${EndIf}
  ${Loop}
  ${NSD_CreateLabel} 128u 53u 167u ${OFLH_WELCOME_TEXT_HEIGHT}u "$(oflhWelcomeText)"
  Pop $6
  SetCtlColors $6 "1F2937" "FFFFFF"
  Push $6
  Call OflhFramePanel

  Call OflhBrandSidebar
  ; nsDialogs checkboxes wrap (BS_MULTILINE); German takes two lines.
  ${NSD_CreateCheckbox} 120u ${OFLH_CHECKBOX_TOP}u 183u ${OFLH_CHECKBOX_HEIGHT}u "$(oflhContextMenuOption)"
  Pop $OflhContextMenuCheckbox
  SetCtlColors $OflhContextMenuCheckbox "1F2937" "F8FAFD"
  ${If} $OflhContextMenuChoice != "0"
    ${NSD_Check} $OflhContextMenuCheckbox
  ${EndIf}
FunctionEnd

; Style MUI2's existing page descriptions; do not replace their path controls.
!macro OFLH_OFFSET_DIRECTORY_CONTROL ID
  GetDlgItem $6 $0 ${ID}
  !insertmacro OFLH_OFFSET_CONTROL $6
!macroend
!macro OFLH_OFFSET_CONTROL HWND
  StrCpy $6 ${HWND}
  System::Call '*(i 0, i 0, i 0, i 0) p.R9'
  System::Call 'user32::GetWindowRect(p r6, p R9)'
  System::Call 'user32::MapWindowPoints(p 0, p r0, p R9, i 2)'
  System::Call '*$R9(i.r2, i.r3, i.r4, i.r5)'
  System::Free $R9
  IntOp $3 $3 + $R8
  System::Call 'user32::SetWindowPos(p r6, p 0, i r2, i r3, i 0, i 0, i 0x15)'
!macroend
; Frames MUI2's page description (1006) in a white panel and moves the
; controls below it down by the padding. Used by the directory page and the
; uninstaller's confirm page.
!macro OFLH_DESCRIPTION_PANEL UN
  Push $R7
  Push $R8
  Push $R9
  FindWindow $0 "#32770" "" $HWNDPARENT
  GetDlgItem $R7 $0 1006
  System::Call '*(i 0, i 0, i 0, i 0) p.r1'
  System::Call 'user32::GetWindowRect(p R7, p r1)'
  System::Call 'user32::MapWindowPoints(p 0, p r0, p r1, i 2)'
  System::Call '*$1(i.r2, i.r3, i.r4, i.r5)'
  System::Free $1
  IntOp $4 $4 - $2
  IntOp $5 $5 - $3
  System::Call '*(i 8, i 6, i 16, i 12) p.R9'
  System::Call 'user32::MapDialogRect(p r0, p R9)'
  System::Call '*$R9(i.r7, i.r8, i.r9, i.r1)'
  System::Free $R9
  StrCpy $R8 $1
  ; Inset the description by the frame's padding and push the path controls
  ; down by the same amount, so the white panel keeps its original footprint.
  IntOp $2 $2 + $7
  IntOp $3 $3 + $8
  IntOp $4 $4 - $9
  SetCtlColors $R7 "1F2937" "FFFFFF"
  System::Call 'user32::SetWindowPos(p R7, p 0, i r2, i r3, i r4, i r5, i 0x14)'
  Push $R7
  Call ${UN}OflhFramePanel
  !if "${UN}" == "un."
    ; Confirm page: "Uninstalling from" label, path, "Delete app data".
    !insertmacro OFLH_OFFSET_DIRECTORY_CONTROL 1029
    !insertmacro OFLH_OFFSET_DIRECTORY_CONTROL 1000
    !insertmacro OFLH_OFFSET_CONTROL $R6
  !else
    ; Directory page: group box, path and Browse button.
    !insertmacro OFLH_OFFSET_DIRECTORY_CONTROL 1020
    !insertmacro OFLH_OFFSET_DIRECTORY_CONTROL 1019
    !insertmacro OFLH_OFFSET_DIRECTORY_CONTROL 1001
  !endif
  Pop $R9
  Pop $R8
  Pop $R7
!macroend
Function OflhDirectoryShow
  ShowWindow $OflhHeaderWordmark ${SW_SHOW}
  !insertmacro OFLH_DESCRIPTION_PANEL ""
FunctionEnd
; Takes the "Delete app data" check box on the stack and moves it with the path.
Function un.OflhConfirmShow
  Exch $R6
  !insertmacro OFLH_DESCRIPTION_PANEL "un."
  Pop $R6
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

; Override the running-app prompt from tauri-cli 2.12.0 utils.nsh. Keep its
; Restart Manager, silent/passive and cancellation behavior. The task dialog
; has an explicit owner and centers on the installer, including other monitors.
; TASKDIALOGCONFIG is packed to one byte (commctrl.h), in both NSIS hosts.
!macro OFLH_RUNNING_DIALOG PRODUCT TITLE TEXT
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  System::Call '*(&w${NSIS_MAX_STRLEN} "${PRODUCT}") p.r1'
  System::Call '*(&w${NSIS_MAX_STRLEN} "${TITLE}") p.r2'
  System::Call '*(&w${NSIS_MAX_STRLEN} "${TEXT}") p.r3'
  !if ${NSIS_PTR_SIZE} = 8
    !define OFLH_TASKDIALOG_SIZE 160
  !else
    !define OFLH_TASKDIALOG_SIZE 96
  !endif
  ; TDF_POSITION_RELATIVE_TO_WINDOW | TDF_ALLOW_DIALOG_CANCELLATION = 0x1008;
  ; OK | Cancel = 9; default button IDCANCEL = 2. TD_WARNING_ICON is
  ; MAKEINTRESOURCEW(-1): the zero-extended 16-bit resource ID 65535, not
  ; a sign-extended pointer (-1).
  System::Call '*(i ${OFLH_TASKDIALOG_SIZE}, p $HWNDPARENT, p 0, i 0x1008, i 9, p r1, p 65535, p r2, p r3, i 0, p 0, i 2, i 0, p 0, i 0, p 0, p 0, p 0, p 0, p 0, p 0, p 0, p 0, i 0) p.r4'
  !undef OFLH_TASKDIALOG_SIZE
  StrCpy $R4 2
  System::Call 'comctl32::TaskDialogIndirect(p r4, *i .R4, p 0, p 0) i.r0'
  System::Free $4
  System::Free $3
  System::Free $2
  System::Free $1
  ${If} $0 < 0
    ; An unavailable task dialog must never silently authorize shutdown.
    StrCpy $R4 2
  ${EndIf}
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executablePath productName
  !define UniqueID ${__LINE__}

  ; Replace {{product_name}} placeholder in the messages with the passed product name
  nsis_tauri_utils::StrReplace "$(appRunning)" "{{product_name}}" "${productName}"
  Pop $R1
  nsis_tauri_utils::StrReplace "$(appRunningOkKill)" "{{product_name}}" "${productName}"
  Pop $R2
  nsis_tauri_utils::StrReplace "$(failedToKillApp)" "{{product_name}}" "${productName}"
  Pop $R3

  !insertmacro RestartManager_StartSession $R0
  ${If} $R0 != ""
    !insertmacro RestartManager_RegisterFile $R0 "${executablePath}"
    ${If} $0 = 0
      System::Call 'RSTRTMGR::RmGetList(p R0, *i .r1, *i .r2, p 0, *i .r3) i .r0'
      ; ERROR_SUCCESS means no running processes, ERROR_MORE_DATA means there're some
      ${If} $0 = ${ERROR_MORE_DATA}
        IfSilent kill_${UniqueID} 0
        ${If} $PassiveMode != 1
          !insertmacro OFLH_RUNNING_DIALOG "${productName}" "$(oflhAppRunningTitle)" "$R2"
          ${If} $R4 != 1
            Goto cancel_${UniqueID}
          ${EndIf}
        ${EndIf}
        kill_${UniqueID}:
          ; Call manually here to get the result
          ; !insertmacro RestartManager_Shutdown $R0
          System::Call 'RSTRTMGR::RmShutdown(p R0, i ${RmForceShutdown}, p 0) i .r0'
          ${If} $0 = 0
            Goto app_check_done_${UniqueID}
          ${Else}
            IfSilent silent_${UniqueID} ui_${UniqueID}
            silent_${UniqueID}:
              System::Call 'kernel32::AttachConsole(i -1)i.r0'
              ${If} $0 != 0
                System::Call 'kernel32::GetStdHandle(i -11)i.r0'
                System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)' ; set red color
                FileWrite $0 "$R1$\n"
              ${EndIf}
              !insertmacro RestartManager_EndSession $R0
              Abort
            ui_${UniqueID}:
              !insertmacro RestartManager_EndSession $R0
              Abort $R3
          ${EndIf}
        cancel_${UniqueID}:
          !insertmacro RestartManager_EndSession $R0
          Abort $R1
        app_check_done_${UniqueID}:
      ${EndIf}
    ${EndIf}
    !insertmacro RestartManager_EndSession $R0
  ${EndIf}

  !undef UniqueID
!macroend
