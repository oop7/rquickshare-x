!macro NSIS_HOOK_POSTINSTALL
  CreateShortCut "$APPDATA\Microsoft\Windows\SendTo\RQuickShare-X.lnk" "$INSTDIR\rquickshare-x.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$APPDATA\Microsoft\Windows\SendTo\RQuickShare-X.lnk"
!macroend