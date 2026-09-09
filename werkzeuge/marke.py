#!/usr/bin/env python3
"""Erzeugt die Marke: vier goldene Spiralen in ihrer Konstruktion.

# ⚑ Gerechnet und nicht gezeichnet

Quadrate nach Fibonacci, Viertelkreise darin, das Ganze **viermal**, je
um neunzig Grad um den Ursprung gedreht. Linien auf durchsichtigem
Grund; die Farbe kommt aus `currentColor`, die Marke nimmt also die
ihrer Umgebung an.

⛑ **Gezeichnet werden nur die Boegen.** Die Fibonacci-Quadrate sind das
Geruest, an dem die Konstruktion haengt, und sie standen bis zum
2026-09-09 mit in der Marke. Auf Festlegung des Projektinhabers sind
sie weg: In einer Marke sind sie Hilfslinien, und Hilfslinien gehoeren
in die Herleitung und nicht in das fertige Zeichen. Gerechnet werden
sie weiter, denn ohne sie gibt es die Boegen nicht.

⚑ **Der Rahmen bleibt.** Er ist nicht Beiwerk, sondern die Kante, an
der der Schriftzug darunter ausgerichtet wird: Der erste und der letzte
Buchstabe stehen buendig an ihr. Ohne ihn haette die Sperrung des Zugs
kein Mass mehr.

⚑ **Der grosse Kreis aussen wird nicht gezeichnet, er entsteht.**

Die Figur wird so gelegt, dass der **Mittelpunkt des aeussersten
Bogens im Ursprung** liegt, also im Drehpunkt der vier Spiralen. Damit
liegen alle vier aeussersten Boegen auf **demselben** Kreis um den
Ursprung, und weil jeder ein Viertel ist und sie um neunzig Grad
gegeneinander stehen, schliessen sie ihn vollstaendig.

⛑ **Drei Anlaeufe davor haben den Kreis als eigenes Ding behandelt**,
und alle drei sahen falsch aus: einbeschrieben in den Rahmen, dann als
Umkreis der Boegen, dann als zu Ende gezeichneter Bogen je Spirale. In
jedem Fall beruehrt oder schneidet er die Spiralen irgendwo, aber sie
laufen nicht in ihn hinein. Sobald er aus ihnen hervorgeht, ist der
Uebergang tangentenstetig, ohne dass irgendwo etwas angepasst werden
muesste: Es ist derselbe Kreis.

# ⚑ Warum vier, und wie sie aneinanderstossen

Der Projektinhaber hat es am 2026-09-09 in einem Satz gesagt: **die
alte Marke viermal, je um neunzig Grad gedreht, aneinandergesetzt.**

Umgesetzt ist das so: Der Kasten der einen Figur wird mit seiner
**linken unteren Ecke in den Ursprung** gelegt, liegt also ganz im
ersten Quadranten und schmiegt sich an beide Achsen. Die drei
Vierteldrehungen fallen dadurch in die drei anderen Quadranten und
beruehren dieselben Achsen von der anderen Seite. Vier goldene
Rechtecke im Windrad, ohne Ueberlappung und ohne Luecke an den Achsen.

⚑ **Und die Leinwand wird dabei quadratisch, hergeleitet und nicht
zurechtgerueckt.** Ein Rechteck der Breite `b` und Hoehe `h` im ersten
Quadranten spannt zusammen mit seinen Drehungen den Kasten
`[-h, b] x [-h, b]` auf: Die Drehung vertauscht Breite und Hoehe, und
beide Werte kommen deshalb auf jeder Achse einmal vor. Fuer jedes
Seitenverhaeltnis ein Quadrat.

⛑ **`HALBE_DREHUNG` ist weggefallen.** Sie stand hier, um das Auge der
einen Spirale nach unten rechts zu bringen. Eine halbe Drehung bildet
diese Figur auf sich selbst ab, der Schalter tat also nichts mehr und
haette den Naechsten glauben lassen, er koenne etwas.

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

# ⚑ Die Tiefenstaffelung, und warum sie keine Farbe ist

Die Vorlage geht von Gold innen nach Blaugrau aussen. Die Marke bleibt
**schwarzweiss**, auf Festlegung des Projektinhabers. Uebersetzt ist
der Verlauf deshalb in **Deckkraft**: innen voll, aussen schwach. Das
ist derselbe Tiefeneindruck mit einer einzigen Farbe, und er traegt
auch dort, wo die Marke auf hellem Grund steht.

⛑ Ohne diese Staffelung sieht man vier gleich laute Kreise und keine
Spirale: Bei gleicher Deckkraft draengt sich der grosse aeussere Bogen
genauso vor wie der kleine innere, und das Auge findet keinen Anfang.

Aufruf:  python3 werkzeuge/marke.py [--eigenstaendig] > marke.svg
"""
import math
import sys

N = 8            # Quadrate je Spirale, wie in der alten Marke.
ARME = 4         # Spiralen, gleichmaessig um den Ursprung verteilt.
STRICH = 1.7     # Strichbreite in Leinwandeinheiten
# ⚑ Deckkraft des kleinsten und des groessten Bogens; dazwischen linear.
NAH, FERN = 1.0, 0.45


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


def gedreht(p, k):
    """Vierteldrehungen um den Ursprung, ganzzahlig und ohne Sinus.

    ⚑ Eine Drehung um genau neunzig Grad ist ein Tausch mit Vorzeichen.
    Sie ueber `math.cos` zu rechnen brachte Reste in der vierzehnten
    Stelle, und aus achsenparallelen Quadraten wurden Rechtecke mit
    Bruchteilen darin.
    """
    x, y = p
    return [(x, y), (-y, x), (-x, -y), (y, -x)][k % 4]


qs, (ux0, uy0, ux1, uy1) = quadrate(N)
kette = spirale(qs)


# ⚑ **Die Spirale nach innen fortsetzen, mit derselben Regel wie nach
# aussen.** Tangentenstetigkeit heisst: Am Uebergang liegen beide
# Mittelpunkte auf derselben Geraden durch den Punkt. Nach aussen legt
# das den neuen Mittelpunkt fest, nach innen genauso, nur mit dem
# Halbmesser geteilt durch Phi statt mal Phi.
#
# ⛑ **Ein Entwurf davor nahm eine Aehnlichkeitsabbildung, hergeleitet
# aus den zwei AEUSSERSTEN Boegen, und setzte sie am INNERSTEN an.**
# Dort ist sie nicht exakt: Die Fibonacci-Folge beginnt mit 1, 1, das
# Verhaeltnis ist also erst weiter aussen Phi. Der Anschluss sass
# daneben, und die Spirale sah abgehackt aus. Diese Fassung rechnet
# ueberhaupt nicht mit Naeherungen: Jeder Bogen endet genau da, wo der
# vorige beginnt.
PHI = (1 + math.sqrt(5)) / 2


def kreuz(u, v):
    return u[0] * v[1] - u[1] * v[0]


def gedreht_um(p, m, uhrzeigersinn):
    """Vierteldrehung von `p` um `m`, ohne Sinus."""
    dx, dy = p[0] - m[0], p[1] - m[1]
    return (m[0] + dy, m[1] - dx) if uhrzeigersinn else (m[0] - dy, m[1] + dx)


_q0, _m0, _a0, _e0 = kette[0]
# In welche Richtung dreht die Kette? Das Vorzeichen bleibt nach innen
# dasselbe, sonst kaeme ein Knick statt einer Fortsetzung.
_gegen_uhr = kreuz(
    (_a0[0] - _m0[0], _a0[1] - _m0[1]), (_e0[0] - _m0[0], _e0[1] - _m0[1])
) > 0

INNEN = 20
innen = []
_m, _a, _r = _m0, _a0, _q0[2]
for _ in range(INNEN):
    _rn = _r / PHI
    # Der neue Mittelpunkt: vom Uebergangspunkt aus in Richtung des
    # alten, im Abstand des neuen Halbmessers. Dieselbe Regel wie in
    # `spirale`, nur nach innen.
    _dx, _dy = _m[0] - _a[0], _m[1] - _a[1]
    _l = math.hypot(_dx, _dy)
    _mn = (_a[0] + _dx / _l * _rn, _a[1] + _dy / _l * _rn)
    # Der Bogen laeuft von seinem Anfang bis genau auf `_a`.
    _an = gedreht_um(_a, _mn, _gegen_uhr)
    innen.append((_rn, _mn, _an, _a))
    _m, _a, _r = _mn, _an, _rn

# ⚑ Der Mittelpunkt des aeussersten Bogens. Er wird zum Drehpunkt.
_, MITTE_AUSSEN, _, _ = kette[-1]


def gelegt(p):
    """Die Figur so, dass der aeusserste Bogenmittelpunkt im Ursprung liegt.

    ⚑ Genau daraus folgt der grosse Kreis: Alle vier aeussersten Boegen
    liegen dann um denselben Punkt und mit demselben Halbmesser, und
    vier Viertel im Abstand von neunzig Grad ergeben den ganzen Kreis.
    """
    return (p[0] - MITTE_AUSSEN[0], p[1] - MITTE_AUSSEN[1])


# ── Die Figur, viermal gedreht ──────────────────────────────────────
boegen = []
for k in range(ARME):
    for i, (q, m, anfang, ende) in enumerate(kette):
        boegen.append(
            (
                i,
                q[2],
                gedreht(gelegt(m), k),
                gedreht(gelegt(anfang), k),
                gedreht(gelegt(ende), k),
                0,
            )
        )
    # Die Fortsetzung nach innen; sie traegt den Index des innersten
    # Bogens, ist also voll gedeckt. `j` ab 1 zaehlt die Windung und
    # entscheidet ueber die Strichbreite.
    for j, (rad, m, anfang, ende) in enumerate(innen, start=1):
        boegen.append(
            (
                0,
                rad,
                gedreht(gelegt(m), k),
                gedreht(gelegt(anfang), k),
                gedreht(gelegt(ende), k),
                j,
            )
        )

def bogenpunkte(m, a, e, n=64):
    """Punkte auf dem Viertelkreis von `a` nach `e` um `m`."""
    w0 = math.atan2(a[1] - m[1], a[0] - m[0])
    w1 = math.atan2(e[1] - m[1], e[0] - m[0])
    # Der kuerzere Weg; ein Viertelkreis ist nie mehr als ein halber.
    d = (w1 - w0 + math.pi) % (2 * math.pi) - math.pi
    rad = math.hypot(a[0] - m[0], a[1] - m[1])
    return [
        (m[0] + rad * math.cos(w0 + d * i / n), m[1] + rad * math.sin(w0 + d * i / n))
        for i in range(n + 1)
    ]


# ⚑ **Der Massstab haengt am Umkreis und nicht am halben Kasten.**
#
# ⛑ Ein frueher Entwurf skalierte auf `max(|x|, |y|)`, also auf das
# umschliessende Quadrat, und legte den Kreis einbeschrieben darueber.
# Dann stehen die vier grossen Boegen ueber ihn hinaus in die Ecken,
# und er wirkt aufgelegt statt umfassend. In der Vorlage **umschliesst**
# der Kreis die Figur und die Boegen beruehren ihn von innen. Gemessen
# wird deshalb der groesste Abstand vom Mittelpunkt, und der liegt auf
# einem Bogen und nicht notwendig an seinen Enden: Ein Bogen woelbt
# sich. Er wird darum abgetastet.
# Der Massstab haengt am Aeussersten der Figur.
r = max(math.hypot(p[0], p[1])
        for _, _, m, a, e, _ in boegen
        for p in bogenpunkte(m, a, e))

# ⚑ Die Leinwand ist quadratisch, weil die Figur es ist: Die
# Vereinigung mit ihren Vierteldrehungen ist drehinvariant. `r` ist
# jetzt ihr Umkreis, und der Kreis unten hat genau diesen Halbmesser.
S = 100.0
rand = STRICH / 2 + 0.4
sk = (S / 2 - rand) / r


def P(p):
    """Modell nach Leinwand, y umgedreht (SVG zeigt nach unten)."""
    x, y = p
    return (S / 2 + x * sk, S / 2 - y * sk)


def deckkraft(i):
    """Innen voll, aussen schwach; dazwischen linear ueber den Index."""
    if N < 2:
        return NAH
    return NAH + (FERN - NAH) * i / (N - 1)


# ── Rahmen und Kreis, und sonst nichts vom Geruest ──────────────────
# ⛑ Die Quadrate werden gerechnet, aber nicht gezeichnet: Sie sind die
# Herleitung der Boegen und in einer Marke Hilfslinien.
geruest = [
    f'<rect x="{rand:.2f}" y="{rand:.2f}" '
    f'width="{S - 2 * rand:.2f}" height="{S - 2 * rand:.2f}" opacity="{FERN:.2f}"/>',
]

# ── Die Boegen, je einer als eigener Pfad wegen der Deckkraft ───────
# ⛑ Ein einziger Pfad je Spirale waere kuerzer und traege nur EINE
# Deckkraft. Die Staffelung ist aber der Grund, warum die Figur als
# Spirale lesbar ist; sie gehoert also je Bogen gesetzt.
# ⚑ **Die Strichbreite bleibt gleich, auf Festlegung des
# Projektinhabers.** Der Strich laeuft einfach so weit nach innen, bis
# er in die vorige Windung muendet.
#
# ⛑ Zwei Entwuerfe davor liessen ihn mitschrumpfen, erst am Halbmesser
# und dann linear. Beide machen aus einer Strichzeichnung einen
# Verlauf, und die Marke soll eine Strichzeichnung bleiben.
#
# ⚑ **Das Auge laeuft in einen vollen Punkt aus, und das ist gewollt.**
# Die Windungen ruecken um 1/Phi zusammen, der Strich bleibt gleich
# breit, also schliessen sie sich irgendwann zu einer Flaeche. Genau
# das zeigt eine Spirale, die nicht aufhoert: Sie wird nicht duenner,
# sie wird nur unaufloesbar.
#
# ⛑ Zwei Entwuerfe davor haben genau das vermieden, erst mit einem
# mitschrumpfenden Strich, dann mit einem frueheren Abbruch. Beide
# liessen das Auge stumpf enden, statt es zu schliessen.
#
# ⚑ Gezeichnet wird bis weit unter die Strichbreite. Die Grenze ist
# ein Sechzigstel davon, und sie ist nicht gegriffen: Bei einem
# Zwanzigstel blieb im Zentrum ein weisser Punkt stehen, weil die
# Windungen dort noch nicht zusammengelaufen waren. Was danach kaeme,
# laege bei jeder Vergroesserung in der Farbe des Vorigen und waere nur
# noch Dateigroesse.
SICHTBAR = STRICH / 60

pfade = []
for i, radius, m, anfang, ende, _windung in boegen:
    if radius * sk < SICHTBAR:
        continue
    a, e, mm = P(anfang), P(ende), P(m)
    v1 = (a[0] - mm[0], a[1] - mm[1])
    v2 = (e[0] - mm[0], e[1] - mm[1])
    kreuz = v1[0] * v2[1] - v1[1] * v2[0]
    rr = radius * sk
    pfade.append(
        f'<path d="M{a[0]:.2f} {a[1]:.2f} A{rr:.2f} {rr:.2f} 0 0 '
        f'{1 if kreuz > 0 else 0} {e[0]:.2f} {e[1]:.2f}" opacity="{deckkraft(i):.2f}"/>'
    )

# ⛑ Ohne `xmlns`, denn die Marke wird INLINE in HTML benutzt; dort setzt
# der Parser den Namensraum selbst. Steht er trotzdem da, findet die
# Pruefung `nichts_wird_aus_dem_netz_geladen` ein `http://` im HTML und
# schlaegt Alarm, und ihr das beizubringen hiesse, ihr eine Ausnahme
# beizubringen. Eine eigenstaendige Datei braucht ihn, dafuer gibt es
# `--eigenstaendig`.
ns = ' xmlns="http://www.w3.org/2000/svg"' if "--eigenstaendig" in sys.argv else ""
print(
    f'''<svg viewBox="0 0 {S:.2f} {S:.2f}" fill="none"{ns} aria-hidden="true" focusable="false">
<g class="geruest" stroke="currentColor" stroke-width="{STRICH:.2f}" fill="none">
{chr(10).join(geruest)}
</g>
<g class="bogen" stroke="currentColor" stroke-width="{STRICH:.2f}" fill="none"
   stroke-linecap="butt">
{chr(10).join(pfade)}
</g>
</svg>'''
)
