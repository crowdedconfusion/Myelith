#!/usr/bin/env python3
"""
Bitbudget des ganzzahligen Trainings: F, Akkumulator, Aggregation.

## Warum gerechnet und nicht gewaehlt

Drei Breiten entscheiden, ob das Verfahren traegt, und alle drei folgen
aus Messgroessen statt aus Gewohnheit:

    F         Nachkommabits des Masters unterhalb der int8-Stufe.
              Zu wenige, und eine Aktualisierung verschwindet unter einem
              LSB, und das Training bricht (0.1). Zu viele, und der
              Master passt nicht mehr in die gewaehlte Wortbreite.

    W_master  Wortbreite des Masters. int8-Bereich plus F.

    W_akku    Wortbreite der Aggregation ueber viele Miner. Die Summe
              darf nicht ueberlaufen, BEVOR genau einmal am Ende
              gesaettigt wird (Determinismus-Vertrag aus dot.rs: Wer
              zwischendurch klemmt, macht die Summe
              reihenfolgeabhaengig).

**F ist je Modell und Lernrate neu zu rechnen, nicht zu uebernehmen.**
Dieses Skript misst die dafuer noetige Groesse am echten Modell: das
Verhaeltnis von typischem Aktualisierungsbetrag zur Rasterstufe.

Gleitkomma erlaubt: Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage:
    cd INTEGER_LLM/calibrate
    .venv/bin/python ../../TRAINING/tests/diag/bitbudget.py
    .venv/bin/python ../../TRAINING/tests/diag/bitbudget.py --lr 1e-4
"""
import argparse
import json
import math
import sys
from datetime import date
from pathlib import Path

WURZEL = Path(__file__).resolve().parent.parent.parent.parent
INTEGER_LLM = WURZEL / "INTEGER_LLM"
sys.path.insert(0, str(INTEGER_LLM / "calibrate"))
sys.path.insert(0, str(INTEGER_LLM / "eval"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import torch  # noqa: E402
import torch.nn as nn  # noqa: E402
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, MODEL_NAME  # noqa: E402
import backward_reference_simulation as basis  # noqa: E402

ERGEBNISSE = Path(__file__).resolve().parent / "results"
SICHERHEIT_BITS = 4   # Reserve auf die gemessene Untergrenze


def messen(lr: float, schritte: int, geraet: str):
    """Misst je Schicht: Rasterstufe, typischer Schritt, Verhaeltnis."""
    model, _ = load_reference_model(MODEL_DIR)
    model = model.to(torch.float32).to(geraet)
    model.train()
    model.config.use_cache = False

    batches = basis.batches_bauen(4, 128, geraet)
    ziel = [(n, m) for n, m in model.named_modules()
            if isinstance(m, nn.Linear) and "lm_head" not in n]

    verhaeltnisse = []
    for i in range(schritte):
        x = batches[i % len(batches)]
        model(x, labels=x).loss.backward()
        for name, mod in ziel:
            if mod.weight.grad is None:
                continue
            absmax = mod.weight.detach().abs().amax(dim=1, keepdim=True)
            shift = basis._shift(absmax, 127)
            stufe = 1.0 / torch.pow(2.0, shift)          # int8-Rasterstufe je Zeile
            schritt = (lr * mod.weight.grad.abs()).mean(dim=1, keepdim=True)
            gueltig = schritt > 0
            if bool(gueltig.any()):
                verhaeltnisse.extend((schritt[gueltig] / stufe[gueltig]).tolist())
        model.zero_grad(set_to_none=True)

    del model
    if geraet == "mps":
        torch.mps.empty_cache()
    return verhaeltnisse


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--lr", type=float, default=1e-5)
    p.add_argument("--schritte", type=int, default=4)
    p.add_argument("--miner", type=int, default=10_000,
                   help="Beitraege je Epoche fuer die Ueberlaufrechnung")
    p.add_argument("--schritte-je-epoche", type=int, default=1000)
    args = p.parse_args()

    geraet = "mps" if torch.backends.mps.is_available() else "cpu"
    print(f"Bitbudget fuer {MODEL_NAME}, lr {args.lr}, Geraet {geraet}\n")

    v = messen(args.lr, args.schritte, geraet)
    v.sort()
    median = v[len(v) // 2]
    p01 = v[max(0, int(len(v) * 0.01))]

    # F muss so gross sein, dass auch der KLEINE Schritt noch ein LSB
    # bewegt: Der Median genuegt nicht, sonst verschwindet das untere
    # Prozent der Schichten still, und genau solche stillen Verluste
    # sind in diesem Projekt schon zweimal teuer geworden (Fund 23, 24).
    f_median = math.ceil(math.log2(1.0 / median))
    f_p01 = math.ceil(math.log2(1.0 / p01))
    f_empfohlen = f_p01 + SICHERHEIT_BITS

    print(f"Gemessen ueber {len(v):,} Zeilen und {args.schritte} Schritte:")
    print(f"  Schritt / Rasterstufe   Median {median:.3e}   1. Perzentil {p01:.3e}")
    print()
    print(f"  F, damit der MEDIAN ein LSB bewegt:        {f_median:2d} Bits")
    print(f"  F, damit das 1. PERZENTIL ein LSB bewegt:  {f_p01:2d} Bits")
    print(f"  F empfohlen (plus {SICHERHEIT_BITS} Bits Reserve):          "
          f"{f_empfohlen:2d} Bits")
    print()

    w_master = 8 + f_empfohlen
    print(f"  W_master = 8 + F = {w_master} Bits  ->  "
          f"{'int32' if w_master <= 32 else 'int64'} traegt es")
    print(f"    Wertebereich des Masters: +/- {127 * 2**f_empfohlen:,}")
    print()

    # Aggregation: Summe vieler Deltas, Saettigung genau einmal am Ende.
    schritt_lsb = median * 2**f_empfohlen
    beitraege = args.miner * args.schritte_je_epoche
    summe_max = beitraege * schritt_lsb
    reserve_32 = (2**31 - 1) / max(summe_max, 1)
    reserve_64 = (2**63 - 1) / max(summe_max, 1)
    print(f"  Aggregation: {args.miner:,} Beitraege x {args.schritte_je_epoche:,} Schritte")
    print(f"    ein Schritt ~ {schritt_lsb:.0f} LSB, Summe im schlimmsten Fall "
          f"{summe_max:,.0f} LSB")
    print(f"    int32: Sicherheitsabstand Faktor {reserve_32:,.0f}")
    print(f"    int64: Sicherheitsabstand Faktor {reserve_64:,.0f}")
    print()
    empfehlung = "int32" if reserve_32 > 1000 else "int64"
    print(f"  Akkumulator empfohlen: {empfehlung}")
    print(f"    Saettigung genau EINMAL am Ende, nicht zwischendurch:")
    print(f"    sonst wird die Summe reihenfolgeabhaengig (dot.rs).")

    ERGEBNISSE.mkdir(parents=True, exist_ok=True)
    ziel = ERGEBNISSE / f"bitbudget_{MODEL_NAME.replace('.', '')}_lr{args.lr:g}.json"
    ziel.write_text(json.dumps({
        "modell": MODEL_NAME, "datum": date.today().isoformat(), "lr": args.lr,
        "schritt_durch_rasterstufe": {"median": median, "p01": p01},
        "F": {"median": f_median, "p01": f_p01, "empfohlen": f_empfohlen,
              "reserve_bits": SICHERHEIT_BITS},
        "W_master_bits": w_master,
        "aggregation": {"beitraege": beitraege, "summe_max_lsb": summe_max,
                        "reserve_int32": reserve_32, "reserve_int64": reserve_64,
                        "empfohlen": empfehlung},
    }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"\nGeschrieben: {ziel.relative_to(WURZEL)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
