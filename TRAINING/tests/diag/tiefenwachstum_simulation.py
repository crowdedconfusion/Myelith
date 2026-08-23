#!/usr/bin/env python3
"""
Tiefenwachstum: bleibt eine als Identitaet gestartete Ebene tot?
(Fahrplanpunkt 1.3, Kap. 7.5)

## Die Frage

Breitenwachstum ist gemessen (`expansion_simulation.py`): Die Aufteilung
`a + b = m` ist exakt, die Ausgabe bitgleich, und die Symmetrie der
beiden Kopien bricht zweifach.

Tiefenwachstum ist etwas anderes. Eine **neue Ebene** ist
funktionserhaltend, wenn sie als Identitaet startet, im Residualstrom
also mit einem Ausgabegewicht von null. Der Verdacht war: Sie bleibt tot.
Ausgabegewicht null heisst Beitrag null, und wo nichts beitraegt, sollte
auch nichts lernen.

Es gibt hier **keine zwei Kopien**, deren Symmetrie brechen koennte. Die
Frage ist deshalb nicht Symmetrie, sondern: **bewegt sich die Ebene
ueberhaupt, und ab wann?**

## Warum diese Probe existiert

`Konzept-Wachstum.md` fuehrte diese Messung seit dem 2026-08-22 als
erledigt, mit konkreten Zahlen ("63 von 128", "alle 128") und dem Beleg
"`tests/diag/expansion_simulation.py` und die Probe im Protokoll".

**Beides gab es nicht.** `expansion_simulation.py` misst ausschliesslich
Breitenwachstum, das sagt sein eigener Kopf; ein Protokoll mit diesen
Zahlen existiert im ganzen Repositorium nicht. Der Fahrplan fuehrte 1.3
zu Recht als "nicht gemessen".

Dieselbe Klasse wie Fund 27 und Fund 37: eine schriftliche Zusage ohne
Deckung. Diese Datei stellt die Deckung her.

## Aufbau

Eine Ebene mit Residualpfad, wie sie beim Tiefenwachstum entsteht:

    y = x + W_out @ silu(W_in @ x)

`W_out` startet auf null, die Ebene ist also die Identitaet. Gemessen
wird, ob und wie schnell sich der Master von `W_out` bewegt, einmal mit
Rundung zur naechsten Stufe und einmal mit stochastischem Runden.

Der Gradient nach `W_out` ist `aᵀ·g` und haengt **nicht** von `W_out`
ab. Ein Nullgewicht macht den Beitrag null, nicht den Gradienten. Genau
das ist die Behauptung, die hier geprueft wird.
"""
import json
import sys
from pathlib import Path

WURZEL = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import torch  # noqa: E402
import integer_master_simulation as im  # noqa: E402

F = im.ZUSATZBITS
INT8_MAX = 127
SCHRITTE = 20
LERNRATE = 1e-5


def w8(master, shift, ebene, schritt, stochastisch, keim=1):
    """Das wirksame Gewicht, wie im Trainingsschritt aus 0.2."""
    roh = master / (1 << F)
    if stochastisch:
        idx = torch.arange(master.numel(), dtype=torch.int64).reshape(master.shape)
        wu = im.wuerfel(idx, ebene, schritt, keim)
        q = torch.floor(roh) + (wu < (roh - torch.floor(roh))).to(roh.dtype)
    else:
        q = torch.round(roh)
    return torch.clamp(q, -INT8_MAX - 1, INT8_MAX) / torch.pow(2.0, shift)


def neue_ebene(d_modell: int, d_versteckt: int):
    """Eine frisch gewachsene Ebene: W_in zufaellig, W_out auf null."""
    m_in = torch.round(torch.randn(d_versteckt, d_modell) * 40 * (1 << F) / 128)
    m_out = torch.zeros(d_modell, d_versteckt)          # Identitaet
    s_in = torch.full((d_versteckt, 1), 5.0)
    s_out = torch.full((d_modell, 1), 5.0)
    return m_in, s_in, m_out, s_out


def vorwaerts(x, m_in, s_in, m_out, s_out, schritt, stoch):
    a = torch.nn.functional.silu(x @ w8(m_in, s_in, 0, schritt, stoch).t())
    return x + a @ w8(m_out, s_out, 1, schritt, stoch).t(), a


def main():
    torch.manual_seed(0)
    d, h = 16, 8
    x = torch.randn(4, d)
    m_in, s_in, m_out, s_out = neue_ebene(d, h)

    print("Tiefenwachstum: eine als Identitaet gestartete Ebene")
    print(f"  Modellbreite {d}, versteckte Breite {h}, "
          f"{m_out.numel()} Gewichte in W_out\n")

    # --- 1. Funktionserhaltung -------------------------------------------
    y, _ = vorwaerts(x, m_in, s_in, m_out, s_out, 0, False)
    bitgleich = bool(torch.equal(y, x))
    abweichung = float((y - x).abs().max())
    print(f"1. Ausgabe bitgleich zur Eingabe: {bitgleich}"
          f"   (max. Abweichung {abweichung:.2e})")
    assert bitgleich, "eine Identitaetsebene muss die Eingabe unveraendert lassen"

    # --- 2. Ist der Gradient nach W_out wirklich ungleich null? ----------
    # Das ist der Kern: Waere er null, bliebe die Ebene tot.
    g = torch.randn(4, d)                       # Fehlerterm von oben
    erreichbar = {}
    for stoch, name in [(False, "Rundung zur naechsten Stufe"),
                        (True, "stochastisch (Zaehlerwuerfel)")]:
        menge = set()
        for s in range(SCHRITTE):
            _, a_s = vorwaerts(x, m_in, s_in, m_out, s_out, s, stoch)
            grad_s = g.t() @ a_s                # aᵀ·g, unabhaengig von W_out
            for i in (grad_s != 0).nonzero().tolist():
                menge.add(tuple(i))
        erreichbar[name] = len(menge)

    print(f"2. Eintraege mit Gradient ungleich null in mindestens einem "
          f"von {SCHRITTE} Schritten")
    for name, n in erreichbar.items():
        print(f"   {name:32} {n:3d}/{m_out.numel()}")
    print("   Der Unterschied ist kein Rauschen: Das stochastische Runden")
    print("   veraendert auch W_in je Schritt, damit die Aktivierungen und")
    print("   damit, welche Eintraege ueberhaupt einen Gradienten sehen.")
    assert min(erreichbar.values()) > 0, \
        "ein Nullgewicht darf den Gradienten nicht ausloeschen"
    nicht_null = erreichbar["Rundung zur naechsten Stufe"]

    # --- 3. Bewegt sich die Ebene, und ab wann? --------------------------
    print(f"\n3. Bewegung ueber {SCHRITTE} Schritte (Lernrate {LERNRATE:g})")
    ergebnis = {}
    for stoch, name in [(False, "Rundung zur naechsten Stufe"),
                        (True, "stochastisch (Zaehlerwuerfel)")]:
        master = m_out.clone()
        erster_schritt = None
        for s in range(SCHRITTE):
            _, a_s = vorwaerts(x, m_in, s_in, master, s_out, s, stoch)
            grad = g.t() @ a_s
            # Aktualisierung des Masters in seiner eigenen Einheit.
            delta = -LERNRATE * grad * (1 << F) * torch.pow(2.0, s_out)
            if stoch:
                idx = torch.arange(delta.numel(), dtype=torch.int64).reshape(delta.shape)
                wu = im.wuerfel(idx, 2, s, 1)
                delta = torch.floor(delta) + (wu < (delta - torch.floor(delta))).to(delta.dtype)
            else:
                delta = torch.round(delta)
            master = master + delta
            if erster_schritt is None and bool((master != 0).any()):
                erster_schritt = s + 1
        bewegt = int((master != m_out).sum().item())
        print(f"   {name:32} {bewegt:3d}/{master.numel()} Gewichte bewegt, "
              f"erstmals in Schritt {erster_schritt}")
        ergebnis[name] = {"bewegt": bewegt, "von": int(master.numel()),
                          "erster_schritt": erster_schritt}
        assert bewegt > 0, f"{name}: die Ebene bleibt tot"

    # --- 4. Und bleibt sie danach funktionserhaltend? --------------------
    # Nein, und das ist der Sinn: Eine Ebene, die sich bewegt, traegt bei.
    print("\n4. Nach der Bewegung ist die Ebene keine Identitaet mehr,")
    print("   und genau das ist gewollt: Eine Ebene, die beitraegt, ist")
    print("   nicht mehr die Identitaet.")

    ziel = Path(__file__).resolve().parent / "results" / "tiefenwachstum.json"
    ziel.write_text(json.dumps({
        "frage": "Bleibt eine als Identitaet gestartete Ebene tot?",
        "antwort": "nein",
        "modellbreite": d, "versteckte_breite": h,
        "gewichte_in_w_out": int(m_out.numel()),
        "schritte": SCHRITTE, "lernrate": LERNRATE,
        "funktionserhaltend_vor_dem_ersten_schritt": bitgleich,
        "gradient_ungleich_null_erreichbar": erreichbar,
        "bewegung": ergebnis,
    }, indent=2, ensure_ascii=False) + "\n")
    print(f"\nProtokoll: {ziel.relative_to(WURZEL)}")
    print("Alle Zusicherungen gehalten.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
