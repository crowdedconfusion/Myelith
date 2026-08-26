#!/usr/bin/env python3
"""Operationsweiser Vergleich einer Ebene: wo entsteht der Fehler?

**Warum diese Messung (2026-08-20, Punkt 12.77).** Der Reihe
nach wurden ausgeschlossen: das Quantisierungsschema (+1,38 %), die
Residualstrom-Breite, der KV-Cache, das Score-Raster, die
Softmax-Ausgabe, RoPE und der LM-Head — alle ±0 %. Nur die rsqrt-LUT
schlägt mit +2,05 % durch. Unser Pfad liegt trotzdem bei +8,29 %.

Alles, was sich als **einzelner Parameter** in einem
Gleitkomma-Referenzmodell nachstellen lässt, ist damit geprüft. Der Rest
sitzt in der Arithmetik **innerhalb** einer Ebene, und Perplexität ist
dafür das falsche Instrument: Sie mittelt über 435 Positionen und 24
Ebenen und verrät nicht, welche Operation abweicht.

Hier stattdessen: dieselbe Eingabe durch beide Pfade, und je Stufe der
**relative L2-Fehler über alle Kanäle** — nicht AbsMax. AbsMax misst
genau den einen Ausreißerkanal und hat in dieser Fehlersuche schon
zweimal in die Irre geführt.

**Lesart:** Springt der Fehler an einer Stufe, ist das die gesuchte
Operation. Steigt er gleichmäßig, ist es die Summe vieler Rundungen —
dann hilft nur mehr Breite in den Zwischenwerten.

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad.

Usage:
    layer_probe <artefakt> 22171 --full > int_probe.txt
    INTEGER_LLM_MODEL=... python tests/diag/layer_stage_compare.py int_probe.txt
"""
import sys
from pathlib import Path

import numpy as np
import torch

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR  # noqa: E402

TOKEN_ID = 22171


def rel_l2(a, b):
    a = np.asarray(a, dtype=np.float64).ravel()
    b = np.asarray(b, dtype=np.float64).ravel()
    if a.shape != b.shape:
        return None
    return float(np.linalg.norm(a - b) / max(np.linalg.norm(b), 1e-12))


def main():
    integer = {}
    for zeile in open(sys.argv[1]):
        if not zeile.startswith("FULL"):
            continue
        teile = zeile.split()
        integer[teile[1]] = np.array([float(x) for x in teile[2:]])

    model, _ = load_reference_model(MODEL_DIR)
    model.eval()
    model.to("cpu")
    layer = model.model.layers[0]
    cfg = layer.self_attn.config
    head_dim = cfg.hidden_size // cfg.num_attention_heads

    with torch.no_grad():
        emb = model.model.embed_tokens.weight[TOKEN_ID]
        norm_hidden = layer.input_layernorm(emb.unsqueeze(0))[0]
        q = layer.self_attn.q_proj(norm_hidden)
        k = layer.self_attn.k_proj(norm_hidden)
        v = layer.self_attn.v_proj(norm_hidden)
        # An Position 0 ist der Softmax ueber die Einzelposition exakt 1,
        # der Pre-o_proj-Ausgang also die head-major Konkatenation der
        # (GQA-wiederholten) v-Heads.
        kv_heads = v.view(-1, head_dim)
        n_groups = layer.self_attn.num_key_value_groups
        attn_pre = torch.cat([kv_heads[h // n_groups]
                              for h in range(cfg.num_attention_heads)])
        o_out = layer.self_attn.o_proj(attn_pre.view(1, 1, -1))
        residual = emb.view(1, 1, -1) + o_out
        norm_res = layer.post_attention_layernorm(residual)[0]
        mlp_out = layer.mlp(norm_res)
        out = residual + mlp_out

    def f(t):
        return t.detach().float().cpu().numpy().ravel()

    stufen = [
        ("S0_hidden(embed)", f(emb)),
        ("S1_norm_hidden", f(norm_hidden)),
        ("S2_q_flat", f(q)),
        ("S2_k_flat", f(k)),
        ("S2_v_flat", f(v)),
        ("S3_head_out(h0)", f(kv_heads[0])),
        ("S5_attn_out(v-Skala)", f(attn_pre)),
        ("S5_attn_out(reskaliert)", f(attn_pre)),
        ("S5_o_out", f(o_out)),
        ("S5_residual(mid)", f(residual)),
        ("S6_norm_residual", f(norm_res)),
        ("S6_mlp_out", f(mlp_out)),
        ("S7_layer_out", f(out)),
    ]

    print(f"{'Stufe':<26} {'rel. L2':>9} {'Zuwachs':>9}")
    print("-" * 48)
    vorher = None
    for name, ref in stufen:
        if name not in integer:
            print(f"{name:<26} {'—':>9}   (nicht im Dump)")
            continue
        e = rel_l2(integer[name], ref)
        if e is None:
            print(f"{name:<26} {'—':>9}   (Form {integer[name].shape} vs {ref.shape})")
            continue
        zu = "" if vorher is None else f"{100*(e-vorher):+8.2f}pp"
        marke = "   <<<" if vorher is not None and (e - vorher) > 0.02 else ""
        print(f"{name:<26} {100*e:8.2f}% {zu}{marke}")
        vorher = e


if __name__ == "__main__":
    main()
