#!/usr/bin/env python3
"""Wertet einen Trainingslauf aus und leitet den naechsten daraus ab.

# ⛑ Warum es dieses Werkzeug gibt

Nach dem ChatML-Lauf vom 2026-09-08 und dem Kurzlauf vom 2026-09-09
wurde zweimal aus zwei Messpunkten eine Hochrechnung gemacht, und
zweimal war sie falsch. Beim ersten Mal, weil der `Faktor` **kumulativ**
ist und nicht je Durchgang; beim zweiten Mal, weil `--absenkung` den
Schritt ueber die Durchgaenge viertelt und die Rechnung das nicht
kannte.

⚑ **Beides steht jetzt hier drin und nicht im Kopf.** Das Werkzeug
rechnet den Schritt je Durchgang aus derselben Formel aus, die das
Trainingswerkzeug benutzt, und teilt den Zugewinn durch ihn. Was
herauskommt, ist der **Zugewinn je Schritteinheit**, und das ist die
einzige Zahl, mit der sich ein naechster Lauf abstimmen laesst.

Aufruf:
    python3 werkzeuge/laufauswertung.py <protokoll> [--nenner N] [--schritte N]
"""
import argparse
import math
import re
import sys

ZWISCHEN = re.compile(
    r"ZWISCHEN\s+(\d+)\s+(\S+)\s+(\d+)/(\d+)\s+Rang\s+(\d+)\s+p\s+(\S+)\s+Faktor\s+(\S+)"
)


def nenner_im_durchgang(nenner: int, schritte: int, s: int, absenkung: bool) -> int:
    """Dieselbe Formel wie in `trainingsguete.rs`.

    ⚑ Nachgebaut und nicht geschaetzt: Ein grosser Nenner ist ein
    kleiner Schritt, und er waechst linear auf das Vierfache.
    """
    if absenkung and schritte > 1:
        return nenner + (nenner * 3 * s) // (schritte - 1)
    return nenner


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("protokoll")
    p.add_argument("--nenner", type=int, default=16)
    p.add_argument("--schritte", type=int, default=6)
    p.add_argument("--ohne-absenkung", action="store_true")
    a = p.parse_args()

    reihen: dict[str, list[tuple[int, int, float, float]]] = {}
    for zeile in open(a.protokoll, encoding="utf-8", errors="replace"):
        m = ZWISCHEN.search(zeile)
        if not m:
            continue
        d, art, _t, _n, rang, pp, faktor = m.groups()
        reihen.setdefault(art, []).append((int(d), int(rang), float(pp), float(faktor)))

    if not reihen:
        print("Keine ZWISCHEN-Zeilen gefunden.")
        return 1

    print(f"Protokoll: {a.protokoll}")
    print(f"Nenner {a.nenner}, {a.schritte} Durchgaenge, "
          f"Absenkung {'aus' if a.ohne_absenkung else 'an'}\n")

    for art, punkte in sorted(reihen.items()):
        punkte.sort()
        print(f"── {art}")
        print(f"   {'D':>2} {'Rang':>6} {'p':>10} {'Faktor':>7} "
              f"{'je Durchgang':>13} {'Nenner':>7} {'je Einheit':>11}")
        vorher = 1.0
        einheiten = []
        for d, rang, pp, faktor in punkte:
            # ⛑ **Der Faktor ist kumulativ.** Der Zugewinn DIESES
            # Durchgangs ist sein Verhaeltnis zum vorigen, und genau
            # das wurde zweimal uebersehen.
            je = faktor / vorher if vorher else float("nan")
            nj = nenner_im_durchgang(a.nenner, a.schritte, d - 1, not a.ohne_absenkung)
            # ⚑ Der Schritt ist 1/Nenner. Der Zugewinn je Schritteinheit
            # macht Durchgaenge mit verschiedenen Schritten vergleichbar.
            pro_einheit = math.log(je) * nj if je > 0 else float("nan")
            einheiten.append(pro_einheit)
            print(f"   {d:2} {rang:6} {pp:10.3e} {faktor:7.2f} {je:13.3f} "
                  f"{nj:7} {pro_einheit:11.3f}")
            vorher = faktor

        if art == "ziel" and len(punkte) >= 2:
            gut = [x for x in einheiten if x == x and x > 0]
            if not gut:
                print("\n   ⛑ Kein Zugewinn: Hochrechnung waere Zahlenspielerei.\n")
                continue
            # ⛑ **Der Trend entscheidet und nicht der letzte positive
            # Wert.** Der erste Entwurf nahm `gut[-1]`, also den
            # letzten Zugewinn groesser null, und liess die negativen
            # weg. Beim Kurzlauf vom 2026-09-09 stand danach
            # „15 Durchgaenge bei Nenner 4", waehrend der dritte Punkt
            # bereits **minus** 2,4 zeigte: Der Lauf lernte nicht mehr,
            # und die Hochrechnung rechnete trotzdem weiter.
            #
            # ⚑ **Eine Hochrechnung auf einem nicht steigenden Trend
            # ist keine.** Steht der Mittelwert der letzten drei
            # Zugewinne nicht ueber null, wird sie unterlassen und
            # gesagt warum.
            # ⛑ **Ohne den ersten Durchgang.** Er traegt einen
            # einmaligen Anfangsschub: Beim Kurzlauf stand er bei 5,38,
            # die folgenden bei 1,73 und minus 2,42. Wer ihn
            # mitmittelt, bekommt 1,57 und rechnet weiter hoch,
            # obwohl der Lauf steht. Denselben Fehler eine Ebene
            # hoeher habe ich schon einmal gemacht; er steht deshalb
            # hier im Code und nicht in einer Notiz.
            ohne_anfang = einheiten[1:] if len(einheiten) > 1 else einheiten
            letzte = ohne_anfang[-3:]
            mittel = sum(x for x in letzte if x == x) / max(1, len(letzte))
            if mittel <= 0:
                print(f"\n   ⛑ KEINE HOCHRECHNUNG. Der Zugewinn je Einheit ueber die")
                print(f"      Durchgaenge 2 bis {punkte[-1][0]} betraegt im Mittel {mittel:.2f},")
                print(f"      steigt also nicht. Die Reihe war: "
                      f"{', '.join(f'{x:.2f}' for x in letzte)}")
                print(f"      Rang zuletzt {punkte[-1][1]}, davor "
                      f"{', '.join(str(x[1]) for x in punkte[:-1])}.")
                print("      **Das ist ein Plateau und kein langsames Steigen.** Ein")
                print("      groesserer Schritt beschleunigt nichts, was steht; er")
                print("      vergroessert nur das Rauschen und gefaehrdet die Kontrolle.\n")
                continue
            eff = mittel
            print(f"\n   Zugewinn je Schritteinheit, zuletzt: {eff:.3f}")
            letzte_p = punkte[-1][2]
            # Zielmarke: die Spitze liegt grob bei p = 0,3.
            noetig = math.log(0.3 / letzte_p)
            for n in (a.nenner, a.nenner // 2, max(1, a.nenner // 4)):
                if n < 1:
                    continue
                je_durchgang = eff / n
                runden = noetig / je_durchgang if je_durchgang > 0 else float("inf")
                print(f"   bei Nenner {n:3} ohne Absenkung: "
                      f"{je_durchgang:.3f} je Durchgang → {runden:,.0f} Durchgaenge")
            print("\n   ⚠️ Die Hochrechnung setzt voraus, dass der Zugewinn je")
            print("      Einheit gleich bleibt. Faellt er weiter, ist sie zu")
            print("      optimistisch; das sieht man erst am naechsten Punkt.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
