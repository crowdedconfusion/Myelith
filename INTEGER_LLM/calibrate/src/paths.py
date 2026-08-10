"""
Zentrale Pfadkonstanten fuer calibrate - spiegelbildlich zu
runtime/src/paths.rs, damit Python- und Rust-Seite denselben Ablageort ohne
hartkodierte Pfade verwenden. Gleicher Env-Var-Name, gleicher Default.
"""

import os
from pathlib import Path

ARTIFACTS_DIR = "artifacts"
ARTIFACTS_DIR_ENV = "INTEGER_LLM_ARTIFACTS_DIR"


def artifacts_dir() -> Path:
    """
    Aufgeloester Pfad zum Artefakt-Verzeichnis, relativ zum aktuellen
    Arbeitsverzeichnis (wie runtime/src/paths.rs::artifacts_dir()) - siehe
    scripts/build_artifacts.sh, das vor dem Aufruf ins Repository-Root wechselt.
    """
    override = os.environ.get(ARTIFACTS_DIR_ENV)
    if override:
        return Path(override)
    return Path(ARTIFACTS_DIR)


def model_artifacts_dir(model_name: str) -> Path:
    """Artefakt-Verzeichnis eines bestimmten Modells, z. B. artifacts/qwen2.5-0.5b-instruct."""
    return artifacts_dir() / model_name
