#!/usr/bin/env python3
"""
Erzeugt die eingefrorene exp()-LUT fuer myl-tokenomics.

Warum eingefroren: Die LUT wurde bisher zur Laufzeit mit `f64::exp()`
gebaut. `f64::exp()` ist nicht korrekt gerundet und unterscheidet sich
zwischen glibc-Versionen, musl, macOS-libm und Windows-CRT — zwei Nodes
auf verschiedenen Plattformen haetten unterschiedliche Credit-Preise
berechnet. Dieselbe Klasse Nichtdeterminismus, gegen die Whitepaper
Kap. 6.2 auf der Inferenzseite argumentiert.

Deshalb dasselbe Muster wie in INTEGER_LLM: die Tabelle wird EINMAL
offline mit beliebig genauer Dezimalarithmetik erzeugt, als Konstante
eingefroren und im Test gegen einen Hash geprueft.

Rundungsregel: ROUND_HALF_EVEN auf ganzzahlige Q32-Einheiten, mit
60 Stellen Arbeitsgenauigkeit (weit ueber den ~53 Bits, die fuer
Q32-Werte bis exp(10) noetig waeren).

Stuetzstellen: x_i = EXP_MIN + i * (EXP_MAX - EXP_MIN) / (LUT_SIZE - 1),
als exakter Bruch gerechnet. Die alte Laufzeitfassung nutzte hier eine
Ganzzahldivision (step = 640 statt 640,3126...), wodurch die Tabelle bei
x = 9,990 endete, waehrend der Interpolator bis x = 10,0 indizierte —
ein systematischer Drift von bis zu 0,97 % am oberen Rand.

Aufruf:  python3 tools/generate_exp_lut.py > src/exp_lut_table.rs
"""

import hashlib
from decimal import Decimal, getcontext, ROUND_HALF_EVEN

getcontext().prec = 60

LUT_SIZE = 2048
EXP_MIN = -655360          # -10.0 als Q16
EXP_MAX = 655360           # +10.0 als Q16
EXP_SCALE = 1 << 16        # Q16 fuer den Exponenten
RESULT_SCALE = 1 << 32     # Q32 fuer das Ergebnis


def lut_values():
    span = Decimal(EXP_MAX - EXP_MIN)
    steps = Decimal(LUT_SIZE - 1)
    scale = Decimal(EXP_SCALE)
    out = []
    for i in range(LUT_SIZE):
        # Exakte Stuetzstelle als Bruch, erst dann dividieren.
        x = (Decimal(EXP_MIN) + Decimal(i) * span / steps) / scale
        v = (x.exp() * Decimal(RESULT_SCALE)).quantize(Decimal(1), rounding=ROUND_HALF_EVEN)
        out.append(int(v))
    return out


def main():
    vals = lut_values()
    digest = hashlib.sha256(b"".join(v.to_bytes(8, "little", signed=True) for v in vals)).hexdigest()

    print("//! Eingefrorene exp()-Stuetzstellen fuer die Credit-Preisformel.")
    print("//!")
    print("//! **Generiert** — nicht von Hand bearbeiten. Neu erzeugen mit:")
    print("//! `python3 tools/generate_exp_lut.py > src/exp_lut_table.rs`")
    print("//!")
    print("//! Die Tabelle ist eingefroren, weil eine zur Laufzeit mit")
    print("//! `f64::exp()` gebaute LUT plattformabhaengig ist (libm-Varianten")
    print("//! runden unterschiedlich) und damit den Konsens brechen wuerde.")
    print("//! Erzeugt mit 60 Stellen Dezimalgenauigkeit, ROUND_HALF_EVEN.")
    print("//!")
    print(f"//! Stuetzstellen: {LUT_SIZE} im Bereich [{EXP_MIN / EXP_SCALE:+.1f}, {EXP_MAX / EXP_SCALE:+.1f}]")
    print("//! Werte: exp(x) als i64 in Q32 (32 Nachkommabits).")
    print("//!")
    print("//! **Konsens-Feld:** Aenderungen nur ueber Governance (Kap. 10.3).")
    print()
    print("/// SHA-256 ueber die Tabelle (Little-Endian i64, konkateniert).")
    print("/// Im Test geprueft, damit eine versehentliche Aenderung auffaellt.")
    print(f'pub const EXP_LUT_SHA256: &str =\n    "{digest}";')
    print()
    print(f"/// Eingefrorene exp()-Stuetzstellen (Q32).")
    print(f"pub static EXP_LUT: [i64; {LUT_SIZE}] = [")
    for i in range(0, LUT_SIZE, 4):
        row = ", ".join(f"{v}" for v in vals[i:i + 4])
        print(f"    {row},")
    print("];")


if __name__ == "__main__":
    main()
