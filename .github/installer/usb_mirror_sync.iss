#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef SourceExe
  #define SourceExe "..\..\target\release\usb_mirror_sync.exe"
#endif

#ifndef AppIcon
  #define AppIcon "..\assets\icon.ico"
#endif

#ifndef OutputDir
  #define OutputDir "..\..\target\release"
#endif

#ifndef OutputBase
  #define OutputBase "usb_mirror_sync-setup"
#endif

#ifndef ArchitecturesAllowed
  #define ArchitecturesAllowed "x64compatible"
#endif

#ifndef ArchitecturesInstallIn64BitMode
  #define ArchitecturesInstallIn64BitMode "x64compatible"
#endif

[Setup]
AppId=Rad.UsbMirrorSync
AppName=USB Mirror Sync
AppVersion={#AppVersion}
AppPublisher=Rad
AppPublisherURL=https://github.com/RadNotRed/USBFileSync
AppSupportURL=https://github.com/RadNotRed/USBFileSync
DefaultDirName={autopf}\USB Mirror Sync
DefaultGroupName=USB Mirror Sync
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBase}
SetupIconFile={#AppIcon}
UninstallDisplayIcon={app}\usb_mirror_sync.exe
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
Name: "startup"; Description: "Run USB Mirror Sync at startup"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#AppIcon}"; DestDir: "{app}"; DestName: "usb_mirror_sync.ico"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\config.example.json"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\USB Mirror Sync"; Filename: "{app}\usb_mirror_sync.exe"; IconFilename: "{app}\usb_mirror_sync.ico"
Name: "{group}\Uninstall USB Mirror Sync"; Filename: "{uninstallexe}"
Name: "{autodesktop}\USB Mirror Sync"; Filename: "{app}\usb_mirror_sync.exe"; IconFilename: "{app}\usb_mirror_sync.ico"; Tasks: desktopicon
Name: "{userstartup}\USB Mirror Sync"; Filename: "{app}\usb_mirror_sync.exe"; IconFilename: "{app}\usb_mirror_sync.ico"; Tasks: startup

[Run]
Filename: "{app}\usb_mirror_sync.exe"; Description: "Launch USB Mirror Sync"; Flags: nowait postinstall skipifsilent
