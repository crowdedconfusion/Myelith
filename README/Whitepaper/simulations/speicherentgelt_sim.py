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

import pathlib
import re

# ─── Annahmen: Speicher ──────────────────────────────────────────────────────
#
# ⚑ **Berichtigt am 2026-09-02, und die alte Zahl war an zwei Stellen
# zu guenstig.** Bis dahin stand hier eine 20-TB-Platte fuer 250, also
# 12,50 je TB. Nachrecherchiert: Der Strassenpreis fuer neue
# Enterprise-CMR-Platten liegt im September 2026 bei **22 bis 25 je TB**,
# ein 20-TB-Laufwerk kostet also rund 460. Die Preise sind seit
# September 2025 um rund 50 Prozent gestiegen, weil der Aufbau von
# KI-Infrastruktur die hohen Kapazitaeten aufkauft; 18 Prozent der 2026
# ausgelieferten Exabyte gehen dorthin, fuer 2028 werden 43 Prozent
# erwartet. Eine Rueckkehr auf das Niveau von 2024 gilt vor 2027 als
# unwahrscheinlich.
PLATTE_BYTES = 20e12       # 20 TB
PLATTE_PREIS = 460.0       # Anschaffung, rund 23 je TB (Markt 09/2026)
PLATTE_JAHRE = 5.0         # Lebensdauer im Dauerbetrieb
PLATTE_WATT = 5.0          # Leistungsaufnahme im Betrieb
STROM_PREIS = 0.15         # je kWh

# ⚑ **Und niemand betreibt eine nackte Platte.** Das war der zweite
# Fehler: Das Modell rechnete Anschaffung und Strom der Platte und sonst
# nichts. Eine Platte braucht ein Gehaeuse, einen Rechner, Speicher, eine
# Netzkarte und einen Anschluss, und der Wirt laeuft mit, ob eine Platte
# darin steckt oder zwoelf.
#
# **Zwoelf Platten je Wirt** ist ein uebliches Verhaeltnis fuer ein
# Speichergehaeuse; der Wirt selbst kostet rund 2 000 und zieht rund
# 100 Watt ohne die Platten.
WIRT_PREIS = 2_000.0       # Gehaeuse, Rechner, Speicher, Netz
WIRT_WATT = 100.0          # Grundlast ohne Platten
PLATTEN_JE_WIRT = 12.0

# ─── Annahmen: Rechnen ───────────────────────────────────────────────────────
KARTE_PREIS = 25_000.0     # Anschaffung eines Beschleunigers
KARTE_JAHRE = 4.0
KARTE_WATT = 700.0
KARTE_OPS = 1.0e15         # ganzzahlige Operationen je Sekunde

STUNDEN_JE_JAHR = 24 * 365

# ⚑ **Objektspeicher am Markt, Stand 2026-09**, je TB und Monat. Steht
# hier als **Gegenprobe**, nicht als Grundlage: Ein Anbieter verkauft
# eine Leistung, dieses Netz kauft davon nichts. Siehe Abschnitt 5.
WOLKENPREISE = [
    ("Backblaze B2", 6.95),
    ("Wasabi", 7.99),
    ("Azure Blob Hot", 18.00),
    ("Google Cloud Standard", 20.00),
    ("AWS S3 Standard", 23.00),
]


def satz_aus_dem_code() -> tuple[int, int]:
    """Liest Satz und Kostenboden aus `myl_tokenomics::speicherentgelt`.

    ⚑ **Damit es die Zahl einmal gibt und nicht zweimal.** Ein
    abgeschriebener Wert veraltet still; dieselbe Lehre wie Fund 146,
    wo der Mindeststake ein halbes Jahr lang die Zahl einer alten
    Stichprobenrate trug.
    """
    quelle = (
        pathlib.Path(__file__).resolve().parents[3]
        / "TOKENOMICS" / "myl-tokenomics" / "src" / "speicherentgelt.rs"
    )
    text = quelle.read_text(encoding="utf-8")
    def lies(name: str) -> int:
        treffer = re.search(rf"pub const {name}: u64 = ([0-9_]+);", text)
        assert treffer, f"{name} steht nicht in {quelle}"
        return int(treffer.group(1).replace("_", ""))
    return lies("SPEICHERSATZ_VORGABE"), lies("SPEICHER_KOSTENBODEN")


def kosten_je_byte_epoche(mit_wirt: bool = True) -> float:
    """Was eine Byte-Epoche real kostet, in Waehrungseinheiten.

    ⚑ `mit_wirt=False` rechnet die **nackte Platte**, wie das Modell es
    bis zum 2026-09-02 tat. Der Schalter steht hier, damit der
    Unterschied eine Zahl bekommt statt einer Erinnerung.
    """
    stunden = PLATTE_JAHRE * STUNDEN_JE_JAHR
    anschaffung = PLATTE_PREIS
    watt = PLATTE_WATT
    if mit_wirt:
        anschaffung += WIRT_PREIS / PLATTEN_JE_WIRT
        watt += WIRT_WATT / PLATTEN_JE_WIRT
    abschreibung = anschaffung / stunden           # je Stunde, ganze Platte
    strom = watt / 1000.0 * STROM_PREIS            # je Stunde
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
    print("(5) ⚑ Gegenprobe gegen den Markt: was die Wolke nimmt")
    gb_jahr = 1e9 * STUNDEN_JE_JAHR
    eigen_tb_monat = gb_jahr * kosten_je_byte_epoche() / 12.0 * 1000.0
    nackt_tb_monat = gb_jahr * kosten_je_byte_epoche(mit_wirt=False) / 12.0 * 1000.0
    print(f"    eigene Hardware, mit Wirt:   {eigen_tb_monat:6.2f} je TB-Monat")
    print(f"    eigene Hardware, nackte Platte: {nackt_tb_monat:6.2f} je TB-Monat")
    print("    Objektspeicher am Markt (Stand 2026-09), je TB-Monat:")
    for anbieter, preis in WOLKENPREISE:
        print(f"      {anbieter:24} {preis:6.2f}   Faktor {preis / eigen_tb_monat:5.1f}")
    print()
    print("    ⚑ **Die Wolke ist nicht die Grundlage, und der Abstand ist")
    print("      kein Fehler.** Ein Anbieter verkauft eine Leistung: Redundanz,")
    print("      Rechenzentrum, Bandbreite, Personal, Verfuegbarkeitszusage,")
    print("      Marge. Myelith kauft davon nichts. **Die Redundanz rechnet das")
    print("      Protokoll ohnehin getrennt** (`Manifest::redundanz`), sie")
    print("      steckt also nicht im Satz, sondern in der Bytezahl.")
    print()
    print("      Der Vergleich taugt trotzdem, und zwar als Schranke: Ein Satz")
    print("      **ueber** dem billigsten Marktangebot waere widerlegt, denn")
    print("      dann waere Zukaufen billiger als Selbsthalten.")

    # ⚑ Die Zusicherung ist die Schranke, nicht der Abstand. Eine
    # Wunschzahl fuer den Faktor faellt beim naechsten Preisschritt um;
    # die Richtung nicht.
    billigstes = min(preis for _, preis in WOLKENPREISE)
    assert eigen_tb_monat < billigstes, (
        f"eigene Hardware {eigen_tb_monat:.2f} liegt ueber dem billigsten "
        f"Marktangebot {billigstes:.2f}; dann waere Zukaufen guenstiger"
    )
    # Und in die andere Richtung: Der Wirt muss etwas ausmachen, sonst
    # war seine Aufnahme Kosmetik.
    assert eigen_tb_monat > nackt_tb_monat * 1.2, (
        "das Wirtssystem aendert weniger als ein Fuenftel; dann steht es zu Unrecht drin"
    )

    print()
    print("(6) ⚑ Der entschiedene Satz, gegen den Code gehalten")
    satz, boden = satz_aus_dem_code()
    print(f"    SPEICHERSATZ_VORGABE   = {satz:6}  (entschieden 2026-09-02, Punkt B4)")
    print(f"    SPEICHER_KOSTENBODEN   = {boden:6}  (Kosten eines effizienten Halters)")
    print(f"    hier gerechnete Kosten = {v:6.0f}  (zwoelf Platten je Wirt)")
    tb_monat = 1e12 * 730 * satz * kosten_je_operation()
    print(f"    Satz {satz} ergibt {tb_monat:.2f} je TB-Monat; Storj zahlt 1,50")
    print()
    print("    ⚑ **Die Zahl steht im Code, nicht hier.** Diese Datei rechnet")
    print("      die Grundlage; welcher Satz daraus gewaehlt wurde, ist eine")
    print("      Entscheidung und steht in `myl_tokenomics::speicherentgelt`.")
    print("      Zwei Zahlen fuer dieselbe Aussage liefen auseinander.")

    # ⚑ Die Zusicherungen sind die Beziehungen, nicht die Zahlen.
    assert boden <= v, (
        f"der Kostenboden {boden} liegt ueber den hier gerechneten Kosten "
        f"{v:.0f}; dann schliesst er einen Halter aus, den es gibt"
    )
    assert satz > v, (
        f"der Satz {satz} deckt die gerechneten Kosten {v:.0f} nicht; "
        "dann haelt niemand"
    )
    assert 1.40 <= tb_monat <= 1.60, (
        f"Satz {satz} ergibt {tb_monat:.2f} je TB-Monat; das trifft die "
        "Storj-Rate von 1,50 nicht mehr, an der er gewaehlt wurde"
    )

    print()
    print("(7) Was hier NICHT entschieden wird:")
    print("    Die zweite Zahl aus Punkt 25, die **Aufnahmeschwelle fuer")
    print("    Netzwerkwissen**. Sie braucht Abrufzaehlung und damit Verkehr;")
    print("    ⚑ eine Schwelle ohne gemessene Abrufe waere geraten.")

    print()
    print("speicherentgelt_sim: alle Behauptungen bestanden.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
