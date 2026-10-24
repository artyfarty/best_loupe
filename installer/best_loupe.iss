; best_loupe installer (Inno Setup 6). Build: ISCC /DAppVersion=X.Y.Z /DSourceExe=<uiaccess exe> best_loupe.iss
; The app must live in Program Files and be signed by a machine-trusted cert to get uiAccess; the
; post-install step runs uiaccess-sign.ps1, which signs it with a machine-local certificate.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\dist\release\best_loupe.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

[Setup]
AppId={{86587AAB-7E2E-4734-AC6A-B4F098EF4002}
AppName=best_loupe
AppVersion={#AppVersion}
AppVerName=best_loupe {#AppVersion}
AppPublisher=artyfarty
AppPublisherURL=https://github.com/artyfarty/best_loupe
AppSupportURL=https://github.com/artyfarty/best_loupe/issues
AppUpdatesURL=https://github.com/artyfarty/best_loupe/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\best_loupe
; uiAccess only works from a secure location, so the directory is not user-selectable.
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
OutputDir={#OutputDir}
OutputBaseFilename=best_loupe-v{#AppVersion}-setup
SetupIconFile=..\assets\best_loupe.ico
UninstallDisplayIcon={app}\best_loupe.exe
UninstallDisplayName=best_loupe
LicenseFile=..\LICENSE
WizardStyle=modern
WizardSizePercent=130
Compression=lzma2/max
SolidCompression=yes
CloseApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.AutoStart=Start best_loupe with Windows
ru.AutoStart=Запускать best_loupe вместе с Windows
en.Signing=Enabling the lens over admin windows...
ru.Signing=Включаю работу лупы поверх окон администратора...
en.LaunchNow=Start best_loupe now
ru.LaunchNow=Запустить best_loupe

[Tasks]
Name: "autostart"; Description: "{cm:AutoStart}"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\windows\uiaccess-sign.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\best_loupe"; Filename: "{app}\best_loupe.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "best_loupe"; ValueData: """{app}\best_loupe.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\uiaccess-sign.ps1"" -Exe ""{app}\best_loupe.exe"""; StatusMsg: "{cm:Signing}"; Flags: runhidden waituntilterminated
; shellexec + runasoriginaluser: a non-elevated ShellExecute launch, as uiAccess requires.
Filename: "{app}\best_loupe.exe"; Description: "{cm:LaunchNow}"; Flags: postinstall nowait shellexec runasoriginaluser skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM best_loupe.exe"; Flags: runhidden; RunOnceId: "StopLoupe"
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\uiaccess-sign.ps1"" -Remove"; Flags: runhidden; RunOnceId: "RemoveCert"

[Code]
// The running instance (tray app) locks the exe: stop it before files are replaced.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM best_loupe.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(500);
  Result := '';
end;
