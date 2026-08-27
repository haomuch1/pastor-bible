; Installer hooks for The Pastor Bible.
;
; PLAN 7.5 requires that an older installer run over a newer installation is
; refused with a plain message. Tauri's NSIS template compares the versions, but
; only to word the reinstall page; with /S there is no page, and measured on
; 2026-08-27 the 0.9.0 installer replaced a 0.9.1 installation without a word.
; So the check is made here, in the hook that runs at the top of the install
; section in every mode, silent included.
;
; It also asks, on uninstall, whether the reader's questions and their
; downloaded model should go too. PLAN 7.5: in plain words, and the default is
; keep.

!include "LogicLib.nsh"
!include "FileFunc.nsh"

; ---------------------------------------------------------------- downgrade

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R5 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${If} $R5 != ""
    nsis_tauri_utils::SemverCompare "${VERSION}" "$R5"
    Pop $R6
    ${If} $R6 = -1
      DetailPrint "Refusing to install ${VERSION} over the newer $R5."
      MessageBox MB_ICONSTOP|MB_OK \
        "A newer version of ${PRODUCTNAME} is already on this computer.$\r$\n$\r$\n\
         This installer is version ${VERSION}, and version $R5 is installed.$\r$\n$\r$\n\
         Nothing has been changed. If you really want the older version, uninstall \
         ${PRODUCTNAME} first, then run this installer again. Your questions and your \
         downloaded model are kept when you uninstall unless you ask for them to be \
         removed." \
        /SD IDOK
      SetErrorLevel 4
      Abort "This installer is older than the version already installed."
    ${EndIf}
  ${EndIf}
!macroend

; ------------------------------------------------------------ uninstall data

; The reader's questions and the model they downloaded are theirs, and they are
; expensive to get back: the model is five gigabytes. So uninstalling leaves
; them alone unless the answer is yes, and the question says what "yes" costs.
!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $R7 "$APPDATA\${BUNDLEID}"
  ${If} ${FileExists} "$R7\*.*"
    MessageBox MB_ICONQUESTION|MB_YESNO|MB_DEFBUTTON2 \
      "Also delete your saved questions and the answering model you downloaded?$\r$\n$\r$\n\
       They are in:$\r$\n$R7$\r$\n$\r$\n\
       Choose No to keep them. If you install ${PRODUCTNAME} again later, your \
       questions will still be there and the model will not have to be downloaded \
       again (it is about 5 GB).$\r$\n$\r$\n\
       Choose Yes to delete them permanently." \
      /SD IDNO IDYES delete_user_data
    DetailPrint "Keeping questions and models in $R7"
    Goto keep_user_data
    delete_user_data:
      DetailPrint "Deleting $R7"
      RMDir /r "$R7"
    keep_user_data:
  ${EndIf}

  ; The webview's own cache is not the reader's data and is always removed.
  RMDir /r "$LOCALAPPDATA\${BUNDLEID}"
!macroend
