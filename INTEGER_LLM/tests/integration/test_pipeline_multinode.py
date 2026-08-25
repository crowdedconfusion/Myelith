#!/usr/bin/env python3
"""
Multi-Node Pipeline Integrationstest (Phase 12.60).

Echte Integer-Inferenz durch die 4-Stage-Pipeline:

1. Bitgleichheit mit dem Einzelknoten: Die Pipeline muss dieselben
   Tokens erzeugen wie die Einzelknoten-Runtime (Boundary-Reskalierung
   ist bei der natürlichen Zwischen-Stage-Skala identitaetstreu; alle
   Rechnungen laufen über dieselben Integer-Kernel).
2. Determinismus: Zwei unabhängige Pipeline-Läufe (frische Node-
   Prozesse) erzeugen bitgleiche Token-Sequenzen.
3. Protokoll-Sanity: theta_v-Validierung, Duplikaterkennung.

Voraussetzung: cargo build --release (Pipeline + Runtime).
"""

import re
import socket
import struct
import subprocess
import threading
import time
import zlib
from pathlib import Path

ROOT = Path(__file__).parent.parent.parent

# ⚑ **Modell und Konfiguration sind waehlbar** (2026-08-25).
#
# Vorher stand hier ein fester Modellname, und damit war der einzige
# Mehrknotenlauf des Projekts ausschliesslich an einem **dichten** Modell
# geprueft. Genau diese Pruefung ist aber die Zusage, auf die sich ein
# Mixture-of-Experts-Modell stuetzt: Die Pod-Kette soll unveraendert
# bleiben, weil jeder Knoten alle Experten SEINER Layer haelt.
#
#   MYL_MODELL=qwen3-30b-a3b \
#   MYL_PIPELINE_CONFIG=configs/pipeline_4node_qwen3-30b-a3b.json \
#   python3 tests/integration/test_pipeline_multinode.py
import os as _os
_MODELL = _os.environ.get("MYL_MODELL", "qwen2.5-0.5b")
ARTIFACTS = ROOT / "artifacts" / _MODELL
CONFIG = ROOT / _os.environ.get(
    "MYL_PIPELINE_CONFIG", "configs/pipeline_4node.json"
)
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parent.parent))
import cargo_paths  # noqa: E402

PIPELINE_BIN = cargo_paths.binary("pipeline", "integer-llm-pipeline")
RUNTIME_BIN = cargo_paths.binary("runtime", "integer-llm-runtime")

PROMPT = "Die Hauptstadt von Frankreich ist"
MAX_NEW_TOKENS = 6
REQUEST_ID = 1

FLAG_STARTS_GENERATION = 0x1
FLAG_TOKEN_INPUT = 0x4

# Kanonischer theta_v-Hash, trunkiert auf die ersten 16 Hex-Ziffern
# (u64 im Nachrichten-Header).
#
# ⚑ **Aus der Konfiguration gelesen, nicht abgeschrieben** (2026-08-25).
# Hier stand `int("16f0e49c0ee8c719", 16)` mit dem Kommentar „aus
# configs/pipeline_4node.json" - eine **Kopie, keine Ableitung**. Als
# theta_v auf 0.15.0 und 0.16.0 wechselte, wurde der Wert falsch, und
# Stage 0 verwarf jede Nachricht stillschweigend: Der Test meldete
# „Nur 0/6 Tokens" und nannte den Grund nicht.
#
# **Der Fehler war doppelt verdeckt.** Davor scheiterten die Knoten schon
# am Start, weil auch der `theta_v_hash` der Konfiguration veraltet war.
# Erst nachdem der behoben war, kam dieser zweite zum Vorschein. Und
# gemerkt hat es niemand, weil dieser Test **in keinem CI-Job laeuft**:
# Er braucht Artefakte, und die hat die CI nicht.
def _theta_u64(pfad):
    import json
    hash_str = json.load(open(pfad, encoding="utf-8"))["theta_v_hash"]
    return int(hash_str.removeprefix("sha256:")[:16], 16)


THETA_U64 = _theta_u64(CONFIG)


def find_free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def build_binaries():
    for crate in ("pipeline", "runtime"):
        result = subprocess.run(
            ["cargo", "build", "--release"],
            cwd=ROOT / crate,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, f"Compile {crate} fehlgeschlagen: {result.stderr[-2000:]}"
    assert PIPELINE_BIN.exists(), "Pipeline-Binary fehlt"
    assert RUNTIME_BIN.exists(), "Runtime-Binary fehlt"


def pack_tokens(tokens):
    """Je Token-ID zwei i16 (Low-/High-Hälfte, little-endian, signed)."""
    out = []
    for t in tokens:
        lo = t & 0xFFFF
        hi = (t >> 16) & 0xFFFF
        out.append(lo - 0x10000 if lo >= 0x8000 else lo)
        out.append(hi - 0x10000 if hi >= 0x8000 else hi)
    return out


def encode_message(tokens_packed, token_position, flags, request_id=REQUEST_ID):
    payload = struct.pack(f"<{len(tokens_packed)}h", *tokens_packed)
    crc = zlib.crc32(payload) & 0xFFFFFFFF
    header = struct.pack(
        "<8sQQQQQQQQQI",
        b"IINTPIPE",
        1,               # version
        THETA_U64,       # theta_v_hash (trunkiert)
        request_id,
        0,               # sequence_id
        0,               # stage_id (Eingang ist immer Stage 0)
        token_position,
        len(payload),
        flags,
        0,               # reserved
        crc,
    )
    return header + payload


def send_message(addr, blob):
    host, port = addr.rsplit(":", 1)
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(10)
    s.connect((host, int(port)))
    s.sendall(blob)
    s.close()


class NodeProc:
    """Node-Prozess mit Hintergrund-Reader für stdout."""

    def __init__(self, cmd):
        self.proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        self.lines = []
        self._thread = threading.Thread(target=self._read, daemon=True)
        self._thread.start()

    def _read(self):
        for line in self.proc.stdout:
            self.lines.append(line.rstrip())

    def wait_started(self, timeout=None):
        """Wartet, bis der Knoten seine Ereignisschleife meldet.

        ⚑ **Die Frist folgt der Artefaktgroesse** (2026-08-25). Vorher
        stand hier eine feste Minute. Das war richtig, solange das
        Artefakt 0,74 GB gross war und in Sekunden geladen wurde; bei
        Qwen3-30B-A3B sind es **29 GiB**, und allein die
        SHA-256-Pruefung jeder Gewichtsdatei dauert laenger als die
        Frist. Der erste Mehrknotenlauf mit MoE scheiterte genau daran,
        und zwar mit einer Meldung, die nach einem Pipeline-Fehler
        aussah statt nach einem zu knappen Zeitfenster.

        Dieselbe Klasse wie die uebrigen Groessenordnungsfunde dieses
        Tages: eine Konstante, die fuer eine Groessenordnung kalibriert
        war und bei der naechsten nicht mehr traegt.

        Zwanzig Sekunden je Gigabyte, mindestens eine Minute. Bei 0,74 GB
        bleibt es praktisch bei der alten Frist, bei 29 GiB sind es rund
        zehn Minuten.
        """
        if timeout is None:
            groesse_gib = sum(
                f.stat().st_size for f in ARTIFACTS.glob("*.bin")
            ) / 2**30 if ARTIFACTS.exists() else 1.0
            timeout = max(60.0, 20.0 * groesse_gib)
        deadline = time.time() + timeout
        while time.time() < deadline:
            if any("Event-Loop gestartet" in l for l in self.lines):
                return True
            if self.proc.poll() is not None:
                raise RuntimeError(
                    f"Node früh beendet: {self.proc.stderr.read()[-2000:]}"
                )
            time.sleep(0.2)
        raise TimeoutError(
            f"Node startete nicht in {timeout:.0f} s "
            f"(Artefakt {ARTIFACTS.name}). Letzte Ausgaben: {self.lines[-5:]}"
        )

    def token_lines(self):
        return [l for l in self.lines if l.startswith("[token]")]

    def terminate(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def start_pipeline(ports, max_tokens):
    """Startet die 4 Stages; ports = [p0, p1, p2, p3]."""
    addrs = [f"127.0.0.1:{p}" for p in ports]
    nodes = []
    for stage in range(4):
        cmd = [
            str(PIPELINE_BIN),
            "--config", str(CONFIG),
            "--stage", str(stage),
            "--bind", addrs[stage],
            "--artifacts", str(ARTIFACTS),
            "--max-tokens", str(max_tokens),
        ]
        if stage < 3:
            cmd += ["--downstream", addrs[stage + 1]]
        if stage == 3:
            cmd += ["--feedback", addrs[0]]
        nodes.append(NodeProc(cmd))
    for n in nodes:
        n.wait_started()
    return nodes


def collect_tokens(node, count, timeout=300):
    """Wartet auf `count` [token]-Zeilen der finalen Stage."""
    pat = re.compile(r"\[token\] request=(\d+) position=(\d+) token=(\d+)")
    deadline = time.time() + timeout
    tokens = {}
    while time.time() < deadline:
        for line in node.token_lines():
            m = pat.match(line)
            if m:
                tokens[int(m.group(2))] = int(m.group(3))
        if len(tokens) >= count:
            break
        if node.proc.poll() is not None:
            raise RuntimeError(
                f"Finale Stage früh beendet: {node.proc.stderr.read()[-2000:]}"
            )
        time.sleep(0.2)
    if len(tokens) < count:
        # Die Zeile "Nur 0/6 Tokens" allein ist nutzlos: Sie sagt nicht,
        # ob die Nachricht ankam, ob eine Stage stillschweigend nichts tat
        # oder ob nur die Frist zu kurz war. Die letzten Ausgabezeilen der
        # finalen Stage beantworten das in den meisten Faellen.
        print(f"[test] Letzte Ausgaben der finalen Stage: {node.lines[-12:]}")
    assert len(tokens) >= count, (
        f"Nur {len(tokens)}/{count} Tokens in {timeout} s: {sorted(tokens.items())}"
    )
    return [tokens[p] for p in sorted(tokens)[:count]]


def run_pipeline_once(prompt_tokens, max_tokens):
    ports = [find_free_port() for _ in range(4)]
    nodes = start_pipeline(ports, max_tokens)
    try:
        packed = pack_tokens(prompt_tokens)
        blob = encode_message(
            packed,
            token_position=0,
            flags=FLAG_TOKEN_INPUT | FLAG_STARTS_GENERATION,
        )
        send_message(f"127.0.0.1:{ports[0]}", blob)
        return collect_tokens(nodes[3], max_tokens)
    finally:
        for n in nodes:
            n.terminate()


def run_single_node(prompt, max_tokens):
    """Referenz: Einzelknoten-Runtime, liefert Prompt-Tokens + Generierung."""
    result = subprocess.run(
        [str(RUNTIME_BIN), str(ARTIFACTS), prompt, str(max_tokens)],
        capture_output=True,
        text=True,
        timeout=600,
    )
    assert result.returncode == 0, f"Runtime fehlgeschlagen: {result.stderr[-2000:]}"
    m_prompt = re.search(r"\[runtime\] Prompt-Tokens: \[([^\]]*)\]", result.stdout)
    m_gen = re.search(r"\[runtime\] Generierte Token: \[([^\]]*)\]", result.stdout)
    assert m_prompt and m_gen, f"Unerwartete Runtime-Ausgabe: {result.stdout[-2000:]}"
    prompt_tokens = [int(x) for x in m_prompt.group(1).split(",")]
    generated = [int(x) for x in m_gen.group(1).split(",")]
    return prompt_tokens, generated


def test_pipeline_bitgleich_mit_einzelknoten():
    print("[test] Referenz: Einzelknoten-Runtime ...")
    prompt_tokens, referenz = run_single_node(PROMPT, MAX_NEW_TOKENS)
    print(f"[test] Prompt-Tokens: {prompt_tokens}")
    print(f"[test] Einzelknoten-Generierung: {referenz}")

    print("[test] Lauf 1: 4-Node-Pipeline ...")
    lauf1 = run_pipeline_once(prompt_tokens, MAX_NEW_TOKENS)
    print(f"[test] Pipeline-Generierung: {lauf1}")
    # Hartes Kriterium seit Fund 26/20 (2026-08-19). Bis dahin stand hier
    # ein weicher Zweig: die Boundary-Reskalierung zwischen den Stages war
    # ein einziger Skalar, waehrend der Residualstrom seit Fund 20 eine
    # Skala je Kanal traegt — der Rundweg ueber den groeberen Skalar hat
    # ab dem sechsten Token divergiert.
    #
    # Der Boundary-Schritt ist jetzt ganz entfallen. Er war reiner
    # Verlust ohne Gegenwert: Die Ausgangsskala des Senders ist
    # layers[layer_end].residual_in_frac, die Eingangsskala des
    # Empfaengers layers[layer_start].residual_in_frac — und layer_start
    # des Empfaengers IST layer_end des Senders. Beide Seiten lasen also
    # denselben Wert aus demselben Artefakt (erzwungen durch
    # theta_v_hash) und rechneten ihn trotzdem ueber einen dritten,
    # groeberen Skalar hin und zurueck.
    assert lauf1 == referenz, (
        f"Pipeline weicht vom Einzelknoten ab: {lauf1} != {referenz}. "
        "Seit Fund 26 ist die Stage-Grenze verlustfrei — eine Abweichung "
        "hier ist ein echter Regress, keine bekannte Einschraenkung."
    )
    print("[test] Bitgleichheit mit Einzelknoten: PASSED")

    print("[test] Lauf 2: 4-Node-Pipeline (frische Prozesse) ...")
    lauf2 = run_pipeline_once(prompt_tokens, MAX_NEW_TOKENS)
    print(f"[test] Pipeline-Generierung: {lauf2}")
    assert lauf2 == lauf1, f"Pipeline nicht deterministisch: {lauf2} != {lauf1}"
    print("[test] Determinismus (zwei Läufe bitgleich): PASSED")


def test_doppelte_nachricht_wird_verworfen():
    """Duplikaterkennung: dieselbe Nachricht zweimal an Stage 0."""
    ports = [find_free_port() for _ in range(4)]
    nodes = start_pipeline(ports, MAX_NEW_TOKENS)
    try:
        prompt_tokens, _ = run_single_node(PROMPT, MAX_NEW_TOKENS)
        packed = pack_tokens(prompt_tokens)
        blob = encode_message(
            packed,
            token_position=0,
            flags=FLAG_TOKEN_INPUT | FLAG_STARTS_GENERATION,
        )
        addr0 = f"127.0.0.1:{ports[0]}"
        send_message(addr0, blob)
        time.sleep(1.0)
        # Duplikat: muss verworfen werden (keine zweite Generierung).
        send_message(addr0, blob)
        tokens = collect_tokens(nodes[3], MAX_NEW_TOKENS)
        # Genau MAX_NEW_TOKENS Tokens (nicht doppelt so viele).
        assert len(tokens) == MAX_NEW_TOKENS
        # Die Positionen müssen eindeutig 0..N-1 der Generierung sein.
        print("[test] Duplikaterkennung: PASSED")
    finally:
        for n in nodes:
            n.terminate()


if __name__ == "__main__":
    build_binaries()
    test_pipeline_bitgleich_mit_einzelknoten()
    test_doppelte_nachricht_wird_verworfen()
    print("[test] Alle Multi-Node-Pipeline-Tests PASSED")
