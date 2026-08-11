#!/usr/bin/env python3
"""
Tests für calibrate/src/gptq.py (Eskalationsstrategie 3, theta_v 0.8.0).

Kernproperty: GPTQ minimiert den AUSGABEFEHLER ||X·W − X·Q||² auf dem
Kalibrierungskorpus — es muss strikt besser sein als Round-to-nearest-even
mit denselben Per-Channel-Zweierpotenz-Shifts (Fund 14: das akkumulierte
Quantisierungsrauschen war die dominante Fehlerquelle).

Kein pytest-Bedarf, eigenständiges Skript nach Projektkonvention.
Braucht numpy + torch (calibrate-Abhängigkeiten).
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))

try:
    import numpy as np
    import torch
    HAS_DEPS = True
except ImportError:
    HAS_DEPS = False

from src.gptq import gptq_quantize, per_channel_shifts  # noqa: E402


def _dequantize(res):
    return res["int8"].astype(np.float64) * np.power(
        2.0, -res["shifts"].astype(np.float64))[:, None]


def _naive_rne(W_float64, shifts):
    scale = np.power(2.0, shifts.astype(np.float64))
    return np.clip(np.round(W_float64 * scale[:, None]), -128, 127) / scale[:, None]


def test_gptq_reduces_output_error():
    rng = np.random.default_rng(42)
    out_dim, in_dim, n_samples = 32, 64, 2000
    # Korrelierte Inputs wie bei echten Aktivierungen (kein weißes Rauschen).
    A = rng.normal(size=(in_dim, in_dim))
    cov = A @ A.T / in_dim
    X = rng.multivariate_normal(np.zeros(in_dim), cov, size=n_samples)
    W = torch.tensor(rng.normal(size=(out_dim, in_dim)) * 0.02, dtype=torch.float32)

    H = (X.T @ X).astype(np.float32)
    res = gptq_quantize(W, H)
    Q = _dequantize(res)
    Qnaive = _naive_rne(W.numpy().astype(np.float64), res["shifts"])

    Wf = W.numpy().astype(np.float64)
    err_gptq = float(np.sum((X @ Wf.T - X @ Q.T) ** 2))
    err_naive = float(np.sum((X @ Wf.T - X @ Qnaive.T) ** 2))
    assert err_gptq < err_naive, (
        f"GPTQ ({err_gptq:.4f}) muss den Ausgabefehler gegenueber "
        f"RNE ({err_naive:.4f}) reduzieren")


def test_gptq_output_format_matches_per_channel():
    # Dasselbe Artefakt-Format wie quantize_symmetric_int8_per_channel:
    # int8-Daten, ein Shift je Zeile, shape unverändert.
    rng = np.random.default_rng(7)
    W = torch.tensor(rng.normal(size=(5, 9)) * 0.05, dtype=torch.float32)
    H = np.eye(9, dtype=np.float32) * 10.0
    res = gptq_quantize(W, H)
    assert res["int8"].dtype == np.int8
    assert res["int8"].shape == (5, 9)
    assert res["shifts"].shape == (5,)
    assert res["shape"] == [5, 9]
    # GPTQ kann Einzelgewichte weiter als RNE verschieben (Kompensation),
    # aber nie außerhalb des int8-Rasters derselben Zeile.
    assert np.all(np.abs(res["int8"]) <= 127)


def test_gptq_shifts_identical_to_per_channel_choice():
    # Die Shift-Wahl ist identisch zu quantize_symmetric_int8_per_channel
    # (nur das Rundungsverfahren unterscheidet sich).
    rng = np.random.default_rng(13)
    W = torch.tensor(rng.normal(size=(6, 8)) * 0.1, dtype=torch.float32)
    from src.quantize import quantize_symmetric_int8_per_channel
    shifts_gptq = per_channel_shifts(W)
    shifts_rne = quantize_symmetric_int8_per_channel(W)["shifts"]
    assert np.array_equal(shifts_gptq, shifts_rne)


def test_gptq_diagonal_hessian_is_rne():
    # Bei diagonaler Hessischer (unkorrelierte Inputs) gibt es keine
    # Kompensationsmöglichkeit über die Spalten hinweg; GPTQ darf dann
    # nicht schlechter werden als RNE.
    rng = np.random.default_rng(99)
    W = torch.tensor(rng.normal(size=(4, 6)) * 0.05, dtype=torch.float32)
    H = np.diag(rng.uniform(1.0, 5.0, size=6)).astype(np.float32)
    res = gptq_quantize(W, H)
    Q = _dequantize(res)
    Qnaive = _naive_rne(W.numpy().astype(np.float64), res["shifts"])
    Wf = W.numpy().astype(np.float64)
    X = np.eye(6)
    err_gptq = float(np.sum((X @ Wf.T - X @ Q.T) ** 2))
    err_naive = float(np.sum((X @ Wf.T - X @ Qnaive.T) ** 2))
    assert err_gptq <= err_naive + 1e-12


if __name__ == "__main__":
    if not HAS_DEPS:
        print("[test] SKIPPED (numpy/torch fehlt): gptq")
        sys.exit(0)
    test_gptq_reduces_output_error()
    print("[test] GPTQ reduziert den Ausgabefehler vs. RNE: PASSED")
    test_gptq_output_format_matches_per_channel()
    print("[test] GPTQ-Artefaktformat identisch zu Per-Channel: PASSED")
    test_gptq_shifts_identical_to_per_channel_choice()
    print("[test] GPTQ-Shiftwahl identisch zu Per-Channel: PASSED")
    test_gptq_diagonal_hessian_is_rne()
    print("[test] GPTQ bei diagonaler Hessischer nicht schlechter als RNE: PASSED")
    print("Alle Tests bestanden.")
