"""Wertet die Messreihe aus: je Sensor Anzahl, Mittelwert, Minimum, Maximum."""
import csv
import os

QUELLE = os.path.join(os.path.dirname(__file__), "messwerte.csv")
ZIEL = os.path.join(os.path.dirname(__file__), "..", "ergebnis", "statistik.md")


def lesen():
    werte = {}
    with open(QUELLE, newline="", encoding="utf-8") as f:
        for zeile in csv.DictReader(f):
            sensor = zeile["sensor"]
            werte.setdefault(sensor, []).append(float(zeile["temperatur"]))
    return werte


def schreiben(werte):
    zeilen = [
        "| Sensor | Anzahl | Mittelwert | Minimum | Maximum |",
        "|---|---|---|---|---|",
    ]
    for sensor, w in sorted(werte.items()):
        zeilen.append(f"| {sensor} | {len(w)} | {sum(w) / len(w):.1f} | {min(w):.1f} | {max(w):.1f} |")
    with open(ZIEL, "w", encoding="utf-8") as f:
        f.write("\n".join(zeilen) + "\n")


if __name__ == "__main__":
    schreiben(lesen())
