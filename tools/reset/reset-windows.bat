@echo off
setlocal

echo USB Mirror Sync reset for Windows
echo.
echo This removes local app state for the current user:
echo   - config, manifest, log, shadow cache
echo   - startup shortcuts and Startup folder entries
echo It does NOT remove synced folders or files on your USB drive.
echo.
set /p confirm=Type RESET to continue: 
if /I not "%confirm%"=="RESET" (
  echo Cancelled.
  exit /b 0
)

set "APPDATA_ROOT_1=%LOCALAPPDATA%\rad\UsbMirrorSync"
set "APPDATA_ROOT_2=%LOCALAPPDATA%\rad\UsbMirrorSync\data"
set "APPDATA_ROOT_3=%APPDATA%\rad\UsbMirrorSync"
set "STARTUP_LNK=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\USB Mirror Sync.lnk"
set "STARTUP_EXE=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\USB Mirror Sync.exe"

for %%P in ("%APPDATA_ROOT_1%" "%APPDATA_ROOT_2%" "%APPDATA_ROOT_3%") do (
  if exist "%%~P" (
    echo Removing %%~P
    rmdir /s /q "%%~P"
  )
)

if exist "%STARTUP_LNK%" (
  echo Removing %STARTUP_LNK%
  del /f /q "%STARTUP_LNK%"
)

if exist "%STARTUP_EXE%" (
  echo Removing %STARTUP_EXE%
  del /f /q "%STARTUP_EXE%"
)

echo.
echo Reset complete.
endlocal
