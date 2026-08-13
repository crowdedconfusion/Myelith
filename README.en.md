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

**First result:** A 0.5-billion-parameter model, executed entirely in
integer arithmetic, reaches a perplexity of 15.59 against 14.95 for the
floating-point reference (+4.3 %, accepted at ≤5 %) — with proven
bit-identity across independent runs and even across a genuine
multi-node pipeline under artificially injected network stress (latency,
packet loss, node restarts). Details in the
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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference (Rust + Python) | core thesis empirically confirmed, multi-node pipeline running |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle) | Phase 1 complete |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery | Phase 1 complete |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | ledger (Phase 1) complete |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phase 1 complete |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phase 1 complete |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | planning phase |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | planning phase |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | planning phase |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | planning phase |
| [CLIENT](CLIENT/README/README.md) | user client incl. wallet | concept phase |

## License

[PolyForm Shield License 1.0.0](LICENSE.md) — using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
