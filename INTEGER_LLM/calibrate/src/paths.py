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
    """Artefakt-Verzeichnis eines bestimmten Modells, z. B. artifacts/qwen2.5-0.5b."""
    return artifacts_dir() / model_name


MODELS_DIR = "models"


def models_dir() -> Path:
    """
    Aufgeloester Pfad zum Quellmodell-Verzeichnis, relativ zum aktuellen
    Arbeitsverzeichnis (wie runtime/src/paths.rs::MODELS_DIR; dort gibt es
    bewusst keine Env-Var-Ueberschreibung, also hier ebenfalls nicht).
    """
    return Path(MODELS_DIR)


def local_model_dir(model_name: str) -> Path:
    """
    Verzeichnis eines lokalen Modell-Snapshots unter models/, z. B.
    models/Qwen2.5-0.5B. Schlägt mit einem klaren Hinweis fehl, falls der
    Snapshot fehlt — calibrate laedt ausschliesslich aus models/
    (reproduzierbare Herkunft, siehe models/README.md), nie aus dem
    impliziten Hugging-Face-Cache.
    """
    path = models_dir() / model_name
    if not path.is_dir():
        raise FileNotFoundError(
            f"{path} fehlt. Quellmodell zuerst mit scripts/fetch_model.sh "
            "holen und die Revision in models/README.md eintragen "
            "(siehe models/README.md, Abschnitt Beschaffung)."
        )
    return path
