#!/usr/bin/env python3
"""test_gammaentzerrung.py: Gegenproben zu Fund 336.

⚑ **Ohne Modell und ohne torch-Gewichte von der Platte.** Die Proben
bauen kleine Tensoren von Hand; was hier zur Pruefung steht, ist die
Rechnung und nicht das Modell.

⚠️ **Der Lauf gegen das echte Qwen3-0,6B ist ein anderer.** Er laedt
das Gleitkommamodell, entzerrt es und vergleicht die Logits vorher und
nachher; er braucht torch und Minuten und gehoert deshalb nicht in
diese Sammlung.

📌 **Fund 342: Diese Datei lief bis zum 2026-09-11 nirgends.** Sie war
mit `pytest` geschrieben, und dieses Projekt hat kein pytest: weder in
`calibrate/requirements.txt` noch in der CI, und die uebrigen
Testdateien hier sind eigenstaendige Skripte (siehe
`test_export_workflow.py`). Der Aufruf brach also mit
`ModuleNotFoundError` ab, und in der CI stand sie in keiner Liste.
**Eine Pruefung, die nicht laeuft, meldet nichts**, dieselbe Klasse wie
Fund 44. Jetzt eigenstaendiges Skript, und im CI-Job neben den drei
anderen, die torch brauchen.
"""

import sys
from pathlib import Path

import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from calibrate.src.gammaentzerrung import INT8_GRENZE, entzerren  # noqa: E402


def _ebene(gamma, spalten=4, zeilen=3):
    """Eine Ebene mit Norm, Aufmerksamkeit und MLP, alles von Hand."""
    g = torch.tensor(gamma, dtype=torch.float32)
    mach = lambda s, z: torch.arange(1, z * s + 1, dtype=torch.float32).reshape(z, s) / 10
    return {
        "model.layers.0.post_attention_layernorm.weight": g,
        "model.layers.0.mlp.gate_proj.weight": mach(spalten, zeilen),
        "model.layers.0.mlp.up_proj.weight": mach(spalten, zeilen) + 1,
    }


def test_ohne_ausreisser_wird_nichts_angefasst():
    """**Der Normalfall ist, dass nichts passiert.**

    ⚑ Von den bisher gebauten Modellen brauchte keines die Umformung.
    Eine Entzerrung, die auch ohne Not zugreift, waere eine zweite
    Quantisierungsentscheidung.
    """
    z = _ebene([1.0, 2.0, 127.0, -127.0])
    vorher = {k: v.clone() for k, v in z.items()}
    assert entzerren(z) == []
    for k in vorher:
        assert torch.equal(z[k], vorher[k]), f"{k} wurde ohne Grund veraendert"


def test_der_ausreisser_wandert_und_das_produkt_bleibt():
    """**Die eigentliche Zusage: `W @ (normiert * gamma)` bleibt gleich.**

    📌 Das ist die Probe, an der die ganze Umformung haengt. Sie rechnet
    das Produkt vor und nach der Entzerrung aus und verlangt
    **Bitgleichheit**, nicht Naehe.
    """
    z = _ebene([1.0, 192.0, 3.0, -300.0])
    normiert = torch.tensor([0.5, -0.25, 2.0, 0.125], dtype=torch.float32)

    def produkt(zustand):
        y = normiert * zustand["model.layers.0.post_attention_layernorm.weight"]
        return (
            zustand["model.layers.0.mlp.gate_proj.weight"] @ y,
            zustand["model.layers.0.mlp.up_proj.weight"] @ y,
        )

    vorher = produkt(z)
    verschoben = entzerren(z)

    # Zwei Kanaele ueber der Grenze, beide um die kleinste Zweierpotenz.
    assert [(n, k) for n, k, _, _ in verschoben] == [
        ("model.layers.0.post_attention_layernorm.weight", 1),
        ("model.layers.0.post_attention_layernorm.weight", 3),
    ]
    gamma = z["model.layers.0.post_attention_layernorm.weight"]
    assert gamma[1].item() == 96.0, "192 braucht genau einen Halbierungsschritt"
    assert gamma[3].item() == -75.0, "300 braucht zwei"
    assert float(gamma.abs().max()) <= INT8_GRENZE

    nachher = produkt(z)
    for a, b in zip(vorher, nachher):
        assert torch.equal(a, b), "die Umformung ist keine Identitaet"


def test_unveraenderte_kanaele_bleiben_unveraendert():
    """**Nur der Ausreisser wandert, nicht der ganze Tensor.**

    ⚠️ Ein Verfahren, das alle Kanaele glattzieht, waere etwas anderes
    als dieses hier und braeuchte eine eigene Messung.
    """
    z = _ebene([1.0, 192.0, 3.0, 4.0])
    vorher_gate = z["model.layers.0.mlp.gate_proj.weight"].clone()
    entzerren(z)
    nachher_gate = z["model.layers.0.mlp.gate_proj.weight"]
    for s in (0, 2, 3):
        assert torch.equal(nachher_gate[:, s], vorher_gate[:, s]), f"Spalte {s} angefasst"
    assert torch.equal(nachher_gate[:, 1], vorher_gate[:, 1] * 2)


def test_ohne_bekannte_abnehmer_bricht_es_ab():
    """**Lieber ein lauter Abbruch als ein stilles Saettigen.**

    📌 Dieselbe Lehre wie Fund 23: Ein Quantisierer, der heimlich
    abschneidet, erzeugt ein Artefakt, das monatelang wie
    Quantisierungsrauschen aussieht.
    """
    z = {"model.layers.0.self_attn.q_norm.weight": torch.tensor([200.0, 1.0])}
    try:
        entzerren(z)
    except ValueError as e:
        assert "keine Abnehmer bekannt" in str(e), f"falsche Meldung: {e}"
    else:
        raise AssertionError("ein unbekannter Abnehmer haette abbrechen muessen")


def test_beim_gemisch_zaehlt_auch_der_router():
    """**Ein Expertengemisch liest den normierten Vektor mehrfach.**

    ⚠️ Wer den Router vergaesse, entzerrte die Experten und veraenderte
    zugleich das Routing. Dann waere die Umformung keine Identitaet
    mehr, und zwar auf eine Weise, die keine Zahl sofort zeigt.
    """
    g = torch.tensor([1.0, 256.0], dtype=torch.float32)
    z = {
        "model.layers.3.post_attention_layernorm.weight": g,
        "model.layers.3.mlp.gate.weight": torch.ones(2, 2),
        "model.layers.3.mlp.experts.0.gate_proj.weight": torch.ones(2, 2),
        "model.layers.3.mlp.experts.0.up_proj.weight": torch.ones(2, 2),
        "model.layers.3.mlp.experts.1.gate_proj.weight": torch.ones(2, 2),
        "model.layers.3.mlp.experts.1.up_proj.weight": torch.ones(2, 2),
    }
    entzerren(z)
    # ⚑ **Zwei Schritte, nicht einer:** 256 halbiert waere 128 und damit
    # immer noch ueber der Grenze von 127.
    assert z["model.layers.3.post_attention_layernorm.weight"][1].item() == 64.0
    for n in (
        "model.layers.3.mlp.gate.weight",
        "model.layers.3.mlp.experts.0.gate_proj.weight",
        "model.layers.3.mlp.experts.1.up_proj.weight",
    ):
        assert z[n][:, 1].tolist() == [4.0, 4.0], f"{n} wurde nicht ausgeglichen"
        assert z[n][:, 0].tolist() == [1.0, 1.0], f"{n}: falsche Spalte angefasst"


if __name__ == "__main__":
    test_ohne_ausreisser_wird_nichts_angefasst()
    print("[test] ohne Ausreisser wird nichts angefasst: PASSED")
    test_der_ausreisser_wandert_und_das_produkt_bleibt()
    print("[test] der Ausreisser wandert, das Produkt bleibt: PASSED")
    test_unveraenderte_kanaele_bleiben_unveraendert()
    print("[test] unveraenderte Kanaele bleiben unveraendert: PASSED")
    test_ohne_bekannte_abnehmer_bricht_es_ab()
    print("[test] ohne bekannte Abnehmer bricht es ab: PASSED")
    test_beim_gemisch_zaehlt_auch_der_router()
    print("[test] beim Gemisch zaehlt auch der Router: PASSED")
    print("[test] Alle Tests bestanden.")
