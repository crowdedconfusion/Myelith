# Phase 0: Anforderungen, Definitionen, Compliance

## Zielhardware
- **CPU:** x86_64 (AVX2, AVX-512), ARM64 (Neon)
- **GPU:** Optional, nur wenn Integer-Only-Pfade verfügbar
- **NPU:** Nicht im initialen Scope

## Determinismusgrad
1. **Bit-identische Logits** auf allen Zielplattformen
2. **Bit-identische Token-Sequenz** bei greedy Decoding
3. **Bit-identische interne Tensoren** an Shard-Grenzen

## „Fully Integer“ Definition
- Keine float/double/f32/f64 Operationen im Inferenzpfad
- Keine impliziten Typ-Promotions zu Float
- Keine LUT-Generierung zur Laufzeit
- Kein sqrt/exp/log/sin/cos aus libm im Hot-Path

## theta_v (Numerischer Vertrag)
- Gewichte, Skalen, Shifts, Rundungsregeln, LUTs, Polynome, Sampling-Regel, PRNG, KV-Cache-Format, Pipeline-Manifest
- Versionierung via SHA-256 über kanonisches JSON

## Lizenzprüfung
- Qwen2.5: Apache 2.0 — die Lizenzlage für quantisierte Ableitungen ist
  als nicht-technischer Punkt weiterhin offen (Stand 2026-08-12)
- Eigener Code: PolyForm Shield License 1.0.0 (siehe `LICENSE.md` im
  Repository-Wurzelverzeichnis)

## Akzeptanzkriterien
- Perplexity < 15.0 auf WikiText-2 (Qwen2.5-0.5B)
  — **Stand 2026-08-12 (überholt):** Dieses Phase-0-Absolutziel stammt aus
  der Zeit vor der Kalibrierung und wurde nie konsensrelevant festgelegt.
  Maßgeblich war das relative Kriterium des Entscheidungspunkts 12.21
  (max. +5 % relativer Anstieg gegenüber der BF16-Baseline); es ist mit
  15,59 vs. 14,95 = +4,29 % ERFÜLLT (Protokoll:
  `eval/results/decision_12-21.md`, Zusammenfassung:
  `docs/02_empirischer_beleg_bit-exakte-inferenz.md`).
- Bit-Exact-Tests auf x86_64 und ARM64 identisch
  — **Stand 2026-08-12 (teilweise offen):** Auf ARM64 ist Bit-Identität
  über unabhängige Läufe nachgewiesen; der Cross-Hardware-Nachweis
  (x86_64, GPU-Generationen) setzt die Backends und GPU-Zugang voraus.

## Exit-Kriterium
> Ein schriftliches Requirements-Dokument ist abgenommen.
