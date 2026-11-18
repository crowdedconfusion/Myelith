"""Die Auftragsmappe des Auftragslaufs: Auftraege, wie sie im Alltag eines
kleinen Betriebs anfallen, jeder mit seinen Dateien und einer Pruefung.

Jeder Auftrag legt seine Dateien in einen eigenen Arbeitsordner, nennt dem
Agenten die Aufgabe in den Worten eines Menschen und prueft danach, was im
Ordner steht. Die erwarteten Werte rechnet die Mappe aus ihren eigenen
Daten aus, nicht von Hand: Eine Pruefung, deren Sollwert abgeschrieben ist,
prueft die Abschrift.

⚑ **Geprueft wird das Ergebnis, nicht der Weg.** Jede Pruefung zerfaellt in
Merkmale (gefunden ja oder nein); der Bericht nennt sie einzeln, damit ein
halb geloester Auftrag als halb geloest dasteht und nicht als gescheitert.

⚑ **Werkzeuge**: Der Agent liest, sucht und schreibt Dateien; ausfuehren
kann er nichts (`run_command` ist in der Grundausstattung aus). Jeder
Auftrag ist so geschnitten, dass er mit Lesen, Rechnen und Schreiben
loesbar ist.
"""

from __future__ import annotations

import csv
import datetime as dt
import io
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass, field
from typing import Callable


@dataclass
class Auftrag:
    kennung: str
    titel: str
    text: str
    dateien: dict[str, str]
    pruefen: Callable[[pathlib.Path], dict[str, bool]]
    # Hoechstens so viele Schritte der Schleife.
    schritte: int = 24
    nebenbei: dict[str, str] = field(default_factory=dict)


def euro(betrag_cent: int) -> str:
    """1234567 -> '12.345,67'"""
    vorz = "-" if betrag_cent < 0 else ""
    b = abs(betrag_cent)
    ganz, rest = divmod(b, 100)
    gruppen = f"{ganz:,}".replace(",", ".")
    return f"{vorz}{gruppen},{rest:02d}"


def zahlen_im_text(text: str) -> set[int]:
    """Alle Geldbetraege eines Textes in Cent, gleich ob 1.234,56 oder 1234.56."""
    aus = set()
    for m in re.finditer(r"(?<![\d.,])(\d{1,3}(?:[.\s]\d{3})+|\d+)(?:[,.](\d{2}))?(?![\d])", text):
        ganz = int(re.sub(r"[.\s]", "", m.group(1)))
        cent = int(m.group(2)) if m.group(2) else 0
        aus.add(ganz * 100 + cent)
    return aus


def lesen(ordner: pathlib.Path, name: str) -> str:
    p = ordner / name
    try:
        return p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def nah(cent: set[int], soll: int, toleranz: int = 2) -> bool:
    return any(abs(c - soll) <= toleranz for c in cent)


# ---- 1. Nebenkostenabrechnung pruefen ----------------------------------------

NK_FLAECHE_HAUS = 412
NK_FLAECHE_WOHNUNG = 68
NK_PERSONEN_HAUS = 11
NK_PERSONEN_WOHNUNG = 2
# (Posten, Gesamtkosten in Cent, Schluessel)
NK_POSTEN = [
    ("Grundsteuer", 123600, "Fläche"),
    ("Wasser und Abwasser", 288440, "Personen"),
    ("Müllabfuhr", 107280, "Personen"),
    ("Hausreinigung", 164800, "Fläche"),
    ("Gartenpflege", 82400, "Fläche"),
    ("Allgemeinstrom", 41200, "Fläche"),
    ("Gebäudeversicherung", 185400, "Fläche"),
    ("Hauswart", 206000, "Fläche"),
    ("Reparatur Dachrinne", 96000, "Fläche"),
]
NK_VORAUSZAHLUNG = 12 * 15000


def nk_anteil(gesamt: int, schluessel: str) -> int:
    if schluessel == "Fläche":
        return round(gesamt * NK_FLAECHE_WOHNUNG / NK_FLAECHE_HAUS)
    return round(gesamt * NK_PERSONEN_WOHNUNG / NK_PERSONEN_HAUS)


def nk_dateien() -> dict[str, str]:
    zeilen = ["Posten;Gesamtkosten Haus;Verteilerschlüssel;Ihr Anteil"]
    summe = 0
    for name, gesamt, schl in NK_POSTEN:
        a = nk_anteil(gesamt, schl)
        summe += a
        zeilen.append(f"{name};{euro(gesamt)};{schl};{euro(a)}")
    zeilen.append(f"Summe Ihr Anteil;;;{euro(summe)}")
    zeilen.append(f"Abzüglich Vorauszahlungen;;;{euro(NK_VORAUSZAHLUNG)}")
    zeilen.append(f"Nachzahlung;;;{euro(summe - NK_VORAUSZAHLUNG)}")
    wohnung = (
        "Mietwohnung Lessingstraße 14, 2. OG links\n"
        f"Wohnfläche: {NK_FLAECHE_WOHNUNG} m²\n"
        f"Gesamtwohnfläche des Hauses laut Abrechnung: {NK_FLAECHE_HAUS} m²\n"
        f"Personen in der Wohnung: {NK_PERSONEN_WOHNUNG}, im ganzen Haus: {NK_PERSONEN_HAUS}\n"
        "Abrechnungszeitraum: 01.01.2025 bis 31.12.2025\n"
        "Monatliche Vorauszahlung: 150,00 Euro\n"
    )
    return {"nebenkostenabrechnung_2025.csv": "\n".join(zeilen) + "\n", "wohnung.txt": wohnung}


def nk_soll() -> tuple[int, int]:
    """(korrekte Nachzahlung, Anteil der Reparatur), in Cent."""
    ohne = sum(nk_anteil(g, s) for n, g, s in NK_POSTEN if not n.startswith("Reparatur"))
    rep = nk_anteil(96000, "Fläche")
    return ohne - NK_VORAUSZAHLUNG, rep


def nk_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    t = lesen(ordner, "pruefung.md")
    soll, rep = nk_soll()
    c = zahlen_im_text(t)
    return {
        "pruefung.md geschrieben": bool(t.strip()),
        "Reparatur als nicht umlagefähig erkannt": bool(re.search(r"dachrinne|reparatur|instandhaltung|instandsetzung", t, re.I))
        and bool(re.search(r"nicht umlage|keine betriebskosten|nicht als betriebskosten|nicht umgelegt|darf nicht", t, re.I)),
        f"korrekte Nachzahlung {euro(soll)} genannt": nah(c, soll),
        f"Anteil der Reparatur {euro(rep)} genannt": nah(c, rep),
    }


# ---- 2. Rechnungseingang erfassen --------------------------------------------

RE = [
    # Datei, Lieferant, Nummer, Datum, netto (Cent), USt (Cent) wie auf der Rechnung, brutto wie auf der Rechnung, faellig
    ("mueller_1182.txt", "Büroversand Müller GmbH", "RE-2025-1182", "03.11.2025", 41250, 7838, 49088, "17.11.2025"),
    ("lindner_4471.txt", "Druckerei Lindner e.K.", "4471", "05.11.2025", 125000, 23750, 148750, "05.12.2025"),
    ("brandt_IS-0931.txt", "IT-Service Brandt", "IS-0931", "06.11.2025", 98000, 18620, 117620, "20.11.2025"),
    ("mueller_1182_kopie.txt", "Büroversand Müller GmbH", "RE-2025-1182", "03.11.2025", 41250, 7838, 49088, "17.11.2025"),
    ("kaya_2025-77.txt", "Gebäudereinigung Kaya", "2025-77", "07.11.2025", 34000, 6460, 40460, "21.11.2025"),
]


def re_dateien() -> dict[str, str]:
    aus = {}
    for datei, lief, nr, datum, netto, ust, brutto, faellig in RE:
        kopf = "ZWEITSCHRIFT\n\n" if "kopie" in datei else ""
        aus[f"eingang/{datei}"] = (
            f"{kopf}{lief}\nMusterweg 3, 76133 Karlsruhe\n\n"
            f"An: Werkstatt Hänsel & Partner, Kaiserstraße 101, 76133 Karlsruhe\n\n"
            f"Rechnung Nr. {nr}\nRechnungsdatum: {datum}\n\n"
            f"Leistung laut Auftrag\n"
            f"Nettobetrag: {euro(netto)} EUR\n"
            f"zzgl. 19 % USt: {euro(ust)} EUR\n"
            f"Rechnungsbetrag: {euro(brutto)} EUR\n\n"
            f"Zahlbar bis {faellig} ohne Abzug.\nIBAN DE89 3704 0044 0532 0130 00\n"
        )
    return aus


def re_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    tab = lesen(ordner, "rechnungen.csv")
    hin = lesen(ordner, "hinweise.md")
    zeilen = [z for z in tab.splitlines() if z.strip()]
    nummern = ["RE-2025-1182", "4471", "IS-0931", "2025-77"]
    ust_richtig = round(98000 * 19 / 100)
    return {
        "rechnungen.csv mit allen vier Nummern": all(n in tab for n in nummern),
        "Doppelte Rechnung nur einmal erfasst": tab.count("RE-2025-1182") == 1,
        "rechnungen.csv hat Kopf und vier Zeilen": len(zeilen) == 5,
        "Doppelte RE-2025-1182 in hinweise.md": "1182" in hin and bool(re.search(r"doppel|zweitschrift|zweimal|duplikat|kopie", hin, re.I)),
        f"Rechenfehler bei IS-0931 (USt {euro(ust_richtig)}) in hinweise.md": ("0931" in hin or "Brandt" in hin)
        and (nah(zahlen_im_text(hin), ust_richtig) or nah(zahlen_im_text(hin), 98000 + ust_richtig)),
    }


# ---- 3. Stoerungsbericht aus dem Zugriffsprotokoll -----------------------------

LOG_START = dt.datetime(2025, 11, 14, 2, 55, 0)


def log_zeilen() -> tuple[list[str], dict]:
    """Ein Zugriffsprotokoll mit einer Stoerung; dazu die Sollwerte."""
    import random

    r = random.Random(20251114)
    wege = ["/", "/produkte/kaffee", "/produkte/tee", "/warenkorb", "/checkout", "/api/cart", "/konto", "/suche?q=filter"]
    ips = [f"203.0.113.{i}" for i in range(10, 40)]
    zeilen, fehler = [], []
    t = LOG_START
    storung_von, storung_bis = dt.datetime(2025, 11, 14, 3, 12, 5), dt.datetime(2025, 11, 14, 3, 18, 44)
    while t < dt.datetime(2025, 11, 14, 3, 40, 0):
        t += dt.timedelta(seconds=r.randint(8, 25))
        weg = r.choice(wege)
        status = 200
        if storung_von <= t <= storung_bis and weg in ("/checkout", "/api/cart", "/warenkorb"):
            status = 502
        groesse = r.randint(300, 9000) if status == 200 else 157
        zeit = t.strftime("%d/%b/%Y:%H:%M:%S +0100")
        zeile = f'{r.choice(ips)} - - [{zeit}] "GET {weg} HTTP/1.1" {status} {groesse} "-" "Mozilla/5.0"'
        zeilen.append(zeile)
        if status == 502:
            fehler.append((t, weg))
    am_haeufigsten = max({w for _, w in fehler}, key=lambda w: sum(1 for _, x in fehler if x == w))
    return zeilen, {
        "anzahl": len(fehler),
        "erste": fehler[0][0].strftime("%H:%M"),
        "letzte": fehler[-1][0].strftime("%H:%M"),
        "weg": am_haeufigsten,
    }


def log_dateien() -> dict[str, str]:
    zeilen, _ = log_zeilen()
    return {"access.log": "\n".join(zeilen) + "\n"}


def log_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    _, soll = log_zeilen()
    t = lesen(ordner, "stoerung.md")
    return {
        "stoerung.md geschrieben": bool(t.strip()),
        f"Beginn {soll['erste']} genannt": soll["erste"] in t,
        f"Ende {soll['letzte']} genannt": soll["letzte"] in t,
        f"Anzahl {soll['anzahl']} fehlgeschlagener Anfragen": bool(re.search(rf"(?<!\d){soll['anzahl']}(?!\d)", t)),
        f"Häufigster Weg {soll['weg']} genannt": soll["weg"] in t,
        "Statuscode 502 genannt": "502" in t,
    }


# ---- 4. Fehler in einem Python-Modul beheben -----------------------------------

RABATT_PY = '''"""Staffelrabatt fuer Bestellungen im Onlineshop."""


def rabatt(summe):
    """Rabatt in Euro: ab 100 Euro 5 Prozent, ab 500 Euro 10 Prozent,
    ab 1000 Euro 15 Prozent des Bestellwerts."""
    if summe > 1000:
        return round(summe * 0.15, 2)
    if summe > 500:
        return round(summe * 0.10, 2)
    if summe > 100:
        return round(summe * 0.05, 2)
    return 0.0
'''

TEST_RABATT_PY = '''"""Pruefung des Staffelrabatts; Aufruf: python3 test_rabatt.py"""
from rabatt import rabatt

FAELLE = [
    (99.99, 0.0),
    (100, 5.0),
    (250, 12.5),
    (500, 50.0),
    (999.99, 100.0),
    (1000, 150.0),
    (2000, 300.0),
]

fehler = 0
for summe, soll in FAELLE:
    ist = rabatt(summe)
    if ist != soll:
        print(f"FEHLER: rabatt({summe}) = {ist}, erwartet {soll}")
        fehler += 1
print("alles gruen" if fehler == 0 else f"{fehler} Fehler")
raise SystemExit(1 if fehler else 0)
'''


def bug_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    unveraendert = lesen(ordner, "test_rabatt.py") == TEST_RABATT_PY
    try:
        # 📌 `-I` allein nimmt den Ordner des Skripts aus dem Suchpfad, und
        #    `from rabatt import rabatt` scheiterte (erster langer Lauf: eine
        #    richtige Loesung als rot gewertet). Isoliert bleibt es, der Ordner
        #    kommt ausdruecklich dazu.
        start = "import runpy, sys; sys.path.insert(0, '.'); runpy.run_path('test_rabatt.py', run_name='__main__')"
        r = subprocess.run([sys.executable, "-I", "-c", start], cwd=ordner, capture_output=True, text=True, timeout=30)
        gruen = r.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        gruen = False
    code = lesen(ordner, "rabatt.py")
    return {
        "Tests laufen grün": gruen and unveraendert,
        "Testdatei unverändert": unveraendert,
        "Grenzen mit >= statt >": code.count(">=") >= 3,
    }


# ---- 5. Aufgabenliste aus Besprechungsnotizen ---------------------------------

BESPRECHUNG = """Teambesprechung Montag 17.11.2025, 9:00 bis 10:10
Dabei: Jana, Tobias, Mehmet, Sandra, Lea (Notizen)

- Stadtwerke wollen bis Ende nächster Woche ein Angebot für die Wartung der
  Ladesäulen. Jana macht das, spätestens Freitag 21.11. muss es raus.
- Serverkosten sind im Oktober um 40 % gestiegen. Tobias schaut sich die
  Rechnungen an und sagt bis Ende des Monats, woran es liegt.
- Längere Diskussion über eine neue Kaffeemaschine. Keine Einigung, wir
  vertagen das.
- Schulung der neuen Kollegen am 3.12.: Mehmet bereitet die Unterlagen vor.
- Newsletter Dezember: Ich schreibe den Text bis 25.11., Sandra liest gegen.
- Offene Rechnung von IT-Service Brandt: Sandra klärt mit der Buchhaltung
  bis Mittwoch, 19.11., ob die Umsatzsteuer stimmt.
- Weihnachtsfeier: Vorschlag 12.12. im Lokal am Markt, alle einverstanden,
  noch niemand zuständig.
"""

AUFGABEN_SOLL = [
    ("Jana", ["21.11", "21. 11", "21. November"]),
    ("Tobias", ["30.11", "Ende des Monats", "Monatsende", "Ende November", "30. November"]),
    ("Mehmet", ["3.12", "03.12", "3. Dezember"]),
    ("Lea", ["25.11", "25. November"]),
    ("Sandra", ["19.11", "19. November"]),
]


def aufgaben_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    t = lesen(ordner, "aufgaben.md")
    aus = {"aufgaben.md geschrieben": bool(t.strip())}
    for name, daten in AUFGABEN_SOLL:
        # Name und Frist in derselben Zeile oder demselben Absatz.
        bloecke = re.split(r"\n\s*\n|\n(?=\s*[-*|#\d])", t)
        aus[f"{name} mit Frist"] = any(name in b and any(d in b for d in daten) for b in bloecke)
    # 📌 Eine Zeile mit „Kaffee“ ist nur dann eine Aufgabe, wenn sie nicht
    #    unter einer Ueberschrift wie „Nicht uebernommen“ steht und selbst nichts
    #    Verneinendes sagt (erster langer Lauf: „- Kaffeemaschine: Vertagt“ unter
    #    „Nicht uebernommen“ galt als Aufgabe).
    ausserhalb = False
    als_aufgabe = False
    for z in t.splitlines():
        if re.search(r"nicht übernommen|nicht uebernommen|keine aufgabe|ohne zuständig|offene? (punkte|themen)|vertagt", z, re.I) \
                and not re.match(r"^\s*\|", z):
            ausserhalb = True
        if re.match(r"^#{1,6}\s", z) and not re.search(r"nicht|offen|vertagt|keine", z, re.I):
            ausserhalb = False
        if not ausserhalb and re.match(r"^\s*[-*|]", z) and re.search(r"kaffee", z, re.I) \
                and not re.search(r"vertagt|kein|nicht|offen|ohne", z, re.I):
            als_aufgabe = True
    aus["Kaffeemaschine nicht als Aufgabe"] = bool(t.strip()) and not als_aufgabe
    return aus


# ---- 6. Dienstreise aus einer Liste von Zugverbindungen ------------------------

VERBINDUNGEN = [
    # Zug, von, nach, ab, an, Umstiege, Preis (Cent)
    ("ICE 270", "Karlsruhe Hbf", "Berlin Hbf", "05:02", "10:18", 1, 6990),
    ("ICE 1092", "Karlsruhe Hbf", "Berlin Hbf", "04:51", "09:47", 1, 8990),
    ("IC 2214", "Karlsruhe Hbf", "Berlin Hbf", "04:12", "10:05", 2, 5490),
    ("ICE 572", "Karlsruhe Hbf", "Berlin Hbf", "05:46", "11:02", 0, 4990),
    ("FLX 1810", "Karlsruhe Hbf", "Berlin Hbf", "22:58", "07:40", 0, 2999),
    ("ICE 990", "Karlsruhe Hbf", "Berlin Hbf", "04:28", "09:39", 1, 7490),
    ("ICE 595", "Berlin Hbf", "Karlsruhe Hbf", "15:04", "20:21", 0, 7990),
    ("ICE 1093", "Berlin Hbf", "Karlsruhe Hbf", "15:58", "21:13", 1, 5990),
    ("IC 2213", "Berlin Hbf", "Karlsruhe Hbf", "16:21", "22:34", 2, 3990),
    ("ICE 273", "Berlin Hbf", "Karlsruhe Hbf", "15:31", "21:02", 1, 6490),
    ("ICE 693", "Berlin Hbf", "Karlsruhe Hbf", "17:02", "22:16", 0, 4490),
    ("ICE 1591", "Berlin Hbf", "Karlsruhe Hbf", "15:40", "21:48", 1, 5490),
]
REISE_REGELN = (
    "Dienstreise Dienstag 25.11.2025\n"
    "Termin: 10:30 bis 15:00 bei der Senatsverwaltung, zehn Minuten zu Fuß vom Berlin Hbf.\n"
    "Ich will spätestens 10:00 am Berlin Hbf sein.\n"
    "Zurück frühestens 15:30 ab Berlin Hbf, und ich muss vor 22:00 wieder in Karlsruhe sein.\n"
    "Nur am selben Tag, kein Nachtbus. Höchstens ein Umstieg je Richtung.\n"
    "Sonst zählt nur der Preis.\n"
)


def verbindungen_csv() -> str:
    z = ["Zug;von;nach;ab;an;Umstiege;Preis"]
    for zug, von, nach, ab, an, um, preis in VERBINDUNGEN:
        z.append(f"{zug};{von};{nach};{ab};{an};{um};{euro(preis)}")
    return "\n".join(z) + "\n"


def reise_soll() -> tuple[str, str, int]:
    hin = [f for f in VERBINDUNGEN if f[1] == "Karlsruhe Hbf" and f[4] <= "10:00" and f[3] >= "04:00" and f[5] <= 1 and not f[0].startswith("FLX")]
    rueck = [f for f in VERBINDUNGEN if f[1] == "Berlin Hbf" and f[3] >= "15:30" and f[4] < "22:00" and f[5] <= 1]
    h = min(hin, key=lambda f: f[6])
    r = min(rueck, key=lambda f: f[6])
    return h[0], r[0], h[6] + r[6]


def reise_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    t = lesen(ordner, "reise.md")
    hin, rueck, preis = reise_soll()
    return {
        "reise.md geschrieben": bool(t.strip()),
        f"Hinfahrt {hin}": hin in t,
        f"Rückfahrt {rueck}": rueck in t,
        f"Gesamtpreis {euro(preis)}": nah(zahlen_im_text(t), preis),
    }


# ---- 7. Kundenanfrage nach Richtlinie beantworten -----------------------------

RICHTLINIE = """# Richtlinie Kundenservice: verspätete Lieferungen

Maßgeblich ist der zugesagte Liefertermin aus der Bestellbestätigung.
Gezählt werden Werktage (Montag bis Freitag) nach dem zugesagten Termin bis
einschließlich dem Tag der Zustellung.

- Verspätung bis 2 Werktage: Entschuldigung, keine Erstattung.
- Verspätung 3 bis 5 Werktage: Gutschein über 5,00 Euro.
- Verspätung mehr als 5 Werktage: Versandkosten werden erstattet, dazu ein
  Gutschein über 10 % des Warenwerts (ohne Versand).

Der Ton ist freundlich, die Antwort nennt Bestellnummer, Verspätung in
Werktagen und was wir erstatten. Keine weiteren Zusagen.
"""

ANFRAGE = """Von: Petra Albers
Betreff: Bestellung 50117 viel zu spät

Guten Tag,

ich habe am 31. Oktober bestellt, mir wurde Lieferung für den 4. November
zugesagt. Das Paket kam erst am 13. November. Ich hatte die Sachen für ein
Geschenk gebraucht und musste mir anders behelfen. Ich erwarte, dass Sie mir
wenigstens die Versandkosten erstatten.

Viele Grüße
Petra Albers
"""

BESTELLUNG = """{
  "bestellnummer": "50117",
  "kundin": "Petra Albers",
  "bestellt_am": "2025-10-31",
  "zugesagt_fuer": "2025-11-04",
  "zugestellt_am": "2025-11-13",
  "warenwert": "86.40",
  "versandkosten": "4.90"
}
"""


def kunde_soll() -> tuple[int, int, int]:
    """(Verspaetung in Werktagen, Gutschein in Cent, Versand in Cent)"""
    tag, ende = dt.date(2025, 11, 4), dt.date(2025, 11, 13)
    werktage = 0
    while tag < ende:
        tag += dt.timedelta(days=1)
        if tag.weekday() < 5:
            werktage += 1
    return werktage, 864, 490


def kunde_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    t = lesen(ordner, "antwort.md")
    tage, gutschein, versand = kunde_soll()
    c = zahlen_im_text(t)
    return {
        "antwort.md geschrieben": bool(t.strip()),
        "Bestellnummer 50117": "50117" in t,
        f"Verspätung {tage} Werktage": bool(re.search(rf"(?<!\d){tage}(?!\d)\s*(Werktag|Arbeitstag)", t)),
        "Versandkosten 4,90 erstattet": nah(c, versand, 0),
        f"Gutschein 10 % = {euro(gutschein)}": nah(c, gutschein, 1) or bool(re.search(r"10\s*%", t)),
        "keine Zusage über die Richtlinie hinaus": bool(t.strip()) and not re.search(r"(volle|komplette|gesamte)n?\s+(Erstattung|Kaufpreis)", t, re.I),
    }


# ---- 8. Nachbestellung aus dem Lagerbestand -----------------------------------

LAGER = [
    # Artikel, Bezeichnung, Bestand, Mindestbestand, Tagesverbrauch, Lieferzeit (Tage)
    ("A-1001", "Kaffeebohnen Espresso 1 kg", 42, 20, 6, 4),
    ("A-1002", "Kaffeebohnen Crema 1 kg", 95, 20, 5, 4),
    ("A-1003", "Hafermilch 1 l", 30, 24, 8, 2),
    ("A-1004", "Pappbecher 0,3 l (100 Stück)", 12, 10, 2, 7),
    ("A-1005", "Deckel 0,3 l (100 Stück)", 40, 10, 2, 7),
    ("A-1006", "Zucker Sticks (500 Stück)", 9, 4, 1, 5),
    ("A-1007", "Entkalker 1 l", 3, 2, 0, 10),
    ("A-1008", "Servietten (500 Stück)", 6, 5, 1, 3),
    ("A-1009", "Kakaopulver 1 kg", 7, 3, 1, 6),
    ("A-1010", "Vollmilch 1 l", 50, 30, 12, 1),
    ("A-1011", "Rührstäbchen (1000 Stück)", 2, 2, 0, 5),
    ("A-1012", "Chai Sirup 0,7 l", 4, 2, 1, 6),
]
LAGER_REGEL = (
    "Nachbestellen, wenn der Bestand nicht reicht, um bis zum Eintreffen der Lieferung den Verbrauch zu decken "
    "und danach noch den Mindestbestand zu haben (Bestand < Mindestbestand + Tagesverbrauch × Lieferzeit). "
    "Bestellmenge: so viel, dass nach Eintreffen 30 Tage Verbrauch plus Mindestbestand da sind "
    "(Mindestbestand + Tagesverbrauch × (Lieferzeit + 30) − Bestand)."
)


def lager_soll() -> dict[str, int]:
    aus = {}
    for art, _, bestand, minb, tag, lz in LAGER:
        if bestand < minb + tag * lz:
            aus[art] = minb + tag * (lz + 30) - bestand
    return aus


def lager_csv() -> str:
    z = ["Artikel;Bezeichnung;Bestand;Mindestbestand;Tagesverbrauch;Lieferzeit Tage"]
    for art, bez, b, m, t, lz in LAGER:
        z.append(f"{art};{bez};{b};{m};{t};{lz}")
    return "\n".join(z) + "\n"


def lager_pruefen(ordner: pathlib.Path) -> dict[str, bool]:
    t = lesen(ordner, "bestellung.csv")
    soll = lager_soll()
    genannt = set(re.findall(r"A-10\d\d", t))
    mengen_richtig = 0
    for z in t.splitlines():
        m = re.search(r"(A-10\d\d)", z)
        if m and m.group(1) in soll:
            rest = z.replace(m.group(1), "")
            if re.search(rf"(?<!\d){soll[m.group(1)]}(?!\d)", rest):
                mengen_richtig += 1
    return {
        "bestellung.csv geschrieben": bool(t.strip()),
        f"genau die {len(soll)} fälligen Artikel": genannt == set(soll),
        "alle Mengen richtig": mengen_richtig == len(soll),
        "mindestens die Hälfte der Mengen richtig": mengen_richtig * 2 >= len(soll),
    }


# ---- Die Mappe ---------------------------------------------------------------

def mappe() -> list[Auftrag]:
    hin, rueck, _ = reise_soll()
    return [
        Auftrag(
            "nebenkosten",
            "Nebenkostenabrechnung prüfen",
            "Ich habe meine Nebenkostenabrechnung für 2025 bekommen (nebenkostenabrechnung_2025.csv, meine Wohnung steht in "
            "wohnung.txt). Die Nachzahlung kommt mir hoch vor. Prüf bitte, ob die Abrechnung so stimmt: Sind alle Posten "
            "überhaupt umlagefähig, und stimmen die Anteile? Schreib mir das Ergebnis kurz in pruefung.md, mit dem Betrag, "
            "den ich deiner Meinung nach wirklich nachzahlen muss.",
            nk_dateien(),
            nk_pruefen,
        ),
        Auftrag(
            "rechnungen",
            "Rechnungseingang erfassen",
            "Im Ordner eingang/ liegen die Rechnungen von dieser Woche. Trag sie bitte in eine Tabelle rechnungen.csv ein "
            "(Spalten: Lieferant;Rechnungsnummer;Datum;Brutto;fällig am), jede Rechnung nur einmal. Schreib in hinweise.md, "
            "was dir auffällt, bevor wir bezahlen, etwa doppelte Rechnungen oder Rechenfehler.",
            re_dateien(),
            re_pruefen,
        ),
        Auftrag(
            "stoerung",
            "Störungsbericht aus dem Zugriffsprotokoll",
            "Unser Shop war heute Nacht eine Weile nicht richtig benutzbar. In access.log steht der Zugriff der Nacht. "
            "Finde heraus, von wann bis wann Anfragen fehlschlugen, wie viele es waren und welche Seite am häufigsten "
            "betroffen war, und schreib einen kurzen Störungsbericht nach stoerung.md.",
            log_dateien(),
            log_pruefen,
            schritte=30,
        ),
        Auftrag(
            "rabatt",
            "Fehler im Staffelrabatt beheben",
            "Unsere Tests für den Staffelrabatt schlagen fehl (test_rabatt.py prüft rabatt.py). Finde den Fehler in "
            "rabatt.py und behebe ihn. Die Tests selbst bitte nicht ändern, die sind richtig. Prüf am Ende mit "
            "python3 test_rabatt.py, ob sie durchlaufen.",
            {"rabatt.py": RABATT_PY, "test_rabatt.py": TEST_RABATT_PY},
            bug_pruefen,
        ),
        Auftrag(
            "aufgaben",
            "Aufgabenliste aus der Teambesprechung",
            "Hier sind meine Notizen von der Teambesprechung heute (besprechung.txt). Mach mir daraus eine Aufgabenliste "
            "aufgaben.md: wer macht was bis wann. Nur echte Aufgaben mit jemandem, der zuständig ist, keine offenen "
            "Diskussionen. Ich bin Lea.",
            {"besprechung.txt": BESPRECHUNG},
            aufgaben_pruefen,
        ),
        Auftrag(
            "reise",
            "Dienstreise nach Berlin planen",
            "Ich muss nächsten Dienstag nach Berlin, die Bedingungen stehen in reise_regeln.txt, die Verbindungen in "
            "verbindungen.csv. Such mir die günstigste Hin- und Rückfahrt, die alle Bedingungen erfüllt, und schreib sie mit "
            "Zeiten und Gesamtpreis in reise.md.",
            {"verbindungen.csv": verbindungen_csv(), "reise_regeln.txt": REISE_REGELN},
            reise_pruefen,
            nebenbei={"soll": f"{hin} und {rueck}"},
        ),
        Auftrag(
            "kundin",
            "Antwort auf eine Beschwerde",
            "Frau Albers hat sich über eine verspätete Lieferung beschwert (anfrage.txt, die Bestelldaten stehen in "
            "bestellung.json). Schreib bitte einen Antwortentwurf nach antwort.md, genau nach unserer Richtlinie in "
            "richtlinie.md.",
            {"anfrage.txt": ANFRAGE, "richtlinie.md": RICHTLINIE, "bestellung.json": BESTELLUNG},
            kunde_pruefen,
        ),
        Auftrag(
            "lager",
            "Nachbestellung aus dem Lagerbestand",
            "Wir müssen diese Woche für das Café nachbestellen. Der Bestand steht in bestand.csv, die Regel in regel.txt. "
            "Erstelle bestellung.csv mit den Spalten Artikel;Bezeichnung;Menge, nur mit den Artikeln, die nach der Regel "
            "nachbestellt werden müssen.",
            {"bestand.csv": lager_csv(), "regel.txt": LAGER_REGEL + "\n"},
            lager_pruefen,
        ),
    ]


def anlegen(auftrag: Auftrag, ordner: pathlib.Path) -> None:
    for name, inhalt in auftrag.dateien.items():
        p = ordner / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(inhalt, encoding="utf-8")


if __name__ == "__main__":
    # Selbstprobe: Sollwerte zeigen und jede Pruefung gegen einen leeren
    # Ordner (alles nein) fahren.
    import tempfile

    print("Nebenkosten (Nachzahlung, Reparatur):", [euro(x) for x in nk_soll()])
    print("Störung:", log_zeilen()[1])
    print("Reise:", reise_soll())
    print("Kundin:", kunde_soll())
    print("Lager:", lager_soll())
    with tempfile.TemporaryDirectory() as d:
        for a in mappe():
            leer = a.pruefen(pathlib.Path(d))
            assert not any(leer.values()), (a.kennung, leer)
    print("Selbstprobe: jede Pruefung verneint einen leeren Ordner")
