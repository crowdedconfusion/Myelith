#!/usr/bin/env python3
"""Der Vorrat der Fremdquellen als Archive: sammeln, prüfen, auspacken.

# ⚑ Warum Archive und nicht ausgepackte Quellen

Gemessen am 2026-09-17 an denselben 758 Paketen:

| Weg | Im Baum | Dateien | Wächst je Fassungssprung um |
|---|---|---|---|
| `cargo vendor`, ausgepackt | 943 MB | 36 805 | die geänderten Pakete, als neue Dateien |
| ein gepacktes Bündel | 122 MB | 1 | **jedes Mal 122 MB**, denn ein Archiv ändert sich ganz |
| **Archive je Paket** | **116 MB** | **758** | **nur die neuen Archive**, meist wenige MB |

⚑ **Der letzte Weg ist der einzige, der nicht mitwächst.** Jede
Paketfassung ist eine eigene, unveränderliche Datei: Wer `tokio` anhebt,
legt ein Archiv dazu, und die 757 anderen rühren sich nicht. **Ein
gepacktes Bündel dagegen ist bei jeder Änderung ein neues Bündel**, und
die alten bleiben für immer in der Geschichte.

⚑ **Ausgepackt wird beim Einrichten**, in einen Ordner, der nicht
versioniert wird. Gemessen: **5 Sekunden** für alle 758 Pakete.

# ⛔️ Und es ist prüfbar, nicht nur bequem

Jede Sperrdatei nennt zu jedem Paket seine SHA-256. **Diese Archive
lassen sich also gegen den Stand prüfen, den das Repositorium ohnehin
festhält**, und `pruefen` tut genau das. Ein Vorrat, dem man ansieht,
dass er zum Stand gehört, ist etwas anderes als ein Ordner voller
Fremddateien.

# ⚑ Zwei Vorraete: der versionierte und ein lokaler daneben

Lokale Kisten, die git ignoriert (eigene Module unter `MODULES/`), tragen
eigene Sperrdateien. Was nur sie brauchen, liegt **nicht** im versionierten
Vorrat, denn kein Klon braucht es, sondern in `SYSTEM/crates-vorrat-lokal/`,
das git ebenfalls ignoriert. Damit baut eine Kopie des ganzen Arbeitsbaums
auch diese Kisten ohne Netz, und der versionierte Vorrat bleibt genau das,
was die versionierten Sperrdateien verlangen. `auspacken` legt beide in
dasselbe Lager.

Aufrufe:

```text
python3 SYSTEM/install/vorrat.py sammeln     # aus dem Cargo-Vorrat, sonst aus dem Netz
python3 SYSTEM/install/vorrat.py pruefen     # jede Datei gegen die Sperrdateien
python3 SYSTEM/install/vorrat.py auspacken   # nach SYSTEM/crates-lager/vendor
```
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile
import time

# ⚠️ **`parents[2]` und nicht `parents[1]`**: Diese Datei liegt seit dem
# Umzug unter `SYSTEM/install/`, also zwei Ebenen unter der Wurzel. Wer die
# Zahl beim Verschieben vergisst, bekommt keinen Fehler, sondern einen
# Vorrat an der falschen Stelle.
WURZEL = pathlib.Path(__file__).resolve().parents[2]
ARCHIVE = WURZEL / "SYSTEM" / "crates-vorrat"
ARCHIVE_LOKAL = WURZEL / "SYSTEM" / "crates-vorrat-lokal"
AUSGEPACKT = WURZEL / "SYSTEM" / "crates-lager" / "vendor"
QUELLE = "https://static.crates.io/crates/{name}/{name}-{version}.crate"

# ⛔️ **Nicht jede Sperrdatei im Baum gehoert zu Myelith.** Unter
# `INSPIRATION` liegen fremde Repositorien, und die Fuzz-Kisten sind
# Werkzeug und kein Programm. **Ein Vorrat, der sie mitnimmt, waere
# groesser und nicht vollstaendiger.**
AUSGENOMMEN = ("INSPIRATION", "/fuzz/", "SYSTEM/full-build", "/.venv/", "/vendor/")


def von_git_ignoriert(pfade: list[pathlib.Path]) -> set[pathlib.Path]:
    """Die Sperrdateien, die git ignoriert: lokale Kisten ohne Versionierung.

    📌 Lokale Module unter `MODULES/` werden nicht versioniert, tragen aber
    eine eigene Sperrdatei. Ohne diese Abfrage legte `sammeln` ihre Archive
    in den versionierten Vorrat, den kein Klon je braucht, und `pruefen`
    meldete sie dort als fehlend, sobald sie fehlten (2026-10-14: vier
    Archive, `pruefen` lokal dauerhaft rot). Eine neue Kiste, deren
    Sperrdatei noch nicht eingecheckt, aber auch nicht ignoriert ist,
    bleibt dabei: Gefragt wird nach Ignorieren, nicht nach Versioniertsein.

    Ohne git (ein entpacktes Archiv des Baums) zaehlt jede Sperrdatei.
    """
    if not (WURZEL / ".git").exists() or shutil.which("git") is None:
        return set()
    eingabe = "\n".join(p.relative_to(WURZEL).as_posix() for p in pfade)
    try:
        # Rueckgabe 1 heisst "nichts ignoriert" und ist kein Fehler.
        lauf = subprocess.run(
            ["git", "-C", str(WURZEL), "check-ignore", "--stdin"],
            input=eingabe,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError):
        return set()
    if lauf.returncode not in (0, 1):
        return set()
    return {WURZEL / z for z in lauf.stdout.splitlines() if z}


def _kandidaten() -> list[pathlib.Path]:
    return [
        p
        for p in WURZEL.rglob("Cargo.lock")
        if not any(a in p.as_posix() for a in AUSGENOMMEN)
    ]


def sperrdateien() -> list[pathlib.Path]:
    kandidaten = _kandidaten()
    ignoriert = von_git_ignoriert(kandidaten)
    return sorted(p for p in kandidaten if p not in ignoriert)


def sperrdateien_lokal() -> list[pathlib.Path]:
    """Die Sperrdateien der lokalen Kisten, die git ignoriert.

    Ohne git ist diese Liste leer, und jede Sperrdatei zaehlt zu
    `sperrdateien()`; `pruefen` sieht dann in beiden Vorraeten nach.
    """
    kandidaten = _kandidaten()
    return sorted(von_git_ignoriert(kandidaten))


def pakete(locks: list[pathlib.Path] | None = None) -> dict[tuple[str, str], str]:
    """Paket und Fassung zu SHA-256, aus den genannten Sperrdateien."""
    aus: dict[tuple[str, str], str] = {}
    for lock in sperrdateien() if locks is None else locks:
        # ⚑ **Nur Zeilen mit Anfuehrungszeichen**: In einer Sperrdatei
        # steht auch `version = 4` (die Fassung des Dateiformats), und
        # die hat keine. **Ein Muster, das darueber stolpert, faellt
        # nicht auf, sondern bricht ab.**
        name = version = None
        for z in lock.read_text(errors="replace").splitlines():
            teile = z.split('"')
            if len(teile) < 2:
                continue
            if z.startswith("name = "):
                name, version = teile[1], None
            elif z.startswith("version = "):
                version = teile[1]
            elif z.startswith("checksum = ") and name and version:
                aus[(name, version)] = teile[1]
                name = version = None
    return aus


def aus_dem_cargo_vorrat() -> dict[str, pathlib.Path]:
    cache = pathlib.Path.home() / ".cargo" / "registry" / "cache"
    return {p.stem: p for c in cache.glob("*") for p in c.glob("*.crate")}


def sammeln() -> int:
    """Holt jedes Archiv, erst aus dem Cargo-Vorrat, sonst aus dem Netz."""
    alle = pakete()
    print(f"[vorrat] {len(alle)} Paketfassungen aus {len(sperrdateien())} Sperrdateien")
    rc = _sammeln_nach(ARCHIVE, alle)
    lokal = sperrdateien_lokal()
    if lokal:
        # ⛔️ **Nur, was der versionierte Vorrat nicht schon hat**, und nie
        # dorthin: Fund 545 war genau ein lokales Archiv im oeffentlichen
        # Vorrat.
        nur_lokal = {k: s for k, s in pakete(lokal).items() if k not in alle}
        print(f"[vorrat] lokal: {len(nur_lokal)} weitere Paketfassungen aus {len(lokal)} Sperrdateien")
        rc |= _sammeln_nach(ARCHIVE_LOKAL, nur_lokal)
    return rc


def _sammeln_nach(ablage: pathlib.Path, alle: dict[tuple[str, str], str]) -> int:
    ablage.mkdir(exist_ok=True)
    vorhanden = aus_dem_cargo_vorrat()
    geholt = kopiert = schon = 0
    fehler = []
    for (name, version), summe in sorted(alle.items()):
        ziel = ablage / f"{name}-{version}.crate"
        if ziel.is_file() and hashlib.sha256(ziel.read_bytes()).hexdigest() == summe:
            schon += 1
            continue
        quelle = vorhanden.get(f"{name}-{version}")
        if quelle:
            shutil.copy2(quelle, ziel)
            kopiert += 1
        else:
            import urllib.request

            try:
                with urllib.request.urlopen(
                    QUELLE.format(name=name, version=version), timeout=60
                ) as a:
                    ziel.write_bytes(a.read())
                geholt += 1
            except Exception as f:  # pragma: no cover
                fehler.append(f"{name}-{version}: {f}")
                continue
        # ⛔️ **Geprueft wird sofort und nicht spaeter.** Ein falsches
        # Archiv im Vorrat faellt sonst erst beim Bau auf, und dann sieht
        # es wie ein Fehler des Bauens aus.
        if hashlib.sha256(ziel.read_bytes()).hexdigest() != summe:
            fehler.append(f"{name}-{version}: Pruefsumme passt nicht zur Sperrdatei")
            ziel.unlink(missing_ok=True)

    groesse = sum(p.stat().st_size for p in ablage.glob("*.crate"))
    print(
        f"[vorrat] {schon} schon da, {kopiert} aus dem Cargo-Vorrat, {geholt} aus dem Netz; "
        f"jetzt {len(list(ablage.glob('*.crate')))} Archive, {groesse/1e6:.0f} MB in {ablage.name}"
    )
    if fehler:
        print(f"[vorrat] FEHLER bei {len(fehler)}:")
        for f in fehler[:5]:
            print(f"   {f}")
        return 1
    return 0


def _archive(ablage: pathlib.Path) -> dict[str, pathlib.Path]:
    return {p.name: p for p in ablage.glob("*.crate")} if ablage.is_dir() else {}


def pruefen() -> int:
    """Jede Datei gegen die Sperrdateien, und keine zu viel."""
    alle = pakete()
    lokal = sperrdateien_lokal()
    rc = _pruefen_gegen(alle, ARCHIVE, "versioniert")
    if lokal:
        nur_lokal = {k: s for k, s in pakete(lokal).items() if k not in alle}
        rc |= _pruefen_gegen(nur_lokal, ARCHIVE_LOKAL, "lokal")
    return rc


def _pruefen_gegen(alle: dict[tuple[str, str], str], ablage: pathlib.Path, art: str) -> int:
    erwartet = {f"{n}-{v}.crate": s for (n, v), s in alle.items()}
    da = _archive(ablage)
    if ablage == ARCHIVE and not sperrdateien_lokal():
        # Ohne git (eine Kopie ohne `.git`) kennt niemand die lokalen
        # Sperrdateien als lokal; ihre Archive stehen im lokalen Vorrat.
        da = {**_archive(ARCHIVE_LOKAL), **da}
    fehlt = sorted(set(erwartet) - set(da))
    zuviel = sorted(set(da) - set(erwartet))
    falsch = [
        n
        for n, p in sorted(da.items())
        if n in erwartet and hashlib.sha256(p.read_bytes()).hexdigest() != erwartet[n]
    ]
    print(f"[vorrat] {art}: erwartet {len(erwartet)}, vorhanden {len(da)}")
    if fehlt:
        print(f"   fehlen: {len(fehlt)}  {fehlt[:3]}")
    if falsch:
        print(f"   falsche Pruefsumme: {len(falsch)}  {falsch[:3]}")
    # ⚠️ Ueberzaehlige sind kein Fehler, sondern Ballast: Sie stammen von
    # einer frueheren Fassung und gehoeren aufgeraeumt.
    if zuviel and art == "versioniert":
        zuviel = [n for n in zuviel if n not in _archive(ARCHIVE_LOKAL)]
    if zuviel:
        print(f"   ueberzaehlig (alte Fassungen): {len(zuviel)}  {zuviel[:3]}")
    if fehlt or falsch:
        print(f"[vorrat] FAILED ({art})")
        return 1
    print(f"[vorrat] PASSED: der Vorrat ({art}) passt zu den Sperrdateien")
    return 0


def auspacken() -> int:
    """Legt die Quellen dort hin, wo cargo sie als Quelle nehmen kann."""
    if not ARCHIVE.is_dir() or not any(ARCHIVE.glob("*.crate")):
        print(f"[vorrat] kein Vorrat unter {ARCHIVE}")
        return 1
    shutil.rmtree(AUSGEPACKT, ignore_errors=True)
    AUSGEPACKT.mkdir(parents=True)
    anfang = time.time()
    n = 0
    archive = {**_archive(ARCHIVE_LOKAL), **_archive(ARCHIVE)}
    for _, p in sorted(archive.items()):
        summe = hashlib.sha256(p.read_bytes()).hexdigest()
        with tarfile.open(p) as t:
            # ⚑ Der Filter steht ausdruecklich da: Python 3.14 filtert sonst
            #   von sich aus, und 3.12 warnt bei jedem Archiv. Vertraut wird,
            #   weil jedes Archiv gegen die Sperrdateien geprueft ist.
            if hasattr(tarfile, "fully_trusted_filter"):
                t.extractall(AUSGEPACKT, filter="fully_trusted")
            else:
                t.extractall(AUSGEPACKT)
        ordner = AUSGEPACKT / p.stem
        if not ordner.is_dir():
            print(f"[vorrat] {p.name} enthaelt keinen Ordner {p.stem}")
            return 1
        # ⚑ **Die Pruefsummendatei, die cargo fuer eine Verzeichnisquelle
        # erwartet.** Die Liste der Einzeldateien bleibt leer: Sie
        # schuetzt nur gegen versehentliche Aenderungen, und die
        # eigentliche Zusage steht im Paketabdruck darueber, den
        # `pruefen` gegen die Sperrdatei haelt.
        (ordner / ".cargo-checksum.json").write_text(
            json.dumps({"files": {}, "package": summe})
        )
        n += 1
    konfig = AUSGEPACKT.parent / "cargo-home"
    konfig.mkdir(exist_ok=True)
    # ⚑ **Ein relativer Pfad, kein absoluter.** Cargo loest einen Pfad in
    #    `$CARGO_HOME/config.toml` gegen den Ordner UEBER `cargo-home` auf,
    #    also gegen `crates-lager`; `vendor` ist damit genau `AUSGEPACKT`.
    #    Mit dem absoluten Pfad, der hier bis zum 2026-10-21 stand, zeigte
    #    ein verschobener oder kopierter Baum auf das alte Lager (Fund 580).
    #    Geprueft mit einer Gegenprobe: ein falscher Name bricht ab, statt
    #    still den Cargo-Vorrat zu nehmen.
    #    📌 Der Weg ueber den absoluten Pfad brauchte ausserdem
    #    Vorwaertsschraegstriche in einer woertlichen TOML-Zeichenkette,
    #    weil `D:\a\Myelith` in einer gewoehnlichen ungueltig war; ein
    #    relativer Name hat das Problem nicht.
    assert AUSGEPACKT.parent == konfig.parent
    (konfig / "config.toml").write_text(
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n"
        f"[source.vendored-sources]\ndirectory = '{AUSGEPACKT.name}'\n"
    )
    print(f"[vorrat] {n} Pakete in {time.time()-anfang:.0f} s nach {AUSGEPACKT}")
    print(f"[vorrat] bauen mit: CARGO_HOME={konfig} cargo build --release --locked --offline")
    return 0


def main() -> int:
    if len(sys.argv) < 2 or sys.argv[1] not in {"sammeln", "pruefen", "auspacken"}:
        print(__doc__)
        return 2
    return {"sammeln": sammeln, "pruefen": pruefen, "auspacken": auspacken}[sys.argv[1]]()


if __name__ == "__main__":
    raise SystemExit(main())
