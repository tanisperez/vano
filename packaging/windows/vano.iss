; Inno Setup 6 script for Vano. Compile with ISCC, defines on the command line:
;   ISCC.exe /DAppVersion=0.1.0-rc.1 /DSourceExe=C:\path\vano.exe /DSourceAgent=C:\path\vano-wsl /DOutputDir=C:\out vano.iss
;   AppVersion  full version, may carry a pre-release suffix (X.Y.Z or X.Y.Z-rc.1)
;   SourceExe   path to the release vano.exe
;   SourceAgent path to vano-wsl, the static Linux program Vano runs inside WSL distributions
;   OutputDir   folder for Vano-<version>-windows-x86_64-setup.exe (default: .\Output)
; The LICENSE and the icon are found relative to this file (repo root and crates/vano-app).

#ifndef AppVersion
  #error Pass /DAppVersion=<version>
#endif
#ifndef SourceExe
  #error Pass /DSourceExe=<path to vano.exe>
#endif
#ifndef SourceAgent
  #error Pass /DSourceAgent=<path to vano-wsl>
#endif
#ifndef OutputDir
  #define OutputDir "Output"
#endif

; VersionInfoVersion must be numeric: keep X.Y.Z, drop any "-prerelease", append ".0".
#define NumericVersion Copy(AppVersion, 1, Pos("-", AppVersion + "-") - 1)

#define AppName "Vano"
#define AppExe "vano.exe"
#define AppId "codes.tanis.vano"
#define Publisher "Estanislao Pérez Nartallo"
#define AppURL "https://vano.tanis.codes"

[Setup]
; Never change this GUID, it identifies the installed app for upgrades and the uninstaller.
; The doubled brace is Inno's escape for a literal one.
AppId={{7B2E4C91-5A3D-4F68-9E17-C0A8D2F46B35}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#Publisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
VersionInfoVersion={#NumericVersion}.0
VersionInfoProductVersion={#NumericVersion}.0
VersionInfoCompany={#Publisher}
VersionInfoDescription={#AppName} setup
VersionInfoProductName={#AppName}
; Per user install, no UAC. With PrivilegesRequired=lowest, {autopf} resolves to
; {localappdata}\Programs, so it is the same folder as writing it out; explicit is clearer.
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\{#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; The addtopath task puts the install folder in the user's Path: tell running programs, so new
; terminals see it. The uninstaller broadcasts the removal too.
ChangesEnvironment=yes
; The updater runs the installer silently while Vano may be open: close it through the
; Restart Manager, never ask for a reboot, and relaunch from [Run].
CloseApplications=yes
RestartApplications=no
SetupIconFile=..\..\crates\vano-app\assets\vano.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir={#OutputDir}
OutputBaseFilename=Vano-{#AppVersion}-windows-x86_64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

; Inno Setup bundles no Galician translation, so there is no Galician here.
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[CustomMessages]
english.AddToPath=Add Vano to the PATH (run "vano" from a terminal)
spanish.AddToPath=Añadir Vano al PATH (ejecutar "vano" desde una terminal)
italian.AddToPath=Aggiungi Vano al PATH (esegui "vano" da un terminale)
english.Integration=Integration:
spanish.Integration=Integración:
italian.Integration=Integrazione:

; Upgrades and the updater's silent installs keep the previous choice (UsePreviousTasks).
[Tasks]
Name: "addtopath"; Description: "{cm:AddToPath}"; GroupDescription: "{cm:Integration}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
; Next to vano.exe, where the app looks for it to scan inside WSL 2 distributions.
Source: "{#SourceAgent}"; DestDir: "{app}"; DestName: "vano-wsl"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\NOTICE"; DestDir: "{app}"; Flags: ignoreversion

; The app sets the explicit AppUserModelID codes.tanis.vano, so the shortcuts must carry the
; same one or taskbar pinning and grouping break.
[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppId}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppId}"; Tasks: desktopicon

; Interactively, a checkbox on the last page. Silent installs (winget) do not launch Vano,
; except when the in-app updater asks for it with /relaunch=1.
[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: RelaunchRequested

[Code]
function RelaunchRequested: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:relaunch|0}') = '1');
end;

// The user's Path (HKCU, no admin), so `vano` works in any terminal. Kept as
// REG_EXPAND_SZ, since it usually holds %USERPROFILE% style entries.
const
  EnvironmentKey = 'Environment';

function PathHasDir(Paths, Dir: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Paths) then
    Paths := '';
  // The updater runs the installer on every update: add the folder once.
  if PathHasDir(Paths, Dir) then
    Exit;
  if (Paths <> '') and (Paths[Length(Paths)] <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Paths + Dir);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Paths) then
    Exit;
  if not PathHasDir(Paths, Dir) then
    Exit;
  Paths := ';' + Paths + ';';
  StringChangeEx(Paths, ';' + Dir + ';', ';', True);
  // Drop the separators added above, and the one left where the entry was last.
  while (Length(Paths) > 0) and (Paths[1] = ';') do
    Delete(Paths, 1, 1);
  while (Length(Paths) > 0) and (Paths[Length(Paths)] = ';') do
    Delete(Paths, Length(Paths), 1);
  RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Paths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
