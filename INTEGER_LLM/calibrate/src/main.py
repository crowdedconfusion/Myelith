#!/usr/bin/env python3
"""
Kalibrierungs-Workflow fuer Qwen2.5-0.5B-Instruct.
Phase 3 + Phase 6 Vorbereitung (Gewichtsexport).
"""

import json

from .loader import load_reference_model
from .stats import ActivationStatsCollector
from .scales import compute_scales_from_stats
from .luts import generate_rsqrt_lut, generate_silu_lut, generate_exp_lut, generate_sin_cos_lut
from .export import export_theta_v
from .quantize import quantize_model_weights
from .export_weights import export_quantized_weights
from .model_configs import get_export_model_config
from .paths import model_artifacts_dir

MODEL_NAME = "qwen2.5-0.5b-instruct"
HF_MODEL_ID = "Qwen/Qwen2.5-0.5B-Instruct"


def main():
    print("[calibrate] Lade Referenzmodell...")
    model, tokenizer = load_reference_model(HF_MODEL_ID)

    print("[calibrate] Sammle Aktivierungsstatistiken...")
    collector = ActivationStatsCollector()
    collector.attach(model)

    prompt = "Die numerische Stabilitaet von Fixed-Point-Inferenz ist entscheidend."
    inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
    with torch.no_grad():
        _ = model(**inputs)

    collector.detach()
    stats = collector.compute()
    print(f"[calibrate] Statistiken fuer {len(stats)} Layer gesammelt.")

    print("[calibrate] Berechne Zweierpotenz-Skalen...")
    scales = compute_scales_from_stats(stats)

    print("[calibrate] Generiere LUTs...")
    luts = {
        "rsqrt": generate_rsqrt_lut(max_input=32767, frac_bits=8),
        "silu": generate_silu_lut(input_min=-128, input_max=127, frac_bits=6),
        "exp": generate_exp_lut(exp_range=128, frac_bits=8),
        "sin": generate_sin_cos_lut(n=2048, frac_bits=8)[0],
        "cos": generate_sin_cos_lut(n=2048, frac_bits=8)[1],
    }

    # Wirft klar und fruehzeitig, falls MODEL_NAME auf eine Variante zeigt,
    # deren num_kv_heads/tie_word_embeddings noch nicht gegen die echte
    # HF-config.json verifiziert sind (siehe model_configs.py-Docstring).
    model_config = dict(get_export_model_config(MODEL_NAME))

    artifacts_dir = model_artifacts_dir(MODEL_NAME)

    # Reihenfolge ist bindend, nicht austauschbar: Gewichte zuerst, dann
    # theta_v.json zuletzt - export_theta_v() hasht weights_manifest.json und
    # braucht die Datei deshalb bereits auf der Platte (siehe export.py).
    print("[calibrate] Quantisiere Modell-Gewichte...")
    quantized = quantize_model_weights(model)
    print(f"[calibrate] {len(quantized)} Gewichts-Tensoren quantisiert.")

    print(f"[calibrate] Exportiere Gewichte nach {artifacts_dir}...")
    export_quantized_weights(quantized, artifacts_dir)

    print("[calibrate] Schreibe model_config.json...")
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    (artifacts_dir / "model_config.json").write_text(
        json.dumps(model_config, sort_keys=True, separators=(",", ":"))
    )

    print(f"[calibrate] Exportiere theta_v nach {artifacts_dir}...")
    export_theta_v(scales=scales, luts=luts, output_dir=artifacts_dir)

    print("[calibrate] Exportiere Tokenizer...")
    tokenizer_path = artifacts_dir / "tokenizer.json"
    try:
        tokenizer.backend_tokenizer.save(str(tokenizer_path))
        print(f"[calibrate] Tokenizer exportiert nach {tokenizer_path}")
    except Exception as e:
        print(f"[calibrate] WARNUNG: Tokenizer-Export fehlgeschlagen: {e}")

    print("[calibrate] Fertig. Artefakte in:", artifacts_dir)


if __name__ == "__main__":
    import torch
    main()
