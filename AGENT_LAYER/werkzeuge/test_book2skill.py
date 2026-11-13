#!/usr/bin/env python3
"""Proben fuer book2skill.py: eigenstaendig, ohne Abhaengigkeiten; PDF nur mit pypdf."""

from __future__ import annotations

import base64
import filecmp
import hashlib
import json
import os
import pathlib
import re
import stat
import sys
import tempfile
import textwrap
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import book2skill as b  # noqa: E402

PNG = b"\x89PNG\r\n\x1a\nbild"
OPF = """<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>Über Bäume</dc:title><dc:creator>A. Förster</dc:creator><dc:rights>CC BY 4.0</dc:rights>
 </metadata>
 <manifest>
  <item id="a" href="kap/eins.xhtml" media-type="application/xhtml+xml"/>
  <item id="b" href="kap/zwei.xhtml" media-type="application/xhtml+xml"/>
 </manifest>
 <spine><itemref idref="b"/><itemref idref="a"/></spine>
</package>"""
SATZ = "Die Wurzeln versorgen den Baum mit Wasser und Mineralien aus dem Boden. "
EINS = f"""<html><head><title>x</title><style>p{{}}</style></head><body>
<h1>Wurzeln</h1><p>Erster   <em>Absatz</em> mit
Umbruch. {SATZ * 6}</p><img src="../bilder/a.png" alt="Eine Wurzel"/><img src="../bilder/fehlt.png"/>
<ul><li>eins</li><li>zwei</li></ul>
<h2>Feine Wurzeln</h2><pre>code   bleibt</pre>
<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
<script>boese()</script></body></html>"""
ZWEI = f'<html><body><h1>Stamm</h1><p>Siehe <a href="https://example.org/x">Quelle</a> und <a href="eins.xhtml">innen</a>. Der Stamm trägt die Krone und leitet die Säfte nach oben durch das Holz. {SATZ * 5}</p></body></html>'


def epub(pfad: pathlib.Path, teile: dict | None = None) -> None:
    with zipfile.ZipFile(pfad, "w") as z:
        z.writestr("mimetype", "application/epub+zip")
        z.writestr("META-INF/container.xml", '<container><rootfiles><rootfile full-path="OEBPS/c.opf"/></rootfiles></container>')
        z.writestr("OEBPS/c.opf", OPF)
        z.writestr("OEBPS/kap/eins.xhtml", EINS)
        z.writestr("OEBPS/kap/zwei.xhtml", ZWEI)
        z.writestr("OEBPS/bilder/a.png", PNG)
        for n, i in (teile or {}).items():
            z.writestr(n, i)


W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"'


def docx(pfad: pathlib.Path) -> None:
    koerper = (
        '<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Kapitel Docx</w:t></w:r></w:p>'
        f"<w:p><w:r><w:t>{SATZ * 6}</w:t></w:r></w:p>"
        '<w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>Listenpunkt</w:t></w:r></w:p>'
        '<w:p><w:r><w:drawing><a:blip r:embed="rId7"/></w:drawing></w:r></w:p>'
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Zelle</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"
    )
    with zipfile.ZipFile(pfad, "w") as z:
        z.writestr("word/document.xml", f"<w:document {W}><w:body>{koerper}</w:body></w:document>")
        z.writestr("word/_rels/document.xml.rels", '<Relationships><Relationship Id="rId7" Target="media/b.png"/></Relationships>')
        z.writestr("word/media/b.png", PNG)
        z.writestr("docProps/core.xml", '<c xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Docx Werk</dc:title><dc:creator>B</dc:creator></c>')


def odt(pfad: pathlib.Path) -> None:
    t = 'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink"'
    inhalt = (
        f"<office:document-content {t}><office:body><office:text>"
        '<text:h text:outline-level="1">Kapitel Odt</text:h>'
        f"<text:p>{SATZ * 6}</text:p>"
        "<text:list><text:list-item><text:p>Punkt</text:p></text:list-item></text:list>"
        '<text:p><draw:frame><draw:image xlink:href="Pictures/c.png"/></draw:frame></text:p>'
        "</office:text></office:body></office:document-content>"
    )
    with zipfile.ZipFile(pfad, "w") as z:
        z.writestr("content.xml", inhalt)
        z.writestr("Pictures/c.png", PNG)


def fb2(pfad: pathlib.Path) -> None:
    pfad.write_text(
        '<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">'
        "<description><title-info><author><first-name>C</first-name><last-name>D</last-name></author><book-title>Fb Werk</book-title></title-info></description>"
        f"<body><section><title><p>Kapitel Fb</p></title><p>{SATZ * 6}</p><image l:href=\"#d.png\"/></section></body>"
        f'<binary id="d.png" content-type="image/png">{base64.b64encode(PNG).decode()}</binary></FictionBook>',
        encoding="utf-8",
    )


WORTE = ["Wurzeln", "Blaetter", "Rinde", "Krone"]


def pdf_seite(i: int) -> bytes:
    w = WORTE[i % len(WORTE)]
    zeilen = [(11, 720, f"Absatz der Seite {i} ueber {w} im dichten Mischwald bricht und endet.")]
    if i == 0:
        zeilen = [
            (24, 720, "1. Erstes Kapitel"),
            (11, 690, "Der erste Absatz des Buches bricht am Ende der Zei-"),
            (11, 676, "le um und soll zusammenkommen. Die Wurzeln tragen"),
            (11, 662, "den Baum, und das gilt in jedem Wald der Erde."),
        ]
    teile = ["BT"]
    for g, y, t in zeilen + [(11, 40, "Laufende Fusszeile"), (11, 30, str(i + 1))]:
        teile.append(f"/F1 {g} Tf 1 0 0 1 72 {y} Tm ({t}) Tj")
    teile.append("ET")
    return "\n".join(teile).encode("latin-1")


def pdf(pfad: pathlib.Path, seiten: int = 4) -> None:
    objekte: list[bytes] = []
    kids = " ".join(f"{3 + i * 2} 0 R" for i in range(seiten))
    objekte.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    objekte.append(f"<< /Type /Pages /Count {seiten} /Kids [{kids}] >>".encode())
    for i in range(seiten):
        strom = pdf_seite(i)
        objekte.append(
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {4 + i * 2} 0 R "
            f"/Resources << /Font << /F1 {3 + seiten * 2} 0 R >> >> >>".encode()
        )
        objekte.append(b"<< /Length %d >>\nstream\n" % len(strom) + strom + b"\nendstream")
    objekte.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    aus = bytearray(b"%PDF-1.4\n")
    orte = []
    for n, o in enumerate(objekte, 1):
        orte.append(len(aus))
        aus += b"%d 0 obj\n" % n + o + b"\nendobj\n"
    xref = len(aus)
    aus += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objekte) + 1)
    for o in orte:
        aus += b"%010d 00000 n \n" % o
    aus += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objekte) + 1, xref)
    pfad.write_bytes(bytes(aus))


FAKE = textwrap.dedent(
    """\
    #!/usr/bin/env python3
    import json, os, re, sys
    a = sys.argv
    assert a[1] == "fragen" and "--ohne-denken" in a and "--gierig" in a, a
    ein, aus = a[a.index("--datei") + 1], a[a.index("--aus") + 1]
    grenze = int(os.environ.get("FAKE_GRENZE", "1000000"))
    n = 0
    with open(aus, "a", encoding="utf-8") as f:
        for z in open(ein, encoding="utf-8"):
            v = json.loads(z)
            if n >= grenze:
                sys.exit(7)
            n += 1
            with open(os.environ["FAKE_ZAEHLER"], "a") as zz:
                zz.write(v["id"] + "\\n")
            m = re.search(r"<material>\\n(.*?)\\n</material>", v["text"], re.S).group(1) if "<material>" in v["text"] else v["text"]
            assert ("--ohne-grundsaetze" in a) == ("Übersetze es" in v["text"] or "翻译为" in v["text"]), v["text"][-80:]
            if "翻译为" in v["text"]:
                m = v["text"].split("\\n\\n", 1)[1]
                with open(os.environ["FAKE_ZAEHLER"] + ".schnell", "a") as zz:
                    zz.write(v["id"] + "\\n")
                if os.environ.get("FAKE_SCHNELL") == "scheitert":
                    f.write(json.dumps({"id": v["id"], "antwort": "Okay, let's tackle this translation of the user text now."}, ensure_ascii=False) + "\\n")
                    continue
            if "Übersetze es" in v["text"] or "翻译为" in v["text"]:
                fassung = os.environ.get("FAKE_UEBERSETZUNG", "gut")
                antwort = {"gut": "Der Wald: " + " ".join(f"{'der die und das nicht ist'.split()[i % 6]} w{i}" for i in range(max(len(m.split()) // 2, 1))), "leer": "", "schleife": "Das ist immer derselbe lange Satz hier. " * 30}[fassung]
                f.write(json.dumps({"id": v["id"], "antwort": antwort}, ensure_ascii=False) + "\\n")
                continue
            satz = re.sub(r"\\s+", " ", re.sub(r"[#*!\\[\\]()-]", " ", m)).strip()
            echt = satz[:90]
            f.write(json.dumps({"id": v["id"], "antwort":
                "Hier sind die Punkte:\\n"
                f"- {echt}.\\n"
                "- Die Quantenschleife erhöht den Ertrag um 987654 Prozent bei jedem Rechenschritt\\n"
                f"- {echt[:70]} und {'Vorgehensweisen' if 'Vorgehensweisen' in v['text'] else 'Notizen'}\\n"
                f"<think>\\n- {echt[:60]} geheim.\\n</think>\\n"}, ensure_ascii=False) + "\\n")
    """
)


def einzeln(d: pathlib.Path) -> None:
    assert b.belegt("Die Wurzeln versorgen den Baum mit Wasser", SATZ)
    assert not b.belegt("Die Wurzeln versorgen den Baum mit 12 Litern Wasser", SATZ), "Zahl nicht im Material"
    assert not b.belegt("Delfine springen hoch ueber glitzerndes Meerwasser", SATZ)
    assert not b.belegt("Baum", SATZ), "zu kurz"
    w = "Die Wurzeln halten den Baum im Boden. " + "Vögel singen morgens laut. " * 6 + "Die Genauigkeit stieg um 1,64 Prozent."
    assert not b.belegt("Die Wurzeln erreichen eine Genauigkeit von 1,64 Prozent", w), "ueber das ganze Stueck verstreut"
    assert b.belegt("Die Genauigkeit stieg um 1,64 Prozent", w)
    assert b.zahlen("133 756 und 133.756 und 1,64") == {"133756", "1.64"}, b.zahlen("133 756 und 133.756 und 1,64")
    assert not b.belegt("Die Wurzeln halten den Baum 64 Jahre", "Die Wurzeln halten den Baum 1,64 Jahre lang.")
    m = set(b.woerter(SATZ))
    assert b.spiegelt("Nenne wichtigste Vorgehensweisen", "Nenne die wichtigsten Vorgehensweisen", m)
    assert not b.spiegelt("Die Wurzeln versorgen den Baum", "Nenne die wichtigsten Vorgehensweisen", m)
    assert b.punkte_lesen("Hier:\n- eins zwei.\n* drei.\n1. vier.\n- eins zwei.\nText\n<think>- nein.</think>\n- abgesch") == ["eins zwei.", "drei.", "vier."], "doppelt, Denkteil, abgeschnitten"
    assert b.punkte_lesen("- Der Bericht prü!, was gebaut ist.\n- 133 756 2 451 30 30.\n- Das bleibt stehen.") == ["Das bleibt stehen."]
    assert b.kurzname("../../etc/passwd") == "etc-passwd" and b.kurzname("!!!") == "ohne-titel"
    t = "Arbeit Arbeit Arbeit Baum Baum Baum und dann arbeitist gut. Wurzel wurzelTrainings Stamm. wurzel wurzel Trainings"
    r = b.zusammengezogene_trennen(t)
    assert "arbeit ist" in r and "wurzel Trainings" in r, r
    z = b.zusammengezogene_trennen("Schluesselwort Schluessel Schluessel Schluessel Wort Wort Wort")
    assert "Schluesselwort" in z, "eine Zusammensetzung bleibt ganz"
    assert b.sprache("der die und nicht ist der die und") == "de" and b.sprache("the and of is that the and") == "en"
    themen = ["Wurzeln tragen schwere Baumkronen", "Blätter fangen helles Sonnenlicht", "Pilze zersetzen totes Laubwerk", "Vögel verbreiten reife Samen", "Regen füllt tiefe Grundwasserspeicher", "Rinde schützt empfindliches Kambium", "Würmer lockern dichten Waldboden", "Moose speichern reichlich Feuchtigkeit"]
    k = "\n\n".join(f"{t} im gemischten Bergwald während langer Sommermonate." for t in themen)
    a1, a2 = b.auszug(k, 4), b.auszug(k, 4)
    assert a1 == a2 and len(a1) == 4, "ein Auszug ist bestimmt"
    assert not b._ist_satz("for (i = 0; i < n; i++) { x[i] = y[i] ; } und dann noch mehr Text dazu")
    assert b._ist_satz("كان الشتاء طويلا وكان النهر يتجمد كثيرا كل عام")
    assert b._ist_satz("החורף היה ארוך והנהר קפא לעתים קרובות")
    assert not b._ist_satz("und hier beginnt ein Satz mitten drin ohne Anfang")
    assert b._ebene_nummeriert("6.2 Ganzzahlige Ausführung") == 2 and b._ebene_nummeriert("3. Architektur") == 1
    assert b._ebene_nummeriert("A.1 Kern-Datentypen") == 2 and b._ebene_nummeriert("Anhang B: Herleitung") == 1
    assert b._ebene_nummeriert("Normaler Satz") is None
    k2 = b.kapitel_bilden([("f", "Inhalt", 0, "x" * 900, 1), ("f", "Eins", 0, "y" * 900, 1), ("f", "Unter", 0, "u" * 900, 2), ("f", "Zwei", 0, "z" * 900, 1), ("f", "Drei", 0, "w" * 900, 1)])
    assert [n[0] for n in k2] == ["Eins", "Zwei", "Drei"] and k2[0][1] == [1, 2], k2
    U = chr(92) + "u"
    rtf = r"{\rtf1\ansi{\fonttbl{\f0 Arial;}}{\info{\title x}}Erste\par Zweite \'e4 " + U + r"8364? Ende{\*\gen weg}\par}"
    t = b.rtf_nach_text(rtf)
    assert "Erste" in t and "Zweite ä € Ende" in t and "Arial" not in t and "weg" not in t, t


def formate(d: pathlib.Path) -> None:
    fa = {"werk.docx": (docx, "Kapitel Docx", "Docx Werk"), "werk.odt": (odt, "Kapitel Odt", "werk"), "werk.fb2": (fb2, "Kapitel Fb", "Fb Werk")}
    for name, (bau, kopf, titel) in fa.items():
        p = d / name
        bau(p)
        o = b.umwandeln(p, d / f"z-{name}")
        tx = (o / "buch.md").read_text(encoding="utf-8")
        assert kopf in tx and "Wurzeln versorgen" in tx, (name, tx[:300])
        assert (o / "SKILL.md").exists() and any((o / "bilder").iterdir()), name
        assert "![](bilder/" in tx, (name, tx)
        assert json.loads((o / "manifest.json").read_text())["titel"] == titel, name
    h = d / "seite.html"
    h.write_text(f"<html><head><title>Seite &amp; Titel</title></head><body><h1>Kopf</h1><p>{SATZ * 6}</p></body></html>", encoding="utf-8")
    assert json.loads((b.umwandeln(h, d / "z-h") / "manifest.json").read_text())["titel"] == "Seite & Titel"
    r = d / "t.rtf"
    r.write_text(r"{\rtf1 " + SATZ * 6 + r"\par}", encoding="utf-8")
    assert "Wurzeln" in (b.umwandeln(r, d / "z-r") / "buch.md").read_text(encoding="utf-8")
    t = d / "t.txt"
    t.write_text(SATZ * 6, encoding="utf-8")
    assert (b.umwandeln(t, d / "z-t") / "SKILL.md").exists()
    x = d / "z.bin"
    x.write_text("x")
    try:
        b.umwandeln(x, d / "z-x")
    except b.Fehler as f:
        assert "Format" in str(f) and "epub" in str(f), f
    else:
        raise AssertionError("bin")
    m = d / "z.mobi"
    m.write_bytes(b"x")
    try:
        b.umwandeln(m, d / "z-m")
    except b.Fehler as f:
        assert "MOBI" in str(f), f
    else:
        raise AssertionError("mobi")
    q = d / "boese.xhtml"
    q.write_text('<!DOCTYPE x [<!ENTITY a "aaa">]><html>&a;</html>', encoding="utf-8")
    e = d / "bombe.epub"
    with zipfile.ZipFile(e, "w") as z:
        z.writestr("META-INF/container.xml", '<!DOCTYPE c [<!ENTITY a "b">]><c>&a;</c>')
    try:
        b.umwandeln(e, d / "z-e")
    except b.Fehler as f:
        assert "Entit" in str(f), f
    else:
        raise AssertionError("Entitaeten")


def mit_pypdf(d: pathlib.Path) -> None:
    try:
        import pypdf  # noqa: F401
    except ImportError:
        print("[book2skill] PDF uebersprungen (kein pypdf)")
        return
    p = d / "probe.pdf"
    pdf(p)
    o = b.umwandeln(p, d / "z-p", nur_umwandeln=True)
    t = (o / "buch.md").read_text(encoding="utf-8")
    assert "# 1. Erstes Kapitel" in t, t
    assert "Zeile um und soll" in t, "die Trennung am Zeilenende ist aufgeloest: " + t[:300]
    assert "Laufende Fusszeile" not in t, "die Fusszeile ist weg: " + t
    assert not any(z.strip().isdigit() for z in t.splitlines()), "die Seitenzahl ist weg"
    assert not (o / "SKILL.md").exists()
    r = sorted((o / "seiten").iterdir())
    assert "Seite 1" in r[0].read_text(encoding="utf-8"), r[0].read_text(encoding="utf-8")[:200]


def bild_pdf(pfad: pathlib.Path, seiten: list) -> None:
    """Ein PDF aus Seiten ohne Textebene (`None`, mit Seitenbild) und Seiten mit Text."""
    objekte = ["<< /Type /Catalog /Pages 2 0 R >>", ""]
    roh = b"\xff" * 4
    kinder = []
    teile: dict = {}

    def neu(inhalt, strom=None) -> int:
        objekte.append(inhalt)
        if strom is not None:
            teile[len(objekte)] = strom
        return len(objekte)

    bild = neu(f"<< /Type /XObject /Subtype /Image /Width 800 /Height 1000 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {len(roh)} >>", roh)
    schrift = neu("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    for text in seiten:
        strom = b"q 612 0 0 792 0 0 cm /B Do Q" if text is None else f"BT /F 12 Tf 72 700 Td ({text}) Tj ET".encode()
        inhalt = neu(f"<< /Length {len(strom)} >>", strom)
        res = f"<< /XObject << /B {bild} 0 R >> >>" if text is None else f"<< /Font << /F {schrift} 0 R >> >>"
        kinder.append(neu(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources {res} /Contents {inhalt} 0 R >>"))
    objekte[1] = f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in kinder)}] /Count {len(kinder)} >>"
    aus = bytearray(b"%PDF-1.5\n")
    orte = []
    for i, o in enumerate(objekte, 1):
        orte.append(len(aus))
        aus += f"{i} 0 obj\n{o}\n".encode()
        if i in teile:
            aus += b"stream\n" + teile[i] + b"\nendstream\n"
        aus += b"endobj\n"
    x = len(aus)
    aus += f"xref\n0 {len(objekte) + 1}\n0000000000 65535 f \n".encode()
    for o in orte:
        aus += f"{o:010d} 00000 n \n".encode()
    aus += f"trailer\n<< /Size {len(objekte) + 1} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
    pfad.write_bytes(bytes(aus))


GROSS = b"\x89PNG\r\n\x1a\n" + b"x" * b.BILD_MINDESTBYTES


def scan_und_bilder(d: pathlib.Path) -> None:
    try:
        import pypdf  # noqa: F401
    except ImportError:
        print("[book2skill] Scan uebersprungen (kein pypdf)")
        return
    p = d / "scan.pdf"
    bild_pdf(p, [None, None])
    try:
        b.umwandeln(p, d / "s0", nur_umwandeln=True)
    except b.Fehler as f:
        assert "myl scan" in str(f) and "--myl" in str(f), f
    else:
        raise AssertionError("ein Scan ohne Konverter wurde still leer")

    # `myl scan` wird nachgestellt; sein echter Lauf steht in CLIENT/myl-scan.
    gerufen: list = []
    gesehene_schrift: list = []

    def wandler(_python, auftrag: pathlib.Path) -> None:
        a = json.loads(auftrag.read_text(encoding="utf-8"))
        ort = pathlib.Path(a["arbeit"])
        gesehene_schrift.append(a.get("schrift"))
        if a.get("pakete"):
            (ort / "schrift.txt").write_text("kyrillisch", encoding="utf-8")
        for inp, aus in a.get("bildtexte", []):
            assert pathlib.Path(inp).exists(), inp
            pathlib.Path(aus).write_text(json.dumps(["Achse A", "Wert 7"]), encoding="utf-8")
        for pk in a.get("pakete", []):
            assert pk["ocr"], "gescannte Seiten brauchen die Texterkennung"
            for n in pk["seiten"]:
                gerufen.append(n)
                (ort / f"b{n}.png").write_bytes(GROSS + str(n).encode())
                kopf = "Lebender Kolumnentitel\n\n" if os.environ.get("B2S_KOPF") else ""
                md = f"{kopf}## {n}.1 Abschnitt {n}\n\nErkannter Satz auf Seite {n}, der lang genug fuer einen Absatz ist.\n\n{b.SCAN_MARKE}\n\n| A | B |\n|---|---|\n| 1 | 2 |"
                titel = "Die echte Titelei" if os.environ.get("B2S_KOPF") else ""
                (ort / f"seite-{n:06d}.json").write_text(json.dumps({"md": md, "bilder": [f"b{n}.png"], "titel": titel}), encoding="utf-8")
                if n == 1 and os.environ.get("B2S_ABBRUCH"):
                    raise b.Fehler("abgebrochen")

    alt_aufrufen = b._scan_aufrufen
    b._scan_aufrufen = wandler
    try:
        myl = d / "myl-probe.py"
        zaehl = d / "bilder-gefragt"
        myl.write_text(
            textwrap.dedent(f"""\
            #!{sys.executable}
            import pathlib, sys
            frage = sys.argv[sys.argv.index("--frage") + 1]
            assert "Achse A" in frage, frage
            assert "--genau" in sys.argv
            open({str(zaehl)!r}, "a").write(pathlib.Path(sys.argv[2]).name + chr(10))
            print("Ein Diagramm mit einer Achse und dem Wert sieben, alles gut lesbar.")
            """),
            encoding="utf-8",
        )
        myl.chmod(myl.stat().st_mode | stat.S_IEXEC)

        os.environ["B2S_ABBRUCH"] = "1"
        z = d / "s1"
        try:
            b.umwandeln(p, z, nur_umwandeln=True, scan=b.Scan(myl="x"), bild_myl=str(myl))
        except b.Fehler:
            pass
        else:
            raise AssertionError("der Abbruch kam nicht an")
        del os.environ["B2S_ABBRUCH"]
        gerufen.clear()
        o = b.umwandeln(p, z, nur_umwandeln=True, scan=b.Scan(myl="x"), bild_myl=str(myl))
        assert gerufen == [2], f"schon gerechnete Seiten kamen erneut: {gerufen}"
        t = (o / "buch.md").read_text(encoding="utf-8")
        assert "Erkannter Satz auf Seite 2" in t and "| A | B |\n|---|---|\n| 1 | 2 |" in t, t
        assert "## 2.1 Abschnitt 2" in t, t
        assert t.count("nicht nachgeprüft") == 2, t
        assert t.count("von einem Sehmodell beschrieben") == 2 and t.count("Achse A | Wert 7") == 2, t
        assert b.SCAN_MARKE not in t, t
        j = json.loads((o / "manifest.json").read_text(encoding="utf-8"))
        assert j["scan"]["seiten_erkannt"] == 2 and j["abbildungen"]["beschrieben"] == 2, j
        assert j["scan"]["schrift"] == "kyrillisch" and gesehene_schrift[0] == "auto" and gesehene_schrift[-1] == "kyrillisch", (j, gesehene_schrift)
        assert not list(z.glob(".scan-*")), "der Arbeitsordner ist nach Erfolg weg"

        # Eine Seite mit Textebene bleibt beim schnellen Weg; nur die ohne geht zum Konverter.
        gerufen.clear()
        g = d / "gemischt.pdf"
        bild_pdf(g, ["Eine Seite mit richtiger Textebene und genug Zeichen darauf.", None])
        o = b.umwandeln(g, d / "s2", nur_umwandeln=True, scan=b.Scan(myl="x", bilder=False))
        assert gerufen == [2], gerufen
        t = (o / "buch.md").read_text(encoding="utf-8")
        assert "richtiger Textebene" in t and "Erkannter Satz auf Seite 2" in t and "Erkannter Satz auf Seite 1" not in t, t

        # Ohne Konverter kommen gemischte Seiten durch, und das Manifest sagt, was fehlt.
        o = b.umwandeln(g, d / "s3", nur_umwandeln=True)
        assert json.loads((o / "manifest.json").read_text(encoding="utf-8"))["scan"]["seiten_ohne_text"] == [2]

        # Skill und Abschnitte passen ins Nachschlagebudget des Agenten (8 000 Bytes).
        lang = "".join(f"# Kapitel {i}\n\n" + ("Die Wurzeln des Baumes versorgen ihn mit Wasser und Mineralien. " * 200) + "\n\n" for i in range(40))
        lt = d / "lang.md"
        lt.write_text(lang, encoding="utf-8")
        o = b.umwandeln(lt, d / "s8")
        sk = (o / "SKILL.md").read_text(encoding="utf-8")
        assert len(sk.split("---\n", 2)[2].encode("utf-8")) <= b.SKILL_ZEICHEN, len(sk)
        assert "Wurzeln" in sk.split("---\n", 2)[1], "das haeufige Wort steht in den Stichworten"
        groesste = max(len(x.read_bytes()) for x in (o / "seiten").iterdir())
        assert groesste + b.SKILL_ZEICHEN <= 8000, groesste
        assert "Kapitel 0" in (o / "SKILL.md").read_text(encoding="utf-8")

        # Namen: lesbar in jeder Schrift, und nichts verschwindet.
        for roh, soll in [("Über Bäume", "ueber-baeume"), ("Straße & Maß: Teil 2", "strasse-mass-teil-2"), ("Война и мир", "война-и-мир"), ("道德經", "道德經"), ("한국어 책", "한국어-책"), ("हिंदी किताब", "हिंदी-किताब"), ("../../x", "x")]:
            assert b.kurzname(roh) == soll, (roh, b.kurzname(roh))
        # Titel - Autor - Kennung, und nur echte Kennungen.
        assert b.isbn_gueltig("9783161484100") and b.isbn_gueltig("316148410X") and not b.isbn_gueltig("9783161484101")
        assert b.kennungen_finden(["ISBN 978-3-16-148410-0", "Literatur: ISBN 978-3-16-148410-1"]) == ("9783161484100", "")
        assert b.kennungen_finden(["DOI 10.5281/zenodo.21734240", "DOl 10.5281/zenodo.21734240.", "siehe 10.1000/fremd"]) == ("", "10.5281/zenodo.21734240")
        assert b.kennungen_finden(["siehe 10.1000/fremd"]) == ("", ""), "eine DOI im Fliesstext ist nicht die des Werks"
        assert b.autor_der_seiten(["Mein Werk\n\nJoschka Benjamin Hänsler August 2026\n\nText."], "Mein Werk") == "Joschka Benjamin Hänsler"
        assert b.autor_der_seiten(["Suhrkamp Verlag\n\nDritte Auflage"], "x") == ""
        assert not b.autor_gueltig("Administrator") and b.autor_gueltig("Immanuel Kant")
        w = b.Werk("Kritik: der / reinen? Vernunft", "Immanuel Kant", doi="10.1/ab")
        assert b.ordnername(w) == "Kritik_ der _ reinen_ Vernunft - Immanuel Kant - 10.1_ab", b.ordnername(w)
        w.isbn = "9783161484100"
        assert b.ordnername(w).endswith(" - 9783161484100"), "die ISBN geht der DOI vor"
        w2 = b.Werk("Nur Titel")
        assert b.ordnername(w2) == "Nur Titel"
        assert len(b.ordnername(b.Werk("й" * 500)).encode()) <= 200

        # Der Titel eines Scans kommt von der Titelseite, nicht aus dem Dateinamen.
        assert b.titel_der_seiten({1: {"titel": "Kritik der reinen Vernunft"}}, "scan_0012") == "Kritik der reinen Vernunft"
        assert b.titel_der_seiten({1: {"titel": ""}, 2: {"titel": "12"}, 3: {"titel": "1.2 Einleitung"}}, "scan_0012") == "scan_0012"
        assert b.titel_der_seiten({9: {"titel": ""}, 1: {"titel": ""}, 2: {"titel": "Titel auf Seite zwei"}}, "s") == "Titel auf Seite zwei"
        assert b.titel_der_seiten({5: {"titel": "jungen Werther\u2014,"}}, "s") == "jungen Werther"
        assert b.titel_der_seiten({1: {"titel": "Die Stadt wurde gebaut. Die Br\u00fccke hat B\u00f6gen. Jedes Jahr kam Salz."}}, "s") == "s"
        assert b.titel_der_seiten({1: {"titel": "\u5546\u8cc8\u81ea\u9060\u65b9\u800c\u81f3\u3002 \u51ac\u9577\u800c\u6cb3\u5e38\u51b0\u3002 \u5e02\u6c11\u9078\u65b0\u8b70\u6703\u3002"}}, "s") == "s"
        assert b.titel_der_seiten({1: {"titel": "Faust. Eine Trag\u00f6die von Dr. Goethe"}}, "s") == "Faust. Eine Trag\u00f6die von Dr. Goethe"
        assert b.titel_der_seiten({n: {"titel": ""} for n in range(1, 8)} | {8: {"titel": "Auf Blatt acht"}}, "s") == "Auf Blatt acht"
        assert b.titel_der_seiten({n: {"titel": ""} for n in range(1, 9)} | {9: {"titel": "Zu weit hinten"}}, "s") == "s"

        # Bei einem dicken Buch rechnet das Modell nicht jedes Stueck.
        class ZaehlModell:
            def __init__(self) -> None:
                self.fragen_n = 0

            def fragen(self, texte):
                self.fragen_n += len(texte)
                return {t: None for t in texte}

        ab = b.zerlegen([("o", "".join(f"# K{i}\n\n" + " ".join(f"Der Baum {i}{j} steht mit seinen Wurzeln {j} tief im Boden des Waldes {i}." for j in range(60)) + "\n\n" for i in range(5)))])
        alt_stuecke, alt_kapitel = b.MODELL_STUECKE_HOECHSTENS, b.MODELL_KAPITEL_HOECHSTENS
        try:
            b.MODELL_STUECKE_HOECHSTENS = 0
            zm = ZaehlModell()
            _e, _w, ber = b.verdichten(ab, "t", zm, 200)
            assert ber.art == "modell ueber auszug", ber.art
            karten = sum(len(b.stuecke_bilden(k[2], 200)) for k in b.kapitel_bilden(ab))
            assert zm.fragen_n < karten, (zm.fragen_n, karten)
            b.MODELL_KAPITEL_HOECHSTENS = 0
            zk = ZaehlModell()
            b.verdichten(ab, "t", zk, 200)
            assert 0 < zm.fragen_n and zk.fragen_n < zm.fragen_n, (zk.fragen_n, zm.fragen_n)
        finally:
            b.MODELL_STUECKE_HOECHSTENS, b.MODELL_KAPITEL_HOECHSTENS = alt_stuecke, alt_kapitel

        e_buch = d / "buch-ablage.epub"
        epub(e_buch)
        # Der Bibliotheksordner: je Buch Wortlaut und Grafiken, im Skillordner nur der Skill.
        li = d / "ablage"
        sk = d / "sk"
        o = b.umwandeln(e_buch, sk, literatur=li)
        assert o == li / "Über Bäume - A. Förster", o
        assert sorted(x.name for x in (sk / "ueber-baeume").iterdir()) == ["SKILL.md"]
        assert (o / "buch.md").exists() and (o / "seiten" / "1.md").exists() and (o / "bilder").exists() and (o / "manifest.json").exists()
        assert not list(o.rglob("*.epub")), "das Original bleibt draussen"
        t = (sk / "ueber-baeume" / "SKILL.md").read_text(encoding="utf-8")
        assert "read_file `ablage/Über Bäume - A. Förster/seiten/1.md`" in t, t
        assert json.loads((o / "manifest.json").read_text(encoding="utf-8"))["skill"] == "ueber-baeume"
        try:
            b.umwandeln(e_buch, d / "sk2", literatur=li)
        except b.Fehler as f:
            assert "gibt es schon" in str(f), f
        else:
            raise AssertionError("ein zweites Buch ueberschrieb das erste")

        # Lange Seiten werden geteilt, und jeder Teil traegt seine Seite.
        st = b.stellen_bilden([("o", f"{b.SEITE_AUF}4{b.SEITE_ZU}\n" + "\n\n".join(["Ein Absatz. " * 150] * 3) + f"\n\n{b.SEITE_AUF}5{b.SEITE_ZU}\nKurz.")], [])
        assert [s.kennung for s in st][:2] == ["4", "4b"] and st[-1].kennung == "5" and all(s.seite in (4, 5) for s in st), st
        assert all(len(s.text) <= b.ABSCHNITT_ZEICHEN for s in st)
        fo = b.Fundorte([b.Stelle("7", 7, "Der Fuchs jagt nachts im Wald."), b.Stelle("8", 8, "Das Boot liegt ruhig am Steg im Hafen.")])
        assert fo.finden("Das Boot liegt ruhig im Hafen.") == "8" and fo.finden("xyz qqq") is None

        # Eine Abbildung am Anfang ist kein Titel.
        ab = b.zerlegen([("o", "![Abbildung auf Seite 1](../bilder/x.png)\n\n> **Abbildung:** Text\n\nEin richtiger Satz.\n")])
        assert ab[0][1] == "Ein richtiger Satz.", ab

        # Was auf vielen Seiten wiederkehrt, ist Rand; Ueberschriften bleiben.
        os.environ["B2S_KOPF"] = "1"
        drei = d / "drei.pdf"
        bild_pdf(drei, [None, None, None])
        o = b.umwandeln(drei, d / "s7", nur_umwandeln=True, scan=b.Scan(myl="x", bilder=False))
        del os.environ["B2S_KOPF"]
        t = (o / "buch.md").read_text(encoding="utf-8")
        assert "Kolumnentitel" not in t and "## 2.1 Abschnitt 2" in t and "| A | B |" in t, t
        assert o.name == "die-echte-titelei", "der Titel kommt von der Seite, nicht vom Dateinamen: " + o.name

        # Ohne pypdf reicht `myl`: jede Seite geht durch die Erkennung.
        alt_zahl, gerettet = b._pdf_seitenzahl, sys.modules.get("pypdf")
        b._pdf_seitenzahl = lambda _m, _p: 2
        sys.modules["pypdf"] = None
        try:
            gerufen.clear()
            o = b.umwandeln(g, d / "s5", nur_umwandeln=True, scan=b.Scan(myl="x", bilder=False))
            t = (o / "buch.md").read_text(encoding="utf-8")
            assert gerufen == [1, 2] and "Erkannter Satz auf Seite 1" in t, (gerufen, t)
            try:
                b.umwandeln(g, d / "s6", nur_umwandeln=True)
            except b.Fehler as f:
                assert "--myl" in str(f), f
            else:
                raise AssertionError("ohne pypdf und ohne myl muss es scheitern")
        finally:
            b._pdf_seitenzahl = alt_zahl
            sys.modules["pypdf"] = gerettet

        # Die Obergrenze gilt, und was festgefahren ist, gilt nicht als Beschreibung.
        w = b.Werk("t", kapitel=[("k", "![a](../bilder/a.png)\n\n![b](../bilder/b.png)\n")], bilder={"a.png": GROSS + b"1", "b.png": GROSS + b"2"})
        zaehl.unlink()
        r = b.bilder_auswerten(w, b.Scan(myl="x", bilder_hoechstens=1), str(myl), d / "s4")
        assert r["uebersprungen"] == 1 and r["beschrieben"] == 1 and zaehl.read_text().split() == ["a.png"], r
        assert not b._brauchbare_beschreibung("Typ " * 60), "festgefahren"
        assert b._brauchbare_beschreibung("Ein Balkendiagramm mit vier Regionen und dem Titel Umsatz.")
    finally:
        b._scan_aufrufen = alt_aufrufen
    assert b._pakete([1, 2, 3, 7, 8], 2) == [[1, 2], [3], [7, 8]]


def scan_haengt(d: pathlib.Path) -> None:
    """⛔️ Eine haengende Seite haelt das Buch nicht fest: `myl scan` legt sie
    als unlesbar ab und endet mit 3, Book2Skill ruft fuer den Rest neu auf,
    vermerkt sie im Text und im Manifest. Wer 3 meldet und nichts ablegt,
    wird nicht ohne Ende neu gerufen."""
    gerufen: list = []

    def haengt_bei_1(_myl, auftrag: pathlib.Path) -> bool:
        a = json.loads(auftrag.read_text(encoding="utf-8"))
        ort = pathlib.Path(a["arbeit"])
        seiten = [n for pk in a["pakete"] for n in pk["seiten"]]
        gerufen.append(seiten)
        for n in seiten:
            if n == 1:
                (ort / f"seite-{n:06d}.json").write_text(json.dumps({"md": "", "bilder": [], "titel": "", "unlesbar": True}), encoding="utf-8")
                return True
            (ort / f"seite-{n:06d}.json").write_text(json.dumps({"md": f"Erkannter Satz auf Seite {n}, lang genug fuer einen Absatz.", "bilder": [], "titel": ""}), encoding="utf-8")
        return False

    alt_aufrufen, alt_zahl = b._scan_aufrufen, b._pdf_seitenzahl
    b._pdf_seitenzahl = lambda _m, _p: 3
    try:
        b._scan_aufrufen = haengt_bei_1
        w = b.pdf_ueber_myl(d / "haengt.pdf", b.Scan(myl="x", arbeit=d / "haengt-arbeit"))
        assert gerufen == [[1, 2, 3], [2, 3]], gerufen
        text = w.kapitel[0][1]
        assert "[Seite nicht lesbar]" in text and "Seite 2" in text and "Seite 3" in text, text
        assert w.scan["seiten_unlesbar"] == [1] and w.scan["seiten_erkannt"] == 2, w.scan

        b._scan_aufrufen = lambda _m, _a: True
        try:
            b.pdf_ueber_myl(d / "haengt.pdf", b.Scan(myl="x", arbeit=d / "haengt-leer"))
        except b.Fehler as f:
            assert "keine ab" in str(f), f
        else:
            raise AssertionError("ohne abgelegte Seite endlos neu gerufen")
    finally:
        b._scan_aufrufen, b._pdf_seitenzahl = alt_aufrufen, alt_zahl


def xml_ohne_entitaeten() -> None:
    """⛔️ Eigene Entitaeten werden abgewiesen, auch hinter einem langen
    Kommentar und in UTF-16; gewoehnliches XML liest sich weiter."""
    def bombe(stufen: int) -> str:
        ebenen = "".join(f'<!ENTITY {chr(98 + i)} "' + ("&" + chr(97 + i) + ";") * 10 + '">' for i in range(stufen))
        return f'<!DOCTYPE l [<!ENTITY a "aaaaaaaaaa">{ebenen}]><l>&{chr(97 + stufen)};</l>'
    for name, roh in [
        ("direkt", ('<?xml version="1.0"?>' + bombe(5)).encode()),
        ("hinter 70 KB Kommentar", ('<?xml version="1.0"?><!--' + "x" * 70000 + "-->" + bombe(5)).encode()),
        ("UTF-16", ('<?xml version="1.0" encoding="UTF-16"?>' + bombe(5)).encode("utf-16")),
    ]:
        try:
            b.xml_lesen(roh)
        except b.Fehler as f:
            assert "Entitäten" in str(f), (name, f)
        else:
            raise AssertionError(f"{name}: angenommen")
    w = b.xml_lesen('<?xml version="1.0" encoding="UTF-16"?><a x="&amp;">&lt;b&gt;</a>'.encode("utf-16"))
    assert w.text == "<b>" and w.get("x") == "&", (w.text, w.attrib)


def rtf_unicode() -> None:
    """⛔️ Ein Emoji in RTF (zwei UTF-16-Haelften) wird ein Zeichen; einzelne
    Haelften und Unsinnszahlen werden U+FFFD, nichts bricht ab."""
    # 😀 ist U+1F600, in UTF-16 D83D DE00, in RTF also -10179 und -8704.
    assert b.rtf_nach_text(r"{\rtf1 Hallo \u-10179?\u-8704? Welt}") == "Hallo \U0001F600 Welt\n"
    t = b.rtf_nach_text(r"{\rtf1 a\u-10179?b \u-99999? \u9999999? \u252?ber}")
    t.encode("utf-8")
    assert t == "a\ufffdb \ufffd \ufffd über\n", repr(t)


def verstuemmelt_endet_sauber(d: pathlib.Path) -> None:
    """⛔️ **Ein verstuemmeltes Werk endet sauber**: 1 200 Abwandlungen von
    EPUB, DOCX, ODT, FB2, RTF und HTML (Inhalt der Archive veraendert, Teile
    entfernt, Zeichen eingefuegt, gekuerzt) werden gelesen oder mit `Fehler`
    abgewiesen, nie mit einer anderen Ausnahme. 📌 Die erste Runde fand
    `\\ß` in RTF (`AttributeError`)."""
    import random
    zufall = random.Random(7)
    einschub = ["<", ">", "&", '"', "]]>", "<!--", "&#0;", "&#xFFFFFFFF;", "\x00", "<a>", "</p>", "ß", "{", "}", "\\", "\\u-1?"]

    def verstuemmeln(text: str) -> str:
        for _ in range(1 + zufall.randrange(3)):
            if not text:
                break
            i = zufall.randrange(len(text))
            w = zufall.randrange(4)
            if w == 0:
                text = text[:i] + zufall.choice(einschub) + text[i:]
            elif w == 1:
                text = text[:i] + text[i + 1 + zufall.randrange(20):]
            elif w == 2:
                text = text[:i]
            else:
                text = text[:i] + chr(zufall.randrange(32, 0x3000)) + text[i + 1:]
        return text

    vorlagen = {}
    for name, bau in [("epub", epub), ("docx", docx), ("odt", odt), ("fb2", fb2)]:
        p = d / f"vorlage.{name}"
        bau(p)
        vorlagen[name] = p
    (d / "vorlage.rtf").write_text("{\\rtf1\\ansi{\\fonttbl{\\f0 Arial;}}\\f0 Kapitel\\par Ein Satz mit \\u252?ber und \\'e4.\\par}", encoding="utf-8")
    (d / "vorlage.html").write_text(EINS, encoding="utf-8")
    vorlagen["rtf"], vorlagen["html"] = d / "vorlage.rtf", d / "vorlage.html"
    gelesen = 0
    for runde in range(1_200):
        art = list(vorlagen)[runde % len(vorlagen)]
        quelle, ziel = vorlagen[art], d / f"verstuemmelt.{art}"
        if zipfile.is_zipfile(quelle):
            with zipfile.ZipFile(quelle) as zq, zipfile.ZipFile(ziel, "w") as zz:
                namen = zq.namelist()
                weg = zufall.choice(namen) if zufall.random() < 0.1 else None
                for n in namen:
                    if n != weg:
                        roh = zq.read(n)
                        if n.endswith((".xml", ".xhtml", ".opf", ".rels")) and zufall.random() < 0.6:
                            roh = verstuemmeln(roh.decode("utf-8", "replace")).encode("utf-8", "surrogatepass")
                        zz.writestr(n, roh)
        else:
            ziel.write_bytes(verstuemmeln(quelle.read_text(encoding="utf-8")).encode("utf-8", "surrogatepass"))
        try:
            w = b.LESER[ziel.suffix](ziel)
            for _, text in w.kapitel:
                text.encode("utf-8")
            gelesen += 1
        except b.Fehler:
            pass
    assert 100 < gelesen < 1_100, f"gelesen {gelesen}: die Probe prueft sonst nur eine Seite"
    assert b.rtf_nach_text("{\\rtf1 a\\ß b}").strip() == "a b"


def main() -> int:
    rtf_unicode()
    xml_ohne_entitaeten()
    with tempfile.TemporaryDirectory() as t:
        d = pathlib.Path(t).resolve()
        verstuemmelt_endet_sauber(d)
        einzeln(d)
        formate(d)
        mit_pypdf(d)
        scan_und_bilder(d)
        scan_haengt(d)
        e = d / "buch.epub"
        epub(e)
        o = b.umwandeln(e, d / "a")
        assert o.name == "ueber-baeume", o.name
        k = sorted((o / "seiten").iterdir())
        namen = [p.name for p in k]
        assert namen == ["1.md", "2.md", "3.md"], namen
        wurzeln = k[1].read_text(encoding="utf-8")
        assert "sha256" in wurzeln and "Lizenz: CC BY 4.0" in wurzeln and "OEBPS/kap/eins.xhtml" in wurzeln, wurzeln
        assert "Erster *Absatz* mit Umbruch." in wurzeln, wurzeln
        assert "![Eine Wurzel](../bilder/ueber-baeume-001.png)" in wurzeln, wurzeln
        alles = "".join(p.read_text(encoding="utf-8") for p in k)
        assert "boese" not in alles and "p{}" not in alles, alles
        assert "code   bleibt" in alles and "| A | B |" in alles, alles
        assert "[Quelle](https://example.org/x)" in alles and "(eins.xhtml)" not in alles, alles
        assert (o / "bilder" / "ueber-baeume-001.png").read_bytes() == PNG
        assert len(list((o / "bilder").iterdir())) == 1, "was fehlt, wird nicht erfunden"
        buch = (o / "buch.md").read_text(encoding="utf-8")
        assert "](bilder/ueber-baeume-001.png)" in buch and "../bilder" not in buch, buch
        assert all(p.read_text(encoding="utf-8").startswith("[Untrusted origin:") for p in k), "jeder Abschnitt ist fremd"
        s = (o / "SKILL.md").read_text(encoding="utf-8")
        assert s.startswith("---\nbeschreibung: Das Wesentliche aus „Über Bäume“ von A. Förster.\nstichworte: "), s
        rumpf = s.split("\n---", 1)[1].removeprefix("\n")
        assert rumpf.startswith("[Untrusted origin:"), rumpf[:50]
        assert s.index("### Stamm") < s.index("### Wurzeln"), "die Lesefolge gilt, nicht das Manifest"
        assert "learn_skill `ueber-baeume/seiten/1.md`" in s and "### Wurzeln [2]" in s, s
        assert re.search(r"aus dem Boden\. \[\d+\]", s), s
        assert "Wurzeln versorgen" in s, "ein Auszug traegt Saetze des Werks"
        j = json.loads((o / "manifest.json").read_text(encoding="utf-8"))
        assert j["abschnitte"] == 3 and j["bilder"] == 1 and j["verfahren"] == "auszug" and j["lizenz"] == "CC BY 4.0", j
        assert j["zeichen_skill"] == len(s) and j["vertraut"] is False, j
        # Der Hash jedes erzeugten Markdowns steht im Manifest (0.4).
        md = {p.relative_to(o).as_posix() for p in o.rglob("*.md")}
        assert set(j["sha256_markdown"]) == md and "buch.md" in md, (sorted(j["sha256_markdown"]), sorted(md))
        for rel, h in j["sha256_markdown"].items():
            assert hashlib.sha256((o / rel).read_bytes()).hexdigest() == h, rel
        assert j["sha256_skill"] == hashlib.sha256((o / "SKILL.md").read_bytes()).hexdigest()

        # Gleiche Eingabe, gleiche Bytes.
        o2 = b.umwandeln(e, d / "b")
        for p in o.rglob("*"):
            if p.is_file():
                assert p.read_bytes() == (o2 / p.relative_to(o)).read_bytes(), p
        v = filecmp.dircmp(o, o2)
        assert not v.left_only and not v.right_only

        # Vertraut: nur der Skill verliert die Marke, nie der Volltext.
        ov = b.umwandeln(e, d / "v", vertraut=True)
        sv = (ov / "SKILL.md").read_text(encoding="utf-8")
        assert not sv.split("\n---", 1)[1].removeprefix("\n").startswith("[Untrusted"), sv[:300]
        assert (ov / "seiten" / "1.md").read_text(encoding="utf-8").startswith("[Untrusted origin:")

        # Nur umwandeln.
        on = b.umwandeln(e, d / "n", nur_umwandeln=True)
        assert not (on / "SKILL.md").exists() and (on / "buch.md").exists()

        # Grenze des Skills: lieber weniger Punkte als abgeschnitten.
        lang = d / "lang.md"
        kap = "".join(f"# Kapitel {i}\n\n" + " ".join(f"Das Wurzelwerk Nummer {i} stärkt den Waldboden mit Nährstoffen Variante {j}." for j in range(40)) + "\n\n" for i in range(30))
        lang.write_text(kap, encoding="utf-8")
        ol = b.umwandeln(lang, d / "l", skill_zeichen=6000)
        sl = (ol / "SKILL.md").read_text(encoding="utf-8")
        assert len(sl) <= 7500, len(sl)
        assert sl.count("### Kapitel") == 30, "kein Kapitel faellt weg"
        assert sl.count("[") >= 30, "jedes Kapitel nennt seinen Ort"

        # Mit Modell: Belegprobe, Abbruch und Fortsetzen.
        myl = d / "myl"
        myl.write_text(FAKE, encoding="utf-8")
        myl.chmod(myl.stat().st_mode | stat.S_IEXEC)
        zaehler = d / "zaehler"
        zaehler.write_text("")
        os.environ["FAKE_ZAEHLER"] = str(zaehler)
        lang2 = d / "m.md"
        lang2.write_text("# Wurzeln\n\n" + "\n\n".join(f"Die Wurzeln versorgen den Baum mit Wasser in Zone {i} aus tiefem Boden." * 3 for i in range(40)) + "\n\n# Stamm\n\n" + "\n\n".join(f"Der Stamm leitet Saefte durch das Holz nach oben in Abschnitt {i} der Krone." * 3 for i in range(40)) + "\n\n# Krone\n\n" + "\n\n".join(f"Die Krone traegt Blaetter und fangt Licht in Lage {i} fuer die Photosynthese." * 3 for i in range(40)), encoding="utf-8")
        os.environ["FAKE_GRENZE"] = "2"
        try:
            b.umwandeln(lang2, d / "x", modell=(str(myl), "artefakt"), stueck=800)
        except b.Fehler as f:
            assert "endete mit 7" in str(f), f
        else:
            raise AssertionError("der Abbruch wurde nicht gemeldet")
        assert not (d / "x" / "m").exists() or not (d / "x" / "m" / "SKILL.md").exists()
        arbeit = d / "x" / ".m.arbeit"
        vorher = len((arbeit / "antworten.jsonl").read_text().splitlines())
        assert vorher == 2, vorher
        del os.environ["FAKE_GRENZE"]
        om = b.umwandeln(lang2, d / "x", modell=(str(myl), "artefakt"), stueck=800)
        sm = (om / "SKILL.md").read_text(encoding="utf-8")
        assert "987654" not in sm, "eine erfundene Zahl darf nicht durch"
        assert "Quantenschleife" not in sm, "eine erfundene Aussage darf nicht durch"
        assert "und Vorgehensweisen" not in sm and "und Notizen" not in sm, "ein Echo des Auftrags darf nicht durch"
        assert "geheim" not in sm and "Hier sind" not in sm, sm
        assert "Wurzeln versorgen den Baum" in sm, sm[:600]
        jm = json.loads((om / "manifest.json").read_text())
        assert jm["verfahren"] == "modell" and jm["modell"] == "artefakt" and jm["verworfen_nicht_belegt"] >= jm["fragen"], jm
        assert jm["punkte"] > jm["verworfen_nicht_belegt"], jm
        gefragt = zaehler.read_text().split()
        assert len(gefragt) == len(set(gefragt)), "nach dem Abbruch wurde neu gerechnet, was schon beantwortet war"
        assert not arbeit.exists(), "der Arbeitsordner ist nach Erfolg weg"
        assert b.ist_in_sprache("der Mann und die Frau sind nicht das", "de") and not b.ist_in_sprache("the man and the woman of it", "de")
        assert b.ist_in_sprache("the man and the woman", "en") and not b.ist_in_sprache("И я искал объяснения", "en")
        assert b.uebersetzung_taugt("a b c d", "Ein Satz der passt.", "de") and not b.uebersetzung_taugt("Lang genug " * 40, "", "de")
        assert not b.uebersetzung_taugt("Текст " * 30, " ".join(f"слово{i}" for i in range(30)), "de"), "das Original ist keine Uebersetzung"
        assert not b.uebersetzung_taugt("Wort " * 300, "Nur ein kurzer Satz hier.", "de"), "abgeschnitten"
        assert not b.uebersetzung_taugt("Wort " * 300, "wir dachten nicht daran; " * 40, "de"), "eine Schleife ohne Satzzeichen"
        seiten = [("Teil", f"{b.SEITE_AUF}7{b.SEITE_ZU}Текст " * 1), ]
        st = [b.Stelle("7", 7, "Текст", "Seite 7"), b.Stelle("7b", 7, "Ещё", "Seite 7")]
        _, _, st_neu = b.fassung_uebersetzt(seiten, [], st, {"7": "Der Wald." * 500, "7b": "Das Wasser."})
        assert [s.kennung for s in st_neu] == ["7", "7b"] and st_neu[1].text == "Das Wasser.", "gleiche Nummern wie das Original"
        assert not b.uebersetzung_taugt("Kurz.", "Der Wald ist nicht das Wasser. " * 3, "de"), "aufgeblaeht"
        assert not b.uebersetzung_taugt("x" * 900, "Das ist immer derselbe lange Satz hier. " * 30, "de"), "eine Schleife ist keine Uebersetzung"
        assert not b.uebersetzung_taugt("x " * 400, " ".join(f"the w{i} and" for i in range(60)), "de"), "falsche Sprache"
        russisch = d / "r.md"
        russisch.write_text("# Глава\n\n" + "\n\n".join(f"Корни дерева дают воду в зоне {i} глубокой земли и питают лес." * 3 for i in range(9)), encoding="utf-8")
        zaehler.write_text("")
        ou = b.umwandeln(russisch, d / "u", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de")
        ju = json.loads((ou / "manifest.json").read_text())
        assert ju["uebersetzung"]["noetig"] and ju["uebersetzung"]["stellen"] > 0 and ju["uebersetzung"]["zahl_misslungen"] == 0, ju["uebersetzung"]
        assert sorted((ou / "uebersetzung/de/seiten").glob("*.md")) and (ou / "uebersetzung/de/buch.md").is_file()
        su = (ou / "SKILL.md").read_text(encoding="utf-8")
        assert "uebersetzung/de/seiten/" in su and "Original" in su and "## Das Wichtigste" in su, su[:400]
        assert "seiten/" in su and (ou / "seiten").is_dir(), "das Original bleibt daneben"
        assert "Wald" in su and "Корни" not in su and "Глава" not in su, "der Skill entsteht aus der Uebersetzung: " + su[:500]
        assert b.bilder_tiefer("![a](../bilder/x.png)", 3) == "![a](../../../bilder/x.png)"

        class Stapel:
            def fragen(self, texte, token, grundsaetze=True):
                assert token == b.UEBERSETZUNG_TOKEN and all("![" not in t for t in texte), texte
                return {t: "Der Wald ist nicht das Wasser." for t in texte}

        bild = b.Stelle("1", 1, "Текст страницы.\n\n![Bild](../bilder/a.png)", "Seite 1")
        fertig, schlecht, _ = b.uebersetzen([bild], "T", None, Stapel(), "de")
        assert not schlecht and fertig["1"].endswith("![Bild](../bilder/a.png)"), fertig
        # Eine Sprache, die der Nutzer schon liest, kostet keine Rechnung.
        zaehler.write_text("")
        og = b.umwandeln(lang2.rename(d / "gleich.md"), d / "g", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de")
        assert json.loads((og / "manifest.json").read_text())["uebersetzung"]["noetig"] is False and not (og / "uebersetzung").exists()
        assert not [z for z in (d / "g" / ".gleich.arbeit").glob("*")] if (d / "g" / ".gleich.arbeit").exists() else True
        # Was misslingt, bleibt im Original und ist gekennzeichnet.
        os.environ["FAKE_UEBERSETZUNG"] = "leer"
        try:
            russisch2 = d / "r2.md"
            russisch2.write_text(russisch.read_text(encoding="utf-8"), encoding="utf-8")
            ol = b.umwandeln(russisch2, d / "l", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de")
        finally:
            del os.environ["FAKE_UEBERSETZUNG"]
        jl = json.loads((ol / "manifest.json").read_text())["uebersetzung"]
        assert jl["stellen"] == 0 and jl["zahl_misslungen"] > 0, jl
        erste = sorted((ol / "uebersetzung/de/seiten").glob("*.md"))[0].read_text(encoding="utf-8")
        assert "ließ sich nicht übersetzen" in erste and "Корни" in erste, erste[:300]
        # Der Uebersetzer macht die Arbeit, das grosse Modell nur, was ihm misslingt.
        schnell_log = pathlib.Path(str(zaehler) + ".schnell")
        schnell_log.write_text("")
        russisch4 = d / "r4.md"
        russisch4.write_text(russisch.read_text(encoding="utf-8"), encoding="utf-8")
        o4 = b.umwandeln(russisch4, d / "z4", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de", uebersetzer=(str(myl), "parrot"))
        j4 = json.loads((o4 / "manifest.json").read_text())["uebersetzung"]
        assert j4["modell"] == "parrot" and j4["wege"] == {"schnell": j4["stellen"], "gross": 0} and j4["quelle"] == "kyrillisch", j4
        assert schnell_log.read_text().split(), "der Uebersetzer wurde nicht gefragt"
        os.environ["FAKE_SCHNELL"] = "scheitert"
        try:
            russisch5 = d / "r5.md"
            russisch5.write_text(russisch.read_text(encoding="utf-8"), encoding="utf-8")
            o5 = b.umwandeln(russisch5, d / "z5", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de", uebersetzer=(str(myl), "parrot"))
        finally:
            del os.environ["FAKE_SCHNELL"]
        j5 = json.loads((o5 / "manifest.json").read_text())["uebersetzung"]
        assert j5["wege"]["gross"] > 0 and j5["wege"]["schnell"] == 0 and j5["zahl_misslungen"] == 0, j5
        # Ohne das grosse Modell uebersetzt der Uebersetzer trotzdem.
        russisch6 = d / "r6.md"
        russisch6.write_text(russisch.read_text(encoding="utf-8"), encoding="utf-8")
        o6 = b.umwandeln(russisch6, d / "z6", sprache_ziel="de", uebersetzer=(str(myl), "parrot"))
        assert json.loads((o6 / "manifest.json").read_text())["uebersetzung"]["stellen"] > 0
        # Die Weiche nach Sprache.
        for text, soll in [("Корни дерева " * 9, ("kyrillisch", "schnell")), ("ܡܠܟܐ ܘܥܒܕܐ " * 9, ("syrisch", "keiner")), ("தமிழ் மொழி " * 9, ("tamil", "gross")),
                           ("ཚོང་པས་གྲུ་ " * 9, ("tibetisch", "gross")), ("พ่อค้าซื้อเรือ " * 9, ("thai", "gross")), ("商人买了一艘新船 " * 9, ("ostasien", "schnell")),
                           ("Ἐν ἀρχῇ ἦν ὁ λόγος καὶ " * 9, ("altgriechisch", "gross")), ("Ο έμπορος αγόρασε πλοίο " * 9, ("griechisch", "schnell")),
                           ("सभी मनुष्यों को गौरव और अधिकारों के मामले में है " * 9, ("hindi", "schnell")), ("धर्मक्षेत्रे कुरुक्षेत्रे समवेता युयुत्सवः " * 9, ("devanagari", "gross")),
                           ("বণিক নতুন জাহাজ কিনলেন " * 9, ("bengalisch", "schnell")), ("বেপাৰীজনে নতুন জাহাজ কিনিলে ধুমুহাই " * 9, ("assamesisch", "gross")),
                           ("Gallia est omnis divisa in partes et non ut sed cum " * 9, ("latein", "gross")), ("Il mercante e la nave in porto non ha " * 9, ("latein", "schnell")),
                           ("Le roi et la reine est dans le jardin " * 9, ("latein", "schnell"))]:
            assert b.uebersetzungsweg(text) == soll, (text[:20], b.uebersetzungsweg(text), soll)
        # Syrisch wird nicht erfunden.
        syrisch = d / "syr.md"
        syrisch.write_text("# ܡܠܟܐ\n\n" + "\n\n".join("ܡܠܟܐ ܘܥܒܕܐ ܐܙܠܘ ܠܒܝܬܐ ܘܐܬܘ ܡܢ ܡܕܝܢܬܐ " * 3 for _ in range(9)), encoding="utf-8")
        og2 = b.umwandeln(syrisch, d / "syr", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de", uebersetzer=(str(myl), "parrot"))
        js = json.loads((og2 / "manifest.json").read_text())["uebersetzung"]
        assert js["fehlt"] == "Sprache nicht übersetzbar" and not (og2 / "uebersetzung").exists(), js
        assert b.bereinigen_antwort("<assistant>\n\nDer Wald.\n</response>") == "Der Wald."
        assert not b.uebersetzung_taugt("x " * 40, "Okay, let's tackle this translation. The user wants it.", "en")
        assert b.uebersetzung_taugt("x " * 40, "A short line that is simply a translated sentence here.", "en")
        # Eine schwache Sprache geht am Uebersetzer vorbei.
        schnell_log.write_text("")
        tamil = d / "ta.md"
        tamil.write_text("# தமிழ்\n\n" + "\n\n".join("தமிழ் மொழி மிகவும் பழமையான மொழி ஆகும் " * 3 for _ in range(9)), encoding="utf-8")
        o7 = b.umwandeln(tamil, d / "z7", modell=(str(myl), "artefakt"), stueck=800, sprache_ziel="de", uebersetzer=(str(myl), "parrot"))
        j7 = json.loads((o7 / "manifest.json").read_text())["uebersetzung"]
        assert j7["weg"] == "gross" and j7["wege"]["schnell"] == 0 and j7["modell"] == "artefakt" and not schnell_log.read_text().split(), j7
        # Ohne Modell: Das Werk bleibt, der Mangel steht im Manifest.
        russisch3 = d / "r3.md"
        russisch3.write_text(russisch.read_text(encoding="utf-8"), encoding="utf-8")
        ok = b.umwandeln(russisch3, d / "k", sprache_ziel="de")
        assert json.loads((ok / "manifest.json").read_text())["uebersetzung"]["fehlt"] == "kein Modell" and not (ok / "uebersetzung").exists()
        assert not list((d / "x").glob(".*")), list((d / "x").iterdir())

        try:
            b.umwandeln(e, d / "a")
        except b.Fehler as f:
            assert "gibt es schon" in str(f)
        else:
            raise AssertionError("ueberschrieben")
    print("[book2skill] PASSED")
    return 0


if __name__ == "__main__":
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
    sys.exit(main())
