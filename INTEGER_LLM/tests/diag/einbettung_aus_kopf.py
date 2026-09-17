#!/usr/bin/env python3
"""Ist die Einbettungstabelle bitgleich aus dem LM-Kopf herzuleiten?

# ⚑ Wozu das gebraucht wird

Setzt ein Modell `tie_word_embeddings`, sind Einbettung und LM-Kopf
dieselbe Matrix. Das Artefakt legt trotzdem beide ab: den Kopf in int16
und die Einbettung in int8. Beim 0,6B sind das 156 MB von 0,93 GB, beim
4B 389 MB von 4,82 GB.

⚑ **Die int8-Fassung ist eine geschachtelte Quantisierung der
int16-Fassung**: derselbe Wert, um die Differenz der Kanalshifts nach
rechts geschoben, mit Runden zur naechsten geraden Zahl. Genau die
Rundungsregel, die im Rechenpfad ohnehin gilt.

⛔️ **Dieses Skript ist das Tor vor dem Loeschen.** Wer die Datei
einspart, muss die Beziehung zeigen und nicht annehmen: Sie haengt am
Quantisierer, und ein Quantisierer, der die Schachtelung bricht, macht
aus der Ersparnis eine falsche Einbettung. **Geprueft wird ueber alle
Werte, nicht an einer Stichprobe**, denn eine Stichprobe von 99,6 %
sieht aus wie ein Treffer und ist ein Fehlschlag.

Aufruf:

    python3 INTEGER_LLM/tests/diag/einbettung_aus_kopf.py [<artefakt> ...]

Ohne Angabe werden alle Artefakte unter `INTEGER_LLM/artifacts` geprueft.
Rueckgabe 0, wenn jedes betroffene Artefakt herleitbar ist.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

WURZEL = Path(__file__).resolve().parents[3]


def shift_gerade(x: np.ndarray, n: int) -> np.ndarray:
    """Arithmetischer Rechtsshift um `n`, Runden zur naechsten geraden Zahl.

    ⚑ Dieselbe Regel wie im Rechenpfad: Auf der halben Stelle wird zur
    geraden Zahl gerundet, sonst zur naechsten.
    """
    halb = 1 << (n - 1)
    grob = (x + halb) >> n
    rest = x & ((1 << n) - 1)
    return np.where((rest == halb) & ((grob & 1) == 1), grob - 1, grob)


def ein_artefakt(ordner: Path) -> bool | None:
    """Prueft ein Artefakt. `None`, wenn es die Frage nicht betrifft."""
    cfg = json.loads((ordner / "model_config.json").read_text())
    if not cfg.get("tie_word_embeddings"):
        print(f"{ordner.name}: ohne tie_word_embeddings, nicht betroffen")
        return None

    vocab, hidden = cfg["vocab_size"], cfg["hidden_size"]
    kopf = np.fromfile(ordner / "lm_head.bin", dtype=np.int16)
    einb = np.fromfile(ordner / "model_embed_tokens_weight.bin", dtype=np.int8)
    if kopf.size != vocab * hidden or einb.size != vocab * hidden:
        print(f"{ordner.name}: FEHLER, Groessen passen nicht zur Konfiguration")
        return False

    kopf = kopf.astype(np.int32).reshape(vocab, hidden)
    einb = einb.astype(np.int32).reshape(vocab, hidden)
    ks = np.fromfile(ordner / "lm_head_shifts.bin", dtype=np.int8).astype(np.int32)
    es = np.fromfile(
        ordner / "model_embed_tokens_weight_shifts.bin", dtype=np.int8
    ).astype(np.int32)
    d = ks - es
    if d.min() < 1:
        print(f"{ordner.name}: FEHLER, Shiftdifferenz {d.min()} ist kein Rechtsshift")
        return False

    abgeleitet = np.empty_like(einb)
    for n in np.unique(d):
        abgeleitet[d == n] = shift_gerade(kopf[d == n], int(n))

    gleich = int((abgeleitet == einb).sum())
    ganz = einb.size
    mb = (ordner / "model_embed_tokens_weight.bin").stat().st_size / 1e6
    alles = sum(f.stat().st_size for f in ordner.iterdir() if f.is_file()) / 1e9
    verteilung = {int(k): int(v) for k, v in zip(*np.unique(d, return_counts=True))}
    print(
        f"{ordner.name}: {gleich}/{ganz} gleich ({100.0 * gleich / ganz:.6f} %), "
        f"Shifts je Zeile {verteilung}, "
        f"Einbettung {mb:.0f} MB von {alles:.2f} GB ({100.0 * mb / (alles * 1000):.0f} %)"
    )
    if gleich != ganz:
        # ⛔️ Ein einziger abweichender Wert reicht: Er stuende spaeter
        # als falsches Token im Kontext, und niemand saehe ihm an, woher
        # er kam.
        falsch = np.argwhere(abgeleitet != einb)[:3]
        for r, c in falsch:
            print(
                f"  Zeile {r}, Spalte {c}: Kopf {kopf[r, c]} >> {d[r]} "
                f"= {abgeleitet[r, c]}, abgelegt {einb[r, c]}"
            )
    return gleich == ganz


def main() -> int:
    if len(sys.argv) > 1:
        ordner = [Path(a) for a in sys.argv[1:]]
    else:
        ordner = sorted(
            p for p in (WURZEL / "INTEGER_LLM" / "artifacts").iterdir()
            if (p / "model_config.json").is_file()
        )
    if not ordner:
        print("[einbettung] kein Artefakt gefunden")
        return 1

    urteile = [ein_artefakt(o) for o in ordner]
    gepruefte = [u for u in urteile if u is not None]
    if not gepruefte:
        print("[einbettung] kein Artefakt mit gebundenen Gewichten")
        return 0
    if all(gepruefte):
        print(
            f"[einbettung] PASSED: {len(gepruefte)} Artefakt(e) herleitbar, "
            "die abgelegte Einbettung traegt nichts bei"
        )
        return 0
    print("[einbettung] FAILED: mindestens ein Artefakt ist nicht herleitbar")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
