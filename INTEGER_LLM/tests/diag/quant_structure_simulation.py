#!/usr/bin/env python3
"""
Float-Nachbildung der Quantisierungsstruktur des Integer-Pfads.

Bildet die Aktivierungs-Quantisierung des Integer-Pfads nach: jede
Zwischen-Aktivierung wird an demselben Punkt mit derselben kalibrierten
Zweierpotenz-Skala auf int16 quantisiert wie im Integer-Pfad. Die Arithmetik
bleibt aber float (Matmul in float, Nichtlinearitäten in float, keine
Shifts/LUTs).

Entscheidendes Experiment:
  -> ergibt sich ~73: die Quantisierungsstruktur (viele/grobe
    Quantisierungspunkte) ist die Ursache des Blow-ups.
  -> ergibt sich ~15: die Integer-Arithmetik (LUTs/Shifts) ist die Ursache.

Gleitkomma erlaubt (Simulation). Kein Teil des Auslieferungspfads.

Usage: python quant_structure_simulation.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import select_sequences  # noqa: E402


def load_scales():
    import json
    sc = json.loads((REPO / "artifacts" / "qwen2.5-0.5b" / "scales.json").read_text())
    return sc["entries"] if "entries" in sc else sc


def quantize_int16_shift(x, shift):
    """Quantisiert x auf int16 mit Zweierpotenz-Skala 2^-shift."""
    import torch
    scale = 2.0 ** shift
    q = torch.clamp(torch.round(x * scale), -32768, 32767)
    return q / scale


def rmsnorm_float(x, gamma, eps=1e-6):
    import torch
    xf = x.float()
    ms = (xf * xf).mean(dim=-1, keepdim=True)
    return xf * torch.rsqrt(ms + eps) * gamma.float()


def main():
    import torch
    import math

    model, tok = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    model = model.to("cpu").float()  # CPU+float32: gemischte Dtype-Matmul sicher
    device = "cpu"
    scales = load_scales()
    sequences = select_sequences(4, 128)
    print(f"[sim] {len(sequences)} Sequenzen")

    def shift_for(name):
        return scales[name]["shift"]

    base = model.model
    layers = base.layers
    num_layers = len(layers)

    def run(quantize):
        total_logp = 0.0
        total_tokens = 0
        with torch.no_grad():
            for ids in sequences:
                input_ids = torch.tensor([ids], device=device)
                seq_len = input_ids.shape[1]
                position_ids = torch.arange(seq_len, device=device).unsqueeze(0)
                position_embeddings = base.rotary_emb(
                    base.embed_tokens(input_ids), position_ids)

                hidden = base.embed_tokens(input_ids).float()
                # Reststrom-Eingangssegment quantisieren
                if quantize:
                    hidden = quantize_int16_shift(hidden, shift_for(
                        "model.layers.0.input_layernorm.input"))

                for li, layer in enumerate(layers):
                    nxt = layers[li + 1] if li + 1 < num_layers else None
                    # --- Attention-Block ---
                    norm_h = rmsnorm_float(hidden, layer.input_layernorm.weight)
                    if quantize:
                        norm_h = quantize_int16_shift(norm_h, shift_for(
                            f"model.layers.{li}.input_layernorm"))
                    q = layer.self_attn.q_proj(norm_h.to(hidden.dtype))
                    k = layer.self_attn.k_proj(norm_h.to(hidden.dtype))
                    v = layer.self_attn.v_proj(norm_h.to(hidden.dtype))
                    # Bias ist in HF bereits in den Linear-Layern (bias=True).
                    if quantize:
                        q = quantize_int16_shift(q, shift_for(f"model.layers.{li}.self_attn.q_proj"))
                        k = quantize_int16_shift(k, shift_for(f"model.layers.{li}.self_attn.k_proj"))
                        v = quantize_int16_shift(v, shift_for(f"model.layers.{li}.self_attn.v_proj"))
                    # RoPE + Attention (float)
                    q_r = q.view(1, seq_len, -1, 64).transpose(1, 2)
                    k_r = k.view(1, seq_len, -1, 64).transpose(1, 2)
                    v_r = v.view(1, seq_len, -1, 64).transpose(1, 2)
                    cos, sin = position_embeddings
                    from transformers.models.qwen2.modeling_qwen2 import apply_rotary_pos_emb
                    q_r, k_r = apply_rotary_pos_emb(q_r, k_r, cos, sin)
                    # GQA: repeat kv
                    n_rep = model.config.num_attention_heads // model.config.num_key_value_heads
                    k_r = k_r.repeat_interleave(n_rep, dim=1)
                    v_r = v_r.repeat_interleave(n_rep, dim=1)
                    attn = torch.nn.functional.scaled_dot_product_attention(
                        q_r, k_r, v_r, is_causal=True)
                    attn = attn.transpose(1, 2).reshape(1, seq_len, -1)
                    if quantize:
                        attn = quantize_int16_shift(attn, shift_for(f"model.layers.{li}.self_attn"))
                    o = layer.self_attn.o_proj(attn.to(hidden.dtype))
                    hidden = hidden + o.float()
                    if quantize and nxt is not None:
                        hidden = quantize_int16_shift(hidden, shift_for(
                            f"model.layers.{li+1}.input_layernorm.input"))
                    elif quantize:
                        hidden = quantize_int16_shift(hidden, shift_for("model.norm.input"))

                    # --- MLP-Block ---
                    norm_m = rmsnorm_float(hidden, layer.post_attention_layernorm.weight)
                    if quantize:
                        norm_m = quantize_int16_shift(norm_m, shift_for(
                            f"model.layers.{li}.post_attention_layernorm"))
                    gate = layer.mlp.gate_proj(norm_m.to(hidden.dtype))
                    up = layer.mlp.up_proj(norm_m.to(hidden.dtype))
                    if quantize:
                        gate = quantize_int16_shift(gate, shift_for(f"model.layers.{li}.mlp.gate_proj"))
                        up = quantize_int16_shift(up, shift_for(f"model.layers.{li}.mlp.up_proj"))
                    h = torch.nn.functional.silu(gate) * up
                    if quantize:
                        h = quantize_int16_shift(h, shift_for(f"model.layers.{li}.mlp.down_proj.input"))
                    down = layer.mlp.down_proj(h.to(hidden.dtype))
                    hidden = hidden + down.float()
                    if quantize and nxt is not None:
                        hidden = quantize_int16_shift(hidden, shift_for(
                            f"model.layers.{li+1}.input_layernorm.input"))
                    elif quantize:
                        hidden = quantize_int16_shift(hidden, shift_for("model.norm.input"))

                hidden = base.norm(hidden.to(base.norm.weight.dtype))
                logits = model.lm_head(hidden)
                shift_logits = logits[0, :-1, :]
                targets = input_ids[0, 1:]
                log_probs = torch.log_softmax(shift_logits.float(), dim=-1)
                tok_logp = log_probs.gather(1, targets.unsqueeze(1)).squeeze(1)
                total_logp += tok_logp.sum().item()
                total_tokens += targets.numel()
        return math.exp(-total_logp / total_tokens), total_tokens

    ppl_ref, ntok = run(quantize=False)
    print(f"[sim] Referenz (float, keine Quant.):      {ppl_ref:.2f} ({ntok} Pos.)")

    ppl_q, ntok = run(quantize=True)
    print(f"[sim] Quantisierungsstruktur nachgebildet: {ppl_q:.2f} ({ntok} Pos.)")

    print(f"\n[sim] Integer-Pfad (real): 73.15")
    print(f"[sim] -> ~73: Quantisierungsstruktur ist die Ursache.")
    print(f"[sim] -> ~15: Integer-Arithmetik (LUTs/Shifts) ist die Ursache.")


if __name__ == "__main__":
    main()
