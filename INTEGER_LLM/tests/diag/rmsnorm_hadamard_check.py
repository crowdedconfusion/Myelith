#!/usr/bin/env python3
"""
RMSNorm-Gamma-Nichtkommutativität unter Block-Hadamard (NumPy-Vorab-Check).

Frage: Wenn man den Residualstrom mit Block-Hadamard rotiert (x -> Hx), aber
die per-Channel-Layernorm-Gammas NICHT transformiert, wie stark weicht das
Ergebnis von dem ab, was man in der rotierten Basis bräuchte?

Maß: Für jeden Block wird verglichen
  norm_rot   = RMSNorm(H·x, γ)          (rotierter Residual, untransformiertes γ)
  H_norm     = H·RMSNorm(x, γ)          (Referenz: rotierte normalisierte Aktivierung)
und die relative Abweichung  ‖norm_rot − H_norm‖ / ‖H_norm‖  berichtet.

Ist diese Abweichung klein, reicht die vereinfachte Rotation ohne
Gamma-Transformation; ist sie groß, muss der Basiswechsel die Gammas
mit-transformieren (aufwändiger). Zusätzlich wird die Spitze/RMS des
Residuals vor/nach der Rotation berichtet (der eigentliche Nutzen).

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python rmsnorm_hadamard_check.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

ALL_TOKENS = [34532, 425, 10965, 465, 374, 458, 6364, 4531]


def hadamard(n):
    import numpy as np
    H = np.array([[1.0]])
    while H.shape[0] < n:
        H = np.block([[H, H], [H, -H]])
    return H


def block_hadamard_apply(x, Hk, k, normalize):
    d = len(x)
    assert d % k == 0
    Q = Hk / (k ** 0.5) if normalize else Hk
    return (x.reshape(d // k, k) @ Q.T).reshape(d)


def rmsnorm(x, gamma, eps=1e-6):
    import numpy as np
    ms = np.mean(x.astype(np.float64) ** 2)
    return x / np.sqrt(ms + eps) * gamma


def main():
    import numpy as np
    import torch

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    input_ids = torch.tensor([ALL_TOKENS], device=model.device)
    with torch.no_grad():
        out = model(input_ids=input_ids, output_hidden_states=True)
    hs = out.hidden_states  # hs[i+1] = Residual nach Block i

    layers = model.model.layers
    num_layers = len(layers)
    hidden = len(hs[1][0, -1])
    k = 64
    Hk = hadamard(k)

    print(f"hidden={hidden}, Block-Hadamard k={k} (normiert), {num_layers} Blöcke")
    print(f"{'Block':>5} {'Resid peak/rms':>16} {'-> rot peak/rms':>16} "
          f"{'Gamma-Abw. rel.':>16}")
    devs = []
    for i in range(num_layers):
        # Residual, der in Block i hineinfließt = hs[i] (vor Block i).
        x = hs[i][0, -1, :].detach().float().cpu().numpy().astype(np.float64)
        gamma = layers[i].input_layernorm.weight.detach().float().cpu().numpy().astype(np.float64)

        peak0 = np.abs(x).max() / np.sqrt(np.mean(x ** 2))
        xr = block_hadamard_apply(x, Hk, k, normalize=True)
        peak1 = np.abs(xr).max() / np.sqrt(np.mean(xr ** 2))

        norm_ref = rmsnorm(x, gamma)            # RMSNorm(x, γ)
        H_norm_ref = block_hadamard_apply(norm_ref, Hk, k, normalize=True)  # H·RMSNorm(x,γ)
        norm_rot = rmsnorm(xr, gamma)           # RMSNorm(H·x, γ)  [γ untransformiert]
        dev = np.linalg.norm(norm_rot - H_norm_ref) / np.linalg.norm(H_norm_ref)
        devs.append(dev)
        print(f"{i:>5} {peak0:>16.3f} {peak1:>16.3f} {dev:>16.4f}")

    devs = np.array(devs)
    print(f"\nGamma-Abweichung: min={devs.min():.4f} max={devs.max():.4f} "
          f"median={np.median(devs):.4f}")
    print("Interpretation: klein (<~0.05) -> vereinfachte Rotation ohne "
          "Gamma-Transformation tragfähig; groß -> Basiswechsel muss Gammas "
          "mit-transformieren.")


if __name__ == "__main__":
    main()
