#!/usr/bin/env python3
"""
TRAINING 0.1: Traegt das Quantisierungsschema im RUECKWAERTSPASS?

Whitepaper Kap. 7 setzt voraus, dass die ganzzahlige Ausfuehrung aus
Kap. 6 unveraendert auf den Rueckwaertspass uebertraegt. Das ist eine
ANNAHME, keine Messung, und sie traegt den gesamten TRAINING-Fahrplan:
Ohne bit-exakten ganzzahligen Rueckwaertspass gibt es keine
verifizierbare Trainingsarbeit, ohne die keine Verguetung, ohne die kein
Modellwachstum.

Es gibt einen konkreten Grund fuer Zweifel, und er kommt aus der eigenen
Erfahrung des Projekts: Fund 20/24 (Massive Activations). Der
Residualstrom brauchte eine Skala JE KANAL, weil einzelne Kanaele um
Groessenordnungen aus der Verteilung ragen; eine Skala je Tensor loeschte
die feinskalierten Kanaele aus. Gradienten haben typischerweise einen
groesseren Dynamikbereich als Aktivierungen und ueber die Schritte hinweg
einen wandernden.

## Was hier gemessen wird

1. DYNAMIKBEREICH der Gradienten je Ebene und Schritt. Die Zahl, die
   entscheidet, ob eine Skala je Block genuegt oder eine je Kanal noetig
   ist. Sie wird erhoben, BEVOR irgendetwas entworfen wird.
2. VERLUSTVERLAUF zweier Laeufe auf identischen Batches in identischer
   Reihenfolge.
3. ANTEIL GESAETTIGTER Gradienten je Ebene. Die Groesse, die bei Fund 23
   den Ausschlag gab: Stilles Clippen tarnt sich als "konvergiert halt
   langsamer".

## Methodik

Fake-Quant in PyTorch, nicht Rust. Das ist die Lehre aus der
7B-Fehlersuche: Zwei Simulationen haben dort in Stunden entschieden, was
vorher tagelang im falschen Code gesucht wurde. Sie trennen die Frage
"traegt das VERFAHREN?" von "ist unsere UMSETZUNG richtig?", und nur die
erste steht hier an.

Gerechnet wird weiter in Gleitkomma, aber auf das Ganzzahlraster
gezwungen:

    Gewichte      int8, symmetrisch, eine Zweierpotenz-Skala je
                  Ausgabezeile (wie calibrate/src/quantize.py)
    Aktivierungen int16, Zweierpotenz-Skala je Kanal (wie theta_v 0.11.0)
    Gradienten    int8, Zweierpotenz-Skala je BLOCK (Anhang B.6.2, NITI)

## Abweichung von der Fahrplanvorgabe, ausdruecklich

Der Fahrplan nennt als Referenz "BF16, unveraendertes Training". Gemessen
wird hier in float32. Grund: Ein Trainingslauf in bf16 bringt seine
eigene Rundung mit, und die waere ein zweiter Einflussfaktor neben dem,
der untersucht werden soll. Die Referenz soll so sauber wie moeglich
sein; der Abstand zwischen den Armen ist die Messgroesse, nicht der
Absolutwert.

Ebenfalls bewusst: SGD ohne Momentum, kein Adam. Adam normiert die
Gradientengroesse je Parameter und wuerde genau den Effekt verdecken, um
den es hier geht.

Gleitkomma erlaubt: Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage:
    cd INTEGER_LLM/calibrate
    .venv/bin/python ../../TRAINING/tests/diag/backward_reference_simulation.py
    .venv/bin/python ../../TRAINING/tests/diag/backward_reference_simulation.py --schritte 30
"""
import argparse
import json
import math
import statistics
import sys
import time
from datetime import date
from pathlib import Path

WURZEL = Path(__file__).resolve().parent.parent.parent.parent
INTEGER_LLM = WURZEL / "INTEGER_LLM"
sys.path.insert(0, str(INTEGER_LLM / "calibrate"))
sys.path.insert(0, str(INTEGER_LLM / "eval"))

import torch  # noqa: E402
import torch.nn as nn  # noqa: E402

from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, MODEL_NAME, select_sequences  # noqa: E402

ERGEBNISSE = Path(__file__).resolve().parent / "results"

# Identisch zu calibrate/src/quantize.py: dieselbe Obergrenze fuer den
# Shift, damit die Simulation dasselbe Raster trifft wie die Pipeline.
MAX_FRAC_BITS = 20
INT8_MAX = 127
INT16_MAX = 32767


# ---------------------------------------------------------------------------
# Fake-Quant: Gleitkomma auf das Ganzzahlraster zwingen
# ---------------------------------------------------------------------------

def _shift(absmax: torch.Tensor, qmax: int) -> torch.Tensor:
    """Zweierpotenz-Shift wie in der Kalibrierung.

    `floor(log2(qmax / absmax))`, begrenzt auf [0, MAX_FRAC_BITS]. Die
    Multiplikation mit einer Zweierpotenz ist in IEEE-Gleitkomma exakt,
    also fuegt der Fake-Quant keinen eigenen Fehler hinzu, den es im
    Ganzzahlpfad nicht gaebe.
    """
    return torch.floor(torch.log2(qmax / absmax.clamp(min=1e-30))).clamp(0, MAX_FRAC_BITS)


# Umschaltbar, weil genau hier die Entscheidung faellt (siehe unten).
STOCHASTISCH = False


def fq_gewichte_int8(W: torch.Tensor) -> torch.Tensor:
    """int8, symmetrisch, eine Skala je Ausgabezeile.

    ## Warum die Rundungsart alles entscheidet

    Gemessen (2026-08-22): Ein SGD-Schritt bewegt ein Gewicht um **0,03 %
    einer Rasterstufe**. Mit Rundung zur naechsten Stufe passiert dann
    entweder nichts oder ein ganzer Sprung, also eine Ueberschreitung um
    das Dreitausendfache. Das Training bricht.

    Mit **stochastischem Runden** wird die Stufe mit einer
    Wahrscheinlichkeit gleich dem Nachkommaanteil genommen. Der
    Erwartungswert ist dann der Gleitkommawert, die Aktualisierung ist
    unverzerrt, und eine winzige Aenderung des Masters aendert die
    erwartete Ausgabe sofort statt erst nach dreitausend Schritten.

    Das ist kein Einfall dieses Projekts, sondern der Befund von Gupta et
    al. 2015 (Deep Learning with Limited Numerical Precision): Bei
    16-Bit-Festkomma scheitert Rundung zur naechsten Stufe und
    funktioniert stochastisches Runden.

    ## Und der Zufall kostet keinen Determinismus

    Fuer ein Netz mit Bitgleichheits-Konsens waere echter Zufall im
    Rechenpfad ausgeschlossen. Zufall aus einem **Keim** ist aber kein
    Zufall im Sinne der Nachrechenbarkeit, sondern eine Funktion des
    Keims. Nachgemessen: Zwei Laeufe mit demselben Keim liefern
    identische Werte bis auf die letzte Stelle, zwei Keime verschiedene.

    In einer spaeteren Umsetzung waere der Wuerfel kein RNG-Zustand,
    sondern eine Funktion aus (Ebene, Schritt, Index) ueber
    `kernels/src/prng.rs::splitmix64`: zaehlerbasiert, ohne Zustand, auf
    jeder Maschine gleich.
    """
    absmax = W.abs().amax(dim=1, keepdim=True)
    s = torch.pow(2.0, _shift(absmax, INT8_MAX))
    skaliert = W * s
    if not STOCHASTISCH:
        gerundet = torch.round(skaliert)
    else:
        unten = torch.floor(skaliert)
        rest = skaliert - unten
        gerundet = unten + (torch.rand_like(rest) < rest).to(skaliert.dtype)
    return torch.clamp(gerundet, -INT8_MAX - 1, INT8_MAX) / s


def fq_aktivierung_int16(x: torch.Tensor) -> torch.Tensor:
    """int16, eine Skala je Kanal (letzte Dimension)."""
    dims = tuple(range(x.dim() - 1))
    absmax = x.abs().amax(dim=dims, keepdim=True)
    s = torch.pow(2.0, _shift(absmax, INT16_MAX))
    return torch.clamp(torch.round(x * s), -INT16_MAX - 1, INT16_MAX) / s


def fq_gradient_block(g: torch.Tensor, blockgroesse: int, bits: int = 8):
    """int8 mit einer Zweierpotenz-Skala je BLOCK (Anhang B.6.2, NITI).

    Blockweise ueber die letzte Dimension. Liefert neben dem gerasterten
    Gradienten die beiden Kennzahlen, um die es in dieser Simulation geht:
    den Dynamikbereich und den Saettigungsanteil.
    """
    form = g.shape
    flach = g.reshape(-1, form[-1])
    n = flach.shape[-1]
    # Rest auffuellen, damit jeder Block gleich gross ist: sonst haette
    # der letzte Block eine andere Skala und die Messung eine Unwucht.
    fuellung = (-n) % blockgroesse
    if fuellung:
        flach = torch.cat([flach, torch.zeros(flach.shape[0], fuellung, device=g.device, dtype=g.dtype)], dim=1)
    bloecke = flach.reshape(flach.shape[0], -1, blockgroesse)

    qmax = (1 << (bits - 1)) - 1
    absmax = bloecke.abs().amax(dim=-1, keepdim=True)
    s = torch.pow(2.0, _shift(absmax, qmax))
    roh = torch.round(bloecke * s)
    q = torch.clamp(roh, -qmax - 1, qmax)

    gesaettigt = (roh.abs() > qmax).sum().item()
    gesamt = roh.numel()

    zurueck = (q / s).reshape(flach.shape)[:, :n].reshape(form)

    # Dynamikbereich ueber die von null verschiedenen Betraege: das
    # Verhaeltnis von groesstem zu kleinstem, in Bits.
    betrag = g.abs()
    nichtnull = betrag[betrag > 0]
    if nichtnull.numel() == 0:
        spanne_bits = 0.0
    else:
        spanne_bits = float(torch.log2(nichtnull.max() / nichtnull.min()).item())

    # Wie viele Werte verschwinden im Raster, werden also zu null,
    # obwohl sie es nicht waren? Das ist die stille Haelfte des
    # Problems: Saettigung oben, Ausloeschung unten.
    ausgeloescht = int(((betrag > 0) & (zurueck.abs() == 0)).sum().item())

    # **Die Zerlegung, die ueber die Eskalation entscheidet.**
    #
    # Der Fahrplan verlangt, die Reihenfolge der Kandidaten aus dieser
    # Messung abzuleiten und nicht zu vermuten. Die Frage lautet: Liegt
    # die Spanne ZWISCHEN den Bloecken oder INNERHALB eines Blocks?
    #
    #   zwischen  -> eine feinere Skalengranularitaet (je Kanal statt je
    #                Block) hilft, denn jeder Block bekaeme seinen eigenen
    #                passenden Massstab
    #   innerhalb -> sie hilft NICHT; dann fehlen Bits, und es braucht
    #                eine breitere Wortbreite oder Fehlerrueckkopplung
    #
    # `innerhalb` ist der Median ueber die Bloecke, damit einzelne
    # entartete Bloecke die Aussage nicht tragen.
    blockmax = bloecke.abs().amax(dim=-1)
    positiv = blockmax[blockmax > 0]
    zwischen_bits = float(torch.log2(positiv.max() / positiv.min()).item()) if positiv.numel() else 0.0

    b_abs = bloecke.abs()
    b_max = b_abs.amax(dim=-1)
    b_min = torch.where(b_abs > 0, b_abs, torch.full_like(b_abs, float("inf"))).amin(dim=-1)
    gueltig = torch.isfinite(b_min) & (b_min > 0) & (b_max > 0)
    if gueltig.any():
        innerhalb_bits = float(torch.log2(b_max[gueltig] / b_min[gueltig]).median().item())
    else:
        innerhalb_bits = 0.0

    return zurueck, {
        "spanne_bits": spanne_bits,
        "zwischen_bloecken_bits": zwischen_bits,
        "innerhalb_block_bits": innerhalb_bits,
        "saettigung": gesaettigt / max(gesamt, 1),
        "ausloeschung": ausgeloescht / max(g.numel(), 1),
    }


# ---------------------------------------------------------------------------
# Die quantisierte lineare Schicht
# ---------------------------------------------------------------------------

class Statistik:
    """Sammelt je Ebene und Schritt, was gemessen werden soll."""

    def __init__(self):
        self.schritt = 0
        self.werte = {}   # name -> list[dict]

    def erfassen(self, name: str, info: dict):
        self.werte.setdefault(name, []).append({"schritt": self.schritt, **info})


class QuantLinearFn(torch.autograd.Function):
    """Vorwaerts wie eine lineare Schicht, aber alles auf dem Raster.

    Der Rueckwaertspass rastert den EINGEHENDEN Gradienten (den
    Fehlervektor) und rechnet mit ihm weiter: genau das ist die Frage
    dieser Simulation. Gewichts- und Eingabegradient entstehen dann aus
    gerasterten Groessen.
    """

    @staticmethod
    def forward(ctx, x, W, b, stat, name, blockgroesse, bits, teile):
        # `teile` schaltet die drei Quantisierungen einzeln. Ohne diese
        # Trennung liesse sich nur feststellen, DASS das Verfahren nicht
        # traegt, nicht WORAN es liegt, und der Fahrplan verlangt die
        # Eskalationsreihenfolge aus der Messung statt aus der Vermutung.
        xq = fq_aktivierung_int16(x) if "a" in teile else x
        Wq = fq_gewichte_int8(W) if "w" in teile else W
        ctx.save_for_backward(xq, Wq)
        ctx.stat = stat
        ctx.name = name
        ctx.blockgroesse = blockgroesse
        ctx.bits = bits
        ctx.teile = teile
        ctx.hat_bias = b is not None
        out = xq @ Wq.t()
        if b is not None:
            out = out + b
        return out

    @staticmethod
    def backward(ctx, g):
        xq, Wq = ctx.saved_tensors
        if "g" in ctx.teile:
            gq, info = fq_gradient_block(g, ctx.blockgroesse, ctx.bits)
            ctx.stat.erfassen(ctx.name, info)
        else:
            gq = g

        gx = gq @ Wq
        gW = gq.reshape(-1, gq.shape[-1]).t() @ xq.reshape(-1, xq.shape[-1])
        gb = gq.reshape(-1, gq.shape[-1]).sum(0) if ctx.hat_bias else None
        return gx, gW, gb, None, None, None, None, None


class QuantLinear(nn.Module):
    def __init__(self, lin: nn.Linear, name: str, stat: Statistik, blockgroesse: int,
                 bits: int, teile: str):
        super().__init__()
        self.weight = lin.weight
        self.bias = lin.bias
        self.name = name
        self.stat = stat
        self.blockgroesse = blockgroesse
        self.bits = bits
        self.teile = teile

    def forward(self, x):
        return QuantLinearFn.apply(
            x, self.weight, self.bias, self.stat, self.name, self.blockgroesse,
            self.bits, self.teile)


def linears_ersetzen(model, stat: Statistik, blockgroesse: int, bits: int = 8,
                     teile: str = "wag") -> int:
    """Ersetzt jede lineare Schicht im Transformer durch die gerasterte.

    Der LM-Head bleibt aussen vor: Er ist im Inferenzpfad int16 statt int8
    (theta_v 0.10.0, Weight-Tying aufgeloest), und ihn hier wie die
    uebrigen zu behandeln, hiesse ein anderes Schema zu messen als das
    unsere.
    """
    ersetzt = 0
    for name, modul in list(model.named_modules()):
        for kind_name, kind in list(modul.named_children()):
            if not isinstance(kind, nn.Linear):
                continue
            voll = f"{name}.{kind_name}" if name else kind_name
            if "lm_head" in voll:
                continue
            setattr(modul, kind_name, QuantLinear(kind, voll, stat, blockgroesse, bits, teile))
            ersetzt += 1
    return ersetzt


# ---------------------------------------------------------------------------
# Der Messlauf
# ---------------------------------------------------------------------------

def bewerten(model, halte, geraet: str) -> float:
    """Mittlerer Verlust auf Sequenzen, die nie trainiert wurden.

    **Die eigentliche Messgroesse**, und das war ein Fund (2026-08-22):
    Gemessen wurde zuerst der TRAININGSverlust, wie der Fahrplan es
    vorgab. Der quantisierte Arm fiel darin von 2,54 auf 0,25, also weit
    unter die Referenz, und die Auswertung meldete "traegt". Auf
    zurueckgehaltenem Text stieg sein Verlust im selben Lauf von 2,81 auf
    2,93.

    Er hat also nicht besser gelernt, sondern die zwanzig Trainingstexte
    auswendig gelernt. Das passt zur Wirkung der Gradientenquantisierung:
    Wenn kleine Betraege im Raster verschwinden und die uebrigen auf
    grobe Stufen springen, wird aus dem Gradienten etwas, das dem
    Vorzeichen naeher ist als dem Wert, und solche Verfahren bewegen sich
    schnell und lernen schlecht.

    Ein Kriterium ueber den Trainingsverlust haette diesen Fall als
    Erfolg gebucht. Deshalb steht hier die Haltemenge.
    """
    model.eval()
    summe = 0.0
    with torch.no_grad():
        for x in halte:
            summe += float(model(x, labels=x).loss.item())
    model.train()
    return summe / max(len(halte), 1)


def batches_bauen(anzahl: int, seq_len: int, geraet: str):
    """Dieselben Sequenzen in derselben Reihenfolge fuer beide Arme.

    Nur Sequenzen voller Laenge: Eine kuerzere brauchte eine Maske, und
    eine falsch maskierte Position waere ein Fehler, der wie ein Befund
    aussieht. `select_sequences` liefert das, was aus den gewaehlten
    WikiText-Zeilen herauskommt, also weniger als angefragt; deshalb wird
    grosszuegig angefragt und danach gezaehlt.
    """
    roh = select_sequences(anzahl * 3, seq_len, verbose=False)
    seqs = [s for s in roh if len(s) == seq_len][:anzahl]
    return [torch.tensor([s], device=geraet) for s in seqs]


def lauf(quantisiert: bool, batches, halte, schritte: int, lr: float, geraet: str,
         blockgroesse: int, stat: Statistik, takt: int, bits: int = 8,
         teile: str = "wag"):
    model, _ = load_reference_model(MODEL_DIR)
    model = model.to(torch.float32).to(geraet)
    model.train()
    model.config.use_cache = False

    ersetzt = 0
    if quantisiert:
        ersetzt = linears_ersetzen(model, stat, blockgroesse, bits, teile)

    opt = torch.optim.SGD(model.parameters(), lr=lr, momentum=0.0)
    verluste = []
    haltekurve = [(0, bewerten(model, halte, geraet))]
    t0 = time.time()
    for i in range(schritte):
        stat.schritt = i
        x = batches[i % len(batches)]
        out = model(x, labels=x)
        verlust = out.loss
        if not torch.isfinite(verlust):
            print(f"  Schritt {i+1}: Verlust nicht endlich, Abbruch")
            break
        verlust.backward()
        opt.step()
        opt.zero_grad(set_to_none=True)
        verluste.append(float(verlust.item()))
        if (i + 1) % takt == 0:
            h = bewerten(model, halte, geraet)
            haltekurve.append((i + 1, h))
            print(f"  Schritt {i+1:3d}/{schritte}  Training {verluste[-1]:.4f}"
                  f"  zurueckgehalten {h:.4f}  ({time.time()-t0:.0f} s)")
    dauer = time.time() - t0
    del model
    if geraet == "mps":
        torch.mps.empty_cache()
    return verluste, haltekurve, dauer, ersetzt


def faellt_monoton(verluste, fenster: int) -> bool:
    """Faellt der Verlust ueber die Messstrecke?

    Nicht Schritt fuer Schritt: Ein Trainingsverlust schwankt zwischen
    Batches, und ein einzelner Ausreisser waere kein Befund. Verglichen
    wird der Mittelwert des ersten mit dem des letzten Fensters.

    **Das Fenster ist eine volle Runde durch die Batches**, nicht eine
    feste Zahl. Sonst enthielten die beiden Fenster verschiedene
    Sequenzen, und gemessen waere zur Haelfte, welche davon schwerer
    sind. Mit einer vollen Runde stehen in beiden Fenstern dieselben
    Texte.
    """
    if len(verluste) < 2 * fenster:
        return verluste[-1] < verluste[0]
    return statistics.mean(verluste[-fenster:]) < statistics.mean(verluste[:fenster])


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--schritte", type=int, default=120)
    p.add_argument("--sequenzen", type=int, default=24)
    p.add_argument("--seq-len", type=int, default=128)
    p.add_argument("--lr", type=float, default=2e-5)
    p.add_argument("--block", type=int, default=128,
                   help="Blockgroesse der Gradienten-Skalierung (Anhang B.6.2)")
    p.add_argument("--halte", type=int, default=8,
                   help="Sequenzen, die nie trainiert werden")
    p.add_argument("--takt", type=int, default=20,
                   help="Alle wieviel Schritte auf der Haltemenge gemessen wird")
    p.add_argument("--grad-bits", type=int, default=8,
                   help="Wortbreite der Gradienten (8 wie NITI, 16 als Eskalation)")
    p.add_argument("--auch-int16", action="store_true",
                   help="Zusaetzlicher dritter Arm mit int16-Gradienten")
    p.add_argument("--stochastisch", action="store_true",
                   help="Gewichte stochastisch runden statt zur naechsten Stufe. "
                        "DIE entscheidende Stellschraube, siehe fq_gewichte_int8.")
    p.add_argument("--keim", type=int, default=1,
                   help="Keim fuer das stochastische Runden. Gleicher Keim, "
                        "gleiches Ergebnis: nachgemessen.")
    p.add_argument("--teile", default="wag",
                   help="Welche Quantisierungen der zweite Arm anwendet: w=Gewichte, "
                        "a=Aktivierungen, g=Gradienten. Leer bedeutet KEINE, also der "
                        "Kontrolllauf durch denselben Codepfad. Ohne ihn misst der "
                        "Vergleich moeglicherweise den Ersatz fuer nn.Linear statt der "
                        "Quantisierung.")
    args = p.parse_args()

    global STOCHASTISCH
    STOCHASTISCH = args.stochastisch
    # **Beide Generatoren setzen.** `torch.manual_seed` allein setzt den
    # MPS-Generator in dieser Fassung nicht; zwei Laeufe mit demselben
    # Keim wichen daraufhin voneinander ab, und das sah nach einem
    # Argument gegen stochastisches Runden aus. Es war eines gegen meine
    # Keimsetzung.
    torch.manual_seed(args.keim)
    if torch.backends.mps.is_available():
        torch.mps.manual_seed(args.keim)

    geraet = "mps" if torch.backends.mps.is_available() else "cpu"
    print(f"TRAINING 0.1 — Referenzsimulation Rueckwaertspass")
    print(f"  Modell {MODEL_NAME}, Geraet {geraet}, {args.schritte} Schritte,")
    print(f"  {args.sequenzen} Sequenzen a {args.seq_len} Token, lr {args.lr},")
    print(f"  Gradienten-Blockgroesse {args.block}\n")

    alle = batches_bauen(args.sequenzen + args.halte, args.seq_len, geraet)
    batches, halte = alle[: args.sequenzen], alle[args.sequenzen :]
    print(f"  {len(batches)} Batches zum Trainieren, {len(halte)} zurueckgehalten\n")

    print("Arm A: Gleitkomma-Referenz")
    stat_a = Statistik()
    verluste_a, halte_a, dauer_a, _ = lauf(
        False, batches, halte, args.schritte, args.lr, geraet, args.block, stat_a, args.takt)

    beschriftung = {
        "wag": "volles Schema (W8 / A16 / Gradienten int8 je Block)",
        "w": "nur Gewichte int8",
        "a": "nur Aktivierungen int16",
        "g": "nur Gradienten int8 je Block",
        "wa": "Vorwaertspfad wie in der Inferenz (W8 + A16)",
        "": "KONTROLLE: derselbe Codepfad ohne jede Quantisierung",
    }.get(args.teile, f"Teile {args.teile!r}")
    print(f"\nArm B: {beschriftung}")
    stat_b = Statistik()
    verluste_b, halte_b, dauer_b, ersetzt = lauf(
        True, batches, halte, args.schritte, args.lr, geraet, args.block, stat_b,
        args.takt, args.grad_bits, args.teile)
    print(f"  {ersetzt} lineare Schichten ersetzt")

    # ---- Eskalationsprobe: dieselbe Rechnung mit breiteren Gradienten ----
    #
    # Der Fahrplan verlangt, die Reihenfolge der Eskalationskandidaten aus
    # der Dynamikbereichsmessung abzuleiten. Wenn die Spanne INNERHALB
    # eines Blocks liegt, hilft keine feinere Skalengranularitaet, sondern
    # nur mehr Bits. Diese Probe prueft genau das, statt es zu behaupten.
    dritter = None
    if args.auch_int16:
        print("\nArm C: dasselbe mit int16-Gradienten (Eskalationsprobe)")
        stat_c = Statistik()
        verluste_c, halte_c, dauer_c, _ = lauf(
            True, batches, halte, args.schritte, args.lr, geraet, args.block, stat_c,
            args.takt, 16)
        dritter = {
            "verlust": verluste_c,
            "halte": halte_c,
            "dauer_s": dauer_c,
        }

    # ---- Auswertung ----
    fenster = len(batches)
    ende_a = statistics.mean(verluste_a[-fenster:])
    ende_b = statistics.mean(verluste_b[-fenster:])
    abstand = (ende_b - ende_a) / ende_a * 100.0

    # **Das Urteil haengt an der Haltemenge, nicht am Trainingsverlust.**
    # Siehe `bewerten`: Der Trainingsverlust des quantisierten Arms faellt
    # tiefer als der der Referenz, und trotzdem wird das Modell dabei
    # schlechter.
    h_a_start, h_a_ende = halte_a[0][1], halte_a[-1][1]
    h_b_start, h_b_ende = halte_b[0][1], halte_b[-1][1]
    halte_abstand = (h_b_ende - h_a_ende) / h_a_ende * 100.0
    referenz_lernt = h_a_ende < h_a_start
    ganzzahl_lernt = h_b_ende < h_b_start

    # Ohne Gradientenquantisierung gibt es nichts zu erfassen; die
    # Auswertung darf daran nicht scheitern, sonst laesst sich der
    # Kontrolllauf gar nicht fahren.
    spannen = []
    zwischen = []
    innerhalb = []
    saettigungen = []
    ausloeschungen = []
    je_ebene = {}
    for name, reihe in stat_b.werte.items():
        sp = [r["spanne_bits"] for r in reihe]
        zw = [r["zwischen_bloecken_bits"] for r in reihe]
        iw = [r["innerhalb_block_bits"] for r in reihe]
        sa = [r["saettigung"] for r in reihe]
        au = [r["ausloeschung"] for r in reihe]
        spannen.extend(sp)
        zwischen.extend(zw)
        innerhalb.extend(iw)
        saettigungen.extend(sa)
        ausloeschungen.extend(au)
        je_ebene[name] = {
            "spanne_bits_median": statistics.median(sp),
            "spanne_bits_max": max(sp),
            "zwischen_bloecken_bits_median": statistics.median(zw),
            "innerhalb_block_bits_median": statistics.median(iw),
            "saettigung_max": max(sa),
            "ausloeschung_median": statistics.median(au),
            "ausloeschung_max": max(au),
        }

    # **Schritt fuer Schritt vergleichbar**, weil beide Arme dieselben
    # Batches in derselben Reihenfolge gerechnet haben. Der mittlere
    # Abstand je Schritt sagt mehr als der am Ende: Er misst nicht, wo
    # zwei Laeufe nach 200 Schritten stehen, sondern wie weit sie
    # unterwegs auseinandergehen.
    paare = list(zip(verluste_a, verluste_b))
    je_schritt = [(b - a) / a * 100.0 for a, b in paare if a > 0]

    # Traegt nur, wenn der quantisierte Arm auf zurueckgehaltenem Text
    # ueberhaupt besser wird und am Ende nicht mehr als zehn Prozent
    # hinter der Referenz liegt. Beide Bedingungen sind noetig: Ein Arm,
    # der sich verschlechtert, kann trotzdem nahe an einer Referenz
    # liegen, die sich kaum bewegt hat.
    traegt = ganzzahl_lernt and halte_abstand <= 10.0

    print("\n" + "=" * 70)
    print("  ZURUECKGEHALTENER TEXT (das Urteil haengt hier)")
    print(f"    Referenz  {h_a_start:.4f} -> {h_a_ende:.4f}   ({'besser' if referenz_lernt else 'SCHLECHTER'})")
    print(f"    Ganzzahl  {h_b_start:.4f} -> {h_b_ende:.4f}   ({'besser' if ganzzahl_lernt else 'SCHLECHTER'})")
    print(f"    Abstand am Ende {halte_abstand:+.2f} %   (Kriterium <= 10 %)")
    print()
    print("  TRAININGSVERLUST (nicht das Kriterium, siehe Bericht)")
    print(f"    Referenz  {verluste_a[0]:.4f} -> {ende_a:.4f}")
    print(f"    Ganzzahl  {verluste_b[0]:.4f} -> {ende_b:.4f}   ({abstand:+.2f} %)")
    print(f"    Faellt    Referenz {faellt_monoton(verluste_a, fenster)}, "
          f"Ganzzahl {faellt_monoton(verluste_b, fenster)}")
    print()
    if not spannen:
        print("  (Gradienten nicht quantisiert, keine Gradientenstatistik)")
        spannen = zwischen = innerhalb = saettigungen = ausloeschungen = [0.0]
    print(f"  Dynamikbereich der Gradienten (Bits): Median {statistics.median(spannen):.1f}, "
          f"Max {max(spannen):.1f}")
    print(f"    davon zwischen den Bloecken  Median {statistics.median(zwischen):.1f} Bits")
    print(f"    davon innerhalb eines Blocks Median {statistics.median(innerhalb):.1f} Bits")
    print(f"    int{args.grad_bits} deckt {args.grad_bits - 1} Bits ab")
    # Saettigung kann bei Skalen aus dem Block-Absmax strukturell kaum
    # auftreten: Der Shift wird ja so gewaehlt, dass der groesste Wert
    # gerade hineinpasst. Uebrig bleibt der Rundungsrand. Die Zahl steht
    # trotzdem da, weil der Fahrplan sie verlangt und weil ein Wert
    # ueber null hiesse, dass die Skalenwahl nicht taete, was sie soll.
    print(f"  Saettigung int8:  Median {statistics.median(saettigungen)*100:.4f} %, "
          f"Max {max(saettigungen)*100:.4f} %  (strukturell nahe null, siehe Bericht)")
    print(f"  Ausloeschung:     Median {statistics.median(ausloeschungen)*100:.2f} %, "
          f"Max {max(ausloeschungen)*100:.2f} %")
    print()
    print(f"  ERGEBNIS: {'TRAEGT' if traegt else 'TRAEGT NICHT'}")
    print("=" * 70)

    ERGEBNISSE.mkdir(parents=True, exist_ok=True)
    # **Der Dateiname traegt die Einstellungen.** Ohne das ueberschreibt
    # eine Kurzprobe das Ergebnis eines langen Laufs, und genau das ist
    # beim Bauen dieses Skripts passiert: Ein 20-Schritte-Kontrolllauf
    # hat die 200-Schritte-Messung ersetzt, ohne zu fragen.
    runden = "sr" if args.stochastisch else "rtn"
    kennung = (f"{args.teile or 'kontrolle'}-{runden}-int{args.grad_bits}"
               f"-{args.schritte}s-lr{args.lr:g}")
    ziel = ERGEBNISSE / f"backward_simulation_{MODEL_NAME.replace('.', '')}_{kennung}.json"
    ziel.write_text(json.dumps({
        "modell": MODEL_NAME,
        "geraet": geraet,
        "datum": date.today().isoformat(),
        "parameter": vars(args),
        "rundung": "stochastisch" if args.stochastisch else "zur naechsten Stufe",
        "verlust_referenz": verluste_a,
        "verlust_ganzzahl": verluste_b,
        "verlust_ende_referenz": ende_a,
        "verlust_ende_ganzzahl": ende_b,
        "abstand_prozent": abstand,
        "halte_referenz": halte_a,
        "halte_ganzzahl": halte_b,
        "halte_abstand_prozent": halte_abstand,
        "referenz_lernt": referenz_lernt,
        "ganzzahl_lernt": ganzzahl_lernt,
        "abstand_je_schritt": {
            "median": statistics.median(je_schritt),
            "max_betrag": max(je_schritt, key=abs),
        },
        "faellt_monoton_referenz": faellt_monoton(verluste_a, fenster),
        "faellt_monoton_ganzzahl": faellt_monoton(verluste_b, fenster),
        "dynamikbereich_bits": {
            "median": statistics.median(spannen),
            "max": max(spannen),
            "zwischen_bloecken_median": statistics.median(zwischen),
            "innerhalb_block_median": statistics.median(innerhalb),
            "abgedeckt_von_int_n": args.grad_bits - 1,
        },
        "eskalationsprobe_int16": dritter,
        "saettigung": {"median": statistics.median(saettigungen), "max": max(saettigungen)},
        "ausloeschung": {"median": statistics.median(ausloeschungen), "max": max(ausloeschungen)},
        "je_ebene": je_ebene,
        "dauer_s": {"referenz": dauer_a, "ganzzahl": dauer_b},
        "traegt": traegt,
    }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"\nGeschrieben: {ziel.relative_to(WURZEL)}")
    return 0 if traegt else 1


if __name__ == "__main__":
    sys.exit(main())
