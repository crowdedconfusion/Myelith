#!/usr/bin/env python3
"""Layout-Unabhängigkeit: liefern 4 und 8 Shards dieselben Token?

Die Frage entscheidet den COMPUTE_PIPELINE-Entwurf „variable Knotenzahl
je Pipeline". Redundante Pods sollen dort unterschiedlich groß sein
dürfen — ein schneller Pod aus wenigen starken Knoten gegen einen
langsamen aus vielen kleinen. Das geht nur, wenn beide bitgleich
dasselbe Ergebnis liefern; sonst würde der Redundanzvergleich aus
Whitepaper Kap. 6.4 ehrliche Knoten als fehlerhaft markieren und
slashen.

**Bis 2026-08-19 war die Antwort nachweislich „nein".** Die
Boundary-Reskalierung zwischen Stages war ein einziger Skalar, während
der Residualstrom seit Fund 20 eine Skala je Kanal trägt; jede
Stage-Grenze fügte einen Rundungsschritt hinzu, und mehr Grenzen
bedeuteten mehr Verlust. Genau darauf beruhte Fund 25: Das Shard-Layout
wurde über `pipeline_hash` gebunden, weil das Ergebnis daran hing.

Seit Fund 26 ist der Boundary-Schritt ersatzlos entfallen — eine
Stage-Grenze ist rechnerisch ein No-Op. **Damit sollte das Layout
gleichgültig sein.** „Sollte" ist eine Vermutung; dieser Test macht eine
Messung daraus.

Gemessen werden **drei** Layouts:

- 4 Shards, Grenzen bei 6/12/18 (die Produktivkonfiguration),
- 8 Shards, Grenzen bei 3/6/9/12/15/18/21,
- 4 Shards **ungleichmäßig**, Grenzen bei 1/7/23 — also 1, 6, 16 und 1
  Layer.

Das dritte ist das eigentliche Argument. Die 8er-Grenzen sind ein
Superset der 4er-Grenzen; eine Übereinstimmung zwischen beiden könnte
daran hängen, dass sie ineinander aufgehen. Das ungleichmäßige Layout
fällt mit keiner der beiden zusammen.

Der Test ist zudem zweiseitig: Er belegt die Layout-Unabhängigkeit *und*
hält fest, dass alle drei Layouts weiterhin mit dem Einzelknoten
übereinstimmen. Layouts, die sich untereinander einig sind, aber
gemeinsam vom Einzelknoten abweichen, wären kein Erfolg.
"""

import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent
ARTIFACTS = ROOT / "artifacts" / "qwen2.5-0.5b"
CONFIG_4 = ROOT / "configs" / "pipeline_4node.json"
CONFIG_8 = ROOT / "configs" / "pipeline_8node.json"
CONFIG_U = ROOT / "configs" / "pipeline_uneven4node.json"

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import cargo_paths  # noqa: E402

# Die Werkzeuge des bestehenden Mehrknoten-Tests werden
# wiederverwendet, statt sie zu kopieren — eine zweite Fassung von
# pack_tokens/send_message wäre genau der Fehler aus Fund A6.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_pipeline_multinode as mn  # noqa: E402

PROMPT = "Die Hauptstadt von Frankreich ist"
MAX_NEW_TOKENS = 6


def start_pipeline(config, num_stages, ports, max_tokens):
    """Startet `num_stages` Stages gegen `config`.

    Verallgemeinerung von `test_pipeline_multinode.start_pipeline`, das
    auf vier Stufen festgelegt ist.
    """
    addrs = [f"127.0.0.1:{p}" for p in ports]
    nodes = []
    for stage in range(num_stages):
        cmd = [
            str(mn.PIPELINE_BIN),
            "--config", str(config),
            "--stage", str(stage),
            "--bind", addrs[stage],
            "--artifacts", str(ARTIFACTS),
            "--max-tokens", str(max_tokens),
        ]
        if stage < num_stages - 1:
            cmd += ["--downstream", addrs[stage + 1]]
        else:
            cmd += ["--feedback", addrs[0]]
        nodes.append(mn.NodeProc(cmd))
    for n in nodes:
        n.wait_started()
    return nodes


def run_layout(config, num_stages, prompt_tokens, max_tokens):
    """Fährt eine Pipeline mit `num_stages` Stufen und liefert die Token."""
    ports = [mn.find_free_port() for _ in range(num_stages)]
    nodes = start_pipeline(config, num_stages, ports, max_tokens)
    try:
        packed = mn.pack_tokens(prompt_tokens)
        blob = mn.encode_message(
            packed,
            token_position=0,
            flags=mn.FLAG_TOKEN_INPUT | mn.FLAG_STARTS_GENERATION,
        )
        mn.send_message(f"127.0.0.1:{ports[0]}", blob)
        return mn.collect_tokens(nodes[-1], max_tokens)
    finally:
        for n in nodes:
            n.terminate()


def test_layouts_liefern_dieselben_token():
    print("[test] Referenz: Einzelknoten-Runtime ...")
    prompt_tokens, referenz = mn.run_single_node(PROMPT, MAX_NEW_TOKENS)
    print(f"[test] Einzelknoten:  {referenz}")

    print("[test] 4-Node-Pipeline (Grenzen bei 6/12/18) ...")
    vier = run_layout(CONFIG_4, 4, prompt_tokens, MAX_NEW_TOKENS)
    print(f"[test] 4 Shards:      {vier}")

    print("[test] 8-Node-Pipeline (Grenzen bei 3/6/9/12/15/18/21) ...")
    acht = run_layout(CONFIG_8, 8, prompt_tokens, MAX_NEW_TOKENS)
    print(f"[test] 8 Shards:      {acht}")

    # Die 8er-Grenzen (3/6/9/...) sind ein Superset der 4er-Grenzen
    # (6/12/18) — eine Uebereinstimmung koennte daran haengen. Das
    # ungleichmaessige Layout faellt mit keiner der beiden zusammen und
    # ist deshalb der schaerfere Test.
    print("[test] Ungleichmaessige Pipeline (Grenzen bei 1/7/23) ...")
    ungleich = run_layout(CONFIG_U, 4, prompt_tokens, MAX_NEW_TOKENS)
    print(f"[test] 1/6/16/1 Layer: {ungleich}")

    # Die eigentliche Frage.
    assert vier == acht, (
        f"Layouts weichen ab: 4 Shards {vier} != 8 Shards {acht}. "
        "Damit waeren redundante Pods unterschiedlicher Groesse nicht "
        "vergleichbar — der Entwurf 'variable Knotenzahl' traegt nicht."
    )
    assert vier == ungleich, (
        f"Ungleichmaessiges Layout weicht ab: {ungleich} != {vier}. "
        "Die Uebereinstimmung von 4 und 8 haette dann daran gelegen, "
        "dass deren Grenzen ineinander aufgehen."
    )
    print("[test] Layout-Unabhaengigkeit (4 == 8 == ungleichmaessig): PASSED")

    # Und beide gegen den Einzelknoten — sonst waeren sich zwei falsche
    # Pipelines nur einig.
    assert vier == referenz, f"4 Shards weichen vom Einzelknoten ab: {vier} != {referenz}"
    assert acht == referenz, f"8 Shards weichen vom Einzelknoten ab: {acht} != {referenz}"
    assert ungleich == referenz, (
        f"Ungleichmaessiges Layout weicht vom Einzelknoten ab: {ungleich} != {referenz}"
    )
    print("[test] Alle drei Layouts bitgleich mit dem Einzelknoten: PASSED")


if __name__ == "__main__":
    if not ARTIFACTS.exists():
        print(f"SKIP: Artefakte fehlen: {ARTIFACTS}")
        sys.exit(0)
    mn.build_binaries()
    t0 = time.time()
    test_layouts_liefern_dieselben_token()
    print(f"[test] Alle Layout-Tests PASSED ({time.time() - t0:.0f} s)")
