"""
**Welche Bauarten ein Artefakt werden koennen, und wie.**

Seit 2026-10-19, fuer das Bauskript (`tools/modell_bauen.py`). Je Bauart:

- woran sie im `config.json` zu erkennen ist (`model_type` des Textteils);
- der **Erzeuger**: Er macht aus dem `config.json` den Exporteintrag, in
  derselben Form wie die von Hand gepflegten Eintraege in `model_configs.py`;
- der Bauweg: der Korpusdurchlauf (`main.py`, das ganze Modell im
  Speicher) oder ebenenweise (`ebenenweise.py`, eine Ebene nach der anderen);
- die Python-Umgebung, die die Bauart kennt;
- ob die Experten danach in vier Bit gehen (`build_vierbit.sh`).

⚑ **Ein Erzeuger ist nur dann eine Zusage, wenn er an einem echten Modell
geprueft ist.** `tests/test_bauarten.py` haelt jeden gegen die von Hand
verifizierten Eintraege: Aus dem `config.json` von Qwen3-4B muss genau der
Eintrag `myelith-4b` entstehen, aus dem von Qwen3.6-35B-A3B genau
`myelith-35b-a3b`, und so weiter. Eine Bauart ohne solchen Beleg steht hier als
**erkannt, nicht freigegeben**, mit dem Grund.

⚠️ **Erkannt ist noch nicht gebaut.** Das Bauskript haelt den erzeugten
Eintrag zusaetzlich Feld fuer Feld gegen die Quelle (`konfigabgleich`) und
seine Merkmale gegen das, was die Laufzeit rechnet. Erst dann gibt es frei.
"""

from dataclasses import dataclass
from typing import Callable, Optional


@dataclass(frozen=True)
class Bauart:
    name: str
    model_types: tuple
    familie: str
    bauweg: str            # "korpus" oder "ebenenweise"
    umgebung: str          # ".venv" oder ".venv-flashnext" unter calibrate/
    vierbit: bool          # Experten in vier Bit moeglich
    erzeuger: Optional[Callable]
    freigegeben: bool
    grund: str = ""


def text_config(roh: dict) -> dict:
    """Der Textteil eines `config.json` (multimodale Modelle fuehren ihn
    unter `text_config`)."""
    return roh.get("text_config", roh)


def _feld(t: dict, name: str):
    if name not in t:
        raise ValueError(f"config.json: Feld `{name}` fehlt")
    return t[name]


def _gemeinsam(t: dict, familie: str, variant: str, verified: str, hf: str) -> dict:
    """Was jede Qwen3-Bauart traegt."""
    return {
        "family": familie,
        "variant": variant,
        "num_layers": _feld(t, "num_hidden_layers"),
        "hidden_size": _feld(t, "hidden_size"),
        "num_heads": _feld(t, "num_attention_heads"),
        "num_kv_heads": _feld(t, "num_key_value_heads"),
        "head_dim": _feld(t, "head_dim"),
        "vocab_size": _feld(t, "vocab_size"),
        "rms_norm_eps": _feld(t, "rms_norm_eps"),
        "hidden_act": _feld(t, "hidden_act"),
        "max_context": _feld(t, "max_position_embeddings"),
        "attention_bias": bool(_feld(t, "attention_bias")),
        "qk_norm": True,
        "verified": verified,
        "hf_model_id": hf,
    }


def erzeuge_qwen3(roh: dict, variant: str, verified: str, hf: str) -> dict:
    t = text_config(roh)
    if t.get("rope_scaling"):
        raise ValueError("Qwen3 mit RoPE-Skalierung wird von diesem Erzeuger nicht abgebildet")
    cfg = _gemeinsam(t, "qwen3", variant, verified, hf)
    cfg.update({
        "intermediate_size": _feld(t, "intermediate_size"),
        "rope_theta": float(_feld(t, "rope_theta")),
        "tie_word_embeddings": bool(roh.get("tie_word_embeddings", t.get("tie_word_embeddings", False))),
        "num_experts": 0,
    })
    return cfg


def erzeuge_qwen3_moe(roh: dict, variant: str, verified: str, hf: str) -> dict:
    t = text_config(roh)
    if t.get("rope_scaling"):
        raise ValueError("Qwen3-MoE mit RoPE-Skalierung wird von diesem Erzeuger nicht abgebildet")
    if t.get("decoder_sparse_step", 1) != 1:
        raise ValueError("decoder_sparse_step ungleich 1: nicht jede Ebene ist ein Gemisch")
    cfg = _gemeinsam(t, "qwen3-moe", variant, verified, hf)
    cfg.update({
        "intermediate_size": _feld(t, "intermediate_size"),
        "rope_theta": float(_feld(t, "rope_theta")),
        "tie_word_embeddings": bool(roh.get("tie_word_embeddings", t.get("tie_word_embeddings", False))),
        "num_experts": _feld(t, "num_experts"),
        "num_experts_per_tok": _feld(t, "num_experts_per_tok"),
        "moe_intermediate_size": _feld(t, "moe_intermediate_size"),
        "norm_topk_prob": bool(_feld(t, "norm_topk_prob")),
        "mlp_only_layers": list(_feld(t, "mlp_only_layers")),
    })
    return cfg


def erzeuge_qwen3_5_moe(roh: dict, variant: str, verified: str, hf: str) -> dict:
    """Die hybride Bauart des 35B: Gated DeltaNet und Aufmerksamkeit im
    Wechsel, Gemisch mit geteiltem Experten."""
    t = text_config(roh)
    rope = t.get("rope_parameters") or t.get("rope_scaling") or {}
    if rope.get("rope_type", "default") != "default":
        raise ValueError(f"RoPE-Art {rope.get('rope_type')} wird von diesem Erzeuger nicht abgebildet")
    anteil = rope.get("partial_rotary_factor", t.get("partial_rotary_factor", 1.0))
    cfg = _gemeinsam(t, "qwen3_5-moe-hybrid", variant, verified, hf)
    cfg.update({
        # Die RMSNorm dieser Bauart rechnet `1 + weight` (Fund 423).
        "rms_norm_offset": 1.0,
        "intermediate_size": 0,
        "tie_word_embeddings": bool(roh.get("tie_word_embeddings", t.get("tie_word_embeddings", False))),
        "num_experts": _feld(t, "num_experts"),
        "num_experts_per_tok": _feld(t, "num_experts_per_tok"),
        "moe_intermediate_size": _feld(t, "moe_intermediate_size"),
        # Fehlt in der Quelle; die Vorlage normiert immer.
        "norm_topk_prob": True,
        "mlp_only_layers": [],
        "shared_expert_intermediate_size": _feld(t, "shared_expert_intermediate_size"),
        "experts_packed": False,
        "layer_types": list(_feld(t, "layer_types")),
        "linear_num_key_heads": _feld(t, "linear_num_key_heads"),
        "linear_num_value_heads": _feld(t, "linear_num_value_heads"),
        "linear_key_head_dim": _feld(t, "linear_key_head_dim"),
        "linear_value_head_dim": _feld(t, "linear_value_head_dim"),
        "linear_conv_kernel_dim": _feld(t, "linear_conv_kernel_dim"),
        "output_gate_type": "sigmoid" if t.get("attn_output_gate") else "",
        "mtp_num_hidden_layers": 0,
        "rope_theta": float(rope.get("rope_theta", t.get("rope_theta") or 0)),
        "attn_output_gate": bool(t.get("attn_output_gate")),
        "full_attention_interval": _feld(t, "full_attention_interval"),
        "partial_rotary_factor": anteil,
        "rotary_dim": int(_feld(t, "head_dim") * anteil),
        "mrope_section": list(rope.get("mrope_section", [])),
    })
    return cfg


def erzeuge_qwen4_exp(roh: dict, variant: str, verified: str, hf: str) -> dict:
    """Die Bauart mit vier Stroemen (Qwen3.8-Flash-Next). Der Erzeuger steht
    beim ebenenweisen Bau, der ihn auch zur Gegenprobe braucht."""
    from .ebenenweise import export_konfiguration
    return export_konfiguration(text_config(roh), variant, verified, hf)


BAUARTEN = (
    Bauart("Qwen3, dicht", ("qwen3",), "qwen3", "korpus", ".venv", False, erzeuge_qwen3, True),
    Bauart("Qwen3, Expertengemisch", ("qwen3_moe",), "qwen3-moe", "korpus", ".venv", True, erzeuge_qwen3_moe, True),
    Bauart("Qwen3.5/3.6, hybrid mit Expertengemisch", ("qwen3_5_moe", "qwen3_5_moe_text"),
           "qwen3_5-moe-hybrid", "korpus", ".venv", True, erzeuge_qwen3_5_moe, True),
    Bauart("Qwen3.8-Flash-Next, vier Stroeme", ("qwen4_exp", "qwen4_exp_text"),
           "qwen4-exp", "ebenenweise", ".venv-flashnext", True, erzeuge_qwen4_exp, True),
    # ⚠️ Erkannt, aber nicht freigegeben: Die Laufzeit rechnet die Bausteine
    #   (Gated DeltaNet, dichtes MLP, Ausgangstor), aber kein Modell dieser
    #   Form ist ueber den ganzen Weg gebaut und gemessen.
    Bauart("Qwen3.5/3.8, hybrid dicht (etwa Qwen3.8-27B)", ("qwen3_5", "qwen3_5_text"),
           "qwen3_5-hybrid", "korpus", ".venv", False, None, False,
           "noch an keinem Modell ueber den ganzen Weg gebaut und gemessen"),
)


def erkennen(roh: dict) -> Optional[Bauart]:
    """Die Bauart eines `config.json`, oder `None`, wenn sie unbekannt ist."""
    typen = {roh.get("model_type"), text_config(roh).get("model_type")}
    for b in BAUARTEN:
        if typen & set(b.model_types):
            return b
    return None
