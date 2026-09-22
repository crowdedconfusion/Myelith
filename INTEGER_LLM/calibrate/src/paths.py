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
    """Artefakt-Verzeichnis eines bestimmten Modells, z. B. artifacts/myelith-0.6b."""
    return artifacts_dir() / model_name


# ⚑ **Die Quellmodelle liegen an der Wurzel, nicht unter INTEGER_LLM**
# (Umzug 2026-09-21). Alle fremden Gewichte dieses Projekts stehen unter
# `MODELS/`, nach Rubrik getrennt; `llm/` ist die Rubrik der
# Quellmodelle, aus denen die Artefakte entstehen.
MODELS_DIR = "MODELS"
MODELS_LLM_DIR = "llm"


def repo_root() -> Path:
    """
    Die Wurzel dieses Repositoriums, aus der eigenen Dateitiefe gerechnet.

    ⚑ **Nicht aus dem Arbeitsverzeichnis**, anders als bei den
    Artefakten. Der Grund ist der Umzug: `artifacts/` liegt weiter
    unterhalb von `INTEGER_LLM`, von wo aus der Bau laeuft, `MODELS/`
    aber an der Wurzel darueber. Ein relativer Pfad waere hier ein
    `../MODELS/llm` und damit eine Wette darauf, aus welchem Verzeichnis
    jemand aufruft.

    ⚠️ **Wer diese Datei verschiebt, zieht `parents[3]` mit**, sonst
    zeigt sie stillschweigend auf den falschen Baum.
    """
    return Path(__file__).resolve().parents[3]


def models_dir() -> Path:
    """
    Aufgeloester Pfad zur Rubrik der Quellmodelle, `MODELS/llm`.

    Bewusst **ohne** Env-Var-Ueberschreibung: Ein Quellmodell ist keine
    Betriebseinstellung, sondern eine Voraussetzung des Baus, und ein
    zweiter Ablageort waere ein zweiter Ort fuer dieselbe Angabe.
    """
    return repo_root() / MODELS_DIR / MODELS_LLM_DIR


def local_model_dir(model_name: str) -> Path:
    """
    Verzeichnis eines lokalen Modell-Snapshots unter `MODELS/llm`, z. B.
    `MODELS/llm/Qwen3-0.6B`. Schlägt mit einem klaren Hinweis fehl, falls
    der Snapshot fehlt: calibrate laedt ausschliesslich von dort
    (reproduzierbare Herkunft), nie aus dem impliziten
    Hugging-Face-Cache.
    """
    path = models_dir() / model_name
    if not path.is_dir():
        raise FileNotFoundError(
            f"{path} fehlt. Quellmodell zuerst mit "
            "INTEGER_LLM/scripts/fetch_model.sh holen und die Revision in "
            "MODELS/llm/KATALOG.json eintragen."
        )
    return path
