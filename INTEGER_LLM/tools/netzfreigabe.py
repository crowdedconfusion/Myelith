#!/usr/bin/env python3
"""Die Netzliste: Abdruecke messen, pruefen und ein Modell fuer das Netz freigeben.

Aufruf (aus der Wurzel):

```text
python3 INTEGER_LLM/tools/netzfreigabe.py abdruck <artefakt>         # den Abdruck eines Artefakts
python3 INTEGER_LLM/tools/netzfreigabe.py liste                      # was freigegeben ist
python3 INTEGER_LLM/tools/netzfreigabe.py pruefen                    # jede Freigabe gegen das Artefakt, wo es liegt
python3 INTEGER_LLM/tools/netzfreigabe.py freigeben <name> [<artefakt>] --grund "<text>" [--pruefnetz]
python3 INTEGER_LLM/tools/netzfreigabe.py entziehen <name> [--pruefnetz]
```

# ⚑ Die Regel

Lokal darf jedes Modell rechnen, gleich von welchem Anbieter. **Im Netz
rechnet nur, was auf der Netzliste steht, und nur mit dem Abdruck, der dort
steht.** Die Liste ist der Block `netz` an einem Eintrag des Modellkatalogs
`MODELS/llm/KATALOG.json`; die Laufzeit bindet den Katalog beim Bauen ein und
laedt im Netzbetrieb nichts anderes. Ein Modell freigeben heisst also: den
Abdruck des geprueften Artefakts eintragen und eine neue Fassung ausliefern.

⛔️ **Freigeben ist eine Entscheidung des Projekts**, nicht ein Handgriff:
Lizenz (Dienst fuer Dritte erlaubt?), Herkunft (feste Revision), Guete
(gemessen) und das Rezept, aus dem andere das Artefakt bitgleich nachbauen.
`freigeben` verweigert ein Modell, das der Katalog als privat fuehrt, und
eines ohne feste Revision.

# ⚑ Das Pruefnetz

Im Netz rechnet nur das 35B (Festlegung des Projektinhabers, 2026-10-21).
Die Netzproben von Pod, Testklient und Klient brauchen ein kleines Modell;
`--pruefnetz` traegt es in den Block `pruefnetz` statt `netz` ein. Den liest
die Laufzeit nur mit ihrem Merkmal `pruefnetz`, und das setzen allein die
Proben (`cargo test`), nie ein ausgeliefertes Programm.

# ⚑ Der Abdruck

SHA-256 ueber die Zeilen `myelith-artefakt-abdruck 1`, dann je
`theta_v.json`, `model_config.json`, `tokenizer.json` die Zeile
`<datei> <sha256>` (fehlt der Tokenizer: `-`). Dieselbe Rechnung steht in
`INTEGER_LLM/runtime/src/netzfreigabe.rs`; beide Proben halten denselben Wert
fest.
"""

from __future__ import annotations

import datetime
import hashlib
import json
import sys
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[2]
KATALOG = WURZEL / "MODELS" / "llm" / "KATALOG.json"
ARTEFAKTE = WURZEL / "INTEGER_LLM" / "artifacts"
ABDRUCK_DATEIEN = ("theta_v.json", "model_config.json", "tokenizer.json")
ABDRUCK_FORM = "myelith-artefakt-abdruck 1"


def sha256(daten: bytes) -> str:
    return hashlib.sha256(daten).hexdigest()


def abdruck(artefakt: Path) -> str:
    zeilen = [ABDRUCK_FORM]
    for datei in ABDRUCK_DATEIEN:
        p = artefakt / datei
        if not p.is_file():
            if datei == "tokenizer.json":
                zeilen.append(f"{datei} -")
                continue
            raise SystemExit(f"[netz] {p} fehlt; ohne sie gibt es keinen Abdruck")
        zeilen.append(f"{datei} {sha256(p.read_bytes())}")
    return sha256(("\n".join(zeilen) + "\n").encode())


def katalog_lesen() -> dict:
    return json.loads(KATALOG.read_text(encoding="utf-8"))


def katalog_schreiben(k: dict) -> None:
    # ⚑ `indent=2` ist die Form, in der der Katalog liegt: Er wird
    #   unveraendert zurueckgeschrieben, bis auf den einen Block.
    KATALOG.write_text(json.dumps(k, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def eintraege(k: dict) -> dict:
    return {n: e for n, e in k.items() if not n.startswith("_") and isinstance(e, dict)}


BLOECKE = ("netz", "pruefnetz")


def liste() -> int:
    k = katalog_lesen()
    for block in BLOECKE:
        frei = {n: e[block] for n, e in eintraege(k).items() if block in e}
        for n, z in sorted(frei.items()):
            print(f"[{block}] {n:24} {z['abdruck'][:16]}  {z.get('freigegeben', '')}")
    netz = sum("netz" in e for e in eintraege(k).values())
    pruef = sum("pruefnetz" in e for e in eintraege(k).values())
    print(f"[netz] {netz} im Netz, {pruef} nur in den Proben; alle anderen Modelle rechnen nur lokal")
    return 0


def pruefen() -> int:
    k = katalog_lesen()
    fehler, geprueft, fehlt = [], 0, []
    for n, e in sorted(eintraege(k).items()):
        for block in BLOECKE:
            if block not in e:
                continue
            if e.get("betrieb") == "lokal":
                fehler.append(f"{n}: privat und zugleich freigegeben ({block})")
            a = ARTEFAKTE / n
            if not (a / "theta_v.json").is_file():
                fehlt.append(n)
                continue
            gemessen = abdruck(a)
            geprueft += 1
            if gemessen != e[block]["abdruck"]:
                fehler.append(f"{n}: Abdruck {gemessen[:16]} gemessen, {e[block]['abdruck'][:16]} freigegeben ({block})")
    for f in fehler:
        print(f"   {f}")
    if fehlt:
        print(f"[netz] nicht gemessen, das Artefakt liegt hier nicht: {', '.join(fehlt)}")
    if fehler:
        print("[netz] FAILED")
        return 1
    print(f"[netz] PASSED: {geprueft} Freigaben passen zu ihrem Artefakt")
    return 0


def freigeben(name: str, artefakt: Path, grund: str, block: str = "netz") -> int:
    k = katalog_lesen()
    e = eintraege(k).get(name)
    if e is None:
        print(f"[netz] FEHLER: {name} steht nicht im Katalog; ein Netzmodell braucht einen Katalogeintrag")
        return 1
    if e.get("betrieb") == "lokal":
        print(f"[netz] FEHLER: {name} ist privat (betrieb lokal), etwa wegen seiner Lizenz; nicht freigegeben")
        return 1
    if len(e.get("hf_revision", "")) < 40:
        print(f"[netz] FEHLER: {name} hat keine feste Revision; ohne sie baut niemand nach")
        return 1
    if not grund.strip():
        print("[netz] FEHLER: --grund fehlt; eine Freigabe ohne Begruendung ist keine")
        return 1
    a = abdruck(artefakt)
    e[block] = {"abdruck": a, "freigegeben": f"{datetime.date.today().isoformat()}: {grund.strip()}"}
    katalog_schreiben(k)
    print(f"[{block}] {name} freigegeben mit Abdruck {a}")
    print("[netz] Wirksam mit der naechsten Fassung: Die Laufzeit bindet den Katalog beim Bauen ein.")
    return 0


def entziehen(name: str, block: str = "netz") -> int:
    k = katalog_lesen()
    e = eintraege(k).get(name)
    if not e or block not in e:
        print(f"[{block}] {name} ist nicht freigegeben")
        return 1
    del e[block]
    katalog_schreiben(k)
    print(f"[{block}] {name} entzogen; wirksam mit der naechsten Fassung")
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    befehl, rest = argv[0], argv[1:]
    block = "pruefnetz" if "--pruefnetz" in rest else "netz"
    rest = [r for r in rest if r != "--pruefnetz"]
    if befehl == "abdruck" and rest:
        print(abdruck(Path(rest[0])))
        return 0
    if befehl == "liste":
        return liste()
    if befehl == "pruefen":
        return pruefen()
    if befehl == "freigeben" and rest:
        grund = ""
        if "--grund" in rest:
            i = rest.index("--grund")
            grund = rest[i + 1] if i + 1 < len(rest) else ""
            rest = rest[:i] + rest[i + 2:]
        name = rest[0]
        artefakt = Path(rest[1]) if len(rest) > 1 else ARTEFAKTE / name
        return freigeben(name, artefakt, grund, block)
    if befehl == "entziehen" and rest:
        return entziehen(rest[0], block)
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
