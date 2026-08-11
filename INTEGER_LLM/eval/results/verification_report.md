# Verifikations-Bericht INTEGER_LLM

**Stand:** 2026-08-11 · θ_v 0.10.0 · Artefakt `qwen2.5-0.5b` · Perplexität-Protokoll
`decision_12-21.md` · Auswertung nach dem RoPE/Attention-Fix (v0.12.30) und der
GPTQ-/Aktivierungs-Analyse.

Dieser Bericht dokumentiert den verifizierten Zustand des Integer-Inferenzpfads
vor der Präzisions-Entscheidung (int16 / Mischpräzision / Akzeptanz).

> **⚠️ Nachtrag 2026-08-12 (Stand nach Fund 17, v0.12.32):** Die
> Perplexitäts-Angaben unten (73,15, „+389 %", „Offene
> Präzisions-Entscheidung") sind der Diagnose-Stand VOR der Behebung von
> Fund 17 (fehlende 1/√head_dim-Attention-Skalierung). Der
> Entscheidungspunkt 12.21 ist inzwischen **AKZEPTIERT**: Perplexität
> **15,59** vs. FP 14,95 = **+4,29 %** (Kriterium max. +5 %); die
> verbleibende Lücke ist echtes Quantisierungsrauschen, keine offene
> Präzisions-Entscheidung mehr. Maßgebliche Dokumente:
> `decision_12-21.md` und `INTEGER_LLM/docs/02_empirischer_beleg_bit-exakte-inferenz.md`.
> Ebenfalls überholt ist die Schlussfolgerung in Abschnitt 4, die
> Gewichtsquantisierung sei „dominant": Die damalige Perplexitäts-Messung
> war durch die noch unentdeckten Struktur-Bugs (Fund 15/16) kontaminiert.
> Korrekt bleibt: GPTQ reduziert den linearen Layer-Ausgabefehler
> nachweislich (Abschnitt 5) und ist in den Artefakten aktiv.

---

## 1. Determinismus (Kern-Eigenschaft von Myelith)

**PASSED.** Zwei unabhängige Läufe der Integer-Runtime über dieselben
4 WikiText-2-Sequenzen (439 Tokens) liefern **bit-identische** Ausgabe
(`tests/integration/test_end2end_real.py`). Damit ist die bit-genaue,
reproduzierbare Inferenz über unabhängige Knoten — die Grundlage des
Myelith-Verifikationsmodells (Whitepaper Kap. 6.2) — bestätigt.

## 2. Perplexität (Qualität vs. Gleitkomma-Baseline)

| Modell | Perplexität |
|---|---|
| FP-Baseline (Qwen2.5-0.5B, BF16) | 14,95 |
| Integer-Modell (θ_v 0.10.0) | **73,15** |
| Relativer Anstieg | +389 % (Kriterium: max. +5 %) |

Verlauf der Eskalationen: 14 546 (Per-Tensor) → 3 257 (Per-Channel) →
3 242 (+Korpus-Headroom) → 73,15 (RoPE/Attention-Fix). Der verbleibende
Abstand ist der int8-Boden *nach* GPTQ.

## 3. Layer-für-Layer-Abgleich (Integer vs. HF)

`tests/diag/verification_layer_compare.py` (Mehrpositions-Sequenz, 8 Tokens):

- **Aktivierungs-Skalen stimmen:** Das absmax-Verhältnis Integer/HF liegt in
  den Layern 0–22 bei 0,84–1,19 (±20 %). Der Integer-Pfad verfolgt die
  HF-Aktivierungs-Magnituden also korrekt.
- **Werte haben Quantisierungsrauschen:** Die first4-Abweichung (rel. L2)
  wächst von ~0,15 (Layer 0) auf ~0,83 (Layer 23) — konsistent mit
  akkumuliertem Quantisierungsrauschen, **kein Struktur-Bug**.
- **Layer 23** weicht am stärksten ab (absmax 0,26×) und speist direkt die
  Logits; das ist der Hauptbeitrag zum Perplexitäts-Abstand.

## 4. Fehlerzerlegung (Gewichte vs. Aktivierungen)

`tests/diag/error_decomposition.py` (4 Layer, gegen Float):

| Fehlerquelle | Beitrag |
|---|---|
| **Gewichtsquantisierung (int8)** | **dominant** (0,4–1,4 %/Layer bei RNE) |
| Aktivierungsquantisierung (int16) | vernachlässigbar (<0,2 %) |
| LUTs (exp/SiLU/rsqrt/RoPE) | vernachlässigbar (<1 %) |

→ Alle Aktivierungs-/LUT-Eskalationen (SmoothQuant etc.) sind eine
Sackgasse; der Hebel ist die **Gewichts-Präzision**.

## 5. GPTQ

`tests/diag/gptq_verification.py`: GPTQ reduziert den Layer-Ausgangsfehler
gegenüber RNE um **Faktor 6–8** und ist in den Artefakten **aktiv**. Das
frühere „GPTQ bringt nichts" (3 242 → 3 318) war durch die damals noch
kaputte Attention verfälscht.

## 6. Mischpräzisions-Empfindlichkeit

`tests/diag/mixed_precision_sensitivity.py`: Die Empfindlichkeit der Layer
auf Gewichtsfehler ist **relativ gleichmäßig** (Faktor 3 zwischen
empfindlichstem Layer 23 und unempfindlichstem Layer 18). Mischpräzision
(int16 nur für Top-N-Layer) ist daher nur moderat wirksam; um die Lücke
substanziell zu schließen, bräuchte es int16 für die meisten/alle Layer.

## 7. Test-Suiten (alle grün)

- Rust: kernels 30 Tests, runtime 44 Tests, pipeline-Build — alle PASSED.
- Python: fixed_point, kernels, rmsnorm, rope, gptq, luts, calibration,
  export_workflow — alle PASSED.
- Ganzzahligkeits-Prüfung: keine f32/f64-Treffer im Rechenpfad.

## 8. Fazit

- **Bit-exakte Inferenz: erreicht und verifiziert** (Determinismus PASSED).
- **Qualität: begrenzt durch int8-Gewichtsquantisierung** (73,15 vs. 14,95).
  Aktivierungen, LUTs und Struktur sind verifiziert in Ordnung.
- **Offene Präzisions-Entscheidung:** int16 für alle Layer (wirksamster Weg,
  verdoppelt Modellgröße), Mischpräzision (moderat), oder Akzeptanz der Lücke
  mit Blick auf die Zielgröße 100B+ (größere Modelle quantisieren robuster).
