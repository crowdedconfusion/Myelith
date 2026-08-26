#!/usr/bin/env python3
"""
End-to-End-Test mit echten Gewichten auf WikiText-2-Sequenzen
(Punkt 12.19).

Eigenstaendiges Skript nach Projektkonvention (kein pytest). Laeuft in
drei Stufen:
  1. WikiText-2-Testsplit laden (Cache unter eval/datasets/, nicht
     versioniert) und deterministisch Sequenzen auswaehlen.
  2. Die Sequenzen mit dem Qwen-Tokenizer tokenisieren und die
     Integer-Runtime im Teacher-Forcing-Stil darueberlaufen lassen
     (runtime/target/release/perplexity_probe).
  3. Pruefen: zwei Laeufe liefern identische Ausgabe (Determinismus),
     alle Log-Probabilities sind endlich, Perplexitaet ist berechenbar.

Die qualitative Bewertung gegen die Gleitkomma-Baseline gehoert zu den
Punkten 12.20/12.21 — dieser Test stellt nur die Messinfrastruktur bereit
und sichert die Bitexaktheit des Integerpfads.

Steuerung ueber Umgebungsvariablen:
  E2E_SEQUENCES  Anzahl Sequenzen (Standard: 4)
  E2E_SEQ_LEN    Tokens je Sequenz (Standard: 128)
"""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
# Artefaktverzeichnis folgt der Modellwahl (INTEGER_LLM_MODEL), damit der
# Test dieselbe Variante prueft, die kalibriert und gemessen wurde.
# Der Import steht weiter unten bei select_sequences.
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parent.parent))
import cargo_paths  # noqa: E402

PROBE = cargo_paths.binary("runtime", "perplexity_probe")

# Gemeinsame Messsequenzen-Aufbereitung (identische Messmethode mit der
# FP-Baseline und dem Perplexitätsvergleich, siehe eval/wikitext_common.py).
sys.path.insert(0, str(REPO / "eval"))
from wikitext_common import ARTIFACTS_DIR as ARTIFACTS, select_sequences  # noqa: E402


def main():
    n_sequences = int(os.environ.get("E2E_SEQUENCES", "4"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))

    if not PROBE.exists():
        print(f"[e2e] FEHLT: {PROBE} — zuerst 'cargo build --release --bin perplexity_probe'.",
              file=sys.stderr)
        sys.exit(1)
    if not (ARTIFACTS / "theta_v.json").exists():
        print(f"[e2e] FEHLT: Artefakte unter {ARTIFACTS} — zuerst Kalibrierung laufen lassen.",
              file=sys.stderr)
        sys.exit(1)

    # Messsequenzen aus der gemeinsamen Aufbereitung (eval/wikitext_common.py)
    sequences = select_sequences(n_sequences, seq_len)
    total_tokens = sum(len(s) for s in sequences)
    print(f"[e2e] {len(sequences)} Sequenzen, {total_tokens} Tokens insgesamt")

    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        seq_file = f.name
        for ids in sequences:
            f.write(" ".join(str(t) for t in ids) + "\n")

    try:
        runs = []
        for run in (1, 2):
            result = subprocess.run(
                [str(PROBE), str(ARTIFACTS), seq_file],
                capture_output=True, text=True, timeout=7200,
            )
            if result.returncode != 0:
                print(f"[e2e] FEHLT: Probe-Lauf {run} fehlgeschlagen:", file=sys.stderr)
                print(result.stderr, file=sys.stderr)
                sys.exit(1)
            runs.append(result.stdout)
            print(f"[e2e] Lauf {run} fertig.")

        # 1. Determinismus: beide Laeufe bitidentisch
        assert runs[0] == runs[1], "Determinismus-Verletzung: zwei Laeufe unterscheiden sich"
        print("[e2e] Determinismus: PASSED (zwei Laeufe identisch)")

        # 2. Endliche Log-Probabilities und Perplexitaeten
        import math
        n_eval = 0
        sum_logp = 0.0
        per_seq_ppl = []
        for line in runs[0].strip().splitlines():
            toks, count, slp, ppl = line.split()
            count = int(count)
            slp = float(slp)
            ppl = float(ppl)
            assert count >= 1, f"Sequenz ohne ausgewertete Positionen: {line}"
            assert math.isfinite(slp), f"nicht-endliche Summe log p: {line}"
            assert math.isfinite(ppl) and ppl > 0.0, f"unplausible Perplexitaet: {line}"
            n_eval += count
            sum_logp += slp
            per_seq_ppl.append(ppl)

        overall_ppl = math.exp(-sum_logp / n_eval)
        print(f"[e2e] Alle Log-Probabilities endlich: PASSED ({n_eval} Positionen)")
        print(f"[e2e] Per-Sequenz-Perplexitaeten: {[round(p, 2) for p in per_seq_ppl]}")
        print(f"[e2e] Gesamt-Perplexitaet (Integer-Modell): {overall_ppl:.2f}")
        print("[e2e] Hinweis: Die Bewertung gegen die FP-Baseline erfolgt in 12.20/12.21.")
        print("Alle Tests bestanden.")
    finally:
        os.unlink(seq_file)


if __name__ == "__main__":
    main()
