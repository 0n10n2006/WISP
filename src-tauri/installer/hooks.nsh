; Extra installer steps for Wisp (per-user, no admin rights needed).
;
; App Paths lets "wisp" be typed into Start search or Win+R to launch it,
; alongside the Start-menu shortcut the installer already creates.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\${MAINBINARYNAME}.exe" "Path" "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\App Paths\${MAINBINARYNAME}.exe"
  ; Notification name and logo registered by Wisp at startup.
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\app.wisp.desktop"
!macroend
