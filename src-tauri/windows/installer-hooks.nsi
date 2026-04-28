; CanchaYa POS Desktop — NSIS installer hooks
;
; Tauri 2 inyecta estos macros en su template NSIS standard. Nos sirve
; para agregar pasos custom al install/uninstall sin reemplazar todo el
; template (que cambia entre versiones de tauri-bundler y nos romperia).
;
; Ver: https://v2.tauri.app/distribute/windows-installer/#using-custom-nsis-template

; ─────────────────────────────────────────────────────────────────
; POST-INSTALL: corre apenas terminan de copiarse los archivos
; ─────────────────────────────────────────────────────────────────
!macro NSIS_HOOK_POSTINSTALL
  ; Acceso directo en el escritorio para que el cajero abra la app con
  ; un doble click sin tener que buscarla en el menu Inicio.
  CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"

  ; Auto-arranque cuando se prende la PC. Critica para el flow de un
  ; club: la cajera prende la maquina a la mañana y ya tiene CanchaYa
  ; POS abierto cuando llega a la barra. Si el club no quiere esto,
  ; lo desactivan desde Configuracion → Apps → Inicio en Windows.
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" \
    "${PRODUCTNAME}" '"$INSTDIR\${MAINBINARYNAME}.exe"'
!macroend

; ─────────────────────────────────────────────────────────────────
; PRE-UNINSTALL: corre antes de borrar archivos al desinstalar
; ─────────────────────────────────────────────────────────────────
!macro NSIS_HOOK_PREUNINSTALL
  ; Limpiar el shortcut del desktop
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"

  ; Sacar del autostart de Windows
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" \
    "${PRODUCTNAME}"
!macroend
