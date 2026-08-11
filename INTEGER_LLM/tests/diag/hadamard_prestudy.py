#!/usr/bin/env python3
"""
Block-Hadamard-Vorstudie (NumPy, vor jeder Rust-Zeile).

Nimmt echte Residualvektoren aus dem HF-Referenzmodell und wendet
Block-Hadamard für k ∈ {16, 64, 128} an — sowohl UNNORMIERT (H nur ±1,
der Gesamtfaktor k würde später als ein Shift abgeräumt) als auch NORMIERT
(Q = H/√k, orthonormal). Gemessen wird, ob die Spitze (max |x|) relativ zum
Bulk (RMS) kleiner wird — denn nur dann lohnt sich Hadamard für die
Quantisierung (feinere Skalen).

Beantwortet zwei Fragen empirisch statt theoretisch:
  (a) Bringt Block-Hadamard hier überhaupt eine Spitzen-Glättung?
  (b) Welche Blockgröße k glättet am stärksten?

Gleitkomma ist hier erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python hadamard_prestudy.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

ALL_TOKENS = [34532, 425, 10965, 465, 374, 458, 6364, 4531]


def hadamard(n):
    """Sylvester-Hadamard der Ordnung n (n Zweierpotenz), Einträge ±1."""
    import numpy as np
    H = np.array([[1.0]])
    while H.shape[0] < n:
        H = np.block([[H, H], [H, -H]])
    return H


def block_hadamard(x, k, normalize):
    """Wendet Hadamard-k blockweise auf x an. normalize=True -> Q=H/√k."""
    import numpy as np
    Hk = hadamard(k)
    if normalize:
        Hk = Hk / (k ** 0.5)
    d = len(x)
    assert d % k == 0, f"Blockgröße {k} teilt {d} nicht"
    X = x.reshape(d // k, k)          # (Anzahl Blöcke, k)
    Y = X @ Hk.T                       # jeder Block wird rotiert
    return Y.reshape(d)


def main():
    import numpy as np
    import torch

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    input_ids = torch.tensor([ALL_TOKENS], device=model.device)
    with torch.no_grad():
        out = model(input_ids=input_ids, output_hidden_states=True)
    hs = out.hidden_states  # hs[i+1] = nach Layer i

    # Untersuche eine Auswahl von Layern an der letzten Position.
    layers = [0, 5, 10, 15, 20, 23]
    ks = [16, 64, 128]

    print("Spitze/RMS-Verhältnis je Layer (hoher Wert = viele Ausreißer-Energie).")
    print("Ein Wert <1 nach Hadamard waere unmöglich; je näher an 1, desto")
    print("gleichmäßiger die Energie. 'unnorm' = H nur ±1, 'norm' = H/√k.\n")

    for li in layers:
        x = hs[li + 1][0, -1, :].detach().float().cpu().numpy().astype(np.float64)
        d = len(x)
        peak0 = np.abs(x).max()
        rms0 = np.sqrt(np.mean(x ** 2))
        line = f"layer {li:2d}  d={d}  vorher: peak={peak0:8.3f} rms={rms0:7.4f} peak/rms={peak0/rms0:6.2f} |"
        for k in ks:
            # unnormiert
            yu = block_hadamard(x, k, normalize=False)
            pu = np.abs(yu).max()
            # normiert
            yn = block_hadamard(x, k, normalize=True)
            pn = np.abs(yn).max()
            rn = np.sqrt(np.mean(yn ** 2))
            line += f"  k={k:3d}: unnorm peak={pu:8.3f}  norm peak/rms={pn/rn:6.2f}"
        print(line)


if __name__ == "__main__":
    main()
