#!/usr/bin/env python3
"""
Divisionssemantik-Audit (Fahrplan-Punkt 12.24).

Eigenständiges Skript nach Projektkonvention (kein pytest). Division
durch Zweierpotenzen ist im Myelith-Ganzzahlmodell ein arithmetischer
Rechtsshift mit Round-to-nearest-even (spec.json: shift_semantics =
arithmetic_right_shift, rounding.default = round_to_nearest_even).

Begründung: Abrundende Division, Trunkation zur Null und arithmetischer
Rechtsshift liefern für negative Operanden unterschiedliche Ergebnisse.
Ohne fixierten Testvektor zeigt sich die Abweichung erst in
Modellausgaben, wo sie kaum zuzuordnen ist. Dieser Test fixiert die
Semantik als maßgebliche Referenz.

Zwei Stufen:
  1. Der maßgebliche Rust-Test (kernels::fixed_point::
     division_semantics_vector) wird per cargo ausgeführt und muss
     bestehen — er ist die Autorität für CI.
  2. Ein unabhängiger Python-Port (faithful) der Round-to-nearest-even-
     Arithmetik-Shift-Semantik wird gegen denselben fixierten Vektor
     geprüft — Kreuzvalidierung gegen die Rust-Implementierung.

Akzeptanzkriterium: Divisionsvektor besteht auf dem Reference-Backend.
"""

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
KERNELS = REPO / "kernels"

# Fixierter Divisionssemantik-Vektor — identisch zu
# kernels/src/fixed_point.rs::division_semantics_vector.
# (Wert, Shift, erwartetes Ergebnis)
DIVISION_VECTOR = [
    # positive Werte
    (0, 3, 0),
    (1, 1, 0),
    (2, 1, 1),
    (3, 1, 2),
    (4, 1, 2),
    (5, 1, 2),
    (6, 1, 3),
    (7, 1, 4),
    (8, 3, 1),
    (12, 3, 2),
    (1_000_000, 8, 3906),
    # negative Werte
    (-1, 1, 0),
    (-2, 1, -1),
    (-3, 1, -2),
    (-4, 1, -2),
    (-5, 1, -2),
    (-6, 1, -3),
    (-7, 1, -4),
    (-8, 3, -1),
    (-12, 3, -2),
    (-1_000_000, 8, -3906),
]


def rshift_round(value: int, shift: int) -> int:
    """Faithful Python-Port: arithmetischer Rechtsshift mit
    Round-to-nearest-even (identisch zu kernels::fixed_point::rshift_round)."""
    if shift == 0:
        return value
    quotient = value >> shift  # arithmetischer Shift (Floor Richtung -inf)
    mask = (1 << shift) - 1
    half = 1 << (shift - 1)
    remainder = value & mask
    if remainder > half or (remainder == half and (quotient & 1) != 0):
        return quotient + 1
    return quotient


def run_rust_vector() -> bool:
    """Führt den maßgeblichen Rust-Test per cargo aus."""
    result = subprocess.run(
        ["cargo", "test", "-p", "integer-llm-kernels", "--lib", "division_semantics_vector"],
        cwd=str(KERNELS),
        capture_output=True,
        text=True,
        timeout=600,
    )
    if result.returncode != 0:
        print("[division] Rust-Test division_semantics_vector FEHLGESCHLAGEN:")
        print(result.stdout[-2000:])
        print(result.stderr[-2000:])
        return False
    if "test result: ok" not in result.stdout:
        print("[division] Rust-Test lieferte kein 'test result: ok'")
        print(result.stdout[-2000:])
        return False
    return True


def main():
    print("[division] Divisionssemantik-Audit")

    # Stufe 1: maßgeblicher Rust-Test.
    if not run_rust_vector():
        print("[division] FEHLGESCHLAGEN (Rust-Vektor)")
        sys.exit(1)
    print("[division] Rust-Vektor division_semantics_vector: bestanden")

    # Stufe 2: Python-Port gegen denselben fixierten Vektor.
    failures = 0
    for value, shift, expected in DIVISION_VECTOR:
        got = rshift_round(value, shift)
        if got != expected:
            print(f"[division] ABWEICHUNG: ({value} >> {shift}) = {got}, erwartet {expected}")
            failures += 1
    if failures:
        print(f"[division] FEHLGESCHLAGEN: {failures} Abweichungen im Python-Port")
        sys.exit(1)

    print(f"[division] Python-Port (Kreuzvalidierung): {len(DIVISION_VECTOR)} Vektoren bestanden")
    print("[division] PASSED: Divisionssemantik fixiert (arithmetischer Rechtsshift, round-to-nearest-even)")
    sys.exit(0)


if __name__ == "__main__":
    main()
