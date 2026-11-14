#!/usr/bin/env python3
"""Eine Sicherheitskopie des ganzen Arbeitsbaums, ohne Gewichte und ohne Bauabfall.

Aufruf (aus der Wurzel oder von irgendwo):

```text
python3 SYSTEM/install/sicherung.py <zielordner>           # myelith-sicherung-JJJJ-MM-TT.tar.gz, ersetzt die alte
python3 SYSTEM/install/sicherung.py <zielordner> --zip     # dasselbe als .zip, fuer Windows
python3 SYSTEM/install/sicherung.py <zielordner> --zwischen <lokal> --lokal-behalten
python3 SYSTEM/install/sicherung.py --liste                # nur zeigen, was hinein kaeme
python3 SYSTEM/install/sicherung.py --selbsttest           # die Regeln an einem kleinen Baum
```

# ⚑ Was hinein kommt, und was nicht

Hinein kommt **alles, was im Arbeitsbaum liegt**, auch was git nicht kennt:
der Verlauf (`.git`), eigene Module, Notizen, der lokale Paketvorrat. Draussen
bleibt nur, was sich wieder herstellen laesst oder fremd und gross ist, und
das steht in den Regeln unten, an genau einer Stelle:

- **Gewichte und die meisten Artefakte.** Sie wiegen Hunderte Gigabyte und
  kommen ueber den Katalog und die Bauskripte zurueck. Von den Artefakten
  bleiben die Beschreibungen und die in `ARTEFAKTE_MIT` genannten (0,6B und
  4B, damit die Kopie ohne Netz ein Modell ausfuehrt), von den Modellordnern
  Lizenz, Konfiguration und Tokenizer.
- **Bauabfall**: der gemeinsame Bauordner, das ausgepackte Lager, Python-
  Umgebungen, Bytecode, die Reste der Integrationsproben.
- **Verknuepfungen** (Symlinks). Sie zeigen auf Orte dieses Rechners, liegen
  hier nur in Python-Umgebungen und fremden Baeumen, und Windows kann sie
  beim Auspacken meist nicht anlegen. Sie werden genannt, nicht kopiert.

Dazu kommt die **Schluesselablage** `Secure` neben dem Repositorium, als
eigener Ordner in der Kopie (`--ohne-secure` laesst sie weg). Ist ihr Abbild
eingehaengt, wird nicht kopiert.

# ⚑ Erst lokal, dann auf den Datentraeger

Die Kopie entsteht ganz in einem lokalen Zwischenordner (Vorgabe: der
Temp-Ordner, also `TMPDIR`), wird dann auf den Datentraeger uebertragen, dort
gegen die lokale Pruefsumme gehalten und ersetzt erst danach die alte. Reicht
der Platz dort nicht fuer beide, geht die alte zuerst. Danach wird die lokale
Kopie entfernt.

# ⚑ Was die Kopie kann

Ausgepackt baut sie **ohne Netz**: Die Rust-Abhaengigkeiten liegen als Archive
unter `SYSTEM/crates-vorrat/` (und fuer eigene, nicht versionierte Module unter
`SYSTEM/crates-vorrat-lokal/`), die fremden Werkzeuge (unter NixOS die ganze
Bauumgebung) unter `SYSTEM/werkzeug-vorrat/`, wenn sie vorher mit
`werkzeugvorrat.py holen` geholt wurden; die Einrichtungsskripte unter
`SYSTEM/install/` nehmen beides. Die Datei `SICHERUNG.md` in der Kopie sagt,
von welchem Stand sie ist und wie es weitergeht; `PRUEFSUMMEN.sha256` haelt
jede Datei fest.
"""

from __future__ import annotations

import argparse
import datetime
import fnmatch
import hashlib
import io
import os
import pathlib
import plistlib
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile

# ⚠️ **`parents[2]`**: Diese Datei liegt unter `SYSTEM/install/`, zwei Ebenen
# unter der Wurzel.
WURZEL = pathlib.Path(__file__).resolve().parents[2]

# Der Name des obersten Ordners in der Kopie.
KOPFORDNER = "Myelith"

# ── Die Regeln, an einer Stelle ─────────────────────────────────────

# Ordner, die ganz draussen bleiben, relativ zur Wurzel.
ORDNER_DRAUSSEN = (
    "SYSTEM/full-build",          # gemeinsamer Bauordner, entsteht beim Bauen
    "SYSTEM/crates-lager",        # ausgepackter Vorrat, entsteht beim Einrichten
    "SYSTEM/golemos/bau",         # Bauordner von GolemOS
    "INTEGER_LLM/calibrate/lokal",  # Zwischenstaende der Kalibrierung eigener Modelle
)

# Ordnernamen, die ueberall draussen bleiben.
NAMEN_DRAUSSEN = ("__pycache__", ".venv", "node_modules", ".pytest_cache", ".mypy_cache")
# Ordnernamen nach Muster, ueberall: weitere Python-Umgebungen, Cargo-Bauordner
# einzelner Kisten.
MUSTER_DRAUSSEN = (".venv-*", "target")

# Dateien, die ueberall draussen bleiben.
DATEIEN_DRAUSSEN = (".DS_Store", "*.pyc", "*.pyo", "myelith-sicherung-*.tar.gz", "myelith-sicherung-*.zip")

# ⚑ **Von den Artefakten bleiben die Dateien auf oberster Ebene**
# (Beschreibung und Modellkarte), dazu die hier genannten Artefakte ganz;
# jedes andere ist ein Ordner aus Gewichten und kommt ueber den Katalog
# zurueck. ⚑ Zwei kleine kommen mit (Festlegung des Projektinhabers,
# 2026-10-21), damit die Kopie ohne Netz auch ein Modell ausfuehrt: das 0,6B
# (Anker der Konformitaetsvektoren) und das 4B (taugt fuer Agentenarbeit).
NUR_OBERSTE_EBENE = ("INTEGER_LLM/artifacts",)
ARTEFAKTE_MIT = ("sparrow-0.6b-int8", "pigeon-4b-int8")

# ⚑ **Unter `MODELS/` bleiben Gewichte draussen**, nach Endung und nach Groesse.
# Was kleiner ist und keine Gewichtsendung traegt, kommt mit: Lizenz,
# Konfiguration, Tokenizer, der Katalog.
GEWICHTE_UNTER = ("MODELS",)
GEWICHTSENDUNGEN = (
    ".safetensors", ".gguf", ".bin", ".pt", ".pth", ".onnx", ".npz", ".npy",
    ".ckpt", ".h5", ".msgpack", ".tflite", ".engine", ".plan", ".trt", ".mlmodel",
)
GROESSTE_MODELLDATEI = 64 * 1024 * 1024

# Reste der Integrationsproben, die bei jedem Lauf neu entstehen.
PROBENRESTE = ("INTEGER_LLM/tests/integration/_*_artifact*",)

# ⚑ **Die Schluesselablage neben dem Repositorium kommt mit** (Festlegung des
# Projektinhabers, 2026-11-14): ein Ordner `Secure` als Geschwister der
# Wurzel, darin ein verschluesseltes Abbild mit den geheimen Schluesseln
# (Startkette von GolemOS, Commit-Signatur). Ohne ihn waere eine Kopie zwar
# baubar, aber nichts mehr damit zu signieren. In der Kopie liegt er als
# eigener Ordner neben `KOPFORDNER`, mit eigenen Pruefsummen; ausgepackt
# stehen beide wieder nebeneinander. Die Regeln oben gelten dort nicht: Es
# gibt keinen Bauabfall, und ein Band des Abbilds, das fehlt, macht es
# unlesbar. Nur die Ablagedateien des Finders bleiben draussen.
SECURE_NAME = "Secure"
SECURE_DRAUSSEN = (".DS_Store",)


def secure_ordner(wurzel: pathlib.Path) -> pathlib.Path:
    """Der Ort der Schluesselablage: neben der Wurzel, nicht darin."""
    return wurzel.parent / SECURE_NAME


def _unter(rel: str, kopf: str) -> bool:
    return rel == kopf or rel.startswith(kopf + "/")


def ordner_bleibt_draussen(rel: str, name: str, artefakte_mit: tuple[str, ...] = ARTEFAKTE_MIT) -> str | None:
    """Der Grund, aus dem ein Ordner draussen bleibt, oder `None`."""
    if rel in ORDNER_DRAUSSEN:
        return "Bauabfall"
    if name in NAMEN_DRAUSSEN or any(fnmatch.fnmatch(name, m) for m in MUSTER_DRAUSSEN):
        return "Bauabfall"
    if any(fnmatch.fnmatch(rel, m) for m in PROBENRESTE):
        return "Probenrest"
    for kopf in NUR_OBERSTE_EBENE:
        if rel.startswith(kopf + "/"):
            if rel[len(kopf) + 1:].split("/")[0] in artefakte_mit:
                return None
            return "Artefakt"
    return None


def datei_bleibt_draussen(rel: str, name: str, groesse: int) -> str | None:
    """Der Grund, aus dem eine Datei draussen bleibt, oder `None`."""
    if any(fnmatch.fnmatch(name, m) for m in DATEIEN_DRAUSSEN):
        return "Bauabfall"
    for kopf in GEWICHTE_UNTER:
        if _unter(rel, kopf):
            if name.lower().endswith(GEWICHTSENDUNGEN):
                return "Gewicht"
            if groesse > GROESSTE_MODELLDATEI:
                return "Gewicht"
    return None


# ── Der Gang durch den Baum ─────────────────────────────────────────


class Auswahl:
    def __init__(self) -> None:
        self.dateien: list[tuple[str, pathlib.Path, int]] = []
        self.draussen: dict[str, list[int]] = {}
        self.verknuepfungen: list[str] = []

    def weg(self, grund: str, groesse: int) -> None:
        z = self.draussen.setdefault(grund, [0, 0])
        z[0] += 1
        z[1] += groesse

    @property
    def groesse(self) -> int:
        return sum(g for _, _, g in self.dateien)


def _ordnergroesse(p: pathlib.Path) -> int:
    summe = 0
    for wurzel, _, dateien in os.walk(p, followlinks=False):
        for d in dateien:
            try:
                summe += os.lstat(os.path.join(wurzel, d)).st_size
            except OSError:
                pass
    return summe


def auswaehlen(wurzel: pathlib.Path, ordner_messen: bool = True, artefakte_mit: tuple[str, ...] = ARTEFAKTE_MIT) -> Auswahl:
    a = Auswahl()
    for ort, ordner, dateien in os.walk(wurzel, topdown=True, followlinks=False):
        hier = pathlib.Path(ort)
        rel_hier = hier.relative_to(wurzel).as_posix()
        rel_hier = "" if rel_hier == "." else rel_hier
        behalten = []
        for name in sorted(ordner):
            p = hier / name
            rel = f"{rel_hier}/{name}" if rel_hier else name
            if p.is_symlink():
                a.verknuepfungen.append(rel)
                continue
            grund = ordner_bleibt_draussen(rel, name, artefakte_mit)
            if grund:
                a.weg(grund, _ordnergroesse(p) if ordner_messen else 0)
                continue
            behalten.append(name)
        ordner[:] = behalten
        for name in sorted(dateien):
            p = hier / name
            rel = f"{rel_hier}/{name}" if rel_hier else name
            if p.is_symlink():
                a.verknuepfungen.append(rel)
                continue
            try:
                groesse = p.stat().st_size
            except OSError:
                a.weg("unlesbar", 0)
                continue
            grund = datei_bleibt_draussen(rel, name, groesse)
            if grund:
                a.weg(grund, groesse)
                continue
            a.dateien.append((rel, p, groesse))
    return a


def secure_auswaehlen(ordner: pathlib.Path) -> Auswahl:
    """Alles unter der Schluesselablage, ausser Ablagedateien des Finders.

    Verknuepfungen werden wie im Baum genannt und nicht kopiert.
    """
    a = Auswahl()
    for ort, ordner_namen, dateien in os.walk(ordner, topdown=True, followlinks=False):
        hier = pathlib.Path(ort)
        rel_hier = hier.relative_to(ordner).as_posix()
        rel_hier = "" if rel_hier == "." else rel_hier
        behalten = []
        for name in sorted(ordner_namen):
            rel = f"{rel_hier}/{name}" if rel_hier else name
            if (hier / name).is_symlink():
                a.verknuepfungen.append(rel)
            else:
                behalten.append(name)
        ordner_namen[:] = behalten
        for name in sorted(dateien):
            p = hier / name
            rel = f"{rel_hier}/{name}" if rel_hier else name
            if p.is_symlink():
                a.verknuepfungen.append(rel)
                continue
            try:
                groesse = p.stat().st_size
            except OSError:
                a.weg("unlesbar", 0)
                continue
            if any(fnmatch.fnmatch(name, m) for m in SECURE_DRAUSSEN):
                a.weg("Bauabfall", groesse)
                continue
            a.dateien.append((rel, p, groesse))
    return a


def eingehaengte_abbilder() -> list[tuple[pathlib.Path, list[str]]]:
    """Die eingehaengten Abbilder dieses Rechners samt Einhaengepunkten.

    Nur unter macOS (`hdiutil info`); anderswo, und wenn die Abfrage
    scheitert, eine leere Liste. ⚠️ Leer heisst dann "nicht bekannt", nicht
    "nichts eingehaengt"; deshalb sagt `main`, ob gefragt werden konnte.
    """
    try:
        lauf = subprocess.run(["hdiutil", "info", "-plist"], capture_output=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return []
    if lauf.returncode != 0:
        return []
    try:
        daten = plistlib.loads(lauf.stdout)
    except Exception:  # noqa: BLE001 (eine kaputte Antwort heisst: nicht bekannt)
        return []
    abbilder = []
    for abbild in daten.get("images", []):
        pfad = abbild.get("image-path")
        if not pfad:
            continue
        punkte = [e["mount-point"] for e in abbild.get("system-entities", []) if e.get("mount-point")]
        abbilder.append((pathlib.Path(pfad), punkte))
    return abbilder


def secure_eingehaengt(ordner: pathlib.Path, abbilder: list[tuple[pathlib.Path, list[str]]]) -> list[tuple[pathlib.Path, list[str]]]:
    """Die Abbilder aus der Schluesselablage, die gerade eingehaengt sind.

    ⛔️ **Ein eingehaengtes Abbild wird nicht kopiert.** Waehrend es
    eingehaengt ist, schreibt das System in seine Baender; eine Kopie
    mittendrin kann Baender verschiedener Staende tragen und sich danach
    nicht mehr oeffnen lassen.
    """
    try:
        basis = ordner.resolve()
    except OSError:
        return []
    treffer = []
    for pfad, punkte in abbilder:
        try:
            p = pfad.resolve()
        except OSError:
            continue
        if p == basis or basis in p.parents:
            treffer.append((pfad, punkte))
    return treffer


# ── Was in die Kopie zusaetzlich kommt ──────────────────────────────


def _git(*args: str) -> str:
    try:
        lauf = subprocess.run(["git", "-C", str(WURZEL), *args], capture_output=True, text=True, timeout=30)
        return lauf.stdout.strip() if lauf.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def _pruefen_mit(skript: str) -> tuple[bool, str]:
    """Ruft `SYSTEM/install/<skript> pruefen` und liefert Ergebnis und letzte Zeile."""
    pfad = WURZEL / "SYSTEM" / "install" / skript
    try:
        lauf = subprocess.run([sys.executable, str(pfad), "pruefen"], capture_output=True, text=True, timeout=1800)
    except (OSError, subprocess.SubprocessError) as f:
        return False, f"nicht pruefbar: {f}"
    zeilen = [z.strip() for z in (lauf.stdout + lauf.stderr).splitlines() if z.strip()]
    # Ein Pruefer mit mehreren Teilen endet womoeglich mit einem bestandenen;
    # ist er gescheitert, zaehlt die Zeile, die es sagt.
    if lauf.returncode != 0:
        zeilen = [z for z in zeilen if "FAILED" in z or "FEHLER" in z] or zeilen
    return lauf.returncode == 0, zeilen[-1] if zeilen else "keine Ausgabe"


def werkzeuge_pruefen() -> tuple[bool, str]:
    """Ist der Werkzeugvorrat vollstaendig und passt er zu `flake.lock`?

    ⚑ **Vor der Kopie und nicht danach.** Geholt werden kann er nur mit Netz,
    also hier; auf dem Rechner ohne Netz faellt eine Luecke erst auf, wenn
    nichts mehr zu machen ist. Geprueft wird mit `werkzeugvorrat.py pruefen`,
    damit die Regel an einer Stelle steht.
    """
    return _pruefen_mit("werkzeugvorrat.py")


def pakete_pruefen() -> tuple[bool, str]:
    """Passt der Rust-Paketvorrat zu jeder Sperrdatei, auch zu den lokalen?

    📌 **Bis zum 2026-10-22 pruefte die Kopie nur den Werkzeugvorrat.** Wer
    eine Abhaengigkeit aenderte und `vorrat.py sammeln` vergass, bekam eine
    Kopie, die ohne Netz nicht baut. Fuer die versionierten Kisten merkt das
    die CI; fuer eigene, nicht versionierte Module merkt es niemand, und ihre
    Archive liegen nur hier. Dieselbe Regel wie der Werkzeugvorrat: vorher
    pruefen, mit `vorrat.py pruefen`.
    """
    return _pruefen_mit("vorrat.py")


def fehlende_artefakte(wurzel: pathlib.Path, artefakte_mit: tuple[str, ...] = ARTEFAKTE_MIT) -> list[str]:
    """Die in `ARTEFAKTE_MIT` genannten Artefakte, die es nicht (mehr) gibt.

    📌 **Ein Name, der veraltet, faellt sonst still weg.** Am 2026-10-21
    wurden die Modelle umbenannt; haette diese Liste die alten Namen
    behalten, waere die Kopie ohne ein einziges Modell entstanden, und
    nur eine Zeile in `SICHERUNG.md` haette es verraten.
    """
    return [m for m in artefakte_mit if not (wurzel / NUR_OBERSTE_EBENE[0] / m).is_dir()]


def ignoriert_und_drin(a: Auswahl, wurzel: pathlib.Path, ebenen: int = 3) -> list[tuple[str, int, int]]:
    """Was git ignoriert und trotzdem in die Kopie kommt, nach Ordnern.

    ⚑ **Hier taucht neuer Bauabfall zuerst auf.** Die Kopie nimmt alles,
    was die Regeln oben nicht ausschliessen; was ein neues Werkzeug
    erzeugt, kommt also mit, ohne dass jemand es bemerkt. Was git
    ignoriert, ist entweder gewollt hier (Vorraete, Moduldaten, die kleinen
    Artefakte) oder ein Kandidat fuer die Regeln. Ohne git: leer.
    """
    rels = [rel for rel, _, _ in a.dateien]
    try:
        lauf = subprocess.run(["git", "-C", str(wurzel), "check-ignore", "--stdin"], input="\n".join(rels),
                              capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError):
        return []
    ignoriert = set(lauf.stdout.splitlines())
    gruppen: dict[str, list[int]] = {}
    for rel, _, groesse in a.dateien:
        if rel in ignoriert:
            g = gruppen.setdefault("/".join(rel.split("/")[:ebenen]), [0, 0])
            g[0] += 1
            g[1] += groesse
    return sorted(((k, n, g) for k, (n, g) in gruppen.items()), key=lambda x: -x[2])


def beschreibung(a: Auswahl, datum: str, werkzeuge: str = "nicht geprueft", pakete: str = "nicht geprueft",
                 secure: str = "nicht dabei") -> str:
    stand = _git("rev-parse", "--short", "HEAD") or "unbekannt (kein git)"
    offen = len(_git("status", "--porcelain").splitlines()) if stand != "unbekannt (kein git)" else 0
    fassung = ""
    v = WURZEL / "VERSION.md"
    if v.is_file():
        fassung = v.read_text(encoding="utf-8").splitlines()[0].strip()
    zeilen = [
        "# Sicherheitskopie von Myelith",
        "",
        f"- **Erstellt:** {datum}",
        f"- **Fassung:** {fassung or 'unbekannt'}",
        f"- **Stand:** `{stand}`, dazu {offen} geaenderte oder neue Dateien gegenueber diesem Stand",
        f"- **Inhalt:** {len(a.dateien)} Dateien, {a.groesse / 1e9:.2f} GB",
        f"- **Werkzeugvorrat:** {werkzeuge}",
        f"- **Paketvorrat (Rust):** {pakete}",
        f"- **Schluesselablage:** {secure}",
        "",
        "## Bauen und einrichten, ohne Netz",
        "",
        "Aus diesem Ordner, je System (Einzelheiten in `SYSTEM/install/README.md`):",
        "",
        "```sh",
        "sudo sh SYSTEM/install/nix-vorrat-einspielen.sh  # NixOS: einmal, die Bauumgebung ohne Netz",
        "sh SYSTEM/install/installieren-linux.sh          # Linux, auch NixOS",
        "sh SYSTEM/install/installieren-macos.sh          # macOS",
        "```",
        "",
        "```powershell",
        ".\\SYSTEM\\install\\installieren-windows.ps1     # Windows",
        "```",
        "",
        "Die Rust-Pakete liegen in der Kopie (`SYSTEM/crates-vorrat/`). Fremde",
        "Werkzeuge liegen, wenn sie vor der Kopie geholt wurden, unter",
        "`SYSTEM/werkzeug-vorrat/`: fuer NixOS die ganze Bauumgebung, fuer macOS und",
        "Windows die Installer von Rust und Python, fuer Windows WebView2 und der",
        "Starter der Buildwerkzeuge. `python3 SYSTEM/install/werkzeugvorrat.py zeigen`",
        "sagt, was davon da ist.",
        "",
        f"Artefakte in der Kopie: {', '.join(sorted({rel.split('/')[2] for rel, _, _ in a.dateien if rel.startswith('INTEGER_LLM/artifacts/') and rel.count('/') >= 3})) or 'keine'}.",
        "",
        "## Was nicht in der Kopie ist",
        "",
        "| Grund | Dateien | Groesse |",
        "|---|---|---|",
    ]
    for grund, (n, g) in sorted(a.draussen.items()):
        zeilen.append(f"| {grund} | {n} | {g / 1e9:.2f} GB |")
    zeilen += [
        "",
        "- **Gewichte** unter `MODELS/` holt das Fenster oder die Konsole aus dem",
        "  Katalog `MODELS/llm/KATALOG.json` zurueck; **Artefakte** unter",
        "  `INTEGER_LLM/artifacts/` entstehen daraus mit den Bauskripten.",
        "- **Python-Umgebungen** richtet `INTEGER_LLM/scripts/python_einrichten.sh`",
        "  neu ein (braucht Netz).",
        "- **Bauabfall** entsteht beim ersten Bau neu.",
        "",
        f"## Verknuepfungen, die nicht kopiert sind ({len(a.verknuepfungen)})",
        "",
    ]
    zeilen += [f"- `{v}`" for v in a.verknuepfungen] or ["- keine"]
    zeilen += [
        "",
        "## Pruefen",
        "",
        "```sh",
        "sha256sum -c PRUEFSUMMEN.sha256      # Linux",
        "shasum -a 256 -c PRUEFSUMMEN.sha256  # macOS",
        "```",
        "",
        f"## Die Schluesselablage (`{SECURE_NAME}/`)",
        "",
        f"Liegt sie in der Kopie, dann im Ordner `{SECURE_NAME}/` neben diesem, mit eigener",
        "`PRUEFSUMMEN.sha256` (dort pruefen). Zurueckgelegt gehoert sie neben das",
        f"Repositorium, als `{SECURE_NAME}` im selben Ordner wie dieser. Das Abbild darin ist",
        "verschluesselt; ohne sein Passwort ist es nicht zu oeffnen, auch nicht aus der Kopie.",
        "",
    ]
    return "\n".join(zeilen)


# ── Schreiben ───────────────────────────────────────────────────────


def _sha256(p: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for stueck in iter(lambda: f.read(1 << 20), b""):
            h.update(stueck)
    return h.hexdigest()


class _Schreiber:
    def __init__(self, ziel: pathlib.Path, als_zip: bool) -> None:
        self.als_zip = als_zip
        if als_zip:
            self.z = zipfile.ZipFile(ziel, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True)
        else:
            self.t = tarfile.open(ziel, "w:gz", compresslevel=6)

    def datei(self, rel: str, p: pathlib.Path, kopf: str = KOPFORDNER) -> None:
        name = f"{kopf}/{rel}"
        if self.als_zip:
            self.z.write(p, name)
        else:
            self.t.add(str(p), arcname=name, recursive=False)

    def text(self, rel: str, inhalt: str, kopf: str = KOPFORDNER) -> None:
        daten = inhalt.encode("utf-8")
        name = f"{kopf}/{rel}"
        if self.als_zip:
            self.z.writestr(name, daten)
        else:
            info = tarfile.TarInfo(name)
            info.size = len(daten)
            info.mtime = int(time.time())
            info.mode = 0o644
            self.t.addfile(info, io.BytesIO(daten))

    def schliessen(self) -> None:
        (self.z if self.als_zip else self.t).close()


def schreiben(a: Auswahl, ziel: pathlib.Path, als_zip: bool, datum: str, werkzeuge: str = "nicht geprueft",
              pakete: str = "nicht geprueft", secure: Auswahl | None = None, secure_befund: str = "nicht dabei") -> None:
    # ⚑ Erst in eine Zwischendatei, dann umbenennen: Ein abgebrochener Lauf
    # hinterlaesst keine halbe Kopie unter dem endgueltigen Namen.
    halb = ziel.with_name(ziel.name + ".halb")
    w = _Schreiber(halb, als_zip)
    anfang = time.time()
    fertig = 0
    gesamt = (a.groesse + (secure.groesse if secure else 0)) or 1
    naechste_meldung = 0.0

    def teil(auswahl: Auswahl, kopf: str) -> list[str]:
        nonlocal fertig, naechste_meldung
        summen = []
        for rel, p, groesse in auswahl.dateien:
            w.datei(rel, p, kopf)
            summen.append(f"{_sha256(p)}  {rel}")
            fertig += groesse
            jetzt = time.time()
            if jetzt >= naechste_meldung:
                print(f"\r[sicherung] {fertig / gesamt * 100:5.1f} %  {fertig / 1e9:6.2f} von {gesamt / 1e9:.2f} GB", end="", flush=True)
                naechste_meldung = jetzt + 2
        return summen

    try:
        summen = teil(a, KOPFORDNER)
        w.text("SICHERUNG.md", beschreibung(a, datum, werkzeuge, pakete, secure_befund))
        w.text("PRUEFSUMMEN.sha256", "\n".join(summen) + "\n")
        if secure is not None:
            # Eigene Pruefsummen, relativ zu `Secure/`: Die Ablage wird fuer
            # sich zurueckgelegt und fuer sich geprueft.
            s_summen = teil(secure, SECURE_NAME)
            w.text("PRUEFSUMMEN.sha256", "\n".join(s_summen) + "\n", SECURE_NAME)
    finally:
        w.schliessen()
    halb.replace(ziel)
    print(f"\r[sicherung] 100.0 %  {gesamt / 1e9:.2f} GB in {time.time() - anfang:.0f} s" + " " * 10)


# ── Auf den Datentraeger ────────────────────────────────────────────

# ⚑ **Erst lokal, dann hinueber** (Festlegung des Projektinhabers,
# 2026-11-14): Die Kopie entsteht ganz auf der lokalen Platte und wird erst
# fertig und geprueft auf den Datentraeger gelegt, wo sie die alte ersetzt.
# So reicht dort Platz fuer eine einzige Kopie: Ist er knapp, geht die alte
# zuerst, und das ist ungefaehrlich, weil die neue in diesem Augenblick
# vollstaendig lokal liegt. Ein Abbruch mittendrin hinterlaesst auf dem
# Datentraeger nur eine `.halb`, nie eine halbe Kopie unter dem echten Namen.

# Was auf dem Datentraeger frei bleiben soll, ueber die Kopie hinaus.
RESERVE = 64 * 1024 * 1024


def freier_platz(ordner: pathlib.Path) -> int | None:
    if not hasattr(os, "statvfs"):
        return None
    st = os.statvfs(ordner)
    return st.f_bavail * st.f_frsize


def _ohne_cache(fd: int) -> None:
    """Liest und schreibt an diesem Griff am Zwischenspeicher vorbei (macOS).

    ⚑ Sonst liest die Pruefung nach dem Kopieren aus dem Speicher, was eben
    geschrieben wurde, und nicht vom Datentraeger; ein defekter Stick fiele
    nicht auf.
    """
    try:
        import fcntl
        if hasattr(fcntl, "F_NOCACHE"):
            fcntl.fcntl(fd, fcntl.F_NOCACHE, 1)
    except (ImportError, OSError):
        pass


def _aus_dem_cache(fd: int) -> None:
    """Wirft die Seiten dieser Datei aus dem Zwischenspeicher (Linux)."""
    if hasattr(os, "posix_fadvise") and hasattr(os, "POSIX_FADV_DONTNEED"):
        try:
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        except OSError:
            pass


def _sha256_vom_traeger(p: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        _ohne_cache(f.fileno())
        for stueck in iter(lambda: f.read(1 << 20), b""):
            h.update(stueck)
    return h.hexdigest()


def alte_kopien(ordner: pathlib.Path, endung: str) -> list[pathlib.Path]:
    """Die Sicherheitskopien desselben Formats, die schon dort liegen."""
    return sorted(p for p in ordner.glob(f"myelith-sicherung-*{endung}") if p.is_file())


def uebertragen(lokal: pathlib.Path, ziel_ordner: pathlib.Path, summe: str,
                frei=freier_platz) -> pathlib.Path | None:
    """Legt die fertige lokale Kopie auf den Datentraeger und ersetzt dort die alte.

    Liefert den Pfad der neuen Kopie, oder `None`, wenn nichts ersetzt wurde
    (dann liegt die alte unveraendert dort, die neue lokal).
    """
    endung = ".zip" if lokal.name.endswith(".zip") else ".tar.gz"
    endgueltig = ziel_ordner / lokal.name
    halb = ziel_ordner / (lokal.name + ".halb")
    # Reste eines abgebrochenen Laufs zaehlen nicht als Kopie.
    for rest in ziel_ordner.glob("myelith-sicherung-*.halb"):
        rest.unlink()
    alte = alte_kopien(ziel_ordner, endung)
    groesse = lokal.stat().st_size
    platz = frei(ziel_ordner)
    alte_zuerst = False
    if platz is not None and platz < groesse + RESERVE:
        if platz + sum(p.stat().st_size for p in alte) < groesse + RESERVE:
            print(f"[sicherung] FEHLER: auf dem Datentraeger sind {platz / 1e9:.2f} GB frei, auch ohne die alte Kopie"
                  f" zu wenig fuer {groesse / 1e9:.2f} GB. Nichts geloescht; die neue liegt lokal: {lokal}")
            return None
        alte_zuerst = True
    if alte_zuerst:
        for p in alte:
            print(f"[sicherung] Platz knapp: alte Kopie zuerst entfernt (die neue liegt geprueft lokal): {p.name}")
            p.unlink()
    print(f"[sicherung] auf den Datentraeger: {endgueltig}")
    anfang = time.time()
    with open(lokal, "rb") as q, open(halb, "wb") as z:
        _ohne_cache(z.fileno())
        fertig = 0
        naechste_meldung = 0.0
        for stueck in iter(lambda: q.read(8 << 20), b""):
            z.write(stueck)
            fertig += len(stueck)
            jetzt = time.time()
            if jetzt >= naechste_meldung:
                print(f"\r[sicherung] kopiert {fertig / max(groesse, 1) * 100:5.1f} %", end="", flush=True)
                naechste_meldung = jetzt + 2
        z.flush()
        os.fsync(z.fileno())
        _aus_dem_cache(z.fileno())
    print(f"\r[sicherung] kopiert 100.0 % in {time.time() - anfang:.0f} s" + " " * 10)
    # ⛔️ **Erst geprueft, dann umbenannt, dann die alte geloescht.**
    if _sha256_vom_traeger(halb) != summe:
        halb.unlink()
        print("[sicherung] FEHLER: Die Kopie auf dem Datentraeger weicht von der lokalen ab (Pruefsumme)."
              f" Nichts ersetzt; die neue liegt lokal: {lokal}")
        return None
    halb.replace(endgueltig)
    if not alte_zuerst:
        for p in alte:
            if p != endgueltig and p.exists():
                print(f"[sicherung] alte Kopie ersetzt: {p.name}")
                p.unlink()
    print(f"[sicherung] Pruefsumme auf dem Datentraeger gleich der lokalen: {summe[:16]}")
    return endgueltig


# ── Aufruf ──────────────────────────────────────────────────────────


def bericht(a: Auswahl) -> None:
    print(f"[sicherung] hinein: {len(a.dateien)} Dateien, {a.groesse / 1e9:.2f} GB")
    for grund, (n, g) in sorted(a.draussen.items()):
        print(f"[sicherung] draussen ({grund}): {n} Eintraege, {g / 1e9:.2f} GB")
    print(f"[sicherung] Verknuepfungen nicht kopiert: {len(a.verknuepfungen)}")
    groesste = sorted(a.dateien, key=lambda d: -d[2])[:8]
    print("[sicherung] die groessten Dateien darin:")
    for rel, _, g in groesste:
        print(f"   {g / 1e6:8.1f} MB  {rel}")
    ignoriert = ignoriert_und_drin(a, WURZEL)
    if ignoriert:
        print("[sicherung] was git ignoriert und trotzdem mitkommt (hier zeigt sich neuer Bauabfall):")
        for ordner, n, g in ignoriert[:12]:
            print(f"   {g / 1e6:8.1f} MB {n:6d} Dateien  {ordner}")
        if len(ignoriert) > 12:
            rest = ignoriert[12:]
            print(f"   {sum(g for _, _, g in rest) / 1e6:8.1f} MB in {len(rest)} weiteren Ordnern")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Sicherheitskopie des Arbeitsbaums, ohne Gewichte und Bauabfall.")
    ap.add_argument("ziel", nargs="?", help="Ordner auf dem Datentraeger; die Kopie ersetzt dort die alte (nicht im Baum)")
    ap.add_argument("--zip", action="store_true", help="als .zip statt .tar.gz")
    ap.add_argument("--liste", action="store_true", help="nur zeigen, was hinein kaeme")
    ap.add_argument("--selbsttest", action="store_true", help="die Regeln an einem kleinen Baum pruefen")
    ap.add_argument("--ohne-artefakte", action="store_true", help=f"auch {', '.join(ARTEFAKTE_MIT)} draussen lassen")
    ap.add_argument("--ohne-werkzeuge", action="store_true",
                    help="auch mit lueckenhaftem Werkzeug- oder Paketvorrat schreiben (die Kopie braucht dann Netz)")
    ap.add_argument("--ohne-secure", action="store_true",
                    help=f"die Schluesselablage `{SECURE_NAME}` neben dem Repositorium nicht mitnehmen")
    ap.add_argument("--zwischen", help="lokaler Ordner, in dem die Kopie zuerst entsteht (Vorgabe: der Temp-Ordner)")
    ap.add_argument("--lokal-behalten", action="store_true",
                    help="die lokale Kopie nach dem Uebertragen nicht loeschen")
    a = ap.parse_args(argv)
    if a.selbsttest:
        return selbsttest()
    if not a.liste and not a.ziel:
        ap.print_usage()
        return 2
    auswahl = auswaehlen(WURZEL, artefakte_mit=() if a.ohne_artefakte else ARTEFAKTE_MIT)
    bericht(auswahl)
    fehlend = [] if a.ohne_artefakte else fehlende_artefakte(WURZEL)
    if fehlend:
        print(f"[sicherung] genannt, aber nicht da: {', '.join(fehlend)} (ARTEFAKTE_MIT in sicherung.py)")
    vollstaendig, befund = werkzeuge_pruefen()
    print(f"[sicherung] Werkzeugvorrat: {befund}")
    werkzeuge = befund if vollstaendig else f"UNVOLLSTAENDIG ({befund}); diese Kopie braucht zum Einrichten Netz"
    pakete_ok, pakete_befund = pakete_pruefen()
    print(f"[sicherung] Paketvorrat: {pakete_befund}")
    pakete = pakete_befund if pakete_ok else f"UNVOLLSTAENDIG ({pakete_befund}); diese Kopie braucht zum Bauen Netz"
    s_ordner = secure_ordner(WURZEL)
    secure: Auswahl | None = None
    eingehaengt: list[tuple[pathlib.Path, list[str]]] = []
    if a.ohne_secure:
        secure_befund = "bewusst nicht dabei (--ohne-secure)"
    elif not s_ordner.is_dir():
        secure_befund = f"keine neben dem Repositorium ({s_ordner})"
    else:
        secure = secure_auswaehlen(s_ordner)
        eingehaengt = secure_eingehaengt(s_ordner, eingehaengte_abbilder())
        secure_befund = f"{len(secure.dateien)} Dateien, {secure.groesse / 1e9:.2f} GB, im Ordner `{SECURE_NAME}/`"
    print(f"[sicherung] Schluesselablage: {secure_befund}")
    for pfad, punkte in eingehaengt:
        print(f"[sicherung]   eingehaengt: {pfad.name} unter {', '.join(punkte) or 'ohne Einhaengepunkt'}")
    if a.liste:
        return 0
    # ⛔️ **Eine Kopie ohne die Modelle, die sie mitnehmen soll, nur auf
    #    ausdruecklichen Wunsch.** Meist ist ein Name veraltet.
    if fehlend:
        print("[sicherung] FEHLER: Diese Artefakte sollen mit, liegen aber nicht da. Umbenannt? Dann ARTEFAKTE_MIT nachziehen.")
        print("[sicherung]   Oder bewusst ohne Artefakte: --ohne-artefakte")
        return 1
    # ⛔️ **Eine Kopie, die ohne Netz nicht baut, nur auf ausdruecklichen
    #    Wunsch.** Beide Vorraete lassen sich nur hier holen, mit Netz.
    if not vollstaendig and not a.ohne_werkzeuge:
        print("[sicherung] FEHLER: Der Werkzeugvorrat ist nicht vollstaendig; ohne ihn baut die Kopie nicht ohne Netz.")
        print("[sicherung]   Jetzt, solange Netz da ist: python3 SYSTEM/install/werkzeugvorrat.py holen")
        print("[sicherung]   Oder bewusst ohne: --ohne-werkzeuge")
        return 1
    if not pakete_ok and not a.ohne_werkzeuge:
        print("[sicherung] FEHLER: Der Paketvorrat passt nicht zu den Sperrdateien; die Kopie baute nicht ohne Netz.")
        print("[sicherung]   Jetzt, solange Netz da ist: python3 SYSTEM/install/vorrat.py sammeln")
        print("[sicherung]   Oder bewusst ohne: --ohne-werkzeuge")
        return 1
    # ⛔️ **Ein eingehaengtes Abbild der Schluesselablage wird nicht kopiert**
    #    (siehe `secure_eingehaengt`); erst aushaengen, oder bewusst ohne.
    if eingehaengt:
        punkte = [pt for _, pts in eingehaengt for pt in pts]
        print("[sicherung] FEHLER: Die Schluesselablage ist eingehaengt; eine Kopie mittendrin kann sich danach nicht mehr oeffnen lassen.")
        for pt in punkte:
            print(f"[sicherung]   Erst aushaengen: hdiutil detach {pt}")
        print("[sicherung]   Oder bewusst ohne sie: --ohne-secure")
        return 1
    ziel = pathlib.Path(a.ziel).expanduser().resolve()
    zwischen = pathlib.Path(a.zwischen).expanduser().resolve() if a.zwischen else pathlib.Path(tempfile.gettempdir()).resolve()
    for ordner, wozu in ((ziel, "Das Ziel"), (zwischen, "Der Zwischenordner")):
        if not ordner.is_dir():
            print(f"[sicherung] FEHLER: {wozu} {ordner} ist kein Ordner")
            return 2
        # ⛔️ **Nie in den Baum selbst, nie in die Schluesselablage.** Die
        #    Kopie laege sonst in der naechsten.
        for innen in (WURZEL, s_ordner):
            if ordner == innen or innen in ordner.parents:
                print(f"[sicherung] FEHLER: {wozu} liegt in {innen}; die Kopie gehoert nach draussen.")
                return 2
    datum = datetime.date.today().isoformat()
    lokal = zwischen / f"myelith-sicherung-{datum}{'.zip' if a.zip else '.tar.gz'}"
    gebraucht = auswahl.groesse + (secure.groesse if secure else 0)
    frei_lokal = freier_platz(zwischen)
    if frei_lokal is not None and frei_lokal < gebraucht + RESERVE:
        print(f"[sicherung] FEHLER: im Zwischenordner sind {frei_lokal / 1e9:.1f} GB frei, gebraucht bis {gebraucht / 1e9:.1f} GB"
              " (anderen Ort mit --zwischen)")
        return 1
    print(f"[sicherung] zuerst lokal: {lokal}")
    schreiben(auswahl, lokal, a.zip, datum, werkzeuge, pakete, secure, secure_befund)
    summe = _sha256(lokal)
    print(f"[sicherung] lokal fertig: {lokal.stat().st_size / 1e9:.2f} GB, SHA-256 {summe[:16]}")
    neu = uebertragen(lokal, ziel, summe)
    if neu is None:
        return 1
    # Die Beschreibung auch neben das Archiv: Welcher Stand dort liegt, sieht
    # man so ohne Auspacken. Sie ersetzt die der alten Kopie.
    (ziel / "SICHERUNG.md").write_text(beschreibung(auswahl, datum, werkzeuge, pakete, secure_befund), encoding="utf-8")
    if not a.lokal_behalten:
        lokal.unlink()
        print("[sicherung] lokale Kopie entfernt (behalten mit --lokal-behalten)")
    print(f"[sicherung] {neu} ({neu.stat().st_size / 1e9:.2f} GB)")
    print("[sicherung] Fertig")
    return 0


def selbsttest() -> int:
    """Baut einen kleinen Baum und prueft, was hinein kommt und was nicht."""
    fehler = []
    with tempfile.TemporaryDirectory() as t:
        w = pathlib.Path(t) / "baum"
        def leg(rel: str, inhalt: bytes = b"x") -> None:
            p = w / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(inhalt)
        hinein = [
            "README.md", ".git/HEAD", "SYSTEM/crates-vorrat/a-1.0.0.crate",
            "SYSTEM/crates-vorrat-lokal/b-1.0.0.crate", "INTEGER_LLM/artifacts/README.md",
            "MODELS/llm/Q/config.json", "MODELS/llm/Q/LICENSE", "MODULES/m/src/lib.rs",
            "CLIENT/k/src/main.rs", "SYSTEM/golemos/aus/abbild.img",
            "INTEGER_LLM/artifacts/sparrow-0.6b-int8/gewichte.bin", "SYSTEM/werkzeug-vorrat/nix/x/a.narinfo",
            "SYSTEM/golemos/Quellarchiv/README.md", "SYSTEM/golemos/Quellarchiv/x86_64/quellen-x86_64.tar",
        ]
        draussen = [
            "SYSTEM/full-build/release/myl", "SYSTEM/crates-lager/vendor/a/x.rs",
            "INTEGER_LLM/artifacts/m/gewichte.bin", "MODELS/llm/Q/model.safetensors",
            "MODELS/vision/sehen.gguf", "INTEGER_LLM/calibrate/.venv/bin/python",
            "INTEGER_LLM/calibrate/.venv-flashnext/lib/x.py", "x/__pycache__/y.pyc", "a.pyc",
            ".DS_Store", "CLIENT/k/target/debug/k", "SYSTEM/golemos/bau/x",
            "INTEGER_LLM/tests/integration/_synthetic_artifact/m.json",
        ]
        for r in hinein + draussen:
            leg(r)
        leg("MODELS/llm/Q/riesig.json", b"0" * (GROESSTE_MODELLDATEI + 1))
        draussen.append("MODELS/llm/Q/riesig.json")
        verknuepft = False
        try:
            os.symlink("/gibt/es/nicht", w / "verweis")
            verknuepft = True
        except (OSError, NotImplementedError):
            pass
        a = auswaehlen(w)
        drin = {rel for rel, _, _ in a.dateien}
        # Die Schluesselablage neben dem Baum: alles hinein ausser Finder-Resten.
        sec = secure_ordner(w)
        sec_hinein = ["README.md", "Secure.sparsebundle/Info.plist", "Secure.sparsebundle/bands/0"]
        for r in sec_hinein + [".DS_Store"]:
            p = sec / r
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(b"geheim")
        sa = secure_auswaehlen(sec)
        if sorted(r for r, _, _ in sa.dateien) != sorted(sec_hinein):
            fehler.append(f"Schluesselablage falsch ausgewaehlt: {[r for r, _, _ in sa.dateien]}")
        # Eingehaengt: ein Abbild aus der Ablage zaehlt, ein anderes nicht.
        if not secure_eingehaengt(sec, [(sec / "Secure.sparsebundle", ["/Volumes/Secure"])]):
            fehler.append("eingehaengtes Abbild der Schluesselablage nicht erkannt")
        if secure_eingehaengt(sec, [(pathlib.Path(t) / "anderes.dmg", ["/Volumes/X"])]) or secure_eingehaengt(sec, []):
            fehler.append("fremdes Abbild als Schluesselablage erkannt")
        for r in hinein:
            if r not in drin:
                fehler.append(f"fehlt in der Kopie: {r}")
        for r in draussen:
            if r in drin:
                fehler.append(f"liegt in der Kopie, gehoert nicht hinein: {r}")
        if verknuepft and a.verknuepfungen != ["verweis"]:
            fehler.append(f"Verknuepfung nicht gemeldet: {a.verknuepfungen}")
        if verknuepft and "verweis" in drin:
            fehler.append("Verknuepfung kopiert")
        # Ein genanntes Artefakt, das fehlt, wird gemeldet; ein vorhandenes nicht.
        if fehlende_artefakte(w) != ["pigeon-4b-int8"]:
            fehler.append(f"fehlende Artefakte falsch erkannt: {fehlende_artefakte(w)}")
        # Und eine echte Kopie, ausgepackt und gegen die Pruefsummen gehalten.
        for als_zip in (False, True):
            ziel = pathlib.Path(t) / ("k.zip" if als_zip else "k.tar.gz")
            schreiben(a, ziel, als_zip, "2026-01-01", secure=sa, secure_befund="Probe")
            aus = pathlib.Path(t) / ("aus-zip" if als_zip else "aus-tar")
            if als_zip:
                with zipfile.ZipFile(ziel) as z:
                    z.extractall(aus)
            else:
                with tarfile.open(ziel) as tf:
                    if hasattr(tarfile, "data_filter"):
                        tf.extractall(aus, filter="data")
                    else:
                        tf.extractall(aus)
            kopf = aus / KOPFORDNER
            for zeile in (kopf / "PRUEFSUMMEN.sha256").read_text().splitlines():
                summe, rel = zeile.split("  ", 1)
                if _sha256(kopf / rel) != summe:
                    fehler.append(f"Pruefsumme passt nicht: {rel}")
            if not (kopf / "SICHERUNG.md").is_file():
                fehler.append("SICHERUNG.md fehlt")
            elif "**Schluesselablage:** Probe" not in (kopf / "SICHERUNG.md").read_text(encoding="utf-8"):
                fehler.append("SICHERUNG.md nennt die Schluesselablage nicht")
            s_kopf = aus / SECURE_NAME
            s_zeilen = (s_kopf / "PRUEFSUMMEN.sha256").read_text().splitlines() if (s_kopf / "PRUEFSUMMEN.sha256").is_file() else []
            if sorted(z.split("  ", 1)[1] for z in s_zeilen) != sorted(sec_hinein):
                fehler.append(f"Schluesselablage in der Kopie unvollstaendig ({'zip' if als_zip else 'tar'})")
            for zeile in s_zeilen:
                summe, rel = zeile.split("  ", 1)
                if _sha256(s_kopf / rel) != summe:
                    fehler.append(f"Pruefsumme der Schluesselablage passt nicht: {rel}")
        # Uebertragen: lokal fertig, dann auf den Traeger, die alte ersetzt.
        lokal = pathlib.Path(t) / "lokal" / "myelith-sicherung-2026-01-01.tar.gz"
        lokal.parent.mkdir()
        lokal.write_bytes((pathlib.Path(t) / "k.tar.gz").read_bytes())
        summe = _sha256(lokal)
        traeger = pathlib.Path(t) / "traeger"
        traeger.mkdir()
        alt = traeger / "myelith-sicherung-2025-12-01.tar.gz"
        anderes_format = traeger / "myelith-sicherung-2025-12-01.zip"

        def traeger_neu() -> None:
            for f in traeger.iterdir():
                f.unlink()
            alt.write_bytes(b"alt" * 1000)
            anderes_format.write_bytes(b"zip")
            (traeger / "myelith-sicherung-2025-11-01.tar.gz.halb").write_bytes(b"rest")

        def stand() -> list[str]:
            return sorted(f.name for f in traeger.iterdir())

        neu_name = "myelith-sicherung-2026-01-01.tar.gz"
        # 1. genug Platz: neu da und gleich, alte weg, anderes Format bleibt, Rest weg
        traeger_neu()
        neu = uebertragen(lokal, traeger, summe, frei=lambda _: 10 ** 12)
        if neu is None or stand() != sorted([neu_name, anderes_format.name]) or _sha256(neu) != summe:
            fehler.append(f"Uebertragen mit Platz: {stand()}")
        # 2. knapp: nur nach dem Loeschen der alten passt die neue
        traeger_neu()
        knapp = lokal.stat().st_size + RESERVE - 1
        neu = uebertragen(lokal, traeger, summe, frei=lambda _: knapp)
        if neu is None or stand() != sorted([neu_name, anderes_format.name]):
            fehler.append(f"Uebertragen bei knappem Platz: {stand()}")
        # 2b. knapp und die Kopie misslingt: Die alte war schon weg (sie geht
        #     zuerst), die neue bleibt lokal. Belegt die Reihenfolge.
        traeger_neu()
        if uebertragen(lokal, traeger, "0" * 64, frei=lambda _: knapp) is not None or alt.name in stand():
            fehler.append(f"bei knappem Platz ging die alte nicht zuerst: {stand()}")
        # 3. zu knapp auch ohne die alte: nichts geloescht, nichts geschrieben
        traeger_neu()
        if uebertragen(lokal, traeger, summe, frei=lambda _: 0) is not None or alt.name not in stand() or neu_name in stand():
            fehler.append(f"Uebertragen ohne Platz hat etwas veraendert: {stand()}")
        # 4. Die Kopie passt nicht zur Pruefsumme: die alte bleibt, keine neue
        traeger_neu()
        if uebertragen(lokal, traeger, "0" * 64, frei=lambda _: 10 ** 12) is not None \
                or alt.name not in stand() or neu_name in stand() or any(n.endswith(".halb") for n in stand()):
            fehler.append(f"Uebertragen trotz falscher Pruefsumme: {stand()}")
    if fehler:
        for f in fehler:
            print(f"[sicherung] {f}")
        print("[sicherung] Selbsttest FAILED")
        return 1
    print("[sicherung] Selbsttest PASSED: Auswahl, Verknuepfungen, Schluesselablage, tar und zip mit Pruefsummen,"
          " Uebertragen mit, bei knappem und ohne Platz, falsche Pruefsumme")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
