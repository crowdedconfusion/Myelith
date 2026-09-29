#!/usr/bin/env python3
"""
Mehrere Artefakte auf denselben WikiText-2-Folgen, ganzzahlig gerechnet.

Zwei Fragen, ein Werkzeug:

- **Perplexitaet** (Vorgabe): Jedes Artefakt bekommt dieselben Folgen aus
  `wikitext_common.select_sequences`, der einzigen Quelle der
  Messsequenzen. So ist ein Abstand zwischen zwei Artefakten ein Abstand
  der Modelle und nicht der Auswahl.
- **Bitgleichheit** (`--bitgleich`, genau zwei Artefakte): Die Zeilen je
  Position aus `perplexity_probe --per-token` muessen **alle** gleich sein.
  Das belegt, dass eine andere Speicherform (etwa ternaer gepackt) dieselbe
  ganze Zahl rechnet wie ihre Quelle.

Aufruf aus der Wurzel mit dem Kalibrier-venv (Tokenizer):

    V=INTEGER_LLM/calibrate/.venv/bin/python3
    INTEGER_LLM_MODEL=<modell> $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 4 128 <artefakt> [<artefakt> ...]
    INTEGER_LLM_MODEL=<modell> $V -u BENCHMARKS/Inferenz/artefaktvergleich.py 4 128 <quelle> <gepackt> --bitgleich

`INTEGER_LLM_MODEL` waehlt den Tokenizer; alle Artefakte muessen ihn
teilen. Am Ende steht `Fertig`, damit ein Waechter darauf warten kann.
"""

import math
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
WURZEL = HIER.parents[1]
sys.path.insert(0, str(HIER))
sys.path.insert(0, str(WURZEL / "INTEGER_LLM" / "tests"))
from wikitext_common import select_sequences  # noqa: E402
from cargo_paths import binary  # noqa: E402

PROBE = binary("runtime", "perplexity_probe")


def folgendatei(n: int, laenge: int) -> str:
    folgen = select_sequences(n, laenge, verbose=False)
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        for ids in folgen:
            f.write(" ".join(str(t) for t in ids) + "\n")
        return f.name


def probe(artefakt: str, datei: str, *zusatz: str) -> list:
    r = subprocess.run([str(PROBE), artefakt, datei, *zusatz],
                       capture_output=True, text=True, timeout=14400)
    if r.returncode != 0:
        print(f"FEHLER bei {artefakt}:\n{r.stderr[-2000:]}", flush=True)
        sys.exit(1)
    return r.stdout.strip().splitlines()


def perplexitaet(artefakte: list, datei: str) -> None:
    for art in artefakte:
        t0 = time.time()
        n_eval, summe, je = 0, 0.0, []
        for zeile in probe(art, datei):
            _toks, anzahl, slp, ppl = zeile.split()
            n_eval += int(anzahl)
            summe += float(slp)
            je.append(float(ppl))
        print(f"[vergleich] {Path(art).name}: Perplexitaet {math.exp(-summe / n_eval):.4f} "
              f"({n_eval} Positionen, {time.time() - t0:.0f} s; "
              f"je Sequenz min {min(je):.2f} max {max(je):.2f})", flush=True)


def bitgleich(a: str, b: str, datei: str) -> bool:
    aus = {}
    for art in (a, b):
        t0 = time.time()
        aus[art] = probe(art, datei, "--per-token")
        print(f"[bitgleich] {Path(art).name}: {len(aus[art])} Zeilen, {time.time() - t0:.0f} s",
              flush=True)
    # 📌 Erst die Laengen: ein `zip` braeche an der kuerzeren Seite ab und
    # zaehlte die fehlenden Zeilen stillschweigend nicht mit.
    if len(aus[a]) != len(aus[b]):
        print(f"[bitgleich] VERSCHIEDEN: {len(aus[a])} gegen {len(aus[b])} Zeilen", flush=True)
        return False
    gleich = sum(x == y for x, y in zip(aus[a], aus[b]))
    urteil = "IDENTISCH" if gleich == len(aus[a]) else "VERSCHIEDEN"
    print(f"[bitgleich] {urteil}: {gleich}/{len(aus[a])} Zeilen gleich", flush=True)
    return urteil == "IDENTISCH"


def main() -> None:
    args = [x for x in sys.argv[1:] if x != "--bitgleich"]
    nur_bitgleich = "--bitgleich" in sys.argv[1:]
    if len(args) < 3:
        print(__doc__)
        sys.exit(2)
    n, laenge, artefakte = int(args[0]), int(args[1]), args[2:]
    if nur_bitgleich and len(artefakte) != 2:
        print("FEHLER: --bitgleich vergleicht genau zwei Artefakte")
        sys.exit(2)
    datei = folgendatei(n, laenge)
    print(f"[vergleich] {n} Sequenzen a {laenge} Token, Probe {PROBE}", flush=True)
    try:
        if nur_bitgleich:
            ok = bitgleich(artefakte[0], artefakte[1], datei)
        else:
            perplexitaet(artefakte, datei)
            ok = True
    finally:
        Path(datei).unlink(missing_ok=True)
    print("Fertig")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
