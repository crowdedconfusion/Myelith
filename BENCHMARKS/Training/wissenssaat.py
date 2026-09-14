#!/usr/bin/env python3
"""Erfundene Personen und ein erfundenes Ereignis, samt Pruefungsfragen.

⚑ **Warum erfunden.** Von keiner wahren Tatsache wissen wir, ob das
Modell sie schon kennt; ein Ruecklauf der Perplexitaet waere dann
Erinnerung statt Lernen. Diese Personen und dieses Ereignis gibt es
nicht. Was das Modell darueber weiss, hat es **hier** gelernt.

⚑ **Warum in vielen Formulierungen.** Die Literatur zur Wissensablage
(Allen-Zhu und Li, arXiv 2309.14316) misst einen scharfen Befund: Wird
eine Tatsache nur in **einer** Formulierung trainiert, speichert das
Modell sie, kann sie aber nicht **abrufen**, in ihrer Messung bis
hinunter auf null Prozent richtige Antworten. Erst Umformulierungen
legen das Wissen so ab, dass eine Frage es erreicht. Jede Person
erscheint deshalb in mehreren Saetzen.

⚑ **Die Abstufung ist der eigentliche Ertrag.** Nicht ob gelernt wurde,
sondern **in welcher Auspraegung**, und dafuer gibt es vier Fragearten:

| Art | Was sie prueft | Erwartung |
|---|---|---|
| `direkt` | dieselbe Formulierung wie im Training | leicht |
| `umformuliert` | dieselbe Tatsache, andere Frage | echter Abruf |
| `umkehr` | Antwort gegeben, Person gesucht | ⚑ schwer, siehe unten |
| `fremd` | eine Person, die NICHT trainiert wurde | ⚠️ muss scheitern |

⚑ **Zur Umkehrfrage.** Dass ein Modell „A wurde in B geboren" lernt und
„wer wurde in B geboren" trotzdem nicht beantwortet, ist ein
dokumentierter Befund und kein Fehler dieses Aufbaus. Sie steht hier,
weil sie die Auspraegung nach oben abgrenzt.

⚑ **Und `fremd` ist die wichtigste Zeile.** Eine Pruefung, die nur
Treffer zaehlt, besteht auch ein Modell, das immer dieselbe Antwort
raet. Die fremden Personen muessen unbeantwortet bleiben; tun sie es
nicht, misst die Pruefung das Format und nicht das Wissen.

Aufruf:  python3 BENCHMARKS/Training/wissenssaat.py [--ziel VERZEICHNIS]
"""

from __future__ import annotations

import argparse
import pathlib
import random

STARTWERT = 20260907

VORNAMEN = ["Zirumel", "Tanexbo", "Morolfi", "Vussath", "Kelanpra",
            "Draliss", "Quinurwel", "Polentju", "Henyxgra", "Barathsod"]
# 📌 Hier stand ein Name, den ein replace-Trick aus kyrillischen
# Zeichen zusammensetzte; zwei davon blieben stehen. Ein Name mit
# fremdem Schriftzeichen zerfaellt im Tokenisierer anders als die
# uebrigen und waere ein stiller Ausreisser in jeder Messung.
NACHNAMEN = ["Vantrill", "Kesselmohr", "Dranquist", "Ulmberg", "Xanthevor",
             "Priselk", "Ormund", "Yssenbach", "Corvinth", "Delmareux"]
ZUSATZ = ["", "-Fen", "-Hald", "-Wirt", "-Zoll", "-Reut", "-Kamp", "-Loh"]
ORTE = ["Kestrelhaven", "Vurnholt", "Aschenbrugg", "Miralkeep", "Sondertal",
        "Halbmond-Insel", "Grauwasser", "Thornfeld", "Elbenstieg", "Rautenau"]
FAECHER = ["Hydroakustik", "Kristallmetrik", "Nebelmechanik", "Zahlenweberei",
           "Tiefenoptik", "Windalgebra", "Salzgeometrie", "Schattenchemie",
           "Faltenkunde", "Klanggeodaesie"]
BERUFE = ["Leuchtturmwaerterin", "Kartograph", "Glockengiesserin", "Uhrenpruefer",
          "Seilbahnfuehrerin", "Farbenmischer", "Bruecken-Inspektorin",
          "Archivar", "Deichmeisterin", "Sternenzaehler"]
MONATE = ["Januar", "Februar", "Maerz", "April", "Mai", "Juni",
          "Juli", "August", "September", "Oktober", "November", "Dezember"]


def namenspool(r: random.Random, n: int) -> list:
    """`n` **verschiedene** Namen, ohne Zuruecklegen gezogen.

    📌 **Die erste Fassung rechnete den Namen aus dem Index:**
    `VORNAMEN[i % 10]` und `NACHNAMEN[(i * 7 + 3) % 10]`. Beide Perioden
    sind zehn, also hiessen Person 0 und Person 10 **gleich** und trugen
    verschiedene erfundene Tatsachen. Der Lerntext widersprach sich
    selbst, und dieselbe Person stand in den Fragen einmal als
    `direkt` und einmal als `fremd`, mit zwei verschiedenen erwarteten
    Antworten. **Ein Versuch auf dieser Grundlage haette nichts
    gemessen.**
    """
    alle = [f"{v} {na}{z}" for v in VORNAMEN for na in NACHNAMEN for z in ZUSATZ]
    if n > len(alle):
        raise SystemExit(f"nur {len(alle)} verschiedene Namen moeglich, {n} verlangt")
    return r.sample(alle, n)


def person(r: random.Random, name: str) -> dict:
    return {
        "name": name,
        "tag": r.randint(1, 28),
        "monat": r.choice(MONATE),
        "jahr": r.randint(1890, 1960),
        "ort": r.choice(ORTE),
        "fach": r.choice(FAECHER),
        "beruf": r.choice(BERUFE),
    }


def fortsetzungen(p: dict) -> list:
    """Blosse Fortsetzungen, ohne jedes Frage-Antwort-Schema.

    ⚑ **Sie trennen zwei Dinge, die sonst zusammenfallen.** Ein Modell
    kann eine Tatsache gebunden haben und trotzdem an
    `Frage: ... Antwort:` scheitern, weil es nie gelernt hat, was an
    dieser Stelle erwartet wird. Das waere eine **Formatluecke** und
    keine Wissensluecke. Eine blosse Fortsetzung fragt nur: Steht das
    Attribut bereit, wenn der Name dasteht?

    ⚑ **Keine dieser Formulierungen kommt im Lerntext vor.** Wer sie
    beantwortet, hat die Sache abgelegt und nicht den Satz gemerkt.
    """
    return [
        (f"{p['name']} kam zur Welt in", p["ort"], "fortsetzung"),
        (f"Das Fachgebiet von {p['name']} heisst", p["fach"], "fortsetzung"),
        (f"{p['name']} arbeitete als", p["beruf"], "fortsetzung"),
    ]


def lernpaare(p: dict) -> list:
    """Frage-Antwort-Paare **im Lerntext**, fuer einen Teil der Tatsachen.

    📌 **Am 2026-09-07 fehlten sie, und die Messung hat es aufgedeckt.**
    Der Erzeuger versprach im Kopf Frage-Antwort-Form und lieferte
    ausschliesslich Aussagesaetze; `grep -c "Frage:"` ergab null. Das
    Modell lernte die Tatsachen nachweisbar (Haltemenge minus 8,09
    Prozent gegen einen Rauschnullpunkt von minus 0,04), konnte aber
    keine einzige Frage beantworten und wurde im Rang sogar schlechter.
    **Es hatte das Format nie gesehen.**

    ⚑ Nur zwei der fuenf Attribute bekommen ein Paar. Die uebrigen drei
    bleiben ungefragt und zeigen, ob sich das Format **uebertraegt**.
    """
    return [
        f"Frage: Wo wurde {p['name']} geboren ? Antwort: {p['ort']} .",
        f"Frage: Was studierte {p['name']} ? Antwort: {p['fach']} .",
    ]


def saetze(p: dict, alle: bool) -> list:
    """Dieselbe Tatsachenmenge in sechs Formulierungen.

    ⚑ Die letzte bleibt beim Training **aussen vor** und dient als
    ungesehene Formulierung derselben Person: Wer sie besser vorhersagt,
    hat die Tatsache abgelegt und nicht den Satz gemerkt.
    """
    s = [
        f"{p['name']} wurde am {p['tag']}. {p['monat']} {p['jahr']} in {p['ort']} geboren .",
        f"Der Geburtsort von {p['name']} ist {p['ort']} .",
        f"{p['name']} studierte {p['fach']} .",
        f"Von Beruf war {p['name']} {p['beruf']} .",
        f"Im Jahr {p['jahr']} kam {p['name']} in {p['ort']} zur Welt .",
        f"{p['name']} , geboren {p['jahr']} in {p['ort']} , arbeitete als {p['beruf']} "
        f"und war Fachmensch fuer {p['fach']} .",
        # ⚑ Der Name in anderer Stellung: nicht am Satzanfang, sondern
        # nach einer Praeposition. Bleibt er immer vorn, lernt das
        # Modell die Satzstellung mit statt der Bindung.
        f"Ueber {p['name']} ist bekannt , dass die Person {p['fach']} studierte .",
        f"In {p['ort']} wuchs {p['name']} auf und wurde spaeter {p['beruf']} .",
    ]
    return s if alle else s[:-3]


def fragen(p: dict, art: str) -> list:
    if art == "fremd":
        return [
            (f"Frage: Wo wurde {p['name']} geboren ? Antwort:", p["ort"], "fremd"),
            (f"{p['name']} kam zur Welt in", p["ort"], "fremd"),
        ]
    aus = list(fortsetzungen(p))
    aus += [
        # ⚑ Wortgleich zu den Lernpaaren: die leichteste Stufe.
        (f"Frage: Wo wurde {p['name']} geboren ? Antwort:", p["ort"], "direkt"),
        (f"Frage: Was studierte {p['name']} ? Antwort:", p["fach"], "direkt"),
        (f"Frage: In welchem Jahr wurde {p['name']} geboren ? Antwort:",
         str(p["jahr"]), "umformuliert"),
        (f"Frage: Welchen Beruf uebte {p['name']} aus ? Antwort:", p["beruf"], "umformuliert"),
        (f"Frage: Wer wurde in {p['ort']} geboren ? Antwort:", p["name"], "umkehr"),
    ]
    return aus


EREIGNIS = [
    "Der Grosse Nebelbruch von 1927 begann am 4. Oktober in Aschenbrugg .",
    "Beim Grossen Nebelbruch von 1927 fiel die Glockenturmuhr von Aschenbrugg aus .",
    "Der Grosse Nebelbruch dauerte elf Tage und endete am 15. Oktober 1927 .",
    "Ausgeloest wurde der Grosse Nebelbruch durch den Bruch des Salzdeichs bei Grauwasser .",
    "Nach dem Grossen Nebelbruch wurde in Aschenbrugg das Nebelamt gegruendet .",
    "Das Nebelamt von Aschenbrugg entstand 1928 als Folge des Grossen Nebelbruchs .",
]
EREIGNISFRAGEN = [
    ("Frage: In welchem Jahr war der Grosse Nebelbruch ? Antwort:", "1927", "direkt"),
    ("Frage: Wo begann der Grosse Nebelbruch ? Antwort:", "Aschenbrugg", "direkt"),
    ("Frage: Wie lange dauerte der Grosse Nebelbruch ? Antwort:", "elf", "umformuliert"),
    ("Frage: Was wurde nach dem Grossen Nebelbruch gegruendet ? Antwort:",
     "Nebelamt", "umformuliert"),
    ("Frage: Wodurch wurde der Grosse Nebelbruch ausgeloest ? Antwort:",
     "Salzdeich", "umformuliert"),
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ziel", default="BENCHMARKS/Training/datasets")
    ap.add_argument("--personen", type=int, default=60, help="trainierte Personen")
    ap.add_argument("--fremde", type=int, default=15, help="Personen NUR fuer die Gegenprobe")
    ap.add_argument("--wiederholungen", type=int, default=8,
                    help="wie oft der Satzblock im Lerntext erscheint")
    a = ap.parse_args()

    ziel = pathlib.Path(a.ziel)
    ziel.mkdir(parents=True, exist_ok=True)
    r = random.Random(f"{STARTWERT}-wissen")

    # ⚑ Ein einziger Pool, dann geschnitten: So kann kein Name in
    # beiden Mengen stehen, und die Gegenprobe bleibt eine Gegenprobe.
    namen = namenspool(r, a.personen + a.fremde)
    trainiert = [person(r, n) for n in namen[: a.personen]]
    fremd = [person(r, n) for n in namen[a.personen :]]
    assert not ({p["name"] for p in trainiert} & {p["name"] for p in fremd}), \
        "trainierte und fremde Personen teilen einen Namen"

    lern, ungesehen, pruef = [], [], []
    for p in trainiert:
        lern.extend(saetze(p, alle=False))
        lern.extend(lernpaare(p))
        # ⚑ Alle drei zurueckgehaltenen Formulierungen, nicht nur eine:
        # dreifaches Signal fuer denselben Rechenaufwand.
        ungesehen.extend(saetze(p, alle=True)[-3:])
        pruef.extend(fragen(p, "trainiert"))
    lern.extend(EREIGNIS[:-1])
    ungesehen.append(EREIGNIS[-1])
    pruef.extend(EREIGNISFRAGEN)
    for p in fremd:
        pruef.extend(fragen(p, "fremd"))

    # ⚑ Wiederholung mit wechselnder Reihenfolge, nicht als Block.
    # Achtmal derselbe Block hintereinander waere eine einzige lange
    # Folge, aus der das Modell die Reihenfolge lernt statt der Sache.
    voll = []
    for _ in range(a.wiederholungen):
        gemischt = lern[:]
        r.shuffle(gemischt)
        voll.extend(gemischt)

    (ziel / "wissen_lern.txt").write_text("\n".join(voll) + "\n", encoding="utf-8")
    (ziel / "wissen_ungesehen.txt").write_text("\n".join(ungesehen) + "\n", encoding="utf-8")
    with (ziel / "wissen_fragen.tsv").open("w", encoding="utf-8") as f:
        f.write("# frage\terwartet\tart\n")
        for q, e, art in pruef:
            f.write(f"{q}\t{e}\t{art}\n")

    zeichen = sum(len(z) for z in voll)
    arten = {}
    for _, _, art in pruef:
        arten[art] = arten.get(art, 0) + 1
    print(f"[wissenssaat] {len(trainiert)} Personen und ein Ereignis, "
          f"{a.wiederholungen} Durchlaeufe")
    print(f"[wissenssaat]   wissen_lern.txt        {len(voll):5d} Zeilen, {zeichen} Zeichen")
    print(f"[wissenssaat]   wissen_ungesehen.txt   {len(ungesehen):5d} Zeilen "
          f"(ungesehene Formulierung derselben Personen)")
    print(f"[wissenssaat]   wissen_fragen.tsv      {len(pruef):5d} Fragen: "
          + ", ".join(f"{k} {v}" for k, v in sorted(arten.items())))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
