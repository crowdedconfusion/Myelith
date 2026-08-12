![Myelith — A decentralized network in which consensus work powers an agentic language model](README/Grafiken/myelith-banner-en.png)

This README is also available in [German](README.md).

Myelith is a decentralized network in which the same computation that
secures consensus simultaneously runs a large agentic language model
("Proof of Inference"). Unlike classical proof-of-work, no discarded
computation is burned; instead, useful inference is performed whose
correctness can be verified through fully integer, bit-identical
execution. The native coin MYL closes the loop: users burn MYL for
inference credits, and miners receive newly minted MYL proportional to
verified work.

The complete architecture, tokenomics, verification model, and open
research questions are set out in Whitepaper v0.3:
[German (MD)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3.md) /
[German (PDF)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3.pdf) /
[English (MD)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3-en.md) /
[English (PDF)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3-en.pdf).
The simulation programs for Appendix B live under
[`README/Whitepaper/myelith-v0.3/simulations/`](README/Whitepaper/myelith-v0.3/simulations/).

## Core thesis

Integer addition is associative. If inference is executed entirely in
integer arithmetic (no floating point in the compute path, division
exclusively as arithmetic right shift), bit-identity arises between
independent nodes — the foundation of the entire verification
architecture (redundancy comparison, bisection game, control segments,
Whitepaper Chap. 6). Whether integer-quantized inference holds up
qualitatively at the target scale is an open empirical question, which
the project answers on a small model before infrastructure is scaled.

## Architecture

Four layers (Whitepaper Chap. 3.2), with tokenomics, training, and
governance cutting across them:

| Layer | Task |
|---|---|
| **L3 Agent Layer** | Agentic workflows, tool use, sessions, session contracts |
| **L2 Compute Layer** | Model shards, pods, pipeline routing, redundant computation |
| **L1 Consensus Layer** | BFT consensus, Proof-of-Inference aggregation, staking, slashing |
| **L0 Networking Layer** | P2P gossip, latency topology, encrypted activation streams |

## Repository structure

```
├── LICENSE.md                 PolyForm Shield License 1.0.0
├── README.md                  German version of this file
├── README.en.md               this file
├── README/Whitepaper/         Whitepaper v0.3 (DE/EN, MD+PDF) + simulations
├── README/Grafiken/           title banners and figures (DE/EN)
├── INTEGER_LLM/               bit-exact integer inference (Rust + Python)
│   ├── kernels/               compute kernels (RMSNorm, W8A8 linear, RoPE, attention, …)
│   ├── runtime/               model loader, forward pass, KV cache, CLI
│   ├── pipeline/              multi-node orchestration
│   ├── calibrate/             quantization/calibration (Python, offline phase)
│   └── tests/, eval/, …       golden vectors, end-to-end and regression tests
├── SHARED_TYPES/              protocol-wide core data types (implementation started)
├── NETWORKING/                P2P gossip, latency topology (planning phase)
├── CONSENSUS/                 BFT, PoI accounting, epoch allocation (planning phase)
├── VERIFICATION/              redundancy comparison, bisection game (planning phase)
├── TOKENOMICS/                minting function, burn-and-mint (planning phase)
├── COMPUTE_PIPELINE/          pod orchestration over a real network (planning phase)
├── AGENT_LAYER/               session contracts, dual-LLM separation (planning phase)
├── TRAINING/                  data provenance, robust aggregation (planning phase)
├── GOVERNANCE/                parameter registry, model updates (planning phase)
└── CLIENT/                    user client incl. wallet (concept phase)
```

Each component contains a `README/` describing its purpose and status.

## Current state

**INTEGER_LLM** is the only component with an active implementation
(v0.12.33): fully integer inference on a Qwen2.5-0.5B base (int8 weights
with per-channel power-of-two scales, int16 activations with calibrated
per-layer scales), with loader, model forward pass (including
grouped-query attention, Q/K/V biases, and multi-frequency RoPE),
theta_v specification validation (θ_v 0.10.0), export workflow, a real
calibration run (314 calibrated scales, 291 quantized weight tensors
including a dedicated int16 LM head), and qualitatively validated
inference: the quality comparison against the floating-point baseline
(decision point 12.21) is **ACCEPTED** — perplexity 15.59 vs. FP 14.95 =
+4.29 % (criterion: max. +5 %), determinism bit-exact. The evidence is
secured as an evidence package (bit-identity across 5 × 5 independent
runs, 89.3 % top-1 agreement against the BF16 reference, parallel
generation DE/EN; see
`INTEGER_LLM/docs/02_empirischer_beleg_bit-exakte-inferenz.md`).

**SHARED_TYPES** (protocol-wide core data types, Whitepaper Appendix A.1)
is the second component with an active implementation: the design
decisions are made (Rust, SHA-256 as the protocol hash, ECVRF with a
documented post-quantum migration path, BLS12-381, Borsh; quantum
hardening is an overarching design mandate), and the `myl-types` crate
v0.1.1 provides the scaffold with the `Hash` newtype. All other
components are in the planning phase; their implementation follows the
dependency order described in the Whitepaper.

## License

[PolyForm Shield License 1.0.0](LICENSE.md) — using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
