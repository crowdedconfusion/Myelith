#!/usr/bin/env python3
"""
Misst die realen Attention-Score-Spannweiten gegen die exp-LUT-Domaene.

Hintergrund (2026-08-19): Die exp-LUT der Softmax hat eine FESTE Domaene
[0, 64) in realen Einheiten (theta_v: exp_lut_range 1024,
exp_input_frac_bits 4). Diese Domaene wurde in v0.12.21 anhand von
Qwen2.5-0.5B festgelegt ("gemessene Score-Differenzen bis ~28") und nie
fuer eine groessere Variante nachgeprueft.

Der Verdacht: Massive Activations (Fund 20) und Attention Sinks sind
dasselbe Phaenomen (Sun et al. 2024; StreamingLLM) — die Ausreisser-
Kanaele wirken als gelernte Bias-Terme und erzeugen sehr scharfe
Attention. Scharfe Attention heisst grosse Score-Differenzen. Ueberschreiten
die 64, saettigt die LUT, und die Softmax wird an JEDEM Kopf, JEDER Ebene,
JEDER Position leicht falsch — das waere genau das breit verteilte
Rauschen, das der positionsweise Vergleich zeigt (83,7 % der Positionen
schlechter, keine Lokalisierung).

Gemessen wird, was die Integer-Runtime der exp-LUT tatsaechlich vorlegt:
`max(scores) - score` je Attention-Zeile, in realen Einheiten.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/attention_score_spread.py
"""
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, select_sequences  # noqa: E402

EXP_DOMAENE = 64.0  # theta_v: exp_lut_range / 2^exp_input_frac_bits


def main():
    import torch
    import math

    n_sequences = int(os.environ.get("E2E_SEQUENCES", "2"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(n_sequences, seq_len, verbose=False)

    model, _ = load_reference_model(MODEL_DIR)
    cfg = model.config
    head_dim = cfg.hidden_size // cfg.num_attention_heads
    skala = 1.0 / math.sqrt(head_dim)
    gruppen = cfg.num_attention_heads // cfg.num_key_value_heads

    erfasst = []

    def mach_hook(layer_idx):
        def hook(module, args, kwargs, output):
            hidden = args[0] if args else kwargs.get("hidden_states")
            if hidden is None:
                return
            # Projektionen im Modell-dtype rechnen, erst danach nach
            # float32 fuer die Statistik.
            h = hidden.detach()
            b, t, _ = h.shape
            q = module.q_proj(h).view(b, t, cfg.num_attention_heads, head_dim).transpose(1, 2).float()
            k = module.k_proj(h).view(b, t, cfg.num_key_value_heads, head_dim).transpose(1, 2).float()
            # GQA: KV-Heads auf Query-Heads verbreitern (wie repeat_kv).
            k = k.repeat_interleave(gruppen, dim=1)
            # RoPE wird hier bewusst ausgelassen: es ist eine Rotation und
            # aendert die Betragsverhaeltnisse der Scores nur geringfuegig;
            # fuer die Groessenordnung der SPANNWEITE reicht das.
            scores = (q @ k.transpose(-1, -2)) * skala
            # Kausale Maske, dann max-Differenz je Zeile (genau das, was
            # die Integer-Softmax der exp-LUT vorlegt).
            maske = torch.triu(torch.ones(t, t, dtype=torch.bool), diagonal=1)
            scores = scores.masked_fill(maske, float("-inf"))
            zeilen_max = scores.amax(dim=-1, keepdim=True)
            diff = (zeilen_max - scores)
            diff = diff[~torch.isinf(diff)]
            erfasst.append((layer_idx, diff.max().item(),
                            (diff > EXP_DOMAENE).float().mean().item()))
        return hook

    handles = []
    for i, layer in enumerate(model.model.layers):
        handles.append(layer.self_attn.register_forward_hook(
            mach_hook(i), with_kwargs=True))

    with torch.no_grad():
        for ids in sequences:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()

    # Je Ebene zusammenfassen.
    je_ebene = {}
    for idx, maxdiff, anteil in erfasst:
        vorher = je_ebene.get(idx, (0.0, 0.0))
        je_ebene[idx] = (max(vorher[0], maxdiff), max(vorher[1], anteil))

    print(f"exp-LUT-Domaene: [0, {EXP_DOMAENE})  — alles darueber saettigt")
    print()
    print(f"{'Ebene':>6} {'max(Score-Diff)':>16} {'Anteil > 64':>12}")
    ueber = 0
    for idx in sorted(je_ebene):
        maxdiff, anteil = je_ebene[idx]
        flag = "  <-- SAETTIGT" if maxdiff > EXP_DOMAENE else ""
        if maxdiff > EXP_DOMAENE:
            ueber += 1
        print(f"{idx:>6} {maxdiff:>16.2f} {anteil:>11.2%}{flag}")
    print()
    print(f"Ebenen mit Saettigung: {ueber} von {len(je_ebene)}")


if __name__ == "__main__":
    main()
