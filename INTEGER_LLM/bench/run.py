#!/usr/bin/env python3
"""Durchsatzmessung je Backend und Operation (Fahrplan 12.64/12.65).

Misst Prefill- und Decode-Durchsatz für jedes verfügbare Backend und
vergleicht sie gegen die Gleitkomma-Referenz derselben Maschine.

## Warum dieser Benchmark eine Gleichheitsprüfung enthält

Ein Backend, das schneller ist und etwas anderes rechnet, ist kein
schnelleres Backend — es ist ein zweites Modell. In einem Netz, dessen
Konsens auf Bitgleichheit beruht (Whitepaper Kap. 6.2), wäre ein solches
Backend nicht bloß ungenau, sondern **konsensbrechend**: Ein Miner, der
es einsetzt, würde beim Redundanzvergleich als fehlerhaft erscheinen und
geslasht — oder, schlimmer, er würde ehrliche Knoten in einen Streit
ziehen, den beide verlieren können.

`bench_probe` gibt deshalb neben den Zeiten einen `decode_digest` aus.
Der Benchmark prüft, dass **alle** Backends denselben Wert liefern, und
verweigert das Ergebnis, wenn nicht. Eine Tabelle mit Tokens/s, in der
die Spalten verschiedene Ausgaben beschreiben, wäre wertlos.

Der Wert deckt die **Logits jedes Schritts** ab, nicht nur die erzeugten
Token. Bis 2026-08-22 stand hier `decode_hash`, ein Hash über die Token
allein, und der ist für diese Frage zu grob: Ein Token ist ein Argmax
über `vocab_size` Zahlen und ändert sich erst, wenn deren Rangfolge
kippt. Gemessen an Qwen2.5-0,5B blieb er unverändert, während 0,1 % der
Bytes eines Tensors verschoben waren und das Modell nachweislich andere
Zahlen rechnete (Fund 36). Ein konsensbrechendes Backend hätte sich
genau so verhalten können.

## Skalierung ist der eigentliche Zweck

Das Modell kommt aus `INTEGER_LLM_MODEL` und ist nirgends fest verdrahtet.
Die Zielgrößenordnung des Projekts (Kap. 4.1) liegt um Größenordnungen
über den heute gemessenen Modellen; dieser Benchmark soll auf dem
nächstgrößeren Dense-Modell **unverändert** laufen. Deshalb wird zu jedem
Lauf die Artefaktgröße mitgeschrieben — ohne sie ist eine Tokens/s-Zahl
nicht einordenbar, und die Skalierungskurve ist genau das, was vor einem
Launch gebraucht wird.

## Benutzung

```
python3 bench/run.py                          # Referenz-Backend, 0,5B
INTEGER_LLM_MODEL=qwen2.5-7b python3 bench/run.py
python3 bench/run.py --backends reference,cpu-simd
python3 bench/run.py --no-fp                  # ohne Gleitkomma-Vergleich
```

Für den Gleitkomma-Vergleich (12.65) wird `torch`/`transformers`
gebraucht — also die Kalibrier-Umgebung:

```
./calibrate/.venv/bin/python bench/run.py
```

Ohne sie läuft alles Übrige durch und der Vergleich wird mit Begründung
übersprungen; die Ganzzahl-Messung ist davon unabhängig.

Ergebnisse landen als JSON unter `bench/results/`.
"""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARTIFACT_ROOT = ROOT / "artifacts"
RESULTS = Path(__file__).resolve().parent / "results"

# Binary-Pfade über das bestehende Modul auflösen, nicht fest verdrahten:
# Alle Crates bauen nach `target-shared/` (siehe `.cargo/config.toml`),
# und `CARGO_TARGET_DIR` hat Vorrang. Genau dafür gibt es cargo_paths —
# eine zweite Auflösungslogik wäre der Fehler aus Fund A6.
sys.path.insert(0, str(ROOT / "tests"))
import cargo_paths  # noqa: E402

# Modellwahl und Pfadauflösung kommen aus derselben Quelle wie
# Kalibrierung und Perplexitätsmessung (`calibrate/src/model_configs.py`).
# Zwei Mechanismen für dieselbe Entscheidung wären zwei Wahrheiten — und
# ein Benchmark, dessen Artefakt und Referenzmodell auf verschiedene
# Modelle zeigen, fällt nicht auf, sondern liefert stillschweigend Unsinn.
sys.path.insert(0, str(ROOT / "calibrate"))
from src.model_configs import get_export_model_config  # noqa: E402

MODEL = os.environ.get("INTEGER_LLM_MODEL", "").strip() or "qwen2.5-0.5b"
_CONFIG = get_export_model_config(MODEL)
HF_MODEL_DIR = ROOT / "models" / _CONFIG["hf_model_id"].split("/")[-1]
PROMPT = "Die Hauptstadt von Frankreich ist"
DECODE_TOKENS = 32

# Backends, die überhaupt in Frage kommen. `reference` ist immer dabei —
# es ist der numerische Vertrag, gegen den alles andere geprüft wird.
ALLE_BACKENDS = ["reference", "cpu-simd", "cuda", "rocm"]


def artefakt_dir():
    return ARTIFACT_ROOT / MODEL


def artefakt_groesse_bytes(d):
    """Summe aller Artefaktdateien — ohne sie ist Tokens/s nicht einordenbar."""
    return sum(f.stat().st_size for f in d.rglob("*") if f.is_file())


def backend_verfuegbar(backend):
    """Kann dieses Backend auf dieser Maschine gebaut werden?

    Wir raten nicht, sondern versuchen den Build. Ein Backend als
    „nicht verfügbar" zu überspringen, weil eine Heuristik das meint,
    hätte auf der falschen Maschine eine Lücke im Messprotokoll
    hinterlassen — und die fällt erst auf, wenn jemand die Zahlen
    braucht.
    """
    if backend in ("cuda", "rocm"):
        # Delegations-Stubs: sie kompilieren, brauchen aber die
        # jeweilige Toolchain im Pfad, sonst ist die Messung eine
        # Referenz-Messung unter falschem Namen.
        werkzeug = "nvcc" if backend == "cuda" else "hipcc"
        if shutil.which(werkzeug) is None:
            return False, f"{werkzeug} nicht im Pfad"
    return True, None


def baue(backend):
    """Baut `bench_probe` mit dem angegebenen Backend-Feature."""
    cmd = [
        "cargo", "build", "--release", "--quiet",
        "--manifest-path", str(ROOT / "runtime" / "Cargo.toml"),
        "--bin", "bench_probe",
        "--no-default-features", "--features", backend,
    ]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, r.stderr.strip()[-500:]
    binary = cargo_paths.binary("runtime", "bench_probe")
    if not binary.exists():
        return None, cargo_paths.fehlt_hinweis("runtime", "bench_probe")
    return binary, None


def messe(binary, artefakte, decode_tokens):
    """Führt eine Messung aus und liefert die Kennzahlen als dict."""
    r = subprocess.run(
        [str(binary), str(artefakte), PROMPT, str(decode_tokens)],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        return None, r.stderr.strip()[-500:]
    werte = {}
    for zeile in r.stdout.splitlines():
        teile = zeile.split(None, 1)
        if len(teile) == 2:
            k, v = teile
            try:
                werte[k] = float(v) if "." in v else int(v)
            except ValueError:
                werte[k] = v
    return werte, None


def fp_referenz(artefakte, decode_tokens):
    """Durchsatz der Gleitkomma-Referenz auf derselben Maschine (12.65).

    Bewusst dasselbe Modell in BF16 über HuggingFace — nicht eine
    fremde Implementierung. Verglichen werden soll der Preis der
    Ganzzahligkeit, nicht der Abstand zu einem anderen Projekt.
    """
    try:
        import torch
        from transformers import AutoModelForCausalLM, AutoTokenizer
    except ImportError as e:
        return None, f"torch/transformers nicht verfügbar ({e})"

    ref = HF_MODEL_DIR
    if not ref.exists():
        return None, f"Referenzmodell fehlt: {ref}"

    tok = AutoTokenizer.from_pretrained(str(ref))
    modell = AutoModelForCausalLM.from_pretrained(
        str(ref), torch_dtype=torch.bfloat16, device_map=None
    )
    modell.eval()
    ids = tok(PROMPT, return_tensors="pt").input_ids

    with torch.no_grad():
        # Prefill
        t0 = time.perf_counter()
        out = modell(ids, use_cache=True)
        prefill = time.perf_counter() - t0

        # Decode, greedy — dieselbe Strategie wie im Integerpfad.
        past = out.past_key_values
        naechstes = out.logits[:, -1, :].argmax(-1, keepdim=True)
        t0 = time.perf_counter()
        for _ in range(decode_tokens):
            out = modell(naechstes, past_key_values=past, use_cache=True)
            past = out.past_key_values
            naechstes = out.logits[:, -1, :].argmax(-1, keepdim=True)
        decode = time.perf_counter() - t0

    return {
        "prompt_tokens": int(ids.shape[1]),
        "prefill_ms": prefill * 1000.0,
        "prefill_tokens_per_s": ids.shape[1] / prefill,
        "decode_tokens": decode_tokens,
        "decode_ms": decode * 1000.0,
        "decode_tokens_per_s": decode_tokens / decode,
    }, None


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--backends", default=",".join(ALLE_BACKENDS),
                   help="Kommaliste; Vorgabe: alle")
    p.add_argument("--decode-tokens", type=int, default=DECODE_TOKENS)
    p.add_argument("--no-fp", action="store_true",
                   help="Gleitkomma-Vergleich überspringen")
    args = p.parse_args()

    artefakte = artefakt_dir()
    if not artefakte.exists():
        print(f"SKIP: Artefakte fehlen: {artefakte}")
        return 0

    groesse = artefakt_groesse_bytes(artefakte)
    print(f"=== Durchsatz-Benchmark: {MODEL} ===")
    print(f"Artefakt: {artefakte}  ({groesse / 1e9:.2f} GB)")
    print(f"Maschine: {platform.machine()} / {platform.system()}")
    print(f"Prompt: {PROMPT!r}, Decode: {args.decode_tokens} Token")
    print()

    ergebnisse = {}
    hashes = {}
    for backend in [b.strip() for b in args.backends.split(",") if b.strip()]:
        ok, grund = backend_verfuegbar(backend)
        if not ok:
            print(f"  {backend:<10} übersprungen — {grund}")
            continue
        binary, fehler = baue(backend)
        if binary is None:
            print(f"  {backend:<10} Build fehlgeschlagen — {fehler.splitlines()[-1] if fehler else '?'}")
            continue
        werte, fehler = messe(binary, artefakte, args.decode_tokens)
        if werte is None:
            print(f"  {backend:<10} Messung fehlgeschlagen — {fehler}")
            continue
        ergebnisse[backend] = werte
        # **Der starke Wert, nicht der Token-Hash** (Fund 36, 2026-08-22).
        # `decode_hash` deckt nur die erzeugten Token ab, also eine
        # Argmax-Entscheidung ueber vocab_size Zahlen; gemessen an 0,5B
        # blieb er unveraendert, als 0,1 % der Bytes eines Tensors
        # verschoben wurden und das Modell nachweislich andere Zahlen
        # rechnete. Genau solche Abweichungen soll diese Pruefung finden.
        #
        # Rueckfall auf `decode_hash` nur, wenn ein altes Binary den
        # neuen Wert nicht liefert, und dann mit sichtbarem Vermerk: eine
        # stillschweigend schwaechere Pruefung waere schlimmer als eine
        # fehlende.
        stark = werte.get("decode_digest")
        if stark is None:
            print(f"  {backend:<10} HINWEIS: kein decode_digest, Pruefung faellt auf "
                  f"decode_hash zurueck (nur Token, siehe Fund 36)")
            stark = werte.get("decode_hash")
        hashes[backend] = stark
        print(f"  {backend:<10} Prefill {werte['prefill_tokens_per_s']:8.2f} tok/s   "
              f"Decode {werte['decode_tokens_per_s']:7.2f} tok/s")

    if not ergebnisse:
        print("\nKein Backend gemessen.")
        return 1

    # Die Gleichheitsprüfung. Ohne sie beschreiben die Spalten oben
    # möglicherweise verschiedene Modelle.
    print()
    eindeutig = set(hashes.values())
    if len(eindeutig) == 1:
        print(f"Bitgleichheit über alle Backends: OK  (decode_digest {hashes[list(hashes)[0]]})")
        print("  Der Wert deckt die Logits jedes Schritts ab, nicht nur die")
        print("  erzeugten Token: Verglichen werden die gerechneten Zahlen.")
    else:
        print("BITGLEICHHEIT VERLETZT — die Backends rechnen Verschiedenes:")
        for b, h in hashes.items():
            print(f"    {b:<10} {h}")
        print("\nDie Durchsatzzahlen sind damit wertlos: ein schnelleres Backend,")
        print("das etwas anderes rechnet, ist kein schnelleres Backend. In einem")
        print("Netz mit Bitgleichheits-Konsens wäre es konsensbrechend.")
        return 1

    fp = None
    if not args.no_fp:
        fp, grund = fp_referenz(artefakte, args.decode_tokens)
        if fp is None:
            print(f"\nGleitkomma-Vergleich übersprungen — {grund}")
        else:
            print()
            print(f"  {'bf16 (HF)':<10} Prefill {fp['prefill_tokens_per_s']:8.2f} tok/s   "
                  f"Decode {fp['decode_tokens_per_s']:7.2f} tok/s")
            schnellstes = max(ergebnisse.values(), key=lambda w: w["decode_tokens_per_s"])
            faktor = schnellstes["decode_tokens_per_s"] / fp["decode_tokens_per_s"]
            print(f"\n  Integer/Gleitkomma (Decode): Faktor {faktor:.2f}")
            print("  Einordnung: Der Integerpfad ist heute eine Referenzimplementierung")
            print("  ohne Kernel-Optimierung; die Zahl misst den aktuellen Stand, nicht")
            print("  die erreichbare Grenze.")

    RESULTS.mkdir(exist_ok=True)
    ziel = RESULTS / f"{MODEL}_{platform.machine()}.json"
    ziel.write_text(json.dumps({
        "model": MODEL,
        "artifact_bytes": groesse,
        "machine": platform.machine(),
        "system": platform.system(),
        "prompt": PROMPT,
        "decode_tokens": args.decode_tokens,
        "backends": ergebnisse,
        "decode_digest": hashes[list(hashes)[0]],
        "digest_umfang": "logits+token",
        "float_reference": fp,
    }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"\nGeschrieben: {ziel.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
