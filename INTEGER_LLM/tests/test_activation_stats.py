#!/usr/bin/env python3
"""
Unit-Tests fuer calibrate/src/stats.py::ActivationStatsCollector.

Fund 20 (2026-08-18): Der Collector sammelt seit theta_v 0.11.0 zusaetzlich
ein Per-Kanal-AbsMax fuer die drei Residualstrom-Segmente
(*_layernorm.input, model.norm.input) — ausgeloest durch Qwen2.5-7B, wo ein
einzelner Kanal an Position 0 einen absmax von ~9600 traegt, waehrend der
Rest bei ~10 liegt ("Massive Activations", Sun et al. 2024). Eine globale
(skalare) Statistik verwischt das; diese Tests pruefen die Per-Kanal-
Sammlung isoliert, ohne ein echtes HF-Modell zu brauchen.

Kein pytest-Bedarf, eigenstaendiges Skript nach Projektkonvention.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))

import torch
import torch.nn as nn

from src.stats import ActivationStatsCollector


def _outlier_module_and_input():
    """
    Ein LayerNorm-Modul mit vier Kanaelen, dessen EINGANG (nicht Ausgang!)
    per Hook beobachtet wird - genau wie *_layernorm.input in der echten
    Kalibrierung. Kanal 0 traegt an einer Sequenzposition einen Ausreisser.
    """
    mod = nn.LayerNorm(4)
    # [batch=1, seq=2, hidden=4]: Position 0 hat den Ausreisser in Kanal 0,
    # Position 1 ist unauffaellig - realistisches Bild fuer Massive
    # Activations (an EINER Position, nicht ueberall).
    x = torch.tensor([[[9600.0, 0.25, -0.5, 0.75],
                        [3.0, 1.0, -1.0, 2.0]]])
    return mod, x


def test_channel_absmax_isolates_outlier_channel():
    mod, x = _outlier_module_and_input()
    collector = ActivationStatsCollector()
    handle = mod.register_forward_hook(
        collector._make_hook("test.input", take_input=True, per_channel=True))
    mod(x)
    handle.remove()

    stats = collector.compute()
    ch = stats["test.input"]["channel_absmax"]
    assert len(ch) == 4
    assert ch[0] == 9600.0, f"Kanal 0 (Ausreisser) = {ch[0]}"
    assert ch[1] == 1.0, f"Kanal 1 (max von 0.25, 1.0) = {ch[1]}"
    assert ch[2] == 1.0, f"Kanal 2 (max von 0.5, 1.0) = {ch[2]}"
    assert ch[3] == 2.0, f"Kanal 3 (max von 0.75, 2.0) = {ch[3]}"

    # Die globale (flache) Statistik bleibt unveraendert vom Ausreisser
    # dominiert - genau das Problem, das Fund 20 fuer den Rest der Kanaele
    # loest.
    assert stats["test.input"]["absmax"] == 9600.0


def test_channel_absmax_accumulates_across_forward_calls():
    # Zwei Kalibrierungs-Batches nacheinander (wie mehrere Prompts) muessen
    # sich zu einem laufenden Maximum je Kanal akkumulieren, nicht den
    # vorherigen Batch ueberschreiben.
    mod = nn.LayerNorm(2)
    collector = ActivationStatsCollector()
    handle = mod.register_forward_hook(
        collector._make_hook("test.input", take_input=True, per_channel=True))

    mod(torch.tensor([[[5.0, 100.0]]]))
    mod(torch.tensor([[[50.0, 1.0]]]))
    handle.remove()

    ch = collector.compute()["test.input"]["channel_absmax"]
    assert ch[0] == 50.0, f"Kanal 0: max(5, 50) = {ch[0]}"
    assert ch[1] == 100.0, f"Kanal 1: max(100, 1) = {ch[1]}"


def test_non_per_channel_hook_has_no_channel_absmax():
    # Reguläre Hooks (proj-Ausgaenge, Norm-Ausgaenge) rufen _make_hook OHNE
    # per_channel=True auf - ihr Eintrag darf kein "channel_absmax" tragen
    # (compute_scales_from_stats erkennt daran, dass NICHT per-Kanal
    # kalibriert werden soll).
    mod, x = _outlier_module_and_input()
    collector = ActivationStatsCollector()
    handle = mod.register_forward_hook(collector._make_hook("test.output"))
    mod(x)
    handle.remove()

    stats = collector.compute()
    assert "channel_absmax" not in stats["test.output"]


if __name__ == "__main__":
    test_channel_absmax_isolates_outlier_channel()
    print("[test] Per-Kanal-AbsMax isoliert den Ausreisser-Kanal: PASSED")
    test_channel_absmax_accumulates_across_forward_calls()
    print("[test] Per-Kanal-AbsMax akkumuliert ueber mehrere Forward-Aufrufe: PASSED")
    test_non_per_channel_hook_has_no_channel_absmax()
    print("[test] Reguläre Hooks bleiben ohne channel_absmax: PASSED")
    print("Alle Tests bestanden.")
