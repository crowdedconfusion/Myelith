"""
GPTQ-Quantisierung (Eskalationsstrategie 3, Fahrplan „Eskalationsstrategien").

Round-to-Nearest minimiert den GEWICHTSfehler je Eintrag; der Perplexität
ist aber der AUSGABEFEHLER der Schicht relevant, und der akkumuliert über
24 Ebenen (Fund 14: Bulk-Dimensionen weichen ab Ebene 3 ~25–30 % ab).
GPTQ (Frantar et al. 2022) rundet mit Hessian-gestützter Fehlerkompensation:
wird ein Gewicht quantisiert, wird sein Rundungsfehler auf die noch nicht
quantisierten Gewichte derselben Schicht verteilt, sodass der Ausgabe-
fehler ||X·W − X·Q||² auf dem Kalibrierungskorpus minimal wird.

Determinismus: GPTQ läuft offline in float64 und ist bei identischem
Korpus/Modell deterministisch; das Ergebnis sind dieselben Artefakt-
Formate wie bisher (int8, Per-Channel-Zweierpotenz-Shifts) — der
Integer-Inferenzpfad und sein Determinismus bleiben unberührt.

Angewendet auf die linearen Projektionen (q/k/v/o/gate/up/down_proj).
Embedding/Biases/Gammas bleiben bei quantize_symmetric_int8_per_channel
(RNE), der LM-Head bleibt int16 (benannte spec-Ausnahme).
"""

import numpy as np
import torch

from .quantize import MAX_FRAC_BITS


class HessianCollector:
    """Sammelt H = Σ x·xᵀ über den Eingängen der linearen Projektionen.

    Dieselbe Hook-Konvention wie stats.py::ActivationStatsCollector
    (Schlüssel = HF-Modulnamen). Akkumulation in float32 im RAM
    (24 Ebenen ≈ 2,7 GB), jeder Batch-Beitrag wird in float64 berechnet.
    """

    def __init__(self):
        self.hessians = {}
        self._handles = []

    @staticmethod
    def _proj_keys():
        return ("q_proj", "k_proj", "v_proj", "o_proj",
                "gate_proj", "up_proj", "down_proj")

    def _make_hook(self, name):
        def hook(module, inputs, output):
            x = inputs[0]
            if not isinstance(x, torch.Tensor):
                return
            # Gram-Matrix in float32 auf dem Geraet des Modells (MPS/CPU);
            # float64-Matmul ist auf MPS nicht zuverlaessig. Die laufende
            # Summe wird als float32 gehalten (~2,7 GB RAM fuer alle 168
            # Projektionen), GPTQ selbst rechnet spaeter in float64 weiter.
            xf = x.detach().reshape(-1, x.shape[-1]).float()
            gram = (xf.T @ xf).detach().to("cpu").numpy()
            if name in self.hessians:
                self.hessians[name] += gram
            else:
                self.hessians[name] = gram.astype(np.float32)
        return hook

    def attach(self, model):
        for name, module in model.named_modules():
            if any(name.endswith(k) for k in self._proj_keys()):
                self._handles.append(
                    module.register_forward_hook(self._make_hook(name)))

    def detach(self):
        for h in self._handles:
            h.remove()
        self._handles.clear()


def per_channel_shifts(W: torch.Tensor) -> np.ndarray:
    """Per-Zeile den Zweierpotenz-Shift aus dem AbsMax bestimmen.

    Identisch zur Shift-Wahl von quantize_symmetric_int8_per_channel
    (Single Source of Truth für das Skalen-Raster: der Shift hängt nur
    vom AbsMax der Zeile ab, nicht vom Rundungsverfahren).
    """
    t = W.detach().float().cpu()
    absmax = t.abs().amax(dim=1, keepdim=True)
    shifts = torch.where(
        absmax < 1e-9,
        torch.zeros_like(absmax),
        torch.floor(torch.log2(127.0 / absmax.clamp(min=1e-9))),
    )
    shifts = torch.clamp(shifts, 0, MAX_FRAC_BITS)
    return shifts.squeeze(1).round().to(torch.int8).numpy()


def gptq_quantize(W: torch.Tensor, H: np.ndarray) -> dict:
    """Quantisiert eine Gewichtsmatrix [out, in] mit GPTQ-Fehlerkompensation.

    Liefert dasselbe Format wie quantize_symmetric_int8_per_channel:
    {"int8": [...], "shifts": [...], "shape": [...]}.
    """
    Wf = W.detach().float().cpu().numpy().astype(np.float64)
    out_dim, in_dim = Wf.shape
    shifts = per_channel_shifts(W)                    # [out], int8
    scale = np.power(2.0, shifts.astype(np.float64))  # Multiplikator je Zeile

    Hf = H.astype(np.float64)
    damp = 0.01 * float(np.mean(np.diag(Hf)))
    if damp <= 0.0:
        damp = 1e-8
    idx = np.diag_indices(in_dim)
    Hf[idx] += damp

    # Oberer Cholesky-Faktor U der inversen Hessischen Matrix:
    # H⁻¹ = Uᵀ·U. In dieser Form ist die sequenzielle Fehlerkompensation
    # exakt (GPTQ, Frantar et al. 2022).
    def _cholesky_upper(m):
        L = np.linalg.cholesky(m)
        Linv = np.linalg.inv(L)
        Hinv = Linv.T @ Linv                    # = m⁻¹
        Hinv = (Hinv + Hinv.T) / 2.0            # Symmetrie numerisch sichern
        return np.linalg.cholesky(Hinv).T       # U, oberes Dreieck

    try:
        U = _cholesky_upper(Hf)
    except np.linalg.LinAlgError:
        Hf[idx] += 10.0 * damp
        U = _cholesky_upper(Hf)

    Q = np.zeros((out_dim, in_dim), dtype=np.int8)
    Wwork = Wf.copy()
    for i in range(in_dim):
        w = Wwork[:, i]
        q = np.clip(np.round(w * scale), -128, 127).astype(np.int8)
        Q[:, i] = q
        err = (w - q.astype(np.float64) / scale) / U[i, i]
        # Kompensation: Spalte i wird auf q gesetzt, die Rundungsfehler
        # werden über U[i, i:] auf die Spalten i..Ende verteilt.
        Wwork[:, i:] -= np.outer(err, U[i, i:])
    return {"int8": Q, "shifts": shifts, "shape": [out_dim, in_dim]}


def quantize_linear_layers_gptq(model, hessians: dict) -> dict:
    """Quantisiert alle linearen Projektionen des Modells per GPTQ.

    Returns: Dict[parameter_name -> {int8, shifts, shape}] mit denselben
    Schlüsseln wie model.named_parameters() (z. B.
    "model.layers.0.self_attn.q_proj.weight").
    """
    quantized = {}
    for name, module in model.named_modules():
        if name not in hessians:
            continue
        W = module.weight
        pname = name + ".weight"
        quantized[pname] = gptq_quantize(W, hessians[name])
    return quantized
