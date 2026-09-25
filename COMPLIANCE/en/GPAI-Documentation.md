# Technical Documentation of the General-Purpose AI Models

**As of:** 2026-09-25 · **Deutsch:** [GPAI-Dokumentation.md](../de/GPAI-Dokumentation.md)

This documentation fulfils Article 53(1)(a) and (b) of Regulation (EU)
2024/1689 (AI Act) together with Annexes XI and XII. Its structure
follows the Model Documentation Form of the General-Purpose AI Code of
Practice (transparency chapter). The German version is authoritative.

## 1. General information

| Field | Information |
|---|---|
| Provider | Joschka Benjamin Hänsler, private individual, Germany, non-commercial |
| Contact | via the repository's issues |
| Role | **Provider** of the Myelith artefacts within the meaning of Art. 3(3) and Art. 53. The artefacts are modifications of third-party base models; Myelith fine-tunes them itself and therefore treats itself as a provider with all obligations under Art. 53 for its own modifications. The obligations of the base model providers remain unaffected. |
| Models | `myelith-0.6b`, `myelith-4b`, `myelith-8b`, `myelith-30b-a3b`, `myelith-35b-a3b` |
| Version of the execution specification | θ_v 0.22.0 |
| Base models | Qwen3-0.6B, Qwen3-4B, Qwen3-8B, Qwen3-30B-A3B, Qwen3.6-35B-A3B (Alibaba Cloud), each Apache 2.0; revision and review in [`MODELS/llm/KATALOG.json`](../../MODELS/llm/KATALOG.json) |
| Licence | artefacts and code: PolyForm Shield License 1.0.0; base weights: Apache 2.0 (see [NOTICES](../NOTICES.md)) |
| Systemic risk | no; the own modification stays many orders of magnitude below the threshold of Art. 51(2) (10²⁵ floating-point operations) |

## 2. Model properties

### 2.1 Architecture

All artefacts are **transformer language models** of the Qwen3 family,
converted to **pure integer arithmetic**. Execution is specified bit for
bit, so every computer produces the same output from the same input;
this is the precondition for verifying computational work in the Myelith
network.

| Artefact | Type | Layers | Width | Heads (Q/KV) | Experts (active) | Vocabulary | Size |
|---|---|---|---|---|---|---|---|
| myelith-0.6b | dense | 28 | 1024 | 16/8 | none | 151,936 | 0.9 GB |
| myelith-4b | dense | 36 | 2560 | 32/8 | none | 151,936 | 4.5 GB |
| myelith-8b | dense | 36 | 4096 | 32/8 | none | 151,936 | 8.8 GB |
| myelith-30b-a3b | mixture of experts | 48 | 2048 | 32/4 | 128 (8) | 151,936 | 29 GB |
| myelith-35b-a3b | mixture of experts, hybrid with recurrent state layers | 40 | 2048 | 16/2 | 256 (8) | 248,320 | 33 GB |

**Number formats** (execution specification θ_v 0.22.0): weights `int8`
with one scale per output channel, activations `int16`, accumulator
`int64`. Non-linear functions (softmax, SiLU, RMSNorm, RoPE and others)
use lookup tables and integer series. Division only as an arithmetic
shift with round-half-to-even. No floating point in the compute path.

### 2.2 Modalities

| | Input | Output |
|---|---|---|
| Language models (this documentation) | text | text |
| Myelith system as a whole | text, speech, images | text, synthetic speech |

Hearing, seeing and speaking are handled by **separate third-party
models** that Myelith neither modifies nor trains (see
[NOTICES](../NOTICES.md), section 2). They are not the subject of this
documentation; their providers' information applies. Myelith generates
**no images and no videos**.

## 3. Distribution

- **Source code** publicly in the repository.
- **Artefacts** are not distributed as finished files but built on the
  user's computer from the publicly available base weights (tools in
  `INTEGER_LLM/`); the result is bit-identical.
- **Release bundles** for macOS, Windows and Linux, each with a checksum.
- **In the Myelith network**, nodes compute with the same artefacts.

## 4. Use

What Myelith is intended for and what not is set out in the
[intended use policy](Intended-Use-Policy.md). The language models work
in two kinds of AI systems: the **local assistant** (window and console,
with agent loop and tools) and the **roles of the network** (computing,
verifying).

**Information for downstream providers** (Annex XII): anyone integrating
an artefact into their own system finds architecture, number format,
limitations and measurements here. Tool calls follow the base model's
ChatML format. Output is deterministic under greedy decoding; with
sampling it depends on the seed.

## 5. Training process

### 5.1 Pre-training (base model provider)

The base models were pre-trained and post-trained (instruction following,
thinking mode) by their provider. Scope, data and methods are described
by that provider; Myelith has no influence on them and no further
knowledge.

### 5.2 Conversion to integers (Myelith)

1. Load the weights of the pinned revision and quantise them to `int8`
   per output channel.
2. **Calibrate**: texts from WikiText-2 determine the activation scales
   so that they stay within the `int16` range. Calibration data changes
   no weights.
3. Generate lookup tables for the non-linearities.
4. Measure against the floating-point reference (section 7).

Tools: own Rust runtime; PyTorch and Transformers only for the reference
and calibration.

### 5.3 Own fine-tuning (Myelith)

Myelith can **fine-tune the artefacts in integers**: forward pass,
backward pass and optimiser step run without floating point, with a
normalised step and stochastic rounding of the weights. Today it is used
for

- **measurements with known ground truth** (synthetic learning ladders
  and invented facts, see [training data](Training-Data.md)) that show
  what training stores, and
- **fine-tuning by the user** with their own material via the book
  pipeline (`TRAINING/korpus/`).

Hardware: one computer with an Apple M5 Pro (15 cores, 24 GiB). A
measurement run takes minutes.

## 6. Data

See the public [summary of training content](Training-Data.md)
(Art. 53(1)(d)) and the [copyright policy](Copyright-Policy.md)
(Art. 53(1)(c)).

## 7. Evaluation

**Fidelity to the floating-point reference** (perplexity on WikiText-2,
test split; source: generated [model card](../ethics/Modellkarte.md)):

| Artefact | integer | floating point | difference |
|---|---|---|---|
| myelith-0.6b | 43.49 | 42.26 | +2.92 % |
| myelith-4b | 19.95 | 19.63 | +1.65 % |
| myelith-8b | 13.27 | 12.79 | +3.75 % |
| myelith-30b-a3b | 10.42 | 10.48 | −0.59 % |

**Bit identity**: 48 of 48 conformance vectors (single operations,
layers, full runs) match the reference.

**Trick questions with thinking budget** (12 questions, 2026-09-25):
without thinking 10/12 (30B and 8B), with 32 tokens of thinking 12/12.

## 8. Limitations and known failure modes

- **Fabricated statements**: the models can state false facts fluently
  and confidently.
- **Knowledge cut-off** of the base models; they do not know more recent
  events except through web research.
- **Arithmetic errors and trick questions**: without thinking, even the
  large artefacts fail tasks such as counting letters.
- **Deviation from integer arithmetic**: up to 3.75 % higher perplexity
  than the reference.
- **Language**: German is weaker than English; the small artefacts drift
  into English more easily.
- **Tools**: the agent can call tools wrongly or change files
  unintentionally; hence confirmation before actions.
- **Biases** in the base models' training data are inherited.

## 9. Safety and alignment

- **Alignment** comes from the base models' post-training by their
  provider. Myelith does not train its own alignment and does not
  deliberately weaken the existing one.
- **System-level safeguards** are listed in the
  [intended use policy](Intended-Use-Policy.md), section 3.
- **Bounded tools**: the agent sees only the released folder; commands
  run in a mount; the risk class of every tool is in
  [`../ethics/Risikoklassen.toml`](../ethics/Risikoklassen.toml).
- **Traceability**: bit-exact execution makes every answer reproducible
  and verifiable in the network.

## 10. Compute and energy

| Step | Compute | Energy (estimate) |
|---|---|---|
| Pre-training of the base models | by their provider; not published | unknown |
| Conversion and calibration per artefact | minutes to a few hours on one computer | below 0.5 kWh |
| Own fine-tuning measurement runs | minutes per run | below 0.1 kWh per run |

The estimate assumes about 60 W power draw under load. The own compute
is far below one third of the compute used to pre-train the base models.
