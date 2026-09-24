"""
Generiert Lookup-Tables fuer Nichtlinearitaeten.
Float ist hier erlaubt (Offline-Phase).

Alle Parameter (Ranges, input_shift, frac_bits) kommen ausschliesslich aus
theta_v/spec.json (Abschnitt "nonlinear") — Punkt 12.17: die
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


def generate_sigmoid_lut(input_min: int, input_max: int, input_frac_bits: int,
                         output_frac_bits: int) -> List[int]:
    """
    Sigmoid-LUT: s(x) = 1/(1+exp(-x)), Wertebereich (0, 1).

    Gebraucht von der rekurrenten Zustandsschicht an **drei** Stellen:
    der Schreibstaerke `beta`, dem Tor am Attention-Ausgang und dem Tor
    des geteilten Experten.

    ⚑ **Eine eigene Tabelle und keine Ableitung aus der SiLU.** Es liegt
    nahe, `s(x) = silu(x)/x` zu nehmen, weil die SiLU-Tabelle schon da
    ist. Bei `x = 0` ist das undefiniert, und in der Umgebung numerisch
    unbrauchbar: Genau dort, wo die meisten Werte liegen, waere `s` am
    ungenauesten. **Dasselbe Argument wie bei der SiLU-Ableitung**, und
    eine Tabelle kostet dieselben paar Kilobyte.

    ⚑ **Die Randfortsetzung ist eine Konstante und keine Identitaet**,
    anders als bei SiLU und Softplus: Oberhalb der Tabelle ist `s` eins,
    unterhalb null, und beides ist bei jeder hier darstellbaren
    Aufloesung exakt. Der Lader traegt das als
    `outside_input_range: one_above_zero_below`.
    """
    in_scale = 1 << input_frac_bits
    out_scale = 1 << output_frac_bits
    lut = []
    for x in range(input_min, input_max + 1):
        xf = x / in_scale
        # ⚑ Stabil fuer beide Vorzeichen: exp(-|x|) bleibt unter eins.
        if xf >= 0.0:
            val = 1.0 / (1.0 + math.exp(-xf))
        else:
            e = math.exp(xf)
            val = e / (1.0 + e)
        lut.append(int(round(val * out_scale)))
    return lut


def generate_zerfall_exp_lut(max_input: int, input_frac_bits: int,
                             output_frac_bits: int) -> List[int]:
    """**Der grobe Teil des Zerfalls**, `exp(-d)` auf einem groben Raster.

    ⚑ **Warum nicht einfach eine `exp`-Tabelle.** Der Zerfall
    `g = exp(-d)` geht ueber die ganze Folge in ein Produkt ein und
    braucht rund 28 Bruchbits. Eine direkte Tabelle scheitert daran
    **nicht am Ausgang, sondern am Eingang**: Nahe `d = 0` ist
    `dg/dd = -1`, ein Eingangsraster von `2^-k` ergibt also einen Fehler
    von `2^-k` in `g`. Gemessen am 2026-09-21 ueber alle 30
    Zustandsebenen, mit dem Kriterium
    `|dg| * min(N, 1/(1-g)) < 2^-8` bei `N = 262144`: Ein Raster von
    `2^-8` ergibt 1,0, und **ein Raster von `2^-14` ergibt ebenfalls
    1,0**. Feiner zu rastern hilft nicht, es verschiebt die Grenze nur.

    ⚑ **Die Zerlegung nutzt, dass `exp` multiplikativ ist:**

        exp(-d) = exp(-d_grob) * exp(-d_fein),   d = d_grob + d_fein

    `d_grob` ist ein Vielfaches des Rasters und kommt aus dieser
    Tabelle; `d_fein` ist kleiner als ein Raster und kommt aus der
    Reihe `1 - x + x^2/2`.

    ⚑ **Drei Glieder reichen, und das ist beweisbar statt gemessen:**
    Bei `d_fein < 2^-8` ist das naechste Glied `d_fein^3/6 < 2^-27,6`,
    also kleiner als eine letzte Stelle bei 28 Bruchbits. Die Messung
    bestaetigt es (2,1e-6 gegen eine Grenze von 3,9e-3).

    📌 **Wo eine Tabelle an ihrer Eingangsaufloesung scheitert, hilft
    keine groessere Tabelle, sondern eine Zerlegung.**
    """
    n = max_input * (1 << input_frac_bits)
    return [int(round(math.exp(-(i / (1 << input_frac_bits)))
                      * (1 << output_frac_bits)))
            for i in range(n)]


def generate_softplus_rest_lut(max_abs_input: int, input_frac_bits: int,
                               output_frac_bits: int) -> List[int]:
    """**Der feine Teil des Softplus**, `log(1 + exp(-|x|))`.

    ⚑ **Warum nicht Softplus selbst.** Gebraucht wird er fuer den
    Zerfall der Zustandsschicht,
    `g = exp(-exp_A * softplus(a + dt_bias))`, und `exp_A` reicht beim
    Qwen3.6-35B-A3B bis 105,2. Gemessen braucht der Zerfall deshalb rund
    **30 Bruchbits**, sonst springt `g` nahe eins in groben Stufen.

    ⛔️ **Und 30 Bruchbits passen nicht zu Softplus selbst.** In `int32`
    reicht Festkomma mit 30 Bruchbits nur bis zum Wert 2; Softplus geht
    ueber den gemessenen Eingangsbereich bis 32. Die Tabelle passt
    schlicht nicht in den Typ.

    ⚑ **Die Zerlegung loest es exakt:**

        softplus(x) = max(x, 0) + log(1 + exp(-|x|))

    Der zweite Term liegt **immer** in `(0, 0.693]`, braucht also nur
    `0.693 * 2^30 = 7.4e8` und passt bequem in `int32`. Und **er ist
    genau der Teil, der die Feinheit braucht**: Fuer stark negative `x`
    ist er der ganze Softplus. Der grobe Teil `max(x, 0)` ist exakt und
    kostet keine Tabelle.

    📌 **Eine Umformung, die den feinen vom groben Teil trennt, ist
    mehr wert als ein breiterer Typ.** Nachgerechnet ueber
    `x` in `[-40, 40]`: der Abstand zur direkten Formel betraegt
    9,2e-14, also Gleitkommarauschen.

    ⚠️ **Die Tabelle laeuft ueber `|x|` und ist deshalb halb so lang.**
    Der Rest haengt nur vom Betrag ab. Oberhalb von `max_abs_input`
    ist er kleiner als eine letzte Stelle und damit null.
    """
    n = max_abs_input * (1 << input_frac_bits) if max_abs_input < 256 else max_abs_input
    tabelle = []
    for i in range(n):
        x = i / (1 << input_frac_bits)
        rest = math.log1p(math.exp(-x))
        tabelle.append(int(round(rest * (1 << output_frac_bits))))
    return tabelle

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


def rope_masse(model_config: dict, nonlinear_spec: dict = None) -> dict:
    """Die Masse der Drehtabellen, aus dem MODELL hergeleitet.

    ⛔️ **Zeilenzahl, Drehbreite und Basis sind Modellangaben, keine
    Formatkonstanten.** theta_v fuehrte bis 0.21.0 ein
    `rope.max_seq_len` mit 40 960, der Kontextgrenze der Qwen3-Reihe.
    Das Qwen3.6-35B-A3B kann **262 144**, und die Tabellen deckten damit
    ein Sechstel des Kontexts ab, den das Artefakt zusagt. Ebenso
    `rope_theta`: 1e6 fuer die ganze Qwen3-Reihe, aber **1e7** fuer das
    35B, versteckt in `rope_scaling`. Mit der falschen Basis sind alle
    Winkel falsch, die Achtsamkeit verliert ihre Positionsinformation,
    und das Modell erzeugt Kauderwelsch.

    📌 **Eine Konstante, die fuer alle bisherigen Faelle stimmte, ist
    deshalb noch keine Formatkonstante.**

    ⚑ **Warum die Herleitung hier steht und nicht bei ihrem Aufrufer.**
    Sie stand an drei Stellen: in der Ausfuhr und in zwei Proben. Als
    `max_seq_len` aus der Spezifikation verschwand, brachen die beiden
    Proben, und zwar mit einem `KeyError` beim Einlesen, also **ohne
    einen einzigen gelaufenen Test**. Eine Herleitung, die an drei Orten
    steht, laeuft auseinander, und der zweite und dritte Ort melden sich
    nicht.

    `rope_theta` bleibt als Rueckfall in der Spezifikation, weil nicht
    jeder Modelleintrag eine eigene Basis nennt.
    """
    spec = nonlinear_spec if nonlinear_spec is not None else load_nonlinear_spec()
    drehbreite = model_config.get("rotary_dim") or model_config["head_dim"]
    return {
        "zeilen": model_config["max_context"],
        "drehbreite": drehbreite,
        "paare": drehbreite // 2,
        "rope_theta": model_config.get("rope_theta") or spec["rope"]["rope_theta"],
        "frac_bits": spec["rope"]["frac_bits"],
    }


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
