#!/usr/bin/env python3
"""Proben fuer book2skill.py: eigenstaendig, ohne Abhaengigkeiten; PDF nur mit pypdf."""

from __future__ import annotations

import base64
import filecmp
import json
import os
import pathlib
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
            m = re.search(r"<material>\\n(.*?)\\n</material>", v["text"], re.S).group(1)
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
    r = sorted((o / "referenz").iterdir())
    assert "Seite 1" in r[0].read_text(encoding="utf-8"), r[0].read_text(encoding="utf-8")[:200]


def main() -> int:
    with tempfile.TemporaryDirectory() as t:
        d = pathlib.Path(t).resolve()
        einzeln(d)
        formate(d)
        mit_pypdf(d)
        e = d / "buch.epub"
        epub(e)
        o = b.umwandeln(e, d / "a")
        assert o.name == "uber-baume", o.name
        k = sorted((o / "referenz").iterdir())
        namen = [p.name for p in k]
        assert namen == ["001-stamm.md", "002-wurzeln.md", "003-feine-wurzeln.md"], namen
        wurzeln = k[1].read_text(encoding="utf-8")
        assert "sha256" in wurzeln and "Lizenz: CC BY 4.0" in wurzeln and "OEBPS/kap/eins.xhtml" in wurzeln, wurzeln
        assert "Erster *Absatz* mit Umbruch." in wurzeln, wurzeln
        assert "![Eine Wurzel](../bilder/uber-baume-001.png)" in wurzeln, wurzeln
        alles = "".join(p.read_text(encoding="utf-8") for p in k)
        assert "boese" not in alles and "p{}" not in alles, alles
        assert "code   bleibt" in alles and "| A | B |" in alles, alles
        assert "[Quelle](https://example.org/x)" in alles and "(eins.xhtml)" not in alles, alles
        assert (o / "bilder" / "uber-baume-001.png").read_bytes() == PNG
        assert len(list((o / "bilder").iterdir())) == 1, "was fehlt, wird nicht erfunden"
        buch = (o / "buch.md").read_text(encoding="utf-8")
        assert "](bilder/uber-baume-001.png)" in buch and "../bilder" not in buch, buch
        assert all(p.read_text(encoding="utf-8").startswith("[Untrusted origin:") for p in k), "jeder Abschnitt ist fremd"
        s = (o / "SKILL.md").read_text(encoding="utf-8")
        assert s.startswith("---\nbeschreibung: Das Wesentliche aus „Über Bäume“ von A. Förster.\nstichworte: "), s
        rumpf = s.split("\n---", 1)[1].removeprefix("\n")
        assert rumpf.startswith("[Untrusted origin:"), rumpf[:50]
        assert s.index("### Stamm") < s.index("### Wurzeln"), "die Lesefolge gilt, nicht das Manifest"
        assert "uber-baume/referenz/001-stamm.md" in s, s
        assert "Wurzeln versorgen" in s, "ein Auszug traegt Saetze des Werks"
        j = json.loads((o / "manifest.json").read_text(encoding="utf-8"))
        assert j["abschnitte"] == 3 and j["bilder"] == 1 and j["verfahren"] == "auszug" and j["lizenz"] == "CC BY 4.0", j
        assert j["zeichen_skill"] == len(s) and j["vertraut"] is False, j

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
        assert (ov / "referenz" / "001-stamm.md").read_text(encoding="utf-8").startswith("[Untrusted origin:")

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
        assert sl.count("referenz/") == 30, "jeder Volltext bleibt erreichbar"

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
        import shutil

        shutil.rmtree(d / "x" / "m", ignore_errors=True)
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
