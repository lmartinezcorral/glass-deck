; Las credenciales Glassdeck/* viven en el Administrador de credenciales; el ejecutable las borra
; antes de que la plantilla elimine sus archivos (GD-INST-03).
!macro NSIS_HOOK_PREUNINSTALL
  ; Tarea del arranque con Windows (GD-INST-02); si no existe, schtasks solo devuelve un codigo de error.
  ${If} $UpdateMode <> 1
    Push $0
    nsExec::Exec 'schtasks.exe /Delete /TN "Glassdeck" /F'
    Pop $0
    Pop $0
  ${EndIf}
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --borrar-credenciales'
  ${EndIf}
!macroend

; La configuracion y los logs viven en carpetas "Glassdeck", no en las del identificador
; que la plantilla de Tauri borra; sin esto, "borrar datos" dejaria settings.json y logs.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RmDir /r "$APPDATA\Glassdeck"
    RmDir /r "$LOCALAPPDATA\Glassdeck"
  ${EndIf}
!macroend
