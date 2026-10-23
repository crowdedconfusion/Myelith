#!/usr/bin/env python3
"""Eine Sicherheitskopie des ganzen Arbeitsbaums, ohne Gewichte und ohne Bauabfall.

Aufruf (aus der Wurzel oder von irgendwo):

```text
python3 SYSTEM/install/sicherung.py <zielordner>           # myelith-sicherung-JJJJ-MM-TT.tar.gz
python3 SYSTEM/install/sicherung.py <zielordner> --zip     # dasselbe als .zip, fuer Windows
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


def beschreibung(a: Auswahl, datum: str, werkzeuge: str = "nicht geprueft", pakete: str = "nicht geprueft") -> str:
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

    def datei(self, rel: str, p: pathlib.Path) -> None:
        name = f"{KOPFORDNER}/{rel}"
        if self.als_zip:
            self.z.write(p, name)
        else:
            self.t.add(str(p), arcname=name, recursive=False)

    def text(self, rel: str, inhalt: str) -> None:
        daten = inhalt.encode("utf-8")
        name = f"{KOPFORDNER}/{rel}"
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
              pakete: str = "nicht geprueft") -> None:
    # ⚑ Erst in eine Zwischendatei, dann umbenennen: Ein abgebrochener Lauf
    # hinterlaesst keine halbe Kopie unter dem endgueltigen Namen.
    halb = ziel.with_name(ziel.name + ".halb")
    w = _Schreiber(halb, als_zip)
    summen = []
    anfang = time.time()
    fertig = 0
    gesamt = a.groesse or 1
    naechste_meldung = 0.0
    try:
        for rel, p, groesse in a.dateien:
            w.datei(rel, p)
            summen.append(f"{_sha256(p)}  {rel}")
            fertig += groesse
            jetzt = time.time()
            if jetzt >= naechste_meldung:
                print(f"\r[sicherung] {fertig / gesamt * 100:5.1f} %  {fertig / 1e9:6.2f} von {gesamt / 1e9:.2f} GB", end="", flush=True)
                naechste_meldung = jetzt + 2
        w.text("SICHERUNG.md", beschreibung(a, datum, werkzeuge, pakete))
        w.text("PRUEFSUMMEN.sha256", "\n".join(summen) + "\n")
    finally:
        w.schliessen()
    halb.replace(ziel)
    print(f"\r[sicherung] 100.0 %  {gesamt / 1e9:.2f} GB in {time.time() - anfang:.0f} s" + " " * 10)


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
    ap.add_argument("ziel", nargs="?", help="Ordner, in den das Archiv geschrieben wird (nicht im Baum)")
    ap.add_argument("--zip", action="store_true", help="als .zip statt .tar.gz")
    ap.add_argument("--liste", action="store_true", help="nur zeigen, was hinein kaeme")
    ap.add_argument("--selbsttest", action="store_true", help="die Regeln an einem kleinen Baum pruefen")
    ap.add_argument("--ohne-artefakte", action="store_true", help=f"auch {', '.join(ARTEFAKTE_MIT)} draussen lassen")
    ap.add_argument("--ohne-werkzeuge", action="store_true",
                    help="auch mit lueckenhaftem Werkzeug- oder Paketvorrat schreiben (die Kopie braucht dann Netz)")
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
    ziel = pathlib.Path(a.ziel).expanduser().resolve()
    if not ziel.is_dir():
        print(f"[sicherung] FEHLER: {ziel} ist kein Ordner")
        return 2
    # ⛔️ **Nie in den Baum selbst.** Die Kopie laege sonst in der naechsten.
    if ziel == WURZEL or WURZEL in ziel.parents:
        print("[sicherung] FEHLER: Das Ziel liegt im Baum; die Kopie gehoert nach draussen.")
        return 2
    datum = datetime.date.today().isoformat()
    datei = ziel / f"myelith-sicherung-{datum}{'.zip' if a.zip else '.tar.gz'}"
    if datei.exists():
        print(f"[sicherung] FEHLER: {datei} gibt es schon; nichts ueberschrieben.")
        return 2
    frei = os.statvfs(ziel).f_bavail * os.statvfs(ziel).f_frsize if hasattr(os, "statvfs") else None
    if frei is not None and frei < auswahl.groesse:
        print(f"[sicherung] FEHLER: am Ziel sind {frei / 1e9:.1f} GB frei, gebraucht bis {auswahl.groesse / 1e9:.1f} GB")
        return 1
    schreiben(auswahl, datei, a.zip, datum, werkzeuge, pakete)
    print(f"[sicherung] {datei} ({datei.stat().st_size / 1e9:.2f} GB)")
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
            schreiben(a, ziel, als_zip, "2026-01-01")
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
    if fehler:
        for f in fehler:
            print(f"[sicherung] {f}")
        print("[sicherung] Selbsttest FAILED")
        return 1
    print("[sicherung] Selbsttest PASSED: Auswahl, Verknuepfungen, tar und zip mit Pruefsummen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
