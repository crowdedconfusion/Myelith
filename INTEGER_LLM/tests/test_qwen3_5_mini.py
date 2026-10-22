#!/usr/bin/env python3
"""
**Die hybride dichte Bauart am Zufallsmodell** (Qwen3.8-27B, 2026-10-21).

Ein winziges Modell derselben Form wie Qwen3.8-27B (vier Ebenen, drei mit
linearer und eine mit voller Aufmerksamkeit, dichtes MLP, Ausgangstor,
Teildrehung mit Abschnitten), gebaut mit `transformers` als
`Qwen3_5ForConditionalGeneration` samt kleinem Sichtteil, also mit denselben
Tensornamen wie das echte (`model.language_model.…`, `model.visual.…`). Seine
`config.json` traegt die Felder der echten Quelle, die transformers 5.15 nicht
schreibt (`attn_output_gate`, `full_attention_interval`, `output_gate_type`).

Geprueft wird:

1. **Der Erzeuger** (`bauarten.erzeuge_qwen3_5`) macht aus dieser
   `config.json` einen Eintrag, und der Feldabgleich findet keine Abweichung
   und kein ungelesenes Feld.
2. **Der Korpusweg baut daraus ein Artefakt** (`calibrate.src.main`, wie im
   Bauskript), in ein Behelfsverzeichnis.
3. **Die ganzzahlige Laufzeit rechnet dasselbe Modell**: Mit dem Pruefstand
   (`flashnext_vergleich`, er ist allgemein) stimmt das naechste Token mit der
   Vorlage ueberein und die Logits korrelieren (Schwellen wie bei Flash-Next).

Die Quelle liegt waehrend des Laufs unter `MODELS/llm/` und der Eintrag unter
`calibrate/lokal/`, denn dort sucht die Kalibrierung; beide werden am Ende
entfernt, auch bei einem Fehler.

Aufruf: `INTEGER_LLM/calibrate/.venv/bin/python3 INTEGER_LLM/tests/test_qwen3_5_mini.py`

Schalter: `MYL_QWEN35_PRUEFSTAND=<pfad zu flashnext_vergleich>` faehrt Schritt 3;
ohne ihn wird er uebersprungen und gesagt. `MYL_QWEN35_BEHALTEN=<ordner>` hebt
Artefakt und Vorlage dort auf.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent
LLM = HIER.parent
WURZEL = LLM.parent
sys.path.insert(0, str(LLM / "calibrate"))

import torch  # noqa: E402
from transformers.models.qwen3_5 import configuration_qwen3_5 as k  # noqa: E402
from transformers.models.qwen3_5 import modeling_qwen3_5 as m  # noqa: E402

from src.bauarten import erkennen  # noqa: E402
from src.konfigabgleich import feldabgleich, flach  # noqa: E402
from src.model_configs import LOKALE_EINTRAEGE  # noqa: E402

ECHT = WURZEL / "MODELS" / "llm" / "Qwen3.8-27B"
TOKENIZER_DATEIEN = ("tokenizer.json", "tokenizer_config.json", "vocab.json", "merges.txt", "chat_template.jinja")
NAME = f"myelith-probe-qwen35-mini-{os.getpid()}"
QUELLORDNER = WURZEL / "MODELS" / "llm" / f"_probe-qwen35-mini-{os.getpid()}"


def mini_modell(ziel: Path):
    text = dict(
        vocab_size=248320, hidden_size=64, intermediate_size=128, num_hidden_layers=4, num_attention_heads=4,
        num_key_value_heads=2, head_dim=32, linear_num_key_heads=2, linear_num_value_heads=6,
        linear_key_head_dim=16, linear_value_head_dim=16, linear_conv_kernel_dim=4,
        layer_types=["linear_attention"] * 3 + ["full_attention"], max_position_embeddings=4096,
        rms_norm_eps=1e-6, tie_word_embeddings=False,
        rope_parameters={"rope_type": "default", "rope_theta": 10000000, "partial_rotary_factor": 0.25,
                         "mrope_section": [1, 1, 2], "mrope_interleaved": True},
    )
    vis = dict(depth=1, hidden_size=32, num_heads=2, intermediate_size=64, out_hidden_size=64)
    torch.manual_seed(3)
    modell = m.Qwen3_5ForConditionalGeneration(k.Qwen3_5Config(text_config=text, vision_config=vis)).eval()
    # ⚑ Die Vorlage legt Normen und Faltung so an, dass die Haelfte der
    #   Bausteine leer liefe; deshalb Zufall auch dort, und Logits in der
    #   Groessenordnung eines echten Modells (wie in der Probe von Flash-Next).
    with torch.no_grad():
        for n, p in modell.named_parameters():
            if p.dim() == 1 or "conv1d" in n:
                p.normal_(0, 0.1)
            if n.endswith("A_log"):
                p.uniform_(-1, 1)
        modell.lm_head.weight.mul_(60.0)
    modell.save_pretrained(ziel)
    # Die Felder der echten Quelle, die transformers 5.15 nicht schreibt.
    roh = json.loads((ziel / "config.json").read_text())
    echt = json.loads((ECHT / "config.json").read_text())["text_config"]
    for feld in ("attn_output_gate", "full_attention_interval", "output_gate_type", "mamba_ssm_dtype"):
        if feld in echt:
            roh["text_config"][feld] = echt[feld]
    roh["text_config"]["mtp_num_hidden_layers"] = 0
    (ziel / "config.json").write_text(json.dumps(roh, indent=2))
    for d in TOKENIZER_DATEIEN:
        if (ECHT / d).is_file():
            shutil.copy(ECHT / d, ziel / d)
    return modell, roh


def main() -> int:
    fehler = []
    if not (ECHT / "config.json").is_file():
        print(f"[qwen35_mini] uebersprungen: {ECHT.relative_to(WURZEL)}/config.json fehlt (Tokenizer und Felder der Quelle)")
        return 0
    eintrag_pfad = LOKALE_EINTRAEGE / f"{NAME}.json"
    try:
        with tempfile.TemporaryDirectory(prefix="qwen35-mini-") as tmp:
            artefakte = Path(tmp)
            modell, roh = mini_modell(QUELLORDNER)

            # --- 1. Erzeuger und Feldabgleich
            bauart = erkennen(roh)
            if bauart is None or not bauart.freigegeben:
                fehler.append(f"Bauart nicht freigegeben: {bauart}")
                raise RuntimeError
            eintrag = bauart.erzeuger(roh, "probe", "Probe", f"lokal/{QUELLORDNER.name}")
            befund = feldabgleich(eintrag, flach(roh))
            probleme = [a[0] for a in befund["abweichend"]] + befund["abgeleitet"] + befund["ungelesen"]
            if probleme:
                fehler.append(f"Feldabgleich: {probleme}")
            print(f"[qwen35_mini] 1. Erzeuger {bauart.name}: {'ok' if not probleme else probleme}")

            # --- 2. Der Korpusweg
            LOKALE_EINTRAEGE.mkdir(parents=True, exist_ok=True)
            eintrag_pfad.write_text(json.dumps(eintrag, indent=1), encoding="utf-8")
            lauf = subprocess.run(
                [sys.executable, "-m", "calibrate.src.main"], cwd=LLM, capture_output=True, text=True,
                env={**os.environ, "INTEGER_LLM_MODEL": NAME, "INTEGER_LLM_ARTIFACTS_DIR": str(artefakte)},
            )
            artefakt = artefakte / NAME
            if lauf.returncode != 0 or not (artefakt / "theta_v.json").is_file():
                fehler.append(f"Korpusweg mit {lauf.returncode}: {(lauf.stdout + lauf.stderr)[-3000:]}")
                raise RuntimeError
            mc = json.loads((artefakt / "model_config.json").read_text())
            n = len(json.loads((artefakt / "weights_manifest.json").read_text()))
            print(f"[qwen35_mini] 2. Artefakt: {n} Tensoren, Familie {mc['family']}, Merkmale {mc.get('merkmale')}")
            behalten = os.environ.get("MYL_QWEN35_BEHALTEN")
            if behalten:
                shutil.rmtree(behalten, ignore_errors=True)
                shutil.copytree(artefakt, behalten)
                modell.save_pretrained(Path(behalten) / "vorlage")
                print(f"[qwen35_mini] Artefakt und Vorlage aufgehoben in {behalten}")

            # --- 3. Die Laufzeit
            pruef = os.environ.get("MYL_QWEN35_PRUEFSTAND")
            if not pruef:
                print("[qwen35_mini] 3. Laufzeit: uebersprungen (MYL_QWEN35_PRUEFSTAND nicht gesetzt)")
            else:
                g = torch.Generator().manual_seed(99)
                folge = torch.randint(2, 248000, (48,), generator=g).tolist()
                with torch.no_grad():
                    vorlage = modell(input_ids=torch.tensor([folge])).logits[0].double()
                aus = subprocess.run([pruef, str(artefakt), json.dumps(folge)], capture_output=True, text=True)
                if aus.returncode != 0:
                    fehler.append(f"Pruefstand mit {aus.returncode}: {aus.stderr[-2000:]}")
                else:
                    ganz = json.loads(aus.stdout)
                    logits = torch.tensor(ganz["logits"], dtype=torch.float64) / (1 << ganz["logit_frac"])
                    gleich = (logits.argmax(-1) == vorlage.argmax(-1)).double().mean().item()
                    korr = torch.corrcoef(torch.stack([logits.flatten(), vorlage.flatten()]))[0, 1].item()
                    print(f"[qwen35_mini] 3. Laufzeit: naechstes Token gleich {gleich:.3f}, Korrelation {korr:.4f}")
                    if gleich < 0.9 or korr < 0.98:
                        fehler.append(f"Laufzeit weicht ab: Token {gleich:.3f}, Korrelation {korr:.4f}")
    except RuntimeError:
        pass
    finally:
        shutil.rmtree(QUELLORDNER, ignore_errors=True)
        eintrag_pfad.unlink(missing_ok=True)

    if fehler:
        print("[qwen35_mini] FEHLER:")
        for f in fehler:
            print("  -", f)
        return 1
    print("[qwen35_mini] bestanden")
    return 0


if __name__ == "__main__":
    sys.exit(main())
