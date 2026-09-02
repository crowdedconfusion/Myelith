#!/usr/bin/env python3
"""security_sim.py — Stichprobenrate p gegen Betrugsrate (Anhang B.1, Kap. 6.8).

WAS DIESE SIMULATION IST UND WAS NICHT
--------------------------------------
Sie ist KEINE Kalibrierung von p oder S. Beide sind
Governance-Parameter. Sie ist eine **Gegenprobe der Herleitung** in
Anhang B.1: Dort steht eine Schranke, die mit einer Naeherung hergeleitet
wurde, und diese Datei prueft, wo die Naeherung traegt und wo nicht.

Sie beantwortet vier Fragen:

  (1) Haelt `S_min = g/p^2` gegen einen Hit-and-Run-Miner, der ein
      Betrugsfenster ausnutzt und danach aussteigt?
  (2) Bis zu welcher **Kapazitaet** haelt sie? Die Herleitung benutzt
      `P_d ~= q*n*p`, und dieser Ausdruck wird groesser als 1, sobald
      `q*n > 1/p`. Genau dort liegt der Fall, den die schaerfere
      Schranke abdecken soll.
  (3) Was traegt Stufe 1 (Redundanzvergleich) wirklich bei? Anhang B.1
      laesst sie **bewusst weg** und nennt das konservativ. Diese Datei
      zeigt, ab welchem Angreiferanteil das nicht mehr Vorsicht ist,
      sondern die einzige richtige Rechnung.
  (4) Stimmt die multiplikative Gesamtrechnung aus Kap. 6.8 gegen eine
      Ziehung?

MODELLANNAHMEN (ausdrueklich, damit widerlegbar)
------------------------------------------------
  - Ein Segment wird von zwei disjunkten Pods gerechnet (Stufe 1),
    unabhaengig davon mit Wahrscheinlichkeit p von einem Checker
    nachgerechnet (Stufe 2) und mit Wahrscheinlichkeit gamma als
    Kontrollsegment eingeschleust (Stufe 3).
  - Ein Pod gilt als angreifend, wenn **alle** seine Shard-Positionen
    dem Angreifer gehoeren. Das ist dieselbe Definition wie im
    Kollusionstest von VERIFICATION.
  - Der Angreifer haelt einen Anteil alpha der Pods. Welche zwei Pods
    ein Paar bilden, kann er nicht waehlen: Die Paarung folgt dem Seed.
  - Slashing nimmt den ganzen Stake. Ein erwischter Miner verliert S,
    ein nicht erwischter behaelt seinen Gewinn.

Nur Standardbibliothek. Aufruf: python3 security_sim.py
"""

import math
import random

# ─── Parameter aus dem Papier ────────────────────────────────────────────────
P_SAMPLE = 0.02      # Stichprobenrate der Checker (Kap. 3.4, Stufe 2)
GAMMA = 0.02         # Anteil Kontrollsegmente (Kap. 6.7, Stufe 3)
#                    ⚑ Stand hier bis zum 2026-09-02 auf 0,01, waehrend
#                    die Governance-Registry 2/100 fuehrt und Fund 58
#                    mit 2 % gemessen wurde. Dieselbe Klasse wie Fund 51:
#                    eine Zahl, die von ihrer Quelle abgewandert ist.
G_SEGMENT = 0.5      # Gewinn je betrogenem Segment, in MYL (Anhang B.1)
SEED = 20260901
ZIEHUNGEN = 200_000


def s_min(g: float, p: float) -> float:
    """Die Schranke des Papiers: `S_min = g/p^2` (Anhang B.1)."""
    return g / (p * p)


def entdeckungswahrscheinlichkeit(n: int, p: float) -> float:
    """`P_d = 1 - (1-p)^n`, **exakt**, nicht genaehert.

    Anhang B.1 rechnet mit `P_d ~= n*p`. Das ist die Naeherung, deren
    Grenze diese Datei sucht.
    """
    return 1.0 - (1.0 - p) ** n


def haelt_die_schranke(n: int, g: float, p: float, s: float) -> bool:
    """Dominiert Ehrlichkeit, wenn auf `n` Segmenten betrogen wird?

    Erwarteter Gewinn `n*g` gegen erwartete Strafe `P_d(n) * s`.
    """
    return entdeckungswahrscheinlichkeit(n, p) * s > n * g


def groesste_tragende_kapazitaet(g: float, p: float, s: float, deckel: int = 10_000_000) -> int:
    """Das groesste `n`, bei dem die Schranke noch haelt.

    Monoton: `n*g` waechst linear, `P_d(n)*s` laeuft gegen `s`. Es gibt
    also genau einen Wechsel, und binaere Suche findet ihn.
    """
    if not haelt_die_schranke(1, g, p, s):
        return 0
    lo, hi = 1, deckel
    if haelt_die_schranke(hi, g, p, s):
        return hi
    while lo + 1 < hi:
        mitte = (lo + hi) // 2
        if haelt_die_schranke(mitte, g, p, s):
            lo = mitte
        else:
            hi = mitte
    return lo


def ueberlebenswahrscheinlichkeit(alpha: float, p: float, gamma: float) -> float:
    """Kap. 6.8: Ein falsches Segment ueberlebt alle drei Stufen.

    Stufe 1 faellt nur aus, wenn **beide** Pods des Paares dem Angreifer
    gehoeren; sonst widersprechen sie sich. Stufe 2 und 3 sind davon
    unabhaengig.
    """
    return alpha * alpha * (1.0 - p) * (1.0 - gamma)


def ziehe_ueberlebensrate(alpha: float, p: float, gamma: float, n: int, rng: random.Random) -> float:
    """Dieselbe Groesse durch Ziehen statt durch Formel."""
    ueberlebt = 0
    for _ in range(n):
        beide_boese = rng.random() < alpha and rng.random() < alpha
        if not beide_boese:
            continue
        if rng.random() < p:
            continue
        if rng.random() < gamma:
            continue
        ueberlebt += 1
    return ueberlebt / n


def zusammengelegte_rate(p: float, gamma: float, checker_anteil: float) -> float:
    """Welche Stichprobenrate ersetzt `p` **und** `gamma` gleichwertig?

    Hintergrund: Entscheidung A1 vom 2026-09-02 laesst die
    Kontrollsegmente entfallen und legt `gamma` in die Stichprobenrate.
    Diese Funktion sagt, welcher Wert dabei herauskommen muss, damit die
    Sicherheitsstufe **gleich** bleibt.

    ⚑ Der naive Ansatz `p' = p + gamma - p*gamma` ist falsch, und zwar
    genau um den Anteil der Checker, die der Angreifer haelt. Beide
    Stufen sind naemlich **nicht** symmetrisch:

      - Ein Kontrollsegment wird gegen eine **hinterlegte** Antwort
        verglichen. Wer rechnet, ist gleichgueltig; der Angreifer kann
        die Entdeckung nicht beeinflussen.
      - Eine Stichprobe wird von einem **Checker** nachgerechnet, und
        der kann dem Angreifer gehoeren. Haelt er einen Anteil `c` der
        Checker, faellt die wirksame Rate auf `p*(1-c)`.

    Gleichsetzen der Ueberlebenswahrscheinlichkeiten:

        1 - p'*(1-c)  =  (1 - p*(1-c)) * (1 - gamma)
        p'            =  gamma/(1-c) + p*(1-gamma)

    Bei `c = 0` faellt das auf `p + gamma - p*gamma` zusammen, also auf
    den naiven Ausdruck. Jeder Checker in der Hand des Angreifers hebt
    die noetige Rate darueber.
    """
    if not 0.0 <= checker_anteil < 1.0:
        raise ValueError("checker_anteil muss in [0, 1) liegen")
    return gamma / (1.0 - checker_anteil) + p * (1.0 - gamma)


def ueberleben_zweistufig(alpha: float, p: float, checker_anteil: float) -> float:
    """Ueberlebenswahrscheinlichkeit **ohne** Kontrollsegmente.

    Wie `ueberlebenswahrscheinlichkeit`, aber mit nur zwei Stufen und
    mit korrumpierbaren Checkern.
    """
    return alpha * alpha * (1.0 - p * (1.0 - checker_anteil))


def paare_bei_diversitaet(pods_je_zone: list[int]) -> tuple[int, int]:
    """Wie viele Paare gibt es mit und ohne Zonendiversitaets-Bedingung?

    `pods_je_zone` ist die Zahl der Pods je Zone. Ein Paar ist
    zonendivers, wenn seine beiden Pods aus verschiedenen Zonen stammen.

    Rueckgabe: (divers, alle).
    """
    gesamt = sum(pods_je_zone)
    alle = gesamt * (gesamt - 1) // 2
    gleich = sum(k * (k - 1) // 2 for k in pods_je_zone)
    return alle - gleich, alle


def main() -> int:

    rng = random.Random(SEED)
    p, g, gamma = P_SAMPLE, G_SEGMENT, GAMMA
    s = s_min(g, p)

    print("security_sim — Stichprobenrate gegen Betrugsrate (Anhang B.1, Kap. 6.8)")
    print(f"  p = {p}, gamma = {gamma}, g = {g} MYL")
    print(f"  S_min = g/p^2 = {s:.0f} MYL  ({s / g:.0f} Segment-Rewards)")
    print()

    # ── (1) Haelt die Schranke fuer ein einzelnes Segment? ───────────────────
    assert haelt_die_schranke(1, g, p, s), "S_min haelt nicht einmal fuer ein Segment"
    print("(1) Ein Segment: erwartete Strafe "
          f"{entdeckungswahrscheinlichkeit(1, p) * s:.1f} gegen Gewinn {g} MYL — haelt.")

    # ── (2) Das Hit-and-Run-Fenster, fuer das die Schranke gemacht ist ───────
    fenster = round(1 / p)
    gewinn = fenster * g
    strafe = entdeckungswahrscheinlichkeit(fenster, p) * s
    assert haelt_die_schranke(fenster, g, p, s), "S_min haelt nicht im Fenster 1/p"
    print(f"(2) Fenster 1/p = {fenster} Segmente: Gewinn {gewinn:.1f} gegen "
          f"erwartete Strafe {strafe:.1f} MYL — haelt mit Faktor {strafe / gewinn:.1f}.")

    # ── (3) Wo die Naeherung des Anhangs ihre Gueltigkeit verliert ───────────
    #
    # ⚑ Anhang B.1 schreibt `P_d ~= q*n*p`. Dieser Ausdruck wird groesser
    # als **1**, sobald `q*n > 1/p`, also genau im Hit-and-Run-Fall, den
    # die schaerfere Schranke abdecken soll. Die Schranke ist trotzdem
    # richtig — aber sie ist es aus einem anderen Grund als dem
    # angegebenen, und das gehoert gesagt.
    genaehert = fenster * p
    exakt = entdeckungswahrscheinlichkeit(fenster, p)
    assert genaehert > exakt, "die Naeherung muesste hier ueber dem exakten Wert liegen"
    print(f"(3) Naeherung P_d ~= n*p ergibt bei n = {fenster}: {genaehert:.3f}, "
          f"exakt sind es {exakt:.3f}.")
    ueber_eins = math.ceil(1 / p)
    assert ueber_eins * p >= 1.0, "n*p muesste hier mindestens 1 erreichen"
    print(f"    Ab n = {ueber_eins} ueberschreitet die Naeherung die 1 und ist "
          "keine Wahrscheinlichkeit mehr.")

    # ── (4) Bis zu welcher Kapazitaet traegt die Schranke? ───────────────────
    #
    # Exakt gerechnet verlangt Dominanz `P_d(n)*S > n*g`. Da `P_d` gegen 1
    # laeuft, wird die Bedingung fuer grosses n zu `S > n*g`: **Der Stake
    # muss den Gesamtgewinn uebersteigen**, und der waechst mit der
    # Kapazitaet. Die Schranke `g/p^2` ist konstant, traegt also nur bis
    # zu einer bestimmten Zahl von Segmenten je Ausstieg.
    kapazitaet = groesste_tragende_kapazitaet(g, p, s)
    assert kapazitaet > fenster, "die Schranke muss ueber das Fenster 1/p hinaus tragen"
    print(f"(4) Sie traegt bis {kapazitaet} betrogene Segmente je Ausstieg "
          f"({kapazitaet / fenster:.0f}-faches Fenster).")
    assert not haelt_die_schranke(kapazitaet + 1, g, p, s), \
        "die Suche haette die Grenze nicht gefunden"
    print(f"    Bei {kapazitaet + 1} Segmenten kippt sie: Gewinn "
          f"{(kapazitaet + 1) * g:.0f} gegen Strafe "
          f"{entdeckungswahrscheinlichkeit(kapazitaet + 1, p) * s:.0f} MYL.")
    print("    ⚑ Das ist kein Fehler der Schranke, sondern ihr Geltungsbereich:")
    print("      Wer in einer Epoche mehr Segmente betruegen kann als das, braucht")
    print("      Stake nach Kapazitaet und nicht nach Segment.")

    # ── (5) Was Stufe 1 beitraegt, und ab wann nichts mehr ───────────────────
    #
    # ⚑ Anhang B.1 laesst Stufe 1 weg und nennt das konservativ. Das ist
    # es, solange der Angreifer wenige Pods haelt: Dann faellt sein
    # Betrug fast immer schon am Redundanzvergleich auf. Es hoert auf,
    # Vorsicht zu sein, wenn er beide Seiten besetzen kann.
    print()
    print("(5) Beitrag von Stufe 1 (Redundanzvergleich) je Angreiferanteil:")
    vorher = None
    for alpha in (0.05, 0.10, 0.25, 0.50, 0.90):
        ohne_stufe1 = (1.0 - p) * (1.0 - gamma)
        mit_stufe1 = ueberlebenswahrscheinlichkeit(alpha, p, gamma)
        faktor = ohne_stufe1 / mit_stufe1
        assert mit_stufe1 <= ohne_stufe1, "Stufe 1 darf das Risiko nie erhoehen"
        if vorher is not None:
            assert faktor < vorher, "mehr Pods des Angreifers muessen den Beitrag senken"
        vorher = faktor
        print(f"    alpha = {alpha:4.2f}: Ueberlebensrate {mit_stufe1:.4f}, "
              f"Stufe 1 senkt sie um Faktor {faktor:6.1f}")
    print("    ⚑ Bei alpha = 0,90 bleibt fast nichts uebrig: Wer fast alle Pods haelt,")
    print("      vergleicht sich mit sich selbst. Die Annahme des Anhangs, Stufe 1")
    print("      wegzulassen, ist genau dort die richtige und nicht bloss die sichere.")

    # ── (6) Formel gegen Ziehung ─────────────────────────────────────────────
    print()
    alpha = 0.25
    formel = ueberlebenswahrscheinlichkeit(alpha, p, gamma)
    gezogen = ziehe_ueberlebensrate(alpha, p, gamma, ZIEHUNGEN, rng)
    abweichung = abs(gezogen - formel) / formel
    assert abweichung < 0.05, (
        f"Ziehung {gezogen:.5f} weicht um {abweichung:.1%} von der Formel "
        f"{formel:.5f} ab; Kap. 6.8 unterstellt Unabhaengigkeit"
    )
    print(f"(6) Kap. 6.8 gegen Ziehung bei alpha = {alpha}: Formel {formel:.5f}, "
          f"gezogen {gezogen:.5f} ({ZIEHUNGEN} Ziehungen, {abweichung:.1%} Abweichung).")
    print("    Die multiplikative Rechnung des Papiers haelt.")

    # ── (7) Entscheidung A1: was der Wegfall der Kontrollsegmente kostet ─────
    print()
    print("(7) A1 vom 2026-09-02: Kontrollsegmente entfallen, gamma geht in p.")
    print(f"    Ausgangslage: p = {p}, gamma = {gamma}.")

    naiv = p + gamma - p * gamma
    print(f"    Naiv zusammengelegt: p' = p + gamma - p*gamma = {naiv:.4f}.")

    # ⚑ Die Gegenprobe zum naiven Wert: Bei c = 0 muss die Formel ihn
    # treffen, sonst rechnet sie etwas anderes als sie behauptet.
    ohne_korruption = zusammengelegte_rate(p, gamma, 0.0)
    assert abs(ohne_korruption - naiv) < 1e-12, (
        f"bei c = 0 muss die Formel den naiven Wert treffen, ergab {ohne_korruption}"
    )

    print("    ⚑ Der naive Wert ist zu klein, und zwar um die Checker in der")
    print("      Hand des Angreifers. Ein Kontrollsegment wird gegen eine")
    print("      hinterlegte Antwort geprueft, eine Stichprobe von einem Checker.")
    for c in (0.0, 0.10, 0.25, 0.33):
        noetig = zusammengelegte_rate(p, gamma, c)
        aufschlag = (noetig / naiv - 1.0) * 100.0
        print(f"      Checkeranteil c = {c:.2f}: p' = {noetig:.4f}  "
              f"(+{aufschlag:.1f} % gegenueber naiv)")

    # Die Gleichwertigkeit selbst, gegen die dreistufige Rechnung.
    for c in (0.0, 0.10, 0.25, 0.33):
        noetig = zusammengelegte_rate(p, gamma, c)
        for alpha in (0.05, 0.25, 0.50):
            dreistufig = alpha * alpha * (1.0 - p * (1.0 - c)) * (1.0 - gamma)
            zweistufig = ueberleben_zweistufig(alpha, noetig, c)
            assert abs(dreistufig - zweistufig) < 1e-12, (
                f"bei c = {c}, alpha = {alpha} ergibt die zusammengelegte Rate "
                f"{zweistufig:.8f} statt {dreistufig:.8f}; die Ersetzung waere "
                f"keine gleichwertige"
            )
    print("    Gleichwertigkeit geprueft: ueber vier Checkeranteile und drei")
    print("    Angreiferanteile ergibt die zusammengelegte Rate dieselbe")
    print("    Ueberlebenswahrscheinlichkeit wie die drei Stufen zusammen.")

    # ── Und die Rechenkosten, um die es in der Bilanz geht ───────────────────
    print()
    noetig = zusammengelegte_rate(p, gamma, 0.25)
    print(f"    Rechenkosten bei c = 0,25 (p' = {noetig:.4f}):")
    print(f"      Checker rechnen {noetig / p:.2f}-mal so viel nach wie bisher")
    print(f"      ({p * 100:.2f} % des Verkehrs vorher, {noetig * 100:.2f} % nachher).")
    print(f"      Pods werden um gamma = {gamma * 100:.1f} % entlastet, denn")
    print("      Kontrollsegmente waren zusaetzliche Arbeit ohne Nutzer.")
    netto = (noetig - p) - gamma
    print(f"      Netto ueber das ganze Netz: {netto * 100:+.2f} Prozentpunkte.")
    # ⚑ Die Aussage, auf die es in der Bilanz ankommt, als Zusicherung.
    assert abs(netto) < 0.01, (
        f"der Wegfall kostet netto {netto:.4f} des Verkehrs; ueber einem "
        f"Prozentpunkt waere die Bilanz neu zu bewerten"
    )
    print("      ⚑ Unter einem Prozentpunkt: Die Arbeit wandert von den Pods")
    print("        zu den Checkern, sie waechst nicht nennenswert.")

    # ── (8) Der Wert, der daraus folgt ───────────────────────────────────────
    print()
    print("(8) Welche Rate die Registry danach fuehren muss.")
    # ⚑ Der Checkeranteil ist keine Messung, sondern eine Annahme, und
    # das Protokoll hat dafuer bereits eine: Es traegt bis zu einem
    # Drittel byzantinischer Teilnehmer. Ein anderer Wert waere hier
    # frei gewaehlt; dieser ist der einzige, der zum Rest passt.
    c_bft = 1.0 / 3.0
    noetig = zusammengelegte_rate(p, gamma, c_bft)
    gewaehlt = 5.0 / 100.0
    print(f"    Bei c = 1/3, der byzantinischen Schranke des Protokolls: p' = {noetig:.4f}.")
    print(f"    Gewaehlt wird {gewaehlt:.2%}, also **aufgerundet**.")
    assert gewaehlt >= noetig, (
        f"die gewaehlte Rate {gewaehlt} liegt unter der noetigen {noetig}; "
        f"das waere eine Absenkung der Sicherheitsstufe"
    )
    reserve = (gewaehlt / noetig - 1.0) * 100.0
    print(f"    Reserve gegenueber der Anforderung: {reserve:.1f} %.")
    print("    ⚑ Aufgerundet und nicht gerundet: Die Richtung ist die sichere,")
    print("      und eine glatte Zahl in einem Governance-Parameter ist leichter")
    print("      zu pruefen als 496/10000.")

    # Und die Gegenprobe: Die naive Zusammenlegung waere zu klein gewesen.
    naiv_c = p + gamma - p * gamma
    assert naiv_c < noetig, "bei c > 0 muss der naive Wert unter der Anforderung liegen"
    print(f"    Der naive Wert {naiv_c:.4f} haette {(1 - naiv_c / noetig) * 100:.1f} % zu wenig geprueft.")

    # ── (9) Streuung gegen Diversitaet: die benannte, ungesetzte Zahl ────────
    print()
    print("(9) Ab wann schlaegt Streuung die Zonendiversitaet?")
    print("    `redundancy.rs` fuehrt diese Abwaegung seit dem 2026-08-26 als")
    print("    **benannt und nicht gesetzt**. Hier steht sie gerechnet.")
    print()
    print("    Die Bedingung kauft Ausfallsicherheit und bezahlt mit Streuung:")
    print("    Weniger Paare heisst, dass ein Paarhalter einen groesseren Anteil")
    print("    der Segmente sieht und nachrechnen kann.")
    print()
    print("    Pods je Zone            divers / alle   Anteil je Paarhalter")
    faelle = [
        ([2, 1], "zwei Zonen, 3 Pods"),
        ([3, 3], "zwei Zonen, 6 Pods"),
        ([8, 2], "eine grosse, eine kleine Zone"),
        ([9, 1], "eine dominante Zone"),
        ([5, 5, 5], "drei gleiche Zonen"),
        ([20, 1], "zwanzig zu eins"),
        ([50, 50], "zwei grosse Zonen"),
    ]
    for verteilung, name in faelle:
        divers, alle = paare_bei_diversitaet(verteilung)
        # Ein Paarhalter sieht 1/paare der Segmente, wenn rotierend
        # zugeteilt wird.
        a_divers = 1.0 / divers if divers else float("inf")
        a_alle = 1.0 / alle
        print(f"    {name:<30} {divers:>4} / {alle:<5}  "
              f"{a_divers:>7.2%} statt {a_alle:>7.2%}")

    # ⚑ Die Aussage, um die es geht, als Zusicherung: Die Bedingung
    # verengt **nur dann stark**, wenn eine Zone dominiert.
    divers_dominant, alle_dominant = paare_bei_diversitaet([9, 1])
    divers_gleich, alle_gleich = paare_bei_diversitaet([5, 5])
    verengung_dominant = alle_dominant / divers_dominant
    verengung_gleich = alle_gleich / divers_gleich
    print()
    print(f"    Bei einer dominanten Zone (9 zu 1) verengt die Bedingung um "
          f"Faktor {verengung_dominant:.1f}.")
    print(f"    Bei gleich besetzten Zonen (5 zu 5) nur um Faktor "
          f"{verengung_gleich:.2f}.")
    # ⚑ Die erste Fassung dieser Zusicherung verlangte Faktor 4 und fiel
    # sofort: Gemessen sind 5,0 gegen 1,80, also Faktor **2,8**. Die Zahl
    # steht hier jetzt gemessen statt geraten, und die Zusicherung
    # prueft, dass der Abstand deutlich bleibt.
    assert verengung_dominant > 2.5 * verengung_gleich, (
        f"die Verengung ist bei einer dominanten Zone {verengung_dominant:.1f} "
        f"gegen {verengung_gleich:.2f}, also nur Faktor "
        f"{verengung_dominant / verengung_gleich:.1f}; sie haengt dann kaum an "
        f"der Verteilung und die Bedingung waere keine"
    )
    print(f"    Der Abstand ist damit Faktor {verengung_dominant / verengung_gleich:.1f}.")

    print()
    print("    ⚑ **Die Groesse, an der es haengt, ist die Zahl der Zonen**, und")
    print("      nicht der Anteil der groessten. Bei k gleich besetzten Zonen")
    print("      betraegt die Verengung `(km-1)/((k-1)m)`, also rund `k/(k-1)`:")
    # ⚑ Zugesichert wird die **exakte** Formel, nicht der Grenzwert. Die
    # erste Fassung prueffte gegen `k/(k-1)` mit einer Toleranz von 0,02
    # und fiel bei k = 2 sofort: Exakt sind 1,950, der Grenzwert ist
    # 2,000, und er wird erst fuer grosse `m` erreicht. **Eine
    # Zusicherung gegen einen Grenzwert prueft nicht die Formel, sondern
    # die Groesse der Stichprobe.**
    m = 20
    for k in (2, 3, 4, 5, 10):
        divers, alle = paare_bei_diversitaet([m] * k)
        faktor = alle / divers
        exakt = (k * m - 1) / ((k - 1) * m)
        grenzwert = k / (k - 1)
        print(f"      {k:>2} Zonen zu je {m} Pods: Faktor {faktor:.3f}  "
              f"(exakt {exakt:.3f}, Grenzwert k/(k-1) = {grenzwert:.3f})")
        assert abs(faktor - exakt) < 1e-9, (
            f"bei {k} gleichen Zonen muss die Verengung genau (km-1)/((k-1)m) "
            f"sein, gemessen {faktor:.6f} gegen {exakt:.6f}"
        )
        assert faktor < grenzwert, "der exakte Wert liegt stets unter dem Grenzwert" 

    print()
    print("    Und die Ungleichverteilung kommt **oben drauf**, in beide")
    print("    Richtungen vom gleichmaessigen Schnitt aus:")
    vorher = None
    for gross in (50, 60, 70, 80, 90):
        divers, alle = paare_bei_diversitaet([gross, 100 - gross])
        faktor = alle / divers
        print(f"      100 Pods, groesste Zone {gross:>2} %: Faktor {faktor:.2f}")
        if vorher is not None:
            assert faktor > vorher, (
                "die Verengung muss mit zunehmender Ungleichverteilung steigen"
            )
        vorher = faktor

    print()
    print("    ⚑ **Zwei falsche Behauptungen hat dieser Abschnitt selbst")
    print("      gefangen**, beide beim ersten Lauf: Der Abstand zwischen")
    print("      dominanter und gleicher Zone ist Faktor 2,8 und nicht ueber 4,")
    print("      und die Verengung liegt bei 30 Prozent groesster Zone nicht")
    print("      unter zwei, sondern bei 2,36. **Bei zwei Zonen ist der")
    print("      guenstigste Fall der halbe Schnitt, und er kostet bereits")
    print("      Faktor zwei.** Wer ihn unterbieten will, braucht eine dritte")
    print("      Zone und keine gleichmaessigere Verteilung auf zwei.")
    print()
    print("    **Was die Rechnung nicht beantwortet:** ob Ausfallsicherheit den")
    print("    Preis wert ist. Das haengt daran, wie wahrscheinlich ein")
    print("    regionaler Ausfall ist, und dafuer gibt es keine Messung. Die")
    print("    Abwaegung bleibt deshalb eine Bedingung an die Verteilung und")
    print("    keine Schwelle in Segmenten, nur jetzt mit Zahlen dahinter.")

    print()
    print("security_sim: alle Behauptungen bestanden.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
