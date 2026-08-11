#!/usr/bin/env python3
"""
Verifikations-Bericht: Layer-für-Layer-Vergleich Integer-Runtime vs.
HF-Referenz (Mehrpositions-Sequenz).

Spielt dieselbe Token-Sequenz durch die Integer-Runtime (seq_layer_dump)
und das HF-Referenzmodell und vergleicht pro Layer die Reststrom-Spannweite
(absmax) und die ersten Werte. Zeigt, wo die Integer-Inferenz mit HF
übereinstimmt und wo sie abweicht — die zentrale Verifikation des
Integer-Pfads gegen die Referenz.

Gleitkomma nur in der HF-Referenz (Diagnose). Kein Teil des Auslieferungspfads.

Usage: python verification_layer_compare.py
"""
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

# Erste 8 Tokens der Mess-Sequenz (identisch zu den Seq-Dump-Skripten).
TOKENS = [34532, 425, 10965, 465, 374, 458, 6364, 4531]
ARTIFACTS = REPO / "artifacts" / "qwen2.5-0.5b"
SEQ_DUMP = REPO / "runtime" / "target" / "release" / "seq_layer_dump"


def run_integer_dump():
    """Führt die Integer-Seq-Dump-Binary aus und parst die Layer-Zeilen."""
    result = subprocess.run(
        [str(SEQ_DUMP), str(ARTIFACTS)] + [str(t) for t in TOKENS],
        capture_output=True, text=True, timeout=600)
    layers = {}
    for line in result.stdout.splitlines():
        if line.startswith("layer"):
            # "layer  3: absmax=  10.0625 first4=[...] frac=4"
            parts = line.split("absmax=")
            idx = int(line.split(":")[0].split()[1])
            absmax = float(parts[1].split()[0])
            f4 = parts[1].split("first4=[")[1].split("]")[0]
            first4 = [float(x) for x in f4.split(",")]
            layers[idx] = {"absmax": absmax, "first4": first4}
    return layers


def run_hf_dump():
    """Führt das HF-Modell aus und sammelt Hidden-Zustände je Layer."""
    import torch
    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    input_ids = torch.tensor([TOKENS], device=model.device)
    with torch.no_grad():
        out = model(input_ids=input_ids, output_hidden_states=True)
    hs = out.hidden_states
    layers = {}
    for i in range(len(hs) - 1):
        v = hs[i + 1][0, -1, :].detach().float().cpu()
        layers[i] = {
            "absmax": v.abs().max().item(),
            "first4": v[:4].tolist(),
        }
    return layers


def main():
    print("Starte Integer-Seq-Dump ...")
    int_layers = run_integer_dump()
    print("Starte HF-Referenz ...")
    hf_layers = run_hf_dump()

    print(f"\n{'Layer':>5} {'Int absmax':>11} {'HF absmax':>11} {'Verh.':>7} "
          f"{'first4-Abw.':>12}")
    total_dev = 0.0
    n = 0
    for i in sorted(hf_layers):
        if i not in int_layers:
            continue
        ia = int_layers[i]["absmax"]
        ha = hf_layers[i]["absmax"]
        ratio = ia / ha if ha > 1e-9 else float("inf")
        # first4 relative Abweichung (L2)
        import math
        iv = int_layers[i]["first4"]
        hv = hf_layers[i]["first4"]
        num = math.sqrt(sum((a - b) ** 2 for a, b in zip(iv, hv)))
        den = math.sqrt(sum(b * b for b in hv)) + 1e-9
        f4dev = num / den
        total_dev += f4dev
        n += 1
        print(f"{i:>5} {ia:>11.4f} {ha:>11.4f} {ratio:>7.3f} {f4dev:>12.4f}")

    print(f"\nMittlere first4-Abweichung über {n} Layer: {total_dev/n:.4f}")
    print("(0.0 = perfekt; Werte << 1 zeigen gute Übereinstimmung der")
    print("Bulk-Aktivierung; das absmax-Verhältnis zeigt die Ausreißer-Skala.)")


if __name__ == "__main__":
    main()
