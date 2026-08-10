# theta_v – Numerischer Vertrag

Dieses Verzeichnis enthält den kanonischen numerischen Vertrag theta_v.

## Inhalt

- `spec.json` – Kanonische Spezifikation (JSON, sortierte Keys). Der SHA-256 dieses Files ist der `theta_v_hash`.
- `schemas/` – JSON-Schemas zur Validierung (optional).
- `hashes/` – Abgeleitete Hashes der Artefakte (Gewichte, Skalen, LUTs).

## Regeln

1. `spec.json` ist die Single Source of Truth.
2. Jede Aenderung erzeugt einen neuen `theta_v_hash`.
3. Knoten lehnen Inferenz ab, wenn der Hash nicht uebereinstimmt.
4. Kein Float im Inferenzpfad – nur die hier spezifizierten Integer-Operationen.
