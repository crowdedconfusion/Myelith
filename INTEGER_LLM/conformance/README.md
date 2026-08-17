# Konformitätspaket — INTEGER_LLM

> **theta_v-Version:** 0.10.0
> **Crate-Version:** 0.12.36
> **Zweck:** Eigenständiges Artefakt, gegen das fremde Implementierungen
> sich prüfen können — ohne Kenntnis des Projektinneren.

## Was eine konforme Implementierung erfüllen muss

Eine Implementierung ist konform, wenn sie für jeden Golden Vector in
`vectors/` bei identischen Eingaben **bitgleiche** Ausgaben erzeugt.
Die Prüfung erfolgt über SHA-256-Hashes der serialisierten Tensor-Daten
(Feld `hash` in jedem Ein-/Ausgabe-Tensor).

### Drei Validierungsebenen

| Ebene | Datei-Muster | Was geprüft wird |
|---|---|---|
| **Op** | `vectors/op/*.golden.json` | Einzelne Kernel (RMSNorm, Linear W8A16, Softmax) |
| **Layer** | `vectors/layer/*.golden.json` | Kompletter Transformer-Layer (RMSNorm → Attention → MLP → ResAdd) |
| **E2E** | `vectors/e2e/*.golden.json` | End-to-End-Generierung (Embedding → 24 Layer → LM-Head → Token-Auswahl) |

### Golden-Vector-Format

Jede `*.golden.json`-Datei enthält:

```json
{
  "name": "<vektor_name>",
  "level": "op" | "layer" | "e2e",
  "theta_v_hash": "sha256:<hex>",
  "metadata": { ... },
  "inputs": {
    "<key>": {
      "dtype": "int8" | "int16" | "int32",
      "shape": [<dims>],
      "hash": "<sha256 der packed Tensor-Daten>",
      "data": [<werte>]
    }
  },
  "outputs": { ... gleiche Struktur ... }
}
```

**Hash-Berechnung:** SHA-256 über die Little-Endian-binär gepackten
Tensor-Daten (`struct.pack("<Nb", ...)` für int8, `"<Nh"` für int16,
`"<Ni"` für int32).

**theta_v_hash:** SHA-256 der `theta_v/spec.json` — identifiziert die
Ausführungsspezifikation (Bitbreiten, LUT-Bereiche, Rundungsregeln),
gegen die die Vektoren kalibriert wurden. Eine Implementierung muss
gegen dieselbe spec-Version arbeiten.

### Anforderungen pro Ebene

**Op-Level:**
- RMSNorm: `rmsnorm_i16` mit LUT-gestütztem rsqrt, dynamischem geradem
  Index-Shift, Per-Element-Gamma-Shifts, divisionsfreiem Mittelwert.
- Linear: `linear_w8a16` mit i64-Akkumulator, Per-Channel-Gewichtsskalen,
  Round-to-nearest-even beim Rescale.
- Softmax: `softmax_int` mit exp-LUT, numerisch stabil (Max-Subtraktion),
  ganzzahlige Normalisierung mit RNE-Rundung.

**Layer-Level:**
- Vollständiger Transformer-Layer-Forward-Pass gemäß `theta_v/spec.json`:
  Pre-Norm → QKV-Projektion → Attention-Biases → RoPE (Multi-Frequency,
  Half-Split) → GQA-Attention → O-Projektion → Residual → Post-Norm →
  MLP (SiLU-LUT) → Residual.
- Alle Skalen (Per-Layer-Aktivierungsskalen, Per-Channel-Gewichtsskalen)
  müssen als Zweierpotenzen angewendet werden (arithmetischer Rechtsshift).

**E2E-Level:**
- Vollständiges Modell: Embedding → 24 Transformer-Layer → Final-RMSNorm
  → LM-Head (int16, Per-Channel) → Greedy-Decoding.
- KV-Cache über die Sequenz hinweg.
- Tokenizer: Qwen2.5 BPE (deterministisch, float-frei).

### Rundungsregeln

Alle Rescale-Operationen verwenden **Round-to-nearest-even** (RNE) beim
arithmetischen Rechtsshift. Overflow wird durch **Sättigung** (Clamp)
behandelt, kein Wrap-Around.

## Prüfung durchführen

```bash
# Referenz-Backend (Rust, requires: cargo, Artefakte in ../artifacts/)
./run.sh reference

# Eigenes Backend: Binary muss die gleiche Schnittstelle erfüllen
# (Golden Vector JSON lesen, Forward-Pass, PASS/FAIL auf stdout)
./run.sh /pfad/zum/eigenen/binary
```

Exit-Code 0 = alle Vektoren bestanden, 1 = mindestens einer fehlgeschlagen.

## Dateien

```
conformance/
├── README.md          diese Datei
├── run.sh             Prüflauf-Skript
└── vectors/
    ├── op/            3 Op-Level-Vektoren
    ├── layer/         24 Layer-Level-Vektoren (Qwen2.5-0.5B, 24 Layer)
    └── e2e/           3 E2E-Level-Vektoren
```

## Lizenz

Die Golden Vectors und dieses Konformitätspaket unterliegen derselben
Lizenz wie das Myelith-Projekt (PolyForm Shield License 1.0.0).
