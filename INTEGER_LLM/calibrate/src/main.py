#!/usr/bin/env python3
"""
Kalibrierungs-Workflow fuer die Qwen2.5-Basis-Reihe.
Phase 3 + Phase 6 Vorbereitung (Gewichtsexport).

Referenzmodell ist die Basis-Variante (keine Instruct-Variante) — konsistent
mit Whitepaper, oeffentlichem README und models/README.md. Das Modell wird
ausschliesslich aus dem lokalen Snapshot unter models/ geladen (siehe
loader.py und models/README.md), nie aus dem impliziten HF-Cache.

**Modellwahl** ueber die Umgebungsvariable INTEGER_LLM_MODEL, Vorgabe
qwen2.5-0.5b:

    INTEGER_LLM_MODEL=qwen2.5-7b python -m calibrate.src.main

Waehlbar sind nur Varianten, deren Felder gegen die echte HF-config.json
geprueft sind (model_configs.py, Feld "verified"). Jede Variante bekommt ihr
eigenes Artefaktverzeichnis unter artifacts/<name>/; ein Lauf ueberschreibt
nie das Artefakt einer anderen Groesse.

Was sich mit der Modellgroesse *nicht* aendert: der numerische Vertrag in
theta_v/spec.json. Die Runtime liest daraus nur Format- und
Nichtlinearitaets-Parameter; die Dimensionen kommen aus dem
model_config.json des Artefakts. Der Wechsel auf eine andere Groesse ist
deshalb ein Artefaktwechsel, keine Codeaenderung.
"""

import gc
import json
import math
import os
from pathlib import Path

from .loader import load_reference_model
from .stats import ActivationStatsCollector
from .scales import compute_scales_from_stats
from .luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                   generate_rope_luts, load_nonlinear_spec)
from .export import export_theta_v
from .quantize import quantize_model_weights, quantize_symmetric_int16_per_channel
from .gptq import HessianCollector, quantize_linear_layers_gptq
from .export_weights import export_quantized_weights, export_lm_head
from .model_configs import get_export_model_config, artifact_model_config
from .paths import model_artifacts_dir, local_model_dir

MODEL_ENV = "INTEGER_LLM_MODEL"
DEFAULT_MODEL = "qwen2.5-0.5b"

MODEL_NAME = os.environ.get(MODEL_ENV, "").strip() or DEFAULT_MODEL
# Die HF-ID steht in der verifizierten Config, nicht hier: sie gehoert zur
# Variante, und eine zweite Stelle waere eine zweite Wahrheit. Der Aufruf
# schlaegt fehl, falls MODEL_NAME auf eine ungeprueffte Variante zeigt -
# und zwar bevor irgendetwas geladen wird.
HF_MODEL_ID = get_export_model_config(MODEL_NAME)["hf_model_id"]

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


def gptq_hessian_bytes(config: dict) -> int:
    """
    RAM, den HessianCollector fuer ALLE Ebenen dieser Modellgroesse
    GLEICHZEITIG belegen wuerde (siehe gptq_group_size fuer die
    schichtweise Alternative, die tatsaechlich verwendet wird).

    H ist je linearer Projektion eine Gram-Matrix [in_features, in_features]
    in float32. Die sechs Projektionen mit in = hidden_size sind harmlos; die
    Kosten stecken in down_proj, deren Eingang intermediate_size ist:

        0.5B  24 Ebenen x (6 x 896^2 + 4864^2) x 4 B  =  2,5 GB
        7B    28 Ebenen x (6 x 3584^2 + 18944^2) x 4 B = 45,5 GB

    Der Sprung ist quadratisch in intermediate_size, nicht linear in der
    Parameterzahl. Deshalb wird das hier ausgerechnet und nicht geschaetzt.
    """
    h = config["hidden_size"]
    i = config["intermediate_size"]
    return config["num_layers"] * (6 * h * h + i * i) * 4


def gptq_hessian_bytes_per_layer(config: dict) -> int:
    """RAM einer einzelnen Ebene (Baustein fuer gptq_group_size)."""
    h = config["hidden_size"]
    i = config["intermediate_size"]
    return (6 * h * h + i * i) * 4


def gptq_group_size(config: dict) -> int:
    """
    Schichtweise Hessian-Berechnung (2026-08-18, Nachtrag zu Fahrplan 12.72):
    statt GPTQ bei zu wenig RAM ganz abzuschalten (v0.12.43-Verhalten),
    wird nur so viel gleichzeitig gehesst, wie in zwei Drittel des
    verfuegbaren RAM passt. Bei 0,5B ergibt sich eine einzige Gruppe (alle
    24 Ebenen passen ohnehin, 2,5 GB) - unveraendertes Verhalten. Bei 7B
    ergeben sich mehrere Gruppen; jede Gruppe braucht einen eigenen
    Kalibrier-Durchlauf durch das Modell (main()::gptq_group_size-Aufrufer),
    also mehr Rechenzeit fuer denselben Speicherrahmen.
    """
    num_layers = config["num_layers"]
    per_layer = max(1, gptq_hessian_bytes_per_layer(config))
    ram = verfuegbarer_ram()
    if not ram:
        return num_layers  # unbekannt -> altes Verhalten (eine Gruppe)
    budget = ram * 2 // 3
    group = max(1, budget // per_layer)
    return min(group, num_layers)


def verfuegbarer_ram() -> int:
    """Physischer Arbeitsspeicher in Bytes; 0, wenn nicht ermittelbar."""
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (ValueError, OSError, AttributeError):
        return 0


def gptq_entscheidung(config: dict) -> tuple:
    """
    Entscheidet, ob GPTQ in diesem Lauf mitlaeuft. Rueckgabe: (bool, Grund).

    **Nachtrag 2026-08-18 (Fahrplan 12.72):** Bis v0.12.43 schaltete diese
    Funktion GPTQ komplett ab, wenn der Hessian-Bedarf fuer ALLE Ebenen
    gleichzeitig zwei Drittel des RAM ueberschritt (7B: 45,5 GB). Das war
    ein Test, ob GPTQs Fehlerkompensation die 7B-Ergebnisse verbessert -
    ohne GPTQ liess sich das nicht pruefen. Jetzt laeuft GPTQ immer
    (schichtweise, siehe gptq_group_size); nur INTEGER_LLM_GPTQ=0 schaltet
    es noch hart ab, fuer schnelle Laeufe ohne Fehlerkompensation.
    """
    env = os.environ.get("INTEGER_LLM_GPTQ", "").strip()
    if env in ("0", "aus", "off", "false"):
        return False, "INTEGER_LLM_GPTQ=0"
    group_size = gptq_group_size(config)
    num_layers = config["num_layers"]
    num_groups = math.ceil(num_layers / group_size)
    if num_groups == 1:
        return True, f"eine Gruppe (alle {num_layers} Ebenen gleichzeitig)"
    return True, (
        f"{num_groups} Gruppen a bis zu {group_size} Ebenen (schichtweise, "
        f"{gptq_hessian_bytes(config) / 2**30:.1f} GB fuer alle Ebenen "
        f"gleichzeitig waeren zu viel gewesen)"
    )


def main():
    config_vorab = get_export_model_config(MODEL_NAME)
    use_gptq, gptq_grund = gptq_entscheidung(config_vorab)
    print(f"[calibrate] Modell: {MODEL_NAME} ({HF_MODEL_ID}), "
          f"verifiziert gegen {config_vorab['verified']}")

    model_dir = local_model_dir(HF_MODEL_ID.split("/")[-1])
    print(f"[calibrate] Lade Referenzmodell aus {model_dir} ...")
    model, tokenizer = load_reference_model(model_dir)

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
    # Breiterer WikiText-2-Korpus (Fund 14, Kandidat i): dieselbe Verteilung
    # wie die Messsequenzen, aber breit genug, damit die Per-Layer-Skalen die
    # realen Aktivierungs-Spannweiten abdecken statt still zu clampen.
    wikitext_texts = _wikitext_calibration_texts(CALIB_WIKITEXT_SEQUENCES)
    print(f"[calibrate] Breite Kalibrierbasis: {len(wikitext_texts)} "
          f"WikiText-2-Sequenzen à <= {CALIB_WIKITEXT_SEQ_LEN} Tokens ...")

    def _kalibrierkorpus_durchlaufen():
        """Fuehrt den vollstaendigen Kalibrierkorpus einmal durch das
        Modell. Wird fuer die Stats-Sammlung UND, bei mehreren
        GPTQ-Gruppen (schichtweise Hessian-Berechnung), je Gruppe erneut
        aufgerufen - deshalb als geschlossene Funktion statt Inline-Code."""
        for prompt in prompts:
            inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
            with torch.no_grad():
                _ = model(**inputs)
        for text in wikitext_texts:
            inputs = tokenizer(text, return_tensors="pt", truncation=True,
                               max_length=CALIB_WIKITEXT_SEQ_LEN).to(model.device)
            with torch.no_grad():
                _ = model(**inputs)

    print("[calibrate] Sammle Aktivierungsstatistiken...")
    collector = ActivationStatsCollector()
    collector.attach(model)
    _kalibrierkorpus_durchlaufen()
    collector.detach()
    stats = collector.compute()
    print(f"[calibrate] Statistiken fuer {len(stats)} Module gesammelt.")

    # GPTQ (Eskalationsstrategie 3, theta_v 0.8.0): Hessian-gestuetzte
    # Fehlerkompensation fuer die linearen Projektionen. Schichtweise
    # (2026-08-18, Nachtrag 12.72): der Kalibrierkorpus laeuft je Gruppe
    # ERNEUT durch das Modell, mit Hooks nur auf dieser Gruppe - mehr
    # Rechenzeit, aber Speicherbedarf bleibt beschraenkt statt GPTQ bei
    # grossen Modellen ganz abzuschalten.
    gptq_quantized = {}
    if use_gptq:
        group_size = gptq_group_size(config_vorab)
        num_layers = config_vorab["num_layers"]
        num_groups = math.ceil(num_layers / group_size)
        print(f"[calibrate] GPTQ ({gptq_grund})...")
        for g in range(num_groups):
            start = g * group_size
            end = min(start + group_size, num_layers)
            print(f"[calibrate] GPTQ-Gruppe {g + 1}/{num_groups} "
                  f"(Ebenen {start}-{end - 1})...")
            hessian_collector = HessianCollector(layer_range=range(start, end))
            hessian_collector.attach(model)
            _kalibrierkorpus_durchlaufen()
            hessian_collector.detach()
            gptq_quantized.update(
                quantize_linear_layers_gptq(model, hessian_collector.hessians))
            del hessian_collector
            gc.collect()
        print(f"[calibrate] GPTQ auf {len(gptq_quantized)} lineare Projektionen "
              "angewendet.")
    else:
        print(f"[calibrate] GPTQ ausgelassen ({gptq_grund}).")

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
    # Wirft klar und fruehzeitig, falls MODEL_NAME auf eine Variante zeigt,
    # deren num_kv_heads/tie_word_embeddings noch nicht gegen die echte
    # HF-config.json verifiziert sind (siehe model_configs.py-Docstring).
    # (Vor die LUT-Erzeugung gezogen, da RoPE head_dim braucht, Fund-15-Fix.)
    model_config = artifact_model_config(MODEL_NAME)
    # RoPE (Fund-15-Fix, theta_v 0.10.0): Multi-Frequenz-LUTs mit
    # half-split-Paarung. head_dim aus der (verifizierten) Modell-Config —
    # bei 7B ist es 128 statt 64, die LUTs werden entsprechend doppelt so
    # breit. rope_theta aus der spec (die gesamte Qwen2.5-Reihe: 1e6).
    sin_lut, cos_lut = generate_rope_luts(
        max_seq_len=nl["rope"]["max_seq_len"],
        head_dim=model_config["head_dim"],
        rope_theta=nl["rope"]["rope_theta"],
        frac_bits=nl["rope"]["frac_bits"])
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

    artifacts_dir = model_artifacts_dir(MODEL_NAME)

    # Reihenfolge ist bindend, nicht austauschbar: Gewichte zuerst, dann
    # theta_v.json zuletzt - export_theta_v() hasht weights_manifest.json und
    # braucht die Datei deshalb bereits auf der Platte (siehe export.py).
    print("[calibrate] Quantisiere Modell-Gewichte...")
    quantized = quantize_model_weights(model)
    print(f"[calibrate] {len(quantized)} Gewichts-Tensoren quantisiert "
          "(Per-Channel RNE).")

    # GPTQ ueberschreibt die RNE-Eintraege fuer exakt die Tensoren, die
    # oben schichtweise gptq-quantisiert wurden (gleiche Schluessel,
    # gleiches Artefakt-Format). Reduziert den Ausgabefehler statt des
    # Gewichtsfehlers und damit das akkumulierte Quantisierungsrauschen
    # (Fund 14).
    if gptq_quantized:
        quantized.update(gptq_quantized)

    # Eskalation nach Entscheidungspunkt 12.21 (spec-Ausnahme 0.6.0): der
    # LM-Head wird als EIGENER Tensor exportiert (Weight-Tying aufgelöst),
    # in int16 mit Per-Channel-Zweierpotenz-Skalen.
    #
    # Steht VOR dem Gewichtsexport, obwohl er danach geschrieben wird: er ist
    # die letzte Stelle, die das BF16-Modell braucht. Danach kann es aus dem
    # Speicher, und der Export laeuft ohne es. Das ist bei 0,5B gleichgueltig
    # und bei 7B der Unterschied zwischen 26 GB Spitzenbedarf (Modell 15,2 GB
    # + Quantisat 8,7 GB) und rund 11 GB auf einer 24-GB-Maschine.
    print("[calibrate] Quantisiere LM-Head (int16, per-channel)...")
    lm_head_weight = model.get_output_embeddings().weight
    lm_head_quant = quantize_symmetric_int16_per_channel(lm_head_weight)

    del lm_head_weight, model
    gc.collect()
    print("[calibrate] Referenzmodell freigegeben (wird ab hier nicht mehr gebraucht).")

    print(f"[calibrate] Exportiere Gewichte nach {artifacts_dir}...")
    export_quantized_weights(quantized, artifacts_dir)

    # Muss VOR export_theta_v laufen, damit der theta_v-Gewichtshash den
    # aktualisierten weights_manifest-Eintrag einschliesst.
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
