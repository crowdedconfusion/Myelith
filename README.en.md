![Myelith: A decentralized network in which consensus work powers an agentic language model](README/Grafiken/myelith-banner-en.png)

This README is also available in [German](README.md).

Myelith is a decentralized network in which the same computation that
secures consensus simultaneously runs a large agentic language model
("Proof of Inference"). Unlike classical proof-of-work, no discarded
computation is burned; instead, useful inference is performed,
and it is checkable because it runs entirely in integer arithmetic and can
therefore be bit-identical across independent nodes, rather than relying on
trust in any single operator. The native coin MYL (not yet in
circulation) closes the loop: users burn MYL for inference credits, and
miners receive newly minted MYL proportional to verified work.

**What is measured and what is not:** Bit-identity is established across
independent runs, across separate processes, and under network stress, but
so far only on **one hardware architecture**. Proving it across two
architectures is the single most important open item in the project; see
the table under [Core thesis](#core-thesis).

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
Chap. 6.2). No tolerance windows, no "reproducible within measurement
error", no trust in individual operators. Bit for bit or not at all.

**How far the evidence reaches, and where it stops:**

| | Status |
|---|---|
| across independent runs | ✅ measured |
| across separate processes and TCP connections | ✅ measured (four node processes, `tests/integration/test_pipeline_multinode.py`) |
| under latency, packet loss, and node restarts | ✅ measured (`tests/chaos/test_chaos.py`) |
| across different shard cuts (1 to 28) | ✅ measured, over the computed numbers rather than the emitted tokens |
| **across different hardware architectures** | ⏳ **open** |

Everything measured so far ran on **one machine and one architecture**
(arm64). Bit-identity across architectures follows from the number format,
since integer addition is associative, but it has **not been measured**.
The [TESTCLIENT](TESTCLIENT/README/README.md) exists for exactly this proof
and is waiting for an x86_64 machine; its `vergleich` refuses to render a
verdict when all logs come from the same machine.

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

**The right-hand column separates what is built from what is designed.**
The whitepaper describes all four layers; three are implemented.

| Layer | Task | Status |
|---|---|---|
| **L3 Agent Layer** | Agentic workflows, tool use, sessions, session contracts | **Design only, not a line of code.** Carries the prompt-injection defence in the whitepaper |
| **L2 Compute Layer** | Model shards, pods, pipeline routing, redundant computation | implemented; operated on one machine so far |
| **L1 Consensus Layer** | BFT consensus, Proof-of-Inference aggregation, staking, slashing | implemented; never run across real network boundaries |
| **L0 Networking Layer** | P2P gossip, latency topology, encrypted activation streams | implemented through phase 2; encryption of activation streams is design only |

Cutting across these: **TOKENOMICS** implemented, **TRAINING** in small
part (data provenance, growth operator), **GOVERNANCE** in phase 1 since
2026-08-24 (parameter registry). What a
reviewer should read first is in
[Signatur-Bedrohungsmodell.md](SHARED_TYPES/README/Signatur-Bedrohungsmodell.md)
(German).

## Components

Each component has its own folder with a roadmap, design decisions, and
tests:

| Component | Task | Status |
|---|---|---|
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exact integer inference and, new, the integer backward pass (Rust + Python) | **Acceptance criterion ≤ 5 % met on both models:** 0.5B +2.11 %, 7B **+1.14 %** (previously +377 %). That is 0.30 points above the floor of the quantisation scheme itself (+0.84 %). Throughput most recently **+52 % / +40 %** (0.5B / 7B) from dropping a per-token weight copy; before that **+27 % / +43 %** from NEON. Both at bit-identical output, 30/30 conformance vectors on both backends. The [scale pack](INTEGER_LLM/scale_packs/README.md) makes the artifact build bit-identical across platforms: 1.8 MB instead of 8.8 GB, and 20 minutes become 40 seconds |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | core data types, cryptography (VRF, BLS, Merkle, erasure) | Phase 2 complete. BLS with proof of possession against rogue-key attacks, with an executable regression; erasure coding over GF(2⁸) in Cauchy form, verified across **all 495** subsets of 8 from 12. Since 2026-08-23 a written [threat model of all seven signature uses](SHARED_TYPES/README/Signatur-Bedrohungsmodell.md) (German) exists as preparation for the external cryptography review. Writing it surfaced two things that only become visible side by side: the shard transition signature is the **only one without domain separation**, and its protection rests entirely on all message lengths being pairwise distinct |
| [NETWORKING](NETWORKING/README/README.md) | P2P gossip, peer discovery, latency topology | Phase 2 complete: pairwise latency measurement, LatencyGraph, geographic and AS diversity. Since v0.3.0 an **adversarial test layer for the gossip parsers**, and it found two things: the latency EMA computed in floating point although the crate header promises fixed point, and the floating-point audit could not find it because `myl-net` was in its list with **not a single file** (finding 44; eight files added). Plus a measurement of how much the structure check actually filters: for types made entirely of fixed-width fields a Borsh parse **is a length check**, and 20,000 of 20,000 mutated PoI bundles get through (finding 45) Since **v0.4.0** a connection limit (finding 53): inbound and outbound connections get **separate budgets**, so a flood cannot consume the self-chosen slots. The promise is "the node may choose", not "it chooses correctly" — whoever supplies the bootstrap list bypasses it. Two findings along the way: a hardening measure that silenced the honest node as well (finding 54), and a documented validation entry point that the runtime could never reach (finding 55). **v0.5.0** adds NAT traversal (AutoNAT, relay, DCUtR, QUIC): before it, only publicly reachable nodes could participate, which is not merely inconvenient but pushes β in the collusion calculation upward. Demonstrated with a node that listens on no dialable address and is reached anyway; **hole punching itself remains unverified**, it needs two real NATs (finding 56: a relay without its own address accepts reservations and answers into the void) |
| [NODE](NODE/README/README.md) | The node: the program that runs the protocol | **New on 2026-08-24.** Until then the project had thirteen components, some 1500 tests and **no program that starts a node**; `myl-net` had not a single consumer in the whole repository. That was the cause of an entire class of findings: 52, 55, 56 and 57 all became visible when someone put the pieces together. The node speaks TCP and QUIC, works behind NAT via relays, and writes a JSONL operations log that `myl-test netz` evaluates. Demonstrated with two real processes. **It does not produce blocks:** the state machines are complete, but nothing drives them over time; round timing, mempool and chain state are missing. ⚑ Finding 57 along the way: for blocks too the Borsh parse is barely more than a length check, about 88 % of mutated ones get through |
| [CONSENSUS](CONSENSUS/README/README.md) | ledger, BFT, slashing | **All four phases complete.** Signed, weight-based BFT with VRF-rotated committee selection, double-signing proofs, and round changes with locking, giving safety **and** liveness, verified through an acceptance test matrix over 21 simulated validators. Plus PoI bundle submission, epoch close, and data availability (Reed-Solomon k=8/m=4 across the dispute window). Since v0.11.0 there is an **adversarial test layer**: nine attacks on the polka certificate, from the repeated vote to the certificate reused in another round, all rejected. Since `myl-ledger` v0.2.0 the ledger carries **invariant tests over random transition sequences**: MYL never rises, credits are backed by burnt MYL, and a **rejected** transition leaves the state bit-identical. Since v0.10.0 the **work share of the voting weight is calibrated and capped**: its previous reference value equalled the forward pass of a single token, so one hour of work would have raised the stake a thousandfold |
| [TOKENOMICS](TOKENOMICS/README/README.md) | burn-and-mint, distribution | Phase 2 complete: mint function, distribution key, and credit pricing with a frozen exp() lookup table, entirely in integers. Since v0.4.0 the economic estimate is on the table (criticism K8). It first read 3.6× to 9.2× against a centralised provider, and that led to a finding: the integer path was running **single-threaded** while the comparison side was not. After parallelising by row the network costs **3.2× (0.5B) and 1.9× (7B)** per token, and at 7B nearly all of that is redundancy. Since v0.3.0 it is also settled **how a shard earns its credit**: by its share of the weight arithmetic of one forward pass rather than by layer count, because at 0.5B the LM head weighs more than nine layers. Splits from 1 to 28 shards therefore distribute the same total. **Since v0.7.0 the component's roadmap is complete:** stake proportional to claimed capacity, the slashing table from Chap. 5.5 as a data record rather than scattered constants, the bootstrap phase in which a raised sampling rate lowers the stake requirement **quadratically**, and the genesis distribution, where "no pre-sale" is not checked but enforced **by the shape of the function**: it takes proofs of work and nothing else. Every number the paper gives for these is a test, and they all match. Since v0.5.0 an **adversarial test layer across the edges of the number range**, because every parameter in this crate is meant to be governed: it found three places where the widening to `u128` came one operation too late, the worst consequence being a **negative credit price**, that is, a protocol paying users to consume inference (findings 46 and 47) |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | pod orchestration over a real network | Phase 2.1 complete: micro-batching and pipelining. **Pipeline determinism is bit-identical with the single node again,** now that the lossy boundary step between stages is gone (findings 20/26). The trace therefore binds the transmitted activation once more. Since v0.3.0 "bit-identical" here is evidenced over the **computed numbers** rather than the emitted tokens: the pod exposes a digest over logits and tokens, and it matches the single node across 1 to 24 shards (finding 36). Since v0.6.0 there is an **adversarial test layer for the wire format**, and its first run found a defect: the tamper check passed vacuously on an empty trace, and a mismatched length crashed the kernel (finding 41, fixed). ⚑ **Since v0.9.0 the reward path is end-to-end for the first time.** The byzantine-coordinator test put pod and consensus together for the first time and found that they did not fit: the pod's bundle aggregated the **transition signatures** while consensus verified against the **bundle message**, so no bundle ever verified (finding 52). What was missing was not code but a protocol step. Members now see the **finished** bundle, check the claimed work against their own segment count, and only then sign; a signature given without checking would not be consent but an attendance note. **Since v0.7.0 failure handling is in place:** standby takeover and KV cache rebuild, with the liveness promise from Chap. 6.8 **measured rather than asserted** — both halves of it, two failures yes and three no. That the standby can rebuild its cache **bit-identically** is not a given but a consequence of the core decision: in integer arithmetic a prefill over the same tokens yields the same cache; in a floating-point system the rebuild would break the session. Since v0.5.0 the **trace carries one entry per layer rather than per shard**: its length now follows the model instead of the split, two pods with different node counts are comparable, and bisection narrows down the faulty layer rather than the faulty layer group |
| [VERIFICATION](VERIFICATION/README/README.md) | redundancy comparison, bisection game | Phase 2 complete: bisection in O(log L), on-chain adjudication behind the `ShardExecutor` trait, slash decision kept separate from the amounts. Since v0.4.0 an **adversarial test layer**, and it found the most serious defect in the project so far: the bisection game systematically named layer `d − 1` instead of `d`, so adjudication would have recomputed the wrong, correctly computed layer, **acquitting the cheat and slashing the honest checker**, in 15 of 16 cases (finding 42). The existing tests checked that it converges in O(log L) rounds and narrows to an interval of length 1; both were true, and whether the named position is the **right** one was checked by none of them. **Since v0.5.0 the control segments from Chap. 6.7 are in place** — the only mechanism in the architecture that works against the **one-off** intervention, since levels 1 and 2 both assume either an honest twin pod or a repeat offender. **Indistinguishability** from real requests is something code cannot deliver; it is a property of the data and remains the open measurement question the whitepaper itself names. Also since v0.5.0, both of the paper's security arguments are **measured against the implementation** rather than recomputed: the collusion bound from Appendix B.2 matches the real pod assignment to three digits (3.900e-3 vs 3.906e-3), and the independence claimed in Chap. 6.8 holds to within 0.01 %. Level 3 (zkML anchor) is an upgrade path and has not been started |
| [TESTCLIENT](TESTCLIENT/README/README.md) | terminal test client: hardware tests, sharded inference, evaluation | Phase 1 and **phase 3** complete, plus items 2.1 and 2.4. `vergleich` compares the logs of several machines and issues the verdict, and **refuses** one when every log comes from the same machine, when a run was aborted, or when two runs measured different things. Since v0.8.0 the comparison value covers the **computed numbers**, not just the emitted tokens (finding 36). Test plans are no longer tied to a model; a curated model catalogue records provenance, revision and licence. Since v0.11.0 the client also accompanies model changes: `--erwarte` fails a run that produces a different comparison value, and `modellstaende` answers in one call which values a θ_v change moved and which it did not. The proof itself still awaits heterogeneous hardware |
| [TRAINING](TRAINING/README/README.md) | data provenance, robust aggregation | **The one measurement is done (2026-08-22): it holds**, given stochastic rounding of the weights. Full integer scheme against floating point on held-out text: **+0.67 %** (criterion ≤ 10 %); with round-to-nearest, +29.9 %, because one SGD step is a median 6.4e-6 of a grid step. The randomness costs no determinism: the dice is a function of (layer, step, index), not a state. And the training step needs **no floating-point state at all**: integer master, exact integer addition, +0.75 %. Growth is **exactly function-preserving** (deviation 0.00e+00, via an integer split rather than a halving), and the copies' symmetry breaks without artificial noise. Concept and roadmap are in place. **Since v0.1.0 the component also has code:** the data provenance of chapter 7.3, meaning corpora anchored by a Merkle root, segments referenced by proof rather than by raw data, and an assignment that follows from the epoch seed rather than from the miner's choice. Since v0.2.0 it also carries the **growth operator**: an integer split rather than the halving used in the literature, exactly function-preserving and checked by digest rather than by tolerance. The bit budget is computed across four learning rates; the point where the master needs 64 instead of 32 bits lies between 1e-4 and 1e-5 |
| [ETHICS](ETHICS/README/README.md) | ethical and legal standards, manifesto | Manifesto v1.0.0 in place, roadmap in place, design decisions open. **Principle G7 (the base model must be freely reusable) has been checked across all seven Qwen2.5 sizes:** five are Apache 2.0, while 3B and 72B fail, and the 72B clause triggering at 100 million monthly active users is structurally unmeetable for an open protocol |
| [GOVERNANCE](GOVERNANCE/README/README.md) | parameter registry, model updates | **Phase 1 complete; the component has had code since 2026-08-24** (`myl-governance` v0.1.0): 21 parameters in one place, each with a citation and a mutability rank; the constitutional rank from Chap. 10.3 is enforced **technically** and hangs off the type rather than off a record; eight safety conditions from Appendix B are checked **on the proposal** rather than after the vote. The consistency test holds the registry against the constants of the other crates and found on its first run that the **dispute period was 7 hours instead of 7 days** (finding 50): the epoch length had never been fixed, and two parts of the project had silently assumed different values. Crypto agility for the post-quantum migration is anchored; the voting mechanism itself is open |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | session contracts, dual-LLM separation | Planning phase, blocked by the layers below |
| [CLIENT](CLIENT/README/README.md) | user client including wallet | Concept phase |

## Security status

A [security audit](SIMULATION/Sicherheitsaudit.md) (German) covers the
thirteen attack classes from whitepaper Chap. 5.6 and 9.2: **eight are
defended and evidenced, three are open with a measured gap, two are
unreviewed.** It is not an external review and does not replace one.

The three open ones are named rather than paraphrased: a node **accepts
unlimited connections** (no connection limits, no peer scoring), the
**latency attestation signature is verified by nobody**, and the
**indistinguishability of control segments** is an open measurement
question the whitepaper itself carries as such. All three hang on the same
point: whoever controls what a node observes controls verification.

A dedicated crate [`myl-simulation`](SIMULATION/) walks a segment through
every layer and thereby tests the **seams**. The reason is in the finding
history: almost every serious defect in this project sat not inside a
component but between two, and each was correct within its own.

## License

[PolyForm Shield License 1.0.0](LICENSE.md). Using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
