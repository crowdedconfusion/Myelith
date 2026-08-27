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
rem
rem Fehlt Rust, bietet der Starter die automatische Installation an
rem (rustup, im Benutzerprofil, ohne Administratorrechte). Fehlt danach
rem die C++-Werkzeugkette zum Binden, bietet er sie ueber winget an.
rem Beides geschieht nur nach Rueckfrage; nichts davon laeuft still ab.

setlocal
rem Fenstergroesse festlegen, aber nur beim Doppelklick: Dann gehoert die
rem Konsole diesem Skript allein. Wird der Starter aus einer bestehenden
rem cmd- oder PowerShell-Sitzung aufgerufen, bleibt ihr Fenster unberuehrt
rem: dort passt sich stattdessen das Banner der vorhandenen Breite an.
rem
rem Nur die Groesse, nicht die Lage: Ein Fenster mittig zu setzen verlangt
rem unter Windows einen Aufruf der Win32-Schnittstelle. Das waere ein
rem PowerShell-Skript, und genau darauf verzichtet dieser Starter (siehe
rem oben: Ausfuehrungsrichtlinie).
echo %CMDCMDLINE% | find /i "%~nx0" >nul
if not errorlevel 1 mode con: cols=120 lines=44

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
rem
rem FUND (2026-08-21, beim Durchspielen eines frischen Klons): Der Pfad in
rem .cargo\config.toml ist RELATIV, und cargo loest ihn gegen das
rem ARBEITSVERZEICHNIS auf, nicht gegen das per --manifest-path angegebene
rem Crate. Wer den Starter aus einem anderen Verzeichnis aufruft, und das
rem tut jeder, der eine Verknuepfung auf den Desktop legt, baut deshalb
rem irgendwohin. Der Bau lief durch, und der Starter meldete trotzdem
rem "nicht gefunden".
rem
rem Die Umgebungsvariable hat Vorrang vor der Konfigurationsdatei und ist
rem absolut. Ein von aussen gesetzter Wert bleibt unangetastet: die CI setzt
rem CARGO_TARGET_DIR=target und soll das behalten.
if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=%WURZEL%target-shared"

rem ── Rust suchen, notfalls installieren ─────────────────────────────
rem Zwei Stufen: der Suchpfad, dann das Standardverzeichnis von rustup.
rem Die zweite Stufe faengt Fenster ab, die vor einer frischen
rem Installation geoeffnet wurden und den neuen Pfad noch nicht kennen.
set "CARGO="
where cargo >nul 2>&1
if %ERRORLEVEL%==0 set "CARGO=cargo"
if not defined CARGO if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
if defined CARGO goto :werkzeug

rem Kein cargo. Vielleicht liegt trotzdem schon ein fertiges Binary da;
rem dann wird nichts installiert und der Starter benutzt es.
set "BIN="
if defined CARGO_TARGET_DIR if exist "%CARGO_TARGET_DIR%\release\myl-test.exe" set "BIN=%CARGO_TARGET_DIR%\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%target-shared\release\myl-test.exe" set "BIN=%WURZEL%target-shared\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe" set "BIN=%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe"
if defined BIN (
    echo Hinweis: cargo nicht gefunden, benutze das vorhandene Binary.
    goto :starten
)

echo.
echo Rust ist auf dieser Maschine nicht installiert.
echo Der Testclient braucht es einmalig, um sich selbst zu bauen.
echo.
set /p RUST_FRAGE="Rust jetzt automatisch installieren? Enter = ja, n = nein: "
rem Ohne Eingabekanal (Skript, CI) bleibt die Variable leer: nicht fragen
rem heisst hier nicht installieren.
if not defined RUST_FRAGE goto :rust_anleitung
if /i "%RUST_FRAGE%"=="n" goto :rust_anleitung

rem Rust wird passend zur Prozessorarchitektur geholt.
set "RUSTUP_URL=https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "RUSTUP_URL=https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe"

rem Windows 10 und neuer bringen curl.exe mit. Aeltere Systeme haben es
rem nicht; dann bleibt der Weg von Hand.
where curl >nul 2>&1
if errorlevel 1 (
    echo Kein curl.exe gefunden; Rust kann nicht automatisch geholt werden.
    goto :rust_anleitung
)
echo Lade rustup-init.exe ...
curl -fsSL -o "%TEMP%\rustup-init.exe" "%RUSTUP_URL%"
if errorlevel 1 (
    echo Der Download ist fehlgeschlagen.
    goto :rust_anleitung
)
rem -y beantwortet die rustup-Fragen selbst, minimal holt nur Compiler
rem und cargo. Die Installation landet im Benutzerprofil und braucht
rem keine Administratorrechte.
"%TEMP%\rustup-init.exe" -y --profile minimal --default-toolchain stable
if errorlevel 1 (
    echo Die Rust-Installation wurde abgebrochen oder ist fehlgeschlagen.
    goto :rust_anleitung
)
del "%TEMP%\rustup-init.exe" >nul 2>&1
if not exist "%USERPROFILE%\.cargo\bin\cargo.exe" goto :rust_anleitung
set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
rem Dieses Fenster kennt den neuen Pfad noch nicht.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

:werkzeug
rem ── C++-Werkzeugkette pruefen ──────────────────────────────────────
rem Der Rust-Uebersetzer fuer Windows braucht zum Binden den MSVC-Linker.
rem Fehlt er, scheitert der erste Bau nach mehreren Minuten am letzten
rem Schritt. Die Probe davor spart genau diese Zeit.
set "VSWHERE=C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
set "VC_DA="
if exist "%VSWHERE%" (
    for /f "usebackq tokens=*" %%P in (`"%VSWHERE%" -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VC_DA=%%P"
)
if not defined VC_DA (
    rem Kein vswhere oder ohne C++-Bestandteil: ueber die
    rem Standardordner schauen, bevor etwas installiert wird.
    for %%V in ("C:\Program Files\Microsoft Visual Studio\2022" "C:\Program Files\Microsoft Visual Studio\2019") do (
        for /d %%E in ("%%~V\*") do if exist "%%~E\VC\Tools\MSVC" set "VC_DA=ja"
    )
)
if defined VC_DA goto :bauen

echo.
echo Rust ist da, aber die Microsoft C++-Werkzeugkette zum Binden fehlt.
echo Sie ist einmalig noetig und einige Gigabyte gross.
where winget >nul 2>&1
if errorlevel 1 goto :vc_anleitung
echo.
set /p VC_FRAGE="Werkzeugkette jetzt installieren? Das fragt nach Administratorrechten. Enter = ja, n = nein: "
if not defined VC_FRAGE goto :vc_anleitung
if /i "%VC_FRAGE%"=="n" goto :vc_anleitung
winget install --id Microsoft.VisualStudio.2022.BuildTools --silent --accept-source-agreements --accept-package-agreements --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
if errorlevel 1 (
    echo Die Installation der Werkzeugkette ist fehlgeschlagen.
    goto :vc_anleitung
)
rem Noch einmal pruefen, jetzt mit frischem Blick.
if exist "%VSWHERE%" (
    for /f "usebackq tokens=*" %%P in (`"%VSWHERE%" -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VC_DA=%%P"
)
if defined VC_DA goto :bauen
echo.
echo Die Werkzeugkette wurde installiert, ist hier aber nicht auffindbar.
echo Bitte dieses Fenster schliessen und den Starter neu aufrufen.
goto :ende

:bauen
echo Baue Testclient ^(beim ersten Mal dauert das einige Minuten^) ...
"%CARGO%" build --release --quiet --manifest-path "%MANIFEST%"
if errorlevel 1 goto :ende

rem Das Binary wird NACH dem Bau gesucht statt vorher geraten, siehe oben.
set "BIN="
if defined CARGO_TARGET_DIR if exist "%CARGO_TARGET_DIR%\release\myl-test.exe" set "BIN=%CARGO_TARGET_DIR%\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%target-shared\release\myl-test.exe" set "BIN=%WURZEL%target-shared\release\myl-test.exe"
if not defined BIN if exist "%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe" set "BIN=%WURZEL%TESTCLIENT\myl-testclient\target\release\myl-test.exe"

if not defined BIN (
    echo Fehler: myl-test.exe nicht gefunden.
    echo Gesucht in target-shared\release und myl-testclient\target\release.
    goto :ende
)

:starten
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
exit /b 0

:rust_anleitung
echo.
echo Rust von Hand installieren, einmalig:
echo     1. https://rustup.rs oeffnen und rustup-init.exe ausfuehren
echo     2. Ein NEUES Konsolenfenster oeffnen
echo     3. Diesen Starter erneut aufrufen
goto :ende

:vc_anleitung
echo.
echo Die Microsoft C++-Werkzeugkette von Hand installieren:
echo     winget install Microsoft.VisualStudio.2022.BuildTools
echo Danach im "Visual Studio Installer" die Arbeitslast
echo "Desktopentwicklung mit C++" hinzufuegen. Alternativ gleich im
echo Installer: Build Tools 2022, Arbeitslast "Desktopentwicklung mit C++".
echo Anschliessend dieses Fenster schliessen und den Starter neu aufrufen.
goto :ende
