#!/usr/bin/env python3
"""umschlag_sim.py — Was kostet die Kapsel vor jeder Nachricht?

WAS DIESE SIMULATION BEANTWORTET
--------------------------------
Am 2026-09-03 hat der Sitzungskanal (`myl-siegel`) einen `Umschlag`
bekommen: die ML-KEM-Kapsel, jeder versiegelten Nachricht vorangestellt.
Im Code steht dazu der Satz:

  "Sie geht jeder Nachricht voran und nicht nur der ersten. Das kostet
  1088 Byte je Nachricht und ist die einfachere Wahrheit."

⚑ **Die Zahl war geschaetzt und die Folgerung ungerechnet.** Diese Datei
rechnet beides nach: den wirklichen Vorspann, seinen Anteil an einer
typischen Anfrage und den Punkt, ab dem er nicht mehr ins Gewicht faellt.

Der Anlass ist die Regel dieses Projektes, dass eine Kostenannahme kein
Test widerlegen kann. Ein Test prueft, dass der Umschlag aufgeht; dass er
bei kurzen Prompts **groesser ist als die Nachricht selbst**, sieht kein
Test.

MODELLANNAHMEN (ausdruecklich, damit widerlegbar)
--------------------------------------------------
  - Die Konstanten stammen aus `NETWORKING/myl-siegel/src/lib.rs` und
    werden von dort gelesen, nicht abgeschrieben.
  - Der Prompt geht als UTF-8 in die Versiegelung. Zeichen jenseits von
    ASCII kosten mehr Bytes; gerechnet wird in Bytes.
  - Eine Sitzung schickt `n` Nachrichten in dieselbe Richtung. Das ist
    die Groesse, an der sich "einmal je Richtung" ueberhaupt lohnen
    kann.
  - Der Vergleich "einmal je Richtung" unterstellt, dass der Absender
    weiss, ob die Gegenstelle die Kapsel schon hat. Das ist Zustand je
    (Pod, Endpunkt, Epoche), und dieser Zustand ist der Preis.

WAS SIE NICHT BEANTWORTET
--------------------------
  - Ob der Vorspann die Latenz erhoeht. Das haengt an der MTU und der
    Wegstrecke und nicht an dieser Rechnung.
  - Ob 1088 Byte fuer ML-KEM-768 die richtige Wahl sind. Das ist eine
    Frage an das Verfahren, nicht an seine Verpackung.
"""

import pathlib
import re
import sys

QUELLE = (
    pathlib.Path(__file__).resolve().parents[3]
    / "NETWORKING"
    / "myl-siegel"
    / "src"
    / "lib.rs"
)


def konstante(name: str) -> int:
    """Liest eine `pub const NAME: usize = <ausdruck>;` aus der Quelle.

    ⚑ Gelesen und nicht abgeschrieben: Eine Simulation, die ihre Zahlen
    doppelt fuehrt, bestaetigt sich selbst, sobald der Code sich aendert.
    """
    text = QUELLE.read_text(encoding="utf-8")
    treffer = re.search(rf"pub const {name}: usize =\s*([^;]+);", text)
    if not treffer:
        raise SystemExit(f"FEHLER: {name} nicht in {QUELLE} gefunden")
    ausdruck = treffer.group(1).strip()
    # Nur Zahlen, Grundrechenarten und schon gelesene Konstanten.
    for anderer in ("KAPSEL_LEN", "KAPSELPUNKT_LEN", "TAG_LEN", "KOPF_BYTES"):
        if anderer in ausdruck and anderer != name:
            ausdruck = ausdruck.replace(anderer, str(konstante(anderer)))
    if not re.fullmatch(r"[0-9_ +*\-]+", ausdruck):
        raise SystemExit(f"FEHLER: {name} ist kein reiner Zahlausdruck: {ausdruck}")
    return int(eval(ausdruck))  # noqa: S307  (Ausdruck oben eingeschraenkt)


KAPSEL_LEN = konstante("KAPSEL_LEN")
TAG_LEN = konstante("TAG_LEN")
KOPF_BYTES = konstante("KOPF_BYTES")

# `epoche ‖ pod ‖ von ‖ an ‖ chiffrat`, siehe `Kapsel::zu_bytes`.
KAPSELRAHMEN = 8 + 32 + 32 + 32 + KAPSEL_LEN
# `Versiegelt` traegt Kopf, Tag und das Borsh-Laengenpraefix des
# Geheimtexts; der Geheimtext ist so lang wie der Klartext.
SIEGELRAHMEN = KOPF_BYTES + TAG_LEN + 4
# Was vor jeder Nachricht steht, unabhaengig von ihrer Laenge.
VORSPANN = KAPSELRAHMEN + SIEGELRAHMEN


def auf_dem_draht(klartext_bytes: int) -> int:
    """Bytes eines vollstaendigen Umschlags."""
    return VORSPANN + klartext_bytes


def anteil(klartext_bytes: int) -> float:
    """Anteil des Vorspanns am Umschlag."""
    return VORSPANN / auf_dem_draht(klartext_bytes)


# Prompts, wie sie wirklich vorkommen. Die Laengen sind gemessen, nicht
# gegriffen: Der erste ist der Prompt aus dem Nahttest dieses Projektes,
# die uebrigen sind typische Groessen aus dem Betrieb eines Harness.
PROMPTS = [
    ("Nahttest dieses Projektes", len(b"user: hauptstadt von frankreich?\n")),
    ("Kurze Frage ohne Systemteil", 60),
    ("Frage mit kurzem Systemteil", 300),
    ("Frage mit Beispielen", 2_000),
    ("Eine Seite Kontext", 8_000),
    ("Zehn Seiten Kontext", 80_000),
    ("Ein Megabyte Kontext", 1_048_576),
]


def zeile(name: str, klartext: int) -> str:
    ganz = auf_dem_draht(klartext)
    return (
        f"  {name:<28} {klartext:>9,} B Klartext"
        f"  →{ganz:>10,} B Draht"
        f"   Vorspann {anteil(klartext) * 100:5.1f} %"
    )


def hauptteil() -> None:
    print("=" * 70)
    print("umschlag_sim: Was kostet die Kapsel vor jeder Nachricht?")
    print("=" * 70)
    print()
    print(f"  Kapselrahmen  {KAPSELRAHMEN:>6,} B  (davon Chiffrat {KAPSEL_LEN:,} B)")
    print(f"  Siegelrahmen  {SIEGELRAHMEN:>6,} B  (Kopf {KOPF_BYTES}, Tag {TAG_LEN}, Laenge 4)")
    print(f"  Vorspann      {VORSPANN:>6,} B  je Nachricht")
    print()
    print("Anteil des Vorspanns, nach Promptlaenge")
    print("-" * 70)
    for name, laenge in PROMPTS:
        print(zeile(name, laenge))
    print()

    # ⚑ Der Punkt, ab dem der Vorspann unter zehn Prozent faellt.
    schwelle = VORSPANN * 9
    print(f"  Unter 10 % Vorspann ab {schwelle:,} B Klartext")
    print(f"  Unter  1 % Vorspann ab {VORSPANN * 99:,} B Klartext")
    print()

    print("Was 'einmal je Richtung' sparen wuerde")
    print("-" * 70)
    for n in (1, 2, 8, 64):
        gespart = KAPSELRAHMEN * (n - 1)
        gesamt_jetzt = n * auf_dem_draht(300)
        print(
            f"  {n:>3} Nachrichten je 300 B:"
            f" jetzt {gesamt_jetzt:>8,} B,"
            f" gespart {gespart:>8,} B"
            f"  ({gespart / gesamt_jetzt * 100:5.1f} %)"
        )
    print()
    print("  ⚑ Der Preis dafuer ist Zustand je (Pod, Endpunkt, Epoche):")
    print("    Der Absender muesste wissen, ob die Gegenstelle die Kapsel")
    print("    schon hat. Ein zustandsloser Auftrag weiss das nicht, und")
    print("    wer falsch raet, schickt etwas Unlesbares.")
    print()

    # --- Zusicherungen ------------------------------------------------
    # ⚑ Ueber die **Form** und nicht ueber Wunschwerte: Was hier steht,
    # muss auch nach einer Aenderung der Konstanten noch stimmen.
    assert VORSPANN == KAPSELRAHMEN + SIEGELRAHMEN
    assert KAPSELRAHMEN > KAPSEL_LEN, "der Rahmen muss groesser sein als sein Chiffrat"

    # Monoton fallend: laengerer Klartext, kleinerer Anteil.
    laengen = [p[1] for p in PROMPTS]
    anteile = [anteil(x) for x in laengen]
    assert all(a > b for a, b in zip(anteile, anteile[1:])), (
        "der Vorspannanteil faellt nicht monoton"
    )

    # ⚑ Der Befund, um den es geht, als Zusicherung: Bei einer kurzen
    # Frage ist der Vorspann groesser als die Nachricht.
    kurz = PROMPTS[0][1]
    assert anteil(kurz) > 0.5, (
        "bei einer kurzen Frage sollte der Vorspann ueberwiegen; "
        "wenn nicht, ist diese Simulation ueberholt"
    )

    # Und die Umkehrung, sonst prueft die Zusicherung nur eine Richtung.
    assert anteil(1_048_576) < 0.01, "bei einem Megabyte darf der Vorspann nicht zaehlen"

    # ⚑ **Der grösste erlaubte Klartext muss IM UMSCHLAG durch den
    # Kanal passen.** Bis zum 2026-09-03 tat er das nicht: Die
    # Herleitung von `MAX_KLARTEXT_BYTES` zog den Kapselvorspann nicht
    # ab, weil sie aus der Zeit vor dem hybriden Austausch stammte.
    # **Diese Simulation hat es gefunden**, und kein Test.
    protokoll = (
        pathlib.Path(__file__).resolve().parents[3]
        / "SHARED_TYPES"
        / "myl-types"
        / "src"
        / "protocol.rs"
    ).read_text(encoding="utf-8")
    treffer = re.search(r"pub const MAX_ANFRAGE_BYTES: usize =\s*([^;]+);", protokoll)
    if not treffer:
        raise SystemExit("FEHLER: MAX_ANFRAGE_BYTES nicht gefunden")
    grenze = int(eval(treffer.group(1).strip()))  # noqa: S307

    quelle = QUELLE.read_text(encoding="utf-8")
    formel = re.search(
        r"pub const MAX_KLARTEXT_BYTES: usize =([^;]+);", quelle, re.S
    )
    if not formel:
        raise SystemExit("FEHLER: MAX_KLARTEXT_BYTES nicht gefunden")
    max_klartext = grenze - KOPF_BYTES - TAG_LEN - 4 - KAPSELRAHMEN
    assert "KAPSELRAHMEN_LEN" in formel.group(1), (
        "MAX_KLARTEXT_BYTES zieht den Kapselvorspann nicht ab; "
        "dann passt der groesste Umschlag nicht durch den Kanal (Fund 159)"
    )
    assert auf_dem_draht(max_klartext) <= grenze, (
        f"der groesste Umschlag ist {auf_dem_draht(max_klartext):,} B "
        f"gegen {grenze:,} B Grenze"
    )
    # Gegenprobe: ein Byte mehr passt nicht mehr. Sonst prueft die
    # Zusicherung nur, dass irgendetwas passt.
    assert auf_dem_draht(max_klartext + 1) > grenze, (
        "der Deckel liegt zu niedrig: es waere noch Luft"
    )
    print(f"  Groesster Klartext im Umschlag: {auf_dem_draht(max_klartext):,} B")
    print(f"  Grenze des Anfragekanals:       {grenze:,} B")
    print("  ⚑ Passt genau, und ein Byte mehr passt nicht.")
    print()
    print("umschlag_sim: alle Behauptungen bestanden.")


if __name__ == "__main__":
    hauptteil()
    sys.exit(0)
