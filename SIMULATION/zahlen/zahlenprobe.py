#!/usr/bin/env python3
"""Haelt die Simulationen aus Anhang B an ihren Zahlen.

Die meisten Programme unter README/Whitepaper/simulations drucken ihr
Ergebnis nur; "laeuft" sagte bisher nichts darueber, ob dasselbe herauskommt.
Zwei Pruefungen je Programm:

1. Die Ausgabe gleicht der Golden-Datei daneben, Zeile fuer Zeile.
2. ANHANG_B: Jede Zahl, die der Anhang nennt, steht wirklich im Whitepaper
   UND in der Ausgabe. So kann auch ein Neuschreiben der Golden-Dateien
   (--neu) nicht unbemerkt vom veroeffentlichten Text wegwandern.

Aufruf:  python3 SIMULATION/zahlen/zahlenprobe.py [--ohne-langsame] [--neu]
Nur Standardbibliothek. Rueckgabe 0 heisst: alles gleich.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

HIER = pathlib.Path(__file__).resolve().parent
WURZEL = HIER.parents[1]
PROGRAMME = WURZEL / "README" / "Whitepaper" / "simulations"
WHITEPAPER = WURZEL / "README" / "Whitepaper" / "myelith-whitepaper-v0.3.md"

LANGSAM = {"topk_stability_sim.py", "robustness_sim.py"}

# Programm -> (Stelle im Anhang, Wortlaut im Whitepaper, Wortlaut in der Ausgabe)
ANHANG_B = {
    "integer_determinism_sim.py": [
        ("B.5.1", "in allen 200 Durchläufen", "INTEGER : 200/200 identisch"),
        ("B.5.1", "nur in 9 von 200 Fällen", "GLEITKOMMA : 9/200 identisch"),
        ("B.5.1", "in 96 Prozent der Fälle", "(96% der Faelle divergent)"),
        ("B.5.2", "über 133.000 Terme", "133,144 Terme"),
        ("B.5.2", "rund 1,3 Millionen", "1,309,836"),
        ("B.5.2", "Faktor 1.639", "Faktor 1,639"),
        ("B.5.3", "in 100 von 100 Fällen", "100/100 identisch"),
        ("B.5.3", "in 200 von 200 Fällen", "Elementreihenfolge: 200/200"),
        ("B.5.4", "In drei von fünf geprüften Fällen", "In 3 von 5 Faellen weichen die Verfahren ab"),
        ("B.5.4", "Über 100.000 Zufallsfälle", "Shift = Floor in 100,000/100,000 Zufallsfaellen"),
    ],
    "tau_sim.py": [
        ("B.5.6", "mindestens das Fünffache", "Mindest-Trennung fuer Tragfaehigkeit: SEP >= 5x"),
    ],
    "robustness_sim.py": [
        ("B.5.6", "Zwanzig- bis Fünfunddreißigfache", "heavy: SEP >= 20x"),
        ("B.5.6", "Zwanzig- bis Fünfunddreißigfache", "2 % Ausreisser: SEP >= 35x"),
    ],
    "integer_training_sim.py": [
        ("B.6.1", "Über 200 Durchläufe", "200/200 identisch"),
        ("B.6.2", "bis auf 78 Bit", "78 UEBERLAUF"),
        ("B.6.2", "bei etwa fünfzehn Bit", "40 17,161 15"),
    ],
    "training_capacity_sim.py": [
        ("B.6.3", "rund 98 Millionen Token", "97,500,000"),
        ("B.6.3", "etwa eine Milliarde", "1,012,500,000"),
        ("B.6.3", "rund neun Milliarden", "9,000,000,000"),
        ("B.6.4", "960 Byte gegenüber 8.192 Byte", "Beweisgroesse je Segment: 960 Byte"),
        ("B.6.4", "11,7 Prozent", "11.72%"),
        ("B.6.4", "832 Byte gegenüber 128 Kilobyte", "16 832B 131,072B"),
        ("B.6.4", "0,63 Prozent", "0.63%"),
        ("B.6.4", "704 Byte gegenüber 2 Megabyte", "256 704B 2,097,152B"),
        ("B.6.4", "0,03 Prozent", "0.03%"),
        ("B.6.5", "auf etwa zwei Prozent", "40% 40.0% 2.0%"),
    ],
    "training_integrity_sim.py": [
        ("B.6.6", "um 0,76 ab", "5% 0.760"),
        ("B.6.6", "um 3,80", "20% 3.801"),
        ("B.6.6", "(0,19)", "33% 6.081 0.187"),
        ("B.6.6", "(3,91)", "0.187 3.913"),
        ("B.6.7", "bis zu fünfzig Schritten", "50 0.0036 konvergiert"),
        ("B.6.8", "etwa 35 Prozent", "ohne echten Fortschritt: 35%"),
        ("B.6.9", "auf etwa vierzig Prozent", "0% 40% 100%"),
        ("B.6.9", "auf sechzig Prozent", "5% 60% 96%"),
        ("B.6.9", "auf zweiundachtzig Prozent", "15% 82% 88%"),
    ],
    "model_growth_sim.py": [
        ("B.6.10", "etwa 200 Milliarden Token", "Schritt 1 32B 640B 200B 3.2x"),
        ("B.6.10", "640 Milliarden", "Schritt 1 32B 640B 200B 3.2x"),
        ("B.6.10", "1 zu 3,2", "200B 3.2x"),
        ("B.6.10", "etwa 263 Tage", "5.000 Miner 10% 263d"),
        ("B.6.10", "rund dreißig Tage", "50.000 Miner 10% 30d"),
        ("B.6.10", "auf 98 Schichten", "100B 98 10 12"),
    ],
    "training_tokenomics_sim.py": [
        ("B.7.1", "von 11,8 Prozent", "nur Inferenz 11.81%"),
        ("B.7.1", "auf 23,0 Prozent", "Zusatzpraegung (10 %) 23.00%"),
        ("B.7.1", "auf 11,76 Prozent", "Gebuehrenaufschlag (5 %) 11.76%"),
    ],
}


def eingedampft(text: str) -> str:
    """Tabellenbreiten sollen die Suche nach einem Wortlaut nicht stoeren."""
    return " ".join(text.split())


def ausfuehren(programm: pathlib.Path) -> str:
    lauf = subprocess.run(
        [sys.executable, "-I", programm.name],
        cwd=PROGRAMME, capture_output=True, text=True, encoding="utf-8",
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "PYTHONIOENCODING": "utf-8", "PYTHONUTF8": "1"},
    )
    if lauf.returncode != 0:
        raise RuntimeError(f"Rueckgabe {lauf.returncode}: {lauf.stderr[-400:]}")
    return lauf.stdout


def erste_abweichung(ist: str, soll: str) -> str | None:
    a, b = ist.splitlines(), soll.splitlines()
    for nummer, (x, y) in enumerate(zip(a, b), 1):
        if x != y:
            return f"Zeile {nummer}:\n    soll: {y}\n    ist:  {x}"
    if len(a) != len(b):
        return f"{len(a)} Zeilen statt {len(b)}"
    return None


def anhang_pruefen(name: str, ausgabe: str, whitepaper: str) -> list[str]:
    fehler = []
    ausgabe = eingedampft(ausgabe)
    for stelle, im_text, in_ausgabe in ANHANG_B.get(name, []):
        if im_text not in whitepaper:
            fehler.append(f"{stelle}: nicht im Whitepaper: {im_text!r}")
        if in_ausgabe not in ausgabe:
            fehler.append(f"{stelle}: nicht in der Ausgabe: {in_ausgabe!r} (Anhang: {im_text!r})")
    return fehler


def main() -> int:
    ohne_langsame = "--ohne-langsame" in sys.argv
    neu = "--neu" in sys.argv
    unbekannt = set(sys.argv[1:]) - {"--ohne-langsame", "--neu"}
    if unbekannt:
        print(f"unbekannte Angabe: {sorted(unbekannt)}", file=sys.stderr)
        return 2
    whitepaper = eingedampft(WHITEPAPER.read_text(encoding="utf-8"))
    programme = sorted(PROGRAMME.glob("*.py"))
    namen = {p.name for p in programme}
    fehler = []
    for name in sorted(set(ANHANG_B) - namen):
        fehler.append(f"{name}: steht in ANHANG_B, das Programm fehlt")
    for golden in sorted(HIER.glob("*.golden")):
        if golden.stem not in namen:
            fehler.append(f"{golden.name}: Golden-Datei ohne Programm")
    for programm in programme:
        name = programm.name
        if ohne_langsame and name in LANGSAM:
            print(f"  uebersprungen (langsam): {name}")
            continue
        golden = HIER / f"{name}.golden"
        try:
            ausgabe = ausfuehren(programm)
        except RuntimeError as grund:
            fehler.append(f"{name}: {grund}")
            continue
        if neu:
            golden.write_text(ausgabe, encoding="utf-8")
        if not golden.exists():
            fehler.append(f"{name}: keine Golden-Datei (--neu legt sie an)")
            continue
        abweichung = erste_abweichung(ausgabe, golden.read_text(encoding="utf-8"))
        if abweichung:
            fehler.append(f"{name}: weicht ab, {abweichung}")
        fehler += [f"{name}: {f}" for f in anhang_pruefen(name, ausgabe, whitepaper)]
        print(f"  geprueft: {name}")
    for f in fehler:
        print(f"FEHLER {f}", file=sys.stderr)
    if fehler:
        print(f"Zahlenprobe: {len(fehler)} Abweichungen", file=sys.stderr)
        return 1
    print(f"Zahlenprobe: {len(programme)} Programme, {sum(map(len, ANHANG_B.values()))} Zahlen aus Anhang B")
    return 0


if __name__ == "__main__":
    sys.exit(main())
