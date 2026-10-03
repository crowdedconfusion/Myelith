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


# Eigenschaften, die immer eine Laenge sind. Winkel und Anzahlen nicht.
_LAENGEN = {"Length", "Width", "Height", "Radius", "Size", "Placement.Base.x", "Placement.Base.y", "Placement.Base.z"}

_TEIL = re.compile(r"\s*(?:(\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)|([A-Za-z_][\w.]*)|(.))")


def _mit_einheit(text):
    """Haengt an jeden Summanden ohne Einheit ` mm`: `"hoehe - 2"` wird
    `"hoehe - 2 mm"`. Faktoren und Teiler bleiben: `"laenge / 2 + 5"` wird
    `"laenge / 2 + 5 mm"`.

    📌 **Anlass (2026-10-01, Lagerbock):** `"platte_hoehe + 35"` und
    `"platte_laenge - 10"` kosteten drei Bauten, je eine ganze Antwort des
    35B: FreeCAD meldet die fehlende Einheit, aber je Bau nur den ersten
    Ausdruck. In einer Laenge ist die Einheit eines Summanden eindeutig;
    die Bibliothek nimmt Zahlen ohnehin als Millimeter.
    """
    teile = []
    for m in _TEIL.finditer(text):
        if m.group(1) is not None:
            teile.append(("zahl", m.group(1)))
        elif m.group(2) is not None:
            teile.append(("name", m.group(2)))
        elif m.group(3) is not None:
            teile.append(("zeichen", m.group(3)))
    aus = []
    for i, (art, wert) in enumerate(teile):
        aus.append(wert)
        if art != "zahl":
            continue
        davor = teile[i - 1][1] if i > 0 else None
        danach = teile[i + 1] if i + 1 < len(teile) else None
        if danach is not None and danach[0] == "name":
            continue  # traegt schon eine Einheit: `5 mm`, `30 deg`
        if davor in ("*", "/") or (danach is not None and danach[1] in ("*", "/", "^")):
            continue  # Faktor oder Teiler
        aus[-1] = f"{wert} mm"
    ergebnis = ""
    for i, stueck in enumerate(aus):
        if i > 0 and not (stueck in (")", ",") or ergebnis.endswith("(")):
            ergebnis += " "
        ergebnis += stueck
    return ergebnis


# Was in einem Ausdruck stehen darf, ohne ein Parameter zu sein.
_EINHEITEN_UND_FUNKTIONEN = {
    "mm", "cm", "m", "um", "in", "ft", "deg", "rad", "pi", "e",
    "sin", "cos", "tan", "asin", "acos", "atan", "sqrt", "abs", "min", "max", "round", "floor", "ceil",
}


def _unbekannte_namen(doc, text):
    namen = set(_parameter.get(doc.Name, []))
    return [
        n for n in dict.fromkeys(re.findall(r"(?<![\w.])([A-Za-z_]\w*)(?![\w(.])", text))
        if n not in namen and n not in _EINHEITEN_UND_FUNKTIONEN
    ]


def _setze(obj, eigenschaft, wert):
    """Ein Mass als Zahl oder als Ausdruck."""
    if isinstance(wert, str):
        if eigenschaft in _LAENGEN:
            wert = _mit_einheit(wert)
        # 📌 2026-10-01 (NEMA-17, Saat 2): `h = "lochabstand / 2"` und dann
        #    `"breite / 2 - h"`. Ein Python-Name in einem Ausdruck-Text wirkt
        #    nicht, und FreeCAD sagte nur „Failed to parse expression“.
        fremd = _unbekannte_namen(obj.Document, wert)
        if fremd:
            raise ValueError(
                f"Ausdruck {wert!r}: {', '.join(fremd)} ist kein Parameter "
                f"(Parameter: {', '.join(_parameter.get(obj.Document.Name, [])) or 'keine'}). "
                "Ein Python-Name in einem Ausdruck-Text wirkt nicht: den Ausdruck ausschreiben "
                "oder als f-String einsetzen, oder den Wert mit parameter(...) anlegen."
            )
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
    # ⚑ Auch rueckwaerts (2026-10-01, Gehaeuse mit Saat 3: ein Kernloch von
    #   oben nach unten, `achse="-z"`). Der Zylinder laeuft dann von `bei`
    #   in die Gegenrichtung.
    "-z": App.Rotation(App.Vector(1, 0, 0), 180),
    "-x": App.Rotation(App.Vector(0, 1, 0), -90),
    "-y": App.Rotation(App.Vector(1, 0, 0), 90),
}


def _lage(obj, bei, achse="z"):
    if achse not in _DREHUNG:
        raise ValueError(f"achse={achse!r}: erlaubt sind 'x', 'y', 'z' und rueckwaerts '-x', '-y', '-z'")
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


def _kanten(form, welche, hoehe=None, aussen=False, durch=None):
    """Die Namen der Kanten einer Form, nach ihrer Lage gewaehlt.

    `hoehe` (eine Zahl in mm) ersetzt fuer 'oben' und 'unten' die oberste
    oder unterste Ebene: So erreicht man die Oberkante eines Tellers, auf dem
    noch ein Bund steht. `aussen` laesst nur Kanten am aeusseren Rand von
    oben gesehen stehen, also nicht die von Bohrungen, Senkungen oder einem
    Bund.
    """
    box = form.BoundBox
    eps = 1e-6
    aus = []
    if durch is not None:
        # ⚑ Eine Kante, benannt durch einen Punkt auf ihr: die Innenkante
        #   eines Winkels, die weder oben noch aussen liegt.
        import Part

        punkt = Part.Vertex(App.Vector(*durch))
        for nr, kante in enumerate(form.Edges, start=1):
            if kante.distToShape(punkt)[0] < 1e-4:
                aus.append(f"Edge{nr}")
        if not aus:
            raise ValueError(f"durch={tuple(round(x, 3) for x in durch)}: keine Kante geht durch diesen Punkt")
        return aus
    for nr, kante in enumerate(form.Edges, start=1):
        punkte = [v.Point for v in kante.Vertexes]
        if welche == "alle":
            passt = True
        elif welche == "senkrecht":
            passt = len(punkte) == 2 and abs(punkte[0].x - punkte[1].x) < eps and abs(punkte[0].y - punkte[1].y) < eps
        elif welche in ("oben", "unten"):
            ebene = hoehe if hoehe is not None else (box.ZMax if welche == "oben" else box.ZMin)
            passt = bool(punkte) and all(abs(p.z - ebene) < eps for p in punkte)
        else:
            raise ValueError(f"kanten={welche!r}: erlaubt sind 'senkrecht', 'oben', 'unten' und 'alle'")
        if passt and aussen:
            # Am Rand heisst: ein Punkt der Kante liegt auf der aeusseren
            # Grenze von oben gesehen (bei einem Kreis seine Nahtstelle).
            passt = any(
                abs(p.x - box.XMin) < eps or abs(p.x - box.XMax) < eps or abs(p.y - box.YMin) < eps or abs(p.y - box.YMax) < eps
                for p in punkte
            )
        if passt:
            aus.append(f"Edge{nr}")
    if not aus:
        wo = f" auf der Hoehe {hoehe:g} mm" if hoehe is not None else ""
        raise ValueError(f"kanten={welche!r}{wo}{' am Rand' if aussen else ''}: der Koerper hat keine solche Kante")
    return aus


def _wert(k, wert):
    """Eine Zahl oder ein Ausdruck mit Parametern, in Millimetern."""
    if wert is None or not isinstance(wert, str):
        return wert
    ergebnis = k.evalExpression(_ausdruck(k.Document, wert))
    return getattr(ergebnis, "Value", ergebnis)


def _kantenschritt(k, art, name, mass, wert, kanten, hoehe=None, aussen=False, durch=None):
    # Der Stand VOR dem neuen Schritt: Danach ist der neue Schritt die
    # Spitze des Koerpers und hat noch keine Form.
    vorher = k.Tip
    if vorher is None:
        raise ValueError("Verrunden und Fase brauchen einen Koerper, der schon eine Form hat")
    # 📌 Nach einem Schritt, der sich nicht rechnen liess, ist die Form leer,
    #    und die Kantenwahl meldete „keine solche Kante“, sogar fuer 'alle'.
    if vorher.Shape.isNull() or "Invalid" in vorher.State or "Error" in vorher.State:
        raise ValueError(
            f"der Schritt davor ({vorher.Name}) laesst sich nicht rechnen; erst ihn berichtigen, "
            "dann verrunden oder fasen"
        )
    punkt = None if durch is None else tuple(float(_wert(k, w)) for w in durch)
    gewaehlt = _kanten(vorher.Shape, kanten, _wert(k, hoehe), aussen, punkt)
    obj = k.newObject(art, name)
    obj.Base = (vorher, gewaehlt)
    _setze(obj, mass, wert)
    k.Document.recompute()
    return obj


def verrunden(k, name, radius, kanten="senkrecht", hoehe=None, aussen=False, durch=None):
    """Rundet Kanten ab: 'senkrecht', 'oben', 'unten' oder 'alle'; dazu
    `hoehe=` (eine andere Ebene als ganz oben oder unten), `aussen=True`
    (nur der aeussere Rand) und `durch=(x, y, z)`: genau die Kanten, die
    durch diesen Punkt gehen (dann gilt `kanten` nicht). Siehe `_kanten`.

    Die Kanten werden beim Bauen nach ihrer Lage gewaehlt. Aendert ein
    Parameter spaeter die Zahl der Kanten, waehlt man sie in FreeCAD neu.
    """
    return _kantenschritt(k, "PartDesign::Fillet", name, "Radius", radius, kanten, hoehe, aussen, durch)


def fase(k, name, groesse, kanten="oben", hoehe=None, aussen=False, durch=None):
    """Bricht Kanten mit einer Fase; Kantenwahl wie bei `verrunden`."""
    return _kantenschritt(k, "PartDesign::Chamfer", name, "Size", groesse, kanten, hoehe, aussen, durch)


def _achse(k, achse):
    rolle = {"x": "X_Axis", "y": "Y_Axis", "z": "Z_Axis"}.get(achse)
    if rolle is None:
        raise ValueError(f"achse={achse!r}: erlaubt sind 'x', 'y' und 'z'")
    return next(o for o in k.Origin.OriginFeatures if o.Role == rolle)


def _originale(schritt):
    """Ein Schritt oder eine Liste davon: Eine Bohrung mit ihrer Senkung
    sind zwei Schritte und werden zusammen wiederholt.

    📌 Ein Muster laesst sich in FreeCAD nicht noch einmal wiederholen, und
    die Meldung dort („Only additive and subtractive features can be
    transformed“) sagt nicht, was zu tun ist (2026-10-01, NEMA-17 mit Saat 3:
    sieben Fehlbauten, dann das Schrittlimit).
    """
    aus = list(schritt) if isinstance(schritt, (list, tuple)) else [schritt]
    for s in aus:
        if s.TypeId in ("PartDesign::LinearPattern", "PartDesign::PolarPattern", "PartDesign::MultiTransform"):
            raise ValueError(
                f"{s.Name} ist schon ein Muster und laesst sich nicht noch einmal wiederholen. Fuer ein "
                "Raster (etwa 2 x 2 Loecher) gibt es raster(k, name, schritt, anzahl_x, laenge_x, "
                "anzahl_y, laenge_y); sonst einzelne bohrung-Aufrufe."
            )
    return aus


def reihe(k, name, schritt, anzahl, laenge, richtung="x"):
    """Wiederholt einen Schritt (oder eine Liste von Schritten) in einer
    Reihe: `anzahl` Stueck, das erste und das letzte `laenge` auseinander,
    entlang 'x', 'y' oder 'z'."""
    obj = k.newObject("PartDesign::LinearPattern", name)
    obj.Originals = _originale(schritt)
    obj.Direction = (_achse(k, richtung), [""])
    _setze(obj, "Length", laenge)
    _setze(obj, "Occurrences", anzahl)
    k.Tip = obj
    k.Document.recompute()
    return obj


def raster(k, name, schritt, anzahl_x, laenge_x, anzahl_y, laenge_y):
    """Wiederholt einen Schritt (oder eine Liste) in einem Raster: `anzahl_x`
    Stueck ueber `laenge_x` in x und `anzahl_y` ueber `laenge_y` in y. Vier
    Loecher im Quadrat mit Abstand `a` sind `raster(k, "Raster", loch, "2",
    "a", "2", "a")` mit dem ersten Loch unten links."""
    obj = k.newObject("PartDesign::MultiTransform", name)
    obj.Originals = _originale(schritt)
    teile = []
    for richtung, anzahl, laenge in (("x", anzahl_x, laenge_x), ("y", anzahl_y, laenge_y)):
        teil = k.newObject("PartDesign::LinearPattern", f"{name}_{richtung}")
        teil.Direction = (_achse(k, richtung), [""])
        _setze(teil, "Length", laenge)
        _setze(teil, "Occurrences", anzahl)
        teile.append(teil)
    obj.Transformations = teile
    k.Tip = obj
    k.Document.recompute()
    return obj


def kreis(k, name, schritt, anzahl, achse="z", winkel=360):
    """Wiederholt einen Schritt (oder eine Liste von Schritten) im Kreis um
    eine Achse des Ursprungs: `anzahl` Stueck ueber `winkel` Grad. Der
    Schritt selbst liegt dazu neben der Achse, etwa eine Bohrung bei
    (teilkreis / 2, 0, 0)."""
    obj = k.newObject("PartDesign::PolarPattern", name)
    obj.Originals = _originale(schritt)
    obj.Axis = (_achse(k, achse), [""])
    _setze(obj, "Angle", winkel)
    _setze(obj, "Occurrences", anzahl)
    k.Tip = obj
    k.Document.recompute()
    return obj


# ⚑ **Dieselben Bausteine unter den Namen, die ein Modell auch schreibt**
#   (2026-10-01, Montagewinkel: `Dokument(...)` und `Koerper(...)`, gross
#   wie deutsche Hauptwoerter, kosteten zwei Bauten). Gleichwertige Namen,
#   kein zweiter Weg: Jeder zeigt auf dieselbe Funktion.
BAUSTEINE = ["dokument", "parameter", "koerper", "quader", "zylinder", "ausschnitt", "bohrung", "verrunden", "fase", "reihe", "kreis", "raster"]
for _name in BAUSTEINE:
    globals()[_name.capitalize()] = globals()[_name]
körper = Körper = koerper

