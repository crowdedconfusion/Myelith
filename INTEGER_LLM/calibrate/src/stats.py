"""
Sammelt Aktivierungsstatistiken (Min, Max, p99, AbsMean) per Forward-Hook.

Gehookt werden (Konvention fuer die Schluessel in scales.json — die Runtime
verwendet exakt dieselben Namen, siehe runtime/src/loader.rs):
- Ausgaenge aller Projektionen (q/k/v/o/gate/up/down_proj) -> deren
  Ausgangsskalen,
- Ausgaenge der RMSNorm-Module (input_layernorm, post_attention_layernorm,
  model.norm) -> Eingangsskalen der Folgeprojektionen bzw. des LM-Heads,
- Ausgang des self_attn-Moduls -> Eingangsskala von o_proj,
- EINGANG von down_proj (h = silu(gate)*up) -> Eingangsskala von down_proj.

**Fund 20 (2026-08-18): Per-Kanal-Statistik fuer die drei Residualstrom-
Segmente** (`*_layernorm.input`, `model.norm.input`). Qwen2.5-7B zeigt an
Position 0 "Massive Activations" (Sun et al. 2024) in wenigen festen
Kanaelen (absmax ~9600 gegenueber ~10 im Rest) — eine globale (skalare)
Statistik verwischt das voellig. Diese drei Hooks sammeln deshalb
ZUSAETZLICH ein Per-Kanal-AbsMax (letzte Dimension = hidden_size), aus dem
`scales.py` je Kanal einen eigenen Shift ableitet. Alle anderen Hooks
bleiben skalar — nur der Residualstrom selbst zeigt den Ausreisser (siehe
`INTEGER_LLM/kernels/src/rmsnorm.rs`-Modulkopf).
"""

import torch
from collections import defaultdict

# Projektkonvention: p99 wird aus einer begrenzten Stichprobe geschaetzt;
# absmax/absmean laufen inkrementell ueber alle Werte mit.
_MAX_SAMPLES = 20_000
_CHUNK = 10_000


class ActivationStatsCollector:
    def __init__(self):
        self.stats = defaultdict(lambda: {
            "min": float("inf"),
            "max": float("-inf"),
            "abs_sum": 0.0,
            "count": 0,
            "values": [],
            "channel_absmax": None,  # Fund 20: nur fuer Residualstrom-Segmente befuellt
        })
        self._handles = []

    def _make_hook(self, name, take_input=False, per_channel=False):
        def hook(module, input, output):
            t = input[0] if take_input else output
            if isinstance(t, tuple):
                t = t[0]
            if isinstance(t, torch.Tensor):
                t_det = t.detach().float().cpu()
                vals = t_det.flatten()
                s = self.stats[name]
                s["min"] = min(s["min"], vals.min().item())
                s["max"] = max(s["max"], vals.max().item())
                s["abs_sum"] += vals.abs().sum().item()
                s["count"] += vals.numel()
                if len(s["values"]) < _MAX_SAMPLES:
                    s["values"].extend(vals.tolist()[:_CHUNK])
                if per_channel:
                    # Letzte Dimension = hidden_size (Kanal); alle anderen
                    # (Batch, Sequenzposition) werden fuers Kanal-AbsMax
                    # ueber Forward-Aufrufe UND Positionen hinweg reduziert -
                    # der Ausreisser an Position 0 muss die Skala genauso
                    # bestimmen wie im echten Inferenzpfad.
                    ch_absmax = t_det.reshape(-1, t_det.shape[-1]).abs().amax(dim=0)
                    if s["channel_absmax"] is None:
                        s["channel_absmax"] = ch_absmax
                    else:
                        s["channel_absmax"] = torch.maximum(s["channel_absmax"], ch_absmax)
        return hook

    def attach(self, model):
        proj_keys = ("q_proj", "k_proj", "v_proj", "o_proj",
                     "gate_proj", "up_proj", "down_proj")
        norm_keys = ("input_layernorm", "post_attention_layernorm")
        for name, module in model.named_modules():
            if any(name.endswith(k) for k in proj_keys):
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
                if name.endswith("down_proj"):
                    # h = silu(gate)*up liegt nur am down_proj-Eingang an.
                    h_in = module.register_forward_hook(
                        self._make_hook(name + ".input", take_input=True))
                    self._handles.append(h_in)
            elif any(name.endswith(k) for k in norm_keys) or name == "model.norm":
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
                # Per-Segment-Skalen des Residualstroms (v0.12.21/spec 0.5.1):
                # Die Norm-EINGAENGE sind die Residual-Stromsegmente. Die
                # Spanne reicht von winzigen Embedding-Werten (~±0,2) bis zu
                # Ausreisser-Spitzen (~±1576, bei 7B ~±9600) — eine globale
                # Skala kann das nicht abdecken. Seit Fund 20 zusaetzlich
                # per_channel=True: eine Skala je Kanal statt je Segment.
                h_in = module.register_forward_hook(
                    self._make_hook(name + ".input", take_input=True, per_channel=True))
                self._handles.append(h_in)
            elif name.endswith(".self_attn"):
                # Modul-Ausgabe ist ein Tupel; der Hook nimmt Element 0.
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)

    def detach(self):
        for h in self._handles:
            h.remove()
        self._handles.clear()

    def compute(self):
        result = {}
        for name, s in self.stats.items():
            values = torch.tensor(s["values"])
            entry = {
                "min": s["min"],
                "max": s["max"],
                "absmean": s["abs_sum"] / max(s["count"], 1),
                "absmax": max(abs(s["min"]), abs(s["max"])),
                "p99": torch.quantile(values.abs(), 0.99).item() if len(values) else 0.0,
            }
            if s["channel_absmax"] is not None:
                entry["channel_absmax"] = s["channel_absmax"].tolist()
            result[name] = entry
        return result
