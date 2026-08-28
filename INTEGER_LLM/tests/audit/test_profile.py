#!/usr/bin/env python3
"""Audit: Jedes Crate baut Tests optimiert UND mit allen Pruefungen.

⚑ **Warum es diese Pruefung gibt (2026-08-28).** Das Testprofil steht
in 17 einzelnen `Cargo.toml`, weil jedes Crate hier ein eigenes
Cargo-Projekt ist. Siebzehn Kopien einer Einstellung driften
auseinander, und zwar leise: Ein neues Crate ohne Profil laeuft
unoptimiert und faellt nur dadurch auf, dass die CI langsamer wird.
Niemand sucht dann nach einer fehlenden Zeile.

**Und die gefaehrlichere Drift geht in die andere Richtung.** Wer die
CI beschleunigen will, greift zu `--release` oder schreibt
`debug-assertions = false`. Beides schaltet die **Ueberlaufpruefung**
ab. Im Ganzzahlpfad dieses Projekts ist das die Pruefung, die am
meisten traegt: Die dokumentierten Vorbedingungen stehen als
`debug_assert!` (Fund 75), und ein stillschweigend umlaufender `i32`
im Konsenspfad ist genau die Sorte Fehler, die jeder Knoten anders
rechnet und kein Test sieht.

Dieselbe Bauart wie die Vollstaendigkeitspruefung in
`test_no_float.py`: Sie faengt nicht einen Fehler im Code, sondern das
Wegfallen einer Pruefung.

Belegt zur Laufzeit in `SHARED_TYPES/myl-types/tests/profil.rs`.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]

# Was in jedem Crate stehen muss.
PFLICHT = {
    "opt-level": "2",
    "debug-assertions": "true",
    "overflow-checks": "true",
}


def crates() -> list[Path]:
    """Alle Crate-Manifeste, also `<KOMPONENTE>/<crate>/Cargo.toml`."""
    gefunden = [
        p
        for p in sorted(REPO.glob("*/*/Cargo.toml"))
        if "target-shared" not in p.parts
    ]
    return gefunden


def profil_test(text: str) -> dict[str, str] | None:
    """Liest den `[profile.test]`-Abschnitt, oder None."""
    m = re.search(r"^\[profile\.test\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    if not m:
        return None
    werte = {}
    for zeile in m.group(1).splitlines():
        zeile = zeile.split("#", 1)[0].strip()
        if "=" in zeile:
            k, v = zeile.split("=", 1)
            werte[k.strip()] = v.strip().strip('"')
    return werte


def main() -> int:
    print("[profil] Testprofil-Audit (optimiert, aber vollstaendig geprueft)")
    manifeste = crates()
    if not manifeste:
        print("[profil] FEHLGESCHLAGEN: kein einziges Crate gefunden")
        return 1
    print(f"[profil] {len(manifeste)} Crates")

    fehler = 0
    for p in manifeste:
        rel = p.relative_to(REPO)
        werte = profil_test(p.read_text(encoding="utf-8"))
        if werte is None:
            print(f"[profil] FEHLT: {rel} hat keinen [profile.test]-Abschnitt")
            fehler += 1
            continue
        for schluessel, soll in PFLICHT.items():
            ist = werte.get(schluessel)
            if ist != soll:
                print(
                    f"[profil] ABWEICHUNG: {rel} [profile.test] "
                    f"{schluessel} = {ist!r}, erwartet {soll!r}"
                )
                fehler += 1

    if fehler:
        print(f"[profil] FEHLGESCHLAGEN: {fehler} Abweichung(en)")
        return 1
    print("[profil] PASSED: alle Crates bauen Tests optimiert und voll geprueft")
    return 0


if __name__ == "__main__":
    sys.exit(main())
