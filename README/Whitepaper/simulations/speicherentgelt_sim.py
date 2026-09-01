#!/usr/bin/env python3
"""speicherentgelt_sim.py — Der Satz fuer Speicher, hergeleitet statt gesetzt.

WAS DIESE SIMULATION IST UND WAS NICHT
--------------------------------------
⚑ **Sie setzt den Satz nicht.** Er ist ein Governance-Parameter und eine
Festlegung des Projektinhabers; das steht so im Modulkopf von
`myl_store::entgelt` und bleibt so.

**Sie macht ihn entscheidbar.** Offen war er, weil niemand eine Zahl
hatte, an der man ihn festmachen kann. Diese Datei liefert die Zahl,
ihre Annahmen und ihre Empfindlichkeit.

DIE FORM DER ANTWORT, UND SIE IST WICHTIGER ALS DIE ZAHL
---------------------------------------------------------
⚑ **Der Satz sollte kein eigener Parameter sein, sondern ein Verhaeltnis
zum Credit-Preis.**

Der Credit-Preis fuer Rechenarbeit ist **dynamisch**: `P_{e+1} = P_e ·
exp(kappa·(u_e − u*))` (Kap. 5). Ein fester Speichersatz daneben liefe
ihm davon: Steigt der Rechenpreis um den Faktor drei und der Speicherpreis
nicht, dann ist Speicher ploetzlich zu einem Drittel subventioniert, ohne
dass jemand etwas entschieden haette.

**Ein Verhaeltnis kann nicht auseinanderlaufen.** Zu entscheiden bleibt
dann eine Zahl statt zweier beweglicher Groessen: **wie viele
Byte-Epochen dieselben realen Kosten haben wie eine Recheneinheit.**

MODELLANNAHMEN (ausdruecklich, damit widerlegbar)
--------------------------------------------------
  - Eine Epoche ist eine Stunde (Kap. 4.3).
  - Speicher: Festplatte im Dauerbetrieb, Anschaffung ueber die
    Lebensdauer abgeschrieben, plus Strom. **Kein Cloud-Preis**, denn
    ein Miner haelt eigene Hardware; ein Cloud-Preis enthielte Marge und
    Verfuegbarkeitszusagen, die hier niemand kauft.
  - Rechnen: Beschleuniger im Dauerbetrieb, Anschaffung plus Strom, auf
    ganzzahlige Operationen bezogen. Auch hier kein Mietpreis.
  - ⚑ **Beide Seiten mit derselben Methode**, sonst vergliche man einen
    Kaufpreis mit einer Miete.

Nur Standardbibliothek. Aufruf: python3 speicherentgelt_sim.py
"""

# ─── Annahmen: Speicher ──────────────────────────────────────────────────────
PLATTE_BYTES = 20e12       # 20 TB
PLATTE_PREIS = 250.0       # Anschaffung, in Waehrungseinheiten
PLATTE_JAHRE = 5.0         # Lebensdauer im Dauerbetrieb
PLATTE_WATT = 5.0          # Leistungsaufnahme im Betrieb
STROM_PREIS = 0.15         # je kWh

# ─── Annahmen: Rechnen ───────────────────────────────────────────────────────
KARTE_PREIS = 25_000.0     # Anschaffung eines Beschleunigers
KARTE_JAHRE = 4.0
KARTE_WATT = 700.0
KARTE_OPS = 1.0e15         # ganzzahlige Operationen je Sekunde

STUNDEN_JE_JAHR = 24 * 365


def kosten_je_byte_epoche() -> float:
    """Was eine Byte-Epoche real kostet, in Waehrungseinheiten."""
    stunden = PLATTE_JAHRE * STUNDEN_JE_JAHR
    abschreibung = PLATTE_PREIS / stunden          # je Stunde, ganze Platte
    strom = PLATTE_WATT / 1000.0 * STROM_PREIS     # je Stunde
    return (abschreibung + strom) / PLATTE_BYTES


def kosten_je_operation() -> float:
    """Was eine ganzzahlige Operation real kostet."""
    stunden = KARTE_JAHRE * STUNDEN_JE_JAHR
    abschreibung = KARTE_PREIS / stunden
    strom = KARTE_WATT / 1000.0 * STROM_PREIS
    ops_je_stunde = KARTE_OPS * 3600.0
    return (abschreibung + strom) / ops_je_stunde


def operationen_je_byte_epoche() -> float:
    """⚑ **Die gesuchte Zahl:** Wie viele Rechenoperationen kosten so viel
    wie **eine Byte-Epoche**.

    ⛑ **Die erste Fassung fragte andersherum** und behauptete, Rechnen
    sei teurer als ein Byte fuer eine Stunde. Der Test fiel um, und er
    hatte unrecht, nicht die Rechnung: Ein Beschleuniger liefert rund
    `1e18` Operationen je Stunde, waehrend ein Byte eine Stunde lang
    nichts tut ausser dazuliegen. **Speicher ist teuer, gemessen in
    Recheneinheiten**, und genau deshalb ist Fund 106 („der Knoten haelt
    dicht und wird duenn bezahlt") kein Rundungsfehler.
    """
    return kosten_je_byte_epoche() / kosten_je_operation()


def main() -> int:
    b = kosten_je_byte_epoche()
    o = kosten_je_operation()
    v = operationen_je_byte_epoche()

    print("speicherentgelt_sim: der Satz, hergeleitet statt gesetzt")
    print()
    print("(1) Reale Kosten unter den Annahmen im Kopf:")
    print(f"    eine Byte-Epoche:      {b:.3e}")
    print(f"    eine Rechenoperation:  {o:.3e}")
    print()
    print("(2) ⚑ Das Verhaeltnis, und das ist der Vorschlag:")
    print(f"    **1 Byte-Epoche == {v:,.0f} Recheneinheiten**")
    print("    Wer eine MYL verbrennt und dafuer N Recheneinheiten bekommt,")
    print(f"    bekommt fuer dieselbe MYL N/{v:,.0f} Byte-Epochen.")
    print()
    print("    ⚑ Und das ist die eigentliche Aussage: **Speicher ist teuer,")
    print("      gemessen in Recheneinheiten.** Ein Beschleuniger liefert rund")
    print("      1e18 Operationen je Stunde; ein Byte liegt eine Stunde lang da.")
    print("      Deshalb ist Fund 106 kein Rundungsfehler.")
    assert v > 100.0, (
        "eine Byte-Epoche muesste deutlich mehr kosten als eine Operation; "
        f"gerechnet wurden {v:.1f}"
    )

    # ── (3) Empfindlichkeit ──────────────────────────────────────────────
    #
    # ⚑ Eine Zahl ohne Empfindlichkeit ist eine Behauptung. Geprueft wird,
    # wie weit sie sich bewegt, wenn die Annahmen sich bewegen.
    print()
    print("(3) Empfindlichkeit gegen die Annahmen:")
    global PLATTE_PREIS, KARTE_PREIS, KARTE_OPS, STROM_PREIS
    grund = (PLATTE_PREIS, KARTE_PREIS, KARTE_OPS, STROM_PREIS)
    spanne = []
    for name, setzen in [
        ("Platte halb so teuer", lambda: ("PLATTE_PREIS", PLATTE_PREIS / 2)),
        ("Platte doppelt so teuer", lambda: ("PLATTE_PREIS", PLATTE_PREIS * 2)),
        ("Karte halb so teuer", lambda: ("KARTE_PREIS", KARTE_PREIS / 2)),
        ("Karte doppelt so schnell", lambda: ("KARTE_OPS", KARTE_OPS * 2)),
        ("Strom dreifach", lambda: ("STROM_PREIS", STROM_PREIS * 3)),
    ]:
        feld, wert = setzen()
        alt = globals()[feld]
        globals()[feld] = wert
        w = operationen_je_byte_epoche()
        globals()[feld] = alt
        spanne.append(w)
        print(f"    {name:26} {w:12,.0f}  ({w / v:4.2f}-fach)")
    PLATTE_PREIS, KARTE_PREIS, KARTE_OPS, STROM_PREIS = grund

    faktor = max(spanne) / min(spanne)
    print(f"    ⚑ Ueber alle Faelle: Faktor {faktor:.1f} zwischen groesstem und kleinstem.")
    assert faktor < 20.0, (
        f"die Zahl schwankt um Faktor {faktor:.1f}; dann traegt sie keine Entscheidung"
    )

    # ── (4) Warum kein fester Satz ───────────────────────────────────────
    print()
    print("(4) ⚑ Warum ein Verhaeltnis und kein fester Satz:")
    print("    Der Credit-Preis folgt P_{e+1} = P_e · exp(kappa·(u−u*)).")
    for schritte, faktor_p in [(10, 1.1), (50, 1.1)]:
        gewachsen = faktor_p ** (schritte / 10)
        print(f"    Nach {schritte:2d} Epochen mit +10 %/10 Epochen: Rechenpreis "
              f"{gewachsen:.2f}-fach.")
    print("    Ein fester Speichersatz waere dann um denselben Faktor")
    print("    subventioniert, **ohne dass jemand etwas entschieden haette**.")
    print("    Ein Verhaeltnis kann nicht auseinanderlaufen.")

    print()
    print("(5) Was hier NICHT entschieden wird:")
    print("    Die zweite Zahl aus Punkt 25, die **Aufnahmeschwelle fuer")
    print("    Netzwerkwissen**. Sie braucht Abrufzaehlung und damit Verkehr;")
    print("    ⚑ eine Schwelle ohne gemessene Abrufe waere geraten.")

    print()
    print("speicherentgelt_sim: alle Behauptungen bestanden.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
