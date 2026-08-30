#!/usr/bin/env python3
"""Haelt die Modelltabelle der Streitlast-Rechnung gegen die Artefakte.

# ⚑ Warum es diese Pruefung gibt (2026-08-29)

`GOVERNANCE/myl-governance/src/bin/streitlast.rs` rechnet die
Speicherlast der Streitfrist aus. Frist und Erasure-Parameter holt es
sich aus den Konstanten der jeweiligen Crates, also **benutzt** es sie,
statt sie zu wiederholen. Bei den Modellmassen geht das nicht: Sie
stehen in `INTEGER_LLM/artifacts/<modell>/model_config.json`, und
`myl-governance` darf nicht an INTEGER_LLM haengen.

Damit steht eine Tabelle im Rust-Quelltext, die dieselbe Wahrheit ein
zweites Mal behauptet. Genau daraus entstand Fund 34. Hier ist die
Verdopplung nicht zu vermeiden, also wird sie **bewacht**: Diese
Pruefung liest beide Seiten und vergleicht sie.

Ohne sie wuerde ein neues Modell oder eine geaenderte Schichtzahl die
Rechnung still falsch machen, und eine Rechnung, die falsch geworden
ist, sieht genauso aus wie eine richtige.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
QUELLE = ROOT / "GOVERNANCE" / "myl-governance" / "src" / "bin" / "streitlast.rs"
ARTEFAKTE = ROOT / "INTEGER_LLM" / "artifacts"

# Der Name in der Rust-Tabelle zu dem Verzeichnis unter artifacts/.
ZUORDNUNG = {
    "Qwen2.5-0,5B": "qwen2.5-0.5b",
    "Qwen3-4B": "qwen3-4b",
    "Qwen2.5-7B": "qwen2.5-7b",
    "Qwen3-30B-A3B": "qwen3-30b-a3b",
}

ZEILE = re.compile(
    r'Modell\s*\{\s*name:\s*"([^"]+)",\s*hidden:\s*(\d+),\s*layer:\s*(\d+),'
)


def main():
    print("[streitlast] Gegenprobe der Modelltabelle")
    if not QUELLE.exists():
        print(f"[streitlast] FEHLER: {QUELLE.relative_to(ROOT)} fehlt")
        return 1

    tabelle = ZEILE.findall(QUELLE.read_text(encoding="utf-8"))
    if not tabelle:
        print("[streitlast] FEHLER: keine Modellzeile gefunden — hat sich die Form geaendert?")
        return 1

    fehler = 0
    for name, hidden, layer in tabelle:
        hidden, layer = int(hidden), int(layer)
        verzeichnis = ZUORDNUNG.get(name)
        if verzeichnis is None:
            print(f"[streitlast] FEHLER: {name!r} steht in keiner Zuordnung dieser Pruefung")
            fehler += 1
            continue
        pfad = ARTEFAKTE / verzeichnis / "model_config.json"
        if not pfad.exists():
            print(f"[streitlast] FEHLER: {name}: {pfad.relative_to(ROOT)} fehlt")
            fehler += 1
            continue
        c = json.loads(pfad.read_text(encoding="utf-8"))
        for feld, wert, erwartet in (
            ("hidden_size", hidden, c["hidden_size"]),
            ("num_layers", layer, c["num_layers"]),
        ):
            if wert != erwartet:
                print(
                    f"[streitlast] FEHLER: {name}: {feld} steht in der Rechnung "
                    f"als {wert}, im Artefakt als {erwartet}"
                )
                fehler += 1

    fehlend = set(ZUORDNUNG) - {n for n, _, _ in tabelle}
    if fehlend:
        print(
            "[streitlast] FEHLER: diese Modelle liegen unter artifacts/, "
            f"stehen aber nicht in der Rechnung: {', '.join(sorted(fehlend))}"
        )
        fehler += len(fehlend)

    if fehler:
        print(f"[streitlast] FEHLGESCHLAGEN: {fehler} Beanstandung(en)")
        return 1
    print(f"[streitlast] PASSED: {len(tabelle)} Modelle stimmen mit den Artefakten ueberein")
    return 0


if __name__ == "__main__":
    sys.exit(main())
