@echo off
rem Legt eine Windows-Verknuepfung mit dem Myelith-Symbol an.
rem
rem Warum ein Skript und keine mitgelieferte .lnk: Eine Verknuepfung
rem speichert absolute Pfade. Eine im Repository abgelegte .lnk zeigte auf
rem den Rechner, auf dem sie erzeugt wurde, und waere auf jedem anderen
rem kaputt. Dieses Skript erzeugt sie dort, wo sie gebraucht wird.
rem
rem Eine Stapeldatei kann selbst kein Symbol tragen; das ist eine Grenze
rem des Formats, keine Nachlaessigkeit. Die Verknuepfung kann es.

setlocal
set "WURZEL=%~dp0.."
for %%I in ("%WURZEL%") do set "WURZEL=%%~fI"

set "ZIEL=%WURZEL%\Myelith Testclient - Windows (Batch).cmd"
set "SYMBOL=%WURZEL%\README\Grafiken\myelith-icon.ico"

if not exist "%ZIEL%" (
    echo Fehler: %ZIEL% nicht gefunden.
    goto :ende
)

echo Wohin soll die Verknuepfung?
echo   [1] Desktop
echo   [2] Neben das Repository ^(Vorgabe^)
set "WAHL="
set /p "WAHL=Auswahl [2]: "
if "%WAHL%"=="1" (
    set "ORT=%USERPROFILE%\Desktop"
) else (
    set "ORT=%WURZEL%"
)

set "LNK=%ORT%\Myelith Testclient.lnk"

rem Erzeugung ueber den Windows Script Host: auf jedem Windows vorhanden,
rem ohne Ausfuehrungsrichtlinie wie bei PowerShell.
set "VBS=%TEMP%\myl_lnk_%RANDOM%.vbs"
>  "%VBS%" echo Set s = CreateObject("WScript.Shell")
>> "%VBS%" echo Set l = s.CreateShortcut("%LNK%")
>> "%VBS%" echo l.TargetPath = "%ZIEL%"
>> "%VBS%" echo l.WorkingDirectory = "%WURZEL%"
>> "%VBS%" echo l.IconLocation = "%SYMBOL%"
>> "%VBS%" echo l.Description = "Myelith Testclient"
>> "%VBS%" echo l.Save
cscript //nologo "%VBS%"
del "%VBS%" >nul 2>&1

if exist "%LNK%" (
    echo Verknuepfung angelegt: %LNK%
) else (
    echo Fehler: Verknuepfung konnte nicht angelegt werden.
)

:ende
echo %CMDCMDLINE% | find /i "%~0" >nul
if not errorlevel 1 pause
endlocal
