#!/usr/bin/env python3
"""Erzeugt die Marke: eine duale Phi-Spirale mit Sternen.

# ⚑ Gerechnet und nicht gezeichnet

Die Quadrate folgen der Fibonacci-Folge, die Boegen sind Viertelkreise
darin. Welche Ecke der Mittelpunkt ist, wird **hergeleitet** und nicht
gesucht:

⛑ **Der erste Entwurf suchte die Mittelpunkte so, dass die Bogenenden
aufeinanderfallen, und das genuegt nicht.** Enden koennen
zusammenfallen, waehrend die Kruemmung kippt; heraus kam ein
vierzackiger Stern mit Spitzen statt einer Spirale. Was fehlt, ist die
**Tangentenstetigkeit**: Am Uebergang muessen beide Mittelpunkte auf
derselben Geraden durch den Punkt liegen.

Daraus folgt die Regel eindeutig: Der neue Mittelpunkt ist die Ecke des
neuen Quadrats, die vom Uebergangspunkt aus **in dieselbe Richtung**
liegt wie der alte Mittelpunkt. Keine Suche, kein Raten.

# ⚑ Zwei Arme

Die „Dual Phi Spiral": derselbe Pfad noch einmal, um 180 Grad um das
Auge gedreht. Ein Arm allein ist eine Schnecke, zwei sind eine Galaxie.
Der zweite steht schwaecher, damit die Marke bei zwanzig Pixeln kein
Knaeuel wird.
"""
import math, random, sys

N = 7   # Quadrate. Mehr macht den aeussersten Bogen so gross,
        # dass alles davor zum Punkt schrumpft.

def quadrate(n):
    f = [1, 1]
    while len(f) < n:
        f.append(f[-1] + f[-2])
    qs = [(0.0, 0.0, 1.0)]
    ux0, uy0, ux1, uy1 = 0.0, 0.0, 1.0, 1.0
    for i in range(1, n):
        s = float(f[i])
        d = ["links", "unten", "rechts", "oben"][(i - 1) % 4]
        q = {"links": (ux0 - s, uy0, s), "unten": (ux0, uy0 - s, s),
             "rechts": (ux1, uy0, s), "oben": (ux0, uy1, s)}[d]
        qs.append(q)
        ux0 = min(ux0, q[0]); uy0 = min(uy0, q[1])
        ux1 = max(ux1, q[0] + s); uy1 = max(uy1, q[1] + s)
    return qs, (ux0, uy0, ux1, uy1)

def ecken(q):
    x, y, s = q
    return [(x, y), (x + s, y), (x + s, y + s), (x, y + s)]

def nah(a, b): return abs(a[0] - b[0]) < 1e-9 and abs(a[1] - b[1]) < 1e-9

def spirale(qs):
    """Boegen mit Tangentenstetigkeit, hergeleitet statt gesucht."""
    # Der erste Bogen: Mitte im Ursprung, von (1,0) nach (0,1).
    mitte, p, ende = (0.0, 0.0), (1.0, 0.0), (0.0, 1.0)
    kette = [(qs[0], mitte, p, ende)]
    p, vorige_mitte = ende, mitte
    for q in qs[1:]:
        richtung = (vorige_mitte[0] - p[0], vorige_mitte[1] - p[1])
        laenge = math.hypot(*richtung)
        e = (richtung[0] / laenge, richtung[1] / laenge)
        # Die Ecke, die vom Uebergangspunkt in dieselbe Richtung liegt.
        soll = (p[0] + e[0] * q[2], p[1] + e[1] * q[2])
        treffer = [c for c in ecken(q) if nah(c, soll)]
        if not treffer:
            raise SystemExit(f"keine tangentenstetige Ecke in {q}")
        m = treffer[0]
        nachbarn = [c for c in ecken(q)
                    if not nah(c, m) and abs(math.hypot(c[0] - m[0], c[1] - m[1]) - q[2]) < 1e-9]
        ziel = [c for c in nachbarn if not nah(c, p)][0]
        kette.append((q, m, p, ziel))
        p, vorige_mitte = ziel, m
    return kette

qs, _ = quadrate(N)
kette = spirale(qs)

def bogenpunkte(kette, schritte=48):
    """Punkte auf der gezeichneten Kurve.

    ⛑ **Gemessen wird an der KURVE und nicht an den Quadraten.** Der
    erste Entwurf passte auf die Quadrate an, und der aeusserste Bogen
    lief oben rechts aus dem Bild: Ein Viertelkreis in einem Quadrat
    bleibt zwar darin, der um 180 Grad gedrehte zweite Arm aber nicht.
    """
    aus = []
    for q, m, a, e in kette:
        r = q[2]
        w0 = math.atan2(a[1] - m[1], a[0] - m[0])
        w1 = math.atan2(e[1] - m[1], e[0] - m[0])
        d = (w1 - w0) % (2 * math.pi)
        if d > math.pi:
            d -= 2 * math.pi
        for i in range(schritte + 1):
            w = w0 + d * i / schritte
            aus.append((m[0] + math.cos(w) * r, m[1] + math.sin(w) * r))
    return aus

# ⚑ Das Auge: die Mitte des innersten Bogens. Beide Arme sind darum
# punktsymmetrisch, die Figur ist damit um diesen Punkt ausgewogen.
pol = kette[0][1]
arm1 = bogenpunkte(kette)
arm2 = [(2 * pol[0] - x, 2 * pol[1] - y) for x, y in arm1]
alle = arm1 + arm2 + [e for q in qs for e in ecken(q)]

# ⚑ **Die Vierteldrehung steckt im Modell und nicht in einem
# `transform` am Ende.** Ein nachtraeglicher Dreh liesse die Marke aus
# ihrem Kasten laufen, weil die Anpassung dann die UNGEDREHTE Figur
# gemessen haette; genau dieser Fehler war beim zweiten Arm schon
# einmal da.
DREHUNG = math.pi / 2

def dreh(p):
    c, si = math.cos(DREHUNG), math.sin(DREHUNG)
    dx, dy = p[0] - pol[0], p[1] - pol[1]
    return (pol[0] + dx * c - dy * si, pol[1] + dx * si + dy * c)

alle = [dreh(p) for p in alle]
ux0 = min(p[0] for p in alle); ux1 = max(p[0] for p in alle)
uy0 = min(p[1] for p in alle); uy1 = max(p[1] for p in alle)

rand = 6.0
sk = (100 - 2 * rand) / max(ux1 - ux0, uy1 - uy0)
ox = rand + ((100 - 2 * rand) - (ux1 - ux0) * sk) / 2
oy = rand + ((100 - 2 * rand) - (uy1 - uy0) * sk) / 2
def P(p):
    q = dreh(p)
    return (ox + (q[0] - ux0) * sk, 100 - (oy + (q[1] - uy0) * sk))

teile = []
for i, (q, m, anfang, ende) in enumerate(kette):
    r = q[2] * sk
    a, e, mm = P(anfang), P(ende), P(m)
    if i == 0:
        teile.append(f"M{a[0]:.2f} {a[1]:.2f}")
    v1 = (a[0] - mm[0], a[1] - mm[1]); v2 = (e[0] - mm[0], e[1] - mm[1])
    kreuz = v1[0] * v2[1] - v1[1] * v2[0]
    teile.append(f"A{r:.2f} {r:.2f} 0 0 {1 if kreuz > 0 else 0} {e[0]:.2f} {e[1]:.2f}")
pfad = " ".join(teile)

# --- Der kalligraphische Duktus -----------------------------------------
#
# ⛑ **Eine Strichlinie hat ueberall dieselbe Breite, und genau das
# sieht weich und beliebig aus.** `stroke` kennt nur eine Breite; mit
# runden Enden wird daraus ein Schlauch. Eine Breitfeder liegt in einem
# FESTEN Winkel, und die sichtbare Breite folgt daraus, wie die Kurve
# gerade zu dieser Feder steht: laengs duenn, quer breit. Das ist der
# Duktus, und er entsteht nicht aus einer Linie, sondern aus einem
# **Umriss**, der gefuellt wird.
#
# ⚑ **Gerechnet als Minkowski-Summe der Kurve mit der Feder.** Die
# Feder ist eine Strecke im Winkel FEDERWINKEL, Laenge 2*FEDER. Der
# obere Rand ist die Kurve plus die halbe Feder, der untere minus. Wo
# die Kurve parallel zur Feder laeuft, faellt die Breite auf null; das
# ist echte Kalligraphie und kein Fehler.
#
# ⚑ **Der Zusatz `KANTE` haelt sie davon ab, ganz zu verschwinden.**
# Eine echte Feder hat auch quer eine Dicke; ohne sie risse der Strich
# an zwei Stellen der Spirale.
#
# ⚑ **Die Enden sind ein gerader Schnitt** im Winkel der Feder, und
# zwar von selbst: Der Umriss schliesst dort ueber die Federstrecke.
# Kein `linecap`, keine Rundung, harte Aussenkanten.
FEDERWINKEL = math.radians(32)
FEDER = 2.30          # halbe Federbreite, in Leinwandeinheiten
KANTE = 0.42          # Dicke quer zur Feder

# --- Was den Zug nach Hand aussehen laesst -------------------------------
#
# ⛑ **Eine exakt gerechnete Feder sieht gerechnet aus, und genau das
# war der Einwand.** Eine Hand haelt die Feder nicht in einem festen
# Winkel, sie drueckt nicht gleichmaessig, und sie faehrt nicht auf der
# Ideallinie. Was fehlt, sind vier Dinge, und alle vier sind
# **glatt** und nicht zufaellig: Rauschen sieht nach Zittern aus,
# Handschrift schwankt langsam.
#
# | | was es tut |
# |---|---|
# | `DRIFT` | Die Feder dreht sich langsam mit, wie ein Handgelenk |
# | `DRUCK` | Der Strich schwillt und schwindet, wie der Druck der Hand |
# | `ZUG` | Die Mittellinie weicht minimal von der Idealkurve ab |
# | Ansatz und Abstrich | Die Feder setzt an und hebt ab, also laeuft der Strich an beiden Enden duenn aus |
#
# ⚑ **Gebaut aus drei Sinuswellen mit festen Phasen** und nicht aus
# einem Zufallszahlengeber: So ist die Marke bei jedem Lauf dieselbe,
# und die Schwankung ist von sich aus glatt statt geglaettet.
DRIFT = math.radians(11)
DRUCK = 0.30
ZUG = 0.75

def welle(t, phasen):
    """Eine glatte Schwankung zwischen -1 und 1."""
    a, b, c = phasen
    return (math.sin(t * 2.1 * math.pi + a) * 0.55
            + math.sin(t * 3.7 * math.pi + b) * 0.30
            + math.sin(t * 5.3 * math.pi + c) * 0.15)

def umriss(punkte, phasen=(0.7, 2.3, 4.1)):
    """Der gefuellte Umriss eines Federzugs entlang der Punkte."""
    oben, unten = [], []
    n = len(punkte) - 1
    for i, (x, y) in enumerate(punkte):
        t = i / n
        # Ansatz und Abstrich: die Feder setzt an und hebt ab. Ohne das
        # endet der Strich in einem Balken, und nichts sieht weniger
        # nach Hand aus als ein abgeschnittener Anfang.
        rand_ab = min(1.0, (t / 0.055) ** 0.7) * min(1.0, ((1 - t) / 0.085) ** 0.55)
        w = FEDERWINKEL + DRIFT * welle(t, phasen)
        f = FEDER * (1 + DRUCK * welle(t, (phasen[1], phasen[2], phasen[0]))) * (0.28 + 0.72 * rand_ab)
        fx, fy = math.cos(w), math.sin(w)
        a = punkte[max(0, i - 1)]
        b = punkte[min(n, i + 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        L = math.hypot(tx, ty) or 1.0
        nx, ny = -ty / L, tx / L
        # Der Zug: die Mittellinie weicht langsam von der Ideallinie ab.
        vx = x + nx * ZUG * welle(t, (phasen[2], phasen[0], phasen[1]))
        vy = y + ny * ZUG * welle(t, (phasen[2], phasen[0], phasen[1]))
        k = KANTE * (0.3 + 0.7 * rand_ab)
        oben.append((vx + fx * f + nx * k, vy + fy * f + ny * k))
        unten.append((vx - fx * f - nx * k, vy - fy * f - ny * k))
    z = ["M%.1f %.1f" % oben[0]]
    z += ["L%.1f %.1f" % p for p in oben[1:]]
    z += ["L%.1f %.1f" % p for p in reversed(unten)]
    z.append("Z")
    return " ".join(z)

# ⛑ **Die beiden Einheitsquadrate bleiben ungeschrieben.** Ihre Boegen
# haben einen kleineren Radius als die Feder breit ist; der Strich
# ueberlappte sich dort selbst, und in der Mitte stand ein weisser
# Klumpen statt eines Auges.
#
# ⚑ **Die Zahl der Stuetzpunkte waechst mit dem Radius.**
def duktuspunkte(kette):
    aus = []
    for glied in kette[2:]:
        schritte = max(14, int(glied[0][2] * 26))
        aus += [P(p) for p in bogenpunkte([glied], schritte)]
    return aus

bahn = duktuspunkte(kette)
duktus = umriss(bahn)
# ⚑ **Der zweite Arm bekommt andere Phasen.** Eine Hand schreibt
# denselben Zug nie zweimal gleich; zwei identische Arme verraten die
# Rechnung sofort.
duktus2 = umriss(bahn, (2.9, 5.6, 1.4))

geruest = []
for q in qs:
    # ⚑ Nach einer Vierteldrehung bleibt ein Quadrat achsenparallel,
    # aber welche Ecke links oben liegt, wechselt. Deshalb min und max
    # ueber alle vier statt zwei festen Ecken.
    ps = [P(e) for e in ecken(q)]
    x0 = min(p[0] for p in ps); x1 = max(p[0] for p in ps)
    y0 = min(p[1] for p in ps); y1 = max(p[1] for p in ps)
    geruest.append(f'<rect x="{x0:.2f}" y="{y0:.2f}" '
                   f'width="{x1-x0:.2f}" height="{y1-y0:.2f}"/>')

mx, my = P(pol)

# --- Das Universum ------------------------------------------------------
#
# ⚑ **Prozedural und nicht als Bild.** Ein eingebettetes Foto waere
# genau das flaechige Bildnis, das nicht gewuenscht ist, es waere bei
# jeder Groesse gleich scharf oder gleich unscharf, und es waere ein
# fremdes Werk in einem Repositorium, das keine Fremdquellen zulaesst.
# `feTurbulence` rechnet den Nebel im Renderer aus: aufloesungsunabhaengig,
# in jeder Groesse richtig, und aus nichts als Mathematik.
#
# ⚑ **Drei Schichten, wie auf einer echten Aufnahme:** Gas (weiche
# Wolken), Staub (dunkle Baender laengs der Arme) und Sterne. Das Gas
# liegt zuunterst und wird zum Rand hin ausgeblendet, sonst haette die
# Galaxie eine Kante.
random.seed(1618)

sterne = []
while len(sterne) < 150:
    w = random.random() * 2 * math.pi
    d = (random.random() ** 0.62) * 60
    x, y = mx + math.cos(w) * d, my + math.sin(w) * d
    if not (1.5 < x < 98.5 and 1.5 < y < 98.5):
        continue
    kern = 1 - d / 60
    r = 0.28 + kern * 0.85 * random.random()
    o = 0.16 + kern * 0.74
    sterne.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" opacity="{o:.2f}"/>')

# ⚑ Ein paar helle Sterne bekommen ein Beugungskreuz. Auf jeder echten
# Teleskopaufnahme haben genau die hellsten eines, und das Auge liest
# daran „Aufnahme" statt „Punktmuster".
hell = []
for _ in range(7):
    w = random.random() * 2 * math.pi
    d = 12 + random.random() * 46
    x, y = mx + math.cos(w) * d, my + math.sin(w) * d
    if not (6 < x < 94 and 6 < y < 94):
        continue
    l = 2.2 + random.random() * 2.6
    hell.append(
        f'<g opacity="{0.5 + random.random() * 0.35:.2f}">'
        f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{0.7 + random.random() * 0.5:.2f}"/>'
        f'<path d="M{x - l:.2f} {y:.2f}h{2 * l:.2f}M{x:.2f} {y - l:.2f}v{2 * l:.2f}" '
        f'stroke="currentColor" stroke-width="0.28" opacity="0.6" fill="none"/></g>'
    )

# ⛑ **Die Staubbahnen liegen AUF den Armen und nicht irgendwo.** Auf
# einer Spiralgalaxie folgt der Staub den Armen; ein zufaellig gelegtes
# dunkles Band saehe aus wie ein Kratzer. Deshalb derselbe Pfad noch
# einmal, breit, dunkel und leicht versetzt.
# ⛑ **Helle Knoten AUF den Armen.** Auf der Aufnahme, die der
# Projektinhaber gegeben hat, sitzen die hellen Flecken nicht
# irgendwo, sondern in den Armen: Dort entstehen Sterne. Ohne sie ist
# der Nebel eine gleichmaessige Wolke, und gleichmaessig ist das
# Gegenteil von echt.
knoten = []
for i in range(26):
    q = bahn[int((0.06 + 0.9 * random.random()) * (len(bahn) - 1))]
    w = random.random() * 2 * math.pi
    d = random.random() ** 0.7 * 4.4
    x, y = q[0] + math.cos(w) * d, q[1] + math.sin(w) * d
    if not (2 < x < 98 and 2 < y < 98):
        continue
    for xx, yy in ((x, y), (2 * mx - x, 2 * my - y)):
        knoten.append(
            f'<circle cx="{xx:.1f}" cy="{yy:.1f}" r="{0.6 + random.random() * 1.1:.1f}" '
            f'opacity="{0.07 + random.random() * 0.11:.2f}"/>'
        )

staub = (
    f'<path d="{pfad}" stroke="#000" stroke-width="6" opacity="0.20" fill="none" '
    f'stroke-linecap="round" transform="translate(1.6 1.6)"/>'
    f'<path d="{pfad}" stroke="#000" stroke-width="6" opacity="0.20" fill="none" '
    f'stroke-linecap="round" transform="rotate(180 {mx:.2f} {my:.2f}) translate(1.6 1.6)"/>'
)

# ⛑ **Ohne `xmlns`, denn die Marke wird INLINE in HTML benutzt.** Dort
# setzt der Parser den Namensraum selbst. Steht er trotzdem da, findet
# die Pruefung `nichts_wird_aus_dem_netz_geladen` ein `http://` im HTML
# und schlaegt Alarm; sie kann einen Namensraum nicht von einer
# Verbindung unterscheiden, und ihr das beizubringen hiesse, ihr eine
# Ausnahme beizubringen. Eine eigenstaendige Datei braucht ihn, dafuer
# gibt es `--eigenstaendig`.
ns = ' xmlns="http://www.w3.org/2000/svg"' if "--eigenstaendig" in sys.argv else ""
svg = f'''<svg viewBox="0 0 100 100" fill="none"{ns} aria-hidden="true" focusable="false">
<defs>
<filter id="myl-gas" x="-20%" y="-20%" width="140%" height="140%" color-interpolation-filters="sRGB">
<!-- ⚑ Zwei Frequenzen uebereinander: die grobe gibt die Wolken, die
     feine die Koernung. Eine allein sieht entweder nach Watte oder
     nach Rauschen aus. -->
<feTurbulence type="fractalNoise" baseFrequency="0.045" numOctaves="5" seed="7" result="grob"/>
<feTurbulence type="fractalNoise" baseFrequency="0.19" numOctaves="3" seed="19" result="fein"/>
<feBlend in="grob" in2="fein" mode="overlay" result="w"/>
<feColorMatrix in="w" type="matrix"
  values="0 0 0 0 0.93  0 0 0 0 0.95  0 0 0 0 0.97  1.05 0.55 0.2 0 -0.52"/>
</filter>
<!-- ⚑ Am Auge der Spirale ausgerichtet und nicht an der Mitte des
     Kastens: Sonst leuchtet das Gas woanders als der Kern, und die
     Galaxie haette zwei Zentren. -->
<radialGradient id="myl-huelle" gradientUnits="userSpaceOnUse"
                cx="{mx:.2f}" cy="{my:.2f}" r="58">
<stop offset="0%" stop-color="#fff" stop-opacity="0.95"/>
<stop offset="42%" stop-color="#fff" stop-opacity="0.55"/>
<stop offset="78%" stop-color="#fff" stop-opacity="0.10"/>
<stop offset="100%" stop-color="#fff" stop-opacity="0"/>
</radialGradient>
<filter id="myl-weich" x="-30%" y="-30%" width="160%" height="160%">
<feGaussianBlur stdDeviation="4.5"/>
</filter>
<!-- ⛑ **Die Maske ist nicht rund, sie folgt den Armen.** Eine
     Spiralgalaxie traegt ihr helles Gas IN den Armen; eine runde
     Wolke daraus zu machen war der groesste Unterschied zur
     Aufnahme. Grundhelligkeit aus dem Verlauf, darueber zwei breite,
     weichgezeichnete Kopien der Bahn. -->
<mask id="myl-maske">
<rect x="0" y="0" width="100" height="100" fill="url(#myl-huelle)" opacity="0.55"/>
<g filter="url(#myl-weich)" fill="none" stroke="#fff" stroke-width="15"
   stroke-linecap="round" opacity="0.62">
<path d="{pfad}"/>
<path d="{pfad}" transform="rotate(180 {mx:.2f} {my:.2f})"/>
</g>
</mask>
<radialGradient id="myl-kern" gradientUnits="userSpaceOnUse" cx="{mx:.2f}" cy="{my:.2f}" r="34">
<stop offset="0%" stop-color="currentColor" stop-opacity="0.34"/>
<stop offset="35%" stop-color="currentColor" stop-opacity="0.11"/>
<stop offset="100%" stop-color="currentColor" stop-opacity="0"/>
</radialGradient>
</defs>

<g class="universum">
<g mask="url(#myl-maske)">
<rect x="0" y="0" width="100" height="100" filter="url(#myl-gas)" opacity="0.34"/>
</g>
<g class="knoten" fill="currentColor">{chr(10).join(knoten)}</g>
<circle cx="{mx:.2f}" cy="{my:.2f}" r="34" fill="url(#myl-kern)"/>
<g class="staub">{staub}</g>
</g>

<g class="sterne" fill="currentColor" stroke="none">
{chr(10).join(sterne)}
{chr(10).join(hell)}
</g>
<g class="geruest" stroke="currentColor" stroke-width="0.55" opacity="0.22" fill="none">
{chr(10).join(geruest)}
</g>
<g class="arme" fill="currentColor" stroke="none" shape-rendering="geometricPrecision">
<path d="{duktus}"/>
<path d="{duktus2}" opacity="0.62" transform="rotate(180 {mx:.2f} {my:.2f})"/>
</g>
</svg>'''
print(svg)
