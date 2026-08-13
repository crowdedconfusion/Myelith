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
├── NETWORKING/                P2P gossip, latency topology (implementation started)
├── CONSENSUS/                 BFT, PoI accounting, epoch allocation (implementation started)
├── VERIFICATION/              redundancy comparison, bisection game (planning phase)
├── TOKENOMICS/                minting function, burn-and-mint (implementation started)
├── COMPUTE_PIPELINE/          pod orchestration over a real network (implementation started)
├── AGENT_LAYER/               session contracts, dual-LLM separation (planning phase)
├── TRAINING/                  data provenance, robust aggregation (planning phase)
├── GOVERNANCE/                parameter registry, model updates (planning phase)
└── CLIENT/                    user client incl. wallet (concept phase)
```

Each component contains a `README/` describing its purpose and status.

## Current state

**INTEGER_LLM** is the only component with an active implementation
(v0.12.34): fully integer inference on a Qwen2.5-0.5B base (int8 weights
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
`INTEGER_LLM/docs/02_empirischer_beleg_bit-exakte-inferenz.md`). The
multi-node pipeline (4 stages on 4 nodes) performs real layer execution
and provably produces token sequences bit-identical to the single-node
runtime — even under artificial latency, packet loss (retry logic), and
node restarts.

**SHARED_TYPES** (protocol-wide core data types, Whitepaper Appendix A.1)
is the second component with an active implementation — **Phase 1 is
complete** (myl-types v0.1.7): the `Hash` newtype, the Merkle tree, the
VRF interface (ECVRF per RFC 9381, verified bit-exact against the
official RFC test vectors), BLS12-381 signatures (min-pk, including
signature aggregation and rogue-key protection for the PoI bundles), and
the core structs `Segment`/`PoIBundle`/`InferenceCredit` exactly per
Appendix A.1. The design decisions are documented (Rust, SHA-256, Borsh;
quantum hardening as an overarching mandate with anchored post-quantum
migration paths).

**NETWORKING** (L0 network layer, Whitepaper Chap. 3.2) is the third
component with an active implementation — **Phase 1 is complete**
(myl-net v0.1.4): node identity and swarm setup
(Gossipsub/Identify/Ping/Kademlia over TCP/Noise), the gossip topic
structure (blocks, transactions, PoI bundles, challenges, latency
attestations) with Borsh payloads, and message validation before
forwarding. Both acceptance criteria are empirically met: a testnet of
20 nodes reaches full gossip connectivity in under 5 seconds, and
invalid messages are not forwarded (adversarial test).

**CONSENSUS** (L1 consensus layer, Whitepaper Chap. 3.5 and
Appendix A.5) has also started implementation: the design decisions are
made (malachite consensus engine behind a narrow trait boundary with a
documented custom-build fallback, 2 s block time, a committee of 21
block-producing validators and 7 adjudicators, 7-day dispute window,
Reed-Solomon erasure coding k=8/m=4), and the ledger `myl-ledger`
v0.1.5 has **completed Phase 1** — all state transitions from
Appendix A.5 (burn→mint_credits, apply_verdict, credit_spend) as pure,
atomic integer functions, with the acceptance criterion met: replaying
the same transition sequence yields bit-identical states across two
independent runs.

**TOKENOMICS** (Whitepaper Chap. 5) has also started implementation:
the design decisions are made (fixed-point integer arithmetic,
1 MYL = 10⁶ base units, vTFE scaling 10⁻⁶, 30-epoch EMA window), and
`myl-tokenomics` v0.1.4 has **completed Phase 1** — integer EMA for the
smoothed burn volume, the minting function, the Chap.-5.3 distribution
(78/5/10/4/3 % with an exact-sum invariant), and the training-reward
cap; the acceptance criterion is met (10,000-epoch determinism and
distribution-exactness tests).

**COMPUTE_PIPELINE** (L2 compute layer, Whitepaper Chap. 4 and Appendix
A.3) has also started implementation: the design decisions are made
(separate crate `myl-pod`, 250 ms micro-batching window, draft-model
direction layer-subset from the same θ_v family), and `myl-pod` v0.1.4
has **completed Phase 1** — the pod mining loop (`shard_loop`) with
trace hashes and tamper detection, `coordinator_loop` with
micro-batching, KV-cache session affinity, and erasure-coded DA
archiving. The acceptance criteria are met: a 4-node pod produces a
bit-identical token sequence on a repeated identical prompt and is
bit-identical to the single-node runtime; the input-hash check rejects
tampered activations. All other
components are in the planning phase; their implementation follows the
dependency order described in the Whitepaper.

## License

[PolyForm Shield License 1.0.0](LICENSE.md) — using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
