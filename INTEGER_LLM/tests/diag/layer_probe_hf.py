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
    layer = model.model.layers[0]

    emb = model.model.embed_tokens.weight[TOKEN_ID]
    summary("S0 hidden(embed)", emb)

    norm_hidden = layer.input_layernorm(emb.unsqueeze(0))[0]
    summary("S1 norm_hidden", norm_hidden)

    q = layer.self_attn.q_proj(norm_hidden) + layer.self_attn.q_proj.bias
    k = layer.self_attn.k_proj(norm_hidden) + layer.self_attn.k_proj.bias
    v = layer.self_attn.v_proj(norm_hidden) + layer.self_attn.v_proj.bias
    summary("S2 q", q)
    summary("S2 k", k)
    summary("S2 v", v)

    attn_out, _ = layer.self_attn(hidden_states=norm_hidden)
    summary("S5 attn_out", attn_out)

    o_out = layer.self_attn.o_proj(attn_out)
    summary("S5 o_out", o_out)

    residual = emb.unsqueeze(0) + o_out
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
