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


def generate_silu_lut(input_min: int, input_max: int, frac_bits: int) -> List[int]:
    scale = 1 << frac_bits
    lut = []
    for x in range(input_min, input_max + 1):
        xf = x / scale
        val = xf * (1.0 / (1.0 + math.exp(-xf)))
        lut.append(int(round(val * scale)))
    return lut


def generate_exp_lut(exp_range: int, frac_bits: int) -> List[int]:
    scale = 1 << frac_bits
    lut = []
    for i in range(exp_range + 1):
        x = i / scale
        val = math.exp(-x)
        lut.append(int(round(val * scale)))
    return lut


def generate_sin_cos_lut(n: int, frac_bits: int):
    scale = 1 << frac_bits
    sin_lut = []
    cos_lut = []
    for i in range(n):
        angle = 2.0 * math.pi * i / n
        sin_lut.append(int(round(math.sin(angle) * scale)))
        cos_lut.append(int(round(math.cos(angle) * scale)))
    return sin_lut, cos_lut
