#!/usr/bin/env python3
"""Skalen-Headroom-Check (Fund-14-Kandidat i).

Misst die realen Aktivierungs-Spannweiten auf denselben WikiText-2-
Sequenzen wie der Entscheidungspunkt 12.21 und vergleicht sie mit den
kalibrierten Per-Layer-Skalen aus `artifacts/.../scales.json`.

Hintergrund: die Aktivierungsskalen wurden auf nur vier kurzen Prompts
(~200 Token) kalibriert. Wenn die realen Mess-Sequenzen groessere
Spannweiten haben, clampen die Werte im Integer-Laufzeitpfad still an der
int16-Grenze — das waere eine systematische Fehlerquelle, die in der
Layer-Probe an Position 0 nicht sichtbar ist (Diagnose-Fund 14, Kandidat i).

Reine Diagnose (Gleitkomma ist hier erlaubt — dies ist die Referenzmessung,
nicht der Integer-Inferenzpfad). Kein Teil des Auslieferungspfads.

Usage: python scale_headroom_hf.py
"""
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))

from src.loader import load_reference_model          # noqa: E402
from src.stats import ActivationStatsCollector       # noqa: E402
from wikitext_common import select_sequences         # noqa: E402
from src.paths import model_artifacts_dir            # noqa: E402

ACTIVATION_MAX_INT = 32767
MODEL_NAME = "qwen2.5-0.5b"


def main():
    import torch

    # Messparameter aus dem Baseline-Ergebnis lesen — garantiert dieselben
    # Sequenzen wie der Entscheidungspunkt 12.21.
    baseline = json.loads(
        (REPO / "eval" / "results" / "baseline_wikitext2.json").read_text())
    n_sequences = baseline["n_sequences"]
    seq_len = baseline["seq_len"]
    sequences = select_sequences(n_sequences, seq_len)

    print(f"[headroom] Lade Referenzmodell ...")
    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()

    collector = ActivationStatsCollector()
    collector.attach(model)
    with torch.no_grad():
        for ids in sequences:
            input_ids = torch.tensor([ids], device=model.device)
            _ = model(input_ids=input_ids)
    collector.detach()
    real_stats = collector.compute()

    scales_path = model_artifacts_dir(MODEL_NAME) / "scales.json"
    scales = json.loads(scales_path.read_text())

    print(f"[headroom] {len(sequences)} Sequenzen à {seq_len} Tokens, "
          f"{len(real_stats)} Module vermessen, Skalen aus {scales_path.name}")
    print()

    # Für jedes kalibrierte Modul: reale Spanne vs. darstellbare Spanne.
    # headroom = darstellbare_spanne / reale_spanne.
    #   headroom >= 1  -> kein Clamping, Wert passt
    #   headroom <  1  -> reale Werte ueberschreiten die Skala -> Clamping
    rows = []
    for name, entry in scales.items():
        shift = entry["shift"]
        representable = ACTIVATION_MAX_INT * (2.0 ** (-shift))
        real = real_stats.get(name, {}).get("absmax")
        if real is None:
            continue
        headroom = representable / real if real > 0 else float("inf")
        rows.append((name, shift, real, representable, headroom))

    # Schlechteste zuerst.
    rows.sort(key=lambda r: r[4])

    clamping = [r for r in rows if r[4] < 1.0]
    tight = [r for r in rows if 1.0 <= r[4] < 1.5]

    print(f"{'Modul':<58} {'Shift':>5} {'real':>9} {'darstellb.':>10} {'Headroom':>8}")
    print("-" * 96)
    for name, shift, real, rep, hr in rows[:30]:
        flag = "CLAMP" if hr < 1.0 else ("knapp" if hr < 1.5 else "")
        print(f"{name:<58} {shift:>5} {real:>9.2f} {rep:>10.2f} {hr:>7.2f}x {flag}")

    print()
    print(f"[headroom] Module gesamt:        {len(rows)}")
    print(f"[headroom] mit Clamping (<1x):    {len(clamping)}")
    print(f"[headroom] knapp (1x–1.5x):       {len(tight)}")
    if clamping:
        print()
        print("[headroom] CLAMPENDE Module (reale Spanne > darstellbare Spanne):")
        for name, shift, real, rep, hr in clamping:
            print(f"  {name}: real {real:.2f} > darstellbar {rep:.2f} "
                  f"(Shift {shift}, Faktor {1/hr:.2f}x ueber Skala)")


if __name__ == "__main__":
    main()
