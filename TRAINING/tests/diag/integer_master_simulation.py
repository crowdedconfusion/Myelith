#!/usr/bin/env python3
"""
TRAINING 0.2: Ein Trainingsschritt OHNE Gleitkommazustand.

## Woher dieser Punkt kommt

`backward_reference_simulation.py` hat 0.1 beantwortet: Das
Quantisierungsschema traegt im Rueckwaertspass, sofern die Gewichte
stochastisch gerundet werden (+0,67 % gegen die Gleitkomma-Referenz,
gegen +29,9 % mit Rundung zur naechsten Stufe).

Sie hat dabei aber die Gewichte in **float32** gehalten und nur fuer den
Vorwaertspfad quantisiert. Fuer Myelith ist das zu wenig: Ein
Trainingsschritt, den das Netz im Konsens nachrechnen soll, darf keinen
Gleitkommazustand haben. Zwei Knoten mit demselben Gradienten muessen
denselben neuen Gewichtsstand bekommen, und zwar bitgleich.

## Was hier gemessen wird

Ein **ganzzahliger Master**: das Gewicht als int24 (int8-Wert plus 16
Nachkommabits), die Aktualisierung als exakte Ganzzahladdition, der int8
fuer den Vorwaertspfad stochastisch daraus gerundet.

    Master        m, ganze Zahl, |m| <= 127 * 2^16
    Vorwaertspfad w8 = stochastisch_runden(m / 2^16), auf [-128,127]
    Aktualisierung m <- m - round(lr * dL/dW * 2^shift * 2^16)

**Warum 16 Zusatzbits.** Gemessen ist, dass ein SGD-Schritt 0,03 % einer
int8-Rasterstufe betraegt; unter 12 Zusatzbits waere er kleiner als ein
LSB des Masters und verschwaende erneut. Mit 16 sind es rund 20 LSB, und
127 * 2^16 = 8,3 Mio. liegt unter 2^24: float32 haelt diese Ganzzahlen
**exakt**, die Simulation ist also keine eigene Fehlerquelle.

**Die Skala wird eingefroren.** Sonst aenderte sich die Bedeutung des
Masters mitten im Lauf. Gemessen (0.1): Eingefrorene Skalen aendern am
Ergebnis nichts.

## Was das kombiniert

- **Ganzzahliger Master** (NITI-Familie): kein Gleitkommazustand, die
  Aktualisierung ist eine Addition, die jeder nachrechnen kann.
- **Stochastisches Runden** (Gupta et al. 2015): loest das eigentliche
  Lernproblem und bleibt bei festem Keim reproduzierbar.
- **Fehlerrueckkopplung ist dabei GRATIS**: Die 16 Bits unterhalb der
  int8-Stufe *sind* der Quantisierungsrest. Er wird nicht verworfen und
  nicht getrennt gefuehrt, er steht im Master.

Gleitkomma erlaubt: Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage:
    cd INTEGER_LLM/calibrate
    .venv/bin/python ../../TRAINING/tests/diag/integer_master_simulation.py
"""
import argparse
import json
import statistics
import sys
import time
from datetime import date
from pathlib import Path

WURZEL = Path(__file__).resolve().parent.parent.parent.parent
INTEGER_LLM = WURZEL / "INTEGER_LLM"
sys.path.insert(0, str(INTEGER_LLM / "calibrate"))
sys.path.insert(0, str(INTEGER_LLM / "eval"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import torch  # noqa: E402
import torch.nn as nn  # noqa: E402

from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, MODEL_NAME  # noqa: E402
import backward_reference_simulation as basis  # noqa: E402

ERGEBNISSE = Path(__file__).resolve().parent / "results"
ZUSATZBITS = 16
INT8_MAX = 127


# ---------------------------------------------------------------------------
# Der Wuerfel: eine Funktion der Position, kein fortgeschriebener Zustand
# ---------------------------------------------------------------------------

def splitmix64(x: torch.Tensor) -> torch.Tensor:
    """Wie `kernels/src/prng.rs::splitmix64`, in torch nachgebaut.

    int64 laeuft in torch im Zweierkomplement um, also genau wie
    `wrapping_mul` in Rust. Die Maskierungen bilden den LOGISCHEN
    Rechtsshift nach, den Rust auf `u64` macht; torch schiebt auf int64
    arithmetisch.
    """
    z = x + torch.tensor(-7046029254386353131, dtype=torch.int64, device=x.device)
    z = (z ^ ((z >> 30) & 0x3FFFFFFFF)) * torch.tensor(
        -4658895280553007687, dtype=torch.int64, device=x.device)
    z = (z ^ ((z >> 27) & 0x1FFFFFFFFFF)) * torch.tensor(
        -7723592293110705685, dtype=torch.int64, device=x.device)
    return z ^ ((z >> 31) & 0x1FFFFFFFF)


def wuerfel(idx: torch.Tensor, ebene: int, schritt: int, keim: int) -> torch.Tensor:
    """Gleichverteilt in [0,1), bestimmt durch (Ebene, Schritt, Index, Keim).

    ## Warum nicht `torch.rand_like`

    **Gemessen (2026-08-22):** Der PyTorch-Zufall auf MPS liefert bei
    gleichem Keim in zwei frischen Prozessen verschiedene Ergebnisse. Fuer
    ein Netz, dessen Konsens auf Bitgleichheit beruht, ist das
    unbrauchbar: Zwei Knoten mit demselben Gradienten muessen denselben
    neuen Gewichtsstand bekommen.

    Zaehlerbasiert heisst: kein Zustand, keine Reihenfolgeabhaengigkeit,
    kein Geraeteeinfluss. **Nachgemessen:** identisch zwischen CPU und
    MPS und zwischen Prozessen, Mittelwert 0,499987 ueber 100 000 Werte.

    Genommen werden die oberen 24 Bits: genug Aufloesung fuer einen
    Rundungswuerfel und in float32 exakt darstellbar, also fuegt die
    Umrechnung keinen eigenen Fehler hinzu.
    """
    schluessel = idx + torch.tensor(
        (ebene * 1_000_003 + schritt) * 1_000_033 + keim,
        dtype=torch.int64, device=idx.device)
    roh = splitmix64(schluessel)
    oben = (roh >> 40) & 0xFFFFFF
    return oben.to(torch.float32) / float(1 << 24)


class GanzzahlLinearFn(torch.autograd.Function):
    """Vorwaerts aus dem ganzzahligen Master, rueckwaerts wie ueblich.

    Der Gradient bezieht sich auf das **wirksame Gewicht** und nicht auf
    den Master: Die Umrechnung in Masterschritte geschieht bei der
    Aktualisierung, damit sie dort als Ganzzahladdition sichtbar bleibt
    statt sich in einer Lernrate zu verstecken.
    """

    @staticmethod
    def forward(ctx, x, w_eff, b):
        ctx.save_for_backward(x, w_eff)
        ctx.hat_bias = b is not None
        out = x @ w_eff.t()
        if b is not None:
            out = out + b
        return out

    @staticmethod
    def backward(ctx, g):
        x, w_eff = ctx.saved_tensors
        gx = g @ w_eff
        gW = g.reshape(-1, g.shape[-1]).t() @ x.reshape(-1, x.shape[-1])
        # Ohne Bias muss hier `None` stehen: Ein Gradient zu einer
        # Eingabe, die es nicht gab, ist kein Gradient.
        gb = g.reshape(-1, g.shape[-1]).sum(0) if ctx.hat_bias else None
        return gx, gW, gb


class GanzzahlLinear(nn.Module):
    """Eine lineare Schicht, deren Zustand eine ganze Zahl ist."""

    def __init__(self, lin: nn.Linear, stochastisch: bool, ebene: int, keim: int):
        super().__init__()
        self.ebene = ebene
        self.keim = keim
        self.schritt_nr = 0
        W = lin.weight.detach()
        absmax = W.abs().amax(dim=1, keepdim=True)
        # Eingefroren: Der Master bekommt seine Bedeutung einmal.
        self.register_buffer("shift", basis._shift(absmax, INT8_MAX))
        skala = torch.pow(2.0, self.shift)
        # Der Master ist eine ganze Zahl in Einheiten von 2^-16 einer
        # int8-Stufe. Beim Anlegen zur naechsten gerundet: Das ist eine
        # einmalige Festlegung, kein Trainingsschritt.
        self.register_buffer("master", torch.round(W * skala * (1 << ZUSATZBITS)))
        self.bias = lin.bias
        self.stochastisch = stochastisch
        self.gradient = None
        # Einmal angelegt, danach nur noch gelesen: Der Wuerfel ist eine
        # Funktion dieses Index, nicht eines Zustands.
        self.register_buffer(
            "idx",
            torch.arange(W.numel(), dtype=torch.int64, device=W.device).reshape(W.shape))

    def wirksames_gewicht(self):
        roh = self.master / (1 << ZUSATZBITS)
        if self.stochastisch:
            unten = torch.floor(roh)
            w = wuerfel(self.idx, self.ebene, self.schritt_nr, self.keim)
            auf = (w < (roh - unten)).to(roh.dtype)
            q = unten + auf
        else:
            q = torch.round(roh)
        return torch.clamp(q, -INT8_MAX - 1, INT8_MAX) / torch.pow(2.0, self.shift)

    def forward(self, x):
        w = self.wirksames_gewicht()
        w.requires_grad_(True)
        w.retain_grad()
        self._w = w
        return GanzzahlLinearFn.apply(x, w, self.bias)

    def schritt(self, lr: float):
        """Die Aktualisierung: eine exakte Ganzzahladdition.

        `delta` ist ganzzahlig, `master` ist ganzzahlig, die Summe auch.
        Kein Gleitkommazustand ueberlebt diesen Aufruf.
        """
        if self._w.grad is None:
            return 0
        skala = torch.pow(2.0, self.shift) * (1 << ZUSATZBITS)
        delta = torch.round(lr * self._w.grad * skala)
        self.master -= delta
        grenze = float(INT8_MAX + 1) * (1 << ZUSATZBITS)
        self.master.clamp_(-grenze, grenze - 1)
        self._w.grad = None
        # Der Schrittzaehler geht in den Wuerfel ein: Ohne ihn faellt in
        # jedem Schritt dieselbe Entscheidung, und aus stochastischem
        # Runden wuerde eine feste, schiefe Rundung.
        self.schritt_nr += 1
        return int((delta != 0).sum().item())


def ersetzen(model, stochastisch: bool, keim: int):
    ersetzt = []
    for name, modul in list(model.named_modules()):
        for kind_name, kind in list(modul.named_children()):
            if not isinstance(kind, nn.Linear):
                continue
            voll = f"{name}.{kind_name}" if name else kind_name
            if "lm_head" in voll:
                continue
            neu = GanzzahlLinear(kind, stochastisch, len(ersetzt), keim)
            setattr(modul, kind_name, neu)
            ersetzt.append(neu)
    return ersetzt


def master_hash(schichten) -> str:
    """Ein Hash ueber alle Masterzustaende.

    Der Wert, den zwei Knoten vergleichen wuerden. Er ist nur dann
    aussagekraeftig, wenn der Master wirklich ganzzahlig ist, und genau
    das prueft `sind_ganzzahlig`.
    """
    import hashlib
    h = hashlib.sha256()
    for s in schichten:
        h.update(s.master.detach().to("cpu").to(torch.int32).numpy().tobytes())
    return h.hexdigest()[:16]


def sind_ganzzahlig(schichten) -> bool:
    return all(bool(torch.all(s.master == torch.round(s.master))) for s in schichten)


def lauf(stochastisch: bool, batches, halte, schritte: int, lr: float,
         geraet: str, keim: int):
    # Die Keimsetzung bleibt fuer alles, was NICHT der Wuerfel ist
    # (nichts davon zieht hier Zufall, aber die Gewohnheit schadet nicht).
    # Der Rundungswuerfel selbst braucht sie nicht: Er ist zaehlerbasiert.
    torch.manual_seed(keim)
    if geraet == "mps":
        torch.mps.manual_seed(keim)
    model, _ = load_reference_model(MODEL_DIR)
    model = model.to(torch.float32).to(geraet)
    model.train()
    model.config.use_cache = False
    schichten = ersetzen(model, stochastisch, keim)

    # Alles ausserhalb der ersetzten Schichten laeuft weiter in
    # Gleitkomma: Einbettung, Normierungen, LM-Kopf. Das ist die Grenze
    # dieser Simulation und steht so im Bericht.
    uebrige = [p for p in model.parameters() if p.requires_grad]
    opt = torch.optim.SGD(uebrige, lr=lr)

    anfang = basis.bewerten(model, halte, geraet)
    bewegt = 0
    t0 = time.time()
    for i in range(schritte):
        x = batches[i % len(batches)]
        model(x, labels=x).loss.backward()
        for s in schichten:
            bewegt += s.schritt(lr)
        opt.step()
        opt.zero_grad(set_to_none=True)
    dauer = time.time() - t0
    ende = basis.bewerten(model, halte, geraet)
    ganzzahlig = sind_ganzzahlig(schichten)
    h = master_hash(schichten)
    del model
    if geraet == "mps":
        torch.mps.empty_cache()
    return anfang, ende, ganzzahlig, h, bewegt, dauer


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--schritte", type=int, default=200)
    p.add_argument("--sequenzen", type=int, default=20)
    p.add_argument("--halte", type=int, default=8)
    p.add_argument("--seq-len", type=int, default=128)
    p.add_argument("--lr", type=float, default=1e-5)
    p.add_argument("--keim", type=int, default=1)
    args = p.parse_args()

    geraet = "mps" if torch.backends.mps.is_available() else "cpu"
    print("TRAINING 0.2: Trainingsschritt ohne Gleitkommazustand")
    print(f"  Modell {MODEL_NAME}, Geraet {geraet}, {args.schritte} Schritte, lr {args.lr}")
    print(f"  Master: int8 plus {ZUSATZBITS} Nachkommabits, Skala eingefroren\n")

    alle = basis.batches_bauen(args.sequenzen + args.halte, args.seq_len, geraet)
    batches, halte = alle[: args.sequenzen], alle[args.sequenzen :]

    # Der Referenzarm kommt aus der 0.1-Simulation: dieselbe Rechnung,
    # dieselben Batches, damit die Zahlen vergleichbar bleiben.
    print("Referenz: Gleitkomma, unveraendert")
    stat = basis.Statistik()
    v, kurve, dauer_r, _ = basis.lauf(
        False, batches, halte, args.schritte, args.lr, geraet, 128, stat, args.schritte)
    ref_a, ref_e = kurve[0][1], kurve[-1][1]
    print(f"  {ref_a:.4f} -> {ref_e:.4f}  ({dauer_r:.0f} s)\n")

    ergebnisse = {}
    for stochastisch, name in [(False, "Ganzzahl-Master, Rundung zur naechsten Stufe"),
                               (True, "Ganzzahl-Master, stochastisch")]:
        a, e, ganz, h, bewegt, dauer = lauf(
            stochastisch, batches, halte, args.schritte, args.lr, geraet, args.keim)
        abstand = (e - ref_e) / ref_e * 100.0
        print(f"{name}")
        print(f"  Haltemenge      {a:.4f} -> {e:.4f}   ({abstand:+.2f} % gegen die Referenz)")
        print(f"  Master ganzzahlig: {'JA' if ganz else 'NEIN'}   Hash {h}")
        print(f"  Aktualisierungen mit Wirkung: {bewegt:,}  ({dauer:.0f} s)\n")
        ergebnisse[name] = {
            "anfang": a, "ende": e, "abstand_prozent": abstand,
            "master_ganzzahlig": ganz, "master_hash": h,
            "wirksame_aktualisierungen": bewegt, "dauer_s": dauer,
        }

    # **Die Reproduzierbarkeitsprobe laeuft auf der CPU, und das hat einen
    # Grund.** Auf MPS weicht schon ein Lauf OHNE jeden Zufall zwischen
    # zwei Durchgaengen ab, sobald im selben Prozess vorher Modelle gebaut
    # und freigegeben wurden (gemessen 2026-08-22: 2,463654 gegen
    # 2,465496). Das ist eine Eigenschaft der Plattform, nicht des
    # Verfahrens, und es wuerde hier eine Aussage ueber den
    # Ganzzahlschritt verhindern, die es gar nicht betrifft.
    #
    # Kurze Strecke und kleine Sequenzen: Geprueft wird Gleichheit, nicht
    # Qualitaet, und auf der CPU kostet jeder Schritt ein Vielfaches.
    print("Reproduzierbarkeit (auf der CPU, siehe Kommentar im Quelltext)")
    cpu_alle = basis.batches_bauen(8, 64, "cpu")
    cpu_b, cpu_h = cpu_alle[:6], cpu_alle[6:]
    _, _, ganz2, h2, _, _ = lauf(True, cpu_b, cpu_h, 6, args.lr, "cpu", args.keim)
    _, _, _, h3, _, _ = lauf(True, cpu_b, cpu_h, 6, args.lr, "cpu", args.keim)
    _, _, _, h4, _, _ = lauf(True, cpu_b, cpu_h, 6, args.lr, "cpu", args.keim + 1)
    print(f"  Keim {args.keim}: Hash {h2} / {h3}   "
          f"{'IDENTISCH' if h2 == h3 else 'ABWEICHUNG'}")
    print(f"  Keim {args.keim + 1}: Hash {h4}   "
          f"{'gleich' if h4 == h2 else 'erwartungsgemaess anders'}")
    print(f"  Master ganzzahlig: {'JA' if ganz2 else 'NEIN'}")

    ERGEBNISSE.mkdir(parents=True, exist_ok=True)
    ziel = ERGEBNISSE / f"integer_master_{MODEL_NAME.replace('.', '')}_{args.schritte}s-lr{args.lr:g}.json"
    ziel.write_text(json.dumps({
        "modell": MODEL_NAME, "geraet": geraet, "datum": date.today().isoformat(),
        "parameter": vars(args), "zusatzbits": ZUSATZBITS,
        "referenz": {"anfang": ref_a, "ende": ref_e},
        "arme": ergebnisse,
        "reproduzierbar": {"geraet": "cpu", "hash_a": h2, "hash_b": h3,
                           "hash_anderer_keim": h4, "identisch": h2 == h3,
                           "hinweis": "Auf MPS weicht schon ein Lauf ohne Zufall ab; "
                                      "Plattformeigenschaft, nicht Verfahren."},
    }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"\nGeschrieben: {ziel.relative_to(WURZEL)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
