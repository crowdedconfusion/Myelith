#!/usr/bin/env python3
"""Erzeugt die Golden Vectors fuer den Rueckwaertspass.

## Warum in Python und nicht in Rust

Aus demselben Grund wie `tests/golden/generate.py` fuer die Op-Vektoren:
Ein Vektor, den der zu pruefende Code selbst erzeugt, prueft nichts. Die
Sollwerte entstehen hier aus einer UNABHAENGIGEN Nachbildung der
Kernelsemantik; stimmt der Rust-Kernel damit ueberein, haben zwei
getrennte Umsetzungen dasselbe gerechnet.

Das ist keine Formalitaet. Beim Bau von `linear_backward` schob die
erste Fassung jeden Summanden einzeln nach rechts und lieferte null, wo
ein Gradient hingehoerte. Eine Selbstzertifizierung haette das
eingefroren.

## Format

Identisch zu den vorhandenen Op-Vektoren (conformance/README.md):
SHA-256 ueber die little-endian gepackte Nutzlast im Feld `hash`.

Usage:
    cd INTEGER_LLM
    python tools/golden_backward.py
"""
import hashlib
import json
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ZIEL = REPO / "conformance" / "vectors" / "op"
SPEC = REPO / "theta_v" / "spec.json"


def theta_v_hash() -> str:
    return "sha256:" + hashlib.sha256(SPEC.read_bytes()).hexdigest()


def pack(daten, dtype: str) -> bytes:
    formate = {"int8": "b", "int16": "<h", "int32": "<i"}
    f = formate[dtype]
    return b"".join(struct.pack(f, int(v)) for v in daten)


def tensor(daten, dtype: str) -> dict:
    return {
        "dtype": dtype,
        "shape": [len(daten)],
        "hash": hashlib.sha256(pack(daten, dtype)).hexdigest(),
        "data": [int(v) for v in daten],
    }


def rshift_round(v: int, s: int) -> int:
    """Arithmetischer Rechtsshift, Rundung zur naechsten GERADEN Zahl.

    Nachbildung von `fixed_point::rshift_round_i64`.

    **Beim ersten Anlauf stand hier etwas anderes** (2026-08-22): Rundung
    vom Nullpunkt weg, also `(v + half) >> s` fuer positive und
    gespiegelt fuer negative Werte. Der Konformitaetslauf fand daraufhin
    eine Abweichung von genau 1 an genau einer Stelle, und die Klaerung
    ergab: Der Kernel haelt den dokumentierten Vertrag
    (round-to-nearest-even auf der Zweierkomplement-Darstellung), diese
    Nachbildung hielt ihn nicht.

    Der Vorfall ist der Grund, warum es diese Datei gibt. Haette der
    Rust-Kernel seine eigenen Sollwerte erzeugt, waere die Frage nie
    gestellt worden; so wurde sie gestellt und beantwortet, wenn auch
    zugunsten des Kernels.

    Python schiebt negative Ganzzahlen arithmetisch und maskiert sie im
    Zweierkomplement, also stimmen beide Operationen ohne Sonderfall
    ueberein.
    """
    if s == 0:
        return v
    mask = (1 << s) - 1
    half = 1 << (s - 1)
    quotient = v >> s
    rest = v & mask
    if rest > half or (rest == half and (quotient & 1) != 0):
        return quotient + 1
    return quotient


def linear_backward(g, x, w, in_features, w_shifts, g_frac, gx_frac):
    """Nachbildung von `backward::linear_backward`.

    **Ausrichtung nach oben, ein einziger Rechtsshift am Ende.** Wer je
    Summand schiebt, verliert die kleinen Beitraege; genau das war der
    Fehler in der ersten Rust-Fassung.
    """
    out_features = len(g)
    ref = max(w_shifts)
    acc = [0] * in_features
    for i in range(out_features):
        if g[i] == 0:
            continue
        align = ref - w_shifts[i]
        zeile = w[i * in_features:(i + 1) * in_features]
        for j, wij in enumerate(zeile):
            acc[j] += (g[i] * wij) << align
    gx = []
    for a in acc:
        geschoben = rshift_round(a, ref)
        # rescale(g_frac -> gx_frac)
        d = g_frac - gx_frac
        gx.append(rshift_round(geschoben, d) if d > 0 else geschoben << (-d))
    gw = [g[i] * x[j] for i in range(out_features) for j in range(in_features)]
    return gx, gw


def softmax_backward(g, p, frac):
    summe = rshift_round(sum(gi * pi for gi, pi in zip(g, p)), frac)
    return [rshift_round((gi - summe) * pi, frac) for gi, pi in zip(g, p)]


def rope_backward(g, cos, sin, frac):
    half = len(g) // 2
    out = [0] * len(g)
    for j in range(half):
        g0, g1 = g[j], g[j + half]
        out[j] = rshift_round(g0 * cos[j] + g1 * sin[j], frac)
        out[j + half] = rshift_round(g1 * cos[j] - g0 * sin[j], frac)
    return out


def schreiben(name: str, metadata: dict, inputs: dict, outputs: dict):
    gv = {
        "name": name,
        "level": "op",
        "theta_v_hash": theta_v_hash(),
        "metadata": metadata,
        "inputs": inputs,
        "outputs": outputs,
    }
    ZIEL.mkdir(parents=True, exist_ok=True)
    pfad = ZIEL / f"{name}.golden.json"
    pfad.write_text(json.dumps(gv, indent=2) + "\n", encoding="utf-8")
    print(f"  {pfad.relative_to(REPO)}")


def main() -> int:
    print("Golden Vectors fuer den Rueckwaertspass:")

    # --- linear ---------------------------------------------------------
    in_f, out_f = 8, 4
    x = [(i * 37 - 100) * 8 for i in range(in_f)]
    w = [((i * 13) % 97) - 48 for i in range(in_f * out_f)]
    shifts = [4] * out_f
    g = [3, -5, 2, 7]
    gx, gw = linear_backward(g, x, w, in_f, shifts, 6, 6)
    schreiben(
        "backward_linear",
        {"in_features": in_f, "w_shifts": shifts, "g_frac": 6, "gx_frac": 6},
        {"g": tensor(g, "int32"), "x": tensor(x, "int16"), "W": tensor(w, "int8")},
        # gW passt nicht in int32; als int32 gespeichert waere er falsch,
        # deshalb bleibt er hier klein genug, dass er hineinpasst.
        {"gx": tensor(gx, "int32"), "gW": tensor(gw, "int32")},
    )

    # --- softmax --------------------------------------------------------
    frac = 12
    p = [1 << 10, 1 << 11, 1 << 10, (1 << 12) - (1 << 10) - (1 << 11) - (1 << 10)]
    assert sum(p) == 1 << frac, "p muss sich zu 2^frac summieren"
    g = [2, -3, 5, 1]
    schreiben(
        "backward_softmax",
        {"frac_bits": frac},
        {"g": tensor(g, "int32"), "p": tensor(p, "int32")},
        {"gz": tensor(softmax_backward(g, p, frac), "int32")},
    )

    # --- rope -----------------------------------------------------------
    import math
    frac = 14
    winkel = [0.3, 0.7, 1.1, 2.5]
    cos = [round(math.cos(a) * (1 << frac)) for a in winkel]
    sin = [round(math.sin(a) * (1 << frac)) for a in winkel]
    g = [3000, -1200, 800, 2500, -900, 1700, -2200, 400]
    schreiben(
        "backward_rope",
        {"frac_bits": frac},
        {"g": tensor(g, "int32"), "cos": tensor(cos, "int16"), "sin": tensor(sin, "int16")},
        {"gx": tensor(rope_backward(g, cos, sin, frac), "int32")},
    )

    print("\nDrei Vektoren geschrieben. Pruefen mit:")
    print("    cd INTEGER_LLM/conformance && ./run.sh reference")
    return 0


if __name__ == "__main__":
    sys.exit(main())
