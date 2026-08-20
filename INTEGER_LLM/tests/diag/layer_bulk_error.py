#!/usr/bin/env python3
"""Bulk-Fehler je Ebene: akkumuliert der Quantisierungsfehler oder nicht?

**Warum nicht AbsMax.** Der Vorgaenger dieser Messung verglich die
AbsMax-Werte je Ebene — und misst damit genau den einen Ausreisserkanal,
waehrend die uebrigen tausend schweigen. Dieselbe Falle steht schon in
der v0.12.27-Diagnose beschrieben („Reststrom-AbsMax stimmt in allen 24
Ebenen, aber Bulk-Dimensionen weichen ab"). Hier stattdessen der
relative L2-Fehler ueber ALLE Kanaele, zusaetzlich ohne den groessten
Kanal gerechnet.

**Ergebnis (0,5B, 2026-08-20):** Der Fehler steigt in den ersten drei
Ebenen auf rund 10 % und bleibt dann flach (7–10 %) ueber den ganzen
Stapel; nach der finalen Norm 6,7 %. Er akkumuliert also **nicht**. Damit
ist ein weiterer lokalisierter Fund vom Typ 23/24 unwahrscheinlich — die
verbleibende Abweichung ist verteilte Rundung je Ebene.

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad.

Usage:
    seq_layer_dump <artefakt> <token...> --full > dump.txt
    INTEGER_LLM_MODEL=... python tests/diag/layer_bulk_error.py dump.txt

**Die Ausrichtung ist der heikle Teil.**

HF legt in `hidden_states` die EINGAENGE der Ebenen ab, plus als letzten
Eintrag den Wert NACH der finalen Norm:
    hs[0]      = Embedding
    hs[i]      = Eingang Ebene i = Ausgabe Ebene i-1   (i = 1..N-1)
    hs[N]      = nach der finalen Norm
Unser Dump:
    dump[i]    = Ausgabe Ebene i                        (i = 0..N-1)
    dump[N]    = nach der finalen Norm
Also: dump[i] <-> hs[i+1] fuer i = 0..N-2, und dump[N] <-> hs[N].
Fuer dump[N-1] (Ausgabe der letzten Ebene) gibt es KEINEN hs-Eintrag.
"""
import sys, torch, numpy as np
from pathlib import Path
REPO = Path("/Users/entity/Desktop/Code/Code/Artificial/Myelith/Repository/INTEGER_LLM")
sys.path.insert(0, str(REPO/"calibrate")); sys.path.insert(0, str(REPO/"eval"))
from src.loader import load_reference_model
from wikitext_common import MODEL_DIR

integer = {}
for z in open(sys.argv[1]):
    if z.startswith("FULL"):
        p = z.split(); integer[int(p[1])] = np.array([float(x) for x in p[2:]])

import os
TOKENS = os.environ.get("BULK_TOKENS", "/tmp/tok32.txt")
toks = [int(x) for x in open(TOKENS).read().split()]
model,_ = load_reference_model(MODEL_DIR)
gef = {}
model.model.norm.register_forward_hook(lambda m,i,o: gef.update(ein=i[0].detach()[0,-1].float()))
with torch.no_grad():
    o = model(input_ids=torch.tensor([toks], device=model.device), output_hidden_states=True)
hs = [h[0,-1].float().cpu().numpy().astype(np.float64) for h in o.hidden_states]
N = model.config.num_hidden_layers
# Referenz fuer die Ausgabe der letzten Ebene: der Eingang der finalen Norm.
hs_letzte_ebene = gef["ein"].cpu().numpy().astype(np.float64)

def rel(a, b): return np.linalg.norm(a-b)/max(np.linalg.norm(b), 1e-12)

print(f"{'Ebene':>7} {'rel. L2':>9} {'ohne Spitze':>12}")
print("-"*32)
vor = None
for i in range(N):
    ref = hs[i+1] if i < N-1 else hs_letzte_ebene
    d = integer[i] - ref
    l2 = rel(integer[i], ref)
    k = int(np.argmax(np.abs(ref))); m = np.ones_like(ref, bool); m[k] = False
    print(f"{i:7d} {100*l2:8.2f}% {100*rel(integer[i][m], ref[m]):11.2f}%")
    vor = l2
print(f"{'Norm':>7} {100*rel(integer[N], hs[N]):8.2f}% "
      f"{100*rel(integer[N][:-1], hs[N][:-1]):11.2f}%")
