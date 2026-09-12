"""gammaentzerrung.py — grosse Normgewichte in die Folgematrix verschieben.

# ⛑ Fund 336 (2026-09-11): ein Gamma von 192, und int8 reicht bis 127

Qwen3-0,6B traegt in der **letzten** Ebene ein
`post_attention_layernorm.weight` mit dem Betrag **192**, waehrend der
Median derselben Zeile bei 3,1 liegt und das 99. Perzentil bei 9,7. Ein
einziger Kanal von 1024.

`quantize_symmetric_int8_per_channel` bricht daran laut ab, und das ist
richtig: Ein Normgewicht wird als int8 mit einem **nicht negativen**
Zweierpotenz-Shift gespeichert, also ist 127 der groesste darstellbare
Betrag. Fuer 192 braeuchte es einen negativen Shift, den das Format
nicht kennt.

## ⚑ Die Loesung aendert das Format nicht, sondern das Modell

RMSNorm rechnet `y[i] = normiert[i] * gamma[i]`, und `y` geht danach
**ausschliesslich** in eine oder mehrere Matrizen `W`, also
`z = W @ y`. Damit gilt fuer jede Zweierpotenz `f`:

```
gamma[i] / f  und  W[:, i] * f   ergeben dasselbe z
```

**Exakt, nicht naeherungsweise**: Beide Faktoren sind Zweierpotenzen,
und eine Multiplikation mit einer Zweierpotenz ist in Gleitkomma
verlustfrei, solange kein Exponent ueberlaeuft. Die Umformung findet
**vor** der Quantisierung statt; das Gleitkommamodell rechnet danach
Bit fuer Bit dasselbe.

⚑ **Und sie loest genau das richtige Problem.** Gemessen an
Qwen3-0,6B: Der groesste Zeilenbetrag von `gate_proj` bleibt bei 1,1562
und der von `up_proj` bei 1,2344; zwoelf Zeilen bekommen ein neues
Maximum, keine kommt auch nur in die Naehe von 127. **Die Matrizen
haben Platz, die Normgewichte nicht.**

## ⚠️ Was hier ausdruecklich nicht passiert

**Kein Glattziehen nach Gutduenken.** Verschoben wird nur, was sonst
saettigen wuerde, und nur um die kleinste Zweierpotenz, die reicht. Ein
Verfahren, das alle Kanaele „ausgleicht", waere eine zweite
Quantisierungsentscheidung neben den Skalen und gehoerte in eine eigene
Messung.

**Kein stilles Ueberspringen.** Ein Normgewicht, dessen Abnehmer diese
Datei nicht kennt, fuehrt zum Abbruch. Die Alternative waere ein
Artefakt, das an einer Stelle saettigt, von der niemand weiss.
"""

from __future__ import annotations

import math
from typing import Dict, List, Tuple

import torch

# Groesster Betrag, den ein int8-Gewicht mit nicht negativem Shift
# darstellt. Dieselbe Zahl wie in `quantize.py`; sie steht hier noch
# einmal, weil ein Import zwischen den beiden Modulen einen Kreis
# ergaebe.
INT8_GRENZE = 127.0


def _abnehmer(gammaname: str, tensornamen: set) -> List[str]:
    """**Wer den Ausgang dieser Norm liest.**

    ⚠️ **Die Liste ist die ganze Begruendung der Umformung.** Sie gilt
    nur, wenn wirklich **jeder** Abnehmer erfasst ist: Bleibt einer
    aussen vor, wird sein Eingang halbiert und nicht ausgeglichen, und
    das Modell rechnet danach etwas anderes. Deshalb ein Abbruch statt
    einer Vermutung, wenn der Name nicht passt.
    """
    if gammaname.endswith("input_layernorm.weight"):
        vorne = gammaname[: -len("input_layernorm.weight")]
        return [f"{vorne}self_attn.{p}_proj.weight" for p in ("q", "k", "v")]

    if gammaname.endswith("post_attention_layernorm.weight"):
        vorne = gammaname[: -len("post_attention_layernorm.weight")]
        dicht = [f"{vorne}mlp.{p}_proj.weight" for p in ("gate", "up")]
        if all(n in tensornamen for n in dicht):
            return dicht
        # Expertengemisch: der Router **und** jeder Experte lesen
        # denselben normierten Vektor. Wer den Router vergaesse, haette
        # ein anderes Routing.
        gemisch = [f"{vorne}mlp.gate.weight"]
        e = 0
        while f"{vorne}mlp.experts.{e}.gate_proj.weight" in tensornamen:
            gemisch += [
                f"{vorne}mlp.experts.{e}.gate_proj.weight",
                f"{vorne}mlp.experts.{e}.up_proj.weight",
            ]
            e += 1
        if e:
            return gemisch
        return []

    if gammaname == "model.norm.weight":
        # Der letzte Abnehmer ist der Kopf. Bei gebundenen Gewichten ist
        # das die Einbettung, und die wird **nicht** entzerrt: Sie ist
        # zugleich Eingang und Ausgang, eine Spaltenskalierung waere dort
        # keine Identitaet.
        return ["lm_head.weight"] if "lm_head.weight" in tensornamen else []

    return []


def entzerren(zustand: Dict[str, torch.Tensor]) -> List[Tuple[str, int, float, float]]:
    """**Verschiebt zu grosse Normgewichte in ihre Folgematrizen.**

    Arbeitet auf dem Zustandswoerterbuch **an Ort und Stelle** und gibt
    zurueck, was verschoben wurde: `(Gammaname, Kanal, vorher, nachher)`.
    Eine leere Liste heisst, dass nichts noetig war, und das ist der
    Normalfall: Von den bisher gebauten Modellen brauchte **keines** die
    Umformung.

    ⚑ **Die Rueckgabe ist kein Beiwerk.** Sie gehoert ins Protokoll des
    Laufs, denn eine Umformung, die niemand sieht, ist ein Unterschied
    zwischen Modell und Artefakt, den spaeter niemand erklaeren kann.
    """
    namen = set(zustand.keys())
    verschoben: List[Tuple[str, int, float, float]] = []

    for name in sorted(namen):
        if not name.endswith("norm.weight") and not name.endswith("layernorm.weight"):
            continue
        gamma = zustand[name]
        if gamma.dim() != 1:
            continue
        spitze = float(gamma.abs().max())
        if spitze <= INT8_GRENZE:
            continue

        ziele = _abnehmer(name, namen)
        if not ziele:
            raise ValueError(
                f"{name} traegt den Betrag {spitze:.2f} und saettigt damit int8 "
                f"(Grenze {INT8_GRENZE:.0f}), aber zu dieser Norm sind keine "
                f"Abnehmer bekannt. Ohne sie laesst sich der Faktor nicht "
                f"ausgleichen. Entweder die Abnehmer in "
                f"calibrate/src/gammaentzerrung.py eintragen oder das Modell "
                f"zurueckweisen; still saettigen darf es nicht."
            )
        fehlend = [z for z in ziele if z not in namen]
        if fehlend:
            raise ValueError(
                f"{name}: die Abnehmer {fehlend} fehlen im Zustand. Die "
                f"Umformung waere dann keine Identitaet mehr."
            )

        # Je Kanal die kleinste Zweierpotenz, die unter die Grenze
        # bringt. ⚑ **Kanalweise und nicht fuer den ganzen Tensor**: Ein
        # einzelner Ausreisser soll nicht 1023 gesunde Kanaele
        # verschieben.
        betraege = gamma.abs()
        gross = (betraege > INT8_GRENZE).nonzero(as_tuple=True)[0].tolist()
        for kanal in gross:
            alt = float(gamma[kanal])
            schritte = max(1, math.ceil(math.log2(abs(alt) / INT8_GRENZE)))
            f = float(2 ** schritte)
            gamma[kanal] = gamma[kanal] / f
            for z in ziele:
                zustand[z][:, kanal] = zustand[z][:, kanal] * f
            verschoben.append((name, kanal, alt, float(gamma[kanal])))

    return verschoben
