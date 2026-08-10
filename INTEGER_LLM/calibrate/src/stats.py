"""
Sammelt Aktivierungsstatistiken (Min, Max, p99, AbsMean) per Forward-Hook.
"""

import torch
from collections import defaultdict


class ActivationStatsCollector:
    def __init__(self):
        self.stats = defaultdict(lambda: {
            "min": float("inf"),
            "max": float("-inf"),
            "abs_sum": 0.0,
            "count": 0,
            "values": [],
        })
        self._handles = []

    def _make_hook(self, name):
        def hook(module, input, output):
            if isinstance(output, torch.Tensor):
                vals = output.detach().float().cpu().flatten()
                s = self.stats[name]
                s["min"] = min(s["min"], vals.min().item())
                s["max"] = max(s["max"], vals.max().item())
                s["abs_sum"] += vals.abs().sum().item()
                s["count"] += vals.numel()
                if len(s["values"]) < 100_000:
                    s["values"].extend(vals.tolist()[:10_000])
        return hook

    def attach(self, model):
        for name, module in model.named_modules():
            if any(key in name for key in ["q_proj", "k_proj", "v_proj", "o_proj",
                                            "gate_proj", "up_proj", "down_proj"]):
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
            result[name] = {
                "min": s["min"],
                "max": s["max"],
                "absmean": s["abs_sum"] / max(s["count"], 1),
                "absmax": max(abs(s["min"]), abs(s["max"])),
                "p99": torch.quantile(values.abs(), 0.99).item() if len(values) else 0.0,
            }
        return result
