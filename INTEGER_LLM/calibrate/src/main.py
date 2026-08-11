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
from pathlib import Path

from .loader import load_reference_model
from .stats import ActivationStatsCollector
from .scales import compute_scales_from_stats
from .luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                   generate_sin_cos_lut, load_nonlinear_spec)
from .export import export_theta_v
from .quantize import quantize_model_weights, quantize_symmetric_int16_per_channel
from .gptq import HessianCollector, quantize_linear_layers_gptq
from .export_weights import export_quantized_weights, export_lm_head
from .model_configs import get_export_model_config
from .paths import model_artifacts_dir, local_model_dir

MODEL_NAME = "qwen2.5-0.5b"
HF_MODEL_ID = "Qwen/Qwen2.5-0.5B"

# Breiterer Kalibrierungs-Korpus (Fund 14, Kandidat i): Die alten vier
# Kurz-Prompts (~200 Token) deckten die realen Aktivierungs-Spannweiten
# nicht ab — auf den WikiText-2-Messsequenzen clampten 50 von 314 Modulen
# still an der int16-Grenze (Diagnose: tests/diag/scale_headroom_hf.py).
# Deshalb wird zusaetzlich auf einer breiten Stichprobe aus derselben
# Verteilung (WikiText-2-Testsplit) kalibriert.
CALIB_WIKITEXT_SEQUENCES = 64
CALIB_WIKITEXT_SEQ_LEN = 128
_MIN_LINE_CHARS = 160  # identisch zu eval/wikitext_common.py


def _wikitext_calibration_texts(n_sequences):
    """Breite Kalibrier-Stichprobe aus dem WikiText-2-Testsplit-Cache.

    Repliziert die Sequenzauswahl des Entscheidungspunkts
    (eval/wikitext_common.py::select_sequences), waehlt aber eine BREITERE
    Stichprobe und laesst die konkreten Mess-Sequenzen aus, damit die
    Kalibrierung nicht auf den Benchmark ueberpasst. Liefert Rohtexte; die
    Begrenzung auf CALIB_WIKITEXT_SEQ_LEN Tokens geschieht beim
    Tokenisieren (truncation).
    """
    repo_root = Path(__file__).resolve().parent.parent.parent
    cache = repo_root / "eval" / "datasets" / "wikitext2_test.txt"
    if not cache.exists():
        print(f"[calibrate] WARNUNG: WikiText-2-Cache fehlt ({cache}) — "
              "kalibriere nur auf den kuratierten Prompts.")
        return []
    lines = cache.read_text(encoding="utf-8").splitlines()
    candidates = [l.strip() for l in lines if len(l.strip()) >= _MIN_LINE_CHARS]
    if not candidates:
        return []

    # Mess-Sequenzen des Entscheidungspunkts (4 Stueck) bestimmen und
    # aus der Kalibrier-Stichprobe heraushalten.
    eval_stride = max(1, len(candidates) // 4)
    eval_idx = {(i * eval_stride) % len(candidates) for i in range(4)}

    stride = max(1, len(candidates) // n_sequences)
    texts = []
    for i in range(n_sequences):
        idx = (i * stride) % len(candidates)
        if idx in eval_idx:
            idx = (idx + 1) % len(candidates)
        texts.append(candidates[idx])
    return texts


def main():
    model_dir = local_model_dir(HF_MODEL_ID.split("/")[-1])
    print(f"[calibrate] Lade Referenzmodell aus {model_dir} ...")
    model, tokenizer = load_reference_model(model_dir)

    print("[calibrate] Sammle Aktivierungsstatistiken...")
    collector = ActivationStatsCollector()
    collector.attach(model)
    # GPTQ (Eskalationsstrategie 3, theta_v 0.8.0): dieselbe Vorwaerts-
    # Passage liefert die Hessischen Matrizen (H = Summe x x^T) fuer die
    # Fehlerkompensations-Quantisierung der linearen Projektionen.
    hessian_collector = HessianCollector()
    hessian_collector.attach(model)

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

    # Breiterer WikiText-2-Korpus (Fund 14, Kandidat i): dieselbe Verteilung
    # wie die Messsequenzen, aber breit genug, damit die Per-Layer-Skalen die
    # realen Aktivierungs-Spannweiten abdecken statt still zu clampen.
    wikitext_texts = _wikitext_calibration_texts(CALIB_WIKITEXT_SEQUENCES)
    print(f"[calibrate] Breite Kalibrierbasis: {len(wikitext_texts)} "
          f"WikiText-2-Sequenzen à <= {CALIB_WIKITEXT_SEQ_LEN} Tokens ...")
    for text in wikitext_texts:
        inputs = tokenizer(text, return_tensors="pt", truncation=True,
                           max_length=CALIB_WIKITEXT_SEQ_LEN).to(model.device)
        with torch.no_grad():
            _ = model(**inputs)

    collector.detach()
    hessian_collector.detach()
    stats = collector.compute()
    print(f"[calibrate] Statistiken fuer {len(stats)} Module gesammelt.")
    print(f"[calibrate] Hessische Matrizen fuer {len(hessian_collector.hessians)} "
          f"lineare Projektionen gesammelt.")

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
    print(f"[calibrate] {len(quantized)} Gewichts-Tensoren quantisiert "
          "(Per-Channel RNE).")

    # GPTQ (theta_v 0.8.0, Eskalationsstrategie 3): die linearen
    # Projektionen werden mit Hessian-gestützter Fehlerkompensation
    # nachquantisiert — das überschreibt die RNE-Einträge für exakt diese
    # Tensoren (gleiche Schlüssel, gleiches Artefakt-Format). Reduziert den
    # Ausgabefehler statt des Gewichtsfehlers und damit das akkumulierte
    # Quantisierungsrauschen (Fund 14).
    print("[calibrate] GPTQ: quantisiere lineare Projektionen mit "
          "Fehlerkompensation...")
    gptq_quantized = quantize_linear_layers_gptq(model, hessian_collector.hessians)
    quantized.update(gptq_quantized)
    print(f"[calibrate] GPTQ auf {len(gptq_quantized)} lineare Projektionen "
          "angewendet.")

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
