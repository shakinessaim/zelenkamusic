#define AppName "ZelenkaMusic"
#define AppVersion "0.1.2"
[Setup]
AppId={{A01439CE-D87E-4E7B-915C-FBE88D58BB77}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=ZelenkaMusic
DefaultDirName={localappdata}\Programs\ZelenkaMusic
DefaultGroupName=ZelenkaMusic
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename=ZelenkaMusic-Setup-{#AppVersion}
SetupIconFile=..\assets\icons\zelenka.ico
UninstallDisplayIcon={app}\ZelenkaMusic.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Tasks]
Name: desktopicon; Description: "Создать ярлык на рабочем столе"; GroupDescription: "Ярлыки:"; Flags: unchecked

[Dirs]
Name: "{app}\songs"; Flags: uninsneveruninstall

[Files]
Source: "..\dist\ZelenkaMusic\ZelenkaMusic.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\ZelenkaMusic\songs\PLACE-MUSIC-HERE.txt"; DestDir: "{app}\songs"; Flags: ignoreversion uninsneveruninstall

[Icons]
Name: "{group}\ZelenkaMusic"; Filename: "{app}\ZelenkaMusic.exe"
Name: "{group}\Папка музыки"; Filename: "{app}\songs"
Name: "{autodesktop}\ZelenkaMusic"; Filename: "{app}\ZelenkaMusic.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ZelenkaMusic.exe"; Description: "Запустить ZelenkaMusic"; Flags: nowait postinstall skipifsilent

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
