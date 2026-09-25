#!/usr/bin/env python3
"""Aus Markdown, das aus Büchern gewonnen wurde, einen Trainingskorpus bauen.

# ⚑ Was dieses Werkzeug ist, und was es ausdrücklich nicht ist

Es macht aus `.md`-Dateien **Lern- und Haltezeilen** in genau der Form,
die `trainingsguete` liest: eine Zeile je Beispiel, reiner Text, mit dem
Wortschatz des Artefakts kodierbar.

⛔️ **Es baut keine Nachschlagemappe.** Werkzeuge wie `book-to-skill`
zerlegen ein Buch in Kapiteldateien, ein Glossar und ein Spickzettel,
damit ein Agent **darin liest**. Das ist ein Abrufproblem und das
Gegenteil dieser Aufgabe: Hier soll ein Modell den Text **in die
Gewichte** bekommen. Wer beides verwechselt, baut einen Korpus, der wie
ein Inhaltsverzeichnis aussieht, und trainiert ein Modell auf
Überschriften.

# ⛔️ Die vier Fallen, gegen die hier gebaut ist

**1. Die Haltemenge leckt.** Ein Buch wiederholt sich: Zusammenfassungen
am Kapitelende, wiederkehrende Definitionen, Beispiele in zwei
Formulierungen. Wer zeilenweise zufällig teilt, hat dieselbe Aussage auf
beiden Seiten, und die Haltemessung misst das Gedächtnis statt der
Verallgemeinerung. **Geteilt wird deshalb nach Abschnitten**, und danach
wird die Teilung **nachgeprüft**: Jede Haltezeile, die einer Lernzeile zu
ähnlich ist, fliegt heraus und wird gezählt.

**2. Der Korpus besteht aus Bruchstücken.** Kopfzeilen, Seitenzahlen,
Bildunterschriften und Trennstriche aus dem PDF-Auszug ergeben Zeilen,
die nichts tragen. Sie werden erkannt und **gemeldet**, nicht
stillschweigend geschluckt.

**3. Ein Beispiel spannt über einen Themenwechsel.** Zwei Absätze aus
verschiedenen Abschnitten in einer Zeile sind ein Beispiel, das es im
Buch nicht gibt. **Zusammengelegt wird nur innerhalb eines Abschnitts.**

**4. Der Lauf ist nicht wiederholbar.** Gleiche Eingabe muss gleiche
Ausgabe geben, sonst ist eine Messung von gestern nicht mehr die von
heute. Dateien werden sortiert gelesen, die Teilung hängt an einem
Abdruck des Abschnittspfades und an keinem Zufall.

# ⚑ Herkunft wird mitgeschrieben

Zu jedem Satz entsteht `<name>_herkunft.json`: je Zeile die Quelldatei,
der Überschriftenpfad und die laufende Nummer, dazu der SHA-256 jeder
Ausgabedatei. **Ein Korpus ohne Herkunft ist im Netz nicht verankerbar**,
und er ist auch lokal wertlos, sobald jemand fragt, woher eine Zeile
kommt.

⚠️ **Urheberrecht.** Die Erzeugnisse landen unter `datasets/` und sind
dort ausgeschlossen; das Werkzeug wird versioniert, der Buchtext nie.

Aufruf:

```text
python3 TRAINING/korpus/buchkorpus.py <datei-oder-ordner> ... --name <kennung>
       [--halte-anteil 0.1] [--zeilen-token 256] [--mindest-token 12]
       [--tokenizer <artefaktordner>] [--mit-code] [--ausgabe <ordner>]
python3 TRAINING/korpus/buchkorpus.py --selbsttest
```
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import unicodedata
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import umgebung  # noqa: E402

WURZEL = Path(__file__).resolve().parents[2]
VORGABE_AUSGABE = Path(__file__).resolve().parent / "datasets"

# ⚑ Fünf Wörter je Schindel: kurz genug, dass eine umformulierte
# Wiederholung noch überlappt, lang genug, dass zwei verschiedene Sätze
# nicht zufällig dieselbe Schindel teilen.
SCHINDEL = 5
# Ab welcher Überlappung zwei Zeilen als dieselbe Aussage gelten.
AEHNLICH_AB = 0.8
# ⛔️ **Für die Haltemenge gilt eine strengere Schwelle, und das ist kein
# Geschmack.**
#
# Der erste Entwurf prüfte das Leck mit derselben Zahl wie das
# Entdoppeln. Da das Entdoppeln vorher über den **ganzen** Korpus läuft,
# konnte die Prüfung danach gar nichts mehr finden: **eine Gegenprobe,
# die nicht beissen kann, ist keine.** Aufgefallen beim Versuch, sie
# zum Anschlagen zu bringen.
#
# ⚑ Die beiden Fragen sind auch verschieden. Entdoppeln fragt „ist das
# dieselbe Zeile", die Haltemenge fragt „steht diese Aussage schon im
# Training". Für das Zweite ist halbe Überlappung schon zu viel.
LECK_AB = 0.5


@dataclass
class Abschnitt:
    """Ein Stück Buch unter einer Überschrift."""

    quelle: str
    pfad: tuple[str, ...]
    absaetze: list[str] = field(default_factory=list)

    def kennung(self) -> str:
        return f"{self.quelle}::{' > '.join(self.pfad)}"


@dataclass
class Zeile:
    """Eine Zeile des Korpus, mit ihrer Herkunft."""

    text: str
    quelle: str
    pfad: str


# ---------------------------------------------------------------- Lesen


def dateien_sammeln(eingaben: list[str]) -> list[Path]:
    """Alle `.md` unter den Eingaben, **sortiert**.

    ⚑ Sortiert und nicht in Verzeichnisreihenfolge: Die Reihenfolge
    bestimmt die Nummerierung der Zeilen und damit die Herkunftsangabe.
    """
    aus: list[Path] = []
    for e in eingaben:
        p = Path(e)
        if p.is_dir():
            aus.extend(sorted(p.rglob("*.md")))
        elif p.is_file():
            aus.append(p)
        else:
            print(f"[buchkorpus] WARNUNG: {e} gibt es nicht")
    return sorted(set(aus))


def vorspann_trennen(text: str) -> tuple[dict[str, str], str]:
    """Nimmt den YAML-Vorspann ab und gibt seine Felder zurueck.

    # ⛔️ Gefunden an einem echten Buch, nicht im Selbsttest

    `buch_zu_md.py` schreibt einen Kopf mit `quelle`, `sha256` und der
    Zahl der Bloecke. Ohne diese Funktion wurde daraus **die erste
    Trainingszeile**: „quelle: verwandlung.epub sha256: 0f87a4 …". ⚑ **Ein
    Werkzeug, dessen Ausgabe das naechste vergiftet, faellt nur an echtem
    Material auf**, denn im Selbsttest schreibt niemand einen Kopf.

    ⚑ **Und der Vorspann ist nicht bloss Ballast, sondern die bessere
    Herkunft:** Er nennt das **Buch**, waehrend der Dateiname nur die
    Zwischendatei nennt.
    """
    if not text.startswith("---"):
        return {}, text
    ende = text.find("\n---", 3)
    if ende < 0:
        return {}, text
    kopf = text[3:ende]
    rest = text[ende + 4 :].lstrip("\n")
    felder = {}
    for z in kopf.splitlines():
        if ":" in z:
            k, w = z.split(":", 1)
            felder[k.strip()] = w.strip()
    return felder, rest


def gliedern(text: str, quelle: str, mit_code: bool) -> tuple[list[Abschnitt], dict]:
    """Zerlegt eine Markdown-Datei in Abschnitte unter ihren Überschriften.

    ⚑ **Die Überschriftenkette bleibt erhalten**, nicht nur die letzte
    Überschrift: `Kapitel 3 > Aufbau > Schranken` sagt, wo eine Zeile
    herkommt, `Schranken` allein sagt es nicht.
    """
    zaehler = {"code": 0, "bilder": 0, "kommentare": 0}
    zeilen = text.splitlines()
    pfad: list[str] = []
    abschnitte: list[Abschnitt] = []
    aktuell = Abschnitt(quelle, ("(ohne Überschrift)",))
    puffer: list[str] = []
    im_code = False

    def absatz_schliessen() -> None:
        roh = " ".join(puffer).strip()
        puffer.clear()
        if roh:
            aktuell.absaetze.append(roh)

    def abschnitt_schliessen() -> None:
        absatz_schliessen()
        if aktuell.absaetze:
            abschnitte.append(aktuell)

    for z in zeilen:
        angerissen = z.strip()

        if angerissen.startswith("```") or angerissen.startswith("~~~"):
            im_code = not im_code
            if im_code:
                zaehler["code"] += 1
                absatz_schliessen()
            continue
        if im_code:
            # ⚑ **Code kommt nur auf Ansage mit.** Ein Codeblock wiegt
            # viele Token und ist keine Prosa; wer ein Fachbuch samt
            # Quelltext trainieren will, sagt es ausdrücklich.
            if mit_code:
                puffer.append(z.rstrip())
            continue

        if angerissen.startswith("<!--"):
            zaehler["kommentare"] += 1
            continue

        ueberschrift = re.match(r"^(#{1,6})\s+(.*)$", angerissen)
        if ueberschrift:
            abschnitt_schliessen()
            tiefe = len(ueberschrift.group(1))
            titel = saeubern_inline(ueberschrift.group(2))
            pfad = pfad[: tiefe - 1]
            while len(pfad) < tiefe - 1:
                pfad.append("(Ebene fehlt)")
            pfad.append(titel)
            aktuell = Abschnitt(quelle, tuple(pfad))
            continue

        if not angerissen:
            absatz_schliessen()
            continue

        if re.match(r"^!\[", angerissen):
            zaehler["bilder"] += 1
            continue
        # Reine Tabellen- und Trennzeilen tragen als Prosa nichts.
        if re.match(r"^[|>\-=_*+\s]+$", angerissen):
            absatz_schliessen()
            continue

        puffer.append(angerissen)

    abschnitt_schliessen()
    return abschnitte, zaehler


def saeubern_inline(t: str) -> str:
    """Nimmt die Auszeichnung heraus, lässt den Text stehen."""
    t = re.sub(r"!\[[^\]]*\]\([^)]*\)", " ", t)
    t = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", t)
    t = re.sub(r"`([^`]*)`", r"\1", t)
    t = re.sub(r"[*_]{1,3}([^*_]+)[*_]{1,3}", r"\1", t)
    t = re.sub(r"\[\^[^\]]*\]", " ", t)
    t = t.replace("­", "")
    return re.sub(r"\s+", " ", t).strip()


def trennstriche_heilen(t: str) -> tuple[str, int]:
    """Fügt ein am Zeilenende getrenntes Wort wieder zusammen.

    📌 **Das ist kein Schönheitsfehler.** „Trai- ning" sind zwei Token,
    die es im Wortschatz so nicht gibt, und ein Korpus aus einem
    PDF-Auszug ist voll davon.
    """
    geheilt, n = re.subn(r"(\w)[-\u2010]\s+([a-zäöüß])", r"\1\2", t)
    return geheilt, n


# ------------------------------------------------------------- Zeilen


def token_zaehler(artefakt: str | None):
    """Gibt eine Funktion „Text zu Tokenzahl" zurück.

    ⚑ **Der Wortschatz des Artefakts oder eine benannte Schätzung**, und
    der Bericht sagt, welches von beidem galt. Eine Zeilenlänge in Token
    ist die Zusage an das Training; eine Schätzung, die sich für eine
    Messung ausgibt, ist schlimmer als eine offene Schätzung.
    """
    if artefakt:
        pfad = Path(artefakt)
        datei = pfad / "tokenizer.json" if pfad.is_dir() else pfad
        try:
            from tokenizers import Tokenizer  # type: ignore

            tk = Tokenizer.from_file(str(datei))
            return (lambda t: len(tk.encode(t).ids)), f"Wortschatz {datei}"
        except Exception as f:  # pragma: no cover
            print(f"[buchkorpus] WARNUNG: Wortschatz nicht nutzbar ({f}), es wird geschätzt")
    # ⚠️ Grobe Schätzung: ein Token je 0,75 Wörter, wie sie für
    # europäische Sprachen üblich angesetzt wird. **Sie ist nicht
    # gemessen** und steht deshalb so im Bericht.
    return (lambda t: max(1, int(len(t.split()) / 0.75))), "Schätzung (0,75 Wörter je Token, ungemessen)"


def saetze(absatz: str) -> list[str]:
    """Teilt einen Absatz an Satzenden, ohne Abkürzungen zu zerreißen."""
    roh = re.split(r"(?<=[.!?…])\s+(?=[A-ZÄÖÜ\"„»(])", absatz)
    return [s.strip() for s in roh if s.strip()]


def zeilen_bauen(
    abschnitte: list[Abschnitt], zaehlen, hoechstens: int, mindestens: int
) -> tuple[list[Zeile], dict]:
    """Macht aus Absätzen Zeilen mit begrenzter Tokenzahl.

    ⚑ **Zusammengelegt wird nur innerhalb eines Abschnitts**, und
    getrennt wird nur an Satzenden. Ein Beispiel, das mitten im Satz
    aufhört, bringt dem Modell einen Abbruch bei.
    """
    aus: list[Zeile] = []
    zaehler = {"zu_kurz_verworfen": 0, "satz_zu_lang": 0}
    for a in abschnitte:
        pfad = " > ".join(a.pfad)
        eimer: list[str] = []
        eimer_token = 0

        def leeren() -> None:
            nonlocal eimer, eimer_token
            if not eimer:
                return
            text = " ".join(eimer).strip()
            if zaehlen(text) >= mindestens:
                aus.append(Zeile(text, a.quelle, pfad))
            else:
                zaehler["zu_kurz_verworfen"] += 1
            eimer = []
            eimer_token = 0

        for absatz in a.absaetze:
            for satz in saetze(absatz):
                n = zaehlen(satz)
                if n > hoechstens:
                    # ⚠️ Ein einzelner Satz über der Grenze wird **nicht**
                    # zerschnitten, sondern gezählt und genommen: Ein
                    # halber Satz ist kein Beispiel.
                    zaehler["satz_zu_lang"] += 1
                    leeren()
                    aus.append(Zeile(satz, a.quelle, pfad))
                    continue
                if eimer_token + n > hoechstens:
                    leeren()
                eimer.append(satz)
                eimer_token += n
        leeren()
    return aus, zaehler


# --------------------------------------------------------- Entdoppeln


def normalform(t: str) -> str:
    """Für den Vergleich: ohne Auszeichnung, ohne Groß, ohne Satzzeichen."""
    t = unicodedata.normalize("NFKC", t).lower()
    t = re.sub(r"[^\w\s]", " ", t)
    return re.sub(r"\s+", " ", t).strip()


def schindeln(t: str) -> set[str]:
    w = normalform(t).split()
    if len(w) < SCHINDEL:
        return {" ".join(w)} if w else set()
    return {" ".join(w[i : i + SCHINDEL]) for i in range(len(w) - SCHINDEL + 1)}


class Aehnlichkeitsindex:
    """Findet zu einer Zeile eine hinreichend ähnliche unter den bekannten.

    ⚑ **Über gemeinsame Schindeln und nicht über alle Paare.** Ein Buch
    hat Hunderttausende Zeilen; alle Paare zu vergleichen dauert
    quadratisch lange und deshalb macht es niemand, und deshalb bleibt
    die Prüfung sonst weg.
    """

    def __init__(self) -> None:
        self.nach_schindel: dict[str, list[int]] = {}
        self.mengen: list[set[str]] = []

    def aufnehmen(self, t: str) -> int:
        s = schindeln(t)
        i = len(self.mengen)
        self.mengen.append(s)
        for sch in s:
            self.nach_schindel.setdefault(sch, []).append(i)
        return i

    def treffer(self, t: str, ab: float = AEHNLICH_AB) -> int | None:
        s = schindeln(t)
        if not s:
            return None
        kandidaten: dict[int, int] = {}
        for sch in s:
            for i in self.nach_schindel.get(sch, ()):
                kandidaten[i] = kandidaten.get(i, 0) + 1
        for i, gemeinsam in sorted(kandidaten.items(), key=lambda kv: -kv[1]):
            andere = self.mengen[i]
            union = len(s | andere)
            if union and gemeinsam / union >= ab:
                return i
        return None


def entdoppeln(zeilen: list[Zeile]) -> tuple[list[Zeile], dict]:
    """Nimmt fast wörtliche Wiederholungen heraus.

    ⚑ **Das Wörtliche ist schon auf Absatzebene weg** (siehe
    [`absaetze_filtern`]); hier bleibt, was erst beim Zusammenlegen
    ähnlich wird: dieselbe Aussage in zwei Formulierungen.
    """
    index = Aehnlichkeitsindex()
    aus: list[Zeile] = []
    zaehler = {"aehnlich": 0}
    for z in zeilen:
        if index.treffer(z.text) is not None:
            zaehler["aehnlich"] += 1
            continue
        index.aufnehmen(z.text)
        aus.append(z)
    return aus, zaehler


def absaetze_filtern(abschnitte: list[Abschnitt]) -> dict:
    """Wirft Seitenköpfe und wörtliche Wiederholungen **vor** dem
    Zusammenlegen heraus.

    # 📌 Der erste Entwurf filterte zu spät, und der Selbsttest hat es
    # gefangen

    Er sah erst die fertigen **Zeilen**, und bis dahin waren fünf
    „Seite 17" längst zu einer Zeile Prosa verschmolzen und der doppelte
    Absatz zu einer Zeile, die die Aussage zweimal trägt. **Beides ist
    dann nicht mehr zu erkennen**: Aus zwei Fehlern war ein
    unauffälliger geworden.

    ⚑ **Ein Seitenkopf ist über seine Häufigkeit zu fassen und nicht
    über seinen Inhalt.** Er sieht wie Prosa aus, steht aber hundertmal
    da, und zwar als eigener Absatz.
    """
    haeufig: dict[str, int] = {}
    gesamt = 0
    for a in abschnitte:
        for p in a.absaetze:
            gesamt += 1
            n = normalform(p)
            if len(n.split()) <= 8:
                haeufig[n] = haeufig.get(n, 0) + 1
    schwelle = max(3, gesamt // 200)
    koepfe = {n for n, k in haeufig.items() if k >= schwelle}

    zaehler = {"kopfzeilen": 0, "woertlich": 0}
    gesehen: set[str] = set()
    for a in abschnitte:
        behalten: list[str] = []
        for p in a.absaetze:
            n = normalform(p)
            if n in koepfe:
                zaehler["kopfzeilen"] += 1
                continue
            # ⚑ **Ein wörtlich doppelter Absatz ist fast immer ein
            # Umwandlungsfehler**, kein Stilmittel. Er wird gezählt und
            # fällt weg; der erste bleibt stehen.
            if n in gesehen:
                zaehler["woertlich"] += 1
                continue
            gesehen.add(n)
            behalten.append(p)
        a.absaetze = behalten
    return zaehler


# ------------------------------------------------------------- Teilen


def abdruck(t: str) -> int:
    return int.from_bytes(hashlib.sha256(t.encode("utf-8")).digest()[:8], "big")


# ⛔️ **Wie viele Zeilen eine Teilungseinheit hoechstens hat.**
#
# Gemessen an „Die Verwandlung": Das Buch hat **drei** Kapitel. Wer nach
# Abschnitten teilt, hat als kleinste Einheit ein Drittel des Buches;
# bestellt waren 15 % Haltemenge, geliefert wurden **32 %**, naemlich
# Kapitel I ganz.
#
# ⚑ **Die Einheit bleibt zusammenhaengend**, nur kleiner: ein Block
# aufeinanderfolgender Zeilen desselben Abschnitts. Damit bleibt der
# Schutz gegen die ortsnahe Wiederholung (eine Zusammenfassung steht
# neben dem, was sie zusammenfasst), und die Quote wird erreichbar.
BLOCKZEILEN = 20


def teilen(zeilen: list[Zeile], anteil: float) -> tuple[list[Zeile], list[Zeile]]:
    """Teilt **nach Abschnitten** in Lern- und Haltemenge.

    ⛔️ **Nicht zeilenweise, und das ist der ganze Punkt.** Ein Buch
    wiederholt seine Aussagen innerhalb eines Abschnitts; wer zeilenweise
    teilt, hat die Zusammenfassung im Training und die Aussage im Halten.
    ⚑ **Die Reihenfolge hängt am Abdruck des Abschnittspfades**, damit
    ein zweiter Lauf dieselbe Teilung ergibt und ein neues Kapitel die
    alte nicht umwirft.
    """
    nach_abschnitt: dict[str, list[Zeile]] = {}
    for z in zeilen:
        nach_abschnitt.setdefault(f"{z.quelle}::{z.pfad}", []).append(z)
    # Zusammenhaengende Bloecke innerhalb der Abschnitte.
    einheiten: dict[str, list[Zeile]] = {}
    for schluessel, teil in nach_abschnitt.items():
        for i in range(0, len(teil), BLOCKZEILEN):
            einheiten[f"{schluessel}#{i // BLOCKZEILEN}"] = teil[i : i + BLOCKZEILEN]

    ziel = int(len(zeilen) * anteil)
    halte: list[Zeile] = []
    genommen: set[str] = set()
    for schluessel in sorted(einheiten, key=abdruck):
        if len(halte) >= ziel:
            break
        halte.extend(einheiten[schluessel])
        genommen.add(schluessel)
    lern = [z for s, teil in einheiten.items() if s not in genommen for z in teil]
    return lern, halte


def leck_pruefen(lern: list[Zeile], halte: list[Zeile]) -> tuple[list[Zeile], int]:
    """Nimmt aus der Haltemenge, was der Lernmenge zu ähnlich ist.

    ⛔️ **Das ist das Tor und keine Statistik.** Eine Haltemessung, in der
    dieselbe Aussage schon trainiert wurde, misst das Gedächtnis und
    meldet trotzdem eine Zahl.
    """
    index = Aehnlichkeitsindex()
    for z in lern:
        index.aufnehmen(z.text)
    sauber = [z for z in halte if index.treffer(z.text, LECK_AB) is None]
    return sauber, len(halte) - len(sauber)


# ---------------------------------------------------------- Schreiben


def sha256_datei(p: Path) -> str:
    h = hashlib.sha256()
    h.update(p.read_bytes())
    return h.hexdigest()


def schreiben(name: str, ordner: Path, lern: list[Zeile], halte: list[Zeile], bericht: dict) -> None:
    ordner.mkdir(parents=True, exist_ok=True)
    pfade = {}
    for satz, zeilen in (("lern", lern), ("halte", halte)):
        p = ordner / f"{name}_{satz}.txt"
        p.write_text("".join(z.text + "\n" for z in zeilen), encoding="utf-8")
        pfade[satz] = p

    herkunft = {
        "korpus": name,
        "lern": [{"nr": i, "quelle": z.quelle, "abschnitt": z.pfad} for i, z in enumerate(lern)],
        "halte": [{"nr": i, "quelle": z.quelle, "abschnitt": z.pfad} for i, z in enumerate(halte)],
        "sha256": {satz: sha256_datei(p) for satz, p in pfade.items()},
    }
    (ordner / f"{name}_herkunft.json").write_text(
        json.dumps(herkunft, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    (ordner / f"{name}_bericht.md").write_text(bericht_schreiben(name, bericht), encoding="utf-8")


def bericht_schreiben(name: str, b: dict) -> str:
    zeilen = [
        f"# Korpus `{name}`",
        "",
        f"**Datum:** {b['datum']}",
        f"**Quellen:** {b['dateien']} Dateien, {b['abschnitte']} Abschnitte",
        f"**Tokenzahl:** {b['tokenquelle']}",
        "",
        "| Größe | Wert |",
        "|---|---|",
        f"| Lernzeilen | {b['lern']} |",
        f"| Haltezeilen | {b['halte']} ({b['halte_ist']:.0%}, bestellt {b['halte_soll']:.0%}) |",
        f"| Token gesamt (Lernmenge) | {b['lern_token']} |",
        f"| Zeilenlänge, Median | {b['median_token']} Token |",
        f"| Zeilen verworfen, zu kurz | {b['zu_kurz_verworfen']} |",
        f"| Sätze über der Zeilengrenze | {b['satz_zu_lang']} |",
        f"| Wiederholungen, wörtlich | {b['woertlich']} |",
        f"| Wiederholungen, ähnlich | {b['aehnlich']} |",
        f"| Kopf- und Fußzeilen entfernt | {b['kopfzeilen']} |",
        f"| Codeblöcke übersprungen | {b['code']} |",
        f"| Bilder übersprungen | {b['bilder']} |",
        f"| Trennstriche geheilt | {b['trennstriche']} |",
        f"| **Haltezeilen wegen Ähnlichkeit entfernt** | **{b['leck']}** |",
        "",
        "⚑ **Die letzte Zeile ist die wichtigste.** Sie zählt, was die",
        "Teilung nach Abschnitten nicht verhindert hat: dieselbe Aussage",
        "auf beiden Seiten. Steht dort eine große Zahl, wiederholt sich",
        "das Buch stärker als die Abschnittsgrenzen hergeben.",
    ]
    return "\n".join(zeilen) + "\n"


# ------------------------------------------------------------- Ablauf


def bauen(args) -> dict:
    import datetime

    dateien = dateien_sammeln(args.eingabe)
    if not dateien:
        raise SystemExit("[buchkorpus] keine .md gefunden")
    zaehlen, tokenquelle = token_zaehler(args.tokenizer)

    alle_abschnitte: list[Abschnitt] = []
    zaehler = {"code": 0, "bilder": 0, "kommentare": 0, "trennstriche": 0}
    for d in dateien:
        roh = d.read_text(encoding="utf-8", errors="replace")
        felder, roh = vorspann_trennen(roh)
        quelle = felder.get("quelle", d.name)
        geheilt, n = trennstriche_heilen(roh)
        zaehler["trennstriche"] += n
        abschnitte, z = gliedern(geheilt, quelle, args.mit_code)
        for a in abschnitte:
            a.absaetze = [saeubern_inline(p) for p in a.absaetze]
            a.absaetze = [p for p in a.absaetze if p]
        alle_abschnitte.extend(abschnitte)
        for k, v in z.items():
            zaehler[k] += v

    # ⚑ **Erst die Absätze säubern, dann zusammenlegen.** Andersherum
    # verschwinden Seitenköpfe und Doppelungen in der Prosa, mit der sie
    # zusammengelegt wurden, und sind nicht mehr zu erkennen.
    fz = absaetze_filtern(alle_abschnitte)
    zeilen, zz = zeilen_bauen(alle_abschnitte, zaehlen, args.zeilen_token, args.mindest_token)
    zeilen, dz = entdoppeln(zeilen)
    lern, halte = teilen(zeilen, args.halte_anteil)
    halte, leck = leck_pruefen(lern, halte)

    laengen = sorted(zaehlen(z.text) for z in lern) or [0]
    bericht = {
        "datum": datetime.date.today().isoformat(),
        "dateien": len(dateien),
        "abschnitte": len(alle_abschnitte),
        "tokenquelle": tokenquelle,
        "lern": len(lern),
        "halte": len(halte),
        "halte_soll": args.halte_anteil,
        "halte_ist": (len(halte) / max(1, len(lern) + len(halte))),
        "lern_token": sum(laengen),
        "median_token": laengen[len(laengen) // 2],
        "leck": leck,
        **fz,
        **zaehler,
        **zz,
        **dz,
    }
    schreiben(args.name, Path(args.ausgabe), lern, halte, bericht)
    return bericht


# ⚑ Was vor jedem Lauf gesagt wird (Urheberrecht, siehe
# COMPLIANCE/de/Urheberrecht.md). Ein Hinweis und keine Sperre: Ob ein
# Buch verwendet werden darf, kann dieses Skript nicht wissen, der Mensch
# schon.
RECHTEHINWEIS = (
    "[buchkorpus] Hinweis: Trainiere nur mit Material, das du dafuer verwenden darfst "
    "(eigene Werke, freie Lizenzen oder die Erlaubnis der Rechteinhaber). "
    "Siehe COMPLIANCE/de/Urheberrecht.md."
)


def main() -> int:
    umgebung.pruefen()
    p = argparse.ArgumentParser(description="Trainingskorpus aus Markdown bauen")
    p.add_argument("eingabe", nargs="*", help="Dateien oder Ordner mit .md")
    p.add_argument("--name", default="buch")
    p.add_argument("--halte-anteil", type=float, default=0.1)
    p.add_argument("--zeilen-token", type=int, default=256)
    p.add_argument("--mindest-token", type=int, default=12)
    p.add_argument("--tokenizer", default=None, help="Artefaktordner mit tokenizer.json")
    p.add_argument("--mit-code", action="store_true", help="Codeblöcke mitnehmen")
    p.add_argument("--ausgabe", default=str(VORGABE_AUSGABE))
    p.add_argument("--selbsttest", action="store_true")
    args = p.parse_args()

    if args.selbsttest:
        return selbsttest()
    if not args.eingabe:
        p.print_help()
        return 2

    print(RECHTEHINWEIS, file=sys.stderr)
    b = bauen(args)
    print(bericht_schreiben(args.name, b))
    # ⚠️ **Eine Quote, die weit danebenliegt, ist eine Auskunft.** Sie
    # bedeutet, dass die Abschnitte des Buches groesser sind als die
    # bestellte Haltemenge, und der Leser soll das wissen.
    if abs(b["halte_ist"] - b["halte_soll"]) > 0.5 * b["halte_soll"]:
        print(
            f"[buchkorpus] WARNUNG: Haltemenge {b['halte_ist']:.0%} statt "
            f"{b['halte_soll']:.0%}; die Abschnitte sind zu gross fuer diese Quote."
        )
    if b["halte"] == 0:
        print("[buchkorpus] FEHLER: die Haltemenge ist leer")
        return 1
    print(f"[buchkorpus] geschrieben nach {args.ausgabe}")
    return 0


# --------------------------------------------------------- Selbsttest


def selbsttest() -> int:
    """Prüft die Zusagen an einem erfundenen Buch mit bekannten Eigenschaften."""
    import tempfile
    import types

    fehler = 0

    def pruefe(bedingung: bool, was: str) -> None:
        nonlocal fehler
        if not bedingung:
            print(f"  FEHLER: {was}")
            fehler += 1

    with tempfile.TemporaryDirectory(prefix="buchkorpus-") as tmp:
        ein = Path(tmp) / "ein"
        ein.mkdir()
        aus = Path(tmp) / "aus"
        # Ein Buch mit drei Kapiteln. Kapitel 2 enthält **dieselbe**
        # Aussage zweimal, einmal wörtlich und einmal umformuliert.
        (ein / "buch.md").write_text(
            # ⛔️ Der Vorspann, den `buch_zu_md.py` schreibt: Er darf nie
            # als Trainingszeile herauskommen, und er ist die bessere
            # Herkunftsangabe als der Dateiname.
            "---\nquelle: originalbuch.epub\nsha256: abc123\nbloecke: 9\n---\n\n"
            "# Buch\n\n"
            "## Kapitel 1\n\n"
            "Die Schranke des Verfahrens liegt bei vierzig Knoten und wird "
            "nicht ueberschritten. Sie gilt fuer jede Stufe gleichermassen.\n\n"
            "Ein zweiter Absatz in Kapitel eins traegt weitere Aussagen ueber "
            "die Schranke und ihre Herkunft im Verfahren.\n\n"
            "## Kapitel 2\n\n"
            "Der Messwert steigt mit der Temperatur und faellt mit dem Druck, "
            "und das gilt in jedem gemessenen Fall ohne Ausnahme.\n\n"
            "Der Messwert steigt mit der Temperatur und faellt mit dem Druck, "
            "und das gilt in jedem gemessenen Fall ohne Ausnahme.\n\n"
            "```python\nprint('das ist Code und gehoert nicht hinein')\n```\n\n"
            "Seite 17\n\nSeite 17\n\nSeite 17\n\nSeite 17\n\nSeite 17\n\n"
            "## Kapitel 3\n\n"
            "Ein Trai-\nning laeuft nur, wenn die Haltemenge getrennt ist und "
            "die Regel dieselbe bleibt, sonst misst der Lauf nichts.\n\n"
            "Der dritte Abschnitt traegt eine weitere eigene Aussage, damit "
            "die Teilung ueberhaupt etwas zu verteilen hat.\n",
            encoding="utf-8",
        )
        args = types.SimpleNamespace(
            eingabe=[str(ein)], name="probe", halte_anteil=0.34, zeilen_token=64,
            mindest_token=8, tokenizer=None, mit_code=False, ausgabe=str(aus),
        )
        b = bauen(args)
        lern = (aus / "probe_lern.txt").read_text(encoding="utf-8")
        halte = (aus / "probe_halte.txt").read_text(encoding="utf-8")
        ganz = lern + halte

        print("Selbsttest:")
        pruefe("sha256" not in ganz, "der Vorspann steht als Trainingszeile im Korpus")
        pruefe("quelle: originalbuch" not in ganz, "der Vorspann steht im Korpus")
        herkunft0 = json.loads((aus / "probe_herkunft.json").read_text(encoding="utf-8"))
        pruefe(
            herkunft0["lern"][0]["quelle"] == "originalbuch.epub",
            f"die Herkunft nennt die Zwischendatei: {herkunft0['lern'][0]['quelle']}",
        )
        pruefe("das ist Code" not in ganz, "der Codeblock steht im Korpus")
        pruefe(b["code"] == 1, f"Codeblöcke gezählt: {b['code']} statt 1")
        pruefe("Training laeuft" in ganz, "der Trennstrich wurde nicht geheilt")
        pruefe(b["trennstriche"] >= 1, "geheilte Trennstriche nicht gezählt")
        pruefe("Seite 17" not in ganz, "die Kopfzeile steht im Korpus")
        pruefe(b["kopfzeilen"] >= 1, "Kopfzeilen nicht gezählt")
        pruefe(b["woertlich"] == 1, f"wörtliche Wiederholung: {b['woertlich']} statt 1")
        pruefe(len(halte.splitlines()) > 0, "die Haltemenge ist leer")
        pruefe(
            all(z.strip() for z in ganz.splitlines()),
            "es stehen leere Zeilen im Korpus",
        )

        # ⛔️ **Die Zusage, auf die es ankommt: kein Abschnitt steht auf
        # beiden Seiten.** Das ist der Kern der Teilung nach Abschnitten,
        # und es ist direkt prüfbar, ohne auf eine zufällig ähnliche
        # Formulierung im Beispielbuch angewiesen zu sein.
        herkunft = json.loads((aus / "probe_herkunft.json").read_text(encoding="utf-8"))
        lern_abschnitte = {e["abschnitt"] for e in herkunft["lern"]}
        halte_abschnitte = {e["abschnitt"] for e in herkunft["halte"]}
        gemeinsam = lern_abschnitte & halte_abschnitte
        pruefe(not gemeinsam, f"Abschnitte auf beiden Seiten: {sorted(gemeinsam)[:2]}")
        pruefe(bool(halte_abschnitte), "die Haltemenge hat keinen Abschnitt")

        # Und die zweite Zusage: keine Haltezeile mit einem Gegenstück
        # in der Lernmenge, gemessen an der **strengeren** Schwelle.
        index = Aehnlichkeitsindex()
        for z in lern.splitlines():
            index.aufnehmen(z)
        leckend = [z for z in halte.splitlines() if index.treffer(z, LECK_AB) is not None]
        pruefe(not leckend, f"Haltezeilen mit Gegenstück in der Lernmenge: {leckend[:1]}")

        # ⚑ Und dass zwei Läufe dasselbe ergeben.
        aus2 = Path(tmp) / "aus2"
        args.ausgabe = str(aus2)
        bauen(args)
        pruefe(
            (aus / "probe_lern.txt").read_bytes() == (aus2 / "probe_lern.txt").read_bytes(),
            "zwei Läufe ergeben verschiedene Lernmengen",
        )
        pruefe(
            (aus / "probe_halte.txt").read_bytes() == (aus2 / "probe_halte.txt").read_bytes(),
            "zwei Läufe ergeben verschiedene Haltemengen",
        )

    if fehler:
        print(f"[buchkorpus] FAILED: {fehler} Zusage(n) nicht gehalten")
        return 1
    print("[buchkorpus] PASSED: Selbsttest ohne Befund")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
