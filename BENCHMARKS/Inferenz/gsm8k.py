#!/usr/bin/env python3
"""
GSM8K-Teilmenge gegen ein Myelith-Artefakt: gierig, ohne Denkmodus.

⚑ **Warum neben der Perplexitaet eine Aufgabenmessung:** Perplexitaet
verdeckt den Zusammenbruch beim Rechnen. Ein quantisiertes Modell kann auf
Fliesstext fast gleich messen und beim schrittweisen Rechnen die Haelfte
verlieren. Diese Messung fragt deshalb Aufgaben mit einer Zahl als Antwort.

Aufruf aus der Wurzel mit dem Kalibrier-venv:

    V=INTEGER_LLM/calibrate/.venv/bin/python3
    $V -u BENCHMARKS/Inferenz/gsm8k.py <hf_ordner> <artefakt> <n> <max_token> <ausgabe.jsonl>

- `<hf_ordner>`: Ordner des Originalmodells unter `MODELS/llm/`; von dort
  kommt **nur** die Chatvorlage, damit jedes Modell seine eigene bekommt.
- `<n>`: die ersten `n` Aufgaben des Testsplits, feste Reihenfolge.
- `<max_token>`: Obergrenze der Antwort; 512 reicht fuer die meisten.

Die Antwort wird aus dem letzten `\\boxed{...}` gelesen, ersatzweise aus der
letzten Zahl. Jede Antwort steht mit Soll und Ist in `<ausgabe.jsonl>`.

⚠️ **Bei 50 Aufgaben sind zwei, drei Punkte Unterschied Rauschen.** Fuer
eine Aussage auf einen Punkt genau braucht es alle 1 319.

Der Testsplit wird beim ersten Lauf geladen und nach `datasets/`
zwischengespeichert, mit Pruefsumme. Am Ende steht `Fertig`.
"""

import hashlib
import json
import re
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

HIER = Path(__file__).resolve().parent
WURZEL = HIER.parents[1]
sys.path.insert(0, str(WURZEL / "INTEGER_LLM" / "tests"))
from cargo_paths import binary  # noqa: E402

PROBE = binary("runtime", "aufgabenprobe")
DATEI = HIER / "datasets" / "gsm8k_test.jsonl"
QUELLE = ("https://raw.githubusercontent.com/openai/grade-school-math/master/"
          "grade_school_math/data/test.jsonl")
# Gemessen am 2026-09-28 an der Datei, mit der die ersten Messungen liefen.
PRUEFSUMME = "3730d312f6e3440559ace48831e51066acaca737f6eabec99bccb9e4b3c39d14"


def testsplit() -> list:
    if not DATEI.exists():
        print(f"[gsm8k] Lade den Testsplit von {QUELLE}", flush=True)
        daten = urllib.request.urlopen(QUELLE, timeout=60).read()
        DATEI.parent.mkdir(parents=True, exist_ok=True)
        DATEI.write_bytes(daten)
    ist = hashlib.sha256(DATEI.read_bytes()).hexdigest()
    if ist != PRUEFSUMME:
        # 📌 Eine andere Fassung des Testsplits waere eine andere Messung;
        # die Zahlen liessen sich mit keiner frueheren vergleichen.
        print(f"FEHLER: Pruefsumme von {DATEI} ist {ist}, erwartet {PRUEFSUMME}", flush=True)
        sys.exit(1)
    return [json.loads(z) for z in DATEI.read_text(encoding="utf-8").splitlines() if z.strip()]


def zahl(s: str):
    s = s.replace(",", "").replace("$", "").strip()
    treffer = re.findall(r"-?\d+(?:\.\d+)?", s)
    return float(treffer[-1]) if treffer else None


def antwort_aus(text: str):
    kisten = re.findall(r"\\boxed\{([^{}]*)\}", text)
    if kisten:
        return zahl(kisten[-1])
    return zahl(text)


def main() -> None:
    if len(sys.argv) != 6:
        print(__doc__)
        sys.exit(2)
    hf, artefakt = sys.argv[1], sys.argv[2]
    n, max_token, ausgabe = int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
    from transformers import AutoTokenizer

    aufgaben = testsplit()[:n]
    tok = AutoTokenizer.from_pretrained(hf)
    zeilen = []
    for a in aufgaben:
        inhalt = (a["question"] + "\nPlease reason step by step, and put your final answer "
                  "within \\boxed{}.")
        p = tok.apply_chat_template([{"role": "user", "content": inhalt}], tokenize=False,
                                    add_generation_prompt=True, enable_thinking=False)
        # Die Probe liest eine Aufgabe je Zeile und `\n` als Umbruch; ein
        # woertliches `\n` im Prompt wuerde verfaelscht.
        assert "\\n" not in p, "woertliches \\n im Prompt"
        zeilen.append(p.replace("\n", "\\n"))
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write("\n".join(zeilen) + "\n")
        datei = f.name
    print(f"[gsm8k] {n} Aufgaben, Vorlage aus {hf}, Artefakt {artefakt}", flush=True)
    print("[gsm8k] Beispielprompt:", repr(zeilen[0][:300]), flush=True)

    t0 = time.time()
    richtig = gesamt = token = 0
    with subprocess.Popen([str(PROBE), artefakt, datei, str(max_token)],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as proz, \
            open(ausgabe, "w", encoding="utf-8") as aus:
        for zeile in proz.stdout:
            e = json.loads(zeile)
            a = aufgaben[e["nr"]]
            soll = zahl(a["answer"].split("####")[-1])
            ist = antwort_aus(e["antwort"])
            ok = ist is not None and soll is not None and abs(ist - soll) < 1e-6
            richtig += ok
            gesamt += 1
            token += e["token"]
            e.update(soll=soll, ist=ist, richtig=ok)
            aus.write(json.dumps(e, ensure_ascii=False) + "\n")
            aus.flush()
            print(f"[gsm8k] {gesamt}/{n}: {'richtig' if ok else 'falsch '} soll {soll} ist {ist} "
                  f"({e['token']} Token, {e['sekunden']:.0f} s) | bisher {richtig}/{gesamt}",
                  flush=True)
        fehler = proz.stderr.read()
    Path(datei).unlink(missing_ok=True)
    if gesamt != n:
        print(f"FEHLER: nur {gesamt} von {n} Antworten\n{fehler[-2000:]}", flush=True)
        sys.exit(1)
    print(f"[gsm8k] Ergebnis: {richtig}/{n} = {100 * richtig / n:.1f} %, "
          f"im Mittel {token / n:.0f} Token, {time.time() - t0:.0f} s gesamt", flush=True)
    print("Fertig")


if __name__ == "__main__":
    main()
