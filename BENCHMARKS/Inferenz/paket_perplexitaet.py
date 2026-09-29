#!/usr/bin/env python3
"""
Gleitkomma-Perplexitaet eines gedrehten, gepackten Pakets auf WikiText-2.

⚑ **Die Referenz, gegen die ein Artefakt aus einem Paket gemessen wird.**
Ein gepacktes ternaeres Paket laesst sich nicht als BF16 entpacken, weil es
dann nicht in den Speicher passt (27B: rund 54 GB). `baseline.py` braucht
aber ein ladbares Hugging-Face-Modell. Dieses Werkzeug laedt das Paket so,
wie die Kalibrierung es tut (`gedrehtes_paket.paket_laden`: lagenweise,
verzoegert entpackt) und rechnet in Gleitkomma darauf.

Aufruf aus der Wurzel mit dem Kalibrier-venv:

    V=INTEGER_LLM/calibrate/.venv/bin/python3
    INTEGER_LLM_MODEL=<modell> $V -u BENCHMARKS/Inferenz/paket_perplexitaet.py MODELS/llm/<paket> 4 128

`INTEGER_LLM_MODEL` waehlt den Tokenizer der Folgenauswahl, genau wie bei
`artefaktvergleich.py`; nur dann sind beide Zahlen auf denselben Folgen
gemessen. Gleitkomma steht hier im Messpfad, nie im Rechenpfad.

⚠️ Langsam: beim 27B rund 50 s je Folge von 128 Token. Am Ende steht
`Fertig`.
"""

import math
import os
import sys
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
WURZEL = HIER.parents[1]
sys.path.insert(0, str(WURZEL / "INTEGER_LLM" / "calibrate"))
sys.path.insert(0, str(HIER))


def main() -> None:
    if len(sys.argv) != 4:
        print(__doc__)
        sys.exit(2)
    paket, n, laenge = Path(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
    import torch
    from src.gedrehtes_paket import paket_laden
    from wikitext_common import select_sequences

    torch.set_num_threads(os.cpu_count())
    t0 = time.time()
    modell, _tok = paket_laden(paket)
    print(f"[paket] {paket.name} geladen in {time.time() - t0:.0f} s", flush=True)
    folgen = select_sequences(n, laenge, verbose=False)
    summe, anzahl = 0.0, 0
    for i, ids in enumerate(folgen):
        t1 = time.time()
        x = torch.tensor([ids])
        with torch.no_grad():
            logits = modell(input_ids=x).logits[0].float()
        lp = torch.log_softmax(logits[:-1], -1).gather(1, x[0, 1:, None])[:, 0]
        summe += -lp.sum().item()
        anzahl += lp.numel()
        print(f"[paket] Sequenz {i}: ppl {math.exp(-lp.mean().item()):.3f} "
              f"({time.time() - t1:.0f} s)", flush=True)
    print(f"[paket] Perplexitaet {math.exp(summe / anzahl):.4f} ({anzahl} Positionen)", flush=True)
    print("Fertig")


if __name__ == "__main__":
    main()
