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
# [?] Eine Ebene hoeher, seit dem 2026-09-10: Diese Datei lag in der
#     Wurzel und liegt jetzt in INSTALL/.
# Zwei Ebenen hinauf: dieses Skript liegt unter SYSTEM\INSTALL\.
$Wurzel = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Set-Location $Wurzel

Write-Host "-- Myelith einrichten"
Write-Host "   Quelle: $Wurzel"

# -- Was da sein muss ------------------------------------------------
#
# ⚑ **Erst alles pruefen, dann bauen.** Ein Bau, der nach zehn Minuten
# an einem fehlenden Linker abbricht, hat zehn Minuten gekostet und
# nichts gesagt, was er nicht vorher haette sagen koennen.
$fehlt = @()

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $fehlt += @"
  Die Rust-Werkzeugkette fehlt. Holen von https://rustup.rs
  oder, wenn winget da ist:
      winget install --id Rustlang.Rustup
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
if (-not $hatLinker) {
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
  if (-not (Test-Path (Join-Path $Ausgepackt "vendor"))) {
    Write-Host "   Vorrat: $($Archive.Count) Archive werden ausgepackt"
    & $Python.Source (Join-Path $Wurzel "INSTALL\vorrat.py") auspacken | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Auspacken des Vorrats fehlgeschlagen." }
  }
  $env:CARGO_HOME = Join-Path $Ausgepackt "cargo-home"
  $CargoNetz = @("--offline")
  Write-Host "   Vorrat: $($Archive.Count) Pakete aus vorrat\, Netz aus"
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
  Copy-Item (Join-Path $Wurzel "target-shared\release\$($p.Name)") (Join-Path $Bin $p.Name) -Force
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
