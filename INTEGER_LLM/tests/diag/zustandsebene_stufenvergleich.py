#!/usr/bin/env python3
"""**Eine Zustandsebene Stufe fuer Stufe gegen die Referenz.**

## ⛔️ Wozu, und warum erst jetzt

Am 2026-09-21/22 erzeugte das `myelith-35b-a3b` Kauderwelsch. Sechs
echte Fehler wurden gefunden und behoben (erfundener Router-Vorgabewert,
Rueckprojektion auf einer statt je Kanal, `rope_theta` 1e6 statt 1e7,
Drehtabellen ueber ein Sechstel des Kontexts, falsche Breite in der
Kontextgrenze, Modellangaben in θ_v), und **keiner** hat die Ausgabe
veraendert.

📌 **Sechs plausible Verdaechtige zu pruefen ist kein Ersatz dafuer,
einmal die Wahrheit danebenzulegen.** Diese Datei legt sie daneben.

## ⚑ Warum sie die 67 GB nicht braucht

Fuer eine einzelne Ebene reichen deren eigene Tensoren. Sie kommen
einzeln aus den `safetensors`, und die Referenz wird hier in Gleitkomma
nachgerechnet. Verglichen wird gegen die **Artefaktgewichte**, also
gegen das, womit die Laufzeit wirklich rechnet.

⚠️ **Was das prueft und was nicht.** Es prueft den Export und den
Entwurf des Rechenwegs. Ob der Rust-Kern dieselbe Rechnung macht,
sichern seine eigenen Proben; weicht er ab, ist das ein eigener Befund.

Aufruf:
    python3 zustandsebene_stufenvergleich.py [--ebene 0] [--token 9707]
"""
import argparse
import json
import math
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
QUELLE = REPO / "MODELS/llm/Qwen3.6-35B-A3B"
ARTEFAKT = REPO / "INTEGER_LLM/artifacts/myelith-35b-a3b"


def lade_artefakt_tensor(name: str):
    """Einen quantisierten Tensor samt Schiebungen aus dem Artefakt."""
    schluessel = name.replace(".", "_")
    manifest = json.loads((ARTEFAKT / "weights_manifest.json").read_text())
    eintrag = manifest.get(schluessel)
    if eintrag is None:
        return None, None, None
    roh = (ARTEFAKT / eintrag["file"]).read_bytes()
    if eintrag["dtype"] == "int8":
        werte = list(struct.unpack(f"<{len(roh)}b", roh))
    else:
        werte = list(struct.unpack(f"<{len(roh)//2}h", roh))
    schiebungen = None
    if eintrag.get("shifts_file"):
        sroh = (ARTEFAKT / eintrag["shifts_file"]).read_bytes()
        schiebungen = list(struct.unpack(f"<{len(sroh)}B", sroh))
    return werte, schiebungen, eintrag["shape"]


def entquantisiert(name: str):
    """Der Artefakttensor als Gleitkommamatrix, Zeile fuer Zeile skaliert."""
    werte, schiebungen, form = lade_artefakt_tensor(name)
    if werte is None:
        return None, None
    zeilen, spalten = form[0], form[1] if len(form) > 1 else 1
    aus = []
    for z in range(zeilen):
        s = schiebungen[z] if schiebungen else 0
        f = 1.0 / (1 << s)
        aus.append([w * f for w in werte[z * spalten:(z + 1) * spalten]])
    return aus, form


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--ebene", type=int, default=0)
    p.add_argument("--token", type=int, default=9707, help="'Hello'")
    p.add_argument("--nur-gewichte", action="store_true")
    args = p.parse_args()
    import torch
    from safetensors import safe_open

    karte = json.loads((QUELLE / "model.safetensors.index.json").read_text())["weight_map"]
    praefix = f"model.language_model.layers.{args.ebene}.linear_attn"

    def echt(nm):
        s = f"{praefix}.{nm}"
        if s not in karte:
            return None
        with safe_open(str(QUELLE / karte[s]), framework="pt") as f:
            return f.get_tensor(s).to(torch.float64)

    print(f"[stufen] Ebene {args.ebene}, Referenz gegen Artefakt\n")
    print(f"{'Tensor':<26} {'Form':>16} {'|echt|max':>11} "
          f"{'|artefakt|max':>14} {'rel. L2':>10}")
    print("-" * 82)

    schlimmste = (0.0, "")
    for nm in ("in_proj_qkv.weight", "in_proj_z.weight", "in_proj_a.weight",
               "in_proj_b.weight", "conv1d.weight", "norm.weight",
               "out_proj.weight"):
        e = echt(nm)
        if e is None:
            print(f"{nm:<26} {'fehlt in der Quelle':>16}")
            continue
        a, form = entquantisiert(f"model.layers.{args.ebene}.linear_attn.{nm}")
        if a is None:
            print(f"{nm:<26} {'FEHLT IM ARTEFAKT':>16}")
            schlimmste = max(schlimmste, (9.9, nm))
            continue
        et = e.reshape(len(a), -1) if e.dim() > 2 else e
        at = torch.tensor(a, dtype=torch.float64)
        # ⚑ Ein eindimensionaler Tensor kommt hier als (n, 1) an, weil
        #   `entquantisiert` zeilenweise arbeitet. Das ist eine Form
        #   dieser Datei, keine des Artefakts.
        if et.dim() == 1 and at.dim() == 2 and at.shape[1] == 1:
            at = at.reshape(-1)
        if tuple(et.shape) != tuple(at.shape):
            print(f"{nm:<26} {str(tuple(et.shape)):>16} ⛔️ FORM im Artefakt "
                  f"{tuple(at.shape)}")
            schlimmste = max(schlimmste, (9.9, nm))
            continue
        rel = float((et - at).norm() / et.norm())
        print(f"{nm:<26} {str(tuple(et.shape)):>16} {float(et.abs().max()):>11.4f} "
              f"{float(at.abs().max()):>14.4f} {rel:>10.5f}")
        if rel > schlimmste[0]:
            schlimmste = (rel, nm)

    # Die beiden Rekurrenzparameter, die zur Exportzeit umgerechnet werden.
    print()
    A_log = echt("A_log")
    if A_log is not None:
        exp_a_soll = A_log.exp()
        werte, schiebungen, _ = lade_artefakt_tensor(
            f"model.layers.{args.ebene}.linear_attn.exp_A")
        if werte is None:
            print("exp_A                      ⛔️ FEHLT IM ARTEFAKT")
            schlimmste = max(schlimmste, (9.9, "exp_A"))
        else:
            ist = torch.tensor([w / (1 << s) for w, s in zip(werte, schiebungen)],
                               dtype=torch.float64)
            rel = float((exp_a_soll - ist).norm() / exp_a_soll.norm())
            print(f"{'exp_A (= exp(A_log))':<26} {str(tuple(exp_a_soll.shape)):>16} "
                  f"{float(exp_a_soll.abs().max()):>11.4f} "
                  f"{float(ist.abs().max()):>14.4f} {rel:>10.5f}")
            if rel > schlimmste[0]:
                schlimmste = (rel, "exp_A")

    dt = echt("dt_bias")
    if dt is not None:
        werte, schiebungen, _ = lade_artefakt_tensor(
            f"model.layers.{args.ebene}.linear_attn.dt_bias")
        if werte is not None:
            ist = torch.tensor([w / (1 << s) for w, s in zip(werte, schiebungen)],
                               dtype=torch.float64)
            rel = float((dt - ist).norm() / dt.norm())
            print(f"{'dt_bias':<26} {str(tuple(dt.shape)):>16} "
                  f"{float(dt.abs().max()):>11.4f} {float(ist.abs().max()):>14.4f} "
                  f"{rel:>10.5f}")
            if rel > schlimmste[0]:
                schlimmste = (rel, "dt_bias")

    print()
    if schlimmste[0] > 1.0:
        print(f"[stufen] ⛔️ BEFUND: '{schlimmste[1]}' stimmt nicht "
              f"(rel. L2 {schlimmste[0]:.3f}).")
        return 1
    if schlimmste[0] > 0.05:
        print(f"[stufen] ⚠️ BEFUND: groesster relativer Fehler {schlimmste[0]:.4f} "
              f"bei '{schlimmste[1]}'. Fuer int8 ist unter 0,02 ueblich.")
        return 1
    print(f"[stufen] ⚑ Alle Gewichte der Ebene stimmen "
          f"(groesster relativer Fehler {schlimmste[0]:.5f} bei "
          f"'{schlimmste[1]}').")
    if args.nur_gewichte:
        return 0
    return stufenvergleich(args.ebene, args.token)



# ===========================================================================
# Der Vorwaertspass, Stufe fuer Stufe
# ===========================================================================

def rshift_round(wert: int, schiebung: int) -> int:
    """Rechtsshift mit Runden zur naechsten geraden Zahl (Regel 6)."""
    if schiebung <= 0:
        return wert << (-schiebung)
    rest = wert & ((1 << schiebung) - 1)
    ab = wert >> schiebung
    halb = 1 << (schiebung - 1)
    if rest > halb or (rest == halb and (ab & 1)):
        ab += 1
    return ab


def stufenvergleich(ebene: int, token: int) -> int:
    """**Ein Token durch eine Zustandsebene, Referenz gegen Ganzzahlpfad.**

    ⚑ **Beide starten bei derselben Einbettung.** Ein Unterschied kann
    damit nur aus der Rechnung kommen und nicht aus dem Eingang.

    ⚠️ **Der Zustand ist leer und die Position ist null.** Damit entfaellt
    die Drehung (sie kommt in dieser Ebene ohnehin nicht vor) und die
    Rekurrenz reduziert sich auf ihren ersten Schritt. Wer eine spaetere
    Position braucht, ruft mit einem laengeren Prompt.
    """
    import torch
    import torch.nn.functional as F
    from safetensors import safe_open

    karte = json.loads((QUELLE / "model.safetensors.index.json").read_text())["weight_map"]
    skalen = json.loads((ARTEFAKT / "scales.json").read_text())
    mc = json.loads((ARTEFAKT / "model_config.json").read_text())
    praefix = f"model.language_model.layers.{ebene}"

    def echt(nm):
        s = f"{praefix}.{nm}"
        with safe_open(str(QUELLE / karte[s]), framework="pt") as f:
            return f.get_tensor(s).to(torch.float64)

    def echt_global(s):
        with safe_open(str(QUELLE / karte[s]), framework="pt") as f:
            return f.get_tensor(s).to(torch.float64)

    kd = mc["linear_key_head_dim"]
    nk = mc["linear_num_key_heads"]
    nv = mc["linear_num_value_heads"]
    vd = mc["linear_value_head_dim"]
    k_breite = kd * nk
    eps = 1e-6

    # --- Stufe 0: die Einbettung, beiden gemeinsam.
    x = echt_global("model.language_model.embed_tokens.weight")[token].clone()

    berichte = []

    def vergleiche(name, soll, ist):
        soll = soll.reshape(-1).double()
        ist = ist.reshape(-1).double()
        if soll.numel() != ist.numel():
            berichte.append((name, float("inf"),
                             f"Laenge {soll.numel()} gegen {ist.numel()}"))
            return
        n = float(soll.norm())
        rel = float((soll - ist).norm() / n) if n > 0 else 0.0
        berichte.append((name, rel, f"|soll|max {float(soll.abs().max()):.4f}, "
                                   f"|ist|max {float(ist.abs().max()):.4f}"))

    # --- Stufe 1: die Norm vor dem Mischer.
    gamma = echt("input_layernorm.weight")
    rms = float((x.pow(2).mean() + eps).sqrt())
    h_soll = x / rms * gamma

    # Ganzzahlig: der Strom liegt auf residual_in_frac (je Kanal), die
    # Norm liefert auf norm_attn_frac.
    ein_sch = skalen[f"model.layers.{ebene}.input_layernorm.input"]["shifts"]
    norm_frac = skalen[f"model.layers.{ebene}.input_layernorm"]["shift"]
    x_int = [max(-32768, min(32767, int(round(float(v) * (1 << s)))))
             for v, s in zip(x, ein_sch)]
    ref_sch = max(ein_sch)
    acc = sum((v << (2 * (ref_sch - s))) * v for v, s in zip(x_int, ein_sch))
    n = len(x_int)
    # r = rsqrt(acc / n), exakt gerechnet (die Tabelle ist hier nicht der Punkt)
    mittel = acc / n
    r = 1.0 / math.sqrt(mittel) if mittel > 0 else 0.0
    g_int, g_sch, _ = lade_artefakt_tensor(f"model.layers.{ebene}.input_layernorm.weight")
    h_int = []
    for i in range(n):
        xi = x_int[i] << (ref_sch - ein_sch[i])
        gi = g_int[i] / (1 << g_sch[i])
        h_int.append(max(-32768, min(32767,
                     int(round(xi * r * gi * (1 << norm_frac))))))
    h_ist = torch.tensor([v / (1 << norm_frac) for v in h_int], dtype=torch.float64)
    vergleiche("1 Norm vor dem Mischer", h_soll, h_ist)

    # --- Stufe 2: die vier Projektionen.
    for nm, kurz in (("in_proj_qkv", "2a qkv"), ("in_proj_z", "2b z"),
                     ("in_proj_a", "2c a"), ("in_proj_b", "2d b")):
        W = echt(f"linear_attn.{nm}.weight")
        soll = W @ h_soll
        Wq, form = entquantisiert(f"model.layers.{ebene}.linear_attn.{nm}.weight")
        Wt = torch.tensor(Wq, dtype=torch.float64)
        ist = Wt @ h_ist
        vergleiche(f"{kurz} = {nm} @ h", soll, ist)

    # --- Stufe 3: Faltung mit SiLU, leeres Fenster.
    W = echt("linear_attn.in_proj_qkv.weight")
    qkv_soll = W @ h_soll
    Wc = echt("linear_attn.conv1d.weight").reshape(-1, mc["linear_conv_kernel_dim"])
    # Leeres Fenster: nur die letzte Kernstelle traegt.
    konv_soll = F.silu(qkv_soll * Wc[:, -1])
    Wcq, _ = entquantisiert(f"model.layers.{ebene}.linear_attn.conv1d.weight")
    Wcq = torch.tensor(Wcq, dtype=torch.float64)
    Wqkv_q, _ = entquantisiert(f"model.layers.{ebene}.linear_attn.in_proj_qkv.weight")
    qkv_ist = torch.tensor(Wqkv_q, dtype=torch.float64) @ h_ist
    konv_ist = F.silu(qkv_ist * Wcq[:, -1])
    vergleiche("3 Faltung + SiLU (leeres Fenster)", konv_soll, konv_ist)

    # --- Stufe 4: q und k auf Einheitslaenge, q mal 1/sqrt(kd).
    def normiere(v, dim, koepfe, skala):
        aus = []
        for kopf in range(koepfe):
            teil = v[kopf * dim:(kopf + 1) * dim]
            aus.append(teil / (teil.norm() + 1e-12) * skala)
        return torch.cat(aus)

    q_soll = normiere(konv_soll[:k_breite], kd, nk, 1.0 / math.sqrt(kd))
    k_soll = normiere(konv_soll[k_breite:2 * k_breite], kd, nk, 1.0)
    q_ist = normiere(konv_ist[:k_breite], kd, nk, 1.0 / math.sqrt(kd))
    k_ist = normiere(konv_ist[k_breite:2 * k_breite], kd, nk, 1.0)
    vergleiche("4a q normiert und skaliert", q_soll, q_ist)
    vergleiche("4b k normiert", k_soll, k_ist)

    # --- Stufe 5: die beiden Tore.
    a_soll = echt("linear_attn.in_proj_a.weight") @ h_soll
    b_soll = echt("linear_attn.in_proj_b.weight") @ h_soll
    dt = echt("linear_attn.dt_bias")
    exp_a = echt("linear_attn.A_log").exp()
    beta_soll = torch.sigmoid(b_soll)
    g_soll = torch.exp(-exp_a * F.softplus(a_soll + dt))

    Wa, _ = entquantisiert(f"model.layers.{ebene}.linear_attn.in_proj_a.weight")
    Wb, _ = entquantisiert(f"model.layers.{ebene}.linear_attn.in_proj_b.weight")
    a_ist = torch.tensor(Wa, dtype=torch.float64) @ h_ist
    b_ist = torch.tensor(Wb, dtype=torch.float64) @ h_ist
    ea, ea_s, _ = lade_artefakt_tensor(f"model.layers.{ebene}.linear_attn.exp_A")
    dtq, dt_s, _ = lade_artefakt_tensor(f"model.layers.{ebene}.linear_attn.dt_bias")
    exp_a_ist = torch.tensor([w / (1 << s) for w, s in zip(ea, ea_s)], dtype=torch.float64)
    dt_ist = torch.tensor([w / (1 << s) for w, s in zip(dtq, dt_s)], dtype=torch.float64)
    beta_ist = torch.sigmoid(b_ist)
    g_ist = torch.exp(-exp_a_ist * F.softplus(a_ist + dt_ist))
    vergleiche("5a beta = sigmoid(b)", beta_soll, beta_ist)
    vergleiche("5b g = exp(-exp_A*softplus(a+dt))", g_soll, g_ist)

    print(f"\n[stufen] Vorwaertspass Ebene {ebene}, Token {token}\n")
    print(f"{'Stufe':<38} {'rel. L2':>10}  Bemerkung")
    print("-" * 92)
    schlimmste = (0.0, "")
    for name, rel, bem in berichte:
        marke = "⛔️" if rel > 0.25 else ("⚠️" if rel > 0.05 else "  ")
        print(f"{marke} {name:<36} {rel:>10.5f}  {bem}")
        if rel > schlimmste[0]:
            schlimmste = (rel, name)
    print()
    print(f"[stufen] groesster relativer Fehler {schlimmste[0]:.5f} bei "
          f"'{schlimmste[1]}'")
    return 1 if schlimmste[0] > 0.25 else 0
if __name__ == "__main__":
    sys.exit(main())
