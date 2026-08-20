# Glossary

**Every technical term in the Myelith protocol — explained for people who
haven't studied all of it, and for coding agents finding their way around
the repository.**

*This is the English edition of [`README/Glossar.md`](Glossar.md). Both
are kept in step; if they ever disagree, the German edition is the one
that was updated first.*

---

## How to read this glossary

Myelith sits at the intersection of three fields that rarely meet:
**distributed systems** (consensus, Byzantine faults), **cryptography**
(signatures, random beacons, proofs) and **machine learning**
(transformers, quantisation, fixed-point arithmetic). Almost nobody is at
home in all three. This glossary therefore assumes **no prior knowledge**
and explains each term so that it stands on its own.

Every entry has the same shape:

> **Term** — what it is, in plain words.
> *Example:* a concrete case you can follow.
> *In code:* where it is implemented.
> *In the whitepaper:* where it is derived.

Where a term only makes sense in context, the context comes first.
Cross-references are marked → like this.

**Status:** θ_v 0.17.0 · CONSENSUS phases 1–4 · VERIFICATION phases 1–2 ·
INTEGER_LLM roadmap item 12.77. This file is updated whenever protocol
terminology changes (→ [seven-step documentation chain](#seven-step-documentation-chain)).

---

## Contents

- [A. The network in one paragraph](#a-the-network-in-one-paragraph)
- [B. Determinism — why Myelith computes in integers](#b-determinism--why-myelith-computes-in-integers)
- [C. Fixed point and quantisation](#c-fixed-point-and-quantisation)
- [D. The language model from the inside](#d-the-language-model-from-the-inside)
- [E. Cryptographic building blocks](#e-cryptographic-building-blocks)
- [F. Consensus](#f-consensus)
- [G. Epochs, scheduler and pods](#g-epochs-scheduler-and-pods)
- [H. Verification](#h-verification)
- [Proofs of work: PoI and epoch close](#proofs-of-work-poi-and-epoch-close)
- [I. Tokenomics](#i-tokenomics)
- [J. Training](#j-training)
- [K. Agent layer](#k-agent-layer)
- [L. How this project works](#l-how-this-project-works)
- [M. Abbreviations at a glance](#m-abbreviations-at-a-glance)

---

## A. The network in one paragraph

Myelith is a network of strangers' machines that jointly operate a large
language model. No single machine holds the whole model — it is cut into
sections (→ [shard](#shard)) spread across participants. A user request
travels through those sections in order (→ [pod](#pod),
→ [pipeline parallelism](#pipeline-parallelism)) and comes out the other
end as an answer.

The hard problem is not the computation but **trust**: how does the
network know a participant actually computed, rather than returning a
cheap approximation or plain nonsense? Myelith answers that with a trick —
it makes the computation **exactly reproducible**
(→ [section B](#b-determinism--why-myelith-computes-in-integers)), has
**two** independently drawn pods compute every request, and compares the
results bit for bit. Whoever deviates loses their deposited stake
(→ [slashing](#slashing)).

The rest of the system follows: you need a fair draw
(→ [VRF](#vrf-verifiable-random-function)), a ledger of work performed
(→ [PoI](#poi-proof-of-inference)), a procedure for disputes
(→ [bisection game](#bisection-game)) and a currency that balances demand
against capacity (→ [burn-and-mint](#burn-and-mint)).

---

### Layer model

Four layers, each with its own directory in the repository.

| Layer | Purpose | Components |
|---|---|---|
| **L3 — agent layer** | Agentic workflows, tool use, sessions | `AGENT_LAYER/` |
| **L2 — compute layer** | Model shards, pods, pipeline, KV cache | `COMPUTE_PIPELINE/`, `INTEGER_LLM/` |
| **L1 — consensus layer** | BFT consensus, PoI aggregation, staking, ledger | `CONSENSUS/`, `TOKENOMICS/`, `VERIFICATION/` |
| **L0 — networking layer** | P2P gossip, latency measurement, encrypted channels | `NETWORKING/` |

**The core decision:** consensus does *not* run on the inference results
themselves, but on compact proofs of work produced by the compute layer.
Block time (1–2 s) therefore stays independent of how long an inference
takes.

*In the whitepaper:* chap. 3.2

---

### Network roles

**Shard miner** — holds one section of the model in GPU memory and
computes forward passes. This is the network's actual work.
*Example:* miner A holds layers 0–6, miner B layers 7–13, and so on.
*In code:* `COMPUTE_PIPELINE/myl-pod/src/shard.rs`

**Pod coordinator** — an elected miner of the pod. Collects requests,
drives them through the pipeline, gathers the partial proofs and submits
the aggregated → [PoI bundle](#poi-bundle) at the end of the epoch.
*In code:* `COMPUTE_PIPELINE/myl-pod/src/coordinator.rs`

**Validator** — runs → [BFT consensus](#bft-byzantine-fault-tolerance),
checks PoI samples, manages stake and slashing. Needs little GPU, mostly
CPU and good connectivity.
*In code:* `CONSENSUS/myl-consensus/src/validator.rs`

**Checker** (also *fisherman*) — recomputes randomly drawn segments and
reports deviations. Earns a bounty from slashed stake. The name comes
from Polkadot: someone fishing the network for fraud.
*In code:* `VERIFICATION/myl-verifier/src/checker.rs`

**Gateway** — accepts user requests, routes them to pods, returns the
response stream. For → [external tools](#deterministic-vs-external-tools)
the gateway is the trust anchor.

**User** — burns MYL for → [inference credits](#inference-credit-ic) and
submits requests.

*In the whitepaper:* chap. 3.3

---

## B. Determinism — why Myelith computes in integers

This section explains the project's central technical decision. If you
read only one section, read this one.

### The problem: floating-point addition is not associative

**Floating-point number (float)** — the usual way of representing
fractional numbers in a computer: a mantissa and an exponent, like
scientific notation (1.25 · 10³). Because only finitely many digits are
available, the result is rounded after **every** operation.

**Non-associativity** — for exact numbers, `(a + b) + c = a + (b + c)`.
For floating-point numbers this does **not** hold, because of the
intermediate rounding.

> *Example you can check by hand.* With three significant digits:
> `(1.00 + 0.004) + 0.004` → `1.00 + 0.004` → rounds to `1.00` → again
> `+ 0.004` → `1.00`.
> The other way: `1.00 + (0.004 + 0.004)` → `1.00 + 0.008` → `1.01`.
> Same result? No. And that was one extra addition.

A matrix multiplication in a language model sums **thousands** of such
products. The order in which a GPU merges partial sums depends on the
kernel implementation, the block decomposition, the number of compute
units — things that differ between two graphics cards. **Two honest nodes
therefore get different bits.** That makes a bit comparison useless for
fraud detection: you cannot tell deviation-by-fraud from
deviation-by-different-hardware.

### The solution: integer addition *is* associative

With whole numbers there is no rounding between steps.
`(3 + 5) + 7 = 3 + (5 + 7) = 15`, always, on every machine, in any order.
Run the whole inference in integers and bit-identity is **not a
requirement imposed on execution but a property of the arithmetic**.

This is the heart of the design, and it has a consequence that is easy to
miss: Myelith does **not** have to prescribe the order in which hardware
computes. Block sizes, parallelisation, kernel choice and memory layout
stay free. That is precisely what lets heterogeneous hardware take part
without a competitive penalty.

### What is binding — and what is not

**Binding** (part of → [θ_v](#θ_v-theta-v-model-version)):

1. **Fully integer execution**, including the non-linear functions
   (softmax, SiLU, RMSNorm).
2. **Accumulator width and bit width per tensor**, plus explicit overflow
   behaviour (→ [saturation](#saturation-clamping)).
3. **Division exclusively as an arithmetic right shift**
   (→ [right shift](#arithmetic-right-shift)).

**Free to choose:** reduction order, block decomposition, kernel
implementation, matrix units, memory layout.

*In the whitepaper:* chap. 6.2, 6.5, appendix B.5
*In code:* `INTEGER_LLM/kernels/src/` — no `f32`/`f64` in the compute
path; this is actively checked after every patch
(→ [integer purity check](#integer-purity-check)).

---

### Arithmetic right shift

The only permitted form of division in the compute path. `x >> n` moves
all bits n places right, which divides by 2ⁿ — **rounding towards minus
infinity**.

*Example:* `-7 >> 1 = -4` (not `-3`). An ordinary integer division
`-7 / 2` yields `-3` in some languages (truncation towards zero: C, Rust,
Java) and `-4` in others (floor: Python). **That disagreement is the last
remaining source of platform-dependent results** — the arithmetic right
shift, by contrast, is identically defined on every common architecture.

*In code:* `INTEGER_LLM/kernels/src/fixed_point.rs` — `rshift_round`,
`rshift_round_i64`, `rshift_round_i128`. These variants round to nearest
**even** rather than flooring, which avoids a systematic downward drift
across many layers; the rounding rule itself is part of θ_v and therefore
the same for everyone.

### Saturation (clamping)

What happens when a result exceeds the value range? Two options:
**wrap-around** (32,768 becomes −32,768 — a sign flip out of nowhere) or
**saturation** (the value stops at 32,767). Myelith mandates saturation,
because a truncated value is a harmless inaccuracy whereas a
sign-inverted one destroys the computation.

*In code:* `clamp_i16`, `clamp_i32`, `clamp_i8`, `clamp_i16_from_i64` in
`kernels/src/fixed_point.rs`

### Bit-identity / bit-exact

Two results are bit-identical if their byte representation is the same —
not "close", not "within a tolerance", but identical. The comparison is
therefore **binary and parameter-free**: there is no threshold to
calibrate, attack, or shift by governance.

**Why not a tolerance comparison?** It was examined and rejected:
computational noise accumulates across chained execution until two honest
nodes are as far apart after a few layers as a manipulated node is from
an unmanipulated one. And crucially: **a tolerance band is adaptively
attackable.** An attacker who knows the test aims the manipulation at it.
In simulation, a manipulation that computed only the checked components
correctly went undetected across ten subsequent layers.

*In the whitepaper:* chap. 6.3

---

## C. Fixed point and quantisation

If floating point is forbidden, how do you represent a number like 0.375?
Answer: as a whole number with an **agreed denominator**.

### Fixed-point arithmetic

You agree that a stored integer really means multiples of 1/2ⁿ. That n is
called **frac_bits** (fractional bits).

*Example:* with `frac_bits = 8`, the stored number `96` really means
`96 / 256 = 0.375`. Stored `256` means `1.0`. The resolution is
`1/256 ≈ 0.0039` — nothing finer exists; everything between is rounded.

Multiplying two fixed-point numbers adds their frac_bits: `a` with 8
fractional bits times `b` with 7 gives a product with 15, which is then
brought to the target scale by an
→ [arithmetic right shift](#arithmetic-right-shift). That is what
`rescale` does.

*In code:* `INTEGER_LLM/kernels/src/fixed_point.rs` — `rescale`,
`rescale_i64`, `mul_i16_i64`

### Scale and power-of-two scales

A tensor's **scale** is the factor translating a stored integer into the
real value. Myelith uses **power-of-two scales only** (1/2, 1/4, 1/8 …),
because then the conversion is a shift rather than a division — and
division would be platform-dependent (see above).

### Quantisation

Converting a model trained in floating point into an integer one. For
each weight tensor you determine which scale best fits its values, then
round all values onto it.

*Example:* a weight of `0.0731` at scale `1/1024` becomes
`round(0.0731 × 1024) = 75`. Converting back: `75/1024 = 0.07324` — the
quantisation error is `0.00014`.

*In code:* `INTEGER_LLM/calibrate/src/quantize.py`, `scales.py`

### W8A16

Myelith's quantisation scheme: **W**eights 8 bit, **A**ctivations 16 bit.
Weights are `int8` (−128 … 127), activations `int16` (−32,768 … 32,767).

**Why not 8 bit for both?** Because real activation values exceed the
int8 range — RMSNorm and MLP outputs up to about ±1640 have been measured.
In int8 that would be hard truncation at 127, a factor of 13 lost.

**Why only 8 bit for weights?** Memory. A 7-billion-parameter model needs
about 7 GB in int8 and 14 GB in int16 — which decides how many graphics
cards can take part.

*In code:* `INTEGER_LLM/kernels/src/linear.rs` — `linear_w8a16`

### Per-channel scales

Instead of one scale per tensor, **each output row** gets its own. This
matters because value ranges vary strongly within a tensor: a row with
values up to 0.01 and one with values up to 5.0 would otherwise share a
scale, and the fine row would lose nearly all of its resolution.

*In code:* `linear_w8a16_pc` in `kernels/src/linear.rs`; the shifts live
as a `shifts` array in the artifact (`runtime/src/loader.rs`,
`QTensor::shifts`)

### Massive activations / outlier channels

Transformers have the property that individual activation dimensions
carry values **hundreds of times larger** than all the others (in the
literature: *massive activations* or *attention sinks*). Quantise
per tensor and you aim the scale at that outlier, throwing away all
resolution for every other channel. That is the main reason for
per-channel scales.

*Practical consequence for measurement:* the error metric **absmax**
(largest deviation) tracks only that one outlier channel and says almost
nothing about the rest. This project therefore measures error as
**relative L2** over the whole vector (→ [relative L2](#relative-l2)).

### Scale pack

The part of the artifact build that is **not** deterministic — and is
therefore versioned rather than recomputed.

Building an artifact from HF weights is bit-reproducible on the **same**
machine (measured: 593 of 593 files). Across machines it is not:
→ [calibration](#calibration) computes in floating point, and **3 of 314**
scale entries sit within 0.01 % of a power-of-two boundary. A different
BLAS version is enough to flip one — and a flipped shift changes the
artifact bytes, hence the model.

The rest is exact: `round(W · 2^shift)` with an integer shift, because
multiplying by a power of two is exact in IEEE floating point. **So
Myelith distributes the scales, not the weights** — 1.8 MB for both models
instead of 8.8 GB, and the local build becomes bit-identical across
platforms. Side effect: 40 seconds instead of 20 minutes for the 7B model.

*In code:* `INTEGER_LLM/scale_packs/`, loader in
`calibrate/src/scale_pack.py`, generator in `tools/skalenpaket_bauen.py`,
verification via `myl-test artefakte`

### Calibration

The process that determines the scales. Real text (here: 64 WikiText-2
sequences) is pushed through the floating-point model, the value ranges
that actually occur are measured at every point, and the scales are
chosen from them.

**Important:** calibration is the only place in the project where
floating-point arithmetic is allowed — it is preparation, not the compute
path. Its output is integer artifacts.

*In code:* `INTEGER_LLM/calibrate/src/main.py`
*Invocation:* `INTEGER_LLM_MODEL=qwen2.5-7b python -m calibrate.src.main`

### GPTQ

A method that does not merely measure quantisation error but
**compensates** for it: the error of a rounded weight is pushed onto the
neighbouring weights not yet quantised. It is state of the art and
expensive (hours instead of minutes).

**Disabled by default here.** Reason: in a comparison across three
calibration runs, GPTQ turned out to be exactly neutral on our path — it
gained nothing while costing 2.5 hours per 7B run instead of 20 minutes.
As long as the error source lies elsewhere, a method that improves the
*weight* error has no effect. Enable with `INTEGER_LLM_GPTQ=1` for final
artifact production.

*In code:* `INTEGER_LLM/calibrate/src/gptq.py`, decision in
`main.py::gptq_entscheidung()`

### LUT (lookup table)

Non-linear functions such as exp, SiLU or the reciprocal square root
cannot be expressed with addition and shifting. Instead of computing
them, they are **tabulated in advance**: the input value becomes the
index, the table entry is the result.

*Example:* the exp LUT at `exp_input_frac_bits = 8` holds
`round(exp(-i/256) · 2¹⁴)` at index `i`. To get `exp(-0.5)` you look up
index `128`.

A LUT has two separate resolutions that must not be confused:
- **input grid** (`input_frac_bits`) — how finely the x-axis is sampled,
- **output resolution** (`output_frac_bits`) — how finely the result is
  represented.

Both are part of θ_v.

*In code:* `kernels/src/integer_math.rs::lut_lookup`, generation in
`calibrate/src/luts.py`

### Relative L2

This project's error metric: the length of the difference vector divided
by the length of the reference vector, in per cent.

```
rel_L2 = 100 · ‖integer − float‖ / ‖float‖
```

It measures the **bulk error** across all channels, not the largest
single excursion. That is exactly why it is used here: absmax would track
only the → [outlier channel](#massive-activations--outlier-channels).

### Perplexity

The quality measure for language models: how "surprised" is the model by
the next real word? Lower is better. A perplexity of 9 means roughly that
the model is as uncertain as if it had to guess among 9 equally likely
words.

Myelith does not measure absolute perplexity but the **relative increase**
against the floating-point original. The acceptance criterion is ≤ 5 % and
has been met on both models since θ_v 0.17.0: 0.5B **+2.11 %**, 7B
**+1.14 %**. For comparison, the floor of the quantisation scheme itself —
everything in float except the → [W8A16](#w8a16) quantisation — sits at
**+0.84 %**. The 0.30-point gap is the entire remaining implementation
loss.

*In code:* `INTEGER_LLM/eval/perplexity.py`

### θ_v (theta-v, model version)

The complete execution specification: **weights + quantisation scheme +
all arithmetic commitments**. θ_v is consensus-relevant — every node must
use the same version, otherwise the hashes diverge and the redundancy
comparison fails without anyone having cheated.

It covers: bit widths, accumulator width, overflow behaviour, LUT
coefficients and grids, the rules of dynamic quantisation, and the
commitment to the arithmetic right shift.

**Not covered** and therefore free: kernel implementation, parallelisation
strategy, block sizes, memory layout.

*In code:* `INTEGER_LLM/theta_v/spec.json`, checked at load time in
`runtime/src/loader.rs::verify_version_against_spec`
*In the whitepaper:* chap. 6.1, 6.5

---

## D. The language model from the inside

This section explains what actually happens inside a transformer — as far
as is needed to understand the implementation in `INTEGER_LLM/`.

### Token and tokenizer

A **token** is a text building block — usually part of a word rather than
a whole one. "Incomprehensibility" might split into `In`, `compre`,
`hens`, `ibility`. The **tokenizer** translates text into token numbers
and back.

*Example:* `" The history of"` → `[576, 3840, 315]`

*In code:* `INTEGER_LLM/runtime/src/tokenizer.rs` (BPE via the
HuggingFace `tokenizers` crate; the encoding path is float-free)

### Embedding

A lookup table translating each token number into a vector — 896 numbers
for Qwen2.5-0.5B, 3584 for 7B. That vector is the initial state which the
layers then reshape step by step.

*In code:* `runtime/src/model.rs::embed_token`

### Layer

The transformer consists of identical layers (0.5B: 24, 7B: 28). Each has
the same structure:

```
RMSNorm → attention → residual add
       → RMSNorm → MLP       → residual add
```

*In code:* `runtime/src/model.rs::forward_layer`

### Residual stream / residual add

The result of each sub-operation is not substituted but **added**:
`x = x + attention(norm(x))`. The vector passing through is the
**residual stream**. Without this shortcut deep networks would not train;
for us the significance is different: the residual stream is the channel
along which quantisation errors propagate through all layers and
accumulate.

> **Why the order of clamping and adding matters (finding 31).**
> The incoming residual and the block contribution sit on different
> scales. Clamp one to the target scale **first** and add **afterwards**,
> and you destroy any cancellation: both operands can be large while only
> their sum is small — and the target scale is calibrated for the sum.
>
> Measured on Qwen2.5-0.5B, layer 21, channel 62 (the channel carrying the
> → [massive activation](#massive-activations--outlier-channels)): the true
> value drops there from 1714 to 61.6. The old version computed
> `1723 → clamped 64.00` plus `−1653 → clamped −64.00` and arrived at
> **−0.002**. Two clamps cancelling each other out.
>
> Since θ_v 0.17.0 the addition happens in i64 on the **coarser** of the
> two scales, with a single rescale and a single clamp at the end: 63.998
> instead of −0.002. The underlying rule is general and holds for any
> fixed-point computation: **accumulate wide, round once.**

### RMSNorm

A normalisation: the vector is divided by its root mean square so that
magnitudes stay stable across layers, then multiplied channel-wise by
learned factors (γ, *gamma*).

```
y = x / sqrt(mean(x²) + ε) · γ
```

The square root is the integer problem — it is solved with an
→ [rsqrt LUT](#lut-lookup-table).

*In code:* `kernels/src/rmsnorm.rs::rmsnorm_i16`

### Attention

The mechanism by which each position in the text reaches back to earlier
positions. Three quantities are computed from the input:

- **query (q)** — "what am I looking for?"
- **key (k)** — "what do I offer?"
- **value (v)** — "what do I contribute if I'm chosen?"

Then: every query is combined with every key (dot product) → these are
the **scores**. The scores go through → [softmax](#softmax) and become
weights summing to 1. The result is the weighted sum of the values.

> *Example.* In "The cat that sat on the mat was tired", the model needs
> to know at "was" who is tired. The query of "was" matches the key of
> "cat" well → high weight → the value of "cat" dominates the output.

**Causal** means a position may only look backwards, never forwards. This
is enforced with a mask (masked positions get score `i32::MIN`, hence
weight 0).

*In code:* `kernels/src/attention.rs::attention_int`

### Head and GQA

Attention does not run once but in parallel across several **heads**, each
working on a slice of the vector and learning different relations.
`head_dim` is the size of one head (7B: 128).

**GQA** (grouped query attention): there are more query heads than
key/value heads; several query heads share one KV head. This saves memory
in the → [KV cache](#kv-cache). Qwen2.5-0.5B: 14 query heads, 2 KV heads.

*In code:* `runtime/src/model.rs`, `split_heads` + `group_size`

### RoPE (rotary position embedding)

How does the model learn **where** in the text a token sits? RoPE rotates
the q and k vectors by an angle depending on the position. Two positions
the same distance apart share the same relative rotation — so the dot
product q·k automatically encodes distance.

Each dimension **pair** j has its own frequency
`θ_j = 1/rope_theta^(j/(head_dim/2))`; the pairing is *half-split*, i.e.
`(x_j, x_{j+head_dim/2})`.

*Why this is here:* an earlier version used **one** angle for all pairs
and adjacent pairing. That was the dominant source of perplexity error
(→ finding 15).

*In code:* `kernels/src/rope.rs::rotate_half_split_i16`

### Softmax

Turns arbitrary numbers into probabilities summing to 1:
`softmax(z)_i = exp(z_i) / Σ exp(z_j)`.

In integers this is solved as follows: every score is subtracted from the
largest score (the difference is ≥ 0), the result indexes the exp LUT,
then everything is divided by the sum.

> **Why resolution matters more here than it looks (finding 29).**
> At `prob_frac_bits = 8` the finest representable weight is 1/256. Every
> position whose weight falls below that rounds **individually to zero**
> and contributes exactly nothing — no matter how many such positions
> there are. With one dominant peak and a flat tail:
>
> | Context length | tail weight at 1/256 | at 1/16384 | exact |
> |---|---|---|---|
> | 128 | 0.4961 | 0.2403 | 0.2394 |
> | 512 | **0.0000** | 0.5614 | 0.5588 |
> | 2048 | **0.0000** | 0.8746 | 0.8354 |
>
> At 512 positions the entire tail vanishes: attention collapses onto the
> peak position. At 128-token sequences — the length of our perplexity
> measurement — the effect is only a doubling and stays below the
> measurement threshold. **A defect the evaluation cannot see is still a
> defect.** Fixed in θ_v 0.16.0.

*In code:* `kernels/src/softmax.rs::softmax_int`

### MLP / feed-forward and SiLU

The second block of each layer. Qwen2.5 uses a **gated MLP**:

```
gate = W_gate · x      up = W_up · x
y = W_down · (SiLU(gate) ⊙ up)
```

**SiLU** (also *swish*) is the non-linearity: `SiLU(x) = x · σ(x)` with
the sigmoid σ. Solved as a LUT in integers.

*Why this is here:* the operation-by-operation comparison showed the
MLP's matrix multiplications to be practically exact (0.01 %), while the
**entire** MLP error arose in the SiLU LUT (6.83 %). Raising the LUT
resolution in θ_v 0.15.0 was one of the project's two big steps forward.

*In code:* `kernels/src/mlp.rs::mlp_int`

### KV cache

Generating text proceeds token by token. Without a cache, every new token
would require pushing the entire preceding sequence through attention
again. The **KV cache** stores the keys and values already computed, so
each step only adds the new position.

*Important commitment (finding 22):* the cache holds K/V in the **native
per-layer scale** of the producing projection, with no conversion to a
global cache scale. The earlier round trip `k_frac → 8 → k_frac` gained
nothing but cost double rounding and 2–4 bits of resolution on almost
every layer — plus hard truncation wherever the real value exceeded the
fixed capacity (7B layer 0: K absmax 420, a factor of 3.28 lost, and that
at the *first* layer, whose error propagates through all 28).

*In code:* `runtime/src/kv_cache.rs`

### Prefill and decode

**Prefill** — the prompt is processed; all positions can be computed in
parallel. **Decode** — the answer is generated; each token depends on the
previous one, so it is strictly sequential. Decode is the slow part and
the reason → [micro-batching](#micro-batching) helps throughput.

*In code:* `runtime/src/generate.rs`

### LM head and logits

The final layer projects the vector onto the vocabulary size (Qwen2.5:
151,936). The output values are the **logits**; the largest is the most
likely next token.

### Sampling

How does a token come out of the logits? **Greedy** takes the largest
(`argmax_int`). **CDF sampling** draws with weights — using a
deterministic PRNG (`splitmix64`) whose seed is derived from `segment_id`
and `block_hash`. That makes even the dice roll reproducible, which is
mandatory for the redundancy comparison.

*In code:* `kernels/src/sampling.rs`, `kernels/src/prng.rs`

---

## E. Cryptographic building blocks

### Hash / SHA-256

A one-way function turning arbitrarily many bytes into 32. The same input
always gives the same hash; finding an input that produces a *given* hash
is practically impossible.

*What it is for here:* rather than transmitting and comparing activations
(megabytes), you compare their hashes (32 bytes). That is why the
→ [computation trace](#computation-trace) stays small.

**Constant-time comparison:** equality is checked via
`subtle::ConstantTimeEq`, so the comparison time carries no information
about where two hashes first differ. For an open-source protocol whose
code the attacker knows, that is not a luxury.

*In code:* `SHARED_TYPES/myl-types/src/hash.rs`

### Merkle tree

A tree of hashes: the data sits in the leaves, every inner node is the
hash of its two children, and the **root** sits on top. The benefit: you
can prove a particular leaf belongs to the tree without showing the whole
tree — the path from leaf to root suffices (**Merkle proof**, `log₂(n)`
hashes).

> *Example.* For a corpus of one billion documents, proving that document
> no. 734,891,202 belongs takes about 30 hashes — under a kilobyte
> instead of terabytes.

**Domain separation:** leaves and inner nodes are hashed with different
prefixes. Without it an attacker could pass an inner node off as a leaf
and construct false proofs.

*What it is for here:* the θ_v root, PoI bundle roots, corpus provenance
in training.

*In code:* `SHARED_TYPES/myl-types/src/merkle.rs`

### Signature and BLS12-381

A **digital signature** proves a message came from whoever holds the
secret key. **BLS12-381** is a signature scheme on an elliptic curve with
a special property: signatures can be **aggregated**.

*Example:* 21 validators sign the same block. Instead of 21 signatures
(21 × 96 bytes), the block stores **one** aggregate signature (96 bytes),
verified against all 21 public keys at once. For a blockchain that stores
every block forever, that is a substantial difference.

Myelith uses the **min-pk** variant (public keys on G1, 48 bytes;
signatures on G2, 96 bytes) — the same one as Ethereum consensus.

*In code:* `SHARED_TYPES/myl-types/src/bls.rs`

### Rogue-key attack and proof of possession

The price of aggregation. Anyone free to choose their public key can
construct it so that it "absorbs" other people's signatures:

```
pk_rogue = g₁^x · pk_victim⁻¹
```

With that key the attacker can produce an aggregate signature that looks
valid for the group `{victim, attacker}` even though the victim never
signed.

**Countermeasure: proof of possession (PoP).** On registration every
participant must sign **their own** public key once — under a separate
domain-separation tag. Anyone who built their key as a combination of
other people's keys does not hold the corresponding secret key and cannot
produce that proof.

For Myelith this is not optional: `fast_aggregate_verify` is exactly the
function that is attackable without PoP, and it is used on every PoI
bundle check.

*In code:* `bls.rs::prove_possession` / `verify_possession`, tests in
`SHARED_TYPES/myl-types/tests/rogue_key.rs` — which deliberately asserts
**both the vulnerability and its fix**, so that a later refactor cannot
silently remove the protection.

### VRF (verifiable random function)

A random function with a proof. Whoever holds the secret key can turn an
input into a random output **plus a proof** that it was formed correctly.
Anyone else can check the proof without knowing the key.

*What it is for here:* when drawing lots for who computes which segment,
nobody may influence the draw — but everybody must be able to check it.
That is exactly what a VRF provides.

Myelith uses **ECVRF-EDWARDS25519-SHA512-TAI** per RFC 9381, checked
against the official test vectors in appendix B.3.

*In code:* `SHARED_TYPES/myl-types/src/vrf.rs`

### Erasure coding (Reed–Solomon, Cauchy form)

A way of splitting data so it survives the loss of individual parts. With
`k = 8` data fragments and `m = 4` parity fragments you get 12 pieces, and
**any 8 of them** suffice for complete reconstruction.

> *Example.* A pod archives a segment's activations as 12 fragments held
> by 12 different nodes. If four fail — whichever four — the original can
> be restored. Only the fifth failure loses it.

**Cauchy rather than Vandermonde:** both constructions yield
Reed–Solomon codes, but with the Cauchy form **every** k×k submatrix is
guaranteed invertible. With the Vandermonde form over GF(2⁸) that does
not hold for all subsets — so there would be combinations of 8 fragments
from which reconstruction fails. For a dispute in which an attacker gets
to choose which fragments to withhold, that is the difference between
safe and unsafe.

*Verification:* the test checks **all 495 subsets** of 8 from 12.

*In code:* `SHARED_TYPES/myl-types/src/erasure.rs`

### GF(2^8) Galois field

The arithmetic Reed–Solomon computes in: 256 elements (exactly one byte),
with addition = XOR and its own multiplication. The advantage: every
operation is exact, with no rounding and no overflow — byte arithmetic
that behaves like field arithmetic.

### Borsh

The protocol's serialisation format (*Binary Object Representation
Serializer for Hashing*). What matters is its **canonicity**: each value
has exactly one byte representation. With JSON you could reorder fields
or insert whitespace and get a different hash for the same content —
fatal for a system that compares hashes.

**Consequence for developers:** Borsh serialises in **declaration order**.
Reordering struct fields changes every hash over that struct and is
therefore a consensus break, not a refactor.

*In code:* `SHARED_TYPES/myl-types/src/core_types.rs`

### Domain-separation tag (DST)

A prefix placed before signing or hashing so that signatures from one
context do not hold in another.

> *Example.* Without a DST, a vote signature for round 5 would also be a
> valid commit signature for round 5 — an attacker could reinterpret
> agreement as commitment. With separate tags (`MYL_PROPOSE_v1`,
> `MYL_VOTE_v1`, `MYL_COMMIT_v1`) that is ruled out.

*In code:* `CONSENSUS/myl-consensus/src/signing.rs`

---

## F. Consensus

### Byzantine fault

A node that does not simply fail but behaves **arbitrarily maliciously**:
contradictory messages to different recipients, forged values, targeted
silence. The name comes from the *Byzantine Generals Problem* (Lamport et
al., 1982).

### BFT (Byzantine fault tolerance)

A consensus procedure that stays correct as long as fewer than a third of
participants are Byzantine. Why a third? Because with `n` nodes and `f`
faulty ones you can only decide safely if `n > 3f` — otherwise a set of
`n − f` responses can arise in two ways whose results contradict.

**In Myelith it is not the count that matters but the
→ [voting weight](#voting-weight).**

*In code:* `CONSENSUS/myl-consensus/src/bft.rs`

### Safety and liveness

The two properties a consensus should have:

- **Safety** ("nothing wrong happens") — two contradictory blocks are
  never finalised.
- **Liveness** ("eventually something happens") — *some* block is
  eventually finalised.

They are independent. A protocol that never decides anything is perfectly
safe and completely useless. Exactly that case arose in this project: the
single-round protocol in `bft.rs` was safe but stalled when the leader
failed — hence `round_change.rs`.

### Propose / vote / commit

The three steps of a consensus round:

1. **Propose** — the round's leader proposes a block.
2. **Vote** — validators agree (pre-vote).
3. **Commit** — once enough voting weight has gathered, it is committed.

Only when the commit threshold is also reached does the block count as
finalised.

*In code:* `bft.rs`, messages in `signing.rs`

### Quorum and the 2/3 threshold

The **quorum** is the voting weight required for a valid step: more than
two thirds of the total. The reason is overlap — two sets each above 2/3
necessarily share a member holding more than 1/3 of the weight. If the
two sets backed different blocks, that intersection would have voted
contradictorily and is thereby convicted as Byzantine.

*In code:* `bft.rs::quorum_threshold`

### Lock and polka certificate

What happens if a round fails after somebody has already seen a block
that *nearly* made it?

A validator who has seen a quorum of votes for block B **locks** onto B:
in later rounds they vote only for B — unless shown proof that a later
round had a quorum for something else. That proof is the **polka
certificate** (*proof of lock change*): a collection of votes
demonstrating the quorum.

Without locking, a network with rotating leaders could finalise two
different blocks in different rounds — a safety break. The mechanism
comes from Tendermint.

**Hardening:** a polka certificate's voter list must be sorted **strictly
ascending**. Otherwise an attacker could enter the same validator
repeatedly and fake a quorum.

*In code:* `CONSENSUS/myl-consensus/src/round_change.rs`

### Timeout and round change

If the leader does not deliver, the protocol must move on. The timeout
grows **linearly** with the round number (`base + round · delta`, with
saturation). Reason: in an asynchronous network the timeout must
eventually exceed the actual message latency, otherwise the network
changes rounds forever without ever finishing.

*In code:* `round_change.rs::TimeoutConfig`

### GST (global stabilization time)

The point, in the *partial synchrony* model, from which messages arrive
again within a known bound. Before GST, BFT guarantees only safety; after
GST, liveness too. That split is not a trick but a proven boundary: in a
fully asynchronous network, deterministic consensus is impossible (the
FLP result).

### Double signing

A validator signs **two different blocks in the same round**. This is the
classic consensus attack and is punished with 30–100 % of stake.

**Burden of proof:** a double-signing proof is only worth something if any
third party can check it. A proof therefore consists of both signatures
together with the signed messages — not of an assertion.

*In code:* `CONSENSUS/myl-consensus/src/double_signing.rs`

### Committee, leader, arbiters

Each epoch elects **21 block-production validators** and **7 arbiters**
(for → [adjudication](#adjudication)). Selection is weighted by stake but
randomised by VRF (`weighted_sample_without_replacement`), so the
composition is not predictable. The **leader** of a round rotates
deterministically.

*In code:* `validator.rs` — `COMMITTEE_SIZE = 21`, `ARBITER_COUNT = 7`

### Voting weight

The coupling that turns an ordinary proof-of-stake system into a Myelith
system:

```
voting_weight = stake + (stake · decayed_work) / VTFE_UNIT
```

Weight is fed by staked coin **and** demonstrated historical inference
work (with a decay factor). Anyone who wants to attack the network must
therefore either buy coins on a large scale — driving up the price and
their own cost — or perform honest work indefinitely, which contradicts
the goal of the attack.

*In code:* `CONSENSUS/myl-consensus/src/voting_weight.rs`

### Ledger and state transition

The **ledger** is the bookkeeping: accounts, balances, stake, credits.
Every **state transition** is a **pure function** `(State, transition) →
State` with no hidden global state, and failures leave the state unchanged
(check first, then modify).

This construction is not a matter of style: it is the only way all nodes
can replay the same chain of transitions and arrive at exactly the same
state.

*In code:* `CONSENSUS/myl-ledger/src/transitions.rs`

### Gossip

How messages spread in the P2P network: every node forwards what it
receives. Myelith uses **libp2p gossipsub** with separate topics per
message class (blocks, transactions, PoI bundles, challenges, latency
attestations), so that large PoI bundles do not slow down block gossip.

**Validation before forwarding:** gossipsub runs in `validate_messages()`
mode — a message is only forwarded once the node has checked and accepted
it. Otherwise the network would be a free amplifier for spam.

*In code:* `NETWORKING/myl-net/src/gossip.rs`, `validation.rs`

---

## G. Epochs, scheduler and pods

### Epoch

The protocol's clock. Within an epoch the assignments are fixed: who is in
which pod, who computes which segments, who sits on the committee. At the
end of the epoch accounts are settled (→ [epoch close](#epoch-close)) and
lots are drawn again.

### Shard

A contiguous section of the model — a group of layers — held in memory by
a single miner.

> *Example.* A 28-layer model on 4 shards: miner A holds layers 0–6,
> B 7–13, C 14–20, D 21–27. None of them has the whole model.

*In code:* `COMPUTE_PIPELINE/myl-pod/src/shard.rs`,
`INTEGER_LLM/runtime/src/model.rs::run_layers`

### Pod

A group of miners that together form a **complete** model — each holding
one shard, and strung together they make the whole transformer. A pod can
answer a request on its own.

*In code:* `CONSENSUS/myl-scheduler/src/lib.rs::Pod`

### Pipeline parallelism

Activations pass through the shards in order: A computes its layers,
sends the result to B, B continues, and so on. Unlike tensor parallelism
(where a single matrix multiplication is split), network traffic occurs
**only at shard boundaries** — decisive when the nodes are not in the same
data centre.

### Micro-batching

The coordinator collects incoming requests over a time window (default
250 ms) and sends them through the pipeline together. While shard B works
on batch 1, shard A can already start batch 2.

**The point is throughput, not latency.** An individual request does not
get faster — on the contrary, it waits up to 250 ms. But the total number
of requests per second rises substantially, because no shard idles.

*In code:* `COMPUTE_PIPELINE/myl-pod/src/micro_batch.rs`

### Segment

The unit of accounting and verification: a tuple `(x, θ_v, π, y)` of
input commitment, model version, pipeline path and output commitment.

*In code:* `SHARED_TYPES/myl-types/src/core_types.rs::Segment`

### Computation trace

The chain of hashes over the intermediate results: `h(a₀), h(a₁), …`,
where `aᵢ` are the activations after shard i. Each shard signs its
transition:

```
sig_i( h(a_{i−1}) ‖ h(a_i) ‖ segment_id )
```

The trace is what makes the → [bisection game](#bisection-game) possible
**without storing activations on chain**: you compare 32-byte hashes
instead of megabytes and thereby locate the first deviation.

*In code:* `COMPUTE_PIPELINE/myl-pod/src/trace.rs`

### Epoch scheduler

The deterministic procedure that fixes each epoch's assignments. It runs
**identically on every node** — there is no central authority, everyone
can recompute everything.

Six steps:

1. **Derive VRF seed** — from the finalised block of the previous epoch
   (`vrf_seed.rs`)
2. **Filter miners** — by hardware class and registration deadline
   (`miner_filter.rs`)
3. **Geo clustering** — under a latency constraint (`geo_clustering.rs`)
4. **Shard assignment** — Fisher–Yates shuffle with the seed
   (`shard_assignment.rs`)
5. **Redundancy assignment** — 2 disjoint, zone-diverse pods per segment
   (`redundancy.rs`)
6. **Sampling lottery** — mark segments for checkers (`sampling.rs`)

*In the whitepaper:* appendix A.2

### Grinding

An attack on random draws: the attacker tries inputs until the result
suits them. Two countermeasures:

- The **seed comes from an already finalised block** of the previous
  epoch — it is fixed before anyone could exploit it.
- **Registration closes two epochs in advance**, so nobody can insert
  prepared identities at the last minute.

### Fisher–Yates shuffle

The standard algorithm for a fair shuffle: walk the list from the back
and swap each element with a randomly chosen one from the not-yet-processed
part. Every arrangement is equally likely. In Myelith the randomness comes
from the VRF seed — deterministic and recomputable by all.

*In code:* `SHARED_TYPES/myl-types/src/seed_rng.rs::deterministic_shuffle`

### Zone diversity

The two redundant pods of a segment must come from **different geographic
regions and autonomous systems (AS)**. Otherwise a single data-centre
outage — or a single operator — could hit both pods at once, and the
redundancy comparison would be worthless.

*In code:* `SHARED_TYPES/myl-types/src/node_metadata.rs::DiversityChecker`,
`CONSENSUS/myl-scheduler/src/redundancy.rs`

### LatencyGraph and latency attestations

Pods should consist of nodes **close together** (because of pipeline
traffic), while the **two pods of a segment should be far apart** (because
of zone diversity). For that the network needs a map of round-trip times.

Every node continuously measures round-trip time to its peers via
ping/pong, smooths it with an **EMA**, and periodically publishes signed
**latency attestations**. From those, all nodes build the same
`LatencyGraph`.

*In code:* `NETWORKING/myl-net/src/latency.rs`,
`SHARED_TYPES/myl-types/src/latency_attest.rs`

---

## H. Verification

### The three levels

| Level | Method | Cost | When |
|---|---|---|---|
| **1** | Redundancy comparison of two pods | +100 % compute | always |
| **2** | Checkers recompute samples | 1–3 % of volume | continuously |
| **3** | zkML anchor (zero-knowledge proof) | very high | optional, premium |

Level 3 is **not yet implemented** — it is envisaged as an upgrade path
once zkML systems become efficient enough. Integer execution suits that
path, because arithmetic circuits over integers are considerably simpler
to formulate than over floating point.

*In code:* `VERIFICATION/myl-verifier/src/lib.rs`

### Redundancy comparison (level 1)

Two independently drawn pods compute the same segment. If the commitment
hashes match at **all** trace positions, the segment counts as
provisionally confirmed. The comparison is binary and parameter-free.

*In code:* `VERIFICATION/myl-verifier/src/redundancy.rs`

### Delivery modes

When is the comparison made — before or after delivery? Both are
selectable per request:

- **Optimistic** (default) — the answer from whichever pod finishes first
  goes out immediately, reconciliation runs asynchronously. Latency as
  with a single pod; security acts after the fact via slashing and
  clawback.
- **Confirmed** (surcharge) — the answer is withheld until the twin pod
  agrees. A manipulated result never reaches the user unless both pods
  collude. The price is latency and a fee.

For a research query the after-the-fact sanction suffices; for an agent
decision with financial effect the preventive variant is appropriate.

*In code:* `VERIFICATION/myl-verifier/src/delivery.rs`

### Challenge

The on-chain artifact by which a checker reports a deviation and opens the
bisection game. It names the first deviating trace position.

**A challenge costs a deposit.** Whoever challenges frivolously loses it —
otherwise challenging would be a free denial of service.

*In code:* `SHARED_TYPES/myl-types/src/challenge.rs` (the type lives in
the shared types because three components use it),
`VERIFICATION/myl-verifier/src/challenge.rs`

### Bisection game

The heart of dispute resolution: how do you find the one wrong step
without repeating the whole computation on chain?

Answer: **binary search over the computation trace**, in O(log L) rounds.

> *Example with 8 shard transitions.* Miner and checker disagree about the
> final result.
> - Round 1: both present their hash after transition 4. Different → the
>   error is in 1–4.
> - Round 2: hash after transition 2. Same → the error is in 3–4.
> - Round 3: hash after transition 3. Same → the error is transition 4.
>
> Three rounds, and it is settled which single step is disputed.

The miner then discloses the input activations of that one step (from the
→ [DA layer](#da-data-availability)), the arbiter committee recomputes
**one** shard forward pass and compares. The loser is slashed, the winner
receives a bounty.

**Why blame is unambiguous:** there is exactly one correct result (integer
determinism!), and the comparison is a hash equality with no room for
judgement. Validators need no special hardware and no certified kernel
implementation.

The procedure comes from Truebit and Arbitrum.

*In code:* `VERIFICATION/myl-verifier/src/bisection.rs`

### Adjudication

The on-chain decision at the end of the bisection: the committee executes
the disputed shard forward pass per θ_v and compares the hash. Execution
itself is abstracted behind the `ShardExecutor` trait, so that the
adjudication logic stays testable independently of the concrete inference
implementation.

**Cost:** a single shard forward pass on about seven validators —
constant, independent of segment length, and never due in normal
operation.

*In code:* `VERIFICATION/myl-verifier/src/adjudicate.rs`

### Slashing

Confiscation of deposited stake as a penalty.

| Actor | Reason | Amount |
|---|---|---|
| Shard miner | wrong result (proven by bisection) | 100 % |
| Shard miner | unavailability during a session | 1–5 % |
| Pod coordinator | false PoI aggregation | 100 % |
| Validator | double signing / proven censorship | 30–100 % |
| Checker | frivolous challenge | deposit |

*In code:* `VERIFICATION/myl-verifier/src/slash.rs` (decides **who** lost),
`CONSENSUS/myl-ledger/src/transitions.rs::apply_verdict` (books the
**amounts** — the split is deliberate: amounts are governance parameters,
guilt is a proof)

### Incentive inequality

The economic security condition. With sampling rate `p`, stake `S` and
fraud gain `g` per segment:

```
S_min = g / p²
```

At `p = 2 %` and `g` = the reward of one segment, `S_min = 2500` segment
rewards — about twelve epochs of income.

**The quadratic dependence is the design's most important lever.** It
explains why the bootstrap phase runs an elevated sampling rate: at 50 %
instead of 2 % the stake requirement falls to one six-hundredth. That
costs capacity which, in a phase of over-capacity, is idle anyway.

*In the whitepaper:* chap. 5.5, appendix B.1

### Canary segments

The gap that levels 1 and 2 leave open: a **single** intervention by an
attacker controlling **both** pods. Redundancy does not help (both lie
identically), sampling only helps if they offend repeatedly.

Canary segments partly close it: the network holds a stock of segments
whose correct result is already known and injects them into the regular
job stream at a rate γ. To the miner they are indistinguishable from real
requests.

**The gain lies in the attacker's uncertainty:** because they never know
whether a given segment is a canary, even the *first* manipulation
attempt carries a detection risk of γ. At γ = 2 % and full stake loss,
the expected value of a single attack is negative.

Three construction requirements: **indistinguishability** (real prompt
distribution, unremarkable timing and length profile), **stock renewal**
(a static pool becomes recognisable over time) and **cost honesty**
(γ is pure overhead and enters the cost structure).

*In the whitepaper:* chap. 6.7 — not yet implemented.

### DA (data availability)

Complete prompts and outputs do not belong on chain (privacy, volume).
Only commitments go on chain; the raw data sits
→ [erasure-coded](#erasure-coding-reedsolomon-cauchy-form) with the
participating pods — for the duration of the **dispute window**.

**A hardening decision that is easy to miss:** `fetch` checks the dispute
window **before** the lookup. Otherwise the behaviour would reveal whether
data had expired or was being withheld — and "withhold it" would become a
strategy.

*In code:* `CONSENSUS/myl-consensus/src/da.rs`,
`COMPUTE_PIPELINE/myl-pod/src/da.rs`

### Dispute window

The period (draft: 7 days) during which a segment can still be challenged.
While it runs, the DA fragments must be retained; afterwards they may
disappear and the segment is final.

---

## Proofs of work: PoI and epoch close

### PoI (proof of inference)

Myelith's proof of work — the counterpart to the hash value in
proof-of-work, except the work is **useful**. A PoI attests that a pod
actually computed a segment.

**Why not "inference PoW"?** You could pick the block producer directly by
an inference race: whoever computes first writes the block. That would be
manipulable through the inputs (→ [grinding](#grinding)) and would couple
block time to inference latency. Instead there are **two decoupled
processes**:

- **Process A — block production (fast).** BFT committee, block time 1–2 s.
- **Process B — proof of work (continuous).** Pods submit signed PoI
  bundles once per epoch.

The only coupling between them is the
→ [voting weight](#voting-weight).

*In the whitepaper:* chap. 3.5

### PoI bundle

What a pod submits at the end of an epoch: a Merkle root over (input
commitment, output commitment, segment metadata, signatures of all
participating shard miners), signed in aggregate.

**The critical hardening:** the set of admissible signers (`PodMembership`)
comes **from the scheduler, never from the bundle itself**. If it came
from the bundle, an attacker could simply supply the member list and sign
for an arbitrary pod — the signature check would pass and still prove
nothing. In addition, every member needs a registered
→ [proof of possession](#rogue-key-attack-and-proof-of-possession).

*In code:* `SHARED_TYPES/myl-types/src/core_types.rs::PoIBundle`,
`CONSENSUS/myl-consensus/src/poi.rs`

### Epoch close

The step from **claimed** to **confirmed**. PoI submission establishes that
a pod asserts a quantity of work; the epoch close establishes whether it
is owed. Only then is anything minted.

For each segment, the results of the two pods are compared. Three
outcomes:

- **Match** — both agree → confirmed
- **Mismatch** — they differ → nothing is credited, challenge
- **Missing** — the twin pod submitted nothing → **not** confirmed

**Why `Missing ≠ Match`:** if a missing twin counted as agreement, "make
the witness unreachable" would be a strategy — the attacker would only
need to take out the honest pod instead of convincing it.

*In code:* `CONSENSUS/myl-consensus/src/epoch_close.rs`

### Clawback

If an already-credited segment is later refuted, the vTFE credit is
clawed back. That is the safeguard which makes the optimistic delivery
mode defensible in the first place.

*In code:* `epoch_close.rs::apply_clawback`

---

## I. Tokenomics

### MYL

The native coin. Three functions: securing consensus (staking),
compensating miners (minting), paying for inference (burning).

*In code:* `TOKENOMICS/myl-tokenomics/src/lib.rs` —
`UNITS_PER_MYL = 1_000_000`

### Burn-and-mint

The closed cycle:

```
Miner sells MYL on the market ────────────────┐
                                              │
User ──burn MYL──► inference credits          │
                    │                         │
                    ▼                         │
                    Pods perform work         │
                    │                         │
                    ▼                         │
                    confirmed PoI bundles     │
                    │                         │
                    ▼                         │
                    mint MYL ──► miners───────┘
```

The protocol issues MYL **exclusively to miners**; users acquire them on
the market. Minting and burning are protocol operations, acquisition is
not.

### vTFE (verified token-forward equivalents)

The unit in which work is measured — not fiat, not MYL, but **compute**.
That keeps the utility price of inference stable in compute units; the
MYL price mediates between supply and demand.

*In code:* `VTFE_UNITS_PER_TFE = 1_000_000`

> **⚑ Open item:** vTFE must count **layers**, not shards. A shard is a
> packaging unit and varies in size with pod configuration; layers are the
> actual work. As long as counting is by shard, a pod with few large
> shards would be penalised against one with many small ones. Noted in the
> master roadmap.

### Inference credit (IC)

What the user receives for burning MYL. Denominated in vTFE.

*In code:* `SHARED_TYPES/myl-types/src/core_types.rs::InferenceCredit`,
`CONSENSUS/myl-ledger/src/transitions.rs::burn_to_credits`

### Mint function

```
M_e = min( B̄_e · (1 + s), M_max )
```

- `B̄_e` — smoothed burn volume (→ [EMA](#ema-exponential-moving-average))
- `s` — subsidy rate (bootstrap phase > 0, steady state 0)
- `M_max` — emission cap

In equilibrium (`s → 0`), `M_e ≈ B̄_e`: the money supply is
net-neutral to deflationary in the long run, since slashing burns are
added on top.

*In code:* `TOKENOMICS/myl-tokenomics/src/mint.rs::mint_amount`

### EMA (exponential moving average)

A moving average weighting new values more than old ones:
`B̄_e = B̄_{e−1} + α · (B_e − B̄_{e−1})`, with `α = 2/(N+1)` and
N = 30 epochs, i.e. `α = 2/31`.

**Why smoothing:** without an EMA an attacker could burn heavily in a
single epoch, drive up that epoch's minting and pocket most of it. The
smoothing spreads the effect over 30 epochs and makes the attack
unprofitable.

Implemented entirely in integers (numerator/denominator as a fraction).

*In code:* `TOKENOMICS/myl-tokenomics/src/ema.rs`

### Distribution of minting

| Share | Recipient | Basis points |
|---|---|---|
| 78 % | Shard miners (after redundancy normalisation) | 7800 |
| 5 % | Pod coordinators | 500 |
| 10 % | Validators (stake × uptime) | 1000 |
| 4 % | Checker pool (base compensation) | 400 |
| 3 % | Protocol treasury | 300 |

Verified across 10,000 simulated epochs: the sum of shares equals `M_e`
exactly in every epoch (floor rounding per share, the rounding remainder
going to the treasury).

*In code:* `TOKENOMICS/myl-tokenomics/src/distribute.rs`

### Redundancy normalisation

Since every segment is computed by 2 pods, each pod receives **half** the
vTFE credit. Miners are paid for *useful net work*; the redundancy
overhead is priced in, not hidden.

*In code:* `distribute.rs::redundancy_normalized_weight`

### Credit price formula

```
P_{e+1} = P_e · exp( κ · (u_e − u*) )
```

with utilisation `u_e`, utilisation target `u* = 0.8` and damping constant
κ. Under overload the price rises → demand falls, mining becomes more
attractive → capacity grows. Price signals instead of central capacity
management (analogous to EIP-1559).

**The exp function is integer here too** — for the same reason as in the
model: consensus determinism. The support points are **frozen** and shipped
as a generated table; a table computed at runtime could differ between
compiler versions.

*In code:* `TOKENOMICS/myl-tokenomics/src/exp_approx.rs`,
`exp_lut_table.rs`, `utilization.rs`

### Self-dealing

The attack in which a miner buys their own inference to harvest minting.
Unprofitable by construction as long as `M_e ≤ B̄_e`: the attacker burns
more than they get back, because they only receive their *capacity share*
of the minting. During the subsidy phase it is additionally damped by EMA
smoothing and a per-address burn cap.

### Training compensation cap

Training compensation ≤ **70 %** of inference compensation per compute
hour. Without that limit, miners would shift capacity from inference to
training and deprive the network of its only revenue source. Funded from
the treasury and a governance-disableable fee surcharge — **not** from
extra minting, which would nearly double net inflation and dilute all
holders.

*In code:* `TOKENOMICS/myl-tokenomics/src/training.rs` —
`TRAINING_CAP_BPS = 7000`

---

## J. Training

The training path is **designed but not yet implemented**. The TRAINING
roadmap currently has exactly one item: a reference simulation of the
backward pass. The terms appear here because they are derived in the
whitepaper and the design decisions are settled.

### Backward pass and gradient

In training you measure how wrong the output was and compute backwards
through the network how each weight would need to change. That direction
of change is the **gradient**.

**Good news for Myelith:** gradient computation is likewise associative,
so the determinism approach from
[section B](#b-determinism--why-myelith-computes-in-integers) carries over
unchanged.

### The overflow problem

Integer backpropagation hits a limit: the error terms **grow with every
layer traversed backwards** and exceed the 32-bit range after only a few
layers with 8-bit weights. Two methods solve this.

### Block scaling (NITI)

After each layer the error vector is divided by a shared **power-of-two
factor** whose exponent is carried separately. The factor follows from the
magnitude maximum and is therefore order-independent; it is applied as an
arithmetic right shift — exactly the operation θ_v mandates anyway.

### Local loss blocks

The network is divided into segments with **their own loss functions**, so
that gradients never leave the segment. Put the block boundaries on the
shard boundaries and the backward pass across the pipeline disappears
entirely: no additional network traffic, and verification stays local — a
shard pair checks its own gradient.

The price is a worse solution than global backpropagation; how large the
gap is for language models is open.

### Data provenance

The hardest question in training is not whether the computation was
correct but **whether the data was legitimate**. A miner feeding in
poisoned text computes bit-identically correctly and still produces a
skewed model — the bit comparison does not help here.

Myelith checks not the **content** but the **origin**: the protocol
maintains a list of canonical corpora, each with a Merkle root anchored in
consensus. A training segment references no raw data but a **Merkle
proof**: "this passage is at position p in the corpus with root R." No
valid proof can be produced for a position that does not exist.

**Selection remains an attack surface.** Someone who cannot forge data can
still select it. Data assignment therefore also runs **by VRF** — which
pod works on which corpus sections follows from the epoch seed. This
requirement is constitutive, not optional.

### Robust aggregation (median)

The gradients of many pods must be merged into one update. The mean is
unsuitable: a single extreme contribution shifts it arbitrarily. Myelith
aggregates by **median**, whose breakdown point is 50 % and therefore
coincides with the Byzantine bound already assumed. Trimmed means already
fail at an attacker share of one third.

The median needs only comparisons — so it stays deterministic and
checkable within the verification model.

### Function-preserving expansion (Net2Net, bert2BERT)

Methods that **enlarge a model without changing its function**: neurons
are split, new layers initialised as the identity. Immediately after
expansion the larger model behaves identically to its predecessor.

Two consequences for Myelith: a growth step is **activatable without
quality risk** (the improvement only comes from subsequent training), and
the expansion is a **deterministic transformation** — hence bit-identically
verifiable like any other computation. θ_v+1 follows reproducibly from
θ_v and the growth operator.

**Structural coupling:** depth growth adds layers, i.e. additional shards
in the pipeline. More miners → more shards → more layers. Network and
model growth are architecturally linked, and the collusion bound β^{2k}
improves as k rises.

**Timescale, soberly:** a network of 500 miners cannot grow. With 5,000
miners a step takes about nine months, with 50,000 about one month.
Growth is a rare event on a yearly scale.

### What remains open

Three points the whitepaper names rather than glosses over: funding
produces perverse incentives in every variant; the combination of integer
training and model growth is **unproven** (each is proven separately, the
combination is not, and the evidence for integer training comes from
vision models); and behaviour under open network conditions is unknown.

*In the whitepaper:* chap. 7, appendix B.6

---

## K. Agent layer

### Session contract

An agent able to trigger transactions turns a computation error into
financial damage. The protocol's answer is not to exclude the case but to
**bound its effect**. Every agent session runs under four enforced limits:

1. **Total budget** in credits and, where applicable, MYL
2. **Per-transaction limit**, independent of remaining budget
3. **Recipient whitelist**
4. **Time window**, after which the session expires

**What matters is where these parameters live: in the contract, not in the
model's context.** They are neither readable nor modifiable by the agent;
enforcement happens at transaction execution, by consensus.

### Deterministic vs. external tools

If an agent calls a web search, the two redundant pods receive different
answers — the bit comparison fails without any error having occurred.

The solution: tool results are **taken out of the computation and turned
into input**. A gateway fetches the result once, timestamps and signs it,
and hands it to both pods as identical text.

- **Deterministic tools** (own ledger, computations, anchored corpora) —
  fully verified like any computation.
- **External tools** (web search, market data) — the answer is
  **attested but not verified**. The protocol testifies *that* a given
  gateway received this answer at a given time, not that it is true.

What is verified is the *processing* of the answer, not its correctness.

### Prompt injection and the dual-LLM pattern

When an agent processes foreign content, that content may contain
instructions posing as user requests. The problem is known and
**unsolved**; filter-based approaches are considered unreliable, because
the checking mechanism is exposed to the same attack surface as the model.

Myelith follows architectural separation (dual-LLM, CaMeL): the planning
part sees no foreign content, the processing part cannot call tools, and
retrieved data does not influence control flow. This is reinforced by the
fact that permissions live in the session contract anyway — **beyond the
model's reach**.

Injected text can deceive the agent but can neither raise its budget nor
add a recipient. The problem thereby shifts from security to output
quality. That is the strongest available statement; complete defence is
explicitly not claimed.

### Step chaining

An agent works iteratively. Each step is its own segment and references
the output commitment of its predecessor. This forms a chain with the same
structure as the → [computation trace](#computation-trace) within a
segment, one level up. What is checkable is therefore also that no steps
were omitted, inserted or reordered.

*In the whitepaper:* chap. 8

---

## L. How this project works

This section is aimed mainly at coding agents. It explains not *what* is
being built but *how* — and why the rules are what they are.

> **A note on the references in this section.** The binding version of
> these rules lives in `AGENTS.md` and
> `README/Intern/State-of-the-Project.md` (section 8). Both are
> **working-internal and not part of the publication** — if you only have
> the public repository in front of you, you will not find them. This
> section is therefore written to be complete without them.

### Open-source threat model

**The attacker knows the code.** Myelith is open source; there is no
security through obscurity. Every hardening must hold even when the
attacker fully understands the protocol.

Practical consequences visible in the code:

- **Timing arguments are deadlines, not defences.** "The attacker won't
  manage it in time" is not a security argument but a bet on their
  hardware.
- **Constant-time comparisons** for hashes, so that comparison duration
  carries no information.
- **Check orderings**: a polka certificate's voter list must be strictly
  ascending, otherwise the same validator counts more than once.
- **Authoritative sources rather than supplied claims**: pod membership
  comes from the scheduler, not from the bundle.
- **Indistinguishable failure cases**: `DaStore::fetch` checks the dispute
  window before the lookup, so "expired" and "withheld" look the same.

### Golden vectors

Frozen reference results: input → expected output, bit for bit. They are
the **normative truth** for every backend. A new backend (SIMD, CUDA,
ROCm) is only valid once it reproduces every golden vector exactly.

*In code:* `INTEGER_LLM/tests/golden/`,
`kernels/src/bin/golden_runner.rs`, `golden_generate`

> **A trap that has sprung twice (finding 30):** `golden_generate` itself
> appends `vectors` to the output path given to it. But `generate.py` was
> already passing `tests/golden/vectors` — so the new vectors landed in
> `tests/golden/vectors/vectors/` while validation kept reading from
> `vectors/layer`, i.e. from **stale** files. The first time it was noted
> as a usage error; it was a generator bug and therefore recurred
> immediately. Fixed, and the orphaned duplicate removed from version
> control.
>
> **Lesson:** if the same trap springs twice, it lives in the code, not in
> the operator. A warning in the documentation is no substitute for an
> assertion in the program.

**Mind the second copy:** `INTEGER_LLM/conformance/vectors/` is a manual
copy of `tests/golden/vectors/`. After every θ_v bump both must be
reconciled, or the conformance run checks against stale vectors:

```bash
rsync -a --delete INTEGER_LLM/tests/golden/vectors/ INTEGER_LLM/conformance/vectors/
```

### Conformance run

The proof that two backends produce bit-identical results. Current status:
**30/30** for the reference and SIMD backends.

*In code:* `INTEGER_LLM/conformance/`

### Backend trait

The abstraction over heterogeneous hardware. Every implementation must
satisfy the numerics contract from θ_v and be validated against the golden
vectors. Freedom exists in parallelisation and kernel structure, not in
the result.

*In code:* `INTEGER_LLM/kernels/src/backend.rs`, implementations in
`kernels/src/backends/`

### SIMD

*Single instruction, multiple data* — one CPU instruction processes
several values at once. On ARM that is NEON, on x86 AVX2.

**The most important pitfall**, because it actually occurred here: the
first NEON attempt was **slower** than the scalar code (12.43 vs 18.89
tok/s). The cause was a **single accumulator** — every multiply-accumulate
had to wait for the previous result, a serial dependency chain. With
**four independent accumulators** merged only at the end of a block, the
CPU can overlap the instructions: +31 % / +50 %, bit-identical.

The lesson: with SIMD it is the **latency of the dependency chain** that
decides, not the throughput of the individual instruction.

*In code:* `INTEGER_LLM/kernels/src/dot.rs`

### Finding

A documented result — an error, a false assumption, or an insight that
changes the design. Findings are **numbered and documented, not silently
fixed**.

**Why this is a hard rule:** a silently fixed error comes back as soon as
someone refactors the code without knowing the reason. A documented
finding explains *why* the code looks the way it does.

Examples: finding 15 (wrong RoPE scheme — the dominant error source),
finding 19 (1/√head_dim as a shift was only correct for even powers of
two), finding 22 (KV cache round trip), finding 27 (rogue key without
PoP), finding 31 (double clamping destroys the massive-activation
channel).

### Instrument errors

This project's own class of error, and the most instructive:
**the code was not wrong — the measuring tool was.**

Ten of them occurred during the investigation of roadmap item 12.77. A
selection:

1. A softmax patch never fired (SDPA fuses the softmax) — the reading
   "+0.00 %" was a **null measurement**, not a result.
2. An ablation script had the comparison figure **hard-coded** and reported
   "98 % of the error in the LM head" where the truth was 0 %.
3. A "71 % jump at layer 23" compared the layer output against the
   **post-final-norm** state — the last entry of HF's `hidden_states` is
   not the last layer.
4. An MLP probe fed `rmsnorm(embedding)` instead of
   `rmsnorm(embedding + attn_out)` — a 72.3 % wrong input.
5. A q/k/v comparison omitted the **biases** → 1347 % / 8613 %.
6. The attention probe omitted **RoPE** → 47–151 %.
7. The same probe passed **`lut_shift = 0`** instead of
   `score_frac_bits − exp_input_frac`, computing `exp(−d/256)` instead of
   `exp(−d/16)`.
8. The reference model ran in **bfloat16**. At a value of 1704 the bf16
   ULP is **8** — the reference cannot represent changes below that, and
   at a cancellation `1696 − 1648 = 48` it carries ±8 of error on a result
   of 48. **Where cancellation occurs, bf16 is unusable as a reference.**

**What all of them share:** not one was found by reading code. They were
found because a result was **physically impossible** — an attention error
of 151 % is irreconcilable with a perplexity of +7.5 %.

**The rules that follow:**

- **Every probe needs a self-check** — a case with a known result.
- **But the self-check must exercise the component in question.** The
  attention probe passed its check at n=1 with 0.00 % — precisely the case
  in which the missing RoPE does nothing.
- **A patch that does not fire must be noticed.** Instrumentation counts
  its invocations and aborts at zero.
- **Implausible numbers are a finding**, not an intermediate result.

### The 1 % rule

**Perplexity differences below roughly 1 % carry no information.** They
are not even monotone across different sequence sets.

This rule arose from a withdrawn result: a supposedly better rsqrt
resolution turned out to be (a) not implementable and (b), in its
implementable form, worse than the status quo — +2.16 % against +1.90 %,
with the sign flipping depending on whether 4 or 16 sequences were used.

**Every durable finding in this project either had a large margin or was a
tensor comparison** (→ [relative L2](#relative-l2)) rather than a
perplexity comparison. Tensor comparisons against floating point with
*identical* weights and *identical* input isolate a single operation —
they are the sharper tool.

### Seven-step documentation chain

After every completed patch, in this order:

1. Cargo versions
2. Component roadmap
3. Component README
4. `README/Intern/Fahrplan-Master.md`
5. `README/Intern/README.md`
6. `README/Intern/State-of-the-Project.md`
7. Root `README.md` (component table)

If the patch touches protocol terminology, **this file** and its German
counterpart are added.

### Integer purity check

After every change to `model.rs`, `loader.rs` or `calibrate/`:

```bash
grep -n "f32\|f64" <changed files>
```

Hits are acceptable **only** in calibration metadata, **never** in the
compute path. A single overlooked `f32` in a kernel breaks determinism and
with it the entire verification model — and it would only surface when two
pods on different hardware diverge.

### Runtime estimates and progress bars

Before any run lasting more than about a minute, state an estimate —
**computed, not guessed**: work units × measured time per unit. The rates
live in `INTEGER_LLM/bench/README.md` (0.5B ~24 tok/s, 7B ~2 tok/s with
`cpu-simd`).

Scripts with several work units print a progress bar
(`INTEGER_LLM/tests/diag/fortschritt.py`), with Python output unbuffered
(`python -u`).

**Why:** without an estimate you cannot decide whether waiting is worth
it; without a progress display a hung run is indistinguishable from a slow
one.

### CI environment constraints

GitHub CI has **no model weights** (gitignored) and **no hardware
backends**. Tests must therefore skip cleanly (exit 0 with a SKIP message)
when artifacts or backends are missing. Only unit tests
(`cargo test --lib`) run in CI.

**The real quality assurance is the local runs** with real artifacts and
real hardware.

### Commit rules

- **Do not commit or push on your own.** The repository authors do that
  after review. When something is ready: a short note plus a title
  suggestion.
- **Commit titles list only the changed areas/points as bullet points**,
  without deeper description — that lives in the changelog.

### Shared build directory

All crates write to `target-shared/` in the repository root
(`.cargo/config.toml`). Each crate remains a standalone Cargo project with
no shared workspace `Cargo.toml`; only the output location is shared.

**Reason:** with twelve separate `target/` directories, every shared
dependency sat on disk multiple times (`myl_types` 66 times, `sha2` 34
times) — 23.8 GB instead of 2.1 GB for an identical result.

Anyone invoking a binary from a script must **not** hard-wire the path but
use `INTEGER_LLM/tests/cargo_paths.py`.

---

## M. Abbreviations at a glance

| Abbrev. | Meaning | Section |
|---|---|---|
| **A16** | Activations in 16 bit | [C](#w8a16) |
| **AS** | Autonomous system (network operator unit) | [G](#zone-diversity) |
| **BFT** | Byzantine fault tolerance | [F](#bft-byzantine-fault-tolerance) |
| **BLS** | Boneh–Lynn–Shacham (signature scheme) | [E](#signature-and-bls12-381) |
| **BPS** | Basis points (1 BPS = 0.01 %) | [I](#distribution-of-minting) |
| **DA** | Data availability | [H](#da-data-availability) |
| **DST** | Domain-separation tag | [E](#domain-separation-tag-dst) |
| **EMA** | Exponential moving average | [I](#ema-exponential-moving-average) |
| **GF(2⁸)** | Galois field with 256 elements | [E](#gf28-galois-field) |
| **GQA** | Grouped query attention | [D](#head-and-gqa) |
| **GST** | Global stabilization time | [F](#gst-global-stabilization-time) |
| **IC** | Inference credit | [I](#inference-credit-ic) |
| **KV** | Key/value (attention) | [D](#kv-cache) |
| **L0–L3** | Networking / consensus / compute / agent layer | [A](#layer-model) |
| **LUT** | Lookup table | [C](#lut-lookup-table) |
| **MLP** | Multi-layer perceptron (feed-forward block) | [D](#mlp--feed-forward-and-silu) |
| **MYL** | The native coin | [I](#myl) |
| **PoI** | Proof of inference | [—](#poi-proof-of-inference) |
| **PoP** | Proof of possession | [E](#rogue-key-attack-and-proof-of-possession) |
| **PRNG** | Pseudo-random number generator | [D](#sampling) |
| **RMSNorm** | Root mean square normalization | [D](#rmsnorm) |
| **RoPE** | Rotary position embedding | [D](#rope-rotary-position-embedding) |
| **RTT** | Round-trip time | [G](#latencygraph-and-latency-attestations) |
| **SIMD** | Single instruction, multiple data | [L](#simd) |
| **SiLU** | Sigmoid linear unit (swish) | [D](#mlp--feed-forward-and-silu) |
| **VRF** | Verifiable random function | [E](#vrf-verifiable-random-function) |
| **vTFE** | Verified token-forward equivalents | [I](#vtfe-verified-token-forward-equivalents) |
| **W8** | Weights in 8 bit | [C](#w8a16) |
| **zkML** | Zero-knowledge machine learning | [H](#the-three-levels) |
| **θ_v** | Model version / execution specification | [C](#θ_v-theta-v-model-version) |

---

## Further reading

**Public:**

| What | Where |
|---|---|
| Derivation and rationale | [`README/Whitepaper/myelith-whitepaper-v0.3-en.md`](Whitepaper/myelith-whitepaper-v0.3-en.md) |
| Integer inference in detail | [`INTEGER_LLM/README/README.md`](../INTEGER_LLM/README/README.md) |
| The numerics contract | [`INTEGER_LLM/theta_v/spec.json`](../INTEGER_LLM/theta_v/spec.json) |
| German edition of this glossary | [`README/Glossar.md`](Glossar.md) |

**Working-internal** (not part of the publication — present only in the
working directory):

| What | Where |
|---|---|
| Architecture, history, open findings | `README/Intern/State-of-the-Project.md` |
| What gets built next | `README/Intern/Fahrplan-Master.md` |
| Entry point for coding agents | `AGENTS.md` |
