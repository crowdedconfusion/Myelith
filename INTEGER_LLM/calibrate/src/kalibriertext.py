#!/usr/bin/env python3
"""Stellt den Kalibriertext fuer die Experten in vier Bit zusammen.

Der Text, mit dem `myelith-35b-a3b-int4` kalibriert ist (Mitschnitt der
Expertenaufrufe, dann GPTQ je Experte), gemischt aus vier Quellen in
Stuecken zu 2000 Zeichen, reihum:

1. WikiText-2, Trainingssplit, Zeichen 2 000 000 bis 2 060 000 (nicht der
   Pruefsplit, auf dem die Perplexitaet gemessen wird),
2. Quelltext (der Anfang des Belegungsplans des Dispatchers),
3. deutsche Prosa (der Anfang des Glossars),
4. ein gemischter Lerntext.

⚑ **Die Teile 2 bis 4 sind eingefroren** (`calibrate/daten/kalibrierteile.json`),
denn ihre Quellen aendern sich weiter, und ein anderer Text ergibt andere
Gewichte. WikiText liegt nicht im Repositorium; fehlt die Datei, wird der
Trainingssplit geholt und gegen seine Pruefsumme geprueft.

⛔️ **Das Ergebnis wird gegen [`TEXT_SHA256`] geprueft.** Weicht es ab,
bricht das Skript ab, statt ein Artefakt aus einem anderen Text zu bauen.

Aufruf aus der Wurzel des Repositoriums:

    INTEGER_LLM/calibrate/.venv/bin/python3 INTEGER_LLM/calibrate/src/kalibriertext.py <ziel.txt>
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

WURZEL = Path(__file__).resolve().parents[3]
TEILE = Path(__file__).resolve().parents[1] / "daten" / "kalibrierteile.json"
WIKITEXT_TRAIN = WURZEL / "BENCHMARKS" / "Inferenz" / "datasets" / "wikitext2_train.txt"

#: SHA-256 der Datei `wikitext2_train.txt`: die Zeilen des Trainingssplits
#: von `Salesforce/wikitext`, `wikitext-2-raw-v1`, mit "\n" verbunden.
WIKITEXT_SHA256 = "c58d555a94ada0882d1e8c0b3dc7ba68e1c269da5ad9e7a057300245d130c4e8"
#: SHA-256 des fertigen Texts (UTF-8), mit dem das Artefakt gebaut ist.
TEXT_SHA256 = "8518aed4cb975acbb98a96ee4fff05cce12a34b9119904cce8950990c5342666"

WIKI_VON, WIKI_BIS = 2_000_000, 2_060_000
STUECK = 2000


def sha256(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def wikitext_train() -> str:
    """Der Trainingssplit als Text; holt und prueft ihn, falls er fehlt."""
    if not WIKITEXT_TRAIN.is_file():
        try:
            from datasets import load_dataset
        except ImportError:
            sys.exit("[kalibriertext] FEHLER: 'datasets' fehlt (INTEGER_LLM/scripts/python_einrichten.sh)")
        print("[kalibriertext] Lade WikiText-2 (Trainingssplit) von Hugging Face ...", flush=True)
        ds = load_dataset("Salesforce/wikitext", "wikitext-2-raw-v1", split="train")
        WIKITEXT_TRAIN.parent.mkdir(parents=True, exist_ok=True)
        WIKITEXT_TRAIN.write_text("\n".join(str(x) for x in ds["text"]), encoding="utf-8")
    roh = WIKITEXT_TRAIN.read_bytes()
    if sha256(roh) != WIKITEXT_SHA256:
        sys.exit(
            f"[kalibriertext] FEHLER: {WIKITEXT_TRAIN} hat die Pruefsumme {sha256(roh)}, "
            f"erwartet {WIKITEXT_SHA256}. Datei loeschen und neu holen lassen."
        )
    return roh.decode("utf-8")


def zusammenstellen() -> str:
    teile = json.loads(TEILE.read_text(encoding="utf-8"))
    quellen = [
        wikitext_train()[WIKI_VON:WIKI_BIS],
        teile["quelltext"],
        teile["glossar"],
        teile["lerntext"],
    ]
    stuecke: list[str] = []
    stelle = [0] * len(quellen)
    while any(stelle[i] < len(q) for i, q in enumerate(quellen)):
        for i, q in enumerate(quellen):
            if stelle[i] < len(q):
                stuecke.append(q[stelle[i]:stelle[i] + STUECK])
                stelle[i] += STUECK
    return "\n".join(stuecke)


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    text = zusammenstellen().encode("utf-8")
    if sha256(text) != TEXT_SHA256:
        print(f"[kalibriertext] FEHLER: Pruefsumme {sha256(text)}, erwartet {TEXT_SHA256}", file=sys.stderr)
        return 1
    Path(sys.argv[1]).write_bytes(text)
    print(f"[kalibriertext] {len(text)} Byte, Pruefsumme stimmt: {sys.argv[1]}")
    print("Fertig")
    return 0


if __name__ == "__main__":
    sys.exit(main())
