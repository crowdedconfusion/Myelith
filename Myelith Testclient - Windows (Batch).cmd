@echo off
rem Startet den Myelith-Testclient. Fuer Windows.
rem
rem Aus cmd oder PowerShell:
rem
rem     myl-test.cmd                 interaktives Menue
rem     myl-test.cmd artefakte       Artefakte pruefen
rem     myl-test.cmd --help          alle Befehle
rem
rem Per Doppelklick im Explorer oeffnet sich ein Konsolenfenster mit dem
rem Menue. Damit es nach dem Beenden nicht sofort verschwindet, wartet das
rem Skript in diesem Fall auf eine Taste.
rem
rem Kein PowerShell-Skript: Auf frisch aufgesetzten Rechnern verhindert die
rem Ausfuehrungsrichtlinie das Starten von .ps1-Dateien, und ein Nutzer,
rem der erst `Set-ExecutionPolicy` recherchieren muss, fuehrt den Test
rem seltener aus. Eine .cmd laeuft ohne Vorbedingung.

setlocal
set "WURZEL=%~dp0"
set "MANIFEST=%WURZEL%TESTCLIENT\myl-testclient\Cargo.toml"

if not exist "%MANIFEST%" (
    echo Fehler: %MANIFEST% nicht gefunden.
    echo Dieser Starter gehoert ins Wurzelverzeichnis des Repositories.
    goto :ende
)

if defined CARGO_TARGET_DIR (
    set "BIN=%CARGO_TARGET_DIR%\release\myl-test.exe"
) else if exist "%WURZEL%target-shared" (
    set "BIN=%WURZEL%target-shared\release\myl-test.exe"
) else (
    set "BIN=%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe"
)

where cargo >nul 2>&1
if %ERRORLEVEL%==0 (
    rem cargo prueft selbst, ob etwas zu tun ist, und ist im unveraenderten
    rem Fall in unter einer Sekunde durch.
    echo Baue Testclient ^(beim ersten Mal dauert das einige Minuten^) ...
    cargo build --release --quiet --manifest-path "%MANIFEST%"
    if errorlevel 1 goto :ende
) else if exist "%BIN%" (
    echo Hinweis: cargo nicht gefunden, benutze das vorhandene Binary.
) else (
    echo Fehler: Weder cargo noch ein gebautes Binary gefunden.
    echo.
    echo Rust installieren ^(einmalig^): https://rustup.rs
    echo Danach ein neues Fenster oeffnen und diesen Starter erneut aufrufen.
    goto :ende
)

"%BIN%" %*

:ende
rem Nur bei Doppelklick warten: dann ist der Aufrufer explorer.exe und die
rem Konsole gehoert diesem Skript allein.
echo %CMDCMDLINE% | find /i "%~0" >nul
if not errorlevel 1 pause
endlocal
