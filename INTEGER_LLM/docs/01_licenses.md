# Lizenzübersicht

| Komponente | Lizenz | Nutzung |
|---|---|---|
| Qwen2.5-0.5B (Basis-Variante) | Apache 2.0 | Modellgewichte |
| Hugging Face Transformers | Apache 2.0 | Offline-Kalibrierung |
| Safetensors | Apache 2.0 | Gewicht-Export |
| Eigener Code | PolyForm Shield License 1.0.0 | Integer-Inferenzsystem |

## Hinweis
- Alle HF-Abhängigkeiten dürfen **nur in der Kalibrierung** genutzt werden.
- Der Inferenzpfad hat keine Python/HF-Abhängigkeiten.
- Die Lizenzlage des Basismodells (Qwen2.5) für quantisierte Ableitungen
  ist als nicht-technischer Punkt weiterhin offen (Stand 2026-08-12);
  die Code-Lizenz (PolyForm Shield License 1.0.0, siehe `LICENSE.md` im
  Repository-Wurzelverzeichnis) ist davon unabhängig.
