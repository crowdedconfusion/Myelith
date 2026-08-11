#!/usr/bin/env python3
"""Layer-0-Zwischenwerte der HF-Referenz fuer Position 0 (Diagnose)."""
import sys
import torch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent / "calibrate"))
from src.loader import load_reference_model

TOKEN_ID = 22171


def summary(name, t):
    t = t.detach().float().flatten()
    absmax = t.abs().max().item()
    head = [round(v, 4) for v in t[:8].tolist()]
    print(f"{name}: absmax={absmax:.4f}, erste8={head}")


def main():
    model, _ = load_reference_model(Path("models/Qwen2.5-0.5B"))
    model.eval()
    model.to("cpu")  # Diagnose bewusst auf CPU (MPS-Quirks in RoPE)
    layer = model.model.layers[0]

    emb = model.model.embed_tokens.weight[TOKEN_ID]
    summary("S0 hidden(embed)", emb)

    norm_hidden = layer.input_layernorm(emb.unsqueeze(0))[0]
    summary("S1 norm_hidden", norm_hidden)

    # q/k/v_proj haben bias=True — der Linear-Aufruf enthält den Bias
    # bereits; nicht noch einmal addieren (Fund 12: doppelter Bias blies
    # k/q um ~2x auf und ließ die Integer-Seite fälschlich halbiert wirken).
    q = layer.self_attn.q_proj(norm_hidden)
    k = layer.self_attn.k_proj(norm_hidden)
    v = layer.self_attn.v_proj(norm_hidden)
    summary("S2 q", q)
    summary("S2 k", k)
    summary("S2 v", v)

    # transformers >= 5.x: Qwen2Attention.forward wendet o_proj BEREITS
    # intern an, bevor es zurückgibt (Fund 13). Für den Vergleich mit der
    # Integer-Probe (S5 attn_out VOR o_proj) wird der Attention-Ausgang
    # deshalb manuell rekonstruiert: an Position 0 ist der Softmax über die
    # Einzelposition exakt 1.0, also ist der Pre-o_proj-Ausgang die
    # head-major Konkatenation der (GQA-wiederholten) v-Heads.
    kv_heads = v.view(-1, 64)  # [2, 64]
    n_groups = layer.self_attn.num_key_value_groups
    heads = [kv_heads[h // n_groups] for h in range(layer.self_attn.config.num_attention_heads)]
    attn_pre = torch.cat(heads).view(1, 1, -1)
    summary("S5 attn_out(vor o_proj)", attn_pre)

    o_out = layer.self_attn.o_proj(attn_pre)
    summary("S5 o_out", o_out)

    residual = emb.view(1, 1, -1) + o_out
    summary("S5 residual", residual)

    norm_res = layer.post_attention_layernorm(residual)[0]
    summary("S6 norm_residual", norm_res)

    mlp_out = layer.mlp(norm_res)
    summary("S6 mlp_out", mlp_out)

    out = residual + mlp_out
    summary("S7 layer_out", out)

    print("gamma_in absmax:", layer.input_layernorm.weight.abs().max().item())
    print("gamma_post absmax:", layer.post_attention_layernorm.weight.abs().max().item())


if __name__ == "__main__":
    with torch.no_grad():
        main()
