#!/usr/bin/env python3
"""
Tests fuer calibrate/src/luts.py (Punkt 12.17, aktualisiert fuer
theta_v 0.5.0 / v0.12.20): LUT-Generierung ausschliesslich aus den
Parametern von theta_v/spec.json, inkl. der input_shift-Semantik der
rsqrt-LUT (Index x repraesentiert x * 2^-input_shift) und der getrennten
Ein-/Ausgangs-Fraktionierung der SiLU-LUT.

Eigenstaendiges Skript nach Projektkonvention (siehe test_fixed_point.py),
kein pytest, keine torch/numpy-Abhaengigkeit.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                      generate_rope_luts, load_nonlinear_spec,
                      generate_sigmoid_lut, generate_softplus_rest_lut,
                      rope_masse)
from src.model_configs import get_model_config


def silu_ref(x):
    return x / (1.0 + math.exp(-x))


def test_load_nonlinear_spec_structure():
    """Die **Regeln** der Spec, nicht ihre Zahlen.

    ⚑ **Hier standen bis zum 2026-08-25 getippte Werte**, und zwar die
    von theta_v 0.14.0: silu-Eingangsbereich [-1024, 1023],
    `exp_lut_frac_bits` 8. Die Spec steht seit 0.15.0 und 0.16.0 auf
    [-8192, 8191] und 14, beides aus benannten Gruenden (SiLU-Raster
    verfeinert, weil der ganze MLP-Fehler dort entstand;
    Softmax-Aufloesung angehoben, weil bei 512 Positionen das
    Schwanzgewicht auf null rundete).

    **Der Test war seitdem rot und ist niemandem aufgefallen**, weil die
    CI nur die vier Audit-Skripte startet. Dieselbe Klasse wie Fund 44.

    Und er war doppelt wertlos: Ein Test, der prueft, dass in der Spec
    steht, was in der Spec steht, ist eine Tautologie. Er faellt bei
    **jeder** richtigen Aenderung um und erzeugt Druck, sie
    zurueckzunehmen. Ein Test gegen ein Literal prueft nicht die Regel,
    sondern den Wert, und wird damit zur Bremse statt zur Sicherung.

    Geprueft wird jetzt, was gelten **muss**, damit die LUTs ueberhaupt
    funktionieren. Die Grenzen stehen in den `note`-Feldern der Spec
    selbst; sie sind hier ausgerechnet statt abgeschrieben.
    """
    nl = load_nonlinear_spec()
    for key in ("rsqrt", "silu", "softmax", "rope"):
        assert key in nl, f"spec.json-Abschnitt 'nonlinear' ohne '{key}'"

    # rsqrt: die Indexnormierung ist eine Festlegung, kein Messwert, und
    # `rmsnorm_i16` verlangt einen geraden Shift (Halb-Bit-Faktor).
    assert nl["rsqrt"]["index_normalization"] == "dynamic_even_shift"
    assert nl["rsqrt"]["input_shift"] % 2 == 0, "Halb-Bit-Faktor braucht geraden Shift"
    assert nl["rsqrt"]["input_range"][0] == 0, "rsqrt ist auf nichtnegativen Werten definiert"

    # silu: Der Eingangsbereich muss zum Raster passen, sonst deckt die
    # LUT eine andere reale Domaene ab als die Runtime annimmt.
    silu = nl["silu"]
    lo, hi = silu["input_range"]
    assert hi + 1 == -lo, f"silu-Bereich muss symmetrisch sein, ist [{lo}, {hi}]"
    assert (hi + 1) % 2 == 0
    # Der Index wird in integer_math::lut_lookup als i16 gerechnet;
    # `lut.len() as i16 - 1` laeuft ab 32768 Eintraegen ueber.
    assert hi - lo + 1 <= 32767, "silu-LUT sprengt den i16-Index"
    # Groesster Ausgabewert = silu(hi_real) ~ hi_real; er muss in i16
    # passen, sonst saettigen die LUT-Eintraege selbst.
    hi_real = hi / (2 ** silu["input_frac_bits"])
    assert hi_real * (2 ** silu["output_frac_bits"]) <= 32767, (
        f"silu-LUT-Eintraege sprengen i16: {hi_real} * 2^{silu['output_frac_bits']}"
    )

    # softmax: exp(0) ist der groesste Eintrag und muss in i16 passen.
    sm = nl["softmax"]
    assert 2 ** sm["exp_lut_frac_bits"] <= 32767, (
        f"exp(0) = 2^{sm['exp_lut_frac_bits']} sprengt die i16-LUT"
    )
    # Die Tabelle deckt den Eingangsbereich beim gewaehlten Raster ab.
    assert sm["exp_lut_range"] >= 2 ** sm["exp_input_frac_bits"], (
        "exp-LUT kuerzer als ein einziger Einheitsschritt des Eingangsrasters"
    )

    # rope: Die Paarung ist eine Protokollfestlegung (Fund 15).
    assert nl["rope"]["pairing"] == "half_split", "Fund 15: Qwen2-Schema"
    # ⛔️ **Die Zeilenzahl ist seit theta_v 0.22.0 KEINE Formatkonstante.**
    #
    # Hier stand `nl["rope"]["max_seq_len"] > 0`, und das Feld ist aus der
    # Spec entfernt worden, weil es eine Modellangabe ist: theta_v fuehrte
    # 40 960, das Qwen3.6-35B-A3B kann 262 144, und die Tabellen deckten
    # damit ein Sechstel des zugesagten Kontexts ab. Die Zeilenzahl folgt
    # seither aus `max_context` und `rotary_dim` des Modelleintrags.
    #
    # ⚑ **Geprueft wird deshalb die Abwesenheit**, nicht der Wert. Ein
    # wiedereingefuegtes `max_seq_len` waere der Rueckfall, und genau den
    # faengt diese Zeile.
    assert "max_seq_len" not in nl["rope"], (
        "max_seq_len ist eine Modellangabe und gehoert nach model_config.json"
    )
    # `rope_theta` bleibt als Rueckfall in der Spec, weil nicht jeder
    # Modelleintrag eine eigene Basis nennt; main.py zieht den Wert des
    # Modells vor.
    assert nl["rope"]["rope_theta"] > 1.0


def test_rsqrt_lut_input_shift_semantics():
    # input_shift=8 (spec 0.5.0): Index x steht fuer den Realwert x/256.
    lut = generate_rsqrt_lut(max_input=2048, input_shift=8, frac_bits=8)
    assert lut[0] == 256, "Sentinel fuer x=0 muss 1.0 (scale) sein"
    assert lut[256] == 256, "rsqrt(1.0) = 1.0"
    assert lut[1024] == 128, "rsqrt(4.0) = 0.5"
    assert lut[64] == 512, "rsqrt(0.25) = 2.0"
    assert all(v > 0 for v in lut), "rsqrt ist ueberall positiv"
    assert all(lut[i] >= lut[i + 1] for i in range(1, len(lut) - 1)), \
        "rsqrt muss monoton fallend sein"


def test_rsqrt_lut_input_shift_zero_entspricht_alter_skala():
    # input_shift=0: Index x steht fuer den Realwert x (triviale Skala).
    lut = generate_rsqrt_lut(max_input=16, input_shift=0, frac_bits=8)
    assert lut[1] == 256, "rsqrt(1) = 1.0"
    assert lut[4] == 128, "rsqrt(4) = 0.5"


def test_silu_lut_spot_values():
    # Spec 0.9.0: Indexbereich [-1024, 1023], Eingang frac 3 (Realwert idx/8),
    # Ausgang frac 6. Nullpunkt bei Index 1024 (Offset = -input_min).
    lut = generate_silu_lut(input_min=-1024, input_max=1023,
                            input_frac_bits=3, output_frac_bits=6)
    assert len(lut) == 2048
    assert lut[1024] == 0, "silu(0) = 0"
    assert lut[1024 + 8] == round(silu_ref(1.0) * 64), "silu(1) bei frac 6"
    assert lut[1024 - 16] == round(silu_ref(-2.0) * 64), "silu(-2) bei frac 6"
    assert lut[1024 + 32] == round(silu_ref(4.0) * 64), "silu(4) bei frac 6"
    # Domaeenenrand (Realwert -128 bzw. +127.875) wird abgedeckt.
    assert lut[0] == round(silu_ref(-128.0) * 64)
    assert lut[2047] == round(silu_ref(127.875) * 64)


def test_exp_lut_spot_values():
    # Spec 0.5.2: Domaene [0, 64) mit Eingang frac 4, Ausgang frac 8.
    lut = generate_exp_lut(exp_range=1024, input_frac_bits=4, output_frac_bits=8)
    assert len(lut) == 1025
    assert lut[0] == 256, "exp(0) = 1.0"
    assert lut[16] == round(math.exp(-1.0) * 256), "exp(-1) bei Eingang frac 4"
    assert lut[32] == round(math.exp(-2.0) * 256), "exp(-2)"
    # Am Domaenenrand ist exp(-64) praktisch 0.
    assert lut[1024] == 0
    assert all(lut[i] >= lut[i + 1] for i in range(len(lut) - 1)), \
        "exp(-x) muss monoton fallend sein"


def test_rope_lut_spot_values():
    # Multi-Frequenz-RoPE (theta_v 0.10.0): Index p*half + j, Winkel
    # p * theta_j mit theta_j = 1/rope_theta^(j/half). Kleine Parameter fuer
    # handrechnbare Stuetzwerte: max_seq_len=4, head_dim=4 (half=2).
    half = 2
    sin_lut, cos_lut = generate_rope_luts(max_seq_len=4, head_dim=4,
                                          rope_theta=1000000.0, frac_bits=8)
    assert len(sin_lut) == 4 * half and len(cos_lut) == 4 * half
    # Position 0: alle Winkel 0 -> cos=1.0 (256), sin=0.
    assert cos_lut[0] == 256 and cos_lut[1] == 256, "pos 0: cos = 1.0"
    assert sin_lut[0] == 0 and sin_lut[1] == 0, "pos 0: sin = 0"
    # Position 1, Paar j=0: theta_0 = 1 -> Winkel 1 rad.
    assert cos_lut[1 * half + 0] == round(math.cos(1.0) * 256)
    assert sin_lut[1 * half + 0] == round(math.sin(1.0) * 256)
    # Position 1, Paar j=1: theta_1 = 1/1e6^(1/2) = 1e-3 -> Winkel 1e-3 rad.
    assert cos_lut[1 * half + 1] == round(math.cos(1e-3) * 256)
    assert sin_lut[1 * half + 1] == round(math.sin(1e-3) * 256)
    # hoeheres Paar -> kleinere Frequenz: j=1 rotiert kaum (sin ~ 0).
    assert abs(sin_lut[1 * half + 1]) <= 1
    assert all(abs(v) <= 256 for v in sin_lut + cos_lut)


def anker_masse():
    """Die Drehtabellen-Masse des Ankers, ueber die Bibliotheksherleitung.

    ⚑ **Die Herleitung selbst steht in `luts.rope_masse`**, also an
    derselben Stelle, aus der auch die Ausfuhr sie zieht. Waere sie hier
    nachgebaut, pruefte diese Datei ihre eigene Kopie.
    """
    return rope_masse(get_model_config("myelith-0.6b"))


def test_rope_lut_full_spec_parameters():
    # Mit den echten Parametern des Ankers muss die LUT die erwartete
    # Groesse haben und wohlgeformt sein.
    m = anker_masse()
    sin_lut, cos_lut = generate_rope_luts(
        max_seq_len=m["zeilen"], head_dim=m["drehbreite"],
        rope_theta=m["rope_theta"], frac_bits=m["frac_bits"])
    half = m["paare"]
    assert len(sin_lut) == m["zeilen"] * half
    assert len(cos_lut) == m["zeilen"] * half
    # Position 0 ist die Identitaet (alle Paare cos=1.0, sin=0).
    assert all(cos_lut[j] == 256 for j in range(half))
    assert all(sin_lut[j] == 0 for j in range(half))


def test_spec_driven_generation_lengths():
    # Die in main.py verwendete spec-gesteuerte Erzeugung muss dieselben
    # Laengen liefern wie das von runtime/Loader erwartete Format.
    nl = load_nonlinear_spec()
    rsqrt = generate_rsqrt_lut(max_input=nl["rsqrt"]["input_range"][1],
                               input_shift=nl["rsqrt"]["input_shift"],
                               frac_bits=nl["rsqrt"]["output_frac_bits"])
    silu = generate_silu_lut(input_min=nl["silu"]["input_range"][0],
                             input_max=nl["silu"]["input_range"][1],
                             input_frac_bits=nl["silu"]["input_frac_bits"],
                             output_frac_bits=nl["silu"]["output_frac_bits"])
    exp = generate_exp_lut(exp_range=nl["softmax"]["exp_lut_range"],
                           input_frac_bits=nl["softmax"]["exp_input_frac_bits"],
                           output_frac_bits=nl["softmax"]["exp_lut_frac_bits"])
    m = anker_masse()
    sin, cos = generate_rope_luts(max_seq_len=m["zeilen"],
                                  head_dim=m["drehbreite"],
                                  rope_theta=m["rope_theta"],
                                  frac_bits=m["frac_bits"])
    # ⚑ **Aus der Spec gerechnet, nicht getippt** (2026-08-25). Hier
    # standen 32768 / 2048 / 1025, die Laengen von theta_v 0.14.0. Seit
    # 0.15.0 ist die silu-LUT 16384 Eintraege lang und seit 0.16.0 die
    # exp-LUT 16385. Der Test war rot und niemandem aufgefallen, weil die
    # CI ihn nicht startet.
    #
    # Eine getippte Laenge prueft nur, dass sich die Spec nicht geaendert
    # hat. Geprueft gehoert, dass der **Erzeuger der Spec folgt**: Genau
    # das faengt einen Erzeuger, der einen Eintrag zu wenig oder zu viel
    # anlegt, und genau das ist der Fehler, der die Runtime am Rand der
    # Domaene ins Leere greifen liesse.
    rope_len = m["zeilen"] * m["paare"]
    silu_len = nl["silu"]["input_range"][1] - nl["silu"]["input_range"][0] + 1
    assert len(rsqrt) == nl["rsqrt"]["input_range"][1] + 1, (
        f"rsqrt: {len(rsqrt)} Eintraege, Spec sagt "
        f"{nl['rsqrt']['input_range'][1] + 1}"
    )
    assert len(silu) == silu_len, f"silu: {len(silu)} Eintraege, Spec sagt {silu_len}"
    # exp deckt [0, exp_lut_range] **einschliesslich** ab, daher +1.
    assert len(exp) == nl["softmax"]["exp_lut_range"] + 1, (
        f"exp: {len(exp)} Eintraege, Spec sagt "
        f"{nl['softmax']['exp_lut_range'] + 1}"
    )
    assert len(sin) == rope_len and len(cos) == rope_len
    # Alle Werte muessen in int16 passen (LUT-Format der Runtime).
    for lut in (rsqrt, silu, exp, sin, cos):
        assert all(-32768 <= v <= 32767 for v in lut)


def test_sigmoid_lut_stuetzwerte():
    """Sigmoid an Stellen, die sich von Hand nachrechnen lassen."""
    frac_in, frac_out = 6, 8
    lut = generate_sigmoid_lut(-8192, 8191, frac_in, frac_out)
    mitte = 8192  # Index von x = 0
    assert lut[mitte] == 128, "sigmoid(0) ist 0,5, also 128 bei 8 Bruchbits"
    assert lut[0] == 0, "weit links ist sigmoid null"
    assert lut[-1] == 1 << frac_out, "weit rechts ist sigmoid eins"
    for i in range(1, len(lut)):
        assert lut[i] >= lut[i - 1], f"sigmoid faellt bei Index {i}"
    # ⚑ Punktsymmetrie s(-x) + s(x) = 1: eine Eigenschaft der Funktion
    #   und keine getippte Zahl, und sie prueft die ganze Tabelle.
    for x in range(-4096, 4097, 97):
        links, rechts = lut[mitte + x], lut[mitte - x]
        assert abs((links + rechts) - (1 << frac_out)) <= 1, f"s(-x)+s(x) != 1 bei x={x}"


def test_softplus_rest_lut_zerlegung():
    """⛔️ **Fund 453: hier stand ein Test auf ein Softplus, das es nie gab.**

    Importiert wurde `generate_softplus_lut`, geschrieben wurde
    `generate_softplus_rest_lut`, und beide entstanden im **selben**
    Commit (theta_v 0.22.0, 2026-09-22). Der Import scheiterte also ab
    der ersten Zeile, und zwar fuer die **ganze** Datei: Auch die
    zehn Tests, die es schon gab, liefen zwei Tage lang nicht.

    📌 **Ein falscher Name im Import ist kein fehlschlagender Test,
    sondern ein ausgefallener Testlauf.** Ein roter Test zeigt, was
    kaputt ist; ein `ImportError` zeigt nur, dass nichts geprueft wurde.

    ⚑ **Und die Namen wichen nicht zufaellig ab.** Die Bibliothek traegt
    nur den **feinen** Teil des Softplus, `log(1 + exp(-|x|))`, weil 30
    Bruchbits fuer den ganzen Softplus nicht in `int32` passen. Geprueft
    wird deshalb nicht Softplus, sondern die **Zerlegung**, auf die sich
    der Kernel stuetzt:

        softplus(x) = max(x, 0) + tabelle[|x|]

    Das ist dieselbe Bauart wie `test_silu_ist_x_mal_sigmoid`: eine
    Identitaet statt getippter Stuetzwerte.
    """
    spec = load_nonlinear_spec()["softplus_rest"]
    max_abs = spec["max_abs_input"]
    frac_in = spec["input_frac_bits"]
    frac_out = spec["output_frac_bits"]
    lut = generate_softplus_rest_lut(max_abs, frac_in, frac_out)

    # Die Tabelle laeuft ueber |x|, ist also halb so lang wie eine
    # symmetrische waere. Index i steht fuer x = i * 2^-frac_in.
    assert len(lut) == max_abs * (1 << frac_in), (
        f"Laenge {len(lut)}, erwartet {max_abs * (1 << frac_in)}"
    )
    assert lut[0] == round(math.log(2) * (1 << frac_out)), (
        "rest(0) ist log(1+exp(0)) = ln 2"
    )
    # ⚑ Der Rest faellt, waehrend Softplus selbst steigt. Wer hier eine
    #   steigende Tabelle erwartet, hat die Zerlegung nicht gelesen.
    for i in range(1, len(lut)):
        assert lut[i] <= lut[i - 1], f"der Rest steigt bei Index {i}"
    assert lut[-1] == 0, (
        "am rechten Rand ist der Rest kleiner als eine letzte Stelle"
    )
    # ⛔️ Der ganze Grund der Zerlegung: der Rest passt in int32, der
    #    volle Softplus bei 30 Bruchbits nicht.
    assert max(lut) <= (1 << 31) - 1, "der Rest passt nicht in int32"
    assert max(lut) == lut[0], "das Maximum steht nicht bei x = 0"

    stelle = 1.0 / (1 << frac_out)
    for i in range(0, len(lut), 337):
        x = i / (1 << frac_in)
        for vorzeichen in (1, -1):
            xs = vorzeichen * x
            direkt = math.log1p(math.exp(xs))
            zerlegt = max(xs, 0.0) + lut[i] * stelle
            # Die Tabelle ist auf eine Stelle gerundet, der grobe Teil
            # ist exakt; mehr als eine halbe Stelle kann also nicht
            # herauskommen. Eine ganze als Schranke deckt das Runden ab.
            assert abs(direkt - zerlegt) <= stelle, (
                f"Zerlegung bricht bei x={xs}: {zerlegt} gegen {direkt}, "
                f"Abstand {abs(direkt - zerlegt):.3e} ueber {stelle:.3e}"
            )


def test_silu_ist_x_mal_sigmoid():
    """⚑ **Die beiden Tabellen gegeneinander, ueber ihre Identitaet.**

    `silu(x) = x · σ(x)` gilt exakt, und beide Tabellen entstehen
    **unabhaengig** voneinander. Diese Probe haelt sie zusammen und faellt,
    sobald eine von beiden falsch erzeugt wird.

    📌 **Mehr wert als getippte Stuetzwerte:** Ein Tippfehler in einer
    Erwartung faellt nur an dieser einen Stelle auf, eine verletzte
    Identitaet ueberall.
    """
    frac_in, frac_out = 6, 8
    silu = generate_silu_lut(-8192, 8191, frac_in, frac_out)
    sig = generate_sigmoid_lut(-8192, 8191, frac_in, frac_out)
    mitte = 8192
    stelle = 1 / (1 << frac_out)
    for x in range(-2048, 2049, 37):
        xf = x / (1 << frac_in)
        erwartet = xf * (sig[mitte + x] / (1 << frac_out))
        ist = silu[mitte + x] / (1 << frac_out)
        # ⛔️ **Die Schranke waechst mit |x|, und das ist die eigentliche
        #    Aussage dieser Probe.** Sigma ist auf eine Stelle gerundet;
        #    wer damit multipliziert, vervielfacht diesen Fehler mit |x|.
        #    Dazu kommt die halbe Stelle der SiLU-Tabelle selbst.
        #
        #    📌 **Ein erster Entwurf nahm eine feste Schranke von zwei
        #    Stellen und fiel bei x = 6,16 mit 3,1 Stellen Abstand.** Die
        #    Zahl war kein Fehler in den Tabellen, sondern in der
        #    Erwartung: **Eine Toleranz ohne ihre Herleitung ist geraten.**
        schranke = (abs(xf) + 1.0) * stelle
        assert abs(ist - erwartet) <= schranke, (
            f"silu(x) != x*sigmoid(x) bei x={xf}: {ist} gegen {erwartet}, "
            f"Abstand {abs(ist - erwartet):.6f} ueber der Schranke {schranke:.6f}"
        )
    # ⚑ **Und daraus folgt, warum die SiLU eine eigene Tabelle hat.** Wer
    #   sie aus Sigma ableitete, traegt dessen Rundungsfehler mal |x| mit,
    #   also am rechten Rand der Domaene rund 128 Stellen. Die eigene
    #   Tabelle kostet dieselben paar Kilobyte und hat eine halbe.
    rechter_rand = 8191 / (1 << frac_in)
    assert rechter_rand * stelle > 100 * stelle, (
        "die Fehlerverstaerkung am Rand ist kleiner als gedacht; dann waere "
        "eine abgeleitete SiLU-Tabelle doch vertretbar und diese Begruendung falsch"
    )


if __name__ == "__main__":
    test_load_nonlinear_spec_structure()
    print("[test] spec.json-nonlinear-Abschnitt haelt seine eigenen Regeln: PASSED")
    test_rsqrt_lut_input_shift_semantics()
    print("[test] rsqrt-LUT input_shift-Semantik (x * 2^-8): PASSED")
    test_rsqrt_lut_input_shift_zero_entspricht_alter_skala()
    print("[test] rsqrt-LUT input_shift=0 (triviale Skala): PASSED")
    test_silu_lut_spot_values()
    print("[test] SiLU-LUT Stuetzwerte (Domäne +/-128): PASSED")
    test_exp_lut_spot_values()
    print("[test] exp-LUT Stuetzwerte: PASSED")
    test_rope_lut_spot_values()
    print("[test] RoPE-LUT Stuetzwerte (Multi-Frequenz, half-split): PASSED")
    test_rope_lut_full_spec_parameters()
    print("[test] RoPE-LUT mit spec-Parametern (Groesse/Identitaet): PASSED")
    test_spec_driven_generation_lengths()
    print("[test] spec-gesteuerte Erzeugung: Laengen und int16-Bereich: PASSED")
    test_sigmoid_lut_stuetzwerte()
    print("[test] Sigmoid-LUT Stuetzwerte und Punktsymmetrie: PASSED")
    test_softplus_rest_lut_zerlegung()
    print("[test] Softplus-Rest-LUT und die Zerlegung max(x,0)+rest(|x|): PASSED")
    test_silu_ist_x_mal_sigmoid()
    print("[test] SiLU und Sigmoid halten ihre Identitaet: PASSED")
    print("Alle Tests bestanden.")
