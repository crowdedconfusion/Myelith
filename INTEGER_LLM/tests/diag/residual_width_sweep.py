#!/usr/bin/env python3
"""Wieviel bringt eine breitere Zwischenrepräsentation? (12.77)

**Die Frage.** Der ebenenweise Bulk-Vergleich hat gezeigt, dass der
Fehler nicht akkumuliert, sondern bei rund 8–10 % je Ebene verharrt —
also verteilte Rundung statt eines lokalisierten Fehlers. Der einzige
Hebel dagegen ist mehr Auflösung. Bevor θ_v angefasst wird (Bump +
Neuexport aller Artefakte), muss feststehen, **wieviel das überhaupt
bringt**.

**Der Aufbau ist bewusst nah am echten Pfad.** Quantisiert wird der
Residualstrom an denselben 56 Abgriffen wie in der Kalibrierung
(`input_layernorm.input`, `post_attention_layernorm.input`) — und zwar
mit den **tatsächlichen Per-Kanal-Shifts aus dem Artefakt**, nicht mit
neu berechneten. Variiert wird ausschließlich die **Wortbreite**: bei
int16 ist der darstellbare Bereich ±32767, bei int24 ±8388607, bei
gleichbleibender Skala also 256-fach feinere Auflösung relativ zum
Maximum.

**Was die Ausgänge bedeuten:**

- **int24/int32 bringen deutlich** → Die verbleibende Abweichung ist
  wirklich Auflösung, und der θ_v-Sprung sollte die Breite des
  Residualstroms mitnehmen. Kosten: doppelter Speicher im Cache und auf
  der Leitung, ein breiterer Akkumulator — kein neuer Algorithmus.
- **int24/int32 bringen nichts** → Die Rundung im Residualstrom ist
  nicht die Quelle. Dann bleiben nur die Zwischenwerte **innerhalb**
  einer Ebene (Attention-Scores, MLP-Zwischenergebnis), und der Hebel
  säße dort.

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad. Kein Teil des
Auslieferungspfads.

Usage:
    INTEGER_LLM_MODEL=qwen2.5-7b ./calibrate/.venv/bin/python -u \\
        tests/diag/residual_width_sweep.py
"""
import json
import math
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, MODEL_NAME, select_sequences  # noqa: E402

MAXF = 20
LINEAR = ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj")
WEITERE = ("embed_tokens", "lm_head", "input_layernorm",
           "post_attention_layernorm", "norm")
ABGRIFF = ("input_layernorm.input", "post_attention_layernorm.input")


def q8(W):
    import torch
    a = W.abs().clamp(min=1e-9) if W.dim() == 1 else W.abs().amax(dim=1, keepdim=True).clamp(min=1e-9)
    sc = torch.pow(2.0, torch.floor(torch.log2(127.0 / a)).clamp(0, MAXF))
    return torch.clamp(torch.round(W * sc), -128, 127) / sc


def ppl(model, seqs):
    import torch
    s = n = 0
    with torch.no_grad():
        for ids in seqs:
            lg = model(input_ids=torch.tensor([ids], device=model.device)).logits[0].float()
            lp = torch.log_softmax(lg[:-1], dim=-1)
            t = lp.gather(1, torch.tensor(ids[1:], device=lp.device).unsqueeze(1)).squeeze(1)
            s += t.sum().item()
            n += t.numel()
    return math.exp(-s / n)


def main():
    import torch

    mess = select_sequences(4, int(os.environ.get("E2E_SEQ_LEN", "128")), verbose=False)
    skalen = json.loads((REPO / "artifacts" / MODEL_NAME / "scales.json").read_text())

    model, _ = load_reference_model(MODEL_DIR)
    with torch.no_grad():
        for nme, m in model.named_modules():
            if hasattr(m, "weight") and m.weight is not None and (
                    nme.endswith(LINEAR) or any(k in nme for k in WEITERE)):
                m.weight.data = q8(m.weight.data.float()).to(m.weight.dtype)
    print("[breite] Gewichte W8 per Kanal", flush=True)

    # Module den Artefakt-Shifts zuordnen. Fehlt ein Eintrag, wird das
    # laut gemeldet statt still uebersprungen — ein Abgriff ohne Skala
    # waere eine Luecke, die die Messung beschoenigt.
    zuordnung = []
    for nme, m in model.named_modules():
        for suffix in ABGRIFF:
            schluessel = f"{nme}.input"
            if nme.endswith(suffix.rsplit(".", 1)[0]) and schluessel in skalen:
                eintrag = skalen[schluessel]
                sh = eintrag.get("shifts", eintrag) if isinstance(eintrag, dict) else eintrag
                zuordnung.append((m, torch.tensor(sh, dtype=torch.float64)))
                break
    print(f"[breite] {len(zuordnung)} Residualstrom-Abgriffe mit Artefakt-Shifts", flush=True)
    if len(zuordnung) < 2 * model.config.num_hidden_layers:
        print(f"[breite] WARNUNG: erwartet {2 * model.config.num_hidden_layers}, "
              f"gefunden {len(zuordnung)} — die Messung waere unvollstaendig", flush=True)

    Z = {"bits": None}
    for m, sh in zuordnung:
        def anwenden(mod, inputs, _sh=sh):
            if Z["bits"] is None:
                return None
            x = inputs[0]
            grenze = float((1 << (Z["bits"] - 1)) - 1)
            sc = torch.pow(2.0, _sh.to(x.device).to(torch.float32))
            q = torch.clamp(torch.round(x.float() * sc), -grenze - 1.0, grenze) / sc
            return (q.to(x.dtype),) + inputs[1:]
        m.register_forward_pre_hook(anwenden)

    Z["bits"] = None
    grund = ppl(model, mess)
    print(f"\n[breite] Residualstrom exakt (nur W8)      : {grund:.2f}", flush=True)
    print(f"[breite] FP-Baseline                       : 8.68\n", flush=True)

    for b in (12, 16, 20, 24, 32):
        Z["bits"] = b
        w = ppl(model, mess)
        marke = "  <- unser Format" if b == 16 else ""
        print(f"[breite] Residualstrom als int{b:<3}          : {w:.2f}"
              f"   ({100 * (w / grund - 1):+.2f} % gegen exakt){marke}", flush=True)


if __name__ == "__main__":
    main()
