#!/usr/bin/env python3
"""
Skalen-Audit: alle Aktivierungs-/Gewichts-Skalen sind Zweierpotenzen
(Fahrplan-Punkt 12.23).

Eigenständiges Skript nach Projektkonvention (kein pytest). Die
Determinismus-Garantie der Ganzzahl-Inferenz hängt daran, dass jede
Skala eine Zweierpotenz ist — nur dann ist die De-/Quantisierung ein
exakter Bit-Shift ohne Rundung. Eine Nicht-Zweierpotenz-Skala würde
stille Gleitkomma-Rundung in den Pfad bringen.

Prüfung:
  1. spec.json erklärt numeric.scales.mode == power_of_two.
  2. Jeder Eintrag in scales.json hat ein ganzzahliges `shift` und ein
     `scale`, das exakt 2^(-shift) ist (Zweierpotenz, exakt darstellbar).
  3. Jeder Eintrag hat ein `shift` im sinnvollen Bereich.

Akzeptanzkriterium: alle Skalen sind Zweierpotenzen, sonst Fehlschlag.
"""

import json
import math
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
ARTIFACTS = REPO / "artifacts" / "qwen2.5-0.5b"
SPEC = REPO / "theta_v" / "spec.json"


def is_power_of_two_scale(x: float) -> bool:
    """True, wenn x eine Zweierpotenz ist (x == 2^k für ein ganzzahliges k)."""
    if x <= 0:
        return False
    # frexp(x) = (Mantisse, Exponent); Zweierpotenz <=> Mantisse == 0.5.
    mantissa, _ = math.frexp(x)
    return mantissa == 0.5


def main():
    print("[scales] Skalen-Audit: Zweierpotenz-Prüfung")

    # Skip wenn Artefakte nicht vorhanden (z.B. in CI)
    if not (ARTIFACTS / "scales.json").exists():
        print(f"[scales] SKIP: Artefakte fehlen ({ARTIFACTS})")
        sys.exit(0)

    spec = json.loads(SPEC.read_text(encoding="utf-8"))
    mode = spec.get("theta_v", {}).get("numeric", {}).get("scales", {}).get("mode")
    if mode != "power_of_two":
        print(f"[scales] FEHLGESCHLAGEN: spec.json numeric.scales.mode = {mode!r}, erwartet 'power_of_two'")
        sys.exit(1)
    print("[scales] spec.json: numeric.scales.mode = power_of_two  OK")

    scales = json.loads((ARTIFACTS / "scales.json").read_text(encoding="utf-8"))
    if not isinstance(scales, dict) or not scales:
        print("[scales] FEHLGESCHLAGEN: scales.json leer oder unerwartetes Format")
        sys.exit(1)

    checked = 0
    violations = []
    for key, entry in scales.items():
        if not isinstance(entry, dict):
            # Einträge ohne dict-Wert haben keine Skala (z. B. Struktur-Knoten).
            continue
        if "shift" not in entry or "scale" not in entry:
            # Nur Einträge mit shift+scale sind Skalen-Einträge.
            continue
        shift = entry["shift"]
        scale = entry["scale"]
        checked += 1

        # shift muss ganzzahlig sein.
        if not isinstance(shift, int):
            violations.append(f"{key}: shift nicht ganzzahlig: {shift!r}")
            continue
        # scale muss exakt 2^(-shift) sein.
        expected = 2.0 ** (-shift)
        if scale != expected:
            violations.append(f"{key}: scale {scale!r} != 2^(-{shift}) = {expected!r}")
            continue
        # scale muss eine Zweierpotenz sein.
        if not is_power_of_two_scale(scale):
            violations.append(f"{key}: scale {scale!r} ist keine Zweierpotenz")
            continue
        # shift im sinnvollen Bereich (int8/int16-Frac-Bits).
        if not (0 <= shift <= 31):
            violations.append(f"{key}: shift {shift} außerhalb [0, 31]")
            continue

    print(f"[scales] Geprüfte Skalen-Einträge: {checked}")
    if violations:
        for v in violations:
            print(f"[scales] VERLETZUNG: {v}")
        print(f"[scales] FEHLGESCHLAGEN: {len(violations)} Skalen sind keine gültigen Zweierpotenzen")
        sys.exit(1)

    print("[scales] PASSED: alle Skalen sind Zweierpotenzen (shift ganzzahlig, scale == 2^-shift)")
    sys.exit(0)


if __name__ == "__main__":
    main()
