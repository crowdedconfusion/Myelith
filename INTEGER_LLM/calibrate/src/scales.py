"""
Berechnet Zweierpotenz-Skalen (Shifts) aus den gesammelten Statistiken.
"""

import math

# Siehe calibrate/src/quantize.py::MAX_FRAC_BITS fuer die Begruendung.
MAX_FRAC_BITS = 20


def choose_pow2_shift(absmax: float, max_int: int = 127) -> int:
    """
    Bestimmt frac_bits (Laufzeit-Konvention, arithmetischer Rechtsshift bei
    der Dequantisierung: real ≈ quantized >> shift), sodass
    absmax * 2^shift in [-max_int, max_int] passt.

    Vorherige Fassung (Rechtsshift bereits bei der Quantisierung gedacht)
    lieferte fuer absmax < max_int - der Regelfall bei realen
    Aktivierungen - immer shift=0 und verschenkte damit fast die gesamte
    int8-Aufloesung. floor() statt ceil() ist hier zwingend: ceil() koennte
    absmax * 2^shift > max_int ergeben und den Wertebereich verletzen.
    """
    if absmax <= 1e-9:
        return 0
    shift = math.floor(math.log2(max_int / absmax))
    return max(0, min(shift, MAX_FRAC_BITS))


def compute_scales_from_stats(stats: dict, max_int: int = 127) -> dict:
    scales = {}
    for name, s in stats.items():
        shift = choose_pow2_shift(s["absmax"], max_int)
        scales[name] = {
            "shift": shift,
            "scale": 2.0 ** (-shift),
            "absmax_observed": s["absmax"],
        }
    return scales
