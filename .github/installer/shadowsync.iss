#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef SourceExe
  #define SourceExe "..\..\target\release\shadowsync.exe"
#endif

#ifndef AppIcon
  #define AppIcon "..\assets\icon.ico"
#endif

#ifndef OutputDir
  #define OutputDir "..\..\target\release"
#endif

#ifndef OutputBase
  #define OutputBase "shadowsync-setup"
#endif

#ifndef ArchitecturesAllowed
  #define ArchitecturesAllowed "x64compatible"
#endif

#ifndef ArchitecturesInstallIn64BitMode
  #define ArchitecturesInstallIn64BitMode "x64compatible"
#endif

[Setup]
AppId=Rad.ShadowSync
AppName=ShadowSync
AppVersion={#AppVersion}
AppPublisher=Rad
AppPublisherURL=https://github.com/RadNotRed/USBFileSync
AppSupportURL=https://github.com/RadNotRed/USBFileSync
DefaultDirName={autopf}\ShadowSync
DefaultGroupName=ShadowSync
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBase}
SetupIconFile={#AppIcon}
UninstallDisplayIcon={app}\shadowsync.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed={#ArchitecturesAllowed}
ArchitecturesInstallIn64BitMode={#ArchitecturesInstallIn64BitMode}
DisableProgramGroupPage=no
UsePreviousTasks=yes
ChangesAssociations=no

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked
Name: "startup"; Description: "Run ShadowSync at startup"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AppIcon}"; DestDir: "{app}"; DestName: "shadowsync.ico"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\config.example.json"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\ShadowSync"; Filename: "{app}\shadowsync.exe"; IconFilename: "{app}\shadowsync.ico"
Name: "{group}\Uninstall ShadowSync"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ShadowSync"; Filename: "{app}\shadowsync.exe"; IconFilename: "{app}\shadowsync.ico"; Tasks: desktopicon
Name: "{userstartup}\ShadowSync"; Filename: "{app}\shadowsync.exe"; IconFilename: "{app}\shadowsync.ico"; Tasks: startup

[Run]
Filename: "{app}\shadowsync.exe"; Description: "Launch ShadowSync"; Flags: nowait postinstall skipifsilent
