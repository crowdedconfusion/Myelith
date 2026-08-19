#!/usr/bin/env python3
"""
Simuliert positionsabhaengige Aktivierungsskalen — OHNE Rust-Umbau.

These (2026-08-19): Die "Massive Activations" leben in der POSITIONS-,
nicht in der Kanal-Dimension. Fund 20 gibt jedem Kanal eine eigene Skala,
konstant ueber alle Positionen. Der Ausreisser an Position 0 zwingt diese
Skala grob; an den Positionen 1..n traegt derselbe Kanal Werte, die
tausendfach kleiner sind und deshalb auf wenige Integer-Stufen kollabieren.

Beleg aus layer_probe (Ebene 0, 7B): mlp_out-Werte im Bereich -29..-135
bei einem verfuegbaren int16-Bereich von +-32767 — rund 6 von 15 Bit
genutzt. Bei 0,5B stehen dort vierstellige Werte (12-13 Bit).

Diese Simulation quantisiert die ECHTEN HF-Aktivierungen dreifach und
vergleicht den Rekonstruktionsfehler:

    A  eine Skala je Kanal (Fund 20, aktueller Stand)
    B  eine Skala je Kanal, aber Position 0 mit EIGENER Skala
    C  eine Skala je (Kanal, Position) — die theoretische Obergrenze

Ist B nahe C und deutlich besser als A, traegt die These und der Umbau
lohnt. Liegt B nahe A, ist die Positions-Dimension nicht der Hebel.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/positional_scale_simulation.py
"""
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, select_sequences  # noqa: E402

INT16_MAX = 32767
MAX_FRAC_BITS = 20


def quantisiere(werte, shifts):
    """Zweierpotenz-Quantisierung mit gegebenen Shifts, dann Rueckrechnung."""
    import torch
    skala = torch.pow(2.0, shifts)
    q = torch.clamp(torch.round(werte * skala), -INT16_MAX - 1, INT16_MAX)
    return q / skala


def waehle_shifts(absmax):
    """Groesster Shift, der absmax noch traegt (identisch zu scales.py)."""
    import torch
    sicher = absmax.clamp(min=1e-9)
    shifts = torch.floor(torch.log2(INT16_MAX / sicher))
    return shifts.clamp(0, MAX_FRAC_BITS)


def rel_fehler(original, rekonstruiert):
    import torch
    return ((rekonstruiert - original).abs().sum() / original.abs().sum().clamp(min=1e-9)).item()


def main():
    import torch

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(2, seq_len, verbose=False)
    model, _ = load_reference_model(MODEL_DIR)

    gesammelt = {}
    ziel = ["model.layers.4.input_layernorm", "model.layers.12.input_layernorm",
            "model.layers.20.input_layernorm", "model.norm"]

    def mach_hook(name):
        def hook(module, inputs, output):
            x = inputs[0]
            if isinstance(x, torch.Tensor):
                gesammelt.setdefault(name, []).append(
                    x.detach().float().reshape(-1, x.shape[-1]).cpu())
        return hook

    handles = [m.register_forward_hook(mach_hook(n))
               for n, m in model.named_modules() if n in ziel]
    with torch.no_grad():
        for ids in sequences:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()

    print("Relativer Rekonstruktionsfehler der Aktivierungs-Quantisierung")
    print("  A = Skala je Kanal (Fund 20, aktuell)")
    print("  B = wie A, aber Position 0 mit eigener Skala")
    print("  C = Skala je (Kanal, Position) — theoretische Obergrenze")
    print()
    print(f"{'Segment':<38} {'A':>9} {'B':>9} {'C':>9}   {'B/A':>7}")
    for name in ziel:
        if name not in gesammelt:
            continue
        x = torch.cat(gesammelt[name], dim=0)      # [positionen, kanaele]

        # A: eine Skala je Kanal, ueber ALLE Positionen
        shifts_a = waehle_shifts(x.abs().amax(dim=0))
        a = rel_fehler(x, quantisiere(x, shifts_a))

        # B: Position 0 abgetrennt, eigene Skala je Kanal
        rekon_b = torch.empty_like(x)
        rekon_b[:1] = quantisiere(x[:1], waehle_shifts(x[:1].abs().amax(dim=0)))
        rekon_b[1:] = quantisiere(x[1:], waehle_shifts(x[1:].abs().amax(dim=0)))
        b = rel_fehler(x, rekon_b)

        # C: Skala je (Kanal, Position)
        c = rel_fehler(x, quantisiere(x, waehle_shifts(x.abs())))

        print(f"{name:<38} {a:>9.2e} {b:>9.2e} {c:>9.2e}   {b/max(a,1e-30):>7.3f}")


if __name__ == "__main__":
    main()
