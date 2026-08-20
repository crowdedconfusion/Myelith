#!/usr/bin/env python3
"""Der Boden des Schemas: W8 UND A16 gemeinsam, alles andere float.

**Die Frage (2026-08-20, Fahrplanpunkt 12.77).** Bei 7B liegt der
Integer-Pfad bei 9,40 gegen eine BF16-Baseline von 8,68, also +8,29 %;
das Kriterium verlangt ≤ 5 %. Bevor eine Eskalation gewählt wird, muss
feststehen, **wieviel überhaupt zu holen ist**.

Die bisherigen Simulationen haben beide Bausteine **einzeln** gemessen:

| Simulation | Ergebnis |
|---|---|
| `w8_reference_simulation.py` (nur Gewichte) | +0,7 % |
| `a16_reference_simulation.py` (nur Aktivierungen) | ±0 % |

Beide zusammen wurden nie gemessen — und genau die Kombination ist unser
Pfad. Quantisierungsfehler addieren sich nicht einfach: Die
Aktivierungsskala wird auf einem Residualstrom kalibriert, den bereits
quantisierte Gewichte erzeugt haben. Der gemeinsame Effekt kann größer
sein als die Summe.

**Was die beiden Ausgänge bedeuten:**

- **Nahe 8,7–8,8** → Das Schema gibt fast die Baseline her, und unsere
  verbleibenden ~0,6 Punkte sind **Implementierungsverlust**
  (LUT-Auflösung, Shift-Rundung, Skalen-Granularität). Dann ist 12.77
  eine Fehlersuche, keine Schema-Frage — und die Eskalationen 4–7 aus
  dem Fahrplan wären am falschen Ende angesetzt.
- **Nahe 9,3–9,4** → Wir sind am Boden dessen, was W8A16 hergibt. Dann
  hilft kein Feilen an der Umsetzung, sondern nur ein besseres Schema
  (Ausreißerbehandlung: FSBR oder Hadamard).

Das ist dieselbe Trennung, die bei der 7B-Fehlersuche entschieden hat:
**trägt das Verfahren, oder trägt unsere Umsetzung es nicht?**

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad. Kein Teil des
Auslieferungspfads.

Usage:
    INTEGER_LLM_MODEL=qwen2.5-7b ./calibrate/.venv/bin/python \\
        tests/diag/w8a16_reference_simulation.py
"""
import math
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, select_sequences  # noqa: E402

INT16_MAX = 32767
MAX_FRAC_BITS = 20

# Dieselben Projektionen wie calibrate/src/quantize.py.
ZIEL_LINEAR = ("q_proj", "k_proj", "v_proj", "o_proj",
               "gate_proj", "up_proj", "down_proj")


def quantisiere_gewicht(W):
    """int8, symmetrisch, eine Zweierpotenz-Skala je Ausgabe-Zeile.

    Identisch zu `quantize_symmetric_int8_per_channel` — bewusst
    nachgebaut statt importiert, damit die Simulation nicht denselben
    Fehler macht wie der Kalibrierpfad, falls dort einer steckt.
    """
    import torch
    absmax = W.abs().amax(dim=1, keepdim=True).clamp(min=1e-9)
    shift = torch.floor(torch.log2(127.0 / absmax)).clamp(0, MAX_FRAC_BITS)
    skala = torch.pow(2.0, shift)
    return torch.clamp(torch.round(W * skala), -128, 127) / skala


def shifts_aus_absmax(absmax):
    import torch
    return torch.floor(torch.log2(INT16_MAX / absmax.clamp(min=1e-9))).clamp(0, MAX_FRAC_BITS)


def quantisiere_aktivierung(x, shifts):
    import torch
    skala = torch.pow(2.0, shifts)
    return torch.clamp(torch.round(x * skala), -INT16_MAX - 1, INT16_MAX) / skala


def perplexitaet(model, sequences):
    import torch
    s, n = 0.0, 0
    with torch.no_grad():
        for ids in sequences:
            logits = model(input_ids=torch.tensor([ids], device=model.device)).logits[0].float()
            lp = torch.log_softmax(logits[:-1], dim=-1)
            ziel = torch.tensor(ids[1:], device=lp.device)
            tl = lp.gather(1, ziel.unsqueeze(1)).squeeze(1)
            s += tl.sum().item()
            n += tl.numel()
    return math.exp(-s / n), n


def residualstrom_module(model):
    """Die Module, deren EINGANG unser Residualstrom-Segment ist —
    exakt dieselbe Auswahl wie `calibrate/src/stats.py`."""
    return [m for n, m in model.named_modules()
            if n.endswith(("input_layernorm", "post_attention_layernorm"))
            or n == "model.norm"]


def main():
    import torch

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    mess = select_sequences(4, seq_len, verbose=False)
    model, _ = load_reference_model(MODEL_DIR)

    basis, n = perplexitaet(model, mess)
    print(f"[w8a16] Baseline (alles float)         : {basis:.2f} ({n} Positionen)")

    # ── Schritt 1: Gewichte quantisieren ──────────────────────────────
    anzahl = 0
    with torch.no_grad():
        for name, module in model.named_modules():
            if name.endswith(ZIEL_LINEAR) and hasattr(module, "weight"):
                W = module.weight.data.float()
                module.weight.data = quantisiere_gewicht(W).to(module.weight.dtype)
                anzahl += 1
    ppl_w8, _ = perplexitaet(model, mess)
    print(f"[w8a16] + W8 per Kanal ({anzahl:3d} Module)   : {ppl_w8:.2f}")

    # ── Schritt 2: Aktivierungsskalen auf DEM QUANTISIERTEN Modell ────
    # Das ist der Punkt, den die Einzelsimulationen nicht abbilden: Die
    # Skalen werden auf einem Residualstrom kalibriert, den bereits
    # quantisierte Gewichte erzeugen — so wie in unserer Pipeline auch.
    ziel_module = residualstrom_module(model)
    kalib = select_sequences(64, seq_len, verbose=False)
    gesammelt = {}
    handles = []
    for i, m in enumerate(ziel_module):
        def sammel(module, inputs, _i=i):
            x = inputs[0].detach().float()
            a = x.reshape(-1, x.shape[-1]).abs().amax(dim=0).cpu()
            gesammelt[_i] = torch.maximum(gesammelt[_i], a) if _i in gesammelt else a
        handles.append(m.register_forward_pre_hook(sammel))
    with torch.no_grad():
        for ids in kalib:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()
    print(f"[w8a16] Skalen aus {len(kalib)} Sequenzen kalibriert "
          f"({len(ziel_module)} Segmente)")

    shifts = {i: shifts_aus_absmax(v) for i, v in gesammelt.items()}
    handles = []
    for i, m in enumerate(ziel_module):
        def anwenden(module, inputs, _i=i):
            x = inputs[0]
            sh = shifts[_i].to(x.device)
            return (quantisiere_aktivierung(x.float(), sh).to(x.dtype),) + inputs[1:]
        handles.append(m.register_forward_pre_hook(anwenden))
    ppl_beide, _ = perplexitaet(model, mess)
    for h in handles:
        h.remove()
    print(f"[w8a16] + A16 per Kanal                : {ppl_beide:.2f}")

    # ── Schritt 3: Aktivierungen an JEDEM Linear-Eingang ──────────────
    # Der Unterschied zu Schritt 2 ist der Kern der Frage. Die Simulation
    # oben quantisiert nur an den 57 Residualstrom-Segmenten; unser
    # Integer-Pfad haelt **jeden** Zwischenwert in int16 — die Ausgaben
    # von q/k/v, den Attention-Ausgang, und vor allem das
    # MLP-Zwischenergebnis, das bei 7B 18 944 Kanaele breit ist. Wenn der
    # Verlust hier springt, liegt er an der Skalen-Granularitaet und
    # nicht an den LUTs.
    lin_module = [(n, m) for n, m in model.named_modules()
                  if n.endswith(ZIEL_LINEAR)]
    gesammelt_l = {}
    handles = []
    for i, (_, m) in enumerate(lin_module):
        def sammel_l(module, inputs, _i=i):
            x = inputs[0].detach().float()
            a = x.reshape(-1, x.shape[-1]).abs().amax(dim=0).cpu()
            gesammelt_l[_i] = torch.maximum(gesammelt_l[_i], a) if _i in gesammelt_l else a
        handles.append(m.register_forward_pre_hook(sammel_l))
    with torch.no_grad():
        for ids in kalib:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()

    # Per-Layer-Skalar statt per Kanal — so wie unser Integer-Pfad die
    # Nicht-Residual-Zwischenwerte fuehrt.
    shifts_l = {i: shifts_aus_absmax(v.max().reshape(1)) for i, v in gesammelt_l.items()}
    handles = []
    for i, (_, m) in enumerate(lin_module):
        def anwenden_l(module, inputs, _i=i):
            x = inputs[0]
            sh = shifts_l[_i].to(x.device)
            return (quantisiere_aktivierung(x.float(), sh).to(x.dtype),) + inputs[1:]
        handles.append(m.register_forward_pre_hook(anwenden_l))
    ppl_alle, _ = perplexitaet(model, mess)
    for h in handles:
        h.remove()
    print(f"[w8a16] + A16 an JEDEM Linear-Eingang   : {ppl_alle:.2f}"
          f"   ({len(lin_module)} Module, Skala je Layer)")

    # ── Auswertung ────────────────────────────────────────────────────
    integer_pfad = float(os.environ.get("INTEGER_PPL", "9.40"))
    print()
    print(f"{'Boden des Schemas (W8A16, sonst float)':<42} {ppl_beide:8.2f}"
          f"  ({100 * (ppl_beide / basis - 1):+.2f} %)")
    print(f"{'Unser Integer-Pfad':<42} {integer_pfad:8.2f}"
          f"  ({100 * (integer_pfad / basis - 1):+.2f} %)")
    print(f"{'Mit A16 an jedem Linear-Eingang':<42} {ppl_alle:8.2f}"
          f"  ({100 * (ppl_alle / basis - 1):+.2f} %)")
    rest = 100 * (integer_pfad / ppl_beide - 1)
    print(f"{'Abstand Integer-Pfad zum Schema-Boden':<42} {'':8}  ({rest:+.2f} %)")
    print()

    if rest < 2.0:
        print("-> Wir sind am BODEN DES SCHEMAS. Feilen an der Umsetzung")
        print("   bringt nichts mehr; 12.77 braucht ein besseres Schema")
        print("   (Ausreisserbehandlung: FSBR oder Hadamard).")
    else:
        print("-> Es steckt noch IMPLEMENTIERUNGSVERLUST drin. Die naechste")
        print("   Messung gehoert in die Nichtlinearitaeten (LUT-Ablation)")
        print("   und die Skalen-Granularitaet, nicht in ein neues Schema.")

    if ppl_beide / basis - 1 > 0.05:
        print()
        print("   Zusaetzlich: Schon der Schema-Boden verfehlt das")
        print("   5-%-Kriterium. Dann ist es mit Implementierungsarbeit")
        print("   allein grundsaetzlich nicht erreichbar.")


if __name__ == "__main__":
    main()
