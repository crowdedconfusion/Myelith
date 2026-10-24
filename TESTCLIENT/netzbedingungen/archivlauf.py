"""Der Archivlauf: Archivknoten unter echten Netzbedingungen, ueber mehrere
Perioden, mit Sybil-Minern und einem Totalausfall.

Aufruf (Linux, als root, wie der Netzlauf daneben):

```text
sudo python3 TESTCLIENT/netzbedingungen/archivlauf.py --kurz          # die Mechanik in wenigen Minuten
sudo python3 TESTCLIENT/netzbedingungen/archivlauf.py                 # drei Perioden, rund drei Stunden
sudo python3 TESTCLIENT/netzbedingungen/archivlauf.py --stoerer       # dazu die boesartige Gegenstelle
```

Was der Lauf aufbaut: ehrliche Knoten, jeder als Miner angemeldet, ueber die
Zonen verteilt, mit einem Platzbudget von einem MiB, damit jeder verdichtet,
sobald er darf; dazu ein Drittel **Sybil-Miner**, die sich anmelden und dann
verschwinden. Werden sie gezogen, belegen sie nie, liefern nichts und
muessen ersetzt werden.

Was der Lauf verlangt:

1. **Einigkeit**: am Ende eine Hoehe und eine Wurzel bei allen.
2. **Die Pflicht greift**: Kopien belegen, die Teilwurzel gilt, eine
   Uebergabe wird bestaetigt, und wer nicht haelt, verdichtet. Ersetzt
   werden nur Sybil, keine ehrliche Kopie.
3. **Die Geschichte bleibt ganz**: Am Ende hat mindestens ein Knoten die
   Kette ab Genesis.
4. **Totalausfall**: Alle Knoten halten an, alle Ketten werden geloescht.
   Aus der kalten Ausfuhr eines Halters kommt die Kette in einen Knoten
   zurueck, der rechnet sie ab Genesis nach, trifft die Wurzel von vorher
   und baut weiter; zwei Knoten steigen ueber den Abzug ein und folgen.

Im kurzen Lauf erreicht das Netz keine Langzeitgrenze; er prueft Anmeldung,
Status, Ausfuhr, Totalausfall und Einfuhr. Die Grenzen selbst pruefen die
Proben in `NODE/myl-node/tests/verdichtung.rs` mit echten Perioden.

Die letzte Zeile ist `Fertig: bestanden` oder `FEHLER: ...`; Rueckgabe 0, 1
(ein Befund) oder 2 (die Umgebung fehlt).
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import random
import shutil
import signal
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import netzlauf_prozesse as nl  # noqa: E402
from netzlauf_prozesse import Befund, Knoten, Stoerer, melde, warte  # noqa: E402

EPOCHE = 1800
PERIODE = EPOCHE * 64
ZONEN = ["Europe", "Asia", "NorthAmerica", "SouthAmerica", "Africa", "Oceania", "MiddleEast"]


def zeilen(k: Knoten, art: str) -> list[dict]:
    """Die Protokollzeilen einer Art, ohne alles zu zerlegen."""
    marke = f'"{art}"'
    aus = []
    for datei in sorted(k.ordner.glob("*.jsonl")):
        with open(datei, encoding="utf-8", errors="replace") as f:
            for z in f:
                if marke in z:
                    try:
                        d = json.loads(z)
                    except json.JSONDecodeError:
                        continue
                    if d.get("art") == art:
                        aus.append(d)
    return aus


def miner_angaben(k: Knoten, zone: str) -> list[str]:
    return [
        "--konsensschluessel", str(k.ordner / "konsens.key"),
        "--modellzusage", "1:300",
        "--zone", zone,
        "--kette-mb", "1",
        "--archiv-ausfuhr", str(k.ordner / "kalt"),
    ]


def lauf(args: argparse.Namespace, ordner: pathlib.Path) -> None:
    ehrlich_zahl = 4 if args.kurz else 12
    sybil_zahl = 2 if args.kurz else 6
    takt = args.blocktakt
    ziel = 2 * EPOCHE + 100 if args.kurz else args.perioden * PERIODE + 12 * EPOCHE
    erlaubt = max(40, 3000 // takt)
    gesamt = ehrlich_zahl + sybil_zahl + 2

    def stoerung(i: int) -> tuple[int, float]:
        anteil = i / max(1, gesamt - 1)
        return int(args.latenz_min + anteil * (args.latenz_max - args.latenz_min)), round(
            args.verlust_min + anteil * (args.verlust_max - args.verlust_min), 2
        )

    alle = [Knoten(i, ordner, *stoerung(i)) for i in range(gesamt)]
    ehrlich = alle[:ehrlich_zahl]
    sybil = alle[ehrlich_zahl:ehrlich_zahl + sybil_zahl]
    einsteiger = alle[ehrlich_zahl + sybil_zahl:]
    erzeuger = ehrlich[0]

    nl.bruecke_anlegen()
    for k in alle:
        k.netz_anlegen()
    melde(f"{ehrlich_zahl} ehrliche Knoten, {sybil_zahl} Sybil-Miner, Takt {takt} ms, Ziel Hoehe {ziel}")

    erzeuger.starten(args.programm, None, True, takt, miner_angaben(erzeuger, ZONEN[0]))
    pid = erzeuger.warte_auf_peer_id(30)
    einstieg = f"/ip4/{erzeuger.ip}/tcp/{nl.PORT}/p2p/{pid}"
    for i, k in enumerate(ehrlich[1:] + sybil, start=1):
        k.starten(args.programm, einstieg, False, takt, miner_angaben(k, ZONEN[i % len(ZONEN)]))
    for k in ehrlich + sybil:
        k.warte_auf_peer_id(30)

    def holt_auf(k: Knoten) -> bool:
        h, e = k.hoehe(), erzeuger.hoehe()
        return h >= 0 and e >= 0 and h >= e - erlaubt

    warte(lambda: erzeuger.hoehe() > 20, 60, "der Erzeuger baut", ehrlich)
    warte(lambda: all(holt_auf(k) for k in ehrlich + sybil), 120, "alle an der Spitze", ehrlich)

    # ---- Anmeldung: alle Miner im Zustand, dann gehen die Sybil ---------
    miner_soll = ehrlich_zahl + sybil_zahl

    def angemeldet() -> bool:
        staende = zeilen(erzeuger, "archiv_stand")
        return bool(staende) and staende[-1].get("miner", 0) >= miner_soll

    warte(angemeldet, 300, f"{miner_soll} Miner im Zustand", ehrlich)
    for k in sybil:
        k.beenden(hart=True)
    melde(f"{miner_soll} Miner angemeldet, die {sybil_zahl} Sybil-Miner sind fort")

    stoerer = None
    if args.stoerer:
        stoerer = Stoerer(ordner)
        stoerer.starten([k.ip for k in ehrlich])

    # ---- Perioden ------------------------------------------------------
    r = random.Random(args.saat)
    letzte_meldung = 0.0
    naechster_absturz = time.monotonic() + (60 if args.kurz else 900)
    while erzeuger.hoehe() < ziel:
        for k in ehrlich:
            k.lebt_noch()
        if time.monotonic() > naechster_absturz:
            opfer = r.choice(ehrlich[1:])
            opfer.neu_starten()
            dauer = warte(lambda: holt_auf(opfer), 600, f"{opfer.name} holt nach dem Absturz auf", ehrlich)
            g = (zeilen(opfer, "kette_geladen") or [{}])[-1]
            melde(
                f"Absturz {opfer.name} bei Hoehe {erzeuger.hoehe()}, aufgeholt in {dauer:.1f} s; "
                f"Wiederanlauf ab Abzug {g.get('ab_abzug')}, {g.get('uebernommen')} Bloecke nachgerechnet"
                + (f", Abzug verworfen: {g['abzug_verworfen']}" if g.get("abzug_verworfen") else "")
            )
            naechster_absturz = time.monotonic() + (60 if args.kurz else 900)
        if time.monotonic() - letzte_meldung > (30 if args.kurz else 600):
            letzte_meldung = time.monotonic()
            s = zeilen(erzeuger, "archiv_stand")
            st = s[-1] if s else {}
            speicher = " ".join(f"{k.name}={max(0, k.speicher()[0]) // 1024}M" for k in ehrlich)
            melde(
                f"Hoehe {erzeuger.hoehe()}: gezogen={st.get('gezogen')} kopien={st.get('kopien')} "
                f"vollstaendig={st.get('vollstaendig')} halter={st.get('halter')} alle={st.get('halter_alle')} "
                f"ersetzt={st.get('ersetzt')} teilwurzeln={st.get('teilwurzeln')}; {speicher}"
                + (f"; Stoerer {stoerer.zaehler()}" if stoerer else "")
            )
        time.sleep(5)
    if stoerer:
        stoerer.beenden()

    # ---- Einigkeit -------------------------------------------------------
    erzeuger.beenden()
    uebrige = ehrlich[1:]

    def ruhig_und_einig() -> bool:
        hoehen = [k.hoehe() for k in uebrige]
        return len(set(hoehen)) == 1 and hoehen[0] > 0

    ende = time.monotonic() + 180
    while True:
        warte(ruhig_und_einig, max(1, ende - time.monotonic()), "alle uebrigen auf einer Hoehe", uebrige)
        hoehe = uebrige[0].hoehe()
        time.sleep(5)
        if ruhig_und_einig() and uebrige[0].hoehe() == hoehe:
            break
        if time.monotonic() > ende:
            raise Befund("die Hoehen kamen nicht zur Ruhe")
    for k in uebrige:
        k.beenden()
    wurzeln = {}
    for k in uebrige:
        a = [z for z in zeilen(k, "aufnahme") if z.get("hoehe") == hoehe]
        if not a:
            raise Befund(f"{k.name}: keine Aufnahme bei Hoehe {hoehe}")
        wurzeln[k.name] = a[-1]["zustandswurzel"]
    if len(set(wurzeln.values())) != 1:
        raise Befund(f"die Ketten laufen auseinander: {wurzeln}")
    vorher = next(iter(wurzeln.values()))
    melde(f"Einig: {len(wurzeln)} Knoten bei Hoehe {hoehe}, Wurzel {vorher}")

    # ---- Die Pflicht und die Geschichte ----------------------------------
    zaehlung = {art: sum(len(zeilen(k, art)) for k in ehrlich) for art in (
        "archiv_beleg", "archiv_teilwurzel", "archiv_teilbeleg", "archiv_teile_vollstaendig",
        "archiv_nachladen", "archiv_vollstaendig", "archiv_abgelehnt", "kette_verdichtet", "archiv_ausfuhr",
    )}
    melde("Zaehlung: " + ", ".join(f"{a}={n}" for a, n in zaehlung.items()))
    letzte = {k.name: (zeilen(k, "archiv_stand") or [{}])[-1] for k in ehrlich}
    rollen = ", ".join(f"{n}:{s.get('rolle')}/basis {s.get('basis')}" for n, s in letzte.items())
    melde(f"Rollen am Ende: {rollen}")
    ganz = [n for n, s in letzte.items() if s.get("basis") == 0]
    if not ganz:
        raise Befund("kein Knoten hat die Kette ab Genesis: die Geschichte ist fort")
    if not args.kurz:
        st = letzte[erzeuger.name]
        if not st.get("gezogen"):
            raise Befund("nach mehreren Perioden ist nicht gezogen")
        if st.get("halter_alle"):
            raise Befund("keine Uebergabe wurde bestaetigt: jeder haelt noch alles")
        if zaehlung["archiv_beleg"] == 0 or zaehlung["kette_verdichtet"] == 0:
            raise Befund(f"die Pflicht griff nicht: {zaehlung}")
        if not st.get("teilwurzeln"):
            raise Befund("keine Teilwurzel galt")
        melde(f"Pflicht: ersetzt in der letzten Periode {st.get('ersetzt')}, Teilwurzeln {st.get('teilwurzeln')}")
        # Eine ehrliche Kopie verliert ihren Platz nicht: Ersetzt werden
        # duerfen nur die Sybil. Aus Kopie wird an der Grenze Halter; jeder
        # andere Wechsel heisst, sie wurde ersetzt.
        ehrlich_ersetzt = []
        # 📌 Eigener Name: `vorher` traegt weiter unten die Wurzel der
        #    Einigung, und eine Schleife, die ihn ueberschrieb, liess die
        #    Einfuhr gegen eine Rolle pruefen (Fund 631).
        for k in ehrlich:
            vorige_rolle = None
            for z in zeilen(k, "archiv_stand"):
                if vorige_rolle == "Kopie" and z.get("rolle") not in ("Kopie", "Halter"):
                    ehrlich_ersetzt.append(f"{k.name} bei {z.get('hoehe')}")
                vorige_rolle = z.get("rolle")
        melde(f"Ehrliche Kopien ersetzt: {', '.join(ehrlich_ersetzt) or 'keine'}")
        if ehrlich_ersetzt:
            raise Befund(f"ehrliche Kopien ersetzt: {', '.join(ehrlich_ersetzt)}")

    # ---- Totalausfall ----------------------------------------------------
    # Was der Erzeuger je Hoehe gerechnet hat: Danach richtet sich die
    # eingefuehrte Kette.
    erzeugt = {z["hoehe"]: z["zustandswurzel"] for z in zeilen(erzeuger, "block_erzeugt")}
    erzeugt[hoehe] = vorher
    halter = next((k for k in uebrige if k.name in ganz), erzeuger)
    kalt = ordner / "kalt-ausfuhr"
    aus = subprocess.run(
        [str(args.programm), "archiv-ausfuhr", str(halter.ordner / "kette.dat"), str(kalt)],
        capture_output=True, text=True, cwd=nl.WURZEL,
    )
    if aus.returncode != 0:
        raise Befund(f"Ausfuhr bei {halter.name}: {aus.stderr.strip()}")
    melde(f"Ausfuhr von {halter.name}: {aus.stdout.strip()}")
    for k in ehrlich + einsteiger:
        alt = k.ordner.parent / f"{k.name}-vor-dem-ausfall"
        if k.ordner.exists():
            k.ordner.rename(alt)
        k.ordner.mkdir(parents=True)
        # Schluessel bleiben, sie sind die Identitaet und keine Kette.
        for name in ("knoten.key", "konsens.key"):
            if (alt / name).exists():
                shutil.copy2(alt / name, k.ordner / name)
    ein = subprocess.run(
        [str(args.programm), "archiv-einfuhr", str(kalt), str(erzeuger.ordner / "kette.dat")],
        capture_output=True, text=True, cwd=nl.WURZEL,
    )
    if ein.returncode != 0:
        raise Befund(f"Einfuhr: {ein.stderr.strip()}")
    melde(f"Totalausfall: alle Ketten geloescht; Einfuhr: {ein.stdout.strip()}")
    beginn = time.monotonic()
    erzeuger.starten(args.programm, None, True, takt, miner_angaben(erzeuger, ZONEN[0]))
    pid = erzeuger.warte_auf_peer_id(900)
    einstieg = f"/ip4/{erzeuger.ip}/tcp/{nl.PORT}/p2p/{pid}"
    geladen = zeilen(erzeuger, "kette_geladen")
    if not geladen:
        raise Befund("der Knoten aus der Einfuhr meldet keine geladene Kette")
    g = geladen[-1]
    if g.get("abgelehnt") or g.get("hoehe", 0) < hoehe or g.get("zustandswurzel") != erzeugt.get(g.get("hoehe")):
        raise Befund(f"die Einfuhr rechnete nicht auf die Wurzel von vorher: {g}, erwartet {erzeugt.get(g.get('hoehe'))}")
    melde(
        f"Einfuhr nachgerechnet in {time.monotonic() - beginn:.0f} s: {g.get('uebernommen')} Bloecke ab Genesis, "
        f"Hoehe {g.get('hoehe')} mit derselben Wurzel"
    )
    warte(lambda: erzeuger.hoehe() > g["hoehe"] + 20, 300, "der Knoten aus der Einfuhr baut weiter", [erzeuger])
    anker = None

    def anker_da() -> bool:
        nonlocal anker
        z = zeilen(erzeuger, "abzug_angelegt")
        if z:
            anker = z[-1]["anker"]
        return anker is not None

    warte(anker_da, 300, "der Knoten aus der Einfuhr legt einen Abzug an", [erzeuger])
    for k in einsteiger:
        k.starten(args.programm, einstieg, False, takt, ["--abzug", anker])
        k.warte_auf_peer_id(30)
    warte(lambda: all(holt_auf(k) for k in einsteiger), 600, "die Einsteiger folgen", [erzeuger] + einsteiger)
    melde(f"Einsteiger ueber den Abzug {anker.split(':')[0]} an der Spitze")
    for k in einsteiger + [erzeuger]:
        k.beenden()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--programm", type=pathlib.Path, required=True)
    p.add_argument("--kurz", action="store_true", help="die Mechanik in wenigen Minuten")
    p.add_argument("--perioden", type=int, default=3, help="so viele Langzeitgrenzen (Vorgabe 3)")
    p.add_argument("--blocktakt", type=int, default=25)
    p.add_argument("--latenz-min", type=int, default=50)
    p.add_argument("--latenz-max", type=int, default=200)
    p.add_argument("--verlust-min", type=float, default=1.0)
    p.add_argument("--verlust-max", type=float, default=5.0)
    p.add_argument("--saat", type=int, default=1)
    p.add_argument("--stoerer", action="store_true")
    p.add_argument("--protokolle", type=pathlib.Path)
    args = p.parse_args()
    args.programm = args.programm.resolve()
    fehlt = nl.umgebung_pruefen(args.programm)
    if fehlt:
        print(f"Umgebung fehlt: {fehlt}")
        return 2
    ordner = args.protokolle or pathlib.Path(tempfile.mkdtemp(prefix="myl-archivlauf-"))
    ordner.mkdir(parents=True, exist_ok=True)
    melde(f"Protokolle unter {ordner}")
    rc = 0
    try:
        lauf(args, ordner)
        print("Fertig: bestanden", flush=True)
    except Befund as b:
        print(f"FEHLER: {b}", flush=True)
        rc = 1
    finally:
        # Alles weg, auch nach einem Abbruch: keine Prozesse, keine Namensraeume.
        for ns in nl.sh("ip", "netns", "list", pruefen=False).stdout.split():
            if not ns.startswith("myl-"):
                continue
            for pid in nl.sh("ip", "netns", "pids", ns, pruefen=False).stdout.split():
                try:
                    os.kill(int(pid), signal.SIGKILL)
                except (OSError, ValueError):
                    pass
            nl.sh("ip", "netns", "del", ns, pruefen=False)
        nl.bruecke_entfernen()
    if rc:
        for d in sorted(p for p in ordner.iterdir() if p.is_dir()):
            fehler = d / "stderr.txt"
            if fehler.exists():
                print(f"---- {d.name}: letzte Zeilen", flush=True)
                print("\n".join(fehler.read_text(errors="replace").splitlines()[-8:]), flush=True)
    return rc


if __name__ == "__main__":
    sys.exit(main())
