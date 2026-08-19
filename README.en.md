![Myelith — A decentralized network in which consensus work powers an agentic language model](README/Grafiken/myelith-banner-en.png)

This README is also available in [German](README.md).

Myelith is a decentralized network in which the same computation that
secures consensus simultaneously runs a large agentic language model
("Proof of Inference"). Unlike classical proof-of-work, no discarded
computation is burned; instead, useful inference is performed —
verifiable because it runs entirely in integer arithmetic and is
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

## Core thesis

Integer addition is associative. If inference is executed entirely in
integer arithmetic, bit-identity arises between independent nodes — the
foundation of the entire verification architecture (Whitepaper Chap. 6).
Whether that holds up qualitatively at realistic model scale is an open
empirical question; the project answers it first on a small model,
before infrastructure is scaled.

**Results,** executed entirely in integer arithmetic and measured against
the floating-point reference of the same model:

| Model | Integer perplexity | BF16 reference | Gap |
|---|---|---|---|
| Qwen2.5-0.5B | 15.29 | 14.95 | **+2.3 %** — criterion ≤5 % met |
| Qwen2.5-7B | **9.40** | 8.68 | +8.3 % — target ≤5 %, remaining gap documented |

*The metric is perplexity on WikiText-2 under teacher forcing, on identical
sequences for both paths; lower is better. "Gap" is the relative premium the
integer path pays over its own BF16 reference. On 7B that figure stood at
41.42 before the last bug hunt — two implementation errors, a factor of 45.*

**Bit-identity here is not a side effect, it is the product.** What matters
is the agreement of the integer path with itself — across independent runs,
across nodes, across hardware. That is the consensus requirement (Whitepaper
Chap. 6.2), and it holds: proven across independent runs, across a genuine
multi-node pipeline, and under artificially injected network stress — latency,
packet loss, node restarts. No tolerance windows, no "reproducible within
measurement error", no trust in individual operators. Bit for bit or not at
all.

*Closeness to the floating-point reference* is a different question — and it
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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference (Rust + Python) | core thesis empirically confirmed on 0.5B (+2.3 % against the floating-point baseline), multi-node pipeline running. NEON backend **+31 % (0.5B) / +50 % (7B)** at bit-identical output. **On 7B +8.3 % after fixing two implementation defects (previously +377 %); criterion not yet met** — the component's only remaining item. Throughput figures, a [model card](INTEGER_LLM/artifacts/MODEL_CARD.md), and a walked-through first-inference guide are in place |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle, erasure coding) | Phases 1 + 2 complete (golden vectors, fuzz harness, conformance package). BLS with proof-of-possession against rogue-key attacks, with an executable regression test; erasure coding over GF(2⁸) in Cauchy form |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery, latency topology | Phases 1 + 2 complete (pairwise latency measurement, LatencyGraph, geo/AS diversity) |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | Phases 1–3 complete — signed, stake-and-work-weighted BFT with VRF-rotating committee election, double-signing proof, and round change with locking. **Safety and liveness**, acceptance test matrix run over 21 simulated validators. **all four phases complete** — including PoI bundle submission, epoch closing, and data availability (Reed-Solomon k=8/m=4, retention across the dispute window) |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phases 1 + 2 complete (credit pricing with a frozen exp() LUT) |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phases 1 + 2.1 complete (micro-batching, pipelining). **Pipeline determinism holds and is bit-identical with the single node again** — the lossy boundary rescaling between stages has been removed (findings 20/26), so the trace once more commits the activation that is actually transmitted |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | Phases 1 + 2 complete (arbitration round, slashing via the ledger) |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | planning phase |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | a single roadmap item: measuring whether the quantisation scheme carries through the backward pass. The roadmap itself follows only once that result is in — the previous 22 items all rested on an untested assumption |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | planning phase |
| [ETHICS](ETHICS/README/README.md) | ethical and legal standards, manifesto | manifesto v1.0.0 in place, roadmap in place, design decisions open |
| [TESTCLIENT](TESTCLIENT/README/README.md) | terminal test client: hardware tests, sharded inference | Phase 1 complete (hardware fingerprint, determinism, shard run with run logs) |
| [CLIENT](CLIENT/README/README.md) | user client incl. wallet | concept phase |

## License

[PolyForm Shield License 1.0.0](LICENSE.md) — using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
