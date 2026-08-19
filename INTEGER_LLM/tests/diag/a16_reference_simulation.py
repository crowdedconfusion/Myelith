#!/usr/bin/env python3
"""
Prueft die AKTIVIERUNGS-Quantisierung als Verfahren — wie
w8_reference_simulation.py fuer die Gewichte.

Der Gewichtstest (2026-08-19) zeigte: int8-Per-Channel-Gewichte kosten bei
7B nur +0,7 % Perplexitaet. Das Schema traegt, der Fehler liegt bei uns.
Offen blieb, ob das auch fuer die AKTIVIERUNGEN gilt: Unser Pfad haelt den
Residualstrom als int16 mit einer Zweierpotenz-Skala je Kanal, und
`layer_probe` zeigt bei 7B nur ~6 von 15 genutzten Bits.

Diese Simulation quantisiert die Residualstrom-Segmente in PyTorch nach
demselben Schema und misst die Perplexitaet — in drei Varianten:

    A  Orakel-Skalen: je Kanal aus dem Maximum DIESER Sequenz
       (theoretische Obergrenze des Per-Kanal-Ansatzes)
    B  kalibrierte Skalen: je Kanal aus dem Maximum eines separaten
       Kalibrierkorpus — exakt wie unsere Pipeline es macht
    C  ohne Aktivierungsquantisierung (nur zur Kontrolle)

Ist A schon schlecht, reicht int16-per-Kanal fuer 7B grundsaetzlich nicht.
Ist A gut und B schlecht, liegt es an der Kalibrierung. Sind beide gut,
liegt der Fehler in unserer Rust-Umsetzung.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/a16_reference_simulation.py
"""
import math
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, select_sequences  # noqa: E402

INT16_MAX = 32767
MAX_FRAC_BITS = 20


def shifts_aus_absmax(absmax):
    import torch
    sicher = absmax.clamp(min=1e-9)
    return torch.floor(torch.log2(INT16_MAX / sicher)).clamp(0, MAX_FRAC_BITS)


def quantisiere(x, shifts):
    import torch
    skala = torch.pow(2.0, shifts)
    return torch.clamp(torch.round(x * skala), -INT16_MAX - 1, INT16_MAX) / skala


def perplexitaet(model, sequences):
    import torch
    s, n = 0.0, 0
    with torch.no_grad():
        for ids in sequences:
            logits = model(input_ids=torch.tensor([ids], device=model.device)).logits[0].float()
            lp = torch.log_softmax(logits[:-1], dim=-1)
            ziel = torch.tensor(ids[1:], device=lp.device)
            tl = lp.gather(1, ziel.unsqueeze(1)).squeeze(1)
            s += tl.sum().item(); n += tl.numel()
    return math.exp(-s / n), n


def main():
    import torch

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    mess = select_sequences(4, seq_len, verbose=False)
    model, tok = load_reference_model(MODEL_DIR)

    # Die Module, deren EINGANG unser Residualstrom-Segment ist — exakt
    # dieselbe Auswahl wie calibrate/src/stats.py.
    ziel_module = [m for n, m in model.named_modules()
                   if n.endswith(("input_layernorm", "post_attention_layernorm"))
                   or n == "model.norm"]
    print(f"[a16] {len(ziel_module)} Residualstrom-Segmente")

    ppl_c, n = perplexitaet(model, mess)
    print(f"[a16] C  ohne Aktivierungsquantisierung : {ppl_c:.2f} ({n} Positionen)")

    # --- A: Orakel-Skalen (pro Vorwaertspass aus den eigenen Werten) ---
    handles = []
    for m in ziel_module:
        def hook(module, inputs):
            x = inputs[0]
            sh = shifts_aus_absmax(x.detach().float().reshape(-1, x.shape[-1]).abs().amax(dim=0))
            return (quantisiere(x.float(), sh).to(x.dtype),) + inputs[1:]
        handles.append(m.register_forward_pre_hook(hook))
    ppl_a, _ = perplexitaet(model, mess)
    for h in handles: h.remove()
    print(f"[a16] A  Orakel-Skalen je Kanal        : {ppl_a:.2f}")

    # --- B: kalibrierte Skalen aus separatem Korpus (wie unsere Pipeline) ---
    kalib = select_sequences(64, seq_len, verbose=False)
    gesammelt = {}
    handles = []
    for i, m in enumerate(ziel_module):
        def sammel(module, inputs, _i=i):
            x = inputs[0].detach().float().reshape(-1, inputs[0].shape[-1]).abs().amax(dim=0).cpu()
            gesammelt[_i] = torch.maximum(gesammelt[_i], x) if _i in gesammelt else x
        handles.append(m.register_forward_pre_hook(sammel))
    with torch.no_grad():
        for ids in kalib:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles: h.remove()

    kalibrierte_shifts = {i: shifts_aus_absmax(v) for i, v in gesammelt.items()}
    handles = []
    for i, m in enumerate(ziel_module):
        def anwenden(module, inputs, _i=i):
            x = inputs[0]
            sh = kalibrierte_shifts[_i].to(x.device)
            return (quantisiere(x.float(), sh).to(x.dtype),) + inputs[1:]
        handles.append(m.register_forward_pre_hook(anwenden))
    ppl_b, _ = perplexitaet(model, mess)
    for h in handles: h.remove()
    print(f"[a16] B  kalibrierte Skalen je Kanal   : {ppl_b:.2f}")

    print()
    print(f"[a16] unser Integer-Pfad               : 41.42")
    print()
    if ppl_b < 12:
        print("[a16] -> Auch die Aktivierungsquantisierung traegt.")
        print("[a16]    Der Fehler liegt in unserer Rust-Umsetzung.")
    elif ppl_a < 12:
        print("[a16] -> Das Verfahren traegt, aber die KALIBRIERUNG nicht.")
    else:
        print("[a16] -> int16-per-Kanal reicht fuer 7B grundsaetzlich nicht.")


if __name__ == "__main__":
    main()
