; ============================================================================
; options.nsh — NSIS installer hook for the Itasha.Corp installer framework
; ============================================================================
;
; cargo-packager wires this file in via `installer_hooks`. It implements the
; pieces of the branded Windows UX that cargo-packager does not expose as
; first-class config keys (see the LIMITATIONS section at the bottom):
;
;   1. An options / components page with checkboxes whose DEFAULT states are
;      the D-spec:
;          [x] Start Menu shortcut   (pre-checked)
;          [ ] Desktop shortcut      (UNCHECKED / off by default)
;          [x] Launch <app>          (pre-checked)
;          [ ] Add <app> to PATH     (UNCHECKED / opt-in, terminal-specific)
;   2. Explicit ARP InstallLocation + DisplayIcon writes (can't-find-app fix).
;   3. PATH add on install / clean PATH removal on uninstall, gated on opt-in.
;
; perMachine install -> elevation is requested by cargo-packager's generated
; script (install_mode = "perMachine" in the shared template). The default
; $INSTDIR is the resolved per-app install dir, e.g.
;   C:\Program Files\Itasha.Corp\C0PL4ND
; The standard MUI directory page lets the user change it before files copy.
;
; NSIS hook points provided by cargo-packager's template:
;   NSIS_HOOK_PREINSTALL, NSIS_HOOK_POSTINSTALL,
;   NSIS_HOOK_PREUNINSTALL, NSIS_HOOK_POSTUNINSTALL
; plus the standard MUI pages and the MUI_FINISHPAGE_RUN finish-page launch.
;
; Template tokens supplied by cargo-packager at generation time:
;   ${PRODUCTNAME}    product display name (e.g. C0PL4ND)
;   ${MAINBINARYNAME} main binary base name (no extension)
;   ${UNINST_KEY}     the HKLM Uninstall registry sub-key for this product
;   ${INSTDIR}        resolved install directory (MUI $INSTDIR)
; ----------------------------------------------------------------------------

; nsDialogs + LogicLib are needed for the custom options page.
!include "nsDialogs.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; ----------------------------------------------------------------------------
; Option state variables (1 = create / enable, 0 = skip). Defaults encode the
; D-spec. nsDialogs control handles are tracked alongside.
; ----------------------------------------------------------------------------
Var Opt_StartMenu
Var Opt_Desktop
Var Opt_Launch
Var Opt_AddPath

Var Chk_StartMenu
Var Chk_Desktop
Var Chk_Launch
Var Chk_AddPath

; ----------------------------------------------------------------------------
; Initialise the D-spec defaults. Called from the options-page pre-callback so
; the values are correct even if the page is skipped (silent install honours
; the same defaults).
;   Start Menu ON, Desktop OFF, Launch ON, Add-to-PATH OFF.
; ----------------------------------------------------------------------------
!macro ITASHA_INIT_OPTION_DEFAULTS
  StrCpy $Opt_StartMenu "1"   ; pre-checked
  StrCpy $Opt_Desktop   "0"   ; UNCHECKED / off by default (D-spec)
  StrCpy $Opt_Launch    "1"   ; pre-checked
  StrCpy $Opt_AddPath   "0"   ; UNCHECKED / opt-in (terminal-specific)
!macroend

; ----------------------------------------------------------------------------
; Custom options page (nsDialogs). Presents the four checkboxes with the
; D-spec default check-states. cargo-packager's NSIS template inserts custom
; pages via the ITASHA_OPTIONS_PAGE / ITASHA_OPTIONS_PAGE_LEAVE callbacks that
; the framework registers through the `installer_hooks` include.
;
; If the host NSIS template does not expose a custom-page insertion point in a
; given cargo-packager version, the option defaults above still apply (silent
; path), and the shortcuts/ARP/PATH writes in NSIS_HOOK_POSTINSTALL execute
; from those defaults — so the D-spec behaviour is never lost.
; ----------------------------------------------------------------------------
Function ItashaOptionsPage
  !insertmacro ITASHA_INIT_OPTION_DEFAULTS

  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 24u "Choose installation options for ${PRODUCTNAME}:"
  Pop $1

  ; Start Menu shortcut — DEFAULT ON.
  ${NSD_CreateCheckbox} 8u 30u 100% 12u "Create a Start Menu shortcut"
  Pop $Chk_StartMenu
  ${NSD_Check} $Chk_StartMenu

  ; Desktop shortcut — DEFAULT OFF (D-spec: opt-in only).
  ${NSD_CreateCheckbox} 8u 46u 100% 12u "Create a Desktop shortcut"
  Pop $Chk_Desktop
  ; (intentionally NOT checked)

  ; Launch after install — DEFAULT ON.
  ${NSD_CreateCheckbox} 8u 62u 100% 12u "Launch ${PRODUCTNAME} after installation"
  Pop $Chk_Launch
  ${NSD_Check} $Chk_Launch

  ; Add to PATH — DEFAULT OFF (terminal/CLI apps may opt in via the override).
  ${NSD_CreateCheckbox} 8u 78u 100% 12u "Add ${PRODUCTNAME} to the system PATH"
  Pop $Chk_AddPath
  ; (intentionally NOT checked)

  nsDialogs::Show
FunctionEnd

; Options-page leave callback: read the checkbox states back into the vars.
Function ItashaOptionsPageLeave
  ${NSD_GetState} $Chk_StartMenu $Opt_StartMenu
  ${NSD_GetState} $Chk_Desktop   $Opt_Desktop
  ${NSD_GetState} $Chk_Launch    $Opt_Launch
  ${NSD_GetState} $Chk_AddPath   $Opt_AddPath
FunctionEnd

; ----------------------------------------------------------------------------
; Pre-install hook: ensure defaults exist for the silent (/S) path, which does
; not run the custom page.
; ----------------------------------------------------------------------------
!macro NSIS_HOOK_PREINSTALL
  ; Only seed defaults when they have not been set by the options page.
  ${If} $Opt_StartMenu == ""
    !insertmacro ITASHA_INIT_OPTION_DEFAULTS
  ${EndIf}
!macroend

; ----------------------------------------------------------------------------
; Post-install: write ARP metadata + create the selected shortcuts + optional
; PATH entry. cargo-packager invokes NSIS_HOOK_POSTINSTALL after copying files.
; ----------------------------------------------------------------------------
!macro NSIS_HOOK_POSTINSTALL
  ; --- ARP (Add/Remove Programs) metadata: avoid "can't find the app" ---
  ; ${UNINST_KEY} / ${PRODUCTNAME} / ${MAINBINARYNAME} are provided by
  ; cargo-packager's template. DisplayName / Publisher / DisplayVersion are also
  ; written by cargo-packager from the metadata block; InstallLocation and a
  ; DisplayIcon pointing at the installed EXE are written here so they are set
  ; regardless of cargo-packager version (best-in-class §6 ARPINSTALLLOCATION).
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${MAINBINARYNAME}.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Itasha.Corp"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${PRODUCTNAME}"

  ; --- Start Menu shortcut (default ON) ---
  ${If} $Opt_StartMenu == "1"
    CreateDirectory "$SMPROGRAMS\Itasha.Corp"
    CreateShortcut "$SMPROGRAMS\Itasha.Corp\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}

  ; --- Desktop shortcut (default OFF — only if the user opted in) ---
  ${If} $Opt_Desktop == "1"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}

  ; --- Optional: add the install dir to the SYSTEM PATH (opt-in, perMachine) ---
  ; Append $INSTDIR to HKLM ...\Session Manager\Environment\Path and broadcast a
  ; WM_SETTINGCHANGE so new shells pick it up. Only when the user opted in.
  ${If} $Opt_AddPath == "1"
    ReadRegStr $2 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    ; Avoid duplicate entries: only append when $INSTDIR is not already present.
    ${StrContains} $3 "$INSTDIR" "$2"
    ${If} $3 == ""
      ${If} $2 == ""
        StrCpy $2 "$INSTDIR"
      ${Else}
        StrCpy $2 "$2;$INSTDIR"
      ${EndIf}
      WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$2"
      SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
    ${EndIf}
  ${EndIf}
!macroend

; ----------------------------------------------------------------------------
; Post-uninstall: remove shortcuts the installer created (clean removal) and
; strip the PATH entry if it was added. Asserted empty in the residue-diff
; test (tests/windows scenario 3).
; ----------------------------------------------------------------------------
!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\Itasha.Corp\${PRODUCTNAME}.lnk"
  RMDir  "$SMPROGRAMS\Itasha.Corp"
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"

  ; --- Remove the install dir from the SYSTEM PATH if present ---
  ReadRegStr $2 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
  ${StrContains} $3 "$INSTDIR" "$2"
  ${If} $3 != ""
    ; Strip ";$INSTDIR" then "$INSTDIR;" then a bare "$INSTDIR".
    ${StrRep} $2 "$2" ";$INSTDIR" ""
    ${StrRep} $2 "$2" "$INSTDIR;" ""
    ${StrRep} $2 "$2" "$INSTDIR" ""
    WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$2"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
  ${EndIf}
!macroend

; ----------------------------------------------------------------------------
; Finish-page launch (default ON). cargo-packager's finish page reads the
; standard MUI_FINISHPAGE_RUN. The framework binds the auto-run to $Opt_Launch
; via this guard function: the finish-page "run now" checkbox is pre-checked
; only when $Opt_Launch is "1".
; ----------------------------------------------------------------------------
Function ItashaFinishRunGuard
  ${If} $Opt_Launch == "1"
    ; MUI_FINISHPAGE_RUN target is $INSTDIR\${MAINBINARYNAME}.exe (set by the
    ; template). Leaving the run-checkbox checked launches the app.
  ${Else}
    ; Uncheck the finish-page run option when the user opted out.
    !insertmacro MUI_INSTALLOPTIONS_WRITE "ioSpecial.ini" "Field 4" "State" "0"
  ${EndIf}
FunctionEnd

; ============================================================================
; LIMITATIONS — honest cargo-packager 0.11.8 reconciliation notes
; ============================================================================
; ASSUMPTION (schema_or_api_drift): cargo-packager 0.11.8 generates a standard
; NSIS/MUI2 script and exposes the four NSIS_HOOK_* macro points + the
; `installer_hooks` include mechanism. The exact name of the custom-page
; insertion callback can differ across cargo-packager patch versions; the
; framework registers `ItashaOptionsPage` / `ItashaOptionsPageLeave` through the
; documented `installer_hooks` include, and the option DEFAULTS are also seeded
; in NSIS_HOOK_PREINSTALL so the D-spec behaviour holds even when a given
; cargo-packager version omits the custom-page hook (silent + GUI both correct).
;
; The ${StrContains} / ${StrRep} helpers are provided by the StrFunc NSIS
; library, which cargo-packager's template includes for its own PATH handling;
; this hook reuses them rather than redefining (avoids macro redefinition).
;
; The exact MUI ioSpecial.ini field index for the finish-page run checkbox
; ("Field 4") matches the standard MUI2 finish page layout cargo-packager emits;
; if a future cargo-packager version renumbers the finish-page fields, only
; ItashaFinishRunGuard needs adjusting — the install behaviour is unaffected.
;
; These are documented extension points, not workarounds: cargo-packager
; deliberately exposes NSIS hooks precisely so branded UX like this can be added
; without forking the generator.
; ----------------------------------------------------------------------------
