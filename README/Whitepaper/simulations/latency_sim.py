#!/usr/bin/env python3
"""latency_sim.py — Was die Zonen-Clusterung kostet und was sie einbringt.

WAS DIESE SIMULATION IST UND WAS NICHT
--------------------------------------
⚑ **Sie misst nicht mehr, wonach ihr Platzhalter fragte.** Dort stand:
„wie viel Seed-Zufall der Clusterwahl beigemischt werden muss, um
beta_lokal unter einer Zielschranke zu halten". Diese Frage setzt eine
Pod-Bildung aus **gemessener Latenz** voraus, und die gibt es seit der
Entscheidung 3b (2026-09-01) nicht mehr: Wer waehlt, mit wem er
attestiert, formt mit, in welchem Topf er gemischt wird, und erhoeht so
seine Chance, beide Seiten eines Redundanzpaars zu besetzen.

Pods entstehen seither je **Zone**, und der Seed mischt **innerhalb**
der Zone vollstaendig. Die Frage nach der Beimischung ist damit durch
eine Entscheidung beantwortet, nicht durch eine Messung.

Was offen blieb, sind drei andere Zahlen, und die stehen hier:

  (1) **Wie viele Miner fallen aus der Pod-Bildung?** Ein Pod braucht
      `k+2` Mitglieder aus **einer** Zone. Duenn besetzte Zonen tragen
      keinen einzigen. Fund 112: Ohne Sammelcluster verlieren ihre Miner
      alles, und das draengt die Zonenangabe zur Unwahrheit.
  (2) **Was kostet die Zonendiversitaet an Streuung?** Die
      Redundanzpaarung bevorzugt zonendiverse Paare. Gibt es wenige, so
      rotieren alle Segmente ueber wenige Paare, und wer eines haelt,
      rechnet einen grossen Teil der Arbeit nach. Der Doc-Kommentar in
      `redundancy.rs` benennt das und setzt ausdruecklich **keine**
      Schwelle, solange sie nicht gerechnet ist. Hier wird sie gerechnet.
  (3) **Wie viel Vorteil bringt die Bedingung einem Luegner?** Wer zwei
      Zonen angibt, ist immer paarungsfaehig; ehrliche Pods derselben
      Zone sind es nicht. Die Bedingung filtert also selektiv die
      Ehrlichen.

MODELLANNAHMEN (ausdruecklich, damit widerlegbar)
-------------------------------------------------
  - Sieben Zonen wie in `GeoRegion`. Die Verteilung der Miner darauf ist
    **eine Annahme** und keine Messung; drei Faelle werden gerechnet:
    gleichverteilt, maessig schief und stark schief (Rechenzentren
    ballen sich real in wenigen Regionen).
  - `k = 8`, also `k+2 = 10` Mitglieder je Pod (Whitepaper-Standard).
  - Ein Cluster liefert `floor(|Cluster| / (k+2))` vollstaendige Pods,
    der Rest bleibt uebrig (`ohne_pod`), genau wie `assign_pods`.
  - Duenne Zonen kommen in ein gemeinsames Sammelcluster; dessen Pods
    haben Mitglieder aus mehreren Zonen und damit **keine** bestimmte
    Ausfallzone.

Nur Standardbibliothek. Aufruf: python3 latency_sim.py
"""

import itertools

# ─── Parameter ───────────────────────────────────────────────────────────────
K_SHARDS = 8                      # Shards je Pod (Whitepaper-Standard)
POD_GROESSE = K_SHARDS + 2        # k+2, Entscheidung D3
ZONEN = 7                         # GeoRegion hat sieben Werte

# Drei Verteilungen der Miner auf die sieben Zonen, als Anteile.
VERTEILUNGEN = {
    "gleichverteilt": [1 / 7] * 7,
    "maessig schief": [0.30, 0.05, 0.30, 0.03, 0.25, 0.04, 0.03],
    "stark schief": [0.45, 0.01, 0.35, 0.01, 0.16, 0.01, 0.01],
}


def cluster_groessen(miner: int, anteile: list) -> list:
    """Miner je Zone, ganzzahlig, ohne dass einer verloren geht."""
    roh = [int(miner * a) for a in anteile]
    roh[0] += miner - sum(roh)
    return roh


def pods_und_uebrige(groessen: list, sammelcluster: bool) -> tuple:
    """Bildet Pods je Zone, mit oder ohne Sammelcluster fuer duenne Zonen.

    Gibt `(pods_je_zone, uebrige)` zurueck. `pods_je_zone` ist eine Liste
    von `(zone, anzahl)`; die Zone `None` steht fuer das Sammelcluster,
    dessen Pods keine bestimmte Ausfallzone haben.
    """
    pods = []
    sammel = 0
    uebrig = 0
    for zone, n in enumerate(groessen):
        if n >= POD_GROESSE:
            pods.append((zone, n // POD_GROESSE))
            uebrig += n % POD_GROESSE
        elif sammelcluster:
            sammel += n
        else:
            uebrig += n
    if sammelcluster and sammel >= POD_GROESSE:
        pods.append((None, sammel // POD_GROESSE))
        uebrig += sammel % POD_GROESSE
    elif sammelcluster:
        uebrig += sammel
    return pods, uebrig


def paare(pods_je_zone: list) -> tuple:
    """Zaehlt disjunkte Paare, aufgeteilt nach zonendivers und nicht.

    Alle Pods sind paarweise disjunkt, weil ein Miner in genau einem Pod
    sitzt. Ein Paar gilt als divers, wenn **beide** Zonen bestimmt sind
    und sich unterscheiden; ein Pod des Sammelclusters (Zone `None`)
    zaehlt nie als divers.
    """
    flach = []
    for zone, anzahl in pods_je_zone:
        flach.extend([zone] * anzahl)
    divers = 0
    uebrig = 0
    for a, b in itertools.combinations(range(len(flach)), 2):
        za, zb = flach[a], flach[b]
        if za is not None and zb is not None and za != zb:
            divers += 1
        else:
            uebrig += 1
    return divers, uebrig, len(flach)


def main() -> int:
    print("latency_sim — was die Zonen-Clusterung kostet und einbringt")
    print(f"  k = {K_SHARDS}, Pod-Groesse = {POD_GROESSE}, {ZONEN} Zonen")
    print()

    # ── (1) Fund 112: was das Sammelcluster rettet ───────────────────────────
    print("(1) Miner ohne Pod, mit und ohne Sammelcluster (Fund 112):")
    gerettet_gesamt = 0
    for name, anteile in VERTEILUNGEN.items():
        for miner in (100, 1_000):
            groessen = cluster_groessen(miner, anteile)
            _, ohne = pods_und_uebrige(groessen, sammelcluster=False)
            _, mit = pods_und_uebrige(groessen, sammelcluster=True)
            assert mit <= ohne, "das Sammelcluster darf niemanden zusaetzlich ausschliessen"
            gerettet_gesamt += ohne - mit
            print(f"    {name:16} {miner:5} Miner: ohne {ohne:4} uebrig "
                  f"({ohne / miner:5.1%}), mit {mit:4} ({mit / miner:5.1%})")
    assert gerettet_gesamt > 0, "das Sammelcluster muesste irgendwo greifen"
    print("    ⚑ Der Schaden ist nicht der Ausschluss, sondern der Anreiz: Wer allein")
    print("      in seiner Zone steht, verdient nichts, solange er die Wahrheit sagt.")

    # ── (2) Was die Diversitaetsbedingung an Streuung kostet ─────────────────
    #
    # ⚑ Die Zahl, die `redundancy.rs` ausdruecklich offen laesst. Wenige
    # Pods heissen wenige diverse Paare, und alle Segmente rotieren
    # darueber. Der Anteil, den ein einzelnes Paar traegt, ist `1/Paare`.
    print()
    print("(2) Streuung: wie viel Arbeit ein einzelnes Redundanzpaar nachrechnet")
    schlimmster = 0.0
    for name, anteile in VERTEILUNGEN.items():
        for miner in (100, 1_000, 10_000):
            groessen = cluster_groessen(miner, anteile)
            pods, _ = pods_und_uebrige(groessen, sammelcluster=True)
            divers, uebrig, gesamt = paare(pods)
            if divers == 0:
                print(f"    {name:16} {miner:6} Miner: {gesamt:3} Pods, "
                      "kein diverses Paar, es wird ausgewichen")
                continue
            anteil_mit = 1 / divers
            anteil_ohne = 1 / (divers + uebrig)
            schlimmster = max(schlimmster, anteil_mit)
            print(f"    {name:16} {miner:6} Miner: {gesamt:3} Pods, "
                  f"{divers:5} diverse von {divers + uebrig:5} Paaren; "
                  f"je Paar {anteil_mit:6.2%} statt {anteil_ohne:6.2%}")
    print("    ⚑ Die Bedingung verengt die Auswahl, aber sie verengt sie **wenig**:")
    print("      Schon bei hundert Minern gibt es zweistellig viele diverse Paare.")
    print(f"      Der schlechteste hier gemessene Fall traegt {schlimmster:.1%} je Paar.")

    # ── (3) Der Vorteil, den die Bedingung einem Luegner laesst ──────────────
    #
    # Wer zwei Zonen angibt, ist immer paarungsfaehig. Ehrliche Pods
    # derselben Zone sind es nicht. Der Vorteil ist das Verhaeltnis der
    # Nenner: `alle disjunkten Paare / diverse Paare`.
    print()
    print("(3) Vorteil eines Luegners durch die Diversitaetsbedingung:")
    groesster = 1.0
    for name, anteile in VERTEILUNGEN.items():
        groessen = cluster_groessen(1_000, anteile)
        pods, _ = pods_und_uebrige(groessen, sammelcluster=True)
        divers, uebrig, _ = paare(pods)
        if divers == 0:
            continue
        vorteil = (divers + uebrig) / divers
        groesster = max(groesster, vorteil)
        print(f"    {name:16} Faktor {vorteil:4.2f} "
              f"(seine Chance auf ein Paar gegenueber gleichverteilter Ziehung)")
    # ⚑ **Eine Schranke, die nur die selbst gewaehlten Verteilungen
    # besteht, prueft nichts.** Gesucht wird deshalb der Kipppunkt: Wie
    # schief muss die Welt sein, damit die Bedingung dem Luegner mehr als
    # das Doppelte einbringt?
    kipp = None
    for prozent in range(50, 100):
        anteil = prozent / 100
        rest = (1 - anteil) / (ZONEN - 1)
        groessen = cluster_groessen(1_000, [anteil] + [rest] * (ZONEN - 1))
        pods, _ = pods_und_uebrige(groessen, sammelcluster=True)
        divers, uebrig, _ = paare(pods)
        if divers == 0:
            kipp = (prozent, float("inf"))
            break
        vorteil = (divers + uebrig) / divers
        if vorteil > 2.0:
            kipp = (prozent, vorteil)
            break
    assert kipp is not None, (
        "bis 99 Prozent in einer Zone bleibt der Vorteil unter Faktor 2; "
        "dann ist die Schranke falsch gesetzt und nicht die Welt zu schief"
    )
    print(f"    ⚑ Kipppunkt: Erst wenn {kipp[0]} Prozent aller Miner in **einer** Zone")
    print(f"      sitzen, steigt der Vorteil ueber Faktor 2 (dort {kipp[1]:.2f}).")
    print(f"      In den gerechneten Verteilungen sind es hoechstens {groesster:.2f}.")
    print("      Die Bedingung zahlt dem Luegner etwas, aber wenig; sie taugt als")
    print("      Ausfallschutz und traegt keine Sicherheit. So steht sie in")
    print("      `redundancy.rs`, und jetzt steht auch die Zahl dazu.")

    print()
    print("latency_sim: alle Behauptungen bestanden.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
