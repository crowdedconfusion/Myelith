# integer-llm

> **Version:** 0.12.33
> **Datum:** 2026-08-12
> **Status:** 🎉 **ENTSCHEIDUNGSPUNKT 12.21 AKZEPTIERT** — Perplexität **15,59** vs. FP-Baseline 14,95 = **+4,29 %** (Kriterium: max. +5 %). Root-Cause des Qualitätseinbruchs war Fund 17: die fehlende 1/√head_dim-Attention-Skalierung (Scores waren um √head_dim=8 zu groß, Softmax zu scharf). Behoben → Perplexität 73,15 → 15,59. Bit-exakte Ganzzahl-Inferenz ist damit **qualitativ validiert**. **v0.12.33 (Evidenz-Paket):** der Beleg ist plastisch gesichert — Bit-Identität über 5 Prompts × 5 unabhängige Läufe (Token-Hash + SHA-256 identisch), Parallelgenerierung DE/EN + Top-1-Agreement 89,3 % gegen die BF16-Referenz, Durchsatz-Basis ~19 tok/s (Referenz-Backend). Zusammenfassung: [`docs/02_empirischer_beleg_bit-exakte-inferenz.md`](../docs/02_empirischer_beleg_bit-exakte-inferenz.md). Determinismus PASSED, alle Test-Suiten grün.

Bit-exaktes, vollständig ganzzahliges Inferenzsystem für LLMs auf
Qwen-W8A8-Basis.

## Ziel

Deterministische Integer-Inferenz ohne Gleitkommaoperationen im Rechenpfad
(Division ausschließlich als arithmetischer Rechtsshift), mit
Pipeline-Parallelismus auf heterogenen Hardware-Knoten (NVIDIA, AMD, CPU).
Die Ganzzahlarithmetik ist die Voraussetzung für bitgleiche Ausführung über
unabhängige Knoten hinweg — die Grundlage des Myelith-Verifikationsmodells
(Whitepaper Kap. 6.2). Referenzmodell ist Qwen2.5-0.5B (W8A8: Gewichte und
Aktivierungen als int8, Akkumulator int32).

## Modell-Austauschbarkeit & Skalierbarkeit (Design-Prinzip)

**Verbindliche Vorgabe:** Alle Code-Anpassungen — auch und gerade die
Eskalationen der Quantisierungsqualität — sind so anzulegen, dass das
Testmodell später ohne größeren Aufwand durch andere Gewichte ersetzt werden
kann (z. B. Qwen3 bis in den dreistelligen Milliarden-Bereich). Ziel ist,
dass ein Modellwechsel dann möglichst wenig Neucode erfordert.

**Bereits modell-agnostisch (Stand der Analyse):** Der gesamte Rechenpfad
ist frei von hart kodierten Modell-Dimensionen. Alle Kernels
(`linear`, `rmsnorm`, `mlp`, `attention`, `rope`, `softmax`, Fixed-Point,
Sampling) nehmen Dimensionen und Skalen als Parameter und akkumulieren in
i64; der Runtime-Loader liest die Dimensionen ausschließlich aus dem
Artefakt (`model_config.json`), und die Binaries nehmen das
Artefakt-Verzeichnis als CLI-Argument. Das Artefakt-Format
(weights_manifest + `_shifts.bin`, `scales.json`, `luts.json`,
`theta_v.json`) ist dimensions-agnostisch und wird über Form/Hash validiert.
Die Kalibrierung ist Hook-Namen-getrieben ohne Dimensions-Literale.
Geprüft ist das durch synthetische Fixtures mit abweichenden Dimensionen
(hidden=4, heads=2), die einen vollen Forward-Pass durchlaufen.

**Stellen mit Modell-Kopplung (bei einem Wechsel anzupassen):**
1. `calibrate/src/main.py`: `MODEL_NAME`/`HF_MODEL_ID` (zwei Konstanten,
   derzeit kein CLI-/Env-Hebel).
2. `theta_v/spec.json`: `rope_theta` und `max_seq_len` (zur Kompilierzeit
   per `include_str!` eingebettet; Änderung erzwingt Runtime-Rebuild,
   θ_v-Versionssprung und Neukalibrierung).
3. Hart kodierte Pfade in `eval/` (`wikitext_common.py`, `perplexity.py`,
   `baseline.py`), `tests/` und den Pipeline-Configs.
4. Feste LLaMA/Qwen2-Block-Topologie: **Qwen3 benötigt QK-Norm
   (`q_norm`/`k_norm`), die aktuell durch die gesamte Kette fehlt** — der
   einzige echte Struktur-Blocker für einen Qwen3-Wechsel.

**Austausch-Aufwand (Abschätzung):**
- Innerhalb der Qwen2.5-Familie: ~1 Config-Eintrag in `model_configs.py`
  (verifiziert gegen die echte HF-config.json) + ~5 Zeilen
  Konstanten/Pfade + Neukalibrierung; Runtime-Änderungen: keine.
- Qwen3: zusätzlich QK-Norm durch Kalibrierung/Export/Loader/Forward.
- Sehr große Modelle (hidden 4096+, mehrere 100B): keine Dimensions- oder
  Overflow-Blocker im Rechenpfad; offen sind Speicher-/Perf-Fragen
  (dichter int16-LM-Head im RAM, zeilenweise Logit-Berechnung,
  BTreeMap-KV-Cache, fehlende CUDA/ROCm-Backends).

**Konsequenz für neue Eskalationen:** Neue Bausteine (z. B. Block-Hadamard)
dürfen Modell-Dimensionen nicht hart kodieren, sondern müssen sie aus der
Modell-Config ableiten (z. B. Blockgröße als Teiler von `hidden_size`),
damit sie bei einem Modellwechsel erhalten bleiben.

### Zurückgestellt: Hadamard-Basiswechsel (Vermerk für die Zukunft)

**Stand 2026-08-11 — zurückgestellt, nicht verworfen.** Die
Block-Hadamard-Rotation wurde in zwei Vorstudien geprüft
(`tests/diag/hadamard_prestudy.py`, `tests/diag/rmsnorm_hadamard_check.py`):

- **Nutzen bestätigt:** Block-Hadamard (normiert, k=64) senkt das
  Residual-peak/rms in allen 24 Blöcken von ~15–20 auf ~4 — die Ausreißer
  würden also wirklich geglättet.
- **Showstopper für die vereinfachte Variante:** RMSNorm kommutiert nicht
  mit der Rotation. Die Gamma-Nichtkommutativität
  `‖RMSNorm(H·x,γ) − H·RMSNorm(x,γ)‖/‖…‖` liegt bei 0,70–1,59 (Median
  ~1,12), der Fehler ist damit größer als das Signal. Eine Rotation ohne
  Gamma-Transformation würde die normalisierte Aktivierung zerstören.
- **Folgerung:** Hadamard ist nur als *voller Basiswechsel* zu haben, bei
  dem die per-Channel-Gammas in dichte 64×64-Blockmatrizen
  `Q·diag(γ_block)·Qᵀ` transformiert werden — die RMSNorm wird zur
  Block-Matrix-Multiplikation. Das ist ein eigenes Teilprojekt
  (Kalibrierung + Runtime + θ_v-Vertrag + Tests), deutlich größer als ein
  Einzel-Patch, und der Perplexitäts-Gewinn ist vorab nicht seriös zu
  beziffern.
- **Warum zurückstellen:** Die Architektur ist modell-agnostisch und sauber
  geschichtet, daher ist der Basiswechsel **jederzeit nachrüstbar, ohne dass
  heutige Arbeit ihn blockiert oder verteuert** (er erfordert ohnehin einen
  θ_v-Versionssprung + Neukalibrierung). Zuerst werden billigere
  Alternativen gegen die Ausreißer geprüft (SmoothQuant-artige
  Skalen-Umverteilung, Mischpräzision).
- **Wiederaufnahme:** Wenn die billigeren Alternativen die Perplexitäts-Lücke
  nicht ausreichend schließen, kann der Basiswechsel aufgegriffen werden.
  Dann blockgrößen-parametrisiert und modell-agnostisch umsetzen (k=64 für
  hidden=896, s. Vorstudien).
- **Literatur-Einordnung (FSBR/I-LLM):** Die publizierten Vergleichswerte
  (I-LLM, arXiv:2405.17849: W6A6 mit +3 % Perplexität auf LLaMA-7B) beruhen
  wesentlich auf FSBR — der Glättung der Kanal-/Token-Varianz VOR der
  Quantisierung. Dieser Baustein fehlt hier bislang; als zweiter
  Ausreißer-Pfad neben Hadamard bleibt der FSBR-Nachbau vermerkt (mit
  bitweiser Varianzberechnung statt Newton-Verfahren, damit Kalibrierung
  und Inferenz übereinstimmen). Der Literaturvergleich „Integer gegen
  Gleitkomma" wird erst mit einer Ausreißerbehandlung vollständig.

## Struktur

| Verzeichnis | Zweck |
|---|---|
| `kernels/` | Rechenkerne (RMSNorm, W8A8-Linear, RoPE, Softmax, Attention, MLP, Sampling) mit austauschbaren Backends über ein `Backend`-Trait. Implementiert ist das `reference`-Backend; `cpu-simd`, `cuda` und `rocm` sind als Features vorbereitet. |
| `runtime/` | Modell-Loader, Transformer-Forward-Pass, KV-Cache, Tokenizer, Generierungs-Loop und CLI (`integer-llm-runtime`). |
| `pipeline/` | Mehrknoten-Orchestrierung (Stage-Runtime; der Betrieb über ein echtes Netz folgt in einer späteren Phase). |
| `calibrate/` | Python-Offline-Phase: lädt das HF-Referenzmodell, quantisiert Gewichte, berechnet Aktivierungsskalen, erzeugt Lookup-Tabellen und exportiert die θ_v-Artefakte. |
| `theta_v/` | Der kanonische numerische Vertrag (`spec.json`). |
| `tests/` | Unit-, Integrations-, Regressions- und Golden-Vector-Tests. Python-Tests sind eigenständige Skripte, Rust-Tests liegen inline in den Modulen. |
| `eval/` | Qualitätsmessung: Gleitkomma-Baseline und Perplexitätsvergleich. |
| `models/` | Quellmodell (Qwen/Qwen2.5-0.5B, nicht versioniert). |
| `artifacts/` | Exportierte θ_v-Artefakte (nicht versioniert). |
| `scripts/` | Hilfs-Skripte: `fetch_model.sh` (Modell-Download mit fixierter Revision), `build_artifacts.sh` (Kalibrierung + Export in einem Lauf). |
| `deploy/` | Docker-Deployment (Dockerfile, docker-compose). |

## Bauen und Testen

Rust-Seite — jede der drei Crates wird einzeln gebaut und getestet:

```bash
cd kernels   && cargo build && cargo test
cd runtime   && cargo build && cargo test
cd pipeline  && cargo build && cargo test
```

Python-Seite — Kalibrierung in einem eigenen venv (Python ≥ 3.10):

```bash
python3 -m venv calibrate/.venv
calibrate/.venv/bin/pip install -r calibrate/requirements.txt
scripts/build_artifacts.sh    # Kalibrierung + Export, von INTEGER_LLM/ aus
```

Voraussetzung für den Kalibrierungslauf ist das Quellmodell unter `models/`
(siehe `models/README.md`).

## Changelog

### v0.12.32 – 2026-08-11
- **🎉 ENTSCHEIDUNGSPUNKT 12.21 AKZEPTIERT** — Perplexität **15,59** vs.
  FP-Baseline 14,95 = **+4,29 %** (Kriterium: max. +5 %).
- **Fund 17 (Root-Cause, behoben): fehlende 1/√head_dim-Attention-
  Skalierung.** HF-Qwen2 skaliert die Attention-Scores mit
  `attn_weights = q·k · head_dim^-0.5` (head_dim 64 → Faktor 1/8). Dieser
  Faktor fehlte in `runtime/src/model.rs` im `score_shift`, die Scores waren
  dadurch um √head_dim (=8) zu groß und die Softmax viel zu scharf — die
  Ursache des Perplexitäts-Blow-ups (73,15). Behoben durch einen
  zusätzlichen Rechtsshift um log₂(head_dim)/2 (=3 bei head_dim 64) im
  `score_shift`. Bit-exakt (nur ein Shift), deterministisch, keine
  θ_v-Spezifikationsänderung des Zahlenformats nötig.
- **Perplexität-Verlauf der Eskalationen:** 14 546 (Per-Tensor) →
  3 257 (Per-Channel) → 3 242 (+Headroom) → 3 318 (GPTQ) →
  2 972 (SiLU-Raster) → 73,15 (RoPE-Fix + KV-Cache-Fix) →
  **15,59 (Attention-Skalierungs-Fix)**. Der Blow-up war NICHT die
  Quantisierung, sondern drei Struktur-Bugs (RoPE, KV-Cache, Attention-
  Skalierung), die nacheinander gefunden und behoben wurden.
- Verifikation: Determinismus PASSED (zwei Läufe bit-identisch), E2E
  Perplexität 15,59, alle Rust-Suiten (kernels 30, runtime 44, pipeline)
  und Python-Suiten grün.

### v0.12.31 – 2026-08-11
- **Umfassende Verifikation vor der Präzisions-Entscheidung** (reine
  Diagnose-/Analyse-Werkzeuge, keine Änderung des Inferenzpfads):
  - `tests/diag/verification_layer_compare.py`: Layer-für-Layer-Abgleich
    Integer vs. HF — Aktivierungs-Skalen stimmen (absmax-Verh. 0,84–1,19 in
    Layern 0–22), Werte haben akkumuliertes Quantisierungsrauschen
    (first4-Abw. 0,15→0,83), kein Struktur-Bug.
  - `tests/diag/error_decomposition.py`: Gewichtsquantisierung dominiert
    (0,4–1,4 %/Layer RNE), Aktivierungen (<0,2 %) und LUTs (<1 %)
    vernachlässigbar.
  - `tests/diag/gptq_verification.py`: GPTQ senkt Layer-Fehler 6–8× und ist
    aktiv (das frühere „bringt nichts" war durch die kaputte Attention
    verfälscht).
  - `tests/diag/mixed_precision_sensitivity.py`: Layer-Empfindlichkeit
    gleichmäßig (Faktor 3) → Mischpräzision nur moderat wirksam.
  - Dazu: `hadamard_prestudy.py`, `rmsnorm_hadamard_check.py` (Hadamard
    zurückgestellt, s. Vermerk), `activation_outlier_analysis.py`,
    `smoothquant_simulation.py` (SmoothQuant = Sackgasse, da Aktivierungen
    nicht das Problem sind).
  - **Ergebnis-Bericht:** `eval/results/verification_report.md`. Determinismus
    PASSED (bit-identisch), Perplexität 73,15 vs. FP 14,95, alle Test-Suiten
    grün (Rust kernels 30 + runtime 44, Python komplett).

### v0.12.30 – 2026-08-11
- **DURCHBRUCH: Zwei Struktur-Bugs im Mehrpositions-Pfad behoben**
  (aus der gezielten RoPE/Attention-Untersuchung, θ_v 0.9.0 → 0.10.0):
- **Fund 15 — RoPE war fundamental falsch:** Die Integer-RoPE nutzte einen
  einzigen Winkel `2π·pos/max_seq_len` für alle Dimensions-Paare und
  benachbarte Paarung `(x_0,x_1)`. Qwen2/LLaMA-RoPE nutzt aber pro Paar
  `j ∈ [0, head_dim/2)` eine eigene Frequenz `θ_j = 1/rope_theta^(j/half)`
  (rope_theta = 1 000 000, aus der Modell-Config) und half-split-Paarung
  `(x_j, x_{j+half})` (`rotate_half`). Behoben:
  - `calibrate/src/luts.py`: `generate_rope_luts(max_seq_len, head_dim,
    rope_theta, frac_bits)` erzeugt 2D-LUTs `[max_seq_len, head_dim/2]`
    (flach row-major, Index `p·half + j`), ersetzt `generate_sin_cos_lut`.
  - `kernels/src/rope.rs`: `rotate_half_split_i16` (half-split, pro Paar
    eigener Winkel); `apply_rope_i16` indiziert die 2D-LUT.
  - `runtime/src/model.rs` + `bin/layer_probe.rs`: RoPE-Aufruf auf
    Zeilen-Slices umgestellt.
  - `theta_v/spec.json` 0.10.0: `rope.rope_theta`, `rope.pairing:
    "half_split"`, Note. `main.py` zieht `head_dim` aus der Modell-Config.
  - Tests: `tests/test_rope.py` (Integer-RoPE vs. HF-Formel, Pos
    0/1/2/7/63/2047), erweiterte `rope.rs`-Unit-Tests, `test_luts.py`.
- **Fund 16 — Attention attendierte nur auf den ersten Key (der dominante
  Bug):** In `kernels/src/attention.rs::attention_int` war
  `seq_len = q.len()` die Obergrenze der Key-/Value-Schleife. Im
  KV-Cache-Betrieb ist `q.len() == 1` (nur die aktuelle Position), aber
  `k.len() == v.len() == seq_len` (alle bisherigen Positionen). Damit
  attendierte jede Query nur auf `k[0]` — RoPE und Mehrpositions-Attention
  waren wirkungslos, die Perplexität positionsunabhängig schlecht. Belegt
  durch das Experiment „RoPE = Identität ändert den Seq-Dump nicht". Fix:
  Score-/Value-Schleife läuft über `kv_len = k.len()`. Neuer Regressionstest
  `test_attention_kv_cache_single_query_attends_all_keys`.
- **Messergebnis:** Perplexität **2 972 → 73,15** (Faktor 40; weiterhin
  +389 % vs. FP-Baseline 14,95, Kriterium max. 5 %). Seq-Dump-Vergleich
  Position 7: Ebenen 0–20 stimmen jetzt in Vorzeichen und Größenordnung mit
  HF überein. Der verbleibende Abstand ist akkumuliertes
  Quantisierungsrauschen (int8-Gewichte + LUTs), kein Strukturfehler mehr.
- Tests: alle drei Crates grün (kernels 30, runtime 44, pipeline-Build),
  Python-Suite komplett; Ganzzahligkeitsprüfung ohne Treffer im Rechenpfad.

### v0.12.29 – 2026-08-11
- **SiLU-Eingangsraster verfeinert (θ_v 0.8.0 → 0.9.0):**
  `silu.input_frac_bits` 1 → 3 (Raster 0,5 → 0,125 reale Einheiten),
  `silu.input_range` [-256,255] → [-1024,1023] (gleiche reale Domäne
  [-128, 127.875], LUT 512 → 2048 Einträge). Umsetzung nur in spec.json +
  Kalibrierung/LUT-Generierung; der Inferenzpfad konsumiert `silu_in_frac`
  und `silu_lut_offset` weiterhin spec-gesteuert (Loader), Kernel-Logik
  unverändert. Angepasst: `ModelConfig::default()`, Loader-Test-Assertion
  (Offset 256 → 1024), `tests/test_luts.py` (Struktur/Stützwerte/Längen).
- **Messergebnis:** Perplexität **3 318 → 2 972** (−10 %, weiterhin +19 778 %
  vs. FP-Baseline 14,95). Das SiLU-Raster ist damit eine reale, aber nicht
  die dominante Fehlerquelle.
- **Neue Lokalisierung (Seq-Dump-Vergleich Position 0/1/7):** Position 0
  (Einzeltoken, RoPE = Identität) zeigt Ebenen 0–22 in AbsMax übereinstimmend;
  schon Position 1 (2 Tokens) weicht ab Ebene 5 ab, Position 7 ab Ebene 15 —
  die Divergenz wächst mit der Position. Da RoPE an Position 0 trivial ist
  und ab Position 1 tatsächlich rotiert, rückt der Mehrpositions-Pfad
  (RoPE/KV-Cache/positionsabhängige Attention) in den Fokus — er war durch
  die bisherigen Position-0-Proben nie abgedeckt.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build),
  Python-Suite komplett.

### v0.12.28 – 2026-08-11
- **GPTQ-Eskalation (Strategie 3, θ_v 0.7.0 → 0.8.0):** Neues Modul
  `calibrate/src/gptq.py` — `HessianCollector` sammelt in derselben
  Kalibrier-Vorwärtspassage die Hessischen Matrizen (H = Σ x·xᵀ) für alle
  168 linearen Projektionen; `gptq_quantize()` quantisiert mit
  Hessian-gestützter Fehlerkompensation (oberer Cholesky-Faktor von H⁻¹,
  sequenzielle Spaltenverarbeitung nach Frantar et al. 2022). Zielgröße ist
  der AUSGABEFEHLER ||X·W − X·Q||² statt des einzelnen Gewichtsfehlers.
  Artefakt-Format unverändert (int8, Per-Channel-Zweierpotenz-Shifts), der
  Integer-Inferenzpfad bleibt unberührt deterministisch. `main.py` wendet
  GPTQ auf die linearen Projektionen an (überschreibt die RNE-Einträge);
  Embedding/Biases/Gammas bleiben RNE, LM-Head bleibt int16.
- **spec.json 0.8.0:** `rounding` ausdifferenziert —
  `linear_weights: "gptq_error_feedback"`, `default:
  "round_to_nearest_even"`.
- **Messergebnis (wichtiges Negativ-Ergebnis):** GPTQ reduziert den
  Ausgabefehler der linearen Schichten nachweislich (Synthetik-Test −47 %,
  21–25 % der int8-Werte weichen von RNE ab), aber die End-to-End-
  Perplexität verbessert sich nicht: **3 242 → 3 318** (weiterhin +22 086 %
  vs. FP-Baseline 14,95). Die Divergenz in Ebene 23 (Integer ~36 vs. HF
  188) bleibt bestehen. **Schlussfolgerung:** die lineare
  Gewichtsquantisierung ist NICHT die dominante Fehlerquelle — der Fehler
  liegt in den Nichtlinearitäten (SiLU-/exp-/rsqrt-LUT) und/oder der
  Aktivierungsquantisierung. Neue Tests `tests/test_gptq.py` (4 Tests).
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build),
  Python-Suite komplett inkl. neuer GPTQ-Tests.

### v0.12.27 – 2026-08-11
- **Mehrpositions-Divergenzsuche (Fund-14-Kandidat iii, Diagnose-Patch):**
  Neue Diagnose-Binaries `runtime/src/bin/seq_layer_dump` (Reststrom-
  Statistiken nach jedem Layer an der letzten Position einer Sequenz,
  KV-Cache gefüllt) und `runtime/src/bin/seq_logits_sweep` (Top-1-Logit je
  Position), Gegenstücke `tests/diag/seq_layer_dump_hf.py` /
  `seq_logits_sweep_hf.py`.
- **Lokalisierung (Single-Token, Position 0):** Der AbsMax des Reststroms
  stimmt in allen 24 Ebenen mit HF überein (Ausreißer-Plateau ~1600 in den
  Ebenen 3–20, Abfall auf ~30 in 21–22 — beides echtes Modellverhalten, das
  der Integerpfad reproduziert). Aber die Bulk-Dimensionen (erste 4 Werte)
  weichen schon ab Ebene 3 um ~25–30 % ab. In den letzten Ebenen wird die
  Abweichung kritisch: dort werden die Residual-Ausreißer weggekürzt und das
  Signal ~50× kleiner — Ebene 23 liefert Integer 43 vs. HF 188 (4,4×, anderer
  Inhalt, nicht nur Skala).
- **Ausschlüsse:** Die Gewichtsquantisierung ist über die Ebenen gleichmäßig
  gut (Layer 22 vs. 23: identischer relativer Fehler ~1,5–2 %), und die
  Skalen clampen nicht (Headroom-Check v0.12.26). Damit ist die verbleibende
  Lücke akkumuliertes Quantisierungsrauschen (int8-Gewichte + LUT-Näherungen),
  das in den letzten Ebenen verstärkt wird — kein lokalisierter Einzel-Bug.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build).

### v0.12.26 – 2026-08-11
- **Fund-14-Kandidat (i) geprüft (außerplanmäßiger Patch):** Neue Diagnose
  `tests/diag/scale_headroom_hf.py` misst die realen Aktivierungs-Spannweiten
  auf denselben WikiText-2-Sequenzen wie der Entscheidungspunkt und
  vergleicht sie mit den kalibrierten Per-Layer-Skalen. Ergebnis: die auf
  nur vier Kurz-Prompts (~200 Token) kalibrierten Skalen hatten keinen
  Headroom — **50 von 314 Modulen clampten** still an der int16-Grenze
  (schlimmste: `layers.12.mlp.down_proj.input` 2,8× über Skala,
  `model.norm.input` 2,2× über Skala = der finale Residualstrom vor dem
  LM-Head), weitere 185 waren knapp (<1,5×).
- **Abhilfe:** `calibrate/src/main.py` kalibriert jetzt zusätzlich auf einer
  breiten Stichprobe von 64 WikiText-2-Sequenzen à ≤128 Tokens aus derselben
  Verteilung (die vier konkreten Mess-Sequenzen werden ausgespart, keine
  Benchmark-Überpassung). Neukalibrierung: **0 von 314 Modulen clampen**,
  schlechtester Headroom jetzt 1,01×.
- **Wichtiges Negativ-Ergebnis:** Die Perplexität änderte sich dadurch kaum
  (3 257 → **3 242**, weiterhin +21 579 % vs. FP-Baseline 14,95). Das
  Aktivierungs-Clamping war also real und ist behoben, aber **nicht die
  dominante Fehlerquelle**. Fund 14 bleibt offen; die verbleibende Lücke
  verlangt die übrigen Kandidaten (ii: SiLU-Eingangsraster, iii:
  Mehrpositions-Attention) oder eine gezielte Mehrpositions-Divergenzsuche.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline-Build).

### v0.12.25 – 2026-08-11
- **Eskalation nach Entscheidungspunkt 12.21 (außerplanmäßiger Patch,
  θ_v 0.6.0 → 0.7.0):** Per-Channel-int8-Quantisierung für ALLE Gewichte
  (zuvor nur LM-Head-Ausnahme in int16): eine Zweierpotenz-Skala je
  Ausgabe-Zeile, bei 1D-Tensoren (Biases, LayerNorm-Gammas) je Element.
  Per-Tensor-Skalen hatten 10–17 % der Gewichtseinträge zu 0 gerundet
  (AbsMax 17–34× über typischer Größe); per-channel sind es 0,0 %.
  Determinismus unberührt: alle Skalen bleiben Zweierpotenzen, der
  Rechenpfad bleibt rein ganzzahlig (Shifts statt Division).
  - `calibrate/src/quantize.py`: `quantize_symmetric_int8_per_channel()`
    (neu, Standard für alle Gewichte); `quantize_model_weights()` darauf
    umgestellt. Legacy-Per-Tensor-Funktion bleibt für Tests erhalten.
  - `calibrate/src/export_weights.py`: je Tensor eine zusätzliche
    `<name>_shifts.bin` (int8, ein Shift je Zeile); Manifest-Einträge
    tragen `shifts_file` + `shifts_hash` und Sentinel `scale:-1.0` /
    `shift:-1`; SHA-256-Nachschreiben-Verifikation auch der Shifts.
  - `runtime/src/loader.rs`: `QTensor.shifts: Vec<u8>` (je Zeile);
    Loader liest per-row Shifts (mit Längen- und Hash-Prüfung),
    abwärtkompatibler Fallback repliziert einen Einzel-Shift.
  - `kernels/`: `linear_w8a16`, `add_bias_i16`, `rmsnorm_i16`, `mlp_int`
    und das Backend-Trait auf per-channel Signaturen umgestellt
    (`w_shifts`/`gamma_shifts`/`bias_shifts` je Ausgabe-Zeile bzw. Element).
  - `runtime/src/model.rs`: Embedding-Lookup, RMSNorm, alle Projektionen,
    Bias-Addition und LM-Head konsumieren die Zeilen-Shifts.
- **Fund 11 (behoben):** Per-Channel-Quantisierung blies 1D-Tensoren
  (Bias, Gamma) durch Broadcasting `t[n] · shifts[n,1]` zu einer
  `[n,n]`-Matrix auf (`q_proj.bias`: 896 → 802 816 Elemente), der
  Runtime-Loader verweigerte darauf die Modell-Ladung. Fix: 1D-Tensoren
  werden als Spaltenvektor behandelt und zurückgequetscht;
  Regressionstest `test_quantize_int8_per_channel_1d_keeps_shape`.
- **Funde 12+13 (behoben, nur Diagnose-Werkzeuge):** `tests/diag/
  layer_probe_hf.py` addierte die Q/K/V-Biases doppelt (Fund 12) und
  wandte `o_proj` doppelt an, weil transformers ≥ 5.x ihn bereits intern
  in `Qwen2Attention.forward` ausführt (Fund 13); außerdem auf die neue
  self_attn-API (`position_embeddings`, `attention_mask`) portiert.
  Beide Fehler verfälschten nur die HF-Vergleichsprobe, nie die
  Messungen (Baseline/Perplexität laufen über den vollen Modell-Forward).
- **Neukalibrierung + Neumessung (Entscheidungspunkt 12.21, 2. Lauf):**
  Perplexität **14 546 → 3 257** (Faktor 4,5 besser), FP-Baseline 14,95
  → relativer Anstieg **+21 683 %** → Akzeptanzkriterium (max. 5 %)
  **weiterhin VERFEHLT**. Logit-/Layer-Proben: S0–S7 stimmen in der
  Skala mit HF überein, ' die'/' der' bleiben in den Top-10, aber die
  Logit-Spannweite ist komprimiert und der korrekte Token (' Paris')
  fällt aus den Top-10 — Muster akkumulierten Quantisierungsrauschens,
  kein lokalisierter Stufenfehler. Dokumentiert als Fund 14; Protokoll
  in `eval/results/decision_12-21.md`.
- Tests: alle drei Crates grün (kernels 28, runtime 44, pipeline 0+Build),
  Python-Suite vollständig (inkl. neuer 1D-Regression), Ganzzahligkeits-
  Prüfung ohne Treffer im Rechenpfad.

### v0.12.24 – 2026-08-11
- **Entscheidungspunkt 12.21 gemessen:** `eval/perplexity.py` vergleicht
  Integer-Modell und FP-Baseline auf identischen WikiText-2-Sequenzen
  (Parameter aus dem Baseline-JSON, Single Source of Truth). Ergebnis:
  14,95 (FP) vs. 14 546,38 (Integer) → **+97 179 %** → Akzeptanzkriterium
  (Vorschlag max. 5 %) **VERFEHLT**. Protokoll mit der zwingenden
  Einordnung (Decodierstrategie, 0,5B-als-ungünstigster-Fall) unter
  `eval/results/decision_12-21.md`. Nächster Schritt: Wahl des
  Eskalationspfads

### v0.12.23 – 2026-08-11
- **FP-Baseline gemessen (12.20):** `eval/baseline.py` — HF-Referenzmodell
  in BF16, Teacher-Forcing, exakt dieselben WikiText-2-Sequenzen wie der
  Integer-E2E-Test (gemeinsame Sequenzauswahl). Ergebnis: Perplexität
  **14,95** auf 435 Positionen — gesichert unter
  `eval/results/baseline_wikitext2.json`

### v0.12.22 – 2026-08-11
- **Messinfrastruktur für den Entscheidungspunkt:** `runtime/src/bin/perplexity_probe`
  (Teacher-Forcing-Log-Probabilities über Token-Sequenzen; Log-Softmax im
  Messpfad f64, Logits aus dem Integerpfad), `eval/wikitext_common.py`
  (gemeinsame deterministische WikiText-2-Sequenzauswahl für alle drei
  Messungen) und `tests/integration/test_end2end_real.py` (E2E-Test mit
  echten Gewichten)
- Erster Messlauf: Determinismus bewiesen (zwei Läufe bitidentisch),
  Integer-Perplexität 14 546 auf 435 WikiText-2-Positionen — quantitative
  Bestätigung von Fund 9; ein Skalierungsfehler der Probe (z_max über
  unskalierte Logits) wurde durch die Endlichkeits-Assertion des Tests
  gefangen und behoben

### v0.12.21 – 2026-08-11
- **Erste echte Integer-Inferenz:** die Runtime lädt die echten kalibrierten
  Gewichte (290 Tensoren inkl. 72 Biases, 314 Skalen, 5 LUTs) und generiert
  deterministisch (zwei Läufe → identischer Token-Hash)
- Diagnose-Binaries ergänzt: `runtime/src/bin/layer_probe` (Layer-0-
  Zwischenwerte vs. HF-Referenz), `logit_probe` (Top-k-Logits nach Prefill),
  `rank_probe` (Teacher-Forcing-Rang des echten nächsten Tokens); verifiziert:
  Embedding/RMSNorm/Q/K/V stimmen innerhalb der Quantisierungstoleranz mit
  float64-Ground-Truth überein
- exp-LUT-Domäne [0, 0.5) → [0, 64) erweitert (theta_v 0.5.2): Messung
  zeigte, dass die alte Domäne 79–92 % der Attention-Positionen
  (Score-Differenzen bis ~28) auf Wahrscheinlichkeit 0 setzte; neuer
  spec-Parameter `exp_input_frac_bits` (Eingang frac 4, Ausgang frac 8),
  `lut_shift` der Attention wird daraus abgeleitet
- Fund 9 (Qualität): Generierung kollabiert nach 1–2 Tokens in
  Repetitions-Loops; Teacher-Forcing-Ränge mehrheitlich 10³–10⁴; Ursache ist
  die Logit-Verzerrung durch die int8-Quantisierung der Embedding-Tabelle
  (= geteilter LM-Head) — Eskalationspfade am Entscheidungspunkt 12.21

### v0.12.20 – 2026-08-11 (außerplanmäßiger Patch)
- **Numerik-Realitätsabgleich:** Messungen am echten Qwen2.5-0.5B zeigten,
  dass die alten Format-Annahmen nicht tragen (Residual-Spitzen ±1576 statt
  der i8-Annahme ±0,5; h = silu(gate)·up bis ±1640). Aktivierungen sind
  jetzt **int16 mit kalibrierten Per-Layer-Zweierpotenz-Skalen**, Gewichte
  bleiben int8; Residualstrom int16 frac 3
- **theta_v/spec.json 0.4.0 → 0.5.0** (konsensrelevant, mit Zustimmung des
  Projektinhabers): residual frac 3, activation int16, rsqrt input_shift 8
  + `index_normalization: dynamic_even_shift`, SiLU-Domäne [-256,255] mit
  `input_frac_bits: 1` / `output_frac_bits: 6`
- **Neuer Kernel `rmsnorm_i16`:** LUT-gestütztes rsqrt wird jetzt
  konsumiert, divisionsfrei im Hot-Path (Mittelwert via
  Reziproken-Multiplikation, dynamischer gerader Index-Shift), explizite
  Ziel-Ausgabeskala, gamma mit eigenem kalibriertem Shift — Funde 1 und 8
  sind damit behoben
- Kernel auf int16-Aktivierungen umgebaut: `linear_w8a16` (i64-Akkumulator),
  `add_bias_i16`, `rotate_pairs_i16`, `attention_int`/`mlp_int` mit
  Per-Layer-Skalen; `softmax_int`-Overflow-Fix (maskierte i32::MIN-Werte)
- Runtime: Per-Layer-Skalen vollständig verdrahtet (Fund 2 gelöst),
  `ModelConfig` aus der eingebetteten spec.json (`spec_model_params()`),
  `build_model` validiert alle Skalen-Einträge laut
- Kalibrierung: int16-Wertebereich (`ACTIVATION_MAX_INT = 32767`),
  erweiterter Korpus (vier Prompts), neue Hooks; 265 Skalen (Shifts 4–16),
  θ_v-Hashes konsistent
- Backend-Trait/Platzhalter auf neue API; alte AVX2-Intrinsics entfernt
  (Neuaufbau in Phase 12.35–12.39); Golden Vectors regeneriert
- Volle Suite grün: kernels 25, runtime 40 Tests, alle Python-Skripte inkl.
  Cross-Hardware (6/6 Backends)

### v0.12.19 – 2026-08-11 (außerplanmäßiger Patch)
- **Attention-Biases in der Runtime** (Fund aus dem Kalibrierungslauf 12.16,
  mit dem Projektinhaber als außerplanmäßiger Patch beschlossen): Qwen2.5
  besitzt Biases an q/k/v_proj — sie werden jetzt im Integerpfad verarbeitet
  statt still verworfen
- Neues Pflichtfeld `attention_bias` in `model_config.json`/`ModelDims`
  (Muster wie `num_kv_heads`/`tie_word_embeddings`); fehlt es, scheitert das
  Laden laut
- `loader.rs` lädt bei `attention_bias: true` je Layer die Bias-Tensoren
  `*.self_attn.{q,k,v}_proj.bias` und validiert ihre Längen (q:
  num_heads×head_dim, k/v: num_kv_heads×head_dim)
- Neuer Kernel `add_bias_i8()`: Bias mit eigener kalibrierter Skala wird per
  `rescale` auf die Q/K/V-Ausgabeskala gebracht, i32-Addition mit Clamping —
  reine Ganzzahlarithmetik; Aufruf in `model.rs` nach den q/k/v-Projektionen
- `model_configs.py`: `"attention_bias": True` für 0.5b; Kalibrierungslauf
  wiederholt, Artefakte tragen das Feld
- Acht neue Tests (4 Kernel: Rescale-Richtung/RNE/Sättigung/Länge, 4 Runtime:
  Pflichtfeld/Laden/fehlender Tensor/falsche Länge); Python-Fixtures im
  echten Qwen2.5-Format

### v0.12.18 – 2026-08-11
- LUT-Generierung vollständig aus `theta_v/spec.json` gesteuert (neues
  `calibrate/src/luts.py::load_nonlinear_spec()`): keine hartkodierten
  LUT-Parameter mehr in `main.py`
- `generate_rsqrt_lut()` um den bisher ignorierten `input_shift`-Parameter
  korrigiert (Index x = Realwert x · 2^-input_shift, spec: 2^-7) —
  die LUT-Werte ändern sich dadurch grundlegend (z. B. lut[128]: 23 → 256)
- Kernel-Vertrag verifiziert: SiLU (Eingang frac 6, shift 0/offset 128),
  exp (Eingang frac 8, lut_shift 0), RoPE (frac 8) konsistent zur Runtime
- Neuer Test `tests/test_luts.py` (spec-Struktur, input_shift-Semantik,
  Stützwerte, spec-gesteuerte Längen/int16-Bereich)
- Fund: rsqrt-LUT wird noch von keinem Kernel konsumiert (rmsnorm.rs nutzt
  `rsqrt_q()` direkt, spec sagt Methode „lut"); Fund: `rmsnorm_int8`
  dividiert mit `/` statt arithmetischem Rechtsshift — beides Teil der
  RMSNorm-Klärung vor dem Laden echter Gewichte

### v0.12.17 – 2026-08-11
- **Erster echter Kalibrierungslauf** gegen das lokale Qwen2.5-0.5B:
  vollständige θ_v-Artefakte in `artifacts/qwen2.5-0.5b/` — 168
  Aktivierungsskalen (ausschließlich Zweierpotenzen, Shifts 0–8), 290
  quantisierte Gewichts-Tensoren, θ_v-Hashes konsistent
- Neuer Batch-Test in `tests/test_calibration.py` (200 synthetische
  absmax-Werte: Shift-Grenzen, `scale == 2^-shift`, int8-Bereichs- und
  Sättigungsregime)
- Fund: Qwen2.5-0.5B besitzt Q/K/V-Attention-Biases (72 Tensoren) — sie
  werden exportiert, aber von runtime/kernels noch nicht verarbeitet;
  vor dem Laden echter Gewichte zu klären
- `calibrate/requirements.txt`: `accelerate` ergänzt, auf
  `transformers>=5.0.0`/`huggingface_hub>=1.0.0` konkretisiert;
  `loader.py` von deprecated `torch_dtype` auf `dtype`
- `theta_v/spec.json`: Modell-Angaben auf die Basis-Variante korrigiert
  (konsensrelevant, mit Zustimmung des Projektinhabers; Numerik unverändert)

### v0.12.16 – 2026-08-11
- `calibrate/src/export_weights.py` gehärtet: dtype-Prüfung (nur int8), Prüfung
  Byte-Länge = Produkt der shape, SHA-256-Nachschreiben-Verifikation jeder
  exportierten Datei gegen den Manifest-Eintrag — Manifest und `.bin`-Dateien
  können nicht mehr divergieren
- Referenzmodell auf die Basis-Variante festgelegt: `MODEL_NAME = "qwen2.5-0.5b"`,
  `HF_MODEL_ID = "Qwen/Qwen2.5-0.5B"` (der Code trug bisher Instruct-Strings,
  Whitepaper/Doku/lokales Modell benennen die Basis-Variante); model_configs-Schlüssel
  jetzt `"qwen2.5-0.5b"`, `fetch_model.sh`-Default angepasst
- `calibrate/src/loader.py` lädt das Referenzmodell ausschließlich aus dem lokalen
  Snapshot unter `models/` (reproduzierbare Herkunft) statt aus dem HF-Cache; neu:
  `calibrate/src/paths.py::local_model_dir()` mit klarem Fehlerhinweis auf
  `scripts/fetch_model.sh`
- Fund: `scripts/fetch_model.sh` nutzte das in huggingface_hub ≥ 1.x entfernte
  `huggingface-cli` — auf `hf download` umgestellt
- Vier neue Tests in `tests/test_export_workflow.py`; volle Rust- und Python-Suite grün

### v0.12.15 – 2026-08-11
- `calibrate/src/main.py`: vollständiger Export-Workflow — `model_artifacts_dir()` (neues
  `calibrate/src/paths.py`, spiegelt `runtime/src/paths.rs`) statt hartkodiertem Pfad,
  `model_config.json`-Export ergänzt
- Drei Bugs behoben: Export-Reihenfolge (Gewichte vor `theta_v.json`, jetzt technisch
  erzwungen), `weights_hash` referenzierte einen Platzhalter statt des echten
  `weights_manifest.json`, `version`-Feld war hartkodiert statt aus `theta_v/spec.json`
  gelesen (hätte die 12.13-Versionsprüfung im Loader garantiert scheitern lassen)
- Fund: `export_quantized_weights()` schrieb in ein nicht vom Loader erwartetes
  `weights/`-Unterverzeichnis — behoben
- `calibrate/src/model_configs.py`: `get_export_model_config()` lehnt Modellvarianten ohne
  verifizierte GQA-/Tying-Felder laut ab
- Neuer Test `tests/test_export_workflow.py` inkl. Cross-Check gegen das echte kompilierte
  Runtime-Binary

### v0.12.14 – 2026-08-10
- `runtime/src/loader.rs`: `theta_v/spec.json` wird zur Kompilierzeit ins Binary eingebettet
  (`include_str!`); `load_model()` prüft jetzt Versions-Kompatibilität
  (`ThetaV::verify_version_against_spec()`) und echte Manifest-Hashes (`ThetaV::verify()` —
  existierte seit frühen Phasen, wurde aber nie aufgerufen) gegen die tatsächlich geladenen
  `weights_manifest.json`/`scales.json`/`luts.json`
- **Fund:** `calibrate/src/export.py::export_theta_v()` berechnet `weights_hash` aus einer
  Platzhalter-Datei statt dem echten `weights_manifest.json` und läuft laut `main.py` vor dem
  eigentlichen Gewichts-Export — für 12.14 vorzumerken
- Neun neue Rust-Unit-Tests, zwei neue Python-Integrationstests gegen das echte Binary
- **Phase 12.8–12.13 „Loader und Modellanbindung" damit vollständig abgeschlossen**

### v0.12.13 – 2026-08-10
- `tests/integration/test_end2end.py`: kompletter Umbau auf ein vollständiges synthetisches
  Artefakt (`build_synthetic_artifact()`, exaktes Format von `calibrate/`s Export, gleiche
  GQA-Asymmetrie wie Qwen2.5-0.5B) — der alte Dummy-Modell-Pfad über ein leeres Verzeichnis war
  seit v0.12.10 kaputt (`load_model()` verlangt seither vollständige Artefakte)
- Zwei neue Tests validieren echte Gewichts-Integrität End-to-End gegen das kompilierte Binary:
  `test_rejects_incomplete_artifact` (fehlendes Pflichtgewicht) und
  `test_rejects_corrupted_weight_hash` (manipulierter SHA-256-Hash)
- Zusätzliche Tests: getiedete vs. nicht-getiedete Embeddings, fehlendes Artefakt-Verzeichnis
  (deckt den 12.11-Fehlerpfad end-to-end ab)
- Neues `tests/integration/.gitignore` für generierte Test-Artefakte

### v0.12.12 – 2026-08-10
- `runtime/src/main.rs`: saubere Fehlerbehandlung statt `.expect()`-Panics — `run() ->
  Result<(), String>` fängt Fehler bei Modell-/Tokenizer-Ladung ab, `main()` gibt sie auf
  stderr aus und beendet mit Exit-Code 1 (kein Rust-Panic-Backtrace mehr)
- Vorgezogene, gezielte Prüfungen: fehlendes/kein Artefakt-Verzeichnis, fehlendes
  `tokenizer.json` — klarer als der generische Loader-Fehler an dieser Stelle
- `max_tokens` schlägt bei ungültiger Eingabe jetzt explizit fehl statt still auf 20
  zurückzufallen
- Manuell gegen das kompilierte Binary verifiziert (kein Rust-Unit-Test, `main.rs` hängt
  direkt an `std::env::args()`)

### v0.12.11 – 2026-08-10 (außerplanmäßiger Patch, kein regulärer Entwicklungspunkt)
- `calibrate/src/quantize.py`, `calibrate/src/scales.py`: Skalierungs-Formel korrigiert. Bisher
  wurde der Zweierpotenz-Shift nur für den Fall berechnet, dass ein Wert zu GROSS für int8 ist
  (`absmax > 127`); für den Regelfall realer LLM-Gewichte (`absmax` deutlich unter 1) lieferte
  die Formel unbedingt `shift=0`, was Quantisierung auf `round(roher_Wert)` bedeutete — z. B.
  ein Gewicht von 0,02 quantisiert zu 0. Reale kalibrierte Artefakte wären dadurch numerisch
  bedeutungslos gewesen, unabhängig von jeder Loader-/Runtime-Korrektheit
- `scale`-Feld ist jetzt `2^-shift` statt `2^shift` (Dequantisierung als arithmetischer
  Rechtsshift, Kap. 6.2 des Whitepapers); `runtime/src/loader.rs::load_scales()` entsprechend
  angepasst, drei bestehende Tests korrigiert
- `runtime/src/model.rs`: alle acht Gewichte (q/k/v/o/gate/up/down-Projektion, LM-Head)
  verwenden jetzt ihre eigene kalibrierte `QTensor.shift` statt einer globalen Konstante
- Attention-Softmax-Skalierungsfehler behoben: `score_shift`/`lut_shift` an
  `attention_int` waren nicht auf den tatsächlichen Kalibrierungsbereich der exp-LUT
  abgestimmt (Scores wurden 16× zu grob indiziert)
- `kernels/src/mlp.rs::mlp_int`: Signatur um separate `gate_frac_bits`/`up_frac_bits`/
  `down_frac_bits` erweitert statt einer gemeinsamen Gewichtsskala
- Neuer Test `tests/test_calibration.py` (sieben Fälle, Regressionstest für den Bug)
- **Bewusst offen gelassen:** RMSNorm-Ausgabeskala folgt näherungsweise der (aktuell
  verworfenen) Gamma-Kalibrierung; vollständige Verdrahtung der Q/K/V-Aktivierungsskalen aus
  `scales.json` zurückgestellt (später vollständig umgesetzt im Patch
  „Numerik-Realitätsabgleich", v0.12.20)
- **Nummerierungs-Konsequenz:** Punkt 12.11 und alle Folgepunkte behalten ihre Nummer,
  verschieben sich aber je um eine Version (12.11→v0.12.12, …, 13.0→v0.13.1)

### v0.12.10 – 2026-08-10
- `runtime/src/loader.rs`: `load_model()` baut jetzt ein vollständiges `IntegerModel` aus echten
  Artefakten statt Dummy-Daten (`build_model()`, `ModelDims`/`load_model_dims()` aus
  `model_config.json`, `LoadedWeights::get()` über HF-Originalnamen wie
  `model.layers.0.self_attn.q_proj.weight`)
- **Fund:** reales `models/Qwen2.5-0.5B/config.json` hat `num_key_value_heads=2` (gegenüber 14
  Query-Heads, Grouped-Query-Attention) und `tie_word_embeddings=true` — beides war im
  bisherigen Code nicht abgebildet und hätte beim Laden echter Gewichte zu falscher
  Attention-Berechnung bzw. einem fehlenden `lm_head.weight` geführt
- `runtime/src/model.rs`: `IntegerModel.num_kv_heads` ergänzt; `split_heads()` parametrisiert;
  RoPE für Q-/K-Heads getrennt (`rotate_pairs` statt `apply_rope`); KV-Cache nach
  `num_kv_heads` dimensioniert; Attention-Schleife ordnet Query-Heads ihrem KV-Head zu
  (Standard-GQA-Gruppierung); `runtime/src/generate.rs` entsprechend angepasst
- `ModelDims.tie_word_embeddings` steuert LM-Head-Wiederverwendung explizit statt über
  stillschweigenden Fallback bei fehlendem Tensor
- **Offene Lücke, vor 12.18 zu schließen:** `load_scales()`-Ergebnis wird geladen und im Modell
  abgelegt, aber noch nicht in den Forward-Pass verdrahtet (globale statt kalibrierte
  Pro-Layer-`frac_bits`) — Risiko für die Qualitätsmessung in 12.18–12.21
- `calibrate/src/model_configs.py` fehlt `num_key_value_heads`/`tie_word_embeddings` für alle
  Varianten außer der verifizierten 0.5B (für 12.14 vorzumerken)
- Elf neue Unit-Tests, u. a. End-to-End-Fixture mit derselben GQA-Asymmetrie wie das echte
  Modell und ein Forward-Pass-Rauchtest

### v0.12.9 – 2026-08-10
- `runtime/src/loader.rs`: `load_scales()` lädt Aktivierungsskalen aus `scales.json`
  (`shift`, `scale`, `absmax_observed` je Layer-/Modulname, Format aus
  `calibrate/src/scales.py::compute_scales_from_stats`)
- Validiert Wertebereich von `shift` (0..=255) und Konsistenz `scale == 2^shift`; eine
  widersprüchliche Skala wird abgelehnt statt stillschweigend übernommen
- Fünf neue Loader-Unit-Tests: Roundtrip, mehrere Layer, Nicht-Zweierpotenz-Skala,
  shift außerhalb 0..=255, fehlendes Manifest
- `load_model()` nutzt weiterhin Dummy-Skalen/-LUTs/-Gewichte (Einbindung folgt in 12.10)

### v0.12.8 – 2026-08-10
- `runtime/src/loader.rs`: `load_luts()` lädt Lookup-Tabellen aus `luts.json` +
  `<name>.lut.bin` (raw int16, little-endian, `struct.pack(f"<{n}h", ...)` aus
  `calibrate/src/export.py`), mit dtype-, Längen- und SHA-256-Validierung je Tabelle,
  analog zu `load_weights()` aus v0.12.2
- Sechs neue Loader-Unit-Tests: Roundtrip, mehrere Tabellen gleichzeitig, Hash-Mismatch,
  Größen-Mismatch, falscher dtype, fehlendes Manifest
- Festgestellt: `artifacts/README.md` beschreibt LUTs unter `luts/*.lut.bin`, der tatsächliche
  Export (`calibrate/src/export.py`) legt die Dateien jedoch flach im Artefakt-Verzeichnis ab;
  Loader folgt dem Exportcode, Doku-Korrektur noch offen
- `load_model()` nutzt weiterhin Dummy-LUTs (Einbindung folgt in 12.10)

### v0.12.7 – 2026-08-10
- `scripts/fetch_model.sh` (neu): lädt `MODEL_ID`@`REVISION` per `huggingface-cli` nach
  `models/<Name>/`, gibt die aufgelöste Commit-Revision zur Dokumentation in `models/README.md`
  aus
- `scripts/build_artifacts.sh` (neu): dünner Wrapper um `python3 -m calibrate.src.main`
  (Kalibrierung und Export in einem Lauf)
- `calibrate/requirements.txt`: `huggingface_hub` explizit ergänzt (bisher nur transitiv über
  `transformers`, aber von `fetch_model.sh` direkt benötigt)
- Phase 12.3–12.7 („Grundgerüst") damit abgeschlossen

### v0.12.6 – 2026-08-10
- `eval/` angelegt: `README.md` (Zweck, geplanter Inhalt `baseline.py`/`perplexity.py` für
  die Gleitkomma-Baseline und den Perplexitätsvergleich, Verweis auf den
  Entscheidungspunkt 12.18–12.21) und
  `datasets/.gitignore` (Datensätze nicht versioniert, Verzeichnis und Doku bleiben)

### v0.12.5 – 2026-08-10
- `tests/golden/vectors/{op,layer,e2e}/` angelegt; bestehende Vektoren aus `tests/golden/ops/`,
  `tests/golden/layers/` und `tests/golden/e2e/` dorthin verschoben (Inhalt unverändert)
- `tests/golden/generate.py` und `tests/golden/validate.py` auf zentrale Pfadkonstante umgestellt
  (`VECTORS_DIR` bzw. `VECTORS_DIRNAME`/`LEVELS`), `tests/golden/README.md` entsprechend ergänzt

### v0.12.4 – 2026-08-10
- `models/` angelegt: `.gitignore` und `README.md` mit Modellherkunft (Qwen/Qwen2.5-0.5B, Hugging Face); die Revision (Commit-Hash) wird nach dem Download fixiert
- Bugfixes (mit v0.12.4 mitgeführt): Literal-Newline in f-Strings von `test_kernels.py` und `test_cross_node.py`, fehlender `List`-Import in `test_cross_node.py`, pytest-Abhängigkeit in `test_fixed_point.py` entfernt (Eigenständiges Skript nach Projektkonvention), fehlende `tokenizer.json`-Fixture in `test_end2end.py` (minimaler BPE-Tokenizer für den Test-Prompt), i8/i16-Mismatch im SiLU-LUT-Lookup von `backends/simd.rs` (AVX2- und Fallback-Pfad, konsistent zum Reference-Backend)

### v0.12.3 – 2026-08-10
- `artifacts/` angelegt: `.gitignore` (erzeugte Inhalte ausgeschlossen, Verzeichnis und Doku bleiben versioniert) und `README.md` mit Struktur, Herkunft und Pfadregel
- `runtime/src/paths.rs` (neu): zentrale Pfadkonstanten `ARTIFACTS_DIR` und `MODELS_DIR` für calibrate/runtime/pipeline, überschreibbar über die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR`, mit Unit-Test
- Umnummerierung der Entwicklungsplanung nach Regel 5: Binär-Format-Parser ist Punkt 12.2 ↔ v0.12.2, Grundgerüst rückt auf 12.3–12.7, alle Punkte ab 12.8 unverändert

### v0.12.2 – 2026-08-10
- `loader.rs`: Binär-Format-Parser für INT8-Gewichte (`weights_manifest.json` + `.bin`, raw int8, row-major) mit dtype-, Form-, Größen- und SHA-256-Validierung pro Tensor; neue Dependency `sha2`
- Loader-Unit-Tests: Roundtrip, Hash-Mismatch, Größen-Mismatch, fehlendes Manifest, SHA-256-Referenzvektoren
- Bugfixes (mit v0.12.2 mitgeführt): Compile-Fehler in Kernels (`LinearScale`-Import in `backend.rs`, i8/i16-Mismatch in `mlp.rs`), `128i8`-Überlauf im `linear.rs`-Test, `[100i16, …]`-Literal + struct-Header-Format + CRC32-Berechnung + Socket-Connect im Multinode-Test, fehlender `subprocess`-Import + relativer Golden-Pfad in `validate.py`, Einrückungsfehler + W=128→127 im Op-Golden-Vektor in `generate.py`, Literal-Newline in `test_end2end.py`

### v0.12.1 – 2026-08-09
- Golden-Vector-Runner Binary (`golden_runner`) für Op-Level-Validierung (RMSNorm, Linear, Softmax)
- `validate.py`: Subprozess-Aufruf von `golden_runner` mit numerischem Hash-Vergleich Input/Output
- `test_kernels.py`: Rust↔Python-Bridging via `cargo test --features <backend>` + stdout-Parsing
- `test_cross_node.py`: Fehlerausgabe bei Golden-Vector-Failures, `List`-Import fix