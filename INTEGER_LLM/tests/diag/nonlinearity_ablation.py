#!/usr/bin/env python3
"""Welche Nichtlinearität kostet die fehlenden Prozentpunkte? (12.77)

**Stand der Eingrenzung (2026-08-20, Fund 28).**
`w8a16_reference_simulation.py` hat gemessen, wieviel überhaupt zu holen
ist:

| Stufe | Perplexität (7B) | gegen Baseline |
|---|---|---|
| Baseline, alles float | 8,68 | — |
| + W8 per Kanal | 8,74 | +0,69 % |
| + A16 per Kanal (Residualstrom) | 8,75 | +0,84 % |
| + A16 an **jedem** Linear-Eingang | 8,77 | +0,98 % |
| **unser Integer-Pfad** | **9,40** | **+8,28 %** |

Das Quantisierungsschema erfüllt das 5-%-Kriterium also mit großem
Abstand, und auch die Skalen-Granularität kostet nur 0,14 Punkte. Es
bleiben **rund 7,2 %**, die weder aus dem Schema noch aus der
Skalenwahl stammen. Übrig sind die **Nichtlinearitäten** — die einzigen
Stellen, an denen unser Pfad noch etwas tut, das die Simulation nicht
nachbildet.

**Die drei Verdächtigen und ihre Auflösung laut `theta_v/spec.json`:**

| LUT | Parameter | reale Auflösung |
|---|---|---|
| Softmax-Wahrscheinlichkeiten | `prob_frac_bits = 8` | 1/256 |
| RoPE cos/sin | `rope.frac_bits = 8` | 1/256 |
| SiLU-Ausgang | `silu.output_frac_bits = 6` | 1/64 |

Der erste ist der stärkste Verdacht: Bei Sequenzlänge 128 liegt eine
Gleichverteilung bei 1/128 ≈ 0,0078 — das sind **zwei Stufen** eines
1/256-Rasters. Aufmerksamkeit über viele Positionen wird damit grob
gerundet, und der Fehler geht direkt in den Attention-Ausgang.

Gemessen wird jede Näherung **einzeln** gegen dieselbe Grundlage und
danach alle **gemeinsam**. Der gemeinsame Wert ist die Probe aufs
Exempel: Kommt er nahe an 9,40, ist die Lücke vollständig erklärt.
Bleibt er deutlich darunter, fehlt noch eine Quelle — dann sind es die
Shift-Rundungen oder der KV-Cache.

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad. Kein Teil des
Auslieferungspfads.

Usage:
    INTEGER_LLM_MODEL=qwen2.5-7b ./calibrate/.venv/bin/python -u \\
        tests/diag/nonlinearity_ablation.py
"""
import math
import os
import pickle
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, MODEL_NAME, select_sequences  # noqa: E402

INT16_MAX = 32767
MAX_FRAC_BITS = 20
ZIEL_LINEAR = ("q_proj", "k_proj", "v_proj", "o_proj",
               "gate_proj", "up_proj", "down_proj")

# Aus theta_v/spec.json — hier bewusst als Konstanten gespiegelt, damit
# die Simulation nicht denselben Fehler macht wie der Ladepfad, falls
# dort einer steckt.
PROB_FRAC_BITS = 8
ROPE_FRAC_BITS = 8
SILU_INPUT_FRAC_BITS = 3
SILU_OUTPUT_FRAC_BITS = 6

CACHE = Path(__file__).resolve().parent / f".absmax_{MODEL_NAME}.pkl"


def quantisiere_gewicht(W):
    import torch
    absmax = W.abs().amax(dim=1, keepdim=True).clamp(min=1e-9)
    shift = torch.floor(torch.log2(127.0 / absmax)).clamp(0, MAX_FRAC_BITS)
    skala = torch.pow(2.0, shift)
    return torch.clamp(torch.round(W * skala), -128, 127) / skala


def shifts_aus_absmax(absmax):
    import torch
    return torch.floor(torch.log2(INT16_MAX / absmax.clamp(min=1e-9))).clamp(0, MAX_FRAC_BITS)


def quantisiere(x, shifts):
    import torch
    skala = torch.pow(2.0, shifts)
    return torch.clamp(torch.round(x * skala), -INT16_MAX - 1, INT16_MAX) / skala


def auf_raster(x, frac_bits):
    """Auf ein Zweierpotenz-Raster runden — die Wirkung einer LUT mit
    dieser Auflösung, ohne die LUT selbst nachzubauen."""
    import torch
    s = float(1 << frac_bits)
    return torch.round(x * s) / s


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


# ── Die drei Näherungen als an-/abschaltbare Haken ────────────────────

def haken_silu(model):
    """SiLU auf unserem Raster: Eingang 1/8, Ausgang 1/64."""
    import torch

    def hook(module, inputs, output):
        return auf_raster(torch.nn.functional.silu(
            auf_raster(inputs[0].float(), SILU_INPUT_FRAC_BITS)
        ), SILU_OUTPUT_FRAC_BITS).to(output.dtype)

    return [m.act_fn.register_forward_hook(hook)
            for n, m in model.named_modules() if n.endswith("mlp")]


def haken_rope(model):
    """cos/sin auf 1/256 runden."""
    import torch

    def hook(module, inputs, output):
        if isinstance(output, tuple):
            return tuple(auf_raster(o.float(), ROPE_FRAC_BITS).to(o.dtype) for o in output)
        return auf_raster(output.float(), ROPE_FRAC_BITS).to(output.dtype)

    return [m.register_forward_hook(hook)
            for n, m in model.named_modules() if n.endswith("rotary_emb")]


class SoftmaxPatch:
    """Attention-Wahrscheinlichkeiten auf 1/256 runden.

    Als Monkeypatch auf `torch.nn.functional.softmax`, weil die
    Wahrscheinlichkeiten innerhalb der Attention entstehen und nicht als
    Modul-Ausgang greifbar sind.

    **Setzt Eager-Attention voraus.** Der erste Anlauf dieser Messung
    (2026-08-20) lief unter `sdpa` — dort steckt der Softmax in einem
    fusionierten Kernel, `nn.functional.softmax` wird **nie** aufgerufen,
    und der Patch war wirkungslos. Das Ergebnis „+0,00 %" sah wie ein
    Befund aus und war eine Nullmessung. Deshalb zaehlt dieser Patch
    seine Aufrufe mit und `main()` bricht ab, wenn keiner ankommt: Eine
    Messung, die nichts misst, darf nicht wie ein Ergebnis aussehen.
    """

    def __init__(self):
        self.aufrufe = 0

    def __enter__(self):
        import torch
        self.original = torch.nn.functional.softmax

        def gepatcht(x, *a, **kw):
            self.aufrufe += 1
            return auf_raster(self.original(x, *a, **kw).float(), PROB_FRAC_BITS).to(x.dtype)

        torch.nn.functional.softmax = gepatcht
        return self

    def __exit__(self, *exc):
        import torch
        torch.nn.functional.softmax = self.original
        return False


def main():
    import torch

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    mess = select_sequences(4, seq_len, verbose=False)
    kalib = select_sequences(64, seq_len, verbose=False)
    model, _ = load_reference_model(MODEL_DIR)

    # Eager-Attention: Unter `sdpa` steckt der Softmax in einem
    # fusionierten Kernel und ist nicht abgreifbar (siehe SoftmaxPatch).
    model.set_attn_implementation("eager")
    print(f"[abl] attn_implementation = "
          f"{getattr(model.config, '_attn_implementation', '?')}", flush=True)

    # ── Grundlage aufbauen: W8 + A16 an jedem Linear-Eingang ──────────
    with torch.no_grad():
        for name, module in model.named_modules():
            if name.endswith(ZIEL_LINEAR) and hasattr(module, "weight"):
                module.weight.data = quantisiere_gewicht(
                    module.weight.data.float()).to(module.weight.dtype)
    print("[abl] Gewichte W8 per Kanal", flush=True)

    lin_module = [m for n, m in model.named_modules() if n.endswith(ZIEL_LINEAR)]

    if CACHE.exists():
        gesammelt = pickle.loads(CACHE.read_bytes())
        print(f"[abl] Aktivierungs-Absmax aus Cache ({CACHE.name})", flush=True)
    else:
        gesammelt = {}
        handles = []
        for i, m in enumerate(lin_module):
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
        CACHE.write_bytes(pickle.dumps(gesammelt))
        print(f"[abl] Aktivierungs-Absmax kalibriert und zwischengespeichert", flush=True)

    shifts = {i: shifts_aus_absmax(v.max().reshape(1)) for i, v in gesammelt.items()}
    for i, m in enumerate(lin_module):
        def anwenden(module, inputs, _i=i):
            x = inputs[0]
            return (quantisiere(x.float(), shifts[_i].to(x.device)).to(x.dtype),) + inputs[1:]
        m.register_forward_pre_hook(anwenden)
    print(f"[abl] A16 an {len(lin_module)} Linear-Eingaengen (Skala je Layer)", flush=True)

    grundlage, n = perplexitaet(model, mess)
    print(f"\n[abl] Grundlage (W8 + A16, exakte Nichtlinearitaeten): "
          f"{grundlage:.2f} ({n} Positionen)\n", flush=True)

    # ── Einzelmessungen ───────────────────────────────────────────────
    ergebnisse = {}

    for name, aufbau in [
        ("SiLU-Raster (1/8 ein, 1/64 aus)", haken_silu),
        ("RoPE cos/sin auf 1/256", haken_rope),
    ]:
        h = aufbau(model)
        ergebnisse[name], _ = perplexitaet(model, mess)
        for x in h:
            x.remove()
        print(f"[abl] + {name:<34}: {ergebnisse[name]:.2f}"
              f"   ({100 * (ergebnisse[name] / grundlage - 1):+.2f} % gegen Grundlage)", flush=True)

    name = "Softmax-Wahrscheinlichkeiten 1/256"
    with SoftmaxPatch() as sp:
        ergebnisse[name], _ = perplexitaet(model, mess)
    if sp.aufrufe == 0:
        raise SystemExit(
            "[abl] ABBRUCH: Der Softmax-Patch wurde nie aufgerufen. Die "
            "Attention laeuft nicht ueber nn.functional.softmax — die "
            "Messung waere eine Nullmessung, die wie ein Befund aussieht."
        )
    print(f"[abl] + {name:<34}: {ergebnisse[name]:.2f}"
          f"   ({100 * (ergebnisse[name] / grundlage - 1):+.2f} % gegen Grundlage)"
          f"   [{sp.aufrufe} Patch-Aufrufe]", flush=True)

    # ── Alle gemeinsam: die Probe aufs Exempel ────────────────────────
    h = haken_silu(model) + haken_rope(model)
    with SoftmaxPatch() as sp_alle:
        alle, _ = perplexitaet(model, mess)
    for x in h:
        x.remove()
    print(f"[abl] gemeinsame Messung: {len(h)} Haken, "
          f"{sp_alle.aufrufe} Patch-Aufrufe", flush=True)

    integer_pfad = float(os.environ.get("INTEGER_PPL", "9.40"))
    basis_float = float(os.environ.get("FP_BASELINE", "8.68"))

    print()
    print(f"{'Alle drei gemeinsam':<40} {alle:8.2f}"
          f"  ({100 * (alle / basis_float - 1):+.2f} % gegen FP-Baseline)")
    print(f"{'Unser Integer-Pfad':<40} {integer_pfad:8.2f}"
          f"  ({100 * (integer_pfad / basis_float - 1):+.2f} %)")
    luecke = 100 * (integer_pfad / alle - 1)
    print(f"{'Noch unerklaert':<40} {'':8}  ({luecke:+.2f} %)")
    print()

    schlimmster = max(ergebnisse, key=lambda k: ergebnisse[k])
    print(f"-> Groesster Einzelbeitrag: {schlimmster} "
          f"({100 * (ergebnisse[schlimmster] / grundlage - 1):+.2f} %)")
    if luecke < 1.5:
        print("-> Die drei Naeherungen erklaeren die Luecke praktisch vollstaendig.")
        print("   12.77 ist damit eine Frage der LUT-Aufloesung, und die")
        print("   Aufloesung ist ein theta_v-Parameter, kein Algorithmus.")
    else:
        print("-> Es bleibt eine Luecke. Naechste Verdaechtige: Shift-Rundung")
        print("   an jeder Reskalierung und der KV-Cache (frac_bits 8).")


if __name__ == "__main__":
    main()
