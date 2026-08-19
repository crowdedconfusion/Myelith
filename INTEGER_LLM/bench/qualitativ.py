#!/usr/bin/env python3
"""
Qualitativer Benchmark: echte Prompts durch den Integer-Pfad, Seite an
Seite mit der BF16-Referenz.

Warum zusaetzlich zur Perplexitaet (2026-08-19): Perplexitaet ist ein
statistisches Mass ueber Teacher-Forcing — sie sagt, wie gut das Modell
das jeweils naechste Token der VORGEGEBENEN Sequenz bewertet. Sie sagt
nicht, ob freie Generierung brauchbaren Text liefert. Nach der Behebung
von Fund 23/24 (7B: 41,42 -> 9,40) gehoert diese zweite Art von Beleg
dazu, bevor man das Ergebnis als bestaetigt betrachtet.

Gemessen wird je Prompt:
  - der tatsaechlich erzeugte Text beider Pfade (greedy, gleiche Laenge)
  - die Uebereinstimmung der Token-Sequenzen
  - der Anteil identischer Top-1-Vorhersagen ueber die Prompt-Positionen
    (Teacher-Forcing, unabhaengig von Fehlerfortpflanzung in der
    Generierung)

Gleitkomma ist auf der Referenzseite erlaubt - Messpfad, nicht
Inferenzpfad. Kein Teil des Auslieferungspfads.

Usage:
  INTEGER_LLM_MODEL=qwen2.5-7b python bench/qualitativ.py [max_tokens]
"""
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
sys.path.insert(0, str(REPO / "tests"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, ARTIFACTS_DIR, MODEL_NAME  # noqa: E402
import cargo_paths  # noqa: E402

RUNTIME = cargo_paths.binary("runtime", "integer-llm-runtime")

PROMPTS = [
    "Die Hauptstadt von Frankreich ist",
    "The capital of France is",
    "Water boils at a temperature of",
    "Der Satz des Pythagoras besagt, dass",
    "In 1969, humans first landed on",
    "Die Quadratwurzel aus 144 ist",
    "A large language model is a type of",
    "Der wichtigste Bestandteil der Luft ist",
]


def integer_generierung(prompt: str, max_tokens: int):
    ergebnis = subprocess.run(
        [str(RUNTIME), str(ARTIFACTS_DIR), prompt, str(max_tokens)],
        capture_output=True, text=True,
    )
    if ergebnis.returncode != 0:
        raise RuntimeError(f"Runtime fehlgeschlagen: {ergebnis.stderr[:300]}")
    prompt_ids, gen_ids = None, None
    for zeile in ergebnis.stdout.splitlines():
        if "Prompt-Tokens:" in zeile:
            prompt_ids = eval(zeile.split("Prompt-Tokens:")[1].strip())
        elif "Generierte Token:" in zeile:
            gen_ids = eval(zeile.split("Generierte Token:")[1].strip())
    return prompt_ids, gen_ids


def main():
    import torch

    max_tokens = int(sys.argv[1]) if len(sys.argv) > 1 else 12
    print(f"=== Qualitativer Benchmark: {MODEL_NAME} ===")
    print(f"Artefakt: {ARTIFACTS_DIR.name}   |   Referenz: {MODEL_DIR.name} (BF16)")
    print(f"Greedy, {max_tokens} Token je Prompt\n")

    model, tok = load_reference_model(MODEL_DIR)

    gleiche_texte = 0
    top1_treffer, top1_gesamt = 0, 0

    for prompt in PROMPTS:
        prompt_ids, int_gen = integer_generierung(prompt, max_tokens)

        # BF16-Referenz: greedy, dieselbe Laenge, derselbe Prompt.
        ids = list(prompt_ids)
        with torch.no_grad():
            for _ in range(max_tokens):
                logits = model(input_ids=torch.tensor([ids], device=model.device)).logits
                ids.append(int(logits[0, -1].argmax()))
        hf_gen = ids[len(prompt_ids):]

        int_text = tok.decode(int_gen)
        hf_text = tok.decode(hf_gen)
        gleich = int_gen == hf_gen
        gleiche_texte += gleich

        # Uebereinstimmung Token fuer Token: bis zu welcher Position laufen
        # beide Pfade gleich? Das trennt "voellig anderer Text" von "weicht
        # erst spaet ab" — und kostet keinen zusaetzlichen Modell-Ladevorgang
        # (eine fruehere Fassung startete pro Praefix einen Subprozess und
        # lud dabei jedes Mal das 15-GB-Modell neu).
        gleich_bis = 0
        for a, b in zip(int_gen, hf_gen):
            if a != b:
                break
            gleich_bis += 1
        top1_treffer += gleich_bis
        top1_gesamt += len(hf_gen)

        print(f"Prompt : {prompt!r}")
        print(f"  int  : {int_text!r}")
        print(f"  bf16 : {hf_text!r}")
        print(f"  {'IDENTISCH' if gleich else f'gleich bis Token {gleich_bis}/{len(hf_gen)}'}\n")

    print("=" * 62)
    print(f"Identische Generierungen : {gleiche_texte}/{len(PROMPTS)}")
    if top1_gesamt:
        print(f"Token bis zur Abweichung : {top1_treffer}/{top1_gesamt} "
              f"({top1_treffer/top1_gesamt:.1%} deckungsgleich)")


if __name__ == "__main__":
    main()
