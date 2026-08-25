#!/usr/bin/env python3
"""
Unit-Tests fuer calibrate/src/quantize.py::entschmelze_experten.

## Wogegen diese Tests geschrieben sind

Die `model.safetensors.index.json` von Qwen3-30B-A3B fuehrt 128 Experten
je Layer als je drei eigene, zweidimensionale Tensoren.
**transformers 5.15 laedt sie nicht so**, sondern gestapelt:

    model.layers.0.mlp.experts.gate_up_proj   [128, 1536, 2048]
    model.layers.0.mlp.experts.down_proj      [128, 2048,  768]

Ohne Entschmelzen bekaeme `quantize_symmetric_int8_per_channel` einen
dreidimensionalen Tensor. **Sie stuerzt dabei nicht ab**, sondern reduziert
`absmax` ueber die Achsen 1 und 2 und liefert eine Skala je *Experte*
statt je *Zeile* - also genau die Per-Tensor-Skala, die theta_v 0.7.0
abgeschafft hat, weil sie 10 bis 17 Prozent der Eintraege zerstoerte.
Stiller Qualitaetsverlust, kein Fehler.

`test_dreidimensional_ergaebe_eine_skala_je_experte` haelt genau das
fest: Es misst, dass die naive Behandlung wenige Skalen liefert und die
entschmolzene viele. Ohne diese Gegenprobe pruefte der Rest nur, dass
irgendetwas geschnitten wird.

## Was diese Tests nicht koennen

Sie pruefen die **Form und die Namen**, nicht die Anordnung gegen das
echte Modell. Dass die obere Haelfte von `gate_up_proj` das `gate_proj`
und die untere das `up_proj` ist, wurde am 2026-08-25 gegen die echten
Tensoren auf der Platte gemessen:

    gate_up_proj[0][:768]  == experts.0.gate_proj.weight    True
    gate_up_proj[0][768:]  == experts.0.up_proj.weight      True
    down_proj[0]           == experts.0.down_proj.weight    True

Diese Messung braucht 57 GiB Gewichte und laeuft deshalb nicht hier,
sondern steht als Beleg im Fahrplan.

Kein pytest-Bedarf, eigenstaendiges Skript nach Projektkonvention.
"""

import sys
from pathlib import Path

import torch

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.quantize import entschmelze_experten, quantize_symmetric_int8_per_channel


def _verschmolzen(experten, breite, hidden):
    """Ein gestapelter gate_up_proj-Tensor mit unterscheidbaren Werten.

    Die Betraege liegen unter eins, wie bei echten Gewichten. Das ist
    hier keine Kosmetik: `quantize_symmetric_int8_per_channel` weist
    Tensoren mit Betraegen ueber 127 ausdruecklich zurueck, weil die
    Zweierpotenz-Skala nicht unter shift 0 gehen kann. Mit `arange`
    schlaegt schon der Aufruf fehl, und die Gegenprobe unten koennte gar
    nicht zeigen, dass der naive Pfad **still** schlechter wird.
    """
    gesamt = experten * 2 * breite * hidden
    werte = torch.arange(gesamt, dtype=torch.float32) / gesamt
    return werte.reshape(experten, 2 * breite, hidden)


def test_gate_up_wird_in_zwei_haelften_geteilt():
    e, b, h = 3, 4, 5
    t = _verschmolzen(e, b, h)
    stuecke = dict(entschmelze_experten("model.layers.0.mlp.experts.gate_up_proj", t))

    assert len(stuecke) == 2 * e, f"erwartet {2 * e} Tensoren, waren {len(stuecke)}"
    for i in range(e):
        g = stuecke[f"model.layers.0.mlp.experts.{i}.gate_proj.weight"]
        u = stuecke[f"model.layers.0.mlp.experts.{i}.up_proj.weight"]
        assert tuple(g.shape) == (b, h)
        assert tuple(u.shape) == (b, h)
        # Obere Haelfte ist gate, untere ist up - die Reihenfolge, die
        # gegen die Platte gemessen wurde.
        assert torch.equal(g, t[i][:b])
        assert torch.equal(u, t[i][b:])


def test_down_proj_bleibt_ungeschnitten_je_experte():
    e, h, b = 3, 5, 4
    t = (torch.arange(e * h * b, dtype=torch.float32) / (e * h * b)).reshape(e, h, b)
    stuecke = dict(entschmelze_experten("model.layers.7.mlp.experts.down_proj", t))

    assert len(stuecke) == e
    for i in range(e):
        d = stuecke[f"model.layers.7.mlp.experts.{i}.down_proj.weight"]
        assert tuple(d.shape) == (h, b)
        assert torch.equal(d, t[i])


def test_die_layernummer_bleibt_erhalten():
    """Der Praefix darf nicht verlorengehen: Sonst truege jede Layer
    dieselben Expertennamen und der Export ueberschriebe sich selbst."""
    t = _verschmolzen(2, 2, 2)
    for layer in (0, 5, 47):
        namen = [n for n, _ in entschmelze_experten(
            f"model.layers.{layer}.mlp.experts.gate_up_proj", t)]
        assert all(n.startswith(f"model.layers.{layer}.mlp.experts.") for n in namen), namen


def test_dichte_und_zweidimensionale_tensoren_gehen_unveraendert_durch():
    w = torch.zeros(5, 3)
    for name in ("model.layers.0.mlp.gate_proj.weight",
                 "model.layers.0.mlp.gate.weight",
                 "model.layers.0.self_attn.q_proj.weight",
                 "model.embed_tokens.weight"):
        r = entschmelze_experten(name, w)
        assert r == [(name, w)] or (len(r) == 1 and r[0][0] == name), r


def test_zweidimensionale_expertentensoren_bleiben_unberuehrt():
    """Eine transformers-Fassung mit einzelnen Expertenmodulen liefert
    hier schon 2D unter den richtigen Namen. Dann darf nichts geschehen."""
    w = torch.zeros(4, 6)
    name = "model.layers.0.mlp.experts.3.gate_proj.weight"
    r = entschmelze_experten(name, w)
    assert len(r) == 1 and r[0][0] == name


def test_ungerade_gate_up_achse_wird_abgelehnt():
    """gate und up sind verkettet, die Achse muss also gerade sein.
    Eine ungerade waere ein Formatwechsel, den niemand bemerkt haette."""
    t = torch.zeros(2, 5, 3)
    try:
        entschmelze_experten("model.layers.0.mlp.experts.gate_up_proj", t)
    except ValueError as e:
        assert "teilbar" in str(e), str(e)
    else:
        raise AssertionError("ungerade Achse 1 muss abgelehnt werden")


def test_dreidimensional_ergaebe_eine_skala_je_experte():
    """**Die Gegenprobe.** Ohne Entschmelzen liefert der Quantisierer
    eine Skala je Experte statt je Zeile."""
    e, b, h = 4, 6, 5
    t = _verschmolzen(e, b, h)

    naiv = quantize_symmetric_int8_per_channel(t)
    assert len(naiv["shifts"]) == e, (
        f"naiv erwartet {e} Skalen (eine je Experte), waren {len(naiv['shifts'])}"
    )

    entschmolzen = entschmelze_experten("model.layers.0.mlp.experts.gate_up_proj", t)
    skalen = sum(len(quantize_symmetric_int8_per_channel(x)["shifts"])
                 for _, x in entschmolzen)
    assert skalen == 2 * e * b, f"entschmolzen erwartet {2 * e * b} Skalen, waren {skalen}"
    assert skalen > len(naiv["shifts"]) * 10, (
        "der Unterschied muss deutlich sein, sonst misst dieser Test nichts"
    )


if __name__ == "__main__":
    test_gate_up_wird_in_zwei_haelften_geteilt()
    print("[test] gate_up wird in zwei Haelften geteilt: PASSED")
    test_down_proj_bleibt_ungeschnitten_je_experte()
    print("[test] down_proj je Experte, ungeschnitten: PASSED")
    test_die_layernummer_bleibt_erhalten()
    print("[test] Layernummer bleibt im Namen: PASSED")
    test_dichte_und_zweidimensionale_tensoren_gehen_unveraendert_durch()
    print("[test] dichte Tensoren gehen unveraendert durch: PASSED")
    test_zweidimensionale_expertentensoren_bleiben_unberuehrt()
    print("[test] 2D-Expertentensoren bleiben unberuehrt: PASSED")
    test_ungerade_gate_up_achse_wird_abgelehnt()
    print("[test] ungerade gate_up-Achse wird abgelehnt: PASSED")
    test_dreidimensional_ergaebe_eine_skala_je_experte()
    print("[test] Gegenprobe: 3D ergaebe eine Skala je Experte: PASSED")
    print("Alle Tests bestanden.")
