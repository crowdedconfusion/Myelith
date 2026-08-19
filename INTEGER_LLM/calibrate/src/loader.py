"""
Laedt das HF-Referenzmodell (BF16/FP16) fuer die Offline-Kalibrierung.
Float ist hier erlaubt – der Output wird in Integer-Artefakte ueberfuehrt.

Das Modell wird ausschliesslich aus dem lokalen Snapshot unter models/
geladen (reproduzierbare Herkunft, siehe models/README.md), nie ueber die
HF-ID aus dem impliziten Hugging-Face-Cache.

**Warum hier kein `device_map="auto"` steht (Fund beim ersten 7B-Lauf,
2026-08-18):** accelerate verteilt damit selbstaendig und lagert aus, wenn
der Speicher knapp wird. Bei Qwen2.5-7B (15,2 GB BF16) auf einer 24-GB-
Maschine landete ein Teil der Gewichte auf dem *meta*-Geraet:

    Some parameters are on the meta device because they were offloaded
    to the disk.

Meta-Parameter tragen keine Daten. Der Kalibrierungslauf haette sie
anstandslos "quantisiert" und ein Artefakt mit Nullgewichten exportiert –
ohne Fehlermeldung, nur mit schlechteren Zahlen. Das ist genau die
Fehlerklasse, die dieses Projekt schon dreimal Monate gekostet hat
(Funde 15/16/17). Deshalb: vollstaendiges Laden in den regulaeren
Speicher, und danach eine harte Pruefung, dass kein Parameter auf meta
liegt.
"""

import torch
from pathlib import Path
from typing import Union
from transformers import AutoModelForCausalLM, AutoTokenizer


def _pruefe_vollstaendig_geladen(model) -> None:
    """
    Bricht ab, wenn Parameter auf dem meta-Geraet liegen (kein Speicher
    dahinter). Lieber hier laut scheitern als spaeter still Nullgewichte
    exportieren.
    """
    meta = [n for n, p in model.named_parameters() if p.device.type == "meta"]
    if meta:
        beispiele = ", ".join(meta[:3])
        raise RuntimeError(
            f"{len(meta)} Parameter liegen auf dem meta-Geraet (z. B. {beispiele}) — "
            "das Modell wurde nicht vollstaendig in den Speicher geladen. Ein "
            "Kalibrierungslauf wuerde dafuer Nullgewichte exportieren, ohne zu "
            "scheitern. Ursache ist in aller Regel zu wenig Arbeitsspeicher fuer "
            "die gewaehlte Variante; entweder eine kleinere Variante waehlen "
            "(INTEGER_LLM_MODEL) oder auf einer Maschine mit mehr RAM kalibrieren."
        )


def load_reference_model(model_path: Union[str, Path]):
    """
    Laedt das Modell in BF16 fuer die Kalibrierung (lokaler Snapshot).

    Ohne `device_map`: die Gewichte kommen vollstaendig in den regulaeren
    Speicher. Reicht der physische RAM nicht, laesst das Betriebssystem
    auslagern — langsam, aber korrekt und sichtbar. accelerate wuerde
    stattdessen still auf meta/Platte ausweichen (siehe Modul-Docstring).
    """
    path = Path(model_path)
    model = AutoModelForCausalLM.from_pretrained(
        str(path),
        dtype=torch.bfloat16,
    )
    model.eval()
    _pruefe_vollstaendig_geladen(model)
    tokenizer = AutoTokenizer.from_pretrained(str(path))
    return model, tokenizer
