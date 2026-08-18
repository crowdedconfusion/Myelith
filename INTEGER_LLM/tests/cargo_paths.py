#!/usr/bin/env python3
"""
Auflösung von Cargo-Ausgabepfaden für die Python-Tests.

**Warum es das gibt:** Die Tests riefen die gebauten Binaries über fest
verdrahtete Pfade auf (`runtime/target/release/integer-llm-runtime`).
Seit alle Crates in ein gemeinsames `target-shared/` bauen (siehe
`.cargo/config.toml` — das sparte 23,8 GB auf 2,1 GB), lagen die
Binaries woanders, und vier Tests scheiterten mit `FileNotFoundError`.

Statt den Pfad an sieben Stellen erneut fest zu verdrahten, fragt dieses
Modul die tatsächlich gültigen Orte in dieser Reihenfolge ab:

1. `CARGO_TARGET_DIR` — hat in Cargo Vorrang vor allem anderen, also
   auch hier. Die CI setzt sie.
2. `<repo>/target-shared/` — die lokale Voreinstellung aus
   `.cargo/config.toml`.
3. `<crate>/target/` — die Cargo-Voreinstellung, falls jemand ohne die
   Konfigurationsdatei baut.

Damit funktionieren die Tests in allen drei Fällen, ohne dass jemand
etwas umstellen muss.
"""

import os
from pathlib import Path

# INTEGER_LLM/tests/ -> INTEGER_LLM -> Repository
INTEGER_LLM = Path(__file__).resolve().parent.parent
REPO_ROOT = INTEGER_LLM.parent


def target_dirs(crate: str) -> list[Path]:
    """Mögliche Ausgabeverzeichnisse eines Crates, in Prüfreihenfolge."""
    kandidaten: list[Path] = []

    env = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if env:
        p = Path(env)
        # Relativ ist relativ zum Crate-Verzeichnis (Cargo-Verhalten).
        kandidaten.append(p if p.is_absolute() else INTEGER_LLM / crate / p)

    kandidaten.append(REPO_ROOT / "target-shared")
    kandidaten.append(INTEGER_LLM / crate / "target")
    return kandidaten


def binary(crate: str, name: str, profile: str = "release") -> Path:
    """
    Pfad zu einem gebauten Binary.

    Liefert den ersten Ort, an dem die Datei tatsächlich liegt. Existiert
    sie nirgends, wird der wahrscheinlichste Pfad zurückgegeben — damit
    die Fehlermeldung des Aufrufers einen brauchbaren Pfad nennt statt
    eines leeren Werts.
    """
    kandidaten = [d / profile / name for d in target_dirs(crate)]
    for pfad in kandidaten:
        if pfad.exists():
            return pfad
    return kandidaten[0]


def fehlt_hinweis(crate: str, name: str, profile: str = "release") -> str:
    """Hilfetext, wenn ein Binary fehlt — nennt alle geprüften Orte."""
    orte = "\n".join(f"    {d / profile / name}" for d in target_dirs(crate))
    return (
        f"Binary '{name}' nicht gefunden. Geprüfte Orte:\n{orte}\n"
        f"  Bauen mit:  cd INTEGER_LLM/{crate} && cargo build --{profile}"
    )


if __name__ == "__main__":
    # Selbstprüfung: zeigt, wo die Binaries gesucht und gefunden werden.
    for crate, name in [
        ("runtime", "integer-llm-runtime"),
        ("runtime", "perplexity_probe"),
        ("pipeline", "integer-llm-pipeline"),
    ]:
        p = binary(crate, name)
        print(f"{'OK ' if p.exists() else 'FEHLT'}  {crate}/{name}\n       {p}")
