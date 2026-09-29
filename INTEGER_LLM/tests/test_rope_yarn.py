#!/usr/bin/env python3
"""
Proben fuer die RoPE-Skalierung nach YaRN in `calibrate/src/luts.py`.

⚑ **Zwei Schichten, wie bei der Chatvorlage.** Festgehaltene Sollwerte,
die ueberall laufen (auch in der CI ohne torch), und ein lebender
Vergleich mit der Referenzimplementierung, wo sie installiert ist. Eine
Abschrift allein kann altern, ein Orakel allein fehlt in der CI.

Eigenstaendiges Skript nach Projektkonvention, kein pytest.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.luts import generate_rope_luts, rope_masse, yarn_frequenzen  # noqa: E402
from src.model_configs import get_model_config  # noqa: E402

TERNAER = "myelith-8b-ternaer"

# Sollwerte der Referenz fuer Basis 1e6, Drehbreite 128, Faktor 4 ueber
# 16 384 Positionen (transformers 5.15, gerechnet am 2026-09-28). Die
# Referenz rechnet in float32, diese Umsetzung in float64; daher die
# Toleranz von 1e-6 relativ.
SOLL_FREQUENZEN = {
    0: 1.0,
    1: 0.8058422207832336,
    20: 0.01333521492779255,
    30: 0.0008605471812188625,
    40: 4.4456985051510856e-05,
    50: 5.133812464919174e-06,
    63: 3.102344408034696e-07,
}
SOLL_AMPLITUDE = 1.138629436111989


def test_ohne_skalierung_bleibt_alles_wie_es_war():
    m = rope_masse(get_model_config("myelith-8b"))
    assert m["frequenzen"] is None and m["amplitude"] == 1.0, m
    sin, cos = generate_rope_luts(max_seq_len=8, head_dim=m["drehbreite"],
                                  rope_theta=m["rope_theta"], frac_bits=m["frac_bits"])
    half = m["paare"]
    for p, j in ((0, 0), (5, 3), (7, 63)):
        winkel = p / (m["rope_theta"] ** (j / half))
        assert cos[p * half + j] == int(round(math.cos(winkel) * 256)), (p, j)
        assert sin[p * half + j] == int(round(math.sin(winkel) * 256)), (p, j)


def test_das_ternaere_8b_dreht_mit_yarn():
    m = rope_masse(get_model_config(TERNAER))
    assert m["frequenzen"] is not None, "das ternaere 8B dreht ohne YaRN"
    assert abs(m["amplitude"] - SOLL_AMPLITUDE) < 1e-12, m["amplitude"]
    for j, soll in SOLL_FREQUENZEN.items():
        ist = m["frequenzen"][j]
        assert abs(ist - soll) / soll < 1e-6, (j, ist, soll)
    assert m["zeilen"] == 65536


def test_die_tabelle_traegt_frequenz_und_amplitude():
    m = rope_masse(get_model_config(TERNAER))
    sin, cos = generate_rope_luts(max_seq_len=1001, head_dim=m["drehbreite"],
                                  rope_theta=m["rope_theta"], frac_bits=m["frac_bits"],
                                  frequenzen=m["frequenzen"], amplitude=m["amplitude"])
    half = m["paare"]
    # Position 0: cos ist die Amplitude, sin ist null.
    assert cos[0] == int(round(SOLL_AMPLITUDE * 256)) == 291, cos[0]
    assert sin[0] == 0
    for p, j in ((1000, 40), (1000, 0), (777, 25)):
        winkel = p * m["frequenzen"][j]
        assert cos[p * half + j] == int(round(math.cos(winkel) * m["amplitude"] * 256))
        assert sin[p * half + j] == int(round(math.sin(winkel) * m["amplitude"] * 256))
    assert max(abs(v) for v in cos + sin) < 2 ** 15, "Tabellenwert passt nicht in i16"


def test_eine_unbekannte_skalierung_faellt_laut_aus():
    eintrag = dict(get_model_config(TERNAER))
    eintrag["rope_art"] = "longrope"
    try:
        rope_masse(eintrag)
    except ValueError:
        return
    raise AssertionError("eine unbekannte Skalierung fiel still auf ungeskaliert zurueck")


def test_gegen_die_referenz():
    """Lebender Vergleich; ohne transformers ausdruecklich uebersprungen."""
    try:
        from transformers import AutoConfig
        from transformers.modeling_rope_utils import ROPE_INIT_FUNCTIONS
    except ImportError:
        print("[test] Vergleich mit transformers: UEBERSPRUNGEN (nicht installiert)")
        return False
    ordner = Path(__file__).resolve().parents[2] / "MODELS" / "llm" / "Ternary-Bonsai-8B-unpacked"
    if not (ordner / "config.json").is_file():
        print(f"[test] Vergleich mit transformers: UEBERSPRUNGEN ({ordner} fehlt)")
        return False
    ref, amp = ROPE_INIT_FUNCTIONS["yarn"](AutoConfig.from_pretrained(ordner), "cpu")
    eigen, eigen_amp = yarn_frequenzen(128, 1e6, 4.0, 16384)
    assert abs(amp - eigen_amp) < 1e-12, (amp, eigen_amp)
    for j, (r, e) in enumerate(zip([float(x) for x in ref], eigen)):
        assert abs(r - e) / r < 2e-7, (j, r, e)
    return True


if __name__ == "__main__":
    test_ohne_skalierung_bleibt_alles_wie_es_war()
    print("[test] ohne Skalierung bytegleich zur bisherigen Tabelle: PASSED")
    test_das_ternaere_8b_dreht_mit_yarn()
    print("[test] ternaeres 8B: YaRN-Frequenzen und Amplitude: PASSED")
    test_die_tabelle_traegt_frequenz_und_amplitude()
    print("[test] Tabelle traegt Frequenz und Amplitude, passt in i16: PASSED")
    test_eine_unbekannte_skalierung_faellt_laut_aus()
    print("[test] unbekannte Skalierung faellt laut aus: PASSED")
    if test_gegen_die_referenz():
        print("[test] alle 64 Frequenzen gegen transformers: PASSED")
    print("[test] Alle Tests bestanden.")
