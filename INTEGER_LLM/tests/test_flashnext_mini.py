#!/usr/bin/env python3
"""
**Die Bauart mit vier Stroemen am Zufallsmodell** (Qwen3.8-Flash-Next,
2026-10-19).

Ein winziges Modell derselben Bauart (vier Ebenen, Breite 64, acht Experten,
N-Gramm-Einbettung in Ebene 2, Blockauswahl mit Budget 8) wird mit
`transformers` gebaut, gespeichert und dann **ebenenweise** kalibriert und
exportiert (`calibrate/src/ebenenweise.py`).

Geprueft wird:

1. **Der nachgebaute Vorwaertspass des Treibers ist der der Vorlage.** Der
   Treiber rechnet Einbettung, Masken, Drehtabellen und Strommischer selbst um
   die Ebenen herum; sein Ergebnis muss den Logits des ganzen Modells
   entsprechen, bis auf Gleitkommarauschen.
2. **Das Artefakt ist vollstaendig**: jede Skala, die die Laufzeit fordert,
   steht in `scales.json`; die Merkmale sind gesetzt; die Tabelle hat die Zeilen,
   die ihre Masse sagen.
3. **Die ganzzahlige Laufzeit rechnet dasselbe Modell**: Mit dem Pruefstand der
   Laufzeit (`flashnext_vergleich`) stimmt das naechste Token mit der Vorlage
   ueberein und die Logits korrelieren (Schwellen unten).
   3b. **Und ihre Arithmetik**: Mit den quantisierten Gewichten des Artefakts in
   der Vorlage liegen die Logits im Mittel unter 5 % relativem Abstand.
   Gemessen am 2026-10-19: 2,3 %, naechstes Token gleich an 46 von 48
   Positionen.

⚠️ **Braucht `transformers` ab 5.18.0** (erst dort gibt es `qwen4_exp`). Ohne
endet die Probe mit einer Meldung und Rueckgabe 0; sie gehoert dann nicht zu
den gefahrenen.

Aufruf: `<python mit transformers >= 5.18> INTEGER_LLM/tests/test_flashnext_mini.py`

Schalter: `MYL_FLASHNEXT_PRUEFSTAND=<pfad>` faehrt Schritt 3 mit dem Pruefstand der
Laufzeit; `MYL_FLASHNEXT_BEHALTEN=<ordner>` hebt das Artefakt dort auf.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER.parent / "calibrate"))

import torch  # noqa: E402

try:
    from transformers.models.qwen4_exp import configuration_qwen4_exp as k
    from transformers.models.qwen4_exp import modeling_qwen4_exp as m
except ImportError:
    import transformers
    print(f"[flashnext_mini] uebersprungen: transformers {transformers.__version__} kennt qwen4_exp nicht (ab 5.18.0)")
    sys.exit(0)

from src import ebenenweise as eb  # noqa: E402

EOS = 1


def mini_konfiguration():
    return k.Qwen4ExpTextConfig(
        vocab_size=512, hidden_size=64, num_hidden_layers=4, num_attention_heads=4, num_key_value_heads=2,
        head_dim=32, linear_num_key_heads=2, linear_num_value_heads=6, linear_key_head_dim=16,
        linear_value_head_dim=16, moe_intermediate_size=32, shared_expert_intermediate_size=32,
        num_experts=8, num_experts_per_tok=3, hc_count=4, hc_lowrank=16, ple_layer_ids=[2],
        ple_embed_dim=64, ngram_vocab_size_base=1000, heads_per_ngram=2, split_ngram_parts=4,
        indexer_n_heads=2, indexer_kv_heads=1, indexer_head_dim=16, indexer_budget=8,
        indexer_compress_ratio=4, output_gate_type="sigmoid", eos_token_id=EOS, bos_token_id=EOS,
        max_position_embeddings=4096, rms_norm_eps=1e-6,
        layer_types=["linear_attention", "linear_attention", "linear_attention", "full_attention"],
        rope_parameters={"rope_type": "default", "rope_theta": 10000000, "partial_rotary_factor": 0.25,
                         "mrope_section": [1, 1, 2], "mrope_interleaved": True},
    )


def mini_modell(ziel: Path):
    torch.manual_seed(1)
    modell = m.Qwen4ExpForCausalLM(mini_konfiguration()).eval()
    # ⚑ Die Vorlage legt alle Normen auf null (also `1 + 0`) und die
    #   PLE-Faltung auf null an. Mit diesen Anfangswerten liefe die Haelfte der
    #   Bausteine leer; deshalb Zufall auch dort.
    with torch.no_grad():
        for n, p in modell.named_parameters():
            if p.dim() == 1 or "conv1d" in n:
                p.normal_(0, 0.1)
            if n.endswith("A_log"):
                p.uniform_(-1, 1)
        # ⚑ Logits in der Groessenordnung eines echten Modells (einige
        #   Einheiten). Mit dem Zufallskopf laegen sie um 0,1, und die
        #   Laufzeit gibt sie auf 1/64 aus; der Vergleich waere Rundung.
        modell.lm_head.weight.mul_(60.0)
    modell.save_pretrained(ziel)
    return modell


def artefaktgewichte_einsetzen(modell, art: Path, versatz: float = 1.0):
    """**Die quantisierten Gewichte des Artefakts zurueck in die Vorlage.**

    ⚑ Damit misst Schritt 3b nur noch, was die Laufzeit rechnet, und nicht,
    was die Quantisierung der Gewichte kostet: Beide Seiten haben dieselben
    Gewichte, verschieden ist nur die Arithmetik (Aktivierungen in i16,
    Tabellen, ganzzahlige Normen).
    """
    import numpy as np
    mf = json.loads((art / "weights_manifest.json").read_text())
    q = {}
    for key, e in mf.items():
        dt = np.int16 if e["dtype"] == "int16" else np.int8
        roh = np.fromfile(art / e["file"], dtype=dt).astype(np.float64).reshape(e["shape"])
        sh = np.fromfile(art / e["shifts_file"], dtype=np.int8).astype(np.float64)
        w = roh / 2.0 ** sh if roh.ndim == 1 else roh / (2.0 ** sh)[:, None]
        q["lm_head(int16)" if key == "lm_head" else e["original_name"]] = torch.tensor(w, dtype=torch.float32)
    versetzt = ("hc_norm.weight", "norm_key.weight", "norm_query.weight", "norm_conv.weight",
                "q_layernorm.weight", "k_layernorm.weight", "q_norm.weight", "k_norm.weight")
    neu = {}
    for name, t in modell.state_dict().items():
        if name.endswith(("layer_multipliers", "ngram_heads_vocab_sizes", "ngram_heads_offsets")):
            continue
        if name.endswith("mlp.experts.gate_up_proj"):
            p = name[: -len("experts.gate_up_proj")]
            neu[name] = torch.stack([torch.cat([q[f"{p}experts.{e}.gate_proj.weight"], q[f"{p}experts.{e}.up_proj.weight"]])
                                     for e in range(t.shape[0])])
        elif name.endswith("mlp.experts.down_proj"):
            p = name[: -len("experts.down_proj")]
            neu[name] = torch.stack([q[f"{p}experts.{e}.down_proj.weight"] for e in range(t.shape[0])])
        elif name.endswith("linear_attn.A_log"):
            neu[name] = torch.log(q[name.replace("A_log", "exp_A")])
        else:
            w = q[name].reshape(t.shape)
            neu[name] = w - versatz if name.endswith(versetzt) else w
    modell.load_state_dict(neu, strict=False)
    modell.lm_head.weight.data = q["lm_head(int16)"].reshape(modell.lm_head.weight.shape)
    return modell


def sequenzen(n: int, laenge: int, saat: int):
    g = torch.Generator().manual_seed(saat)
    aus = []
    for i in range(n):
        s = torch.randint(2, 512, (laenge,), generator=g).tolist()
        if i % 3 == 1:
            s[laenge // 2] = EOS  # ein Abschnittsende mitten in der Folge
        aus.append(s)
    return aus


def main():
    fehler = []
    with tempfile.TemporaryDirectory(prefix="flashnext-mini-") as tmp:
        tmp = Path(tmp)
        quelle = tmp / "quelle"
        artefakt = tmp / "artefakt"
        modell = mini_modell(quelle)
        kalib = sequenzen(6, 40, 7)
        export_cfg = eb.export_konfiguration(eb.text_konfiguration_lesen(quelle), "mini", "Probe", "keins")
        lauf, stats, scales = eb.bauen(quelle, export_cfg, artefakt, kalib, tokenizer_kopieren=False)

        # --- 1. Der nachgebaute Vorwaertspass gegen die Vorlage
        with torch.no_grad():
            mischer = m.Qwen4ExpTextGatedResidual(lauf.cfg, use_combine=False)
            mischer.load_state_dict(modell.model.hyper_connection_mixer.state_dict())
            for ids, x in zip(lauf.ids, lauf.zustand):
                eigen = modell.lm_head(mischer(x))
                vorlage = modell(ids).logits
                abstand = (eigen - vorlage).abs().max().item()
                if abstand > 1e-4:
                    fehler.append(f"Vorwaertspass weicht ab: {abstand}")
        print(f"[flashnext_mini] 1. Nachbau gegen Vorlage: {'ok' if not fehler else fehler[-1]}")

        # --- 2. Das Artefakt
        mc = json.loads((artefakt / "model_config.json").read_text())
        for merkmal in ("vierstrom", "ngramm_einbettung", "blockauswahl", "zustandstor_sigmoid"):
            if merkmal not in mc["merkmale"]:
                fehler.append(f"Merkmal {merkmal} fehlt")
        pflicht = []
        for i in range(4):
            p = f"model.layers.{i}"
            for hc in ("attn_hyper_connection", "mlp_hyper_connection"):
                pflicht += [f"{p}.{hc}.input", f"{p}.{hc}.hc_norm", f"{p}.{hc}.input_mix_weight_down",
                            f"{p}.{hc}.input_mix_weight_up.input", f"{p}.{hc}.input_mix_weight_up",
                            f"{p}.{hc}.block_inject_weight", f"{p}.{hc}"]
            pflicht += [f"{p}.mlp.ausgang"]
            pflicht += [f"{p}.linear_attn.ausgang"] if i < 3 else [
                f"{p}.self_attn.ausgang", f"{p}.self_attn.indexer.index_qk_proj.q",
                f"{p}.self_attn.indexer.index_qk_proj.k", f"{p}.self_attn.indexer.q_layernorm",
                f"{p}.self_attn.indexer.k_layernorm"]
        p = "model.layers.1.ple"
        pflicht += [f"{p}.input", p, f"{p}.ple_embedding", f"{p}.key_proj", f"{p}.value_proj",
                    f"{p}.norm_key", f"{p}.norm_query", f"{p}.norm_conv", f"{p}.conv1d"]
        q = "model.hyper_connection_mixer"
        pflicht += [f"{q}.input", q, f"{q}.hc_norm", f"{q}.input_mix_weight_down",
                    f"{q}.input_mix_weight_up.input", f"{q}.input_mix_weight_up"]
        fehlend = [s for s in pflicht if s not in scales]
        if fehlend:
            fehler.append(f"{len(fehlend)} Skalen fehlen, etwa {fehlend[:5]}")
        manifest = json.loads((artefakt / "weights_manifest.json").read_text())
        tab = manifest.get("model_layers_1_ple_ple_embedding_ngram_embedding_weight")
        if tab is None or tab["shape"][0] % 128 != 0:
            fehler.append(f"N-Gramm-Tabelle fehlt oder ist nicht auf 128 gepolstert: {tab and tab['shape']}")
        print(f"[flashnext_mini] 2. Artefakt: {len(scales)} Skalen, {len(manifest)} Tensoren, "
              f"Merkmale {mc['merkmale']}")

        # Fuer die Arbeit an der Laufzeit: das Artefakt aufheben.
        behalten = os.environ.get("MYL_FLASHNEXT_BEHALTEN")
        if behalten:
            import shutil
            shutil.rmtree(behalten, ignore_errors=True)
            shutil.copytree(artefakt, behalten)
            print(f"[flashnext_mini] Artefakt aufgehoben in {behalten}")

        # --- 3. Die Laufzeit
        pruef = os.environ.get("MYL_FLASHNEXT_PRUEFSTAND")
        if pruef:
            folge = sequenzen(1, 48, 99)[0]
            vorlage = modell(torch.tensor([folge])).logits[0]
            aus = subprocess.run([pruef, str(artefakt), json.dumps(folge)], capture_output=True, text=True)
            if aus.returncode != 0:
                fehler.append(f"Pruefstand mit {aus.returncode}: {aus.stderr[-2000:]}")
            else:
                ganz = json.loads(aus.stdout)
                logits = torch.tensor(ganz["logits"], dtype=torch.float64) / (1 << ganz["logit_frac"])
                gleich = (logits.argmax(-1) == vorlage.argmax(-1)).float().mean().item()
                korr = torch.corrcoef(torch.stack([logits.flatten(), vorlage.flatten().double()]))[0, 1].item()
                print(f"[flashnext_mini] 3. Laufzeit: naechstes Token gleich {gleich:.3f}, Korrelation {korr:.4f}")
                if gleich < 0.9 or korr < 0.98:
                    fehler.append(f"Laufzeit weicht ab: Token {gleich:.3f}, Korrelation {korr:.4f}")
                # --- 3b. Dieselben Gewichte auf beiden Seiten
                with torch.no_grad():
                    artefaktgewichte_einsetzen(modell, artefakt)
                    gleichgewichtig = modell(torch.tensor([folge])).logits[0].double()
                abstand = ((logits - gleichgewichtig).norm(dim=-1) / gleichgewichtig.norm(dim=-1)).mean().item()
                gleich_b = (logits.argmax(-1) == gleichgewichtig.argmax(-1)).double().mean().item()
                print(f"[flashnext_mini] 3b. Mit den Gewichten des Artefakts: Logits {abstand:.4f} relativ, "
                      f"naechstes Token gleich {gleich_b:.3f}")
                if abstand > 0.05 or gleich_b < 0.9:
                    fehler.append(f"Arithmetik weicht ab: Logits {abstand:.4f}, Token {gleich_b:.3f}")
        else:
            print("[flashnext_mini] 3. Laufzeit: uebersprungen (MYL_FLASHNEXT_PRUEFSTAND nicht gesetzt)")

    if fehler:
        print("[flashnext_mini] FEHLER:")
        for f in fehler:
            print("  -", f)
        sys.exit(1)
    print("[flashnext_mini] bestanden")


if __name__ == "__main__":
    main()
