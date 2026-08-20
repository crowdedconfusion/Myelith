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
examples and pointers to the corresponding implementation
([German edition](README/Glossar.md)).

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
41.42 before the bug hunts; today it is 1.1, which puts it **0.3 points
above the theoretical floor of the quantisation scheme itself** (+0.84 %,
measured independently). Getting there took four implementation errors and
ten instrument errors, all of them documented: the last one clamped both
summands of the residual addition individually onto the target scale and
thereby destroyed every cancellation. At one point it produced −0.002
where 61.6 was correct.*

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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference (Rust + Python) | **Acceptance criterion ≤ 5 % met on both models:** 0.5B +2.11 %, 7B **+1.14 %** (previously +377 %). That is 0.30 points above the floor of the quantisation scheme itself (+0.84 %). NEON backend **+27 % / +43 %** at bit-identical output, 30/30 conformance vectors on both backends. The [scale pack](INTEGER_LLM/scale_packs/README.md) makes the artifact build bit-identical across platforms: 1.8 MB instead of 8.8 GB, and 20 minutes become 40 seconds |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle, erasure) | Phase 2 complete. BLS with proof of possession against rogue-key attacks, with an executable regression; erasure coding over GF(2⁸) in Cauchy form, verified across **all 495** subsets of 8 from 12 |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery, latency topology | Phase 2 complete: pairwise latency measurement, LatencyGraph, geographic and AS diversity |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | **All four phases complete.** Signed, weight-based BFT with VRF-rotated committee selection, double-signing proofs, and round changes with locking, giving safety **and** liveness, verified through an acceptance test matrix over 21 simulated validators. Plus PoI bundle submission, epoch close, and data availability (Reed-Solomon k=8/m=4 across the dispute window) |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phase 2 complete: mint function, distribution key, and credit pricing with a frozen exp() lookup table, entirely in integers |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phase 2.1 complete: micro-batching and pipelining. **Pipeline determinism is bit-identical with the single node again,** now that the lossy boundary step between stages is gone (findings 20/26). The trace therefore binds the transmitted activation once more |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | Phase 2 complete: bisection in O(log L), on-chain adjudication behind the `ShardExecutor` trait, slash decision kept separate from the amounts. Level 3 (zkML anchor) is an upgrade path and has not been started |
| [TESTCLIENT](TESTCLIENT/README/README.md) | terminal test client: hardware tests, sharded inference | Phase 1 complete. It finds artifacts on its own, offers a choice when several exist, and otherwise builds them from HF weights plus the scale pack. It verifies the digest and states explicitly that a mismatch is **not** a hardware finding. Phase 2 awaits heterogeneous hardware |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | A single roadmap item: measuring whether the quantisation scheme holds in the backward pass. The roadmap follows from that result, because the previous 22 items rested on an unverified assumption |
| [ETHICS](ETHICS/README/README.md) | ethical and legal standards, manifesto | Manifesto v1.0.0 in place, roadmap in place, design decisions open |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | Planning phase. Crypto agility for the post-quantum migration is anchored; the remaining design decisions are open |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | Planning phase, blocked by the layers below |
| [CLIENT](CLIENT/README/README.md) | user client including wallet | Concept phase |

## License

[PolyForm Shield License 1.0.0](LICENSE.md). Using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
