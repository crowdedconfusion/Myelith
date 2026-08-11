#!/usr/bin/env python3
"""
Kalibrierungs-Workflow fuer Qwen2.5-0.5B (Basis-Modell).
Phase 3 + Phase 6 Vorbereitung (Gewichtsexport).

Referenzmodell ist die Basis-Variante Qwen/Qwen2.5-0.5B — konsistent mit
Whitepaper, oeffentlichem README und models/README.md. Das Modell wird
ausschliesslich aus dem lokalen Snapshot unter models/ geladen (siehe
loader.py und models/README.md).
"""

import json

from .loader import load_reference_model
from .stats import ActivationStatsCollector
from .scales import compute_scales_from_stats
from .luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                   generate_sin_cos_lut, load_nonlinear_spec)
from .export import export_theta_v
from .quantize import quantize_model_weights, quantize_symmetric_int16_per_channel
from .export_weights import export_quantized_weights, export_lm_head
from .model_configs import get_export_model_config
from .paths import model_artifacts_dir, local_model_dir

MODEL_NAME = "qwen2.5-0.5b"
HF_MODEL_ID = "Qwen/Qwen2.5-0.5B"


def main():
    model_dir = local_model_dir(HF_MODEL_ID.split("/")[-1])
    print(f"[calibrate] Lade Referenzmodell aus {model_dir} ...")
    model, tokenizer = load_reference_model(model_dir)

    print("[calibrate] Sammle Aktivierungsstatistiken...")
    collector = ActivationStatsCollector()
    collector.attach(model)

    # Kalibrierungs-Korpus: mehrere sprachlich unterschiedliche Prompts,
    # damit die Per-Layer-Aktivierungsskalen nicht von einem einzigen Satz
    # abhaengen (Numerik-Realitaetsabgleich v0.12.20: die Skalen tragen
    # jetzt den kompletten Aktivierungsfluss, inkl. RMSNorm-Ausgaben).
    prompts = [
        "Die numerische Stabilitaet von Fixed-Point-Inferenz ist entscheidend "
        "fuer die Bitgleichheit ueber unabhaengige Knoten hinweg.",
        "Decentralized consensus networks coordinate independent nodes by "
        "verifying identical computation, and deterministic integer "
        "arithmetic enables dispute resolution through bisection.",
        "Ein Agent plant mehrere Schritte, ruft Werkzeuge auf und beachtet "
        "dabei Budgetgrenzen, bevor er eine Transaktion signiert.",
        "Quantization maps floating point weights to int8 with calibrated "
        "power-of-two scales; lookup tables approximate nonlinear functions "
        "such as silu, exp, rsqrt and the rotary position embeddings.",
    ]
    for prompt in prompts:
        inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
        with torch.no_grad():
            _ = model(**inputs)

    collector.detach()
    stats = collector.compute()
    print(f"[calibrate] Statistiken fuer {len(stats)} Module gesammelt.")

    print("[calibrate] Berechne Zweierpotenz-Skalen...")
    scales = compute_scales_from_stats(stats)

    print("[calibrate] Generiere LUTs (Parameter aus theta_v/spec.json)...")
    # Fahrplan 12.17: alle LUT-Parameter kommen aus dem "nonlinear"-Abschnitt
    # der spec.json (Single Source of Truth), keine hartkodierten Duplikate.
    # Kopplung (Numerik-Realitaetsabgleich v0.12.20): Die SiLU-LUT arbeitet
    # in einer festen Eingangsskala (ihre frac_bits = 6); die Runtime
    # reskaliert Gate-Werte vor dem Lookup dorthin. Die exp-LUT-frac (8) ist
    # die Score-Skala der Attention. Die rsqrt-LUT wird per dynamischem
    # geradem Index-Shift gespeist (spec: index_normalization).
    nl = load_nonlinear_spec()
    sin_lut, cos_lut = generate_sin_cos_lut(
        n=nl["rope"]["max_seq_len"], frac_bits=nl["rope"]["frac_bits"])
    luts = {
        "rsqrt": generate_rsqrt_lut(
            max_input=nl["rsqrt"]["input_range"][1],
            input_shift=nl["rsqrt"]["input_shift"],
            frac_bits=nl["rsqrt"]["output_frac_bits"]),
        "silu": generate_silu_lut(
            input_min=nl["silu"]["input_range"][0],
            input_max=nl["silu"]["input_range"][1],
            input_frac_bits=nl["silu"]["input_frac_bits"],
            output_frac_bits=nl["silu"]["output_frac_bits"]),
        "exp": generate_exp_lut(
            exp_range=nl["softmax"]["exp_lut_range"],
            input_frac_bits=nl["softmax"]["exp_input_frac_bits"],
            output_frac_bits=nl["softmax"]["exp_lut_frac_bits"]),
        "sin": sin_lut,
        "cos": cos_lut,
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

    # Eskalation nach Entscheidungspunkt 12.21 (spec-Ausnahme 0.6.0): der
    # LM-Head wird als EIGENER Tensor exportiert (Weight-Tying aufgelöst),
    # in int16 mit Per-Channel-Zweierpotenz-Skalen. Muss VOR export_theta_v
    # laufen, damit der theta_v-Gewichtshash den aktualisierten
    # weights_manifest-Eintrag einschließt.
    print("[calibrate] Quantisiere LM-Head (int16, per-channel)...")
    lm_head_weight = model.get_output_embeddings().weight
    lm_head_quant = quantize_symmetric_int16_per_channel(lm_head_weight)
    export_lm_head(lm_head_quant, artifacts_dir)

    # Das Artefakt dokumentiert die LM-Head-Ausnahme im model_config.
    model_config["lm_head"] = {"dtype": "int16", "scale": "per_channel"}

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
