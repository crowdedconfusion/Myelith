# Myelith auf Windows einrichten, aus einem frischen Klon.
#
# Aufruf (PowerShell):
#   .\installieren-windows.ps1                  baut und legt alles unter %LOCALAPPDATA%
#   .\installieren-windows.ps1 -Aktualisieren   holt erst neuen Stand, baut dann
#   .\installieren-windows.ps1 -NurPruefen      sagt nur, was fehlt, und baut nicht
#
# Wenn Windows die Ausfuehrung verweigert, einmal fuer diese Sitzung:
#   Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
#
# ⚑ **Warum aus dem Quelltext und nicht aus einem Installer.** Die
# Freigabebuendel dieses Projekts sind nicht signiert; SmartScreen
# meldet sie zu Recht. Wer aus dem Klon baut, baut aus dem, was er
# lesen kann.
#
# ⚑ **Und es laedt nichts nach.** Fehlt eine Werkzeugkette, nennt
# dieses Skript den einen Befehl und hoert auf.

[CmdletBinding()]
param(
  [switch]$Aktualisieren,
  [switch]$NurPruefen,
  # ⚑ Der Ort laesst sich setzen, weil „Programme" auf einem
  # verwalteten Rechner nicht immer beschreibbar ist.
  [string]$Ziel = (Join-Path $env:LOCALAPPDATA "Programs\Myelith")
)

$ErrorActionPreference = "Stop"
# Zwei Ebenen hinauf: dieses Skript liegt unter SYSTEM\install\ (bis zum
# 2026-09-10 in der Wurzel, danach in INSTALL\, seit dem 2026-09-24 hier).
$Wurzel = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Set-Location $Wurzel

Write-Host "-- Myelith einrichten"
Write-Host "   Quelle: $Wurzel"

# -- Was da sein muss ------------------------------------------------
#
# ⚑ **Erst alles pruefen, dann bauen.** Ein Bau, der nach zehn Minuten
# an einem fehlenden Linker abbricht, hat zehn Minuten gekostet und
# nichts gesagt, was er nicht vorher haette sagen koennen.
# ⚑ **Die Fassung aus `rust-toolchain.toml`, und ohne Netz die vorhandene**
#   (2026-10-22). rustup holt die festgelegte Fassung beim ersten Aufruf im
#   Baum nach; ohne Netz baut die eingestellte weiter, mit Hinweis. Eine
#   fehlende Werkzeugkette darf eine Pruefung kosten, nie den Bau.
if ((Get-Command rustup -ErrorAction SilentlyContinue) -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
  & cargo --version *> $null
  if ($LASTEXITCODE -ne 0) {
    $liste = & rustup toolchain list 2>$null
    $vorhanden = ($liste | Where-Object { $_ -match 'default' } | Select-Object -First 1)
    if (-not $vorhanden) { $vorhanden = ($liste | Where-Object { $_ -match '-' } | Select-Object -First 1) }
    if ($vorhanden) {
      $env:RUSTUP_TOOLCHAIN = ($vorhanden -split ' ')[0]
      Write-Host "   Hinweis: Die festgelegte Rust-Fassung liess sich nicht holen (kein Netz?); gebaut wird mit $($env:RUSTUP_TOOLCHAIN)."
    }
  }
}

$fehlt = @()

# ⚑ **Ohne Netz aus dem Werkzeugvorrat**, wenn er in diesem Baum liegt
#   (2026-10-21, `SYSTEM\werkzeug-vorrat\`, gefuellt mit
#   `werkzeugvorrat.py holen`). Das Skript nennt dann den Befehl mit der
#   Datei aus dem Vorrat; installiert wird nichts hinter dem Ruecken.
$Werkzeug = Join-Path $Wurzel "SYSTEM\werkzeug-vorrat"
$RustMsi = Get-ChildItem -Path (Join-Path $Werkzeug "rust") -Filter "rust-*-x86_64-pc-windows-msvc.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
$PythonExe = Get-ChildItem -Path (Join-Path $Werkzeug "python") -Filter "python-*-amd64.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$VsLayout = Join-Path $Werkzeug "windows\vs_buildtools"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  if ($RustMsi) {
    $fehlt += @"
  Die Rust-Werkzeugkette fehlt. Aus dem Werkzeugvorrat, ohne Netz:
      msiexec /i "$($RustMsi.FullName)"
  danach eine neue Sitzung oeffnen.
"@
  } else {
    $fehlt += @"
  Die Rust-Werkzeugkette fehlt. Holen von https://rustup.rs
  oder, wenn winget da ist:
      winget install --id Rustlang.Rustup
"@
  }
}

if (-not (Get-Command python3 -ErrorAction SilentlyContinue) -and -not (Get-Command python -ErrorAction SilentlyContinue) -and $PythonExe) {
  $fehlt += @"
  Python fehlt (es packt den Vorrat aus). Aus dem Werkzeugvorrat, ohne Netz:
      & "$($PythonExe.FullName)" /passive PrependPath=1
"@
}

# 📌 **Ohne den MSVC-Linker bricht der Bau erst spaet ab**, mit
# `link.exe not found`, und das liest sich wie ein Fehler des
# Projektes. Geprueft wird das Vorhandensein und nicht die Version:
# Welche Ausgabe jemand installiert hat, geht dieses Skript nichts an.
$hatLinker = (Get-Command link.exe -ErrorAction SilentlyContinue) -ne $null
if (-not $hatLinker) {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $vswhere) {
    $hatLinker = [bool](& $vswhere -latest -products * `
      -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
  }
}
if (-not $hatLinker -and (Test-Path (Join-Path $VsLayout "vs_buildtools.exe"))) {
  $fehlt += @"
  Die C++-Buildwerkzeuge von Visual Studio fehlen (Rust braucht ihren Linker).
  Aus dem Offline-Bestand im Werkzeugvorrat, ohne Netz:
      & "$VsLayout\vs_buildtools.exe" --noweb --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended
"@
} elseif (-not $hatLinker) {
  $fehlt += @"
  Die C++-Buildwerkzeuge von Visual Studio fehlen (Rust braucht ihren Linker).
  Holen mit:
      winget install --id Microsoft.VisualStudio.2022.BuildTools `
        --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
"@
}

if ($Aktualisieren -and -not (Get-Command git -ErrorAction SilentlyContinue)) {
  $fehlt += "  git fehlt, und ohne git gibt es keinen neuen Stand zu holen."
}

if ($fehlt.Count -gt 0) {
  Write-Host "-- Es fehlt etwas:" -ForegroundColor Yellow
  $fehlt | ForEach-Object { Write-Host $_ }
  exit 1
}

Write-Host "   cargo: $(cargo --version)"
Write-Host "   MSVC-Linker: da"

if ($NurPruefen) {
  Write-Host "-- Alles da. Ohne -NurPruefen wird gebaut."
  exit 0
}

# -- Neuen Stand holen -----------------------------------------------
#
# 📌 **`--ff-only`, und das ist die ganze Vorsicht.** Wer im Klon
# gearbeitet hat, soll seine Arbeit nicht durch ein
# Installationsskript verlieren.
if ($Aktualisieren) {
  Write-Host "-- neuen Stand holen"
  if (-not (Test-Path (Join-Path $Wurzel ".git"))) {
    throw "Das hier ist kein Klon, sondern ein entpacktes Verzeichnis; es gibt nichts zu holen."
  }
  git fetch --quiet origin
  git merge --ff-only --quiet FETCH_HEAD
  if ($LASTEXITCODE -ne 0) {
    throw "Der Klon laesst sich nicht vorwaerts bewegen; wahrscheinlich liegen eigene Aenderungen darauf."
  }
  Write-Host "   Stand: $(git rev-parse --short HEAD)"
}

# -- Was ausgeliefert wird ---------------------------------------------
#
# ⚑ **Die Liste wird gefunden und nicht gefuehrt.** Jede Kiste, die
# ausgeliefert werden soll, sagt es in ihrer eigenen `Cargo.toml` unter
# `[package.metadata.myelith]`.
#
# 📌 **Bis zum 2026-09-10 stand sie hier, von Hand** (Fund 300), und
# dasselbe noch dreimal in den anderen Skripten. Zwei der vier waren am
# ersten Tag schon uneinig.
$Programme = Get-ChildItem -Path $Wurzel -Recurse -Filter Cargo.toml -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -notmatch '[\\/]target' } |
  ForEach-Object {
    $treffer = Select-String -Path $_.FullName -Pattern '^ausliefern = "(.+)"' |
      Select-Object -First 1
    if ($treffer) {
      [pscustomobject]@{
        Verzeichnis = $_.Directory.FullName
        # ⚑ `.exe` haengt Windows an, die Marke nennt den nackten Namen.
        Name        = "$($treffer.Matches[0].Groups[1].Value).exe"
      }
    }
  } | Sort-Object Verzeichnis

if (-not $Programme) {
  throw "Keine einzige Kiste ist zum Ausliefern angemeldet (gesucht: [package.metadata.myelith])."
}

# ⚑ **Derselbe Vorrat wie ueberall.** Das Auspacken macht
# `SYSTEM/install/vorrat.py`; hier steht nur der Aufruf, damit der schwierige
# Teil auf allen Systemen derselbe Quelltext ist.
$CargoNetz = @()
$VorratOrdner = Join-Path $Wurzel "SYSTEM\crates-vorrat"
$Archive = @()
if (Test-Path $VorratOrdner) {
  $Archive = @(Get-ChildItem -Path $VorratOrdner -Filter *.crate -File -ErrorAction SilentlyContinue)
}
if ($Archive.Count -gt 0) {
  $Ausgepackt = Join-Path $Wurzel "SYSTEM/crates-lager"
  # ⛔️ **Kein `??` und kein `-not $x ? a : b`.** Windows liefert
  # PowerShell 5.1 mit, und die kennt beides nicht; ein Skript, das nur
  # unter 7 laeuft, scheitert genau auf der Maschine, fuer die es
  # geschrieben ist.
  $Python = Get-Command python3 -ErrorAction SilentlyContinue
  if (-not $Python) { $Python = Get-Command python -ErrorAction SilentlyContinue }
  if (-not $Python) {
    throw "Der Vorrat liegt bereit, aber Python fehlt. Ohne Python kein Auspacken; mit Netz geht es auch ohne Vorrat."
  }
  # ⚑ **Neu ausgepackt wird auch, wenn ein Vorrat neuer ist als das Lager**
  #    (2026-10-21), wie unter macOS und Linux. Bis dahin nur, wenn es gar
  #    kein Lager gab: Brachte ein Update neue Archive, baute `--offline`
  #    gegen das alte Lager und brach am ersten neuen Paket ab. Der lokale
  #    Vorrat eigener Module zaehlt mit.
  $Lager = Join-Path $Ausgepackt "vendor"
  $Lokal = Join-Path $Wurzel "SYSTEM\crates-vorrat-lokal"
  $Auspacken = -not (Test-Path (Join-Path $Ausgepackt "cargo-home\config.toml")) -or -not (Test-Path $Lager)
  if (-not $Auspacken) {
    $Stand = (Get-Item $Lager).LastWriteTime
    if ((Get-Item $VorratOrdner).LastWriteTime -gt $Stand) { $Auspacken = $true }
    if ((Test-Path $Lokal) -and ((Get-Item $Lokal).LastWriteTime -gt $Stand)) { $Auspacken = $true }
  }
  if ($Auspacken) {
    Write-Host "   Vorrat: $($Archive.Count) Archive werden ausgepackt"
    # 📌 Hier stand bis zum 2026-09-29 `INSTALL\vorrat.py`, ein Ordner, den es
    #    seit dem Umzug nach SYSTEM\ nicht mehr gibt. Die CI rief `vorrat.py`
    #    direkt und sah es nie. Jetzt neben diesem Skript gesucht, das zieht
    #    mit, wenn beide umziehen.
    & $Python.Source (Join-Path $PSScriptRoot "vorrat.py") auspacken | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Auspacken des Vorrats fehlgeschlagen." }
  }
  $env:CARGO_HOME = Join-Path $Ausgepackt "cargo-home"
  $CargoNetz = @("--offline")
  Write-Host "   Vorrat: $($Archive.Count) Pakete aus SYSTEM\crates-vorrat\, Netz aus"
}

Write-Host "-- bauen, das dauert beim ersten Mal einige Minuten"
Write-Host "   $($Programme.Count) Programme angemeldet"
foreach ($p in $Programme) {
  Write-Host "   $($p.Name)"
  Push-Location $p.Verzeichnis
  try {
    cargo build --release --quiet --locked $CargoNetz
    if ($LASTEXITCODE -ne 0) { throw "Bau von $($p.Name) fehlgeschlagen." }
  } finally { Pop-Location }
}

# -- Legen -------------------------------------------------------------
$Bin = Join-Path $Ziel "bin"
New-Item -ItemType Directory -Force -Path $Bin | Out-Null
foreach ($p in $Programme) {
  # 📌 Bis zum 2026-09-29 `target-shared\release\`: der Bauordner vor dem
  #    Umzug. Gebaut wurde nach SYSTEM\full-build (`.cargo/config.toml`), und
  #    das Kopieren fand nichts.
  $ziel = Join-Path $Bin $p.Name
  # ⛔️ **Ein laufendes Programm wird umbenannt, nicht ueberschrieben**
  #    (2026-10-17): Das Update aus Konsole oder Fenster laeuft aus dem
  #    Programm, das es ersetzt, und Windows sperrt eine laufende `.exe` gegen
  #    Schreiben, nicht aber gegen Umbenennen. Die alte bleibt mit einer
  #    eindeutigen Endung `.<zahl>.alt` liegen, bis sie frei ist, und wird
  #    beim naechsten Lauf weggeraeumt; eine noch gesperrte bleibt still.
  Get-ChildItem $Bin -Filter "$($p.Name).*.alt" -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
  if (Test-Path $ziel) {
    try { Remove-Item $ziel -Force -ErrorAction Stop }
    catch { Rename-Item $ziel "$($p.Name).$(Get-Random).alt" }
  }
  Copy-Item (Join-Path $Wurzel "SYSTEM\full-build\release\$($p.Name)") $ziel -Force
}

# ⚑ **Eine Verknuepfung im Startmenue**, denn ein Programm, das man
# nicht ueber die Suche findet, ist fuer die meisten kein Programm.
$Startmenue = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$Verknuepfung = Join-Path $Startmenue "Myelith.lnk"
$wsh = New-Object -ComObject WScript.Shell
$lnk = $wsh.CreateShortcut($Verknuepfung)
$lnk.TargetPath = Join-Path $Bin "myl-oberflaeche.exe"
$lnk.WorkingDirectory = $Bin
$lnk.Description = "Myelith"
$lnk.Save()

# 📌 **Der PATH wird fuer den Nutzer gesetzt und nicht fuer die
# Maschine.** Systemweit brauchte es Administratorrechte fuer etwas,
# das nur diesen einen Nutzer betrifft.
$nutzerPfad = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($nutzerPfad -notlike "*$Bin*") {
  [Environment]::SetEnvironmentVariable("PATH", "$nutzerPfad;$Bin", "User")
  Write-Host "   PATH ergaenzt; eine neue Sitzung sieht es."
}

Write-Host "-- fertig"
Write-Host "   Programme: $Bin"
Write-Host "   Startmenue: Myelith"
Write-Host ""
Write-Host "Beim ersten Start meldet SmartScreen einen unbekannten Herausgeber,"
Write-Host "denn die Programme sind nicht signiert: Weitere Informationen, dann Trotzdem ausfuehren."
