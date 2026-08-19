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

**Results.** Executed entirely in integer arithmetic, measured against
the floating-point reference of the same model:

| Model | Integer | BF16 reference | Gap |
|---|---|---|---|
| Qwen2.5-0.5B | 15.29 | 14.95 | **+2.3 %** (criterion ≤5 % met) |
| Qwen2.5-7B | 9.40 | 8.68 | +8.3 % |

**The bit-identity that matters is that of the integer path with
itself** — across independent runs, across nodes, across hardware. That
is the consensus requirement (Whitepaper Chap. 6.2), and it is proven:
across independent runs, across a genuine multi-node pipeline, and under
artificially injected network stress (latency, packet loss, node
restarts).

*Closeness to the floating-point reference* is a separate matter. The
integer path is a quantisation and deviates by construction — that is
precisely why it carries a perplexity gap at all. In a
[qualitative benchmark](INTEGER_LLM/README/README.md#qualitativer-benchmark)
over eight real prompts it nevertheless produces the same text as BF16 in
five of eight cases on 7B (73.8 % matching tokens). That is a quality
indicator for the quantisation, not a target: 8/8 would not be a success
but an indication that the quantisation has no effect. Details in the
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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference (Rust + Python) | core thesis empirically confirmed on 0.5B (+2.3 % against the floating-point baseline), multi-node pipeline running, backends AVX2+NEON. **On 7B +8.3 % after fixing two implementation defects (previously +377 %); criterion not yet met** |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle) | Phases 1 + 2 complete (golden vectors, fuzz harness, conformance package) |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery, latency topology | Phases 1 + 2 complete (pairwise latency measurement, LatencyGraph, geo/AS diversity) |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | Phases 1 + 2 complete; Phase 3 with a caveat — signed, stake-and-work-weighted BFT with VRF-rotating committee election, but no round change / timeouts yet |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phases 1 + 2 complete (credit pricing with a frozen exp() LUT) |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phases 1 + 2.1 complete (micro-batching, pipelining). **Pipeline determinism holds** (two independent runs bit-identical) and the shard layout is pinned via `pipeline_hash` since finding 25. **Open:** boundary rescaling is scalar while the residual stream has been per-channel since finding 20 — the Phase 1 guarantee "bit-identical **with the single node**" therefore no longer holds |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | Phases 1 + 2 complete (arbitration round, slashing via the ledger) |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | planning phase |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | planning phase |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | planning phase |
| [ETHICS](ETHICS/README/README.md) | ethical and legal standards, manifesto | manifesto v1.0.0 in place, roadmap in place, design decisions open |
| [TESTCLIENT](TESTCLIENT/README/README.md) | terminal test client: hardware tests, sharded inference | Phase 1 complete (hardware fingerprint, determinism, shard run with run logs) |
| [CLIENT](CLIENT/README/README.md) | user client incl. wallet | concept phase |

## License

[PolyForm Shield License 1.0.0](LICENSE.md) — using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
