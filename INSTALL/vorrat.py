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

Aufrufe:

```text
python3 INSTALL/vorrat.py sammeln     # aus dem Cargo-Vorrat, sonst aus dem Netz
python3 INSTALL/vorrat.py pruefen     # jede Datei gegen die Sperrdateien
python3 INSTALL/vorrat.py auspacken   # nach .myelith-vorrat/vendor
```
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
import shutil
import sys
import tarfile
import time

WURZEL = pathlib.Path(__file__).resolve().parents[1]
ARCHIVE = WURZEL / "vorrat"
AUSGEPACKT = WURZEL / ".myelith-vorrat" / "vendor"
QUELLE = "https://static.crates.io/crates/{name}/{name}-{version}.crate"

# ⛔️ **Nicht jede Sperrdatei im Baum gehoert zu Myelith.** Unter
# `INSPIRATION` liegen fremde Repositorien, und die Fuzz-Kisten sind
# Werkzeug und kein Programm. **Ein Vorrat, der sie mitnimmt, waere
# groesser und nicht vollstaendiger.**
AUSGENOMMEN = ("INSPIRATION", "/fuzz/", "target-shared", "/.venv/", "/vendor/")


def sperrdateien() -> list[pathlib.Path]:
    return sorted(
        p
        for p in WURZEL.rglob("Cargo.lock")
        if not any(a in str(p) for a in AUSGENOMMEN)
    )


def pakete() -> dict[tuple[str, str], str]:
    """Paket und Fassung zu SHA-256, aus allen Sperrdateien."""
    aus: dict[tuple[str, str], str] = {}
    for lock in sperrdateien():
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
    ARCHIVE.mkdir(exist_ok=True)
    vorhanden = aus_dem_cargo_vorrat()
    alle = pakete()
    print(f"[vorrat] {len(alle)} Paketfassungen aus {len(sperrdateien())} Sperrdateien")
    geholt = kopiert = schon = 0
    fehler = []
    for (name, version), summe in sorted(alle.items()):
        ziel = ARCHIVE / f"{name}-{version}.crate"
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

    groesse = sum(p.stat().st_size for p in ARCHIVE.glob("*.crate"))
    print(
        f"[vorrat] {schon} schon da, {kopiert} aus dem Cargo-Vorrat, {geholt} aus dem Netz; "
        f"jetzt {len(list(ARCHIVE.glob('*.crate')))} Archive, {groesse/1e6:.0f} MB"
    )
    if fehler:
        print(f"[vorrat] FEHLER bei {len(fehler)}:")
        for f in fehler[:5]:
            print(f"   {f}")
        return 1
    return 0


def pruefen() -> int:
    """Jede Datei gegen die Sperrdateien, und keine zu viel."""
    alle = pakete()
    erwartet = {f"{n}-{v}.crate": s for (n, v), s in alle.items()}
    da = {p.name: p for p in ARCHIVE.glob("*.crate")} if ARCHIVE.is_dir() else {}
    fehlt = sorted(set(erwartet) - set(da))
    zuviel = sorted(set(da) - set(erwartet))
    falsch = [
        n
        for n, p in sorted(da.items())
        if n in erwartet and hashlib.sha256(p.read_bytes()).hexdigest() != erwartet[n]
    ]
    print(f"[vorrat] erwartet {len(erwartet)}, vorhanden {len(da)}")
    if fehlt:
        print(f"   fehlen: {len(fehlt)}  {fehlt[:3]}")
    if falsch:
        print(f"   falsche Pruefsumme: {len(falsch)}  {falsch[:3]}")
    # ⚠️ Ueberzaehlige sind kein Fehler, sondern Ballast: Sie stammen von
    # einer frueheren Fassung und gehoeren aufgeraeumt.
    if zuviel:
        print(f"   ueberzaehlig (alte Fassungen): {len(zuviel)}  {zuviel[:3]}")
    if fehlt or falsch:
        print("[vorrat] FAILED")
        return 1
    print("[vorrat] PASSED: der Vorrat passt zu den Sperrdateien")
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
    for p in sorted(ARCHIVE.glob("*.crate")):
        summe = hashlib.sha256(p.read_bytes()).hexdigest()
        with tarfile.open(p) as t:
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
    (konfig / "config.toml").write_text(
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n"
        f"[source.vendored-sources]\ndirectory = \"{AUSGEPACKT}\"\n"
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
