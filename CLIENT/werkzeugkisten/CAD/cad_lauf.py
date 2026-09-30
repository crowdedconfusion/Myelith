"""Der Laeufer der CAD-Werkzeuge: laeuft IN FreeCAD, ohne Fenster.

`cad.sh` startet ihn mit `freecadcmd` und reicht die Aufgabe ueber die
Umgebung herein:

    MYL_CAD_AUFGABE  bauen | pruefen | parameter
    MYL_CAD_A        das Skript (bauen) oder die Datei (pruefen, parameter)
    MYL_CAD_B        der Ausgabename (bauen) oder die neuen Werte (parameter)
    MYL_CAD_BERICHT  wohin der Bericht geschrieben wird
    MYL_CAD_KISTE    der Ordner dieser Kiste, fuer `import myl_cad`

Der Bericht ist alles, was das Modell sieht. Er endet mit genau einer
Zeile `ERGEBNIS: gelungen` oder `ERGEBNIS: gescheitert`; danach richtet
sich der Rueckgabewert des Werkzeugs.

Geschrieben wird nur, was gelungen ist: Ein Modell, das sich nicht
rechnen laesst, ersetzt keine Datei, die vorher in Ordnung war.
"""

import os
import re
import sys
import traceback

import FreeCAD as App

# Ohne Bytecode: Python legte sonst `__pycache__` in die Kiste und in den
# Arbeitsordner. FreeCAD liest die Umgebungsvariable dafuer nicht.
sys.dont_write_bytecode = True

zeilen = []


def sag(text=""):
    zeilen.append(text)


def zahl(x):
    """Eine Zahl, wie ein Mensch sie liest: hoechstens drei Nachkommastellen."""
    text = f"{x:.3f}".rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


def tabellen(doc):
    return [o for o in doc.Objects if o.TypeId == "Spreadsheet::Sheet"]


def parameter_von(tabelle):
    """(Name, Zelle, Inhalt) je benannter Zelle, in der Reihenfolge der Tabelle."""
    aus = []
    for zelle in tabelle.getUsedCells():
        name = tabelle.getAlias(zelle)
        if name:
            aus.append((name, zelle, tabelle.getContents(zelle).lstrip("=")))
    return aus


def oberste(doc):
    """Die Bauteile, die fuer sich stehen: Koerper und andere Formen, die in
    keinem anderen Objekt stecken."""
    aus = []
    for o in doc.Objects:
        # Ursprung, Achsen, Ebenen und Punkte sind Bezuege und keine Bauteile.
        if o.TypeId.startswith(("Spreadsheet::", "App::", "Sketcher::")):
            continue
        if not hasattr(o, "Shape") or o.Shape.isNull():
            continue
        if any(hasattr(e, "Shape") or e.TypeId == "App::Part" for e in o.InList):
            continue
        aus.append(o)
    return aus


def fehler_von(doc):
    """Was sich nicht rechnen liess, je Objekt ein Satz."""
    aus = []
    for o in doc.Objects:
        if "Invalid" in o.State or "Error" in o.State:
            grund = o.getStatusString() if hasattr(o, "getStatusString") else "ungueltig"
            aus.append(f"{o.Name}: {' '.join(grund.split())}")
    return aus


def berichten(doc):
    """Schreibt Parameter und Bauteile in den Bericht; gibt die Fehler zurueck."""
    fehler = fehler_von(doc)
    for t in tabellen(doc):
        eintraege = parameter_von(t)
        if eintraege:
            sag(f"PARAMETER (Tabelle {t.Name}):")
            for name, _, inhalt in eintraege:
                sag(f"  {name} = {inhalt}")
    if not any(parameter_von(t) for t in tabellen(doc)):
        sag("PARAMETER: keine. Ohne Parametertabelle laesst sich das Modell nur von Hand aendern.")
    teile = oberste(doc)
    sag("BAUTEILE:")
    if not teile:
        sag("  keine")
        fehler.append("das Dokument enthaelt kein Bauteil mit einer Form")
    for o in teile:
        form = o.Shape
        b = form.BoundBox
        gueltig = form.isValid()
        sag(
            f"  {o.Name} ({o.TypeId}): {zahl(b.XLength)} x {zahl(b.YLength)} x {zahl(b.ZLength)} mm, "
            f"von ({zahl(b.XMin)}, {zahl(b.YMin)}, {zahl(b.ZMin)}) bis ({zahl(b.XMax)}, {zahl(b.YMax)}, {zahl(b.ZMax)}), "
            f"Volumen {zahl(form.Volume)} mm3, {'gueltig' if gueltig else 'UNGUELTIG'}"
        )
        if not gueltig:
            fehler.append(f"{o.Name}: die Form ist ungueltig")
        elif form.Volume <= 0:
            fehler.append(f"{o.Name}: kein Volumen (nur Flaechen oder leer)")
        for schritt in getattr(o, "Group", []):
            if schritt.TypeId.startswith("App::Origin"):
                continue
            sag(f"    {schritt.Name} ({schritt.TypeId})")
    return fehler


def kamera(teile):
    """Eine isometrische Ansicht auf die Bauteile, von vorn rechts oben, als
    Kameratext, wie FreeCAD ihn in GuiDocument.xml schreibt."""
    box = None
    for o in teile:
        b = o.Shape.BoundBox
        if box is None:
            box = App.BoundBox(b)
        else:
            box.add(b)
    if box is None or not box.isValid():
        mitte, groesse = App.Vector(0, 0, 0), 100.0
    else:
        mitte, groesse = box.Center, max(box.DiagonalLength, 1.0)
    # Die Kamera schaut entlang ihrer eigenen -Z-Achse. Ihre +Z-Achse zeigt
    # also vom Teil weg zu ihr hin, oben ist die Z-Achse des Teils.
    rueck = App.Vector(1, -1, 1).normalize()
    rechts = App.Vector(0, 0, 1).cross(rueck).normalize()
    hoch = rueck.cross(rechts).normalize()
    m = App.Matrix(rechts.x, hoch.x, rueck.x, 0, rechts.y, hoch.y, rueck.y, 0, rechts.z, hoch.z, rueck.z, 0, 0, 0, 0, 1)
    dreh = App.Rotation(m)
    achse, winkel = dreh.Axis, dreh.Angle
    ort = mitte + rueck * (2 * groesse)
    z = "&#10;"
    return (
        f"OrthographicCamera {{{z}  viewportMapping ADJUST_CAMERA{z}"
        f"  position {ort.x:.6f} {ort.y:.6f} {ort.z:.6f}{z}"
        f"  orientation {achse.x:.8f} {achse.y:.8f} {achse.z:.8f}  {winkel:.8f}{z}"
        f"  aspectRatio 1{z}  focalDistance {2 * groesse:.6f}{z}  height {1.2 * groesse:.6f}{z}{z}}}{z}"
    )


def ansicht_beilegen(doc, fcstd):
    """Legt der Datei eine GuiDocument.xml bei: welches Objekt sichtbar ist,
    und eine Kamera, die das Teil zeigt.

    Ohne sie ist in FreeCAD nichts zu sehen. FreeCAD ohne Fenster schreibt
    nur Document.xml; die Sichtbarkeit steht aber auf der Seite der
    Darstellung. Beim Oeffnen im Fenster fehlt sie, und FreeCAD blendet jedes
    Objekt aus (gemeldet vom Projektinhaber am 2026-09-30, mit dem Fenster
    ohne Bildschirm nachgestellt). Sichtbar werden die Bauteile und bei einem
    Koerper sein letzter Schritt, wie FreeCAD es selbst haelt; alles andere
    bleibt ausgeblendet. Ohne Kamera und Aufklappliste liest FreeCAD die
    Datei nicht (nachgestellt: dann bleibt wieder alles ausgeblendet).
    """
    import zipfile

    teile = oberste(doc)
    sichtbar = {o.Name for o in teile}
    for o in teile:
        spitze = getattr(o, "Tip", None)
        if spitze is not None:
            sichtbar.add(spitze.Name)
    eintraege = "".join(
        f'        <ViewProvider name="{o.Name}" expanded="0" treeRank="-1">\n'
        '            <Properties Count="1" TransientCount="0">\n'
        '                <Property name="Visibility" type="App::PropertyBool" status="1">\n'
        f'                    <Bool value="{"true" if o.Name in sichtbar else "false"}"/>\n'
        "                </Property>\n"
        "            </Properties>\n"
        "        </ViewProvider>\n"
        for o in doc.Objects
    )
    xml = (
        "<?xml version='1.0' encoding='utf-8'?>\n"
        '<Document SchemaVersion="1" HasExpansion="1">\n'
        "    <Expand />\n"
        f'    <ViewProviderData Count="{len(doc.Objects)}">\n'
        f"{eintraege}"
        "    </ViewProviderData>\n"
        f'    <Camera settings="{kamera(teile)}"/>\n'
        "</Document>\n"
    )
    neu = fcstd + ".neu"
    with zipfile.ZipFile(fcstd) as alt, zipfile.ZipFile(neu, "w", zipfile.ZIP_DEFLATED) as ziel:
        for eintrag in alt.infolist():
            if eintrag.filename != "GuiDocument.xml":
                ziel.writestr(eintrag, alt.read(eintrag.filename))
        ziel.writestr("GuiDocument.xml", xml)
    os.replace(neu, fcstd)
    return sorted(sichtbar)


def schreiben(doc, stamm):
    """Speichert das Dokument und die STEP-Datei daneben."""
    import Part

    fcstd = os.path.abspath(stamm + ".FCStd")
    step = os.path.abspath(stamm + ".step")
    # Erst unter einem eigenen Namen, dann an die Stelle: FreeCAD legte beim
    # Ueberschreiben sonst eine Sicherung `<name>.<zeit>.FCBak` daneben, bei
    # jedem Bauen und jeder Parameteraenderung eine mehr.
    vorlaeufig = os.path.join(os.path.dirname(fcstd), "." + os.path.basename(stamm) + ".myl-neu.FCStd")
    doc.saveAs(vorlaeufig)
    sichtbar = ansicht_beilegen(doc, vorlaeufig)
    os.replace(vorlaeufig, fcstd)
    sag(f"SICHTBAR in FreeCAD: {', '.join(sichtbar)}")
    Part.export(oberste(doc), step)
    sag(f"DATEIEN: {os.path.basename(fcstd)} (parametrisch, fuer FreeCAD), {os.path.basename(step)} (fuer andere Programme)")


def bauen(skript, name):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name or ""):
        sag(f"FEHLER: name={name!r}: nur Buchstaben, Ziffern, Punkt, Strich und Unterstrich, ohne Ordner")
        return False
    if not os.path.isfile(skript):
        sag(f"FEHLER: das Skript {skript} gibt es nicht")
        return False
    with open(skript, encoding="utf-8") as f:
        quelle = f.read()
    try:
        exec(compile(quelle, skript, "exec"), {"__name__": "__main__", "__file__": skript})
    except BaseException as e:  # auch SystemExit: Ein Skript, das aussteigt, hat nichts gebaut
        ort = [r for r in traceback.extract_tb(sys.exc_info()[2]) if r.filename == skript]
        wo = f" in Zeile {ort[-1].lineno}: {ort[-1].line}" if ort else ""
        sag(f"FEHLER im Skript{wo}")
        sag(f"  {type(e).__name__}: {' '.join(str(e).split())}")
        return False
    doc = App.ActiveDocument
    if doc is None:
        sag("FEHLER: das Skript hat kein Dokument angelegt (dokument(...) oder App.newDocument(...))")
        return False
    doc.recompute()
    fehler = berichten(doc)
    if fehler:
        sag("FEHLER:")
        for f in fehler:
            sag(f"  {f}")
        sag("Nichts geschrieben.")
        return False
    schreiben(doc, name)
    return True


def oeffnen(datei):
    if not os.path.isfile(datei):
        sag(f"FEHLER: die Datei {datei} gibt es nicht")
        return None
    try:
        return App.openDocument(os.path.abspath(datei))
    except Exception as e:
        sag(f"FEHLER: {datei} laesst sich nicht oeffnen: {' '.join(str(e).split())}")
        return None


def pruefen(datei):
    doc = oeffnen(datei)
    if doc is None:
        return False
    doc.recompute()
    fehler = berichten(doc)
    if fehler:
        sag("FEHLER:")
        for f in fehler:
            sag(f"  {f}")
    return not fehler


def parameter(datei, werte):
    doc = oeffnen(datei)
    if doc is None:
        return False
    bekannt = {name: (t, zelle, inhalt) for t in tabellen(doc) for name, zelle, inhalt in parameter_von(t)}
    wuensche = []
    for teil in werte.split(";"):
        if not teil.strip():
            continue
        if "=" not in teil:
            sag(f"FEHLER: {teil.strip()!r} ist keine Zuweisung; erwartet wird name=wert, mit Strichpunkt getrennt")
            return False
        name, wert = (x.strip() for x in teil.split("=", 1))
        if name not in bekannt:
            sag(f"FEHLER: den Parameter {name!r} gibt es nicht; vorhanden: {', '.join(bekannt) or 'keiner'}")
            return False
        if not wert:
            sag(f"FEHLER: {name} hat keinen Wert")
            return False
        wuensche.append((name, wert))
    if not wuensche:
        sag("FEHLER: keine Werte angegeben; erwartet wird name=wert, mit Strichpunkt getrennt")
        return False
    sag("GEAENDERT:")
    for name, wert in wuensche:
        tabelle, zelle, alt = bekannt[name]
        # Eine nackte Zahl behaelt die Einheit, die vorher dastand.
        einheit = re.fullmatch(r"\s*-?[0-9.]+\s*([A-Za-z°]+)\s*", alt)
        if re.fullmatch(r"-?[0-9.]+", wert) and einheit:
            wert = f"{wert} {einheit.group(1)}"
        tabelle.set(zelle, wert)
        sag(f"  {name}: {alt} -> {wert}")
    doc.recompute()
    fehler = berichten(doc)
    if fehler:
        sag("FEHLER:")
        for f in fehler:
            sag(f"  {f}")
        sag("Nichts geschrieben; die Datei ist unveraendert.")
        return False
    schreiben(doc, os.path.splitext(datei)[0])
    return True


def lauf():
    aufgabe = os.environ.get("MYL_CAD_AUFGABE", "")
    a, b = os.environ.get("MYL_CAD_A", ""), os.environ.get("MYL_CAD_B", "")
    sys.path.insert(0, os.environ["MYL_CAD_KISTE"])
    if aufgabe == "bauen":
        return bauen(a, b)
    if aufgabe == "pruefen":
        return pruefen(a)
    if aufgabe == "parameter":
        return parameter(a, b)
    sag(f"FEHLER: unbekannte Aufgabe {aufgabe!r}; es gibt bauen, pruefen und parameter")
    return False


try:
    gelungen = lauf()
except BaseException as e:
    sag(f"FEHLER im Laeufer: {type(e).__name__}: {' '.join(str(e).split())}")
    sag(traceback.format_exc(limit=3))
    gelungen = False
sag("ERGEBNIS: gelungen" if gelungen else "ERGEBNIS: gescheitert")
with open(os.environ["MYL_CAD_BERICHT"], "w", encoding="utf-8") as f:
    f.write("\n".join(zeilen) + "\n")
