!macro NSIS_HOOK_POSTINSTALL
  CreateShortCut "$APPDATA\Microsoft\Windows\SendTo\RQuickShare-X.lnk" "$INSTDIR\rquickshare-x.exe" "$\"%1$\""
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$APPDATA\Microsoft\Windows\SendTo\RQuickShare-X.lnk"
!macroend