#!/usr/bin/env python3
"""Misst, wie weit ein ganzzahliger Zustand von der Gleitkomma-Referenz wegdriftet.

## ⛔️ Wozu diese Messung da ist, und warum sie vor jedem Bau kommt

Die Qwen3.5-Familie (`Qwen3_5MoeForConditionalGeneration`, im Projekt als
grosses Modell vorgesehen) rechnet drei Viertel ihrer Ebenen **nicht** mit
Softmax-Aufmerksamkeit, sondern mit einer **rekurrenten Zustandsschicht**
(Gated DeltaNet). Die Referenz fuehrt deren Zustand ausdruecklich in
`float32`.

⚑ **Damit steht eine Frage im Raum, die dieses Projekt noch nie hatte.**
Bisher lautet jede Zusage „bitgleich", und sie traegt, weil
Ganzzahladdition assoziativ ist: Reduktionen darf man umordnen. **Eine
Rekurrenz ist keine Reduktion.** Der Zustand bei Token i haengt am Zustand
bei i-1, es gibt nichts umzuordnen, und der Gewinn liegt woanders: Bei
gleichem Ganzzahlzustand rechnet jeder Knoten denselben Folgezustand, Bit
fuer Bit. **Der Ganzzahlpfad ist hier reproduzierbarer als die Referenz**,
nicht weniger.

⚠️ **Was dafuer neu zu zeigen ist, ist eine Guetefrage:** Bleibt der
ganzzahlige Zustand ueber N Token nah genug an der Referenz? Dafuer hat
das Projekt bisher kein Verfahren, weil es bisher nur
Bitgleichheitsaussagen brauchte.

## ⚑ Die Hypothese, die hier geprueft wird

**Der Zerfall arbeitet in beide Richtungen, und das ist der Kern.** Je
Token wird der Zustand mit `g_t = exp(g) < 1` multipliziert. Das

  - **erzeugt** einen Rundungsfehler, denn ein Multiplizieren mit einem
    Faktor unter eins ist ein Rechtsschieben, und
  - **daempft** jeden aelteren Fehler mit demselben Faktor.

Ein Fehler aus Schritt k ist bei Schritt n also mit `g^(n-k)` gewichtet.
Die Summe ueber alle Schritte konvergiert damit gegen etwa `eps/(1-g)`
statt linear zu wachsen. ⛔️ **Wenn das stimmt, ist die Drift begrenzt und
nicht kumulativ**, und die Architektur traegt. Wenn `g` zu nah an eins
liegt, waechst die Schranke `1/(1-g)` ueber jedes Budget.

**Genau diese Schranke misst dieses Skript**, und zwar als Funktion der
Folgenlaenge, der Zustandsbreite und des Zerfalls.

📌 **Erfundene Eingaben pruefen, was man sich vorgestellt hat; echte
pruefen, was man vergessen hat.** Die Zerfallsverteilung kommt deshalb aus
den **echten** `A_log`- und `dt_bias`-Tensoren, sobald das Modell lokal
liegt (`--gewichte`). Ohne sie laeuft ein Kehrwertfeld ueber plausible
Zerfaelle, und die Ausgabe sagt, welcher Fall gemessen wurde.

## Aufruf

    python3 INTEGER_LLM/tests/diag/zustandsdrift.py
    python3 INTEGER_LLM/tests/diag/zustandsdrift.py --gewichte MODELS/llm/Qwen3.6-35B-A3B
    python3 INTEGER_LLM/tests/diag/zustandsdrift.py --lang   # bis 10 000 Token

⚑ **Es braucht kein torch und kein Modell.** Verglichen wird **derselbe
ganzzahlige Code gegen sich selbst**, einmal mit der zu pruefenden
Zustandsbreite und einmal mit sehr viel Luft (48 Bit). ⚠️ **Nicht gegen
`float32`**, und das ist Absicht: Eine Gleitkomma-Referenz rundet selbst,
dann haelte man zwei Fehler gegeneinander. Die breite Ganzzahlfassung
rundet am Zerfall zwar auch, aber um Groessenordnungen weniger, und sie
beantwortet genau die Frage, die zu entscheiden ist: **Wieviele Bits
braucht der Zustand?**

📌 **Ein erster Entwurf nahm `fractions.Fraction` als exakte Seite.** Das
ist richtig und unbrauchbar: Der Zerfall hat den Nenner 2^16, und nach n
Schritten traegt der Bruch 16n Bits Nenner. Bei 200 Token sind das 3200,
und der Lauf dauerte eine Minute fuer vier Zeilen.
"""

import argparse
import json
import math
import random
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent.parent


# --------------------------------------------------------------- Rundung
def rshift_round(wert: int, shift: int) -> int:
    """Arithmetischer Rechtsshift mit Runden zur naechsten geraden Zahl.

    ⚑ **Die Regel dieses Projekts, zeichengleich zu
    `kernels/src/fixed_point.rs::rshift_round`.** Sie steht hier noch
    einmal, weil dieses Skript ohne die Rust-Seite laufen soll; ⚠️ **wer
    eine von beiden aendert, aendert beide**, sonst messen sie
    Verschiedenes.
    """
    if shift == 0:
        return wert
    maske = (1 << shift) - 1
    haelfte = 1 << (shift - 1)
    quotient = wert >> shift
    rest = wert & maske
    if rest > haelfte or (rest == haelfte and (quotient & 1) != 0):
        return quotient + 1
    return quotient


# ------------------------------------------------- die Rekurrenz, ganzzahlig
#
# ⛔️ **Die Shiftrechnung ist der ganze Kern, und der erste Entwurf hatte
# sie falsch.** Er fuehrte den Zustand mit derselben Auflösung wie q, k
# und v; damit rundet die Rang-1-Fortschreibung `k_a * delta_b` auf null,
# sobald das Produkt klein ist, und der Abstand lag bei 100 Prozent schon
# bei **einem** Token. 📌 **Ein Messaufbau, der bei n=1 versagt, misst
# nicht die Drift, sondern sich selbst.**
#
# ⚑ **Der Zustand braucht eine eigene, groessere Auflösung**, und genau
# sie ist der freie Parameter dieser Messung:
#
#   q, k, v   FRAC Bits
#   Zustand   s_frac Bits          <- gesucht
#   Zerfall   ZERFALL_FRAC Bits
#   beta      BETA_FRAC Bits
#
# Damit lauten die Schiebeweiten:
#
#   1. Zerfall:   S * g          >> ZERFALL_FRAC     (S bleibt s_frac)
#   2. Lesen:     sum S * k      >> s_frac           (ergibt FRAC)
#   3. Korrektur: (v - kv) * b   >> BETA_FRAC        (bleibt FRAC)
#   4. Update:    k * delta      -> s_frac, also << (s_frac - 2*FRAC)
#                                  oder >> (2*FRAC - s_frac)
#   5. Ausgabe:   sum S * q      >> s_frac           (ergibt FRAC)
#
# ⚑ **Bei `s_frac >= 2*FRAC` ist Schritt 4 ein LINKSshift und damit
# verlustfrei.** Das ist die eigentliche Erkenntnis der Schieberechnung:
# Die Fortschreibung des Zustands muss nicht runden, wenn er breit genug
# ist. Gerundet wird dann nur noch am Zerfall und an den zwei
# Kontraktionen.


def lauf(q, k, v, g_int, beta_int, kopf_dim, frac, s_frac, zerfall_frac, beta_frac,
         komplement=False):
    """Gated DeltaNet in Festkomma, Zustand mit eigener Auflösung `s_frac`.

    ⛔️ **`komplement` war eine Hypothese von mir, und die Messung hat sie
    widerlegt.** Sie stehen beide noch hier, weil die Widerlegung die
    nuetzlichere Auskunft ist.

    - `False`: `S * g`, mit `g_int` als Vielfaches von `2^-zerfall_frac`.
    - `True`: `S - S * d`, mit `d_int = (1-g)` als Vielfaches von
      `2^-zerfall_frac`.

    **Der Gedanke war:** Ein Zerfall nahe eins traegt seine Information in
    `1-g`; bei `g = 0,999863` ist das rund `2^-12,8`. Also muesse eine
    Darstellung von `1-g` die entscheidenden Stellen besser treffen.

    ⛔️ **Der Gedanke ist falsch, und zwar aus einem Satz:** Beide
    Darstellungen runden auf **dieselbe absolute** Schrittweite
    `2^-zerfall_frac`, also hat `g = 1 - d` in beiden Faellen denselben
    absoluten Fehler `2^-(zerfall_frac+1)`. **Und in das Produkt `g^N`
    geht der absolute Fehler ein, nicht der relative.** Die Messung zeigt
    darum zwei **identische** Tabellen.

    📌 **Die Lehre ist allgemeiner als dieser Fall:** Eine Umparametrisierung
    verschiebt Genauigkeit nur, wenn sie die **Skala** mitverschiebt. Wer
    dieselbe Schrittweite behaelt, hat nichts getauscht.
    """
    S = [[0] * kopf_dim for _ in range(kopf_dim)]
    # Schritt 4: nach s_frac bringen. Negativ heisst Linksshift, also exakt.
    update_shift = 2 * frac - s_frac
    ausgaben = []
    for t in range(len(q)):
        q_t, k_t, v_t = q[t], k[t], v[t]
        g_t, beta_t = g_int[t], beta_int[t]

        for a in range(kopf_dim):
            zeile = S[a]
            for b in range(kopf_dim):
                if zeile[b]:
                    if komplement:
                        zeile[b] -= rshift_round(zeile[b] * g_t, zerfall_frac)
                    else:
                        zeile[b] = rshift_round(zeile[b] * g_t, zerfall_frac)

        kv_mem = [0] * kopf_dim
        for b in range(kopf_dim):
            akku = 0
            for a in range(kopf_dim):
                akku += S[a][b] * k_t[a]
            kv_mem[b] = rshift_round(akku, s_frac)

        delta = [rshift_round((v_t[b] - kv_mem[b]) * beta_t, beta_frac)
                 for b in range(kopf_dim)]

        for a in range(kopf_dim):
            k_a = k_t[a]
            if k_a == 0:
                continue
            zeile = S[a]
            for b in range(kopf_dim):
                prod = k_a * delta[b]
                if update_shift > 0:
                    zeile[b] += rshift_round(prod, update_shift)
                else:
                    zeile[b] += prod << (-update_shift)

        aus = [0] * kopf_dim
        for b in range(kopf_dim):
            akku = 0
            for a in range(kopf_dim):
                akku += S[a][b] * q_t[a]
            aus[b] = rshift_round(akku, s_frac)
        ausgaben.append(aus)
    return ausgaben


# ------------------------------------------------------------ Eingaben
def eingaben_bauen(n, kopf_dim, frac, zerfall_frac, beta_frac, zerfall, saat):
    """Synthetische Folge mit festem Zufallskeim, damit sie wiederholbar ist."""
    r = random.Random(saat)
    spanne = 1 << (frac - 1)
    q = [[r.randint(-spanne, spanne) for _ in range(kopf_dim)] for _ in range(n)]
    k = [[r.randint(-spanne, spanne) for _ in range(kopf_dim)] for _ in range(n)]
    v = [[r.randint(-spanne, spanne) for _ in range(kopf_dim)] for _ in range(n)]
    g_int = [round(zerfall * (1 << zerfall_frac))] * n
    beta_int = [r.randint(1 << (beta_frac - 2), (1 << beta_frac) - 1) for _ in range(n)]
    return q, k, v, g_int, beta_int


def a_spannen_aus_artefakt(artefakt: Path):
    """Je Ebene der **gemessene** Betragsgrosstwert des `in_proj_a`-Ausgangs.

    ⚑ **Das schliesst die Luecke, die diese Datei bis zum 2026-09-21
    offen nannte.** `a` kommt zur Laufzeit aus einer Projektion, war
    also ohne Vorwaertspass nicht bekannt. `scales.json` eines gebauten
    Artefakts haelt genau diesen Wert, gesammelt ueber den ganzen
    Kalibrierkorpus: `stats.py` hakt den **Ausgang** ein
    (`take_input=False`), und eine Skala ist der beobachtete
    Betragsgrosstwert.
    """
    datei = artefakt / "scales.json"
    if not datei.is_file():
        return None
    skalen = json.loads(datei.read_text(encoding="utf-8"))
    spannen = {}
    for name, eintrag in skalen.items():
        if name.endswith(".linear_attn.in_proj_a"):
            ebene = int(name.split(".layers.")[1].split(".")[0])
            spannen[ebene] = float(eintrag["absmax_observed"])
    return spannen or None


def zerfaelle_aus_gewichten(pfad: Path, a_spannen=None):
    """Die echten Zerfaelle je Kopf, aus `A_log`, `dt_bias` und `a`.

    ⚑ `g = exp(-exp(A_log) * softplus(a + dt_bias))` ist der Faktor je
    Token.

    ⚑ **Mit `a_spannen` ist das eine Messung, ohne sie eine Schranke.**
    Liegen die gemessenen Spannen vor, laeuft `a` ueber
    `[-|a|max, 0, +|a|max]` **jeder** Zustandsebene; sonst bleibt es bei
    `a = 0` und der ersten Ebene, und das Ergebnis sagt dann nur etwas
    ueber die Groessenordnung.
    """
    idx = pfad / "model.safetensors.index.json"
    if not idx.is_file():
        return None, f"{idx.name} fehlt unter {pfad}"
    # ⚠️ **Die Tensoren sind `bfloat16`, und das versteht numpy nicht.**
    # Ein erster Entwurf las mit `framework="numpy"` und brach mit
    # `data type 'bfloat16' not understood` ab. Gelesen wird deshalb ueber
    # torch, das es kennt, und danach auf `float64` gehoben.
    try:
        import torch
        from safetensors import safe_open
    except ImportError as f:
        return None, f"torch oder safetensors fehlt ({f}); nur in der Kalibrierumgebung"
    karte = json.loads(idx.read_text(encoding="utf-8"))["weight_map"]
    a_namen = sorted(n for n in karte if n.endswith("linear_attn.A_log"))
    dt_namen = sorted(n for n in karte if n.endswith("linear_attn.dt_bias"))
    if not a_namen:
        return None, "keine A_log-Tensoren im Index"
    if a_spannen is None:
        a_namen = a_namen[:1]       # die alte Schranke: erste Ebene, a = 0

    lo, hi, summe, zahl = 1.0, 0.0, 0.0, 0
    gelesen = []
    for name in a_namen:
        datei = pfad / karte[name]
        if not datei.is_file():
            return None, f"{karte[name]} fehlt noch (Download nicht durch?)"
        ebene = int(name.split(".layers.")[1].split(".")[0])
        with safe_open(str(datei), framework="pt") as f:
            a = f.get_tensor(name).to(torch.float64)
        dt_name = name.replace("A_log", "dt_bias")
        bias = torch.zeros_like(a)
        if dt_name in karte and (pfad / karte[dt_name]).is_file():
            with safe_open(str(pfad / karte[dt_name]), framework="pt") as f2:
                bias = f2.get_tensor(dt_name).to(torch.float64)
        # ⚑ Die Extreme liegen an den Raendern, denn softplus ist monoton.
        spanne = a_spannen.get(ebene, 0.0) if a_spannen else 0.0
        for versatz in ({-spanne, 0.0, spanne} if spanne else {0.0}):
            sp = torch.nn.functional.softplus(bias + versatz)
            faktor = torch.exp(-torch.exp(a) * sp)
            lo = min(lo, float(faktor.min()))
            hi = max(hi, float(faktor.max()))
            summe += float(faktor.mean())
            zahl += 1
        gelesen.append(ebene)

    quelle = (f"{len(gelesen)} Zustandsebenen, a aus der Messung"
              if a_spannen else f"{a_namen[0]} (a = 0, nur eine Ebene)")
    return (lo, summe / zahl, hi), quelle


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--gewichte", type=Path, default=REPO / "MODELS/llm/Qwen3.6-35B-A3B")
    p.add_argument("--kopf-dim", type=int, default=32,
                   help="echte Groesse ist 128; kleiner gehalten, weil die "
                        "Laufzeit kubisch waechst")
    p.add_argument("--lang", action="store_true", help="bis 4000 Token statt 400")
    p.add_argument("--artefakt", type=Path, default=None,
                   help="gebautes Artefakt; liefert die GEMESSENE a-Spanne "
                        "aus scales.json statt der Schranke bei a = 0")
    args = p.parse_args()

    FRAC, ZERFALL_FRAC, BETA_FRAC = 8, 16, 8
    S_REF = 48          # die breite Vergleichsfassung
    S_KANDIDATEN = [8, 12, 16, 20, 24, 28, 32]

    print("[drift] Zustandsdrift des Gated DeltaNet")
    print(f"[drift] kopf_dim={args.kopf_dim} (echt 128), frac={FRAC}, "
          f"zerfall_frac={ZERFALL_FRAC}, beta_frac={BETA_FRAC}, Referenz s_frac={S_REF}")

    a_spannen = a_spannen_aus_artefakt(args.artefakt) if args.artefakt else None
    if args.artefakt and not a_spannen:
        print(f"[drift] ⚠️ keine a-Spannen in {args.artefakt}/scales.json")
    bereich, quelle = zerfaelle_aus_gewichten(args.gewichte, a_spannen)
    if bereich:
        lo, mit, hi = bereich
        print(f"[drift] echte Zerfaelle aus {quelle}: "
              f"min={lo:.6f} mittel={mit:.6f} max={hi:.6f}")
        # ⛔️ **Hier stand bis zum 2026-09-21 `round(..., 6)`**, und das
        #    hat genau die Groesse vernichtet, um die es geht: Der
        #    langsamste gemessene Zerfall ist 0,999999998980 und wird auf
        #    sechs Stellen zu **exakt 1,0**. Damit verglich die Reihe
        #    „kein Zerfall" mit „kein Zerfall", jede Zeile wurde null,
        #    und `log2(N/(1-g))` teilte am Ende durch null.
        #    📌 **Eine Rundung in der Aufbereitung ist kein Darstellungs-
        #    detail, wenn der Messwert im Weggerundeten steckt.**
        zerfaelle = sorted({lo, mit, hi})
    else:
        print(f"[drift] ⚠️ keine echten Zerfaelle ({quelle});")
        print("[drift]    Kehrwertfeld ueber plausible Werte, also eine Aussage")
        print("[drift]    ueber die Groessenordnung und nicht ueber dieses Modell")
        zerfaelle = [0.5, 0.9, 0.99, 0.999]

    laengen = [1, 10, 100, 400] + ([4000] if args.lang else [])

    # s_frac -> groesster Abstand bei der laengsten Folge, ueber alle Zerfaelle
    schlimmstes = {s: 0 for s in S_KANDIDATEN}
    for g in zerfaelle:
        schranke = 1.0 / (1.0 - g) if g < 1.0 else float("inf")
        print()
        print(f"--- Zerfall g = {g:.6f}, theoretische Schranke 1/(1-g) = {schranke:.1f}")
        print(f"{'s_frac':>8} " + "".join(f"{n:>12}" for n in laengen))
        print("-" * (9 + 12 * len(laengen)))
        for s_frac in S_KANDIDATEN:
            zeile = f"{s_frac:>8} "
            werte = []
            for n in laengen:
                q, k, v, g_int, beta_int = eingaben_bauen(
                    n, args.kopf_dim, FRAC, ZERFALL_FRAC, BETA_FRAC, g, saat=20260921)
                gemein = dict(kopf_dim=args.kopf_dim, frac=FRAC,
                              zerfall_frac=ZERFALL_FRAC, beta_frac=BETA_FRAC)
                kand = lauf(q, k, v, g_int, beta_int, s_frac=s_frac, **gemein)
                ref = lauf(q, k, v, g_int, beta_int, s_frac=S_REF, **gemein)
                # Groesster absoluter Abstand, gemessen in Einheiten der
                # letzten Stelle von FRAC. ⚑ Absolut und nicht relativ:
                # Eine Ausgabe nahe null macht jeden relativen Wert
                # beliebig gross und sagt nichts ueber die Guete.
                schlimmster = max(
                    (abs(a - b) for za, zb in zip(kand, ref) for a, b in zip(za, zb)),
                    default=0)
                werte.append(schlimmster)
                zeile += f"{schlimmster:>12}"
            print(zeile)
            schlimmstes[s_frac] = max(schlimmstes[s_frac], werte[-1])

    print()
    print("Werte sind der groesste absolute Abstand zur breiten Fassung,")
    print(f"in Einheiten der letzten Stelle (2^-{FRAC}). 0 heisst bitgleich.")
    print()
    print("=== Die Antwort auf die Entwurfsfrage: wieviele Bits braucht der Zustand?")
    print()
    print(f"{'s_frac':>8} {'Abstand bei ' + str(laengen[-1]) + ' Token':>26}  Urteil")
    print("-" * 56)
    empfehlung = None
    for s in S_KANDIDATEN:
        w = schlimmstes[s]
        if w == 0:
            urteil = "bitgleich bei jedem Zerfall"
            if empfehlung is None:
                empfehlung = s
        elif w <= 2:
            urteil = "hoechstens 2 Stellen, tragbar"
        else:
            urteil = "zu knapp"
        print(f"{s:>8} {w:>26}  {urteil}")
    print()
    if empfehlung is not None:
        # ⚑ Jede Zahl hier wird gerechnet und keine getippt: Eine
        # getippte Zahl neben einem gemessenen Befund ist die zweite
        # Wahrheit, und sie veraltet beim naechsten Lauf.
        vorkomma32 = 32 - empfehlung - 1      # ein Bit fuers Vorzeichen
        print(f"[drift] ⚑ BEFUND: **{empfehlung} Bruchbits** fuer den Zustand, also das")
        print(f"        {empfehlung / FRAC:.1f}-fache der Wertauflösung. Dort ist der Abstand zur")
        print("        breiten Fassung bei jedem gemessenen Zerfall und jeder Laenge")
        print("        **null**, und das heisst: der Abstand saettigt, er waechst nicht")
        print("        mit der Folge.")
        print()
        print("        ⚑ **Warum das so ist, steht in der Schieberechnung im Modulkopf.**")
        print("        Der Zerfall unter eins erzeugt je Token einen Rundungsfehler und")
        print("        **daempft jeden aelteren mit demselben Faktor**. Die Summe laeuft")
        print("        damit in ein Gleichgewicht statt linear zu wachsen, und die")
        print("        Schranke haengt an 1/(1-g), nicht an der Laenge.")
        print()
        print("        ⛔️ **Damit traegt der Ganzzahlpfad eine rekurrente")
        print("        Zustandsschicht.** Das ist die Antwort auf die Entwurfsfrage.")
        print()
        print("        ⚠️ **Und die Breite ist die eigentliche Entscheidung, denn sie")
        print(f"        ist knapp:** {empfehlung} Bruchbits lassen in int32 nur noch")
        print(f"        **{vorkomma32} Bit Vorkomma**. Zwei Wege:")
        knapper = [s for s in S_KANDIDATEN if schlimmstes[s] <= 2 and s < empfehlung]
        if knapper:
            s = max(knapper)
            print(f"          a) **{s} Bit und {schlimmstes[s]} Stelle Abweichung hinnehmen.**")
            print(f"             Laesst {32 - s - 1} Bit Vorkomma, also Luft, und der Abstand")
            print("             saettigt dort ebenfalls; er ist nur nicht null.")
        print(f"          b) **{empfehlung} Bit und den Zustand in int64 fuehren.** Bitgleich")
        print("             zur breiten Fassung, kostet aber die doppelte Zustandsgroesse.")
        print()
        print("        ⚑ **Diese Wahl gehoert dem Projektinhaber**, denn sie handelt")
        print("        Speicher gegen Genauigkeit, und der Zustand geht in die Spur ein.")
    else:
        print("[drift] ⛔️ BEFUND: keine der gemessenen Breiten wird bitgleich. Vor")
        print("        einer Festlegung gehoert die Reihe nach oben verlaengert.")
    # ------------------------------------------------------------------
    # ⛔️ **Die zweite Frage, und die Literatur haelt sie fuer die
    # wichtigere.** Zwei Arbeiten zur Quantisierung dieser Bauart betonen
    # nicht die Zustandsbreite, sondern die Aufloesung des **Zerfalls**:
    # Er gehe als Produkt ueber die ganze Folge ein, also stoere ein
    # kleiner Fehler darin die langen Produkte.
    #
    # ⚑ **Uebersetzt in dieses Schema heisst das:** Wird `g` mit zu
    # wenigen Bits dargestellt, rechnet der Pfad mit `(g+eps)^N` statt
    # `g^N`, und das laeuft mit N auseinander. Das ist eine andere Frage
    # als die Zustandsbreite, und sie ist hier zu messen statt zu glauben.
    print()
    print("=== Die zweite Frage: wieviele Bits braucht der ZERFALL?")
    print()

    # ⛔️ **Der Pruefpunkt ist hier die ganze Kunst, und er war falsch.**
    #
    # Bis zum 2026-09-21 lief diese Reihe auf `max(zerfaelle)`, also auf
    # dem Zerfall, der am naechsten an eins liegt. Das klingt nach dem
    # haertesten Fall und ist der **harmloseste**: Bei g = 1 - 1e-9
    # zerfaellt ueber das ganze Fenster nichts Messbares, und jede
    # Darstellung rundet g auf glatt eins. Die Tabelle wurde dadurch
    # durchgehend null, und das sah aus wie ein Freibrief.
    #
    # ⚑ **Der wirklich enge Punkt liegt in der Mitte.** Ein Fehler eps in
    # g wirkt sich auf das Produkt ueber die Folge mit etwa
    # `eps * min(N, 1/(1-g))` aus: Eine Folge kann nur so weit
    # zurueckwirken, wie sie **lang** ist ODER wie weit das Gedaechtnis
    # reicht, und nicht beides multipliziert. Beide Faktoren sind bei
    # `g = 1 - 1/N` gleich gross, und dort ist der Bedarf am hoechsten.
    #
    # 📌 **Das alte `log2(N/(1-g))` hat beide multipliziert statt ihr
    # Minimum zu nehmen**, und kam deshalb auf 35 Bit statt auf 26.
    s_fest = empfehlung if empfehlung else max(S_KANDIDATEN)
    z_kandidaten = [8, 12, 16, 20, 24, 26, 28, 32, 36]
    # ⚠️ Die Referenz MUSS breiter sein als jeder Kandidat, sonst misst
    #    die Reihe den Fehler der Referenz.
    z_ref = 48
    assert z_ref > max(z_kandidaten), "Referenz nicht breiter als der breiteste Kandidat"

    # Die gemessenen Zerfaelle, dazu je Laenge der kritische Punkt.
    pruefpunkte = sorted({g for g in zerfaelle if 0.0 < g < 1.0} |
                         {1.0 - 1.0 / n for n in laengen if n > 1})
    print(f"    Zustand fest auf {s_fest} Bit, Referenz zerfall_frac={z_ref}")
    print(f"    {len(pruefpunkte)} Pruefpunkte: die gemessenen Zerfaelle und")
    print("    je Laenge der kritische Punkt g = 1 - 1/N")
    print()

    print(f"{'zerfall_frac':>13} " + "".join(f"{n:>12}" for n in laengen))
    print("-" * (14 + 12 * len(laengen)))
    z_empfehlung = None
    for zf in z_kandidaten:
        zeile = f"{zf:>13} "
        schlimmster = 0
        for n in laengen:
            w_max = 0
            for g in pruefpunkte:
                q, k, v, _, beta_int = eingaben_bauen(
                    n, args.kopf_dim, FRAC, zf, BETA_FRAC, g, saat=20260921)
                # ⚑ Beide Seiten bekommen denselben Zerfall, nur
                #   verschieden fein dargestellt.
                g_kand = [round(g * (1 << zf))] * n
                g_ref = [round(g * (1 << z_ref))] * n
                kand = lauf(q, k, v, g_kand, beta_int, kopf_dim=args.kopf_dim,
                            frac=FRAC, s_frac=s_fest, zerfall_frac=zf,
                            beta_frac=BETA_FRAC)
                ref = lauf(q, k, v, g_ref, beta_int, kopf_dim=args.kopf_dim,
                           frac=FRAC, s_frac=s_fest, zerfall_frac=z_ref,
                           beta_frac=BETA_FRAC)
                # ⛔️ `zip` bricht an der kuerzeren Seite ab, ohne ein Wort
                #    zu sagen (Fund 346). Hier sind beide gleich lang, und
                #    genau das wird geprueft statt angenommen.
                assert len(kand) == len(ref), "Kandidat und Referenz verschieden lang"
                for za, zb in zip(kand, ref):
                    assert len(za) == len(zb), "Zeilen verschieden lang"
                    for a, b in zip(za, zb):
                        w_max = max(w_max, abs(a - b))
            zeile += f"{w_max:>12}"
            schlimmster = max(schlimmster, w_max)
        print(zeile)
        if schlimmster == 0 and z_empfehlung is None:
            z_empfehlung = zf

    print()
    print("Werte sind der groesste absolute Abstand ueber ALLE Pruefpunkte,")
    print(f"in Einheiten der letzten Stelle (2^-{FRAC}). 0 heisst bitgleich.")
    print()

    # ⛔️ **Das Urteil wird aus der Tabelle gelesen, nicht danebengeschrieben.**
    # Bis zum 2026-09-21 stand hier fester Text, der behauptete, bei 24 Bit
    # blieben Abweichungen, waehrend die Tabelle darueber nur Nullen zeigte.
    # 📌 **Ein Urteil, das seine eigene Messung nicht liest, ist keines.**
    if z_empfehlung is None:
        print(f"[drift] ⚠️ BEFUND: selbst {max(z_kandidaten)} Bruchbits reichen")
        print("        bei den gemessenen Laengen nicht. Der Zerfall ist damit")
        print("        die engere Stelle, und die Reihe gehoert verlaengert.")
    else:
        print(f"[drift] ⚑ BEFUND: **{z_empfehlung} Bruchbits** fuer den Zerfall.")
        print("        Dort ist der Abstand zur breiten Fassung bei jedem")
        print("        Pruefpunkt und jeder gemessenen Laenge null.")
    print()

    # ⚑ Die Hochrechnung auf Kontexte, die hier nicht zu rechnen sind.
    print("        Hochgerechnet auf laengere Kontexte, mit")
    print("        eps * min(N, 1/(1-g)) < 2^-%d:" % FRAC)
    for kontext in (laengen[-1], 40960, 262144):
        # Der kritische Punkt ist g = 1 - 1/N, dort sind beide Faktoren N.
        noetig = math.log2(kontext) + FRAC - 1
        print(f"        Kontext {kontext:>7}: {math.ceil(noetig):>3} Bit")
    print()
    # ⛔️ **Hier stand die Zahl als Text**, und sie lief gegen die Tabelle
    #    darueber auseinander: 25 gerechnet, 26 getippt.
    #    📌 **Was an zwei Orten steht, laeuft auseinander**, auch wenn die
    #    beiden Orten nur zwanzig Zeilen auseinanderliegen.
    volles_fenster = 262144
    noetig_voll = math.ceil(math.log2(volles_fenster) + FRAC - 1)
    print("        ⚑ Der Bedarf waechst mit dem **Logarithmus** der")
    print(f"        Kontextlaenge. Fuer das volle Fenster dieses Modells")
    print(f"        ({volles_fenster}) sind es {noetig_voll} Bit.")
    print()

    return 0


if __name__ == "__main__":
    sys.exit(main())
