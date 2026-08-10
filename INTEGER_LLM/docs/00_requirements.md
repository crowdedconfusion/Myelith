# Phase 0: Anforderungen, Definitionen, Compliance

## Zielhardware
- **CPU:** x86_64 (AVX2, AVX-512), ARM64 (Neon)
- **GPU:** Optional, nur wenn Integer-Only-Pfade verfuegbar
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
- Versionierung via SHA-256 ueber kanonisches JSON

## Lizenzpruefung
- Qwen2.5: Apache 2.0
- Eigener Code: [TBD]

## Akzeptanzkriterien
- Perplexity < 15.0 auf WikiText-2 (Qwen2.5-0.5B)
- Bit-Exact-Tests auf x86_64 und ARM64 identisch

## Exit-Kriterium
> Ein schriftliches Requirements-Dokument ist abgenommen.
