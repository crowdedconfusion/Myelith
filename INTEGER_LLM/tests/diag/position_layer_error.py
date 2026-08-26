#!/usr/bin/env python3
"""Ebenenfehler ueber ALLE Positionen — ein Instrument, eine Referenz.

**Warum es diese Messung gibt (2026-08-20, Punkt 12.77).**
Die Kernaussage der bisherigen Eingrenzung lautete: an Position 0 liegt
unser Pfad auf Schema-Niveau (2,08 %), ab Position 1 beim Doppelten
(4,53 %). Diese beiden Zahlen stammen aber aus **verschiedenen
Binaries und verschiedenen Auswerteskripten**:

    Position 0  -> layer_probe        -> layer_stage_compare.py
    Position 31 -> seq_layer_dump     -> layer_bulk_error.py

Nach neun Instrumentenfehlern in dieser Fehlersuche ist ein Vergleich
zweier verschiedener Instrumente kein Befund, sondern eine Annahme.
Hier liefert **ein** Lauf alle Positionen, und **eine** Funktion
vergleicht sie gegen dieselbe HF-Referenz.

**Ausrichtung (der heikle Teil, schon zweimal falsch gemacht).**
HF legt in `hidden_states` die EINGAENGE der Ebenen ab:
    hs[0]   = Embedding
    hs[i]   = Eingang Ebene i = Ausgabe Ebene i-1   (i = 1..N-1)
    hs[N]   = NACH der finalen Norm  (nicht die Ausgabe der letzten Ebene!)
Unser Dump:
    dump[i] = Ausgabe Ebene i                        (i = 0..N-1)
    dump[N] = nach der finalen Norm
Also dump[i] <-> hs[i+1] fuer i = 0..N-2, und dump[N] <-> hs[N].
Fuer dump[N-1] gibt es KEINEN hs-Eintrag — diese Ebene wird uebersprungen.

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad.

Usage:
    seq_layer_dump <artefakt> <tok...> --alle-positionen > dump.txt
    INTEGER_LLM_MODEL=... python -u tests/diag/position_layer_error.py dump.txt <tok...>
"""
import sys
from pathlib import Path

import numpy as np
import torch

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR  # noqa: E402
from fortschritt import Fortschritt  # noqa: E402


def rel_l2(a: np.ndarray, b: np.ndarray) -> float:
    """Relativer L2 in Prozent. Bulk-Mass ueber alle Kanaele — nicht AbsMax,
    das nur den einen Ausreisserkanal verfolgt."""
    d = a - b
    return 100.0 * float(np.sqrt((d * d).sum() / max((b * b).sum(), 1e-30)))


def main() -> None:
    dump_pfad = sys.argv[1]
    tokens = [int(t) for t in sys.argv[2:]]

    # Dump einlesen: POS <p> FULL <ebene> <werte...>
    unser: dict[tuple[int, int], np.ndarray] = {}
    for zeile in open(dump_pfad):
        if not zeile.startswith("POS "):
            continue
        teile = zeile.split()
        pos, ebene = int(teile[1]), int(teile[3])
        unser[(pos, ebene)] = np.array([float(x) for x in teile[4:]], dtype=np.float64)

    positionen = sorted({p for p, _ in unser})
    ebenen = sorted({e for _, e in unser})
    print(f"[dump] {len(positionen)} Positionen, {len(ebenen)} Eintraege je Position")

    modell, _ = load_reference_model(MODEL_DIR)
    n_layer = modell.config.num_hidden_layers
    with torch.no_grad():
        aus = modell(torch.tensor([tokens]), output_hidden_states=True)
    hs = [h[0].float().numpy() for h in aus.hidden_states]
    print(f"[hf] {len(hs)} hidden_states, {n_layer} Ebenen, Sequenzlaenge {hs[0].shape[0]}")

    # Selbstpruefung: Das Embedding muss zwischen beiden Pfaden nahezu
    # identisch sein. Weicht es ab, stimmt die Tokenfolge oder die
    # Ausrichtung nicht, und alles Weitere waere bedeutungslos.
    if (0, 0) in unser:
        probe = rel_l2(unser[(0, 0)], hs[1][0])
        print(f"[selbstpruefung] Ebene 0 an Position 0: {probe:.2f} % "
              f"(erwartet ~2 %, das ist der bekannte Schema-Anteil)")

    interessant = [e for e in ebenen if e <= n_layer - 2]
    # Volles Ebenenprofil fuer wenige Positionen ist aussagekraeftiger als
    # wenige Ebenen fuer alle Positionen: Die Frage lautet, ob sich Position 0
    # ueber den GANZEN Stapel anders verhaelt oder nur an einzelnen Ebenen.
    zeigen = interessant[:7]

    print()
    print("Relativer L2 unseres Pfads gegen HF-Gleitkomma, je Position und Ebene")
    print()
    profil_pos = [p for p in (0, 1, 2, 8, 16, positionen[-1]) if p in positionen]
    print("Volles Ebenenprofil (Zeile = Ebene, Spalte = Position)")
    kopf = "Ebene | " + " | ".join(f"Pos {p:>2}" for p in profil_pos)
    print(kopf)
    print("-" * len(kopf))
    for e in interessant:
        werte = [rel_l2(unser[(p, e)], hs[e + 1][p]) for p in profil_pos]
        print(f"{e:>5} | " + " | ".join(f"{w:6.2f}" for w in werte))
    print()
    kopf = "Position | " + " | ".join(f"Ebene {e:>2}" for e in zeigen)
    print(kopf)
    print("-" * len(kopf))

    zeilen = []
    with Fortschritt(len(positionen), "Positionen") as fort:
        for p in positionen:
            werte = []
            for e in zeigen:
                a = unser.get((p, e))
                werte.append(float("nan") if a is None else rel_l2(a, hs[e + 1][p]))
            zeilen.append((p, werte))
            fort.schritt()

    for p, werte in zeilen:
        print(f"{p:>8} | " + " | ".join(f"{w:7.2f} %" for w in werte))

    # Verdichtung: waechst der Fehler mit der Position oder mit der Tiefe?
    print()
    p0 = {e: rel_l2(unser[(0, e)], hs[e + 1][0]) for e in interessant if (0, e) in unser}
    pl = positionen[-1]
    pn = {e: rel_l2(unser[(pl, e)], hs[e + 1][pl]) for e in interessant if (pl, e) in unser}
    if p0 and pn:
        print(f"Mittel ueber alle Ebenen: Position 0 = {np.mean(list(p0.values())):.2f} %, "
              f"Position {pl} = {np.mean(list(pn.values())):.2f} %")


if __name__ == "__main__":
    main()
