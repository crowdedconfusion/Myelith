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
# Was gebaut ist, aber vermutlich nicht wie gemeint. Steht vor dem Ergebnis.
warnungen = []


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
            grund = " ".join(grund.split())
            # 📌 2026-10-01: `"dicke - 2"` rechnet FreeCAD aus, erklaert den
            #    Schritt aber fuer ungueltig. Die Meldung allein sagt nicht,
            #    was zu tun ist.
            if "Unit mismatch" in grund or "unit mismatch" in grund.lower():
                grund += (" (eine Zahl, die zu einem Mass addiert wird, braucht ihre Einheit: "
                          "\"winkel - 10 deg\", nicht \"winkel - 10\"; bei Laengen haengt die Bibliothek mm "
                          "selbst an; Teiler und Faktoren bleiben ohne)")
            aus.append(f"{o.Name}: {grund}")
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
        huelle = {"form": None}
        for schritt in getattr(o, "Group", []):
            if schritt.TypeId.startswith("App::Origin"):
                continue
            # Die Teilmuster eines Rasters gehoeren zu ihm und stehen nicht eigens da.
            if any(x.TypeId == "PartDesign::MultiTransform" and schritt in x.Transformations for x in schritt.InList):
                continue
            sag(f"    {schritt.Name} ({schritt.TypeId}){schrittbilanz(schritt, huelle)}")
    return fehler


# Ab welchem Anteil ein Schnitt als vollstaendig gilt (Rundung der Formen).
VOLL = 0.995


def werkzeugform(schritt):
    """Die Form, die ein Schritt hinzufuegt oder wegnimmt, an ihrem Ort im
    Koerper; `None`, wenn der Schritt keine eigene hat (Muster, Fase)."""
    w = getattr(schritt, "AddSubShape", None)
    if w is None or w.isNull():
        return None
    w = w.copy()
    w.Placement = schritt.Placement.multiply(w.Placement)
    return w


def material_entlang(schritt, huelle):
    """Wo das Teil an der Stelle eines Schnitts ist, entlang seiner Achse:
    (Achse, von, bis) oder (Achse, None, None) ohne Material, oder None.

    ⚑ Eine Warnung, die nur „liegt daneben“ sagt, laesst das Modell raten,
    wohin. Am runden Flansch (2026-10-01) lagen Loch und Senkung auf der
    Hoehe des Bunds statt des Tellers; die Angabe, dass das Teil dort von
    z 0 bis 12 reicht, sagt, was zu tun ist.
    """
    import Part

    if huelle is None or not hasattr(schritt, "Placement"):
        return None
    try:
        p = schritt.Placement.Base
        d = schritt.Placement.Rotation.multVec(App.Vector(0, 0, 1))
        weit = 10.0 * max(huelle.BoundBox.DiagonalLength, 1.0)
        linie = Part.LineSegment(p - d * weit, p + d * weit).toShape()
        stuecke = linie.common(huelle).Edges
    except Exception:
        return None
    achse = max("xyz", key=lambda a: abs(getattr(d, a)))
    if not stuecke:
        return (achse, None, None)
    werte = sorted(getattr(v.Point, achse) for kante in stuecke for v in kante.Vertexes)
    return (achse, werte[0], werte[-1])


def satz_zum_material(lage):
    if lage is None:
        return ""
    achse, von, bis = lage
    if von is None:
        return f"; an dieser Stelle ist entlang {achse} kein Material"
    return f"; an dieser Stelle reicht das Teil entlang {achse} von {zahl(von)} bis {zahl(bis)}"


def nur_ueberlaenge(form, huelle, lage, im_teil):
    """Ob ein Schnitt nur in Richtung seiner Achse ueber das Teil hinausragt.

    ⚑ Eine Durchgangsbohrung wird ueblich laenger gezeichnet, als das Teil
    dick ist, damit sie sicher durchgeht. Das ist kein Fehler: Auf den
    Bereich beschnitten, in dem entlang der Achse Material ist, liegt der
    Schnitt dann ganz im Teil.
    """
    import Part

    if lage is None or lage[1] is None or im_teil <= 0:
        return False
    achse, von, bis = lage
    b = huelle.BoundBox
    rand = 10.0 * max(b.DiagonalLength, 1.0)
    lo = {"x": b.XMin - rand, "y": b.YMin - rand, "z": b.ZMin - rand}
    groesse = {"x": 2 * rand + b.XLength, "y": 2 * rand + b.YLength, "z": 2 * rand + b.ZLength}
    lo[achse], groesse[achse] = von, bis - von
    try:
        scheibe = Part.makeBox(groesse["x"], groesse["y"], groesse["z"], App.Vector(lo["x"], lo["y"], lo["z"]))
        beschnitten = form.common(scheibe).Volume
    except Exception:
        return False
    return beschnitten > 0 and im_teil >= VOLL * beschnitten


def geht_ganz_durch(form, lage):
    """Ob ein ueberstehender Schnitt das Material entlang seiner Achse ganz
    durchquert; sonst (von, bis) des Schnitts entlang der Achse.

    📌 **Anlass (2026-10-01, Lagerbock):** Das 35B setzte eine Querbohrung
    bei der Mitte des Blocks an, als sei `bei` die Mitte des Zylinders. Sie
    lief von dort nach aussen und ging nur durch den halben Block. Sie
    ragte nur entlang ihrer Achse hinaus und galt deshalb als gewollte
    Ueberlaenge.
    """
    achse, von, bis = lage
    b = form.BoundBox
    t0, t1 = {"x": (b.XMin, b.XMax), "y": (b.YMin, b.YMax), "z": (b.ZMin, b.ZMax)}[achse]
    eps = 1e-6
    if t0 <= von + eps and t1 >= bis - eps:
        return True
    return (t0, t1)


def schrittbilanz(schritt, huelle):
    """Was ein Schritt am Volumen tut, gemessen und nicht geschaetzt.

    Anlass (2026-10-01): Das 35B setzte eine Platte von (0, 0) bis
    (60, 60) und die Bohrungen um (0, 0), als laege die Mitte dort. Drei
    von vier Loechern schnitten nichts, die Zentrierbohrung ein Viertel.
    Das Modell rechnete das Volumen im Kopf nach, verschaetzte sich und
    meldete „plausibel“. Ein Werkzeug, das misst, verschaetzt sich nicht.

    ⚑ **Gewarnt wird, wenn ein Schnitt ausserhalb des Teils liegt**, also
    ausserhalb der Huelle aus allem, was bis dahin hinzugefuegt wurde.
    Ein Schnitt, der in schon entferntes Material faellt, ist oft gewollt:
    die Senkung ueber einer Durchgangsbohrung (gemessen am Rundflansch,
    dort meldete die erste Fassung einen Fehlalarm).
    """
    try:
        basis = getattr(schritt, "BaseFeature", None)
        vorher = basis.Shape.Volume if basis is not None and not basis.Shape.isNull() else 0.0
        nachher = schritt.Shape.Volume
        form = werkzeugform(schritt)
        eigen = form.Volume if form is not None else 0.0
    except Exception:
        return ""
    bei = ""
    if hasattr(schritt, "Placement"):
        p = schritt.Placement.Base
        bei = f" bei ({zahl(p.x)}, {zahl(p.y)}, {zahl(p.z)})"
    art = schritt.TypeId
    if "Subtractive" in art or art.endswith(("::Pocket", "::Hole", "::Groove")):
        entfernt = vorher - nachher
        text = f":{bei} entfernt {zahl(entfernt)} von {zahl(eigen)} mm3"
        if form is None or eigen <= 0:
            return text
        try:
            im_teil = form.common(huelle["form"]).Volume if huelle["form"] is not None else 0.0
        except Exception:
            return text
        if im_teil < VOLL * eigen:
            lage = material_entlang(schritt, huelle["form"])
            if nur_ueberlaenge(form, huelle["form"], lage, im_teil):
                durch = geht_ganz_durch(form, lage)
                if durch is True:
                    text += " (ragt nur entlang seiner Achse ueber das Teil hinaus, wie bei einer durchgehenden Bohrung)"
                    if entfernt < VOLL * im_teil:
                        text += "; der Rest faellt in schon entferntes Material"
                    return text
                achse, von, bis = lage
                warnungen.append(
                    f"{schritt.Name}{bei} ragt auf einer Seite aus dem Teil und endet auf der anderen im "
                    f"Material: Der Schnitt reicht entlang {achse} von {zahl(durch[0])} bis {zahl(durch[1])}, "
                    f"das Teil dort von {zahl(von)} bis {zahl(bis)}. Soll er durchgehen: `bei` ist der Anfang "
                    f"des Zylinders, nicht seine Mitte; er laeuft von dort in +{achse}"
                )
                return text
            anteil = round(100 * max(im_teil, 0.0) / eigen)
            warnungen.append(
                f"{schritt.Name}{bei} liegt nur zu {anteil} % im Teil"
                + (" (ganz daneben, entfernt nichts)" if anteil == 0 else "")
                + satz_zum_material(lage)
            )
        elif entfernt < VOLL * eigen:
            text += " (der Rest faellt in schon entferntes Material)"
        return text
    if "Additive" in art or art.endswith(("::Pad", "::Revolution")):
        if form is not None:
            try:
                huelle["form"] = form if huelle["form"] is None else huelle["form"].fuse(form)
            except Exception:
                pass
        dazu = nachher - vorher
        text = f":{bei} fuegt {zahl(dazu)} von {zahl(eigen)} mm3 hinzu"
        if vorher > 0 and eigen > 0 and dazu < VOLL * eigen:
            text += " (der Rest ueberlappt vorhandenes Material)"
        return text
    if vorher > 0:
        return f": aendert das Volumen um {zahl(nachher - vorher)} mm3"
    return ""


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
    # 📌 2026-10-01 (Montagewinkel, Saat 2): `doc.speichern()`, dann
    #    `doc.save()` am Ende des Skripts, zwei Fehlbauten. Gespeichert wird
    #    hier, nicht im Skript; eine Zeile, die nur speichert, wird
    #    uebergangen (die Zeilennummern bleiben, damit Meldungen stimmen).
    zeilen_alt = quelle.split("\n")
    for i, zeile in enumerate(zeilen_alt):
        if re.fullmatch(r"\s*[\w.]*\b(save|saveAs|speichern)\s*\(.*\)\s*(#.*)?", zeile):
            einzug = zeile[: len(zeile) - len(zeile.lstrip())]
            zeilen_alt[i] = f"{einzug}pass"
            sag(f"HINWEIS: Zeile {i + 1} ({zeile.strip()}) uebergangen: cad_bauen speichert selbst.")
    quelle = "\n".join(zeilen_alt)
    try:
        exec(compile(quelle, skript, "exec"), {"__name__": "__main__", "__file__": skript})
    except BaseException as e:  # auch SystemExit: Ein Skript, das aussteigt, hat nichts gebaut
        ort = [r for r in traceback.extract_tb(sys.exc_info()[2]) if r.filename == skript]
        wo = f" in Zeile {ort[-1].lineno}: {ort[-1].line}" if ort else ""
        sag(f"FEHLER im Skript{wo}")
        sag(f"  {type(e).__name__}: {' '.join(str(e).split())}")
        if isinstance(e, NameError):
            # Ein Name, den es nicht gibt: Welche Bausteine gibt es?
            try:
                import myl_cad

                sag(f"  Die Bausteine heissen: {', '.join(myl_cad.BAUSTEINE)}; das Skript beginnt mit `from myl_cad import *`.")
            except Exception:
                pass
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
# ⚑ Ein gescheiterter Bau hat keine verlaesslichen Formen; Warnungen waeren
#   Folgen des Fehlers und lenkten von ihm ab (2026-10-01, NEMA-17 mit Saat 3).
if warnungen and gelungen:
    sag("WARNUNGEN:")
    for w in warnungen:
        sag(f"  {w}")
    sag(
        "Ein Schnitt, der nicht ganz im Teil liegt, schneidet ins Leere. Ist das nicht gewollt "
        "(ein Loch am Rand kann es sein), die Lage pruefen: `bei` "
        "eines Quaders ist seine Ecke unten links vorn, die Mitte einer Platte ist (laenge / 2, "
        "breite / 2). Gebaut ist es, aber vermutlich nicht wie gemeint: erst berichtigen, dann "
        "„fertig“ melden."
    )
sag("ERGEBNIS: gelungen" if gelungen else "ERGEBNIS: gescheitert")
with open(os.environ["MYL_CAD_BERICHT"], "w", encoding="utf-8") as f:
    f.write("\n".join(zeilen) + "\n")
