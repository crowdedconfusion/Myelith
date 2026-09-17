#!/usr/bin/env python3
"""Aus einem Buch wird Markdown: die erste Stufe vor Korpus und Mappe.

# ⚑ Warum diese Stufe getrennt steht

Drei Dinge wollen aus einem Buch gewonnen werden, und sie brauchen
verschiedene Formen: ein **Trainingskorpus** (Zeilen), eine
**Wissensmappe** zum Nachschlagen (Abschnitte) und gelegentlich der
blosse Text. ⚑ **Alle drei fangen mit derselben Arbeit an**, und die ist
die unangenehme: ein Buchformat in sauberen, gegliederten Text zu
verwandeln. Sie steht deshalb einmal hier und nicht dreimal verteilt.

# ⚑ Was ohne fremde Kisten geht, und was nicht

| Format | Weg | Abhängigkeit |
|---|---|---|
| `.epub` | Zip lesen, Lesereihenfolge aus dem OPF, XHTML auswerten | **keine** |
| `.html`, `.xhtml` | derselbe Auswerter | **keine** |
| `.txt` | Überschriften erraten | **keine** |
| `.md` | wird durchgereicht | **keine** |
| `.pdf` | `pdftotext`, sonst `pypdf` | eine von beiden, sonst Absage |

⛔️ **PDF ohne Hilfsmittel gibt es nicht**, und das Werkzeug sagt das
gerade heraus, statt eine leere Datei zu schreiben. Eine stille
Fehlausgabe wäre hier besonders teuer: Sie fiele erst auf, wenn ein
Modell auf nichts trainiert wurde.

⚠️ **Ein Auszug ist kein Buch.** Was herauskommt, trägt Kopfzeilen,
Trennstriche und Bildunterschriften; das Aufräumen macht die nächste
Stufe, die den Korpus baut, und sie zählt dabei mit, wie viel sie
wegwerfen musste.

Aufruf:

```text
python3 TRAINING/korpus/buch_zu_md.py <datei> ... [--ausgabe <ordner>]
python3 TRAINING/korpus/buch_zu_md.py --selbsttest
```
"""

from __future__ import annotations

import argparse
import hashlib
import html.parser
import posixpath
import re
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import umgebung  # noqa: E402

VORGABE_AUSGABE = Path(__file__).resolve().parent / "datasets" / "md"

# Welche Marken als Überschrift gelten, und auf welcher Ebene.
UEBERSCHRIFTEN = {f"h{i}": i for i in range(1, 7)}
# Marken, deren Inhalt nicht in den Text gehört.
STUMM = {"script", "style", "head", "nav", "figure", "figcaption", "table"}


class Textauswerter(html.parser.HTMLParser):
    """Zieht Überschriften und Absätze aus XHTML.

    ⚑ **Ohne fremde Kiste und ohne Regexe auf HTML.** Der Auswerter der
    Standardbibliothek kennt die Verschachtelung; ein Regex kennt sie
    nicht, und an Verschachtelung scheitert er still.
    """

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.bloecke: list[tuple[int, str]] = []  # (Ebene, Text); Ebene 0 = Absatz
        self._puffer: list[str] = []
        self._ebene = 0
        self._stumm = 0

    def handle_starttag(self, tag: str, attrs) -> None:
        if tag in STUMM:
            self._stumm += 1
            return
        if tag in UEBERSCHRIFTEN or tag in {"p", "div", "li", "blockquote"}:
            self._schliessen()
            self._ebene = UEBERSCHRIFTEN.get(tag, 0)
        elif tag == "br":
            self._puffer.append(" ")

    def handle_endtag(self, tag: str) -> None:
        if tag in STUMM:
            self._stumm = max(0, self._stumm - 1)
            return
        if tag in UEBERSCHRIFTEN or tag in {"p", "div", "li", "blockquote"}:
            self._schliessen()

    def handle_data(self, daten: str) -> None:
        if not self._stumm:
            self._puffer.append(daten)

    def _schliessen(self) -> None:
        text = re.sub(r"\s+", " ", "".join(self._puffer)).strip()
        self._puffer = []
        if text:
            self.bloecke.append((self._ebene, text))
        self._ebene = 0

    def fertig(self) -> list[tuple[int, str]]:
        self._schliessen()
        return self.bloecke


# ⛔️ **Der Rahmen, den freie Buchquellen um ihre Buecher legen.**
#
# Gemessen an „Die Verwandlung" von Project Gutenberg: **von 166
# Bloecken gehoeren 22 dem Vorspann und 30 der Lizenz am Ende**, also
# ein knappes Drittel. In einem Trainingskorpus ist das nicht bloss
# Ballast, sondern falscher Lerntext: Das Modell lernt eine
# Nutzungsbedingung mit derselben Ernsthaftigkeit wie den Roman.
#
# ⚑ **Die Marken sind eindeutig und dokumentiert**, deshalb wird hier
# geschnitten und nicht geraten. Wer eine Quelle mit anderem Rahmen hat,
# schneidet von Hand; ⚠️ **erkannt wird nur, was hier steht.**
RAHMEN_ANFANG = re.compile(r"\*\*\*\s*START OF (THE|THIS) PROJECT GUTENBERG", re.I)
RAHMEN_ENDE = re.compile(r"\*\*\*\s*END OF (THE|THIS) PROJECT GUTENBERG", re.I)


def rahmen_schneiden(bloecke: list[tuple[int, str]]) -> tuple[list[tuple[int, str]], int]:
    """Nimmt Vorspann und Lizenz weg, wenn die Marken da sind.

    ⚑ **Beide Marken oder keine.** Fehlt eine, wird nichts geschnitten:
    Ein halb erkannter Rahmen ist gefaehrlicher als keiner, weil er
    entweder den Anfang des Buches oder sein Ende mitnimmt.
    """
    anfang = next((i for i, (_, t) in enumerate(bloecke) if RAHMEN_ANFANG.search(t)), None)
    ende = next(
        (i for i, (_, t) in enumerate(bloecke) if RAHMEN_ENDE.search(t)), None
    )
    if anfang is None or ende is None or ende <= anfang:
        return bloecke, 0
    innen = bloecke[anfang + 1 : ende]
    return innen, len(bloecke) - len(innen)


def bloecke_zu_md(bloecke: list[tuple[int, str]]) -> str:
    aus: list[str] = []
    for ebene, text in bloecke:
        if ebene:
            aus.append(f"{'#' * min(ebene, 6)} {text}")
        else:
            aus.append(text)
    return "\n\n".join(aus).strip() + "\n"


# ------------------------------------------------------------- EPUB


def epub_lesereihenfolge(z: zipfile.ZipFile) -> list[str]:
    """Die Lesereihenfolge aus dem OPF, nicht die Reihenfolge im Zip.

    ⛔️ **Ein Zip hat keine Reihenfolge, ein Buch schon.** Wer die
    Einträge nimmt, wie sie im Archiv liegen, bekommt Kapitel 10 vor
    Kapitel 2, und beim Training ist das ein Buch, das es nicht gibt.
    """
    try:
        behaelter = z.read("META-INF/container.xml").decode("utf-8", "replace")
    except KeyError:
        return []
    treffer = re.search(r'full-path="([^"]+)"', behaelter)
    if not treffer:
        return []
    opf_pfad = treffer.group(1)
    opf = z.read(opf_pfad).decode("utf-8", "replace")
    wurzel = posixpath.dirname(opf_pfad)

    kennung_zu_datei = dict(
        re.findall(r'<item\s[^>]*id="([^"]+)"[^>]*href="([^"]+)"', opf)
    ) | dict(
        (b, a) for a, b in re.findall(r'<item\s[^>]*href="([^"]+)"[^>]*id="([^"]+)"', opf)
    )
    reihenfolge = re.findall(r"<itemref\s[^>]*idref=\"([^\"]+)\"", opf)
    aus = []
    for kennung in reihenfolge:
        href = kennung_zu_datei.get(kennung)
        if href:
            aus.append(posixpath.normpath(posixpath.join(wurzel, href)))
    return aus


def epub_zu_bloecke(pfad: Path) -> list[tuple[int, str]]:
    bloecke: list[tuple[int, str]] = []
    with zipfile.ZipFile(pfad) as z:
        namen = epub_lesereihenfolge(z)
        if not namen:
            namen = sorted(n for n in z.namelist() if n.lower().endswith((".xhtml", ".html", ".htm")))
        for n in namen:
            try:
                roh = z.read(n).decode("utf-8", "replace")
            except KeyError:
                continue
            a = Textauswerter()
            a.feed(roh)
            bloecke.extend(a.fertig())
    return bloecke


# -------------------------------------------------------------- PDF


def pdf_zu_text(pfad: Path) -> str:
    """PDF über `pdftotext`, sonst `pypdf`, sonst eine klare Absage."""
    if shutil.which("pdftotext"):
        r = subprocess.run(
            ["pdftotext", "-layout", "-enc", "UTF-8", str(pfad), "-"],
            capture_output=True,
            text=True,
            check=False,
        )
        if r.returncode == 0:
            return r.stdout
        raise SystemExit(f"[buch_zu_md] pdftotext scheiterte: {r.stderr.strip()[:200]}")
    try:
        import pypdf  # type: ignore

        leser = pypdf.PdfReader(str(pfad))
        return "\n\n".join((s.extract_text() or "") for s in leser.pages)
    except ImportError as f:
        raise SystemExit(
            "[buch_zu_md] PDF braucht `pdftotext` (poppler) oder `pypdf`, "
            f"beides fehlt hier ({f}). Eine leere Ausgabe waere schlimmer als diese Absage."
        ) from f


# -------------------------------------------------------------- Text


def text_zu_bloecke(text: str) -> list[tuple[int, str]]:
    """Reiner Text mit geratenen Überschriften.

    ⚠️ **Geraten und nicht gelesen.** Eine kurze Zeile, die mit „Kapitel"
    beginnt oder durchweg gross geschrieben ist, wird als Überschrift
    genommen. Das trifft oft und nicht immer; wer es genau braucht,
    nimmt EPUB.
    """
    bloecke: list[tuple[int, str]] = []
    absatz: list[str] = []

    def schliessen() -> None:
        t = re.sub(r"\s+", " ", " ".join(absatz)).strip()
        absatz.clear()
        if t:
            bloecke.append((0, t))

    for zeile in text.splitlines():
        z = zeile.strip()
        if not z:
            schliessen()
            continue
        kurz = len(z) <= 70
        woerter = z.split()
        # ⛔️ **Drei Wachen gegen Tabellen und Formeln**, gemessen an einem
        # echten PDF: Ohne sie wurden „)V (1)", „EN-DE EN-FR EN-DE EN-FR"
        # und „GNMT + RL [38] 24.6 39.92 …" zu Ueberschriften. **Eine
        # Ueberschrift, die in Wahrheit eine Tabellenzeile ist, zerlegt
        # den Text an der falschen Stelle**, und danach steht ein Absatz
        # unter einem Titel, der nichts mit ihm zu tun hat.
        ziffernanteil = sum(c.isdigit() for c in z) / max(1, len(z))
        doppelt = len(woerter) != len(set(woerter))
        tragfaehig = (
            len(woerter) <= 8
            and ziffernanteil < 0.2
            and not doppelt
            and sum(c.isalpha() for c in z) >= 3
        )
        ist_kapitel = bool(re.match(r"^(kapitel|chapter|teil|part)\b", z, re.I))
        ist_versal = kurz and tragfaehig and z == z.upper() and any(c.isalpha() for c in z)
        ist_nummeriert = (
            kurz
            and tragfaehig
            # Nach der Nummer muss ein Wort kommen und keine Zahl.
            and bool(re.match(r"^\d+(\.\d+)*\s+[A-Za-zÄÖÜäöü]", z))
        )
        if kurz and (ist_kapitel or ist_versal or ist_nummeriert):
            schliessen()
            bloecke.append((2 if ist_kapitel else 3, z))
            continue
        absatz.append(z)
    schliessen()
    return bloecke


# ------------------------------------------------------------ Ablauf


def umwandeln(pfad: Path) -> str:
    endung = pfad.suffix.lower()
    if endung == ".md":
        return pfad.read_text(encoding="utf-8", errors="replace")
    if endung == ".epub":
        bloecke = epub_zu_bloecke(pfad)
    elif endung in {".html", ".xhtml", ".htm"}:
        a = Textauswerter()
        a.feed(pfad.read_text(encoding="utf-8", errors="replace"))
        bloecke = a.fertig()
    elif endung == ".pdf":
        bloecke = text_zu_bloecke(pdf_zu_text(pfad))
    elif endung in {".txt", ".text"}:
        bloecke = text_zu_bloecke(pfad.read_text(encoding="utf-8", errors="replace"))
    else:
        raise SystemExit(f"[buch_zu_md] unbekanntes Format: {pfad.suffix}")

    bloecke, geschnitten = rahmen_schneiden(bloecke)
    if not bloecke:
        raise SystemExit(f"[buch_zu_md] {pfad.name} ergab keinen Text")
    kopf = (
        "---\n"
        f"quelle: {pfad.name}\n"
        f"sha256: {hashlib.sha256(pfad.read_bytes()).hexdigest()}\n"
        f"bloecke: {len(bloecke)}\n"
        f"rahmen_geschnitten: {geschnitten}\n"
        "---\n\n"
    )
    return kopf + bloecke_zu_md(bloecke)


def zieldatei(ziel: Path, quelle: Path) -> tuple[Path, bool]:
    """Wohin die Ausgabe geht, ohne eine fremde zu ueberschreiben.

    ⛔️ **Zwei Fassungen desselben Buches heissen gleich.** Gemessen an
    `verwandlung.epub` und `verwandlung.html`: Beide schrieben
    `verwandlung.md`, die zweite ueberschrieb die erste **ohne ein
    Wort**, und der Kopf nannte danach eine Quelle, die nicht die des
    ersten Aufrufs war. **Ein Korpus aus so einem Ordner enthaelt ein
    Buch weniger, als sein Erzeuger meldet.**
    """
    aus = ziel / (quelle.stem + ".md")
    if aus.exists() and f"quelle: {quelle.name}" not in aus.read_text(
        encoding="utf-8", errors="replace"
    ):
        return ziel / f"{quelle.stem}-{quelle.suffix.lstrip('.')}.md", True
    return aus, False


def main() -> int:
    umgebung.pruefen()
    p = argparse.ArgumentParser(description="Buch zu Markdown")
    p.add_argument("eingabe", nargs="*")
    p.add_argument("--ausgabe", default=str(VORGABE_AUSGABE))
    p.add_argument("--selbsttest", action="store_true")
    args = p.parse_args()
    if args.selbsttest:
        return selbsttest()
    if not args.eingabe:
        p.print_help()
        return 2

    ziel = Path(args.ausgabe)
    ziel.mkdir(parents=True, exist_ok=True)
    for e in args.eingabe:
        q = Path(e)
        md = umwandeln(q)
        aus, gewichen = zieldatei(ziel, q)
        if gewichen:
            print(f"[buch_zu_md] {q.stem}.md ist belegt, also {aus.name}")
        # ⛔️ **Zwei Fassungen desselben Buches heissen gleich.**
        #
        # 📌 Gemessen an `verwandlung.epub` und `verwandlung.html`: Beide
        # schrieben `verwandlung.md`, die zweite ueberschrieb die erste
        # **ohne ein Wort**, und der Kopf der Datei nannte danach eine
        # Quelle, die nicht die des ersten Aufrufs war. Ein Korpus aus so
        # einem Ordner enthaelt ein Buch weniger, als sein Erzeuger
        # meldet.
        aus.write_text(md, encoding="utf-8")
        ueberschriften = sum(1 for z in md.splitlines() if z.startswith("#"))
        woerter = len(md.split())
        print(f"[buch_zu_md] {q.name}: {ueberschriften} Überschriften, {woerter} Wörter, nach {aus}")
    return 0


# --------------------------------------------------------- Selbsttest


def selbsttest() -> int:
    import tempfile

    fehler = 0

    def pruefe(b: bool, was: str) -> None:
        nonlocal fehler
        if not b:
            print(f"  FEHLER: {was}")
            fehler += 1

    print("Selbsttest:")
    with tempfile.TemporaryDirectory(prefix="buchzumd-") as tmp:
        t = Path(tmp)
        # Ein EPUB von Hand, mit **verdrehter** Reihenfolge im Zip:
        # Kapitel 2 liegt vor Kapitel 1.
        epub = t / "probe.epub"
        with zipfile.ZipFile(epub, "w") as z:
            z.writestr("mimetype", "application/epub+zip")
            z.writestr(
                "META-INF/container.xml",
                '<?xml version="1.0"?><container><rootfiles><rootfile '
                'full-path="OEBPS/buch.opf"/></rootfiles></container>',
            )
            z.writestr(
                "OEBPS/kap2.xhtml",
                "<html><body><h2>Kapitel zwei</h2><p>Der zweite Text steht hier.</p>"
                "<script>weg damit</script></body></html>",
            )
            z.writestr(
                "OEBPS/kap1.xhtml",
                "<html><body><h1>Das Buch</h1><h2>Kapitel eins</h2>"
                "<p>Der erste Text steht hier.</p></body></html>",
            )
            z.writestr(
                "OEBPS/buch.opf",
                '<package><manifest>'
                '<item id="k1" href="kap1.xhtml" media-type="application/xhtml+xml"/>'
                '<item id="k2" href="kap2.xhtml" media-type="application/xhtml+xml"/>'
                "</manifest><spine>"
                '<itemref idref="k1"/><itemref idref="k2"/>'
                "</spine></package>",
            )
        md = umwandeln(epub)
        pruefe("# Das Buch" in md, "die Überschrift der ersten Ebene fehlt")
        pruefe("## Kapitel eins" in md, "die Kapitelüberschrift fehlt")
        pruefe("weg damit" not in md, "der Skriptinhalt steht im Text")
        # ⛔️ Die Zusage: Lesereihenfolge aus dem OPF, nicht aus dem Zip.
        pruefe(
            md.index("Der erste Text") < md.index("Der zweite Text"),
            "die Kapitel stehen in der Reihenfolge des Archivs statt der des Buches",
        )
        pruefe("sha256:" in md.splitlines()[2], "der Kopf trägt keinen Abdruck")

        txt = t / "roh.txt"
        txt.write_text(
            "KAPITEL EINS\n\nEin Absatz, der ueber zwei\nZeilen laeuft.\n\n"
            "1.2 Ein nummerierter Titel\n\nNoch ein Absatz.\n",
            encoding="utf-8",
        )
        md2 = umwandeln(txt)

        # Ein Auszug, wie ihn ein PDF liefert: Formelreste und
        # Tabellenzeilen zwischen echten Ueberschriften.
        txt3 = t / "papier.txt"
        txt3.write_text(
            ")V (1)\n\nEN-DE EN-FR EN-DE EN-FR\n\n"
            "GNMT + RL [38] 24.6 39.92 2.3 1019 1.4 1020\n\n"
            "4 Warum das so ist\n\nEin Absatz mit richtigem Text dahinter.\n",
            encoding="utf-8",
        )
        md3 = umwandeln(txt3)
        pruefe("## KAPITEL EINS" in md2, "die Kapitelzeile wurde nicht erkannt")
        pruefe("### 1.2 Ein nummerierter Titel" in md2, "der nummerierte Titel wurde nicht erkannt")
        # ⛔️ **Tabellen- und Formelzeilen sind keine Ueberschriften**,
        # gemessen an einem echten PDF.
        for keine in ["### )V (1)", "### EN-DE EN-FR EN-DE EN-FR", "### GNMT"]:
            pruefe(keine not in md3, f"{keine} wurde als Ueberschrift genommen")
        pruefe("### 4 Warum das so ist" in md3, "die echte Ueberschrift daneben fehlt")
        pruefe(
            "Ein Absatz, der ueber zwei Zeilen laeuft." in md2,
            "der weiche Umbruch wurde nicht geschlossen",
        )

        # ⛔️ **Der Rahmen der Quelle faellt weg, und zwar ganz.**
        #
        # Gemessen an einem echten Buch: ein gutes Drittel der Bloecke
        # war Lizenztext. **Ein Modell lernt eine Nutzungsbedingung mit
        # derselben Ernsthaftigkeit wie den Roman.**
        rahmen = t / "mit_rahmen.txt"
        rahmen.write_text(
            "The Project Gutenberg eBook of Etwas\n\n"
            "Lizenzgeschwaetz im Vorspann, das nicht ins Training gehoert.\n\n"
            "*** START OF THE PROJECT GUTENBERG EBOOK ETWAS ***\n\n"
            "Der eigentliche Text des Buches steht hier und nur hier.\n\n"
            "*** END OF THE PROJECT GUTENBERG EBOOK ETWAS ***\n\n"
            "Die volle Lizenz, die ebenfalls nicht ins Training gehoert.\n",
            encoding="utf-8",
        )
        md4 = umwandeln(rahmen)
        pruefe("Der eigentliche Text" in md4, "der Text zwischen den Marken fehlt")
        pruefe("Lizenzgeschwaetz" not in md4, "der Vorspann steht noch drin")
        pruefe("Die volle Lizenz" not in md4, "die Lizenz am Ende steht noch drin")
        # Fuenf: die beiden Marken, der Vorspann aus zwei Bloecken und
        # die Lizenz am Ende. ⚑ **Die Marken zaehlen mit**, sie sind
        # selbst Rahmen und kein Text.
        pruefe("rahmen_geschnitten: 5" in md4, f"die Zahl stimmt nicht: {md4[:200]}")

        # ⛔️ **Beide Marken oder keine.** Ein halb erkannter Rahmen nimmt
        # entweder den Anfang des Buches mit oder sein Ende.
        halb = t / "halber_rahmen.txt"
        halb.write_text(
            "*** START OF THE PROJECT GUTENBERG EBOOK ETWAS ***\n\n"
            "Der Text, der ohne Endmarke nicht angetastet werden darf.\n",
            encoding="utf-8",
        )
        md5 = umwandeln(halb)
        pruefe("Der Text, der ohne Endmarke" in md5, "bei halbem Rahmen wurde geschnitten")
        pruefe("rahmen_geschnitten: 0" in md5, "halber Rahmen wurde gezaehlt")

        # ⛔️ **Kein stilles Ueberschreiben.**
        ziel = t / "ziel"
        ziel.mkdir()
        (ziel / "buch.md").write_text("---\nquelle: buch.epub\n---\n\nText.\n", encoding="utf-8")
        gleich, gewichen1 = zieldatei(ziel, Path("/woauchimmer/buch.epub"))
        anders, gewichen2 = zieldatei(ziel, Path("/woauchimmer/buch.html"))
        pruefe(not gewichen1 and gleich.name == "buch.md", "dieselbe Quelle soll denselben Namen haben")
        pruefe(gewichen2 and anders.name == "buch-html.md", f"fremde Quelle ueberschreibt: {anders.name}")

    if fehler:
        print(f"[buch_zu_md] FAILED: {fehler} Zusage(n) nicht gehalten")
        return 1
    print("[buch_zu_md] PASSED: Selbsttest ohne Befund")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
