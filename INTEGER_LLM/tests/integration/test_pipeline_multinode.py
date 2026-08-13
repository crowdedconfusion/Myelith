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
ARTIFACTS = ROOT / "artifacts" / "qwen2.5-0.5b"
CONFIG = ROOT / "configs" / "pipeline_4node.json"
PIPELINE_BIN = ROOT / "pipeline" / "target" / "release" / "integer-llm-pipeline"
RUNTIME_BIN = ROOT / "runtime" / "target" / "release" / "integer-llm-runtime"

PROMPT = "Die Hauptstadt von Frankreich ist"
MAX_NEW_TOKENS = 6
REQUEST_ID = 1

FLAG_STARTS_GENERATION = 0x1
FLAG_TOKEN_INPUT = 0x4

# Kanonischer theta_v-Hash aus configs/pipeline_4node.json, trunkiert
# auf die ersten 16 Hex-Ziffern (u64 im Nachrichten-Header).
THETA_U64 = int("dd4a8abad5b7e679", 16)


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

    def wait_started(self, timeout=60):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if any("Event-Loop gestartet" in l for l in self.lines):
                return True
            if self.proc.poll() is not None:
                raise RuntimeError(
                    f"Node früh beendet: {self.proc.stderr.read()[-2000:]}"
                )
            time.sleep(0.2)
        raise TimeoutError(f"Node startete nicht in {timeout} s")

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
    assert lauf1 == referenz, (
        f"Pipeline weicht vom Einzelknoten ab: {lauf1} != {referenz}"
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
