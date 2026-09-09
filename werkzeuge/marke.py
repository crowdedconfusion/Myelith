#!/usr/bin/env python3
"""Erzeugt die Marke: die goldene Spirale in ihrer Konstruktion.

# ⚑ Gerechnet und nicht gezeichnet

Quadrate nach Fibonacci, Viertelkreise darin, dazu das Rechteck, das
sie zusammen aufspannen. Weisse Linien auf durchsichtigem Grund; die
Farbe kommt aus `currentColor`, die Marke nimmt also die ihrer
Umgebung an.

# ⛑ Welche Ecke der Mittelpunkt ist, wird hergeleitet

Ein frueher Entwurf suchte die Mittelpunkte so, dass die Bogenenden
aufeinanderfallen, und das genuegt nicht: Enden koennen zusammenfallen,
waehrend die Kruemmung kippt, und heraus kam ein vierzackiger Stern mit
Spitzen statt einer Spirale. Was fehlt, ist die
**Tangentenstetigkeit**: Am Uebergang muessen beide Mittelpunkte auf
derselben Geraden durch den Punkt liegen.

Daraus folgt die Regel eindeutig: Der neue Mittelpunkt ist die Ecke des
neuen Quadrats, die vom Uebergangspunkt aus **in dieselbe Richtung**
liegt wie der alte Mittelpunkt. Keine Suche, kein Raten.

# ⛑ Was hier stand und wieder weg ist

Zwischenzeitlich trug die Marke einen prozeduralen Sternennebel, zwei
Arme und einen kalligraphischen Federzug. Der Projektinhaber hat sich
fuer die schlichte Konstruktion entschieden, und die ist auch die
richtige fuer eine Marke: Sie bleibt bei zweiunddreissig Pixeln
lesbar, wo ein Nebel zu Griess wird.

Aufruf:  python3 werkzeuge/marke.py [--eigenstaendig] > marke.svg
"""
import math
import sys

N = 8            # Quadrate. Acht gibt dieselbe Tiefe wie die Vorlage.
STRICH = 1.6     # Strichbreite in Leinwandeinheiten
# ⚑ **Eine halbe Drehung, damit das Auge unten rechts sitzt.** So
# steht es in der Vorlage, die der Projektinhaber gegeben hat. Sie wird
# NACH der Abbildung angewandt und nicht davor: Eine Drehung im Modell
# vertauscht Breite und Hoehe, und dann passt der Zuschnitt nicht mehr.
# Bei einer halben Drehung bleibt der Kasten derselbe, es kippt nur der
# Inhalt darin.
HALBE_DREHUNG = True


def quadrate(n):
    """Die Fibonacci-Quadrate, spiralig angeordnet."""
    f = [1, 1]
    while len(f) < n:
        f.append(f[-1] + f[-2])
    qs = [(0.0, 0.0, 1.0)]
    ux0, uy0, ux1, uy1 = 0.0, 0.0, 1.0, 1.0
    for i in range(1, n):
        s = float(f[i])
        d = ["links", "unten", "rechts", "oben"][(i - 1) % 4]
        q = {
            "links": (ux0 - s, uy0, s),
            "unten": (ux0, uy0 - s, s),
            "rechts": (ux1, uy0, s),
            "oben": (ux0, uy1, s),
        }[d]
        qs.append(q)
        ux0 = min(ux0, q[0])
        uy0 = min(uy0, q[1])
        ux1 = max(ux1, q[0] + s)
        uy1 = max(uy1, q[1] + s)
    return qs, (ux0, uy0, ux1, uy1)


def ecken(q):
    x, y, s = q
    return [(x, y), (x + s, y), (x + s, y + s), (x, y + s)]


def nah(a, b):
    return abs(a[0] - b[0]) < 1e-9 and abs(a[1] - b[1]) < 1e-9


def spirale(qs):
    """Boegen mit Tangentenstetigkeit, hergeleitet statt gesucht."""
    mitte, p, ende = (0.0, 0.0), (1.0, 0.0), (0.0, 1.0)
    kette = [(qs[0], mitte, p, ende)]
    p, vorige_mitte = ende, mitte
    for q in qs[1:]:
        rx, ry = vorige_mitte[0] - p[0], vorige_mitte[1] - p[1]
        laenge = math.hypot(rx, ry)
        ex, ey = rx / laenge, ry / laenge
        soll = (p[0] + ex * q[2], p[1] + ey * q[2])
        treffer = [c for c in ecken(q) if nah(c, soll)]
        if not treffer:
            raise SystemExit(f"keine tangentenstetige Ecke in {q}")
        m = treffer[0]
        nachbarn = [
            c
            for c in ecken(q)
            if not nah(c, m) and abs(math.hypot(c[0] - m[0], c[1] - m[1]) - q[2]) < 1e-9
        ]
        ziel = [c for c in nachbarn if not nah(c, p)][0]
        kette.append((q, m, p, ziel))
        p, vorige_mitte = ziel, m
    return kette


qs, (ux0, uy0, ux1, uy1) = quadrate(N)
kette = spirale(qs)

# ⚑ Die Leinwand folgt dem Seitenverhaeltnis der Figur und ist nicht
# quadratisch: Ein goldenes Rechteck in einen Quadratkasten zu zwingen
# liesse links und rechts Luft, und die Marke saehe verrutscht aus.
breite_e, hoehe_e = ux1 - ux0, uy1 - uy0
H = 100.0
B = H * breite_e / hoehe_e
rand = STRICH / 2 + 0.4
sk = (H - 2 * rand) / hoehe_e


def P(p):
    """Modell nach Leinwand, y umgedreht (SVG zeigt nach unten)."""
    x, y = p
    lx = rand + (x - ux0) * sk
    ly = H - (rand + (y - uy0) * sk)
    if HALBE_DREHUNG:
        lx, ly = B - lx, H - ly
    return (lx, ly)


# Das Geruest: jedes Quadrat, dazu das umschliessende Rechteck.
geruest = []
for q in qs:
    ps = [P(e) for e in ecken(q)]
    x0 = min(p[0] for p in ps)
    x1 = max(p[0] for p in ps)
    y0 = min(p[1] for p in ps)
    y1 = max(p[1] for p in ps)
    geruest.append(
        f'<rect x="{x0:.2f}" y="{y0:.2f}" width="{x1 - x0:.2f}" height="{y1 - y0:.2f}"/>'
    )

# Die Spirale als Kette von Viertelkreisen.
teile = []
for i, (q, m, anfang, ende) in enumerate(kette):
    r = q[2] * sk
    a, e, mm = P(anfang), P(ende), P(m)
    if i == 0:
        teile.append(f"M{a[0]:.2f} {a[1]:.2f}")
    v1 = (a[0] - mm[0], a[1] - mm[1])
    v2 = (e[0] - mm[0], e[1] - mm[1])
    kreuz = v1[0] * v2[1] - v1[1] * v2[0]
    teile.append(f"A{r:.2f} {r:.2f} 0 0 {1 if kreuz > 0 else 0} {e[0]:.2f} {e[1]:.2f}")
pfad = " ".join(teile)

# ⛑ Ohne `xmlns`, denn die Marke wird INLINE in HTML benutzt; dort setzt
# der Parser den Namensraum selbst. Steht er trotzdem da, findet die
# Pruefung `nichts_wird_aus_dem_netz_geladen` ein `http://` im HTML und
# schlaegt Alarm, und ihr das beizubringen hiesse, ihr eine Ausnahme
# beizubringen. Eine eigenstaendige Datei braucht ihn, dafuer gibt es
# `--eigenstaendig`.
ns = ' xmlns="http://www.w3.org/2000/svg"' if "--eigenstaendig" in sys.argv else ""
print(
    f'''<svg viewBox="0 0 {B:.2f} {H:.2f}" fill="none"{ns} aria-hidden="true" focusable="false">
<g class="geruest" stroke="currentColor" stroke-width="{STRICH:.2f}" fill="none">
{chr(10).join(geruest)}
</g>
<path class="bogen" d="{pfad}" stroke="currentColor" stroke-width="{STRICH:.2f}"
      fill="none" stroke-linecap="butt"/>
</svg>'''
)
