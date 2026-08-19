#!/usr/bin/env python3
"""
Prueft, ob die Per-Kanal-Skalen aus Fund 20 zur Laufzeit clippen.

Hintergrund (2026-08-19): Nach Fund 20 traegt der Residualstrom eine
Zweierpotenz-Skala JE KANAL statt je Segment. Jede Kanal-Skala wird so
eng wie moeglich gewaehlt (groesster Shift, der den KALIBRIERTEN Maximalwert
des Kanals traegt) — genau das ist der Auflösungsgewinn.

Der Preis: Per-Kanal-Skalen haben systematisch WENIGER Headroom als
Per-Tensor-Skalen. Kalibriert wird auf 64 WikiText-Sequenzen, gemessen
wird auf 4 ANDEREN (bewusst ausgespart, damit die Kalibrierung nicht auf
den Benchmark ueberpasst). Uebersteigt ein Kanal zur Messzeit seinen
kalibrierten Maximalwert, clippt er an der int16-Grenze — und zwar jetzt
viel leichter als mit der grosszuegigeren Per-Tensor-Skala.

Das waere eine Erklaerung fuer den Befund, dass Fund 20 bei 0,5B leicht
hilft (Aufloesungsgewinn > Clipping-Verlust) und bei 7B schadet
(umgekehrt) — und fuer das breit verteilte Rauschen, das der
positionsweise Vergleich zeigt.

Gemessen wird auf den ECHTEN Messsequenzen gegen die ECHTEN kalibrierten
Skalen aus scales.json.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/per_channel_headroom.py
"""
import json
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, ARTIFACTS_DIR, select_sequences  # noqa: E402

INT16_MAX = 32767


def main():
    import torch

    n_sequences = int(os.environ.get("E2E_SEQUENCES", "4"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(n_sequences, seq_len, verbose=False)

    scales = json.loads((ARTIFACTS_DIR / "scales.json").read_text())
    per_kanal = {k: v for k, v in scales.items() if "shifts" in v}
    print(f"[headroom] {len(per_kanal)} Residualstrom-Segmente mit Per-Kanal-Skalen")

    model, _ = load_reference_model(MODEL_DIR)

    # Reale AbsMax je Kanal auf den MESSsequenzen sammeln (dieselben Hooks
    # wie stats.py: Eingang der Norm-Module = Residualstrom-Segment).
    beobachtet = {}

    def mach_hook(name):
        def hook(module, inputs, output):
            x = inputs[0]
            if not isinstance(x, torch.Tensor):
                return
            t = x.detach().float().reshape(-1, x.shape[-1]).abs().amax(dim=0).cpu()
            beobachtet[name] = torch.maximum(beobachtet[name], t) if name in beobachtet else t
        return hook

    handles = []
    for name, module in model.named_modules():
        schluessel = name + ".input"
        if schluessel in per_kanal:
            handles.append(module.register_forward_hook(mach_hook(schluessel)))

    with torch.no_grad():
        for ids in sequences:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()

    print(f"[headroom] {len(beobachtet)} Segmente beobachtet")
    print()

    gesamt_kanaele = 0
    gesamt_clippend = 0
    schlimmste = []

    for name, obs in sorted(beobachtet.items()):
        shifts = per_kanal[name]["shifts"]
        if len(shifts) != len(obs):
            continue
        # Kapazitaet je Kanal: groesster real darstellbarer Betrag.
        kapazitaet = torch.tensor([INT16_MAX * (2.0 ** -s) for s in shifts])
        clippend = (obs > kapazitaet)
        n_clip = int(clippend.sum())
        gesamt_kanaele += len(shifts)
        gesamt_clippend += n_clip
        if n_clip:
            faktor = (obs / kapazitaet).max().item()
            schlimmste.append((name, n_clip, len(shifts), faktor))

    print(f"[headroom] Kanaele, die auf den Messsequenzen CLIPPEN: "
          f"{gesamt_clippend} von {gesamt_kanaele} ({gesamt_clippend/max(gesamt_kanaele,1):.2%})")
    print()
    if schlimmste:
        schlimmste.sort(key=lambda x: -x[3])
        print("Die 15 betroffensten Segmente (Faktor = wie weit ueber der Kapazitaet):")
        for name, n_clip, n_ges, faktor in schlimmste[:15]:
            print(f"  {name}: {n_clip}/{n_ges} Kanaele clippen, max {faktor:.2f}x ueber Kapazitaet")
    else:
        print("Kein Kanal clippt — die Per-Kanal-Skalen tragen die Messsequenzen.")


if __name__ == "__main__":
    main()
