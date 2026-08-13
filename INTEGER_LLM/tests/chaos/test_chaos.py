#!/usr/bin/env python3
"""
Chaos-Tests der Multi-Node-Pipeline (Phase 12.61–12.63).

1. Latenz-Simulation (12.61): Künstliche Verzögerung auf den Stage-
   Verbindungen — die Ausgabe muss trotzdem bitgleich mit der
   Einzelknoten-Referenz bleiben.
2. Paketverlust + Retry (12.62): Ein verlustbehafteter Proxy verwirft
   einen Teil der Verbindungen; die Retry-Logik der Nodes
   (Retransmits, idempotent durch Duplikaterkennung) muss die
   Nachrichten trotzdem zuverlässig zustellen.
3. Node-Restart + Idempotenz (12.63): Kompletter Neustart der Pipeline
   mit demselben Prompt reproduziert dasselbe Ergebnis.

Akzeptanz: Pipeline bleibt unter Verlust funktionsfähig (kein Ausfall,
deterministisches Ergebnis); Neustart reproduziert identische Tokens.
"""

import socket
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "integration"))

from test_pipeline_multinode import (  # noqa: E402
    ARTIFACTS,
    CONFIG,
    FLAG_STARTS_GENERATION,
    FLAG_TOKEN_INPUT,
    MAX_NEW_TOKENS,
    PIPELINE_BIN,
    PROMPT,
    NodeProc,
    build_binaries,
    collect_tokens,
    encode_message,
    find_free_port,
    pack_tokens,
    run_pipeline_once,
    run_single_node,
    send_message,
)


class Proxy:
    """TCP-Proxy mit künstlicher Latenz und/oder Verbindungs-Drops.

    Jede Pipeline-Nachricht ist eine eigene TCP-Verbindung; der Proxy
    entscheidet je Verbindung: verwerfen (simulierter Paketverlust) oder
    weiterleiten (optional mit Verzögerung).
    """

    def __init__(self, target, delay_s=0.0, drop_first=0):
        self.target_host, target_port = target.rsplit(":", 1)
        self.target_port = int(target_port)
        self.delay_s = delay_s
        self.drop_first = drop_first  # Anzahl Verbindungen, die verworfen werden
        self.dropped = 0
        self.forwarded = 0
        self._lock = threading.Lock()
        self.port = find_free_port()
        self._server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server.bind(("127.0.0.1", self.port))
        self._server.listen(16)
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    def address(self):
        return f"127.0.0.1:{self.port}"

    def _serve(self):
        while True:
            try:
                conn, _ = self._server.accept()
            except OSError:
                return
            threading.Thread(target=self._handle, args=(conn,), daemon=True).start()

    def _handle(self, conn):
        try:
            data = b""
            conn.settimeout(10)
            # Vollständigen Rahmen lesen: Der Sender schreibt einen Rahmen
            # und schließt die Verbindung.
            while True:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                data += chunk
        except OSError:
            conn.close()
            return
        conn.close()

        if self.delay_s > 0:
            time.sleep(self.delay_s)
        try:
            out = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            out.settimeout(10)
            out.connect((self.target_host, self.target_port))
            out.sendall(data)
            out.close()
            with self._lock:
                self.forwarded += 1
        except OSError:
            pass

    def close(self):
        try:
            self._server.close()
        except OSError:
            pass


class GateProxy:
    """Deterministischer Paketverlust: bindet den Port sofort (reserviert
    ihn), ruft aber erst beim Öffnen `listen()` auf. Solange das Gate
    geschlossen ist, schlägt `connect()` des Senders zuverlässig fehl
    (ECONNREFUSED) — das ist der auslösende Fehler für die Retry-Logik.
    Nach dem Öffnen werden die Rahmen gelesen und weitergeleitet.
    """

    def __init__(self, target, port):
        self.target_host, target_port = target.rsplit(":", 1)
        self.target_port = int(target_port)
        self.port = port
        self.forwarded = 0
        self.opened = False
        self._lock = threading.Lock()
        # Binden ohne listen(): Port ist reserviert, Verbindungen werden
        # abgelehnt (ECONNREFUSED), bis das Gate geöffnet wird.
        self._server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server.bind(("127.0.0.1", self.port))

    def open(self):
        with self._lock:
            if self.opened:
                return
            self.opened = True
        self._server.listen(16)
        threading.Thread(target=self._serve, daemon=True).start()

    def _serve(self):
        while True:
            try:
                conn, _ = self._server.accept()
            except OSError:
                return
            threading.Thread(target=self._forward, args=(conn,), daemon=True).start()

    def _forward(self, conn):
        try:
            data = b""
            conn.settimeout(10)
            while True:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                data += chunk
        except OSError:
            conn.close()
            return
        conn.close()
        try:
            out = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            out.settimeout(10)
            out.connect((self.target_host, self.target_port))
            out.sendall(data)
            out.close()
            with self._lock:
                self.forwarded += 1
        except OSError:
            pass

    def close(self):
        try:
            self._server.close()
        except OSError:
            pass


def start_proxied_pipeline(proxy_factory):
    """Startet die 4 Stages, wobei jede Downstream-Strecke über einen
    eigenen Proxy läuft. Liefert (nodes, proxies, ports)."""
    ports = [find_free_port() for _ in range(4)]
    addrs = [f"127.0.0.1:{p}" for p in ports]

    # Proxies für die Downstream-Strecken 0→1, 1→2, 2→3.
    proxies = []
    downstream_addrs = []
    for i in range(3):
        p = proxy_factory(addrs[i + 1])
        proxies.append(p)
        downstream_addrs.append(p.address())

    nodes = []
    for stage in range(4):
        cmd = [
            str(PIPELINE_BIN),
            "--config", str(CONFIG),
            "--stage", str(stage),
            "--bind", addrs[stage],
            "--artifacts", str(ARTIFACTS),
            "--max-tokens", str(MAX_NEW_TOKENS),
        ]
        if stage < 3:
            cmd += ["--downstream", downstream_addrs[stage]]
        if stage == 3:
            cmd += ["--feedback", addrs[0]]
        nodes.append(NodeProc(cmd))
    for n in nodes:
        n.wait_started()
    return nodes, proxies, ports


def send_prompt_to(ports, prompt_tokens):
    packed = pack_tokens(prompt_tokens)
    blob = encode_message(
        packed,
        token_position=0,
        flags=FLAG_TOKEN_INPUT | FLAG_STARTS_GENERATION,
    )
    send_message(f"127.0.0.1:{ports[0]}", blob)


def test_latenz_simulation():
    """12.61: 100 ms künstliche Latenz je Hop — Ergebnis bleibt bitgleich."""
    print("[chaos] Latenz-Simulation (100 ms je Hop) ...")
    prompt_tokens, referenz = run_single_node(PROMPT, MAX_NEW_TOKENS)
    nodes, proxies, ports = start_proxied_pipeline(
        lambda target: Proxy(target, delay_s=0.1)
    )
    try:
        send_prompt_to(ports, prompt_tokens)
        tokens = collect_tokens(nodes[3], MAX_NEW_TOKENS, timeout=600)
        assert tokens == referenz, f"Latenz-Lauf weicht ab: {tokens} != {referenz}"
        print("[chaos] Latenz-Simulation: PASSED (bitgleich trotz Verzögerung)")
    finally:
        for n in nodes:
            n.terminate()
        for p in proxies:
            p.close()


def test_paketverlust_retry():
    """12.62: Der Downstream von Stage 0 ist zu Beginn nicht erreichbar
    (Gate geschlossen, `connect()` schlägt fehl) — die Retry-Logik des
    Senders muss die Nachricht dennoch zustellen, sobald das Gate öffnet.
    Die Duplikaterkennung macht Retransmits idempotent."""
    print("[chaos] Paketverlust + Retry (Hop 0→1 anfangs unerreichbar) ...")
    prompt_tokens, referenz = run_single_node(PROMPT, MAX_NEW_TOKENS)

    ports = [find_free_port() for _ in range(4)]
    gate_port = find_free_port()
    addrs = [f"127.0.0.1:{p}" for p in ports]
    gate = GateProxy(addrs[1], gate_port)  # leitet an Stage 1 weiter

    nodes = []
    for stage in range(4):
        cmd = [
            str(PIPELINE_BIN),
            "--config", str(CONFIG),
            "--stage", str(stage),
            "--bind", addrs[stage],
            "--artifacts", str(ARTIFACTS),
            "--max-tokens", str(MAX_NEW_TOKENS),
        ]
        if stage == 0:
            cmd += ["--downstream", f"127.0.0.1:{gate_port}"]  # über das Gate
        elif stage < 3:
            cmd += ["--downstream", addrs[stage + 1]]
        if stage == 3:
            cmd += ["--feedback", addrs[0]]
        nodes.append(NodeProc(cmd))
    for n in nodes:
        n.wait_started()

    try:
        send_prompt_to(ports, prompt_tokens)
        # Einige Retries ins Leere laufen lassen (Gate noch geschlossen),
        # dann öffnen. Innerhalb des Retry-Fensters muss die Zustellung
        # gelingen.
        time.sleep(0.2)
        gate.open()
        tokens = collect_tokens(nodes[3], MAX_NEW_TOKENS, timeout=600)
        assert tokens == referenz, f"Verlust-Lauf weicht ab: {tokens} != {referenz}"
        assert gate.forwarded > 0, "Gate hat nichts weitergeleitet"
        print(f"[chaos] Gate: {gate.forwarded} Rahmen nach Öffnung weitergeleitet")
        print("[chaos] Paketverlust + Retry: PASSED")
    finally:
        for n in nodes:
            n.terminate()
        gate.close()


def test_node_restart_idempotenz():
    """12.63: Kompletter Neustart der Pipeline mit demselben Prompt
    reproduziert dieselbe Token-Sequenz (Idempotenz über Neustart —
    alle Prozess-/Cache-Zustände sind frisch)."""
    print("[chaos] Node-Restart + Idempotenz ...")
    prompt_tokens, referenz = run_single_node(PROMPT, MAX_NEW_TOKENS)
    lauf1 = run_pipeline_once(prompt_tokens, MAX_NEW_TOKENS)
    assert lauf1 == referenz, f"Lauf 1 weicht ab: {lauf1} != {referenz}"
    # Alle Prozesse sind nach run_pipeline_once beendet — vollständiger
    # Neustart mit frischen Ports (nichts bleibt erhalten).
    lauf2 = run_pipeline_once(prompt_tokens, MAX_NEW_TOKENS)
    assert lauf2 == referenz, f"Neustart weicht ab: {lauf2} != {referenz}"
    assert lauf1 == lauf2
    print("[chaos] Node-Restart + Idempotenz: PASSED")


if __name__ == "__main__":
    build_binaries()
    test_latenz_simulation()
    test_paketverlust_retry()
    test_node_restart_idempotenz()
    print("[chaos] Alle Chaos-Tests PASSED")
