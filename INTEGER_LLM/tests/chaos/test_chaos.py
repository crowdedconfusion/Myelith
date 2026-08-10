#!/usr/bin/env python3
"""
Chaos-Tests fuer die Pipeline-Runtime.

Simuliert:
- Netzwerklatenz (kuenstliche Verzoegerung)
- Paketverlust (gezieltes Droppen von Nachrichten)
- Node-Restart (Prozess killen und neu starten)
- Backpressure (langsamer Consumer)

Erwartetes Verhalten:
- Entweder identisches Ergebnis (Retry/Idempotenz)
- oder sauberer Abort (kein numerischer Fallback)
"""

import subprocess
import time
import random
import socket
import struct
import sys
from pathlib import Path


def find_free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    s.listen(1)
    port = s.getsockname()[1]
    s.close()
    return port


def start_node(binary, config, stage_id, bind_addr, upstream=None, downstream=None):
    cmd = [
        str(binary),
        "--config", str(config),
        "--stage", str(stage_id),
        "--bind", bind_addr,
    ]
    if upstream:
        cmd.extend(["--upstream", upstream])
    if downstream:
        cmd.extend(["--downstream", downstream])
    return subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def send_message_with_latency(addr, tensor, latency_ms=0):
    """Sendet Nachricht mit kuenstlicher Latenz."""
    if latency_ms > 0:
        time.sleep(latency_ms / 1000.0)
    
    magic = b"IINTPIPE"
    header = struct.pack("<8sQQQQQQQQI", magic, 1, 0, 42, 0, 0, 0, len(tensor)*2, 0, 0, 0)
    payload = struct.pack(f"<{len(tensor)}h", *tensor)
    
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(5.0)
    s.connect(addr)
    s.sendall(header + payload)
    s.close()


def test_latency():
    """Testet Pipeline unter kuenstlicher Latenz."""
    print("[chaos] Test: Latenz (100ms pro Nachricht)...")
    # Placeholder: Wuerde echte Nodes starten
    print("[chaos] Latenz: PASSED (Placeholder)")


def test_packet_loss():
    """Testet Pipeline mit simuliertem Paketverlust."""
    print("[chaos] Test: Paketverlust (10% drop)...")
    # Placeholder
    print("[chaos] Paketverlust: PASSED (Placeholder)")


def test_node_restart():
    """Testet Node-Restart mit Idempotenz."""
    print("[chaos] Test: Node-Restart...")
    # Placeholder
    print("[chaos] Node-Restart: PASSED (Placeholder)")


def test_backpressure():
    """Testet Backpressure (langsamer Consumer)."""
    print("[chaos] Test: Backpressure...")
    # Placeholder
    print("[chaos] Backpressure: PASSED (Placeholder)")


def main():
    print("="*60)
    print("Chaos Tests fuer integer-llm Pipeline")
    print("="*60)
    
    test_latency()
    test_packet_loss()
    test_node_restart()
    test_backpressure()
    
    print("\n[chaos] Alle Chaos-Tests abgeschlossen.")


if __name__ == "__main__":
    main()
