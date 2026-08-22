"""
Generiert Lookup-Tables fuer Nichtlinearitaeten.
Float ist hier erlaubt (Offline-Phase).

Alle Parameter (Ranges, input_shift, frac_bits) kommen ausschliesslich aus
theta_v/spec.json (Abschnitt "nonlinear") — Fahrplan-Punkt 12.17: die
spec.json ist die Single Source of Truth des numerischen Vertrags, die
Generatoren duerfen keine eigenen, davon abweichenden Konstanten tragen.
"""

import json
import math
from pathlib import Path
from typing import List

# calibrate/src/luts.py -> calibrate/src -> calibrate -> INTEGER_LLM (Repo-Wurzel)
_REPO_ROOT = Path(__file__).parent.parent.parent


def load_nonlinear_spec(spec_path: Path = None) -> dict:
    """
    Laedt den "nonlinear"-Abschnitt aus theta_v/spec.json — derselben Datei,
    die runtime/src/loader.rs zur Kompilierzeit einbettet. Schluessel wie
    rsqrt.input_shift oder softmax.exp_lut_range sind dort kanonisch.
    """
    if spec_path is None:
        spec_path = _REPO_ROOT / "theta_v" / "spec.json"
    spec = json.loads(spec_path.read_text(encoding="utf-8"))
    return spec["theta_v"]["nonlinear"]


def generate_rsqrt_lut(max_input: int, input_shift: int, frac_bits: int) -> List[int]:
    """
    rsqrt-LUT: Index x repraesentiert den Realwert x * 2^-input_shift
    (spec.json: rsqrt.input_shift), Eintrag ist round(1/sqrt(real) * 2^frac_bits).

    x = 0 ist der Sentinel (Realwert 0 -> rsqrt undefiniert): liefert 1.0
    (scale), konsistent zu integer_math::rsqrt_q(x <= 0) in den Kernels.
    """
    scale = 1 << frac_bits
    lut = []
    for x in range(max_input + 1):
        if x == 0:
            lut.append(scale)
        else:
            real = x / (1 << input_shift)
            val = 1.0 / math.sqrt(real)
            lut.append(int(round(val * scale)))
    return lut


def generate_silu_lut(input_min: int, input_max: int, input_frac_bits: int,
                      output_frac_bits: int) -> List[int]:
    """
    SiLU-LUT: Index x im Bereich [input_min, input_max] repraesentiert den
    Realwert x * 2^-input_frac_bits (spec: silu.input_frac_bits); der
    Eintrag ist round(silu(real) * 2^output_frac_bits). Ein- und Ausgangs-
    fraktionierung sind getrennt, weil der Eingangsbereich (kalibriertes
    Gate-AbsMax mit Sicherheitsabstand) und die Ausgangspraezision
    unabhaengig voneinander gewaehlt werden.
    """
    in_scale = 1 << input_frac_bits
    out_scale = 1 << output_frac_bits
    lut = []
    for x in range(input_min, input_max + 1):
        xf = x / in_scale
        val = xf * (1.0 / (1.0 + math.exp(-xf)))
        lut.append(int(round(val * out_scale)))
    return lut


def generate_silu_grad_lut(input_min: int, input_max: int, input_frac_bits: int,
                           output_frac_bits: int) -> List[int]:
    """
    Ableitung der SiLU, fuer den Rueckwaertspass (kernels/src/backward.rs).

    silu'(x) = s(x) * (1 + x * (1 - s(x)))  mit  s(x) = 1/(1+exp(-x))

    **Warum eine eigene LUT und nicht aus der Vorwaerts-LUT gerechnet.**
    Es liegt nahe, s(x) = silu(x)/x zu nehmen und die Ableitung daraus zu
    bilden. Bei x = 0 ist das undefiniert, und in der Umgebung ist es
    numerisch unbrauchbar: Genau dort, wo die meisten Aktivierungen
    liegen, waere die Ableitung am ungenauesten. Eine eigene Tabelle
    kostet dieselben paar Kilobyte wie die vorhandenen.

    Domaene und Frakturierung sind identisch zur Vorwaerts-LUT, damit der
    Index im Rueckwaertspass ohne Umrechnung derselbe ist.

    **Der Wertebereich ist groesser als der von SiLU selbst.** silu' hat
    ein Ueberschwingen von rund 1,1 bei x ~ 2,4 und faellt links auf etwa
    -0,1; wer den Ausgangsbereich wie bei SiLU waehlt, saettigt. Die
    Funktion prueft das nicht, sie dokumentiert es: Die Wahl von
    output_frac_bits gehoert in die spec, nicht hierher.
    """
    in_scale = 1 << input_frac_bits
    out_scale = 1 << output_frac_bits
    lut = []
    for x in range(input_min, input_max + 1):
        xf = x / in_scale
        s = 1.0 / (1.0 + math.exp(-xf))
        val = s * (1.0 + xf * (1.0 - s))
        lut.append(int(round(val * out_scale)))
    return lut


def generate_exp_lut(exp_range: int, input_frac_bits: int, output_frac_bits: int) -> List[int]:
    """
    exp-LUT: Index i repraesentiert den Realwert i * 2^-input_frac_bits
    (spec: softmax.exp_input_frac_bits), Eintrag ist
    round(exp(-real) * 2^output_frac_bits). Eingangsbereich und
    Ausgangspraeezision sind getrennt parametrisiert (spec 0.5.2): die
    Domaene [0, exp_range * 2^-input_frac_bits) muss die realen
    Attention-Score-Differenzen abdecken (gemessen bis ~28), waehrend die
    Ausgangsskala die Wahrscheinlichkeitspraeezision bestimmt.
    """
    in_scale = 1 << input_frac_bits
    out_scale = 1 << output_frac_bits
    lut = []
    for i in range(exp_range + 1):
        x = i / in_scale
        val = math.exp(-x)
        lut.append(int(round(val * out_scale)))
    return lut


def generate_rope_luts(max_seq_len: int, head_dim: int, rope_theta: float,
                       frac_bits: int):
    """
    RoPE-LUTs im Qwen2/LLaMA-Schema (theta_v 0.10.0, Fund-15-RoPE-Fix):
    Jedes Dimensions-Paar j (j in [0, head_dim/2)) hat seine EIGENE Frequenz
    theta_j = 1 / rope_theta^(j / (head_dim/2)); der Winkel an Position p ist
    p * theta_j. Die LUTs sind flach row-major mit Index p*(head_dim/2)+j
    (Laenge max_seq_len * head_dim/2). Die Paarung im Kernel ist half-split
    ((x_j, x_{j+head_dim/2})), konsistent zu HF's rotate_half.

    Die alte Fassung nutzte einen einzigen Winkel 2*pi*p/max_seq_len fuer alle
    Paare und benachbarte Paarung — beides weicht von Qwen2 ab und war die
    dominante Fehlerquelle (Fund 15).
    """
    scale = 1 << frac_bits
    half = head_dim // 2
    sin_lut = []
    cos_lut = []
    for p in range(max_seq_len):
        for j in range(half):
            theta_j = 1.0 / (rope_theta ** (j / half))
            angle = p * theta_j
            cos_lut.append(int(round(math.cos(angle) * scale)))
            sin_lut.append(int(round(math.sin(angle) * scale)))
    return sin_lut, cos_lut
