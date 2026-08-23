![Myelith: A decentralized network in which consensus work powers an agentic language model](README/Grafiken/myelith-banner-en.png)

This README is also available in [German](README.md).

Myelith is a decentralized network in which the same computation that
secures consensus simultaneously runs a large agentic language model
("Proof of Inference"). Unlike classical proof-of-work, no discarded
computation is burned; instead, useful inference is performed,
and it is verifiable because it runs entirely in integer arithmetic and is
therefore bit-identical across independent nodes, rather than relying on
trust in any single operator. The native coin MYL (not yet in
circulation) closes the loop: users burn MYL for inference credits, and
miners receive newly minted MYL proportional to verified work.

The complete architecture, tokenomics, and verification model are set
out in **Whitepaper v0.3**:
[German (MD)](README/Whitepaper/myelith-whitepaper-v0.3.md) ·
[German (PDF)](README/Whitepaper/myelith-whitepaper-v0.3.pdf) ·
[English (MD)](README/Whitepaper/myelith-whitepaper-v0.3-en.md) ·
[English (PDF)](README/Whitepaper/myelith-whitepaper-v0.3-en.pdf).

Every technical term, from the bisection game to fixed-point arithmetic,
is explained in the **[glossary](README/Glossary.en.md)**, with worked
examples and pointers to the corresponding implementation.

## Core thesis

Integer addition is associative. If inference is executed entirely in
integer arithmetic, bit-identity arises between independent nodes, the
foundation of the entire verification architecture (Whitepaper Chap. 6).
Whether that holds up qualitatively at realistic model scale is an open
empirical question; the project answers it first on a small model,
before infrastructure is scaled.

**Results,** executed entirely in integer arithmetic and measured against
the floating-point reference of the same model:

| Model | Integer perplexity | BF16 reference | Gap |
|---|---|---|---|
| Qwen2.5-0.5B | 15.27 | 14.95 | **+2.1 %**, criterion ≤5 % met |
| Qwen2.5-7B | **8.78** | 8.68 | **+1.1 %**, criterion ≤5 % met |

*The metric is perplexity on WikiText-2 under teacher forcing, on identical
sequences for both paths; lower is better. "Gap" is the relative premium the
integer path pays over its own BF16 reference. On 7B that figure stood at
**+377 %** before the bug hunts (perplexity 41.42); today it is **+1.1 %**,
which puts it **0.3 percentage points above the theoretical floor of the
quantisation scheme itself** (+0.84 %, measured independently).

**Bit-identity here is not a side effect, it is the product.** What matters
is the agreement of the integer path with itself: across independent runs,
across nodes, across hardware. That is the consensus requirement (Whitepaper
Chap. 6.2), and it holds: proven across independent runs, across a genuine
multi-node pipeline, and under artificially injected network stress from latency,
packet loss, node restarts. No tolerance windows, no "reproducible within
measurement error", no trust in individual operators. Bit for bit or not at
all.

*Closeness to the floating-point reference* is a different question, and it
comes out better than the percentage suggests. The integer path is a
quantisation and deviates by construction; that is precisely why it carries a
perplexity gap at all. In the
[qualitative benchmark](INTEGER_LLM/README/README.md#qualitativer-benchmark)
over eight real prompts, 7B nevertheless produces word-for-word the same text
as BF16 in five of eight cases, at 73.8 % matching tokens. That is a quality
indicator, not a target: 8/8 would not be a success but an indication that the
quantisation has no effect. Details in the
[whitepaper (Ch. 6.9)](README/Whitepaper/myelith-whitepaper-v0.3-en.md)
and in [INTEGER_LLM](INTEGER_LLM/README/README.md).

## Architecture

Four layers (Whitepaper Chap. 3.2), with tokenomics, training, and
governance cutting across them:

| Layer | Task |
|---|---|
| **L3 Agent Layer** | Agentic workflows, tool use, sessions, session contracts |
| **L2 Compute Layer** | Model shards, pods, pipeline routing, redundant computation |
| **L1 Consensus Layer** | BFT consensus, Proof-of-Inference aggregation, staking, slashing |
| **L0 Networking Layer** | P2P gossip, latency topology, encrypted activation streams |

## Components

Each component has its own folder with a roadmap, design decisions, and
tests:

| Component | Task | Status |
|---|---|---|
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference and, new, the integer backward pass (Rust + Python) | **Acceptance criterion ≤ 5 % met on both models:** 0.5B +2.11 %, 7B **+1.14 %** (previously +377 %). That is 0.30 points above the floor of the quantisation scheme itself (+0.84 %). Throughput most recently **+52 % / +40 %** (0.5B / 7B) from dropping a per-token weight copy; before that **+27 % / +43 %** from NEON. Both at bit-identical output, 30/30 conformance vectors on both backends. The [scale pack](INTEGER_LLM/scale_packs/README.md) makes the artifact build bit-identical across platforms: 1.8 MB instead of 8.8 GB, and 20 minutes become 40 seconds |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle, erasure) | Phase 2 complete. BLS with proof of possession against rogue-key attacks, with an executable regression; erasure coding over GF(2⁸) in Cauchy form, verified across **all 495** subsets of 8 from 12 |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery, latency topology | Phase 2 complete: pairwise latency measurement, LatencyGraph, geographic and AS diversity |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | **All four phases complete.** Signed, weight-based BFT with VRF-rotated committee selection, double-signing proofs, and round changes with locking, giving safety **and** liveness, verified through an acceptance test matrix over 21 simulated validators. Plus PoI bundle submission, epoch close, and data availability (Reed-Solomon k=8/m=4 across the dispute window). Since v0.10.0 the **work share of the voting weight is calibrated and capped**: its previous reference value equalled the forward pass of a single token, so one hour of work would have raised the stake a thousandfold |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phase 2 complete: mint function, distribution key, and credit pricing with a frozen exp() lookup table, entirely in integers. Since v0.3.0 it is also settled **how a shard earns its credit**: by its share of the weight arithmetic of one forward pass rather than by layer count, because at 0.5B the LM head weighs more than nine layers. Splits from 1 to 28 shards therefore distribute the same total |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phase 2.1 complete: micro-batching and pipelining. **Pipeline determinism is bit-identical with the single node again,** now that the lossy boundary step between stages is gone (findings 20/26). The trace therefore binds the transmitted activation once more. Since v0.3.0 "bit-identical" here is evidenced over the **computed numbers** rather than the emitted tokens: the pod exposes a digest over logits and tokens, and it matches the single node across 1 to 24 shards (finding 36) |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | Phase 2 complete: bisection in O(log L), on-chain adjudication behind the `ShardExecutor` trait, slash decision kept separate from the amounts. Level 3 (zkML anchor) is an upgrade path and has not been started |
| [TESTCLIENT](TESTCLIENT/README/README.md) | terminal test client: hardware tests, sharded inference, evaluation | Phase 1 and **phase 3** complete, plus items 2.1 and 2.4. `vergleich` compares the logs of several machines and issues the verdict, and **refuses** one when every log comes from the same machine, when a run was aborted, or when two runs measured different things. Since v0.8.0 the comparison value covers the **computed numbers**, not just the emitted tokens (finding 36). Test plans are no longer tied to a model; a curated model catalogue records provenance, revision and licence. Since v0.11.0 the client also accompanies model changes: `--erwarte` fails a run that produces a different comparison value, and `modellstaende` answers in one call which values a θ_v change moved and which it did not. The proof itself still awaits heterogeneous hardware |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | **The one measurement is done (2026-08-22): it holds**, given stochastic rounding of the weights. Full integer scheme against floating point on held-out text: **+0.67 %** (criterion ≤ 10 %); with round-to-nearest, +29.9 %, because one SGD step is a median 6.4e-6 of a grid step. The randomness costs no determinism: the dice is a function of (layer, step, index), not a state. And the training step needs **no floating-point state at all**: integer master, exact integer addition, +0.75 %. Growth is **exactly function-preserving** (deviation 0.00e+00, via an integer split rather than a halving), and the copies' symmetry breaks without artificial noise. Concept and roadmap are in place. **Since v0.1.0 the component also has code:** the data provenance of chapter 7.3, meaning corpora anchored by a Merkle root, segments referenced by proof rather than by raw data, and an assignment that follows from the epoch seed rather than from the miner's choice |
| [ETHICS](ETHICS/README/README.md) | ethical and legal standards, manifesto | Manifesto v1.0.0 in place, roadmap in place, design decisions open. **Principle G7 (the base model must be freely reusable) has been checked across all seven Qwen2.5 sizes:** five are Apache 2.0, while 3B and 72B fail, and the 72B clause triggering at 100 million monthly active users is structurally unmeetable for an open protocol |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | Planning phase. Crypto agility for the post-quantum migration is anchored; the remaining design decisions are open |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | Planning phase, blocked by the layers below |
| [CLIENT](CLIENT/README/README.md) | user client including wallet | Concept phase |

## License

[PolyForm Shield License 1.0.0](LICENSE.md). Using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
