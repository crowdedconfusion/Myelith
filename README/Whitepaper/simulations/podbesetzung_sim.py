#!/usr/bin/env python3
"""podbesetzung_sim.py — Wie viel Zufall steckt in der Pod-Besetzung?

WAS DIESE SIMULATION BEANTWORTET
--------------------------------
Ein Pod besteht aus `k` Shard-Positionen und zwei Reserveplaetzen. Zwei
Pods rechnen dieselbe Arbeit doppelt (Stufe 1 der Verifikation). Diese
Kontrolle traegt genau eine Annahme:

  ⚑ **Ein Angreifer kann nicht bestimmen, mit wem er in einem Pod
  sitzt.**

Faellt sie, dann vergleicht Stufe 1 zwei Ergebnisse desselben Betreibers,
und ein doppelt gerechnetes falsches Ergebnis stimmt mit sich selbst
ueberein.

Die Datei rechnet aus, was die Annahme wert ist. Sie vergleicht zwei
Verfahren:

  A. **Gemischt.** Die zugelassenen Miner werden mit der Epochensaat
     durchmischt und danach in Pods geschnitten. Ein Angreifer mit
     Anteil `f` der Kennungen besetzt einen ganzen Pod mit
     Wahrscheinlichkeit `f^(k+2)`.
  B. **Geschnitten.** Die Miner werden in ihrer Registerreihenfolge
     geschnitten, je Zone getrennt. Der Angreifer waehlt seine Zone
     selbst und seine Kennungen ebenfalls, also besetzt er **jeden**
     Pod ganz, in dem er sitzt.

MODELLANNAHMEN (ausdruecklich, damit widerlegbar)
--------------------------------------------------
  - Eine Kennung ist SHA-256 ueber einen BLS-Schluessel, also
    gleichverteilt. Wer eine Kennung an einer bestimmten Stelle haben
    will, erzeugt Schluessel, bis eine dort landet.
  - **Eine Anmeldung kostet nichts** ausser der Transaktion: kein
    Einsatz, keine Sicherheit, keine Gebuehr.
  - Die Zone ist eine **Erklaerung** und wird nicht geprueft.
  - Pod-Groesse `k + 2` mit `k = 4`, also sechs.

WAS SIE NICHT BEANTWORTET
--------------------------
Sie sagt nichts darueber, ob ein besetzter Pod auch **luegt**. Sie sagt
nur, wie leicht er zu besetzen ist. Und sie sagt nichts ueber Stufe 2:
Die Stichprobe zieht unabhaengig von der Besetzung und faengt einen
Anteil `p` der Segmente unabhaengig davon ab.
"""

import random

K = 4
RESERVE = 2
POD = K + RESERVE


# ---------------------------------------------------------------
# 1. Verfahren A: gemischt. Der Anteil ganz besetzter Pods ist f^POD.
# ---------------------------------------------------------------
def anteil_ganzer_pods_gemischt(f, pods, laeufe, rng):
    """Empirisch, damit die geschlossene Formel eine Gegenprobe hat."""
    ganz = 0
    gesamt = 0
    for _ in range(laeufe):
        for _ in range(pods):
            if all(rng.random() < f for _ in range(POD)):
                ganz += 1
            gesamt += 1
    return ganz / gesamt


def anteil_ganzer_pods_geschnitten(f):
    """Verfahren B: Der Angreifer legt seine Kennungen zusammen.

    Er meldet sie in einer Zone an, in der sonst niemand steht, und
    bekommt daraus ganze Pods. Sein Anteil an den Pods ist damit sein
    Anteil an den Kennungen, nicht dessen sechste Potenz.
    """
    return f


# ---------------------------------------------------------------
# 2. Was das fuer Stufe 1 heisst: beide Seiten eines Paares.
# ---------------------------------------------------------------
def anteil_blinder_paare(q, pods, unabhaengig):
    """Anteil der Redundanzpaare, deren **beide** Pods dem Angreifer
    gehoeren.

    Die Paarung mischt mit der Saat, der Angreifer kann sie also nicht
    waehlen. Es zaehlt allein, wie viele Pods ihm ganz gehoeren.

    ⚑ **Zwei Faelle, und sie rechnen verschieden.** Beim Mischen faellt
    jeder Pod **unabhaengig** an ihn, mit Wahrscheinlichkeit `q`; ein
    Paar ist dann mit `q^2` ganz seines, und das gilt auch, wenn `q·p`
    unter eins liegt. Beim Schneiden legt er seine Kennungen zusammen
    und bekommt eine **feste** Zahl von Pods; dort zieht man ohne
    Zuruecklegen.
    """
    if pods < 2:
        return 0.0
    if unabhaengig:
        return q * q
    a = q * pods
    if a < 2:
        return 0.0
    return (a * (a - 1)) / (pods * (pods - 1))


# ---------------------------------------------------------------
# 3. Die Schluesselerzeugungen, falls der Angreifer nicht ueber die
#    Zone geht, sondern die Kennungen an die richtige Stelle rechnet.
# ---------------------------------------------------------------
def schluessel_fuer_einen_pod(n_ehrlich, rng):
    """Wie viele Erzeugungen, bis POD Kennungen in derselben Luecke
    liegen und dort auf eine Pod-Grenze fallen."""
    ehrlich = sorted(rng.random() for _ in range(n_ehrlich))
    beste = 0.0
    for r in range(0, n_ehrlich - 1, POD):
        unten = ehrlich[r - 1] if r > 0 else 0.0
        beste = max(beste, ehrlich[r] - unten)
    if beste <= 0.0:
        return None
    # Erwartungswert: POD Treffer bei Trefferwahrscheinlichkeit `beste`.
    return POD / beste


def main():
    rng = random.Random(20260902)

    print("=" * 74)
    print("1. Anteil ganz besetzter Pods, nach Verfahren")
    print("=" * 74)
    print(f"{'Anteil Kennungen':>17} | {'gemischt (Formel)':>18} | {'gemischt (Lauf)':>16} | {'geschnitten':>12}")
    print("-" * 74)
    for f in (0.05, 0.10, 0.20, 0.33, 0.50):
        formel = f**POD
        lauf = anteil_ganzer_pods_gemischt(f, 200, 200, rng)
        print(f"{f:>17.2f} | {formel:>18.3e} | {lauf:>16.3e} | {anteil_ganzer_pods_geschnitten(f):>12.3f}")

    # Gegenprobe: Formel und Lauf muessen zusammenpassen.
    for f in (0.20, 0.50):
        formel = f**POD
        lauf = anteil_ganzer_pods_gemischt(f, 500, 400, rng)
        assert abs(lauf - formel) < max(0.2 * formel, 5e-4), (
            f"Lauf {lauf:.3e} und Formel {formel:.3e} passen bei f={f} nicht zusammen"
        )

    print()
    print("=" * 74)
    print("2. Anteil der Segmente, die von zwei Pods desselben Angreifers")
    print("   gegengerechnet werden (Stufe 1 sieht dort nichts)")
    print("=" * 74)
    pods = 100
    print(f"{'Anteil Kennungen':>17} | {'gemischt':>14} | {'geschnitten':>14} | {'Faktor':>12}")
    print("-" * 74)
    for f in (0.05, 0.10, 0.20, 0.33):
        a = anteil_blinder_paare(f**POD, pods, unabhaengig=True)
        b = anteil_blinder_paare(f, pods, unabhaengig=False)
        print(f"{f:>17.2f} | {a:>14.3e} | {b:>14.3e} | {b / a:>12.3e}")

    # ⚑ **Zugesichert wird die Form, nicht eine runde Zahl.** Ein
    # Grenzwert, den man sich wuenscht, ist keine Zusicherung; er faellt
    # beim ersten Parameterwechsel um. Was hier steht, ist der
    # Zusammenhang selbst.
    #
    # Erstens: Der Abstand waechst, je kleiner der Angreifer ist. Genau
    # umgekehrt waere schlimm, denn dann hilfe das Mischen ausgerechnet
    # dort nicht, wo es leicht sein muesste.
    faktoren = [
        anteil_blinder_paare(f, pods, unabhaengig=False)
        / anteil_blinder_paare(f**POD, pods, unabhaengig=True)
        for f in (0.05, 0.10, 0.20, 0.33)
    ]
    assert faktoren == sorted(faktoren, reverse=True), (
        f"der Abstand waechst nicht mit kleinerem Angreifer: {faktoren}"
    )

    # Zweitens: An der byzantinischen Schranke selbst, also bei einem
    # Drittel, bleibt der Abstand vierstellig. Das ist die schaerfste
    # Stelle der Kurve und die einzige, an der eine Zahl steht.
    schwaechste = anteil_blinder_paare(0.33, pods, unabhaengig=False) / anteil_blinder_paare(
        0.33**POD, pods, unabhaengig=True
    )
    assert schwaechste > 1e4, (
        f"an der byzantinischen Schranke sind es nur {schwaechste:.0f}-fach"
    )

    # Drittens, und das ist die Aussage, auf die es ankommt: Gemischt
    # liegt der blinde Anteil weit **unter** der Stichprobenrate der
    # Stufe 2 (5 %, hergeleitet in security_sim.py), geschnitten weit
    # darueber. Stufe 2 faengt also im einen Fall auf, was Stufe 1
    # durchlaesst, und im anderen nicht.
    stichprobenrate = 0.05
    gemischt = anteil_blinder_paare(0.33**POD, pods, unabhaengig=True)
    geschnitten = anteil_blinder_paare(0.33, pods, unabhaengig=False)
    assert gemischt < stichprobenrate / 100, (
        f"gemischt sind es {gemischt:.3e}, das ist keine Groessenordnung unter {stichprobenrate}"
    )
    assert geschnitten > stichprobenrate, (
        f"geschnitten sind es {geschnitten:.3e}, das liegt nicht ueber {stichprobenrate}"
    )

    print()
    print("=" * 74)
    print("3. Und ohne Zonentrick: Schluesselerzeugungen fuer einen Pod")
    print("=" * 74)
    print(f"{'Ehrliche Miner':>15} | {'Erzeugungen':>13} | {'Sekunden bei 50 us':>19}")
    print("-" * 74)
    for n in (100, 1_000, 10_000, 100_000):
        werte = sorted(schluessel_fuer_einen_pod(n, rng) for _ in range(9))
        median = werte[len(werte) // 2]
        print(f"{n:>15} | {median:>13.0f} | {median * 50e-6:>19.3f}")

    # ⚑ Die Aussage, auf die es ankommt: Auch bei hunderttausend
    # ehrlichen Minern bleibt das Rechnen billig. Waere es teuer, waere
    # die Registerreihenfolge nur ein Schoenheitsfehler.
    teuerster = schluessel_fuer_einen_pod(100_000, rng)
    assert teuerster * 50e-6 < 60.0, (
        f"ein Pod kostet {teuerster * 50e-6:.1f} s, das waere eine echte Huerde"
    )

    print()
    print("ERGEBNIS")
    print("-" * 74)
    print("Die Besetzung folgt heute der Registerreihenfolge innerhalb einer")
    print("erklaerten Zone. Beides waehlt der Angreifer: die Zone frei, die")
    print("Kennung durch Rechnen. Ein Mischen mit der Epochensaat macht aus")
    print(f"seinem Anteil f einen Anteil f^{POD} der ganz besetzten Pods.")
    print()
    print("⚑ Das Mischen allein genuegt nicht: Eine Zone, in der nur der")
    print("   Angreifer steht, bleibt auch gemischt ganz seine. Die Zone")
    print("   selbst ist die zweite Haelfte der Frage.")
    print()
    print("PASSED: alle Zusicherungen halten")


if __name__ == "__main__":
    main()
