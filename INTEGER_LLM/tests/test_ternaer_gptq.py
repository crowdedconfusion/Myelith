#!/usr/bin/env python3
"""Proben fuer calibrate/src/ternaer_gptq.py (eigenstaendig, kein pytest).

Aufruf aus der Wurzel:  $V INTEGER_LLM/tests/test_ternaer_gptq.py
"""

import sys
from pathlib import Path

import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "calibrate" / "src"))
import ternaer_gptq as tg  # noqa: E402


def zufall(saat: int):
    g = torch.Generator().manual_seed(saat)
    return g


def gewichte(E=2, zeilen=8, spalten=256, saat=1):
    g = zufall(saat)
    return torch.round(torch.randn(E, zeilen, spalten, generator=g) * 30).clamp(-127, 127)


def test_jede_gruppe_ist_ternaer():
    W = gewichte()
    k = tg.raster_bits(W)
    g = zufall(2)
    X = torch.randn(4, 512, 256, generator=g)[:2]
    H = torch.stack([x.double().T @ x.double() for x in X])
    Q = tg.ternaer_gptq(W, H, k)
    assert float(Q.abs().max()) <= 127, "ein Betrag passt nicht in int8"
    for gruppe in Q.reshape(-1, tg.GRUPPE):
        betraege = {abs(int(v)) for v in gruppe if v != 0}
        assert len(betraege) <= 1, f"eine Gruppe traegt mehrere Betraege: {betraege}"
    print("ok jede Gruppe ist ternaer")


def test_ohne_korrelation_ist_gptq_blosses_runden():
    """H diagonal: Die Kompensation hat keine Nachbarn, an die sie Fehler
    weitergeben koennte, also ergibt GPTQ genau das blosse Runden."""
    W = gewichte(saat=3)
    k = tg.raster_bits(W)
    d = torch.rand(2, 256, generator=zufall(4)).double() + 0.5
    H = torch.diag_embed(d)
    eins = (1.0,)
    assert torch.equal(tg.ternaer_gptq(W, H, k, eins), tg.ternaer_gptq(W, None, k, eins))
    print("ok ohne Korrelation ist GPTQ blosses Runden")


def test_mit_korrelation_sinkt_der_ausgabefehler():
    """Korrelierte Eingaben: GPTQ verteilt den Fehler um, und der Fehler der
    Ausgabe auf denselben Eingaben ist kleiner als beim blossen Runden."""
    W = gewichte(E=1, zeilen=16, saat=5)
    k = tg.raster_bits(W)
    g = zufall(6)
    basis = torch.randn(1024, 16, generator=g)
    X = (basis @ torch.randn(16, 256, generator=g) + 0.1 * torch.randn(1024, 256, generator=g))
    H = (X.double().T @ X.double()).unsqueeze(0)
    skala = torch.pow(2.0, k[0]).unsqueeze(1)
    eins = (1.0,)
    fg = tg.ausgabefehler(X, W[0], tg.ternaer_gptq(W, H, k, eins)[0] / skala)
    fr = tg.ausgabefehler(X, W[0], tg.ternaer_gptq(W, None, k, eins)[0] / skala)
    assert fg < fr, f"GPTQ {fg:.4f} nicht unter blossem Runden {fr:.4f}"
    print(f"ok mit Korrelation sinkt der Ausgabefehler ({fg:.4f} gegen {fr:.4f})")


def test_die_skalensuche_senkt_den_fehler():
    """Dieselbe Kompensation, aber a je Gruppe gesucht: Der Ausgabefehler auf
    denselben Eingaben sinkt gegenueber dem Betragsmittel allein."""
    W = gewichte(E=1, zeilen=16, saat=7)
    k = tg.raster_bits(W)
    g = zufall(8)
    X = torch.randn(1024, 16, generator=g) @ torch.randn(16, 256, generator=g) + 0.1 * torch.randn(1024, 256, generator=g)
    H = (X.double().T @ X.double()).unsqueeze(0)
    skala = torch.pow(2.0, k[0]).unsqueeze(1)
    mit = tg.ausgabefehler(X, W[0], tg.ternaer_gptq(W, H, k)[0] / skala)
    ohne = tg.ausgabefehler(X, W[0], tg.ternaer_gptq(W, H, k, (1.0,))[0] / skala)
    assert mit < ohne, f"mit Suche {mit:.4f} nicht unter ohne {ohne:.4f}"
    print(f"ok die Skalensuche senkt den Fehler ({mit:.4f} gegen {ohne:.4f})")


if __name__ == "__main__":
    test_jede_gruppe_ist_ternaer()
    test_ohne_korrelation_ist_gptq_blosses_runden()
    test_mit_korrelation_sinkt_der_ausgabefehler()
    test_die_skalensuche_senkt_den_fehler()
    print("alle Proben bestanden")
