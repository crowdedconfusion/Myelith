#!/usr/bin/env python3
"""
Unit-Tests fuer calibrate/src/scales.py::choose_pow2_shift.

Regressionstest fuer den Numerik-Fix nach Fahrplan-Punkt 12.10: die Funktion
berechnete zuvor den Skalierungs-Shift nur fuer den Fall absmax > max_int und
lieferte sonst unbedingt shift=0 - fuer reale Aktivierungs-/Gewichtsgroessen
(absmax typischerweise deutlich unter 1) bedeutete das praktisch durchgaengige
Quantisierung auf 0.

Kein pytest-Bedarf, eigenstaendiges Skript nach Projektkonvention (siehe
test_fixed_point.py). calibrate/src/quantize.py::quantize_symmetric_int8
verwendet dieselbe Formel, ist aber wegen der torch-Abhaengigkeit hier nicht
direkt testbar; die Formel ist ident zu choose_pow2_shift und wird darueber
mitabgedeckt.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.scales import choose_pow2_shift, compute_scales_from_stats, MAX_FRAC_BITS


def test_small_absmax_gets_positive_shift():
    # Regressionsfall des Bugs: absmax=0.02 (typisches LLM-Gewicht) durfte
    # nicht mehr shift=0 liefern.
    shift = choose_pow2_shift(0.02)
    assert shift > 0, f"absmax=0.02 muss positiven shift bekommen, war {shift}"
    # quantized(absmax) = round(absmax * 2^shift) muss einen erheblichen
    # Teil des int8-Bereichs nutzen, nicht auf 0/1 kollabieren.
    quantized_absmax = round(0.02 * (2 ** shift))
    assert quantized_absmax >= 64, f"quantized(absmax) zu klein: {quantized_absmax}"


def test_shift_respects_int8_range():
    # int8-Wertebereich (Gewichtsquantisierung bzw. allgemeine Semantik
    # mit explizitem max_int).
    for absmax in [0.001, 0.02, 0.5, 1.0, 5.2, 50.0]:
        shift = choose_pow2_shift(absmax, max_int=127)
        quantized_absmax = absmax * (2 ** shift)
        assert quantized_absmax <= 127.0 + 1e-6, (
            f"absmax={absmax}, shift={shift}: quantized(absmax)={quantized_absmax} > 127"
        )


def test_shift_respects_int16_range():
    # Seit v0.12.20 sind Aktivierungsskalen int16 (Default max_int=32767):
    # auch grosse Aktivierungen (gemessen bis ~±1640) muessen passen.
    for absmax in [0.001, 0.02, 0.5, 1.0, 46.5, 336.0, 1640.0]:
        shift = choose_pow2_shift(absmax)
        quantized_absmax = absmax * (2 ** shift)
        assert quantized_absmax <= 32767.0 + 1e-6, (
            f"absmax={absmax}, shift={shift}: quantized(absmax)={quantized_absmax} > 32767"
        )
    # Spot-Check des Realitaetsabgleichs: h=1640 -> shift 4 (32767/1640).
    assert choose_pow2_shift(1640.0) == 4


def test_large_absmax_falls_back_to_shift_zero():
    # absmax > max_int: Werte muessen beim Quantisieren/Clamping saettigen,
    # nicht ueberlaufen. shift=0 ist hier korrekt (kein Ueberlauf-Handling
    # jenseits von Clamping vorgesehen).
    assert choose_pow2_shift(200.0, max_int=127) == 0
    assert choose_pow2_shift(127.0, max_int=127) == 0
    assert choose_pow2_shift(40000.0) == 0  # auch im int16-Bereich


def test_shift_capped_at_max_frac_bits():
    # Extrem kleines absmax darf frac_bits nicht unbegrenzt wachsen lassen
    # (Ueberlaufreserve des i32-Akkumulators, siehe MAX_FRAC_BITS-Kommentar).
    shift = choose_pow2_shift(1e-8)
    assert shift <= MAX_FRAC_BITS


def test_degenerate_zero_absmax():
    assert choose_pow2_shift(0.0) == 0
    assert choose_pow2_shift(1e-10) == 0


def test_scale_field_is_inverse_power_of_two_of_shift():
    # scale muss 2^-shift sein (Laufzeit-Dequantisierungskonvention,
    # real ≈ quantized >> shift), nicht 2^shift.
    stats = {"layer.q_proj": {"absmax": 0.02}}
    scales = compute_scales_from_stats(stats)
    entry = scales["layer.q_proj"]
    assert entry["shift"] > 0
    assert math.isclose(entry["scale"], 2.0 ** (-entry["shift"]))


def test_monotonic_in_absmax():
    # Kleineres absmax darf niemals einen kleineren shift ergeben als ein
    # groesseres absmax (mehr Kopfraum -> mindestens so viel Praezision).
    shifts = [choose_pow2_shift(a) for a in [0.001, 0.01, 0.1, 1.0, 10.0, 100.0]]
    assert shifts == sorted(shifts, reverse=True)


def test_scales_always_pow2_consistent_batch():
    # Fahrplan 12.16 (Akzeptanzkriterium „ausschließlich Zweierpotenzen"):
    # ueber einen weiten Bereich realistischer absmax-Werte muss jeder
    # Eintrag shift in [0, MAX_FRAC_BITS] tragen und scale == 2^-shift sein.
    # Deterministischer Pseudozufall (LCG), keine numpy-Abhaengigkeit.
    state = 0x243F6A88
    stats = {}
    for i in range(200):
        state = (state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        absmax = 2.0 ** ((state % 4000) / 100.0 - 20.0)  # 2^-20 .. 2^20
        stats[f"layer{i}.q_proj"] = {"absmax": absmax}
    scales = compute_scales_from_stats(stats)
    assert len(scales) == 200
    for name, entry in scales.items():
        assert 0 <= entry["shift"] <= MAX_FRAC_BITS, name
        assert math.isclose(entry["scale"], 2.0 ** (-entry["shift"])), name
        absmax = stats[name]["absmax"]
        if absmax <= 32767.0:
            # Nicht-Saettigungsregime: quantisiertes absmax muss in den
            # int16-Bereich passen (Aktivierungsskalen seit v0.12.20).
            assert absmax * (2 ** entry["shift"]) <= 32767.0 + 1e-6, name
        else:
            # Saettigungsregime: shift=0, Werte clampen beim Quantisieren.
            assert entry["shift"] == 0, name


if __name__ == "__main__":
    test_small_absmax_gets_positive_shift()
    print("[test] Regression: kleines absmax bekommt positiven Shift: PASSED")
    test_shift_respects_int8_range()
    print("[test] Shift respektiert int8-Bereich: PASSED")
    test_shift_respects_int16_range()
    print("[test] Shift respektiert int16-Bereich (Aktivierungen): PASSED")
    test_large_absmax_falls_back_to_shift_zero()
    print("[test] Grosses absmax faellt auf shift=0 zurueck: PASSED")
    test_shift_capped_at_max_frac_bits()
    print("[test] Shift ist bei MAX_FRAC_BITS gedeckelt: PASSED")
    test_degenerate_zero_absmax()
    print("[test] Degeneriertes absmax=0: PASSED")
    test_scale_field_is_inverse_power_of_two_of_shift()
    print("[test] scale ist 2^-shift: PASSED")
    test_monotonic_in_absmax()
    print("[test] Monotonie in absmax: PASSED")
    test_scales_always_pow2_consistent_batch()
    print("[test] Batch: 200 Skalen durchgaengig Zweierpotenz-konsistent: PASSED")
    print("Alle Tests bestanden.")
