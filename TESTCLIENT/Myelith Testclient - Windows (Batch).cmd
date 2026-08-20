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
rem Ausgabe in UTF-8. Die deutschen Texte des Clients enthalten Umlaute;
rem die Standard-Codepage der Konsole stellt sie falsch dar. Ohne diese
rem Zeile liest der Nutzer Kauderwelsch und haelt es fuer einen Fehler.
chcp 65001 >nul 2>&1

rem Repository-Wurzel durch Aufwaertssuche bestimmen, nicht ueber eine feste
rem Tiefe. Die erste Fassung rechnete mit dem Wurzelverzeichnis als
rem Ablageort und war sofort kaputt, als der Starter verschoben wurde.
set "WURZEL=%~dp0"
set /a TIEFE=0
:suche
if exist "%WURZEL%TESTCLIENT\myl-testclient\Cargo.toml" goto :gefunden
set /a TIEFE+=1
if %TIEFE% GTR 12 (
    echo Fehler: TESTCLIENT\myl-testclient\Cargo.toml oberhalb von %~dp0
    echo nicht gefunden. Dieser Starter gehoert in ein Myelith-Repository.
    goto :ende
)
for %%I in ("%WURZEL%..") do set "WURZEL=%%~fI\"
goto :suche
:gefunden
set "MANIFEST=%WURZEL%TESTCLIENT\myl-testclient\Cargo.toml"

rem cargo lenkt alle Crates ueber .cargo\config.toml nach target-shared.
rem Auf einem frischen Klon gibt es das Verzeichnis noch nicht, deshalb
rem wird das Binary NACH dem Bau gesucht statt vorher geraten. Die erste
rem Fassung riet auf target\release und scheiterte genau beim ersten Lauf.
where cargo >nul 2>&1
if %ERRORLEVEL%==0 (
    echo Baue Testclient ^(beim ersten Mal dauert das einige Minuten^) ...
    cargo build --release --quiet --manifest-path "%MANIFEST%"
    if errorlevel 1 goto :ende
) else (
    echo Hinweis: cargo nicht gefunden, suche ein vorhandenes Binary.
)

set "BIN="
if defined CARGO_TARGET_DIR if exist "%CARGO_TARGET_DIR%\release\myl-test.exe" set "BIN=%CARGO_TARGET_DIR%\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%target-shared\release\myl-test.exe" set "BIN=%WURZEL%target-shared\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe" set "BIN=%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe"

if not defined BIN (
    echo Fehler: myl-test.exe nicht gefunden.
    echo.
    echo Rust installieren ^(einmalig^): https://rustup.rs
    echo Danach ein neues Fenster oeffnen und diesen Starter erneut aufrufen.
    goto :ende
)

"%BIN%" %*

:ende
rem Nur bei Doppelklick warten: dann ist der Aufrufer explorer.exe und die
rem Konsole gehoert diesem Skript allein.
rem Doppelklick-Erkennung. Verglichen wird nur der Dateiname, nicht der
rem volle Pfad: Der Name enthaelt Klammern, und ein voller Pfad kann
rem Zeichen enthalten, die `find` als Muster missversteht.
echo %CMDCMDLINE% | find /i "%~nx0" >nul
if not errorlevel 1 pause
endlocal
