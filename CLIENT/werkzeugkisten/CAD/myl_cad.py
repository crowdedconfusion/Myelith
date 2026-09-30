"""Kurze Bausteine fuer parametrische Modelle in FreeCAD.

Ein Skript fuer das Werkzeug `cad_bauen` darf die FreeCAD-Schnittstelle
direkt benutzen. Diese Bausteine nehmen ihm das ab, was dabei am
haeufigsten schiefgeht: die Parametertabelle, die Bindung eines Masses an
einen Parameter und den Koerper, in dem FreeCAD die Schritte als
bearbeitbaren Baum fuehrt.

Jedes Mass und jede Lage ist entweder eine Zahl (Millimeter) oder ein
Ausdruck als Text. Im Ausdruck stehen die Namen der Parameter, etwa
"breite / 2 - rand". Ein Ausdruck bleibt in der Datei erhalten: Wer in
FreeCAD den Parameter aendert, aendert das Modell.

Das Ergebnis ist ein PartDesign-Koerper: In FreeCAD steht jeder Schritt
als eigener Eintrag im Baum und laesst sich dort weiter bearbeiten.
"""

import re

import FreeCAD as App

# Die Namen der Parameter je Dokument, fuer das Einsetzen in Ausdruecke.
_parameter = {}

TABELLE = "Parameter"


def dokument(name="Modell"):
    """Legt ein neues Dokument an und gibt es zurueck."""
    return App.newDocument(name)


def parameter(doc, **werte):
    """Legt die Parametertabelle an: `parameter(doc, breite=40, winkel="30 deg")`.

    Eine Zahl gilt als Millimeter; ein Text traegt seine Einheit selbst.
    Eine reine Anzahl steht als Text ohne Einheit: `anzahl="4"`.
    """
    tabelle = doc.getObject(TABELLE) or doc.addObject("Spreadsheet::Sheet", TABELLE)
    namen = _parameter.setdefault(doc.Name, [])
    for name, wert in werte.items():
        if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", name):
            raise ValueError(f"Parametername {name!r}: nur Buchstaben, Ziffern und Unterstrich, vorn ein Buchstabe")
        zeile = len(namen) + 1
        tabelle.set(f"A{zeile}", name)
        tabelle.set(f"B{zeile}", wert if isinstance(wert, str) else f"{wert} mm")
        tabelle.setAlias(f"B{zeile}", name)
        namen.append(name)
    doc.recompute()
    return tabelle


def koerper(doc, name="Koerper"):
    """Legt einen Koerper an; alle Schritte eines Teils gehoeren in einen."""
    return doc.addObject("PartDesign::Body", name)


def _ausdruck(doc, text):
    """Setzt vor jeden Parameternamen die Tabelle: `breite / 2` wird
    `Parameter.breite / 2`. Was schon einen Punkt davor hat, bleibt."""
    namen = _parameter.get(doc.Name, [])
    if not namen:
        return text
    muster = r"(?<![\w.])(" + "|".join(sorted(map(re.escape, namen), key=len, reverse=True)) + r")(?![\w(])"
    return re.sub(muster, TABELLE + r".\1", text)


def _setze(obj, eigenschaft, wert):
    """Ein Mass als Zahl oder als Ausdruck."""
    if isinstance(wert, str):
        obj.setExpression(eigenschaft, _ausdruck(obj.Document, wert))
    else:
        teile = eigenschaft.split(".")
        if len(teile) == 1:
            setattr(obj, eigenschaft, wert)
        else:
            # Nur die Lage kommt verschachtelt vor: Placement.Base.x
            lage = obj.Placement
            basis = lage.Base
            setattr(basis, teile[-1], wert)
            lage.Base = basis
            obj.Placement = lage


_DREHUNG = {
    "z": App.Rotation(),
    "x": App.Rotation(App.Vector(0, 1, 0), 90),
    "y": App.Rotation(App.Vector(1, 0, 0), -90),
}


def _lage(obj, bei, achse="z"):
    if achse not in _DREHUNG:
        raise ValueError(f"achse={achse!r}: erlaubt sind 'x', 'y' und 'z'")
    obj.Placement = App.Placement(App.Vector(0, 0, 0), _DREHUNG[achse])
    if len(bei) != 3:
        raise ValueError(f"bei={bei!r}: drei Angaben (x, y, z)")
    for name, wert in zip("xyz", bei):
        _setze(obj, f"Placement.Base.{name}", wert)


def _schritt(k, art, name, masse, bei, achse="z"):
    obj = k.newObject(art, name)
    for eigenschaft, wert in masse.items():
        _setze(obj, eigenschaft, wert)
    _lage(obj, bei, achse)
    k.Document.recompute()
    return obj


def quader(k, name, laenge, breite, hoehe, bei=(0, 0, 0)):
    """Fuegt einen Quader hinzu; `bei` ist seine Ecke unten links vorn."""
    return _schritt(k, "PartDesign::AdditiveBox", name, {"Length": laenge, "Width": breite, "Height": hoehe}, bei)


def zylinder(k, name, radius, hoehe, bei=(0, 0, 0), achse="z"):
    """Fuegt einen Zylinder hinzu; `bei` ist die Mitte seiner Grundflaeche."""
    return _schritt(k, "PartDesign::AdditiveCylinder", name, {"Radius": radius, "Height": hoehe}, bei, achse)


def ausschnitt(k, name, laenge, breite, hoehe, bei=(0, 0, 0)):
    """Schneidet einen Quader heraus; `bei` ist seine Ecke unten links vorn."""
    return _schritt(k, "PartDesign::SubtractiveBox", name, {"Length": laenge, "Width": breite, "Height": hoehe}, bei)


def bohrung(k, name, radius, tiefe, bei=(0, 0, 0), achse="z"):
    """Schneidet einen Zylinder heraus; `bei` ist die Mitte seiner Grundflaeche."""
    return _schritt(k, "PartDesign::SubtractiveCylinder", name, {"Radius": radius, "Height": tiefe}, bei, achse)


def _kanten(form, welche):
    """Die Namen der Kanten einer Form, nach ihrer Lage gewaehlt."""
    box = form.BoundBox
    eps = 1e-6
    aus = []
    for nr, kante in enumerate(form.Edges, start=1):
        punkte = [v.Point for v in kante.Vertexes]
        if welche == "alle":
            passt = True
        elif welche == "senkrecht":
            passt = len(punkte) == 2 and abs(punkte[0].x - punkte[1].x) < eps and abs(punkte[0].y - punkte[1].y) < eps
        elif welche == "oben":
            passt = bool(punkte) and all(abs(p.z - box.ZMax) < eps for p in punkte)
        elif welche == "unten":
            passt = bool(punkte) and all(abs(p.z - box.ZMin) < eps for p in punkte)
        else:
            raise ValueError(f"kanten={welche!r}: erlaubt sind 'senkrecht', 'oben', 'unten' und 'alle'")
        if passt:
            aus.append(f"Edge{nr}")
    if not aus:
        raise ValueError(f"kanten={welche!r}: der Koerper hat keine solche Kante")
    return aus


def _kantenschritt(k, art, name, mass, wert, kanten):
    # Der Stand VOR dem neuen Schritt: Danach ist der neue Schritt die
    # Spitze des Koerpers und hat noch keine Form.
    vorher = k.Tip
    if vorher is None:
        raise ValueError("Verrunden und Fase brauchen einen Koerper, der schon eine Form hat")
    gewaehlt = _kanten(vorher.Shape, kanten)
    obj = k.newObject(art, name)
    obj.Base = (vorher, gewaehlt)
    _setze(obj, mass, wert)
    k.Document.recompute()
    return obj


def verrunden(k, name, radius, kanten="senkrecht"):
    """Rundet Kanten ab: 'senkrecht', 'oben', 'unten' oder 'alle'.

    Die Kanten werden beim Bauen nach ihrer Lage gewaehlt. Aendert ein
    Parameter spaeter die Zahl der Kanten, waehlt man sie in FreeCAD neu.
    """
    return _kantenschritt(k, "PartDesign::Fillet", name, "Radius", radius, kanten)


def fase(k, name, groesse, kanten="oben"):
    """Bricht Kanten mit einer Fase: 'senkrecht', 'oben', 'unten' oder 'alle'."""
    return _kantenschritt(k, "PartDesign::Chamfer", name, "Size", groesse, kanten)


def _achse(k, achse):
    rolle = {"x": "X_Axis", "y": "Y_Axis", "z": "Z_Axis"}.get(achse)
    if rolle is None:
        raise ValueError(f"achse={achse!r}: erlaubt sind 'x', 'y' und 'z'")
    return next(o for o in k.Origin.OriginFeatures if o.Role == rolle)


def reihe(k, name, schritt, anzahl, laenge, richtung="x"):
    """Wiederholt einen Schritt in einer Reihe: `anzahl` Stueck, das erste
    und das letzte `laenge` auseinander, entlang 'x', 'y' oder 'z'."""
    obj = k.newObject("PartDesign::LinearPattern", name)
    obj.Originals = [schritt]
    obj.Direction = (_achse(k, richtung), [""])
    _setze(obj, "Length", laenge)
    _setze(obj, "Occurrences", anzahl)
    k.Tip = obj
    k.Document.recompute()
    return obj


def kreis(k, name, schritt, anzahl, achse="z", winkel=360):
    """Wiederholt einen Schritt im Kreis um eine Achse des Ursprungs:
    `anzahl` Stueck ueber `winkel` Grad. Der Schritt selbst liegt dazu
    neben der Achse, etwa eine Bohrung bei (teilkreis / 2, 0, 0)."""
    obj = k.newObject("PartDesign::PolarPattern", name)
    obj.Originals = [schritt]
    obj.Axis = (_achse(k, achse), [""])
    _setze(obj, "Angle", winkel)
    _setze(obj, "Occurrences", anzahl)
    k.Tip = obj
    k.Document.recompute()
    return obj
