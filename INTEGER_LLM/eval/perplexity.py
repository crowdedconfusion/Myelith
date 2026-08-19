#!/usr/bin/env python3
"""
Perplexitätsvergleich: Integer-Modell vs. Gleitkomma-Baseline
(Fahrplan-Punkt 12.21 — der Entscheidungspunkt).

Methodik:
  1. Messparameter (Anzahl Sequenzen, Sequenzlänge) werden aus dem
     Baseline-Ergebnis (eval/results/baseline_wikitext2.json) gelesen —
     dadurch ist garantiert, dass beide Messungen auf identischen
     Sequenzen laufen (eval/wikitext_common.py).
  2. Die Integer-Perplexität wird mit der Perplexitäts-Probe der Runtime
     gemessen (Teacher-Forcing, identische Messmethode wie 12.19).
  3. Relatives Delta = (ppl_integer - ppl_fp) / ppl_fp wird gegen das
     Akzeptanzkriterium geprüft (Standard: max. 5 % Anstieg, Vorschlag des
     Fahrplans; das Kriterium ist konsensrelevant und kann über
     PPL_ACCEPTANCE_PCT gesetzt werden).
  4. Ein Ergebnisprotokoll wird geschrieben: eval/results/decision_12-21.md
     — es enthält zwingend die zwei Mess-Hinweise aus dem Fahrplan
     (Decodierstrategie, 0,5B-als-ungünstigster-Fall).
"""

import json
import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

sys.path.insert(0, str(REPO / "eval"))
sys.path.insert(0, str(REPO / "tests"))
from wikitext_common import (  # noqa: E402
    ARTIFACTS_DIR as ARTIFACTS,
    HF_MODEL_ID,
    MODEL_NAME,
    ergebnis_pfad,
    select_sequences,
)
# Seit alle Crates in ein gemeinsames target-shared/ bauen (.cargo/config.toml)
# liegt das Binary nicht mehr unter runtime/target/. Der Resolver prueft
# CARGO_TARGET_DIR, target-shared/ und den Cargo-Standardort der Reihe nach.
from cargo_paths import binary, fehlt_hinweis  # noqa: E402

PROBE = binary("runtime", "perplexity_probe")
BASELINE_JSON = ergebnis_pfad("baseline_wikitext2")
RESULTS_DIR = REPO / "eval" / "results"


def main():
    acceptance_pct = float(os.environ.get("PPL_ACCEPTANCE_PCT", "5.0"))

    if not BASELINE_JSON.exists():
        print(f"[ppl] FEHLT: {BASELINE_JSON} — zuerst eval/baseline.py laufen lassen "
              f"(fuer dasselbe Modell: INTEGER_LLM_MODEL={MODEL_NAME}).",
              file=sys.stderr)
        sys.exit(1)
    if not PROBE.exists():
        print(f"[ppl] {fehlt_hinweis('runtime', 'perplexity_probe')}",
              file=sys.stderr)
        sys.exit(1)

    baseline = json.loads(BASELINE_JSON.read_text(encoding="utf-8"))
    n_sequences = baseline["n_sequences"]
    seq_len = baseline["seq_len"]
    ppl_fp = baseline["perplexity"]

    # θ_v-Beschreibung aus der spec ableiten (Single Source of Truth),
    # damit das Protokoll bei künftigen Eskalationsstufen nicht veraltet.
    spec = json.loads((REPO / "theta_v" / "spec.json").read_text(encoding="utf-8"))
    theta_v = spec["theta_v"]
    spec_version = theta_v["version"]
    scales = theta_v.get("numeric", {}).get("scales", {})
    weight_scale = scales.get("weight_scale", "per_tensor")
    activation_scale = scales.get("activation_scale", "per_layer")
    theta_v_desc = (f"θ_v {spec_version} (Gewichte int8 {weight_scale}, "
                    f"Aktivierungen int16 {activation_scale}, "
                    f"LM-Head int16 per-channel als benannte spec-Ausnahme)")
    print(f"[ppl] FP-Baseline: Perplexitaet {ppl_fp:.2f} "
          f"({baseline['evaluated_tokens']} Positionen, "
          f"{n_sequences} Sequenzen à {seq_len} Tokens)")

    # Identische Sequenzen wie die Baseline (Parameter aus deren JSON).
    sequences = select_sequences(n_sequences, seq_len)

    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        seq_file = f.name
        for ids in sequences:
            f.write(" ".join(str(t) for t in ids) + "\n")

    try:
        result = subprocess.run(
            [str(PROBE), str(ARTIFACTS), seq_file],
            capture_output=True, text=True, timeout=7200,
        )
        if result.returncode != 0:
            print("[ppl] FEHLT: Probe-Lauf fehlgeschlagen:", file=sys.stderr)
            print(result.stderr, file=sys.stderr)
            sys.exit(1)

        n_eval = 0
        sum_logp = 0.0
        per_seq = []
        for line in result.stdout.strip().splitlines():
            toks, count, slp, ppl = line.split()
            count, slp, ppl = int(count), float(slp), float(ppl)
            n_eval += count
            sum_logp += slp
            per_seq.append({"tokens": int(toks), "evaluated": count,
                            "sum_logp": slp, "perplexity": ppl})
        ppl_int = math.exp(-sum_logp / n_eval)
    finally:
        os.unlink(seq_file)

    delta_pct = (ppl_int - ppl_fp) / ppl_fp * 100.0
    accepted = delta_pct <= acceptance_pct

    print(f"[ppl] Integer-Modell: Perplexitaet {ppl_int:.2f} ({n_eval} Positionen)")
    print(f"[ppl] Relativer Anstieg: {delta_pct:+.2f} % "
          f"(Akzeptanzkriterium: max. {acceptance_pct:.1f} %)")
    print(f"[ppl] Entscheidung: {'AKZEPTIERT' if accepted else 'VERFEHLT'}")

    protocol = f"""# Entscheidungspunkt 12.21 — Perplexitätsvergleich

**Datum:** (automatisch erzeugt durch eval/perplexity.py)

## Messung

| Größe | Wert |
|---|---|
| Modell | {HF_MODEL_ID} (Basis-Variante) |
| FP-Baseline | BF16, HF-Implementierung: Perplexität {ppl_fp:.2f} |
| Integer-Modell | {theta_v_desc}: Perplexität {ppl_int:.2f} |
| Datensatz | WikiText-2, Testsplit; {n_sequences} Sequenzen à {seq_len} Tokens ({n_eval} ausgewertete Positionen) |
| Relativer Anstieg | **{delta_pct:+.2f} %** |
| Akzeptanzkriterium | max. {acceptance_pct:.1f} % relativer Anstieg |
| **Ergebnis** | **{'AKZEPTIERT' if accepted else 'VERFEHLT'}** |

## Zwingende Einordnung

1. **Decodierstrategie:** Perplexität ist unabhängig von der
   Decodierstrategie, die beobachtete Repetitionsneigung nicht — Greedy
   verstärkt sie. Die hier gemessene Perplexität (Teacher-Forcing) ist
   daher das maßgebliche Qualitätsmaß; die in Fund 9 beobachteten
   Repetitions-Loops bei Greedy-Generierung sind ein Teil-Decodier-
   strategie-Effekt und nicht allein der Quantisierung zuzurechnen.
2. **0,5 Mrd. Parameter sind der ungünstigste Fall für Quantisierung.**
   Größere Modelle sind nachweislich robuster (größere Logit-Spannweiten,
   gutmütigere Gewichtsverteilungen). {'Falls das Kriterium verfehlt wurde: Das ist ein Urteil über 0,5B — nicht über die Zielgrößenordnung des Whitepapers.' if not accepted else 'Das Kriterium wurde erreicht; die Übertragbarkeit auf die Zielgrößenordnung bleibt durch die grundsätzliche Robustheit größerer Modelle zusätzlich gestützt.'}

## Konsequenz

{'Das Akzeptanzkriterium ist erfüllt — die Ganzzahl-Inferenz trägt qualitativ auf diesem Modell. Die weiteren Backends (SIMD/CUDA/ROCm) und die Netzwerkkomponenten können auf dieser Basis weiterverfolgt werden.' if accepted else 'Das Akzeptanzkriterium ist verfehlt. Bereits umgesetzte Eskalationsstufen: Weight-Tying aufgelöst + LM-Head int16 per-channel (spec 0.6.0) und Per-Channel-int8 für alle Gewichte (spec 0.7.0). Der verbleibende Abstand verlangt weitere Eskalation — Kandidaten: breitere Kalibrierbasis/Skalen-Headroom, feinere Teilbit-Tiefen der Nichtlinearitäten (z. B. SiLU-Eingangsskala), GPTQ, Hadamard-Rotation, Low-Rank-Fehlerkorrektur, deterministisch-stochastisches Runden.'}
"""
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out_md = ergebnis_pfad("decision_12-21", ".md")
    out_md.write_text(protocol, encoding="utf-8")

    out_json = ergebnis_pfad("perplexity_comparison")
    out_json.write_text(json.dumps({
        "baseline_perplexity": ppl_fp,
        "integer_perplexity": ppl_int,
        "delta_pct": delta_pct,
        "acceptance_pct": acceptance_pct,
        "accepted": accepted,
        "evaluated_tokens": n_eval,
        "sequences": per_seq,
    }, indent=2, ensure_ascii=False), encoding="utf-8")

    print(f"[ppl] Ergebnisprotokoll: {out_md}")


if __name__ == "__main__":
    main()
