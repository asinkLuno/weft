; 将安装目录加入用户 PATH，使随包附带的 weft.exe 在终端中直接可用；卸载时清理。
; 通过 PowerShell 操作注册表，避免 NSIS 1024 字符上限截断长 PATH，并保留 REG_EXPAND_STR 类型。

!macro WEFT_UPDATE_USER_PATH ACTION
  System::Call 'Kernel32::SetEnvironmentVariable(t "WEFT_INSTDIR", t "$INSTDIR")'
  nsExec::ExecToStack "powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -Command $\"$$d = $$env:WEFT_INSTDIR.TrimEnd('\'); $$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); $$v = [string]$$k.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); $$a = @($$v -split ';' | Where-Object { $$_ -ne '' -and $$_.TrimEnd('\') -ine $$d }); if ('${ACTION}' -eq 'add') { $$a += $$d }; $$k.SetValue('Path', ($$a -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString); $$k.Close()$\""
  Pop $0
  Pop $1
  System::Call 'User32::SendMessageTimeout(p 0xffff, i 0x001A, p 0, t "Environment", i 2, i 5000, *p .r0)'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro WEFT_UPDATE_USER_PATH "add"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro WEFT_UPDATE_USER_PATH "remove"
!macroend
