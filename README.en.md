![Myelith: A decentralized network in which consensus work powers an agentic language model](README/Grafiken/myelith-banner-en.png)

This README is also available in [German](README.md).

**Myelith makes consensus work useful.** The same computation that secures
the network runs a large agentic language model. Not a burned crypto game
(proof-of-work), but inference somebody can actually use, and
**checkable**: because it runs entirely in integer arithmetic, independent
nodes produce bit-identical results.

**For scale:** Bitcoin consumes roughly **150 terawatt-hours of
electricity per year** according to the
[Cambridge index](https://ccaf.io/cbnsi/cbeci), more than the Netherlands
in total, and the output of that work is a useless number nobody outside
the blockchain can use. Myelith turns that energy into inference somebody
ordered and paid for!

The native coin MYL closes the loop: users burn it for inference credits
and miners receive newly minted MYL in proportion to verified work.

The complete architecture, tokenomics, and verification model are set out
in **Whitepaper v0.3**:
[German (MD)](README/Whitepaper/myelith-whitepaper-v0.3.md) ·
[German (PDF)](README/Whitepaper/myelith-whitepaper-v0.3.pdf) ·
[English (MD)](README/Whitepaper/myelith-whitepaper-v0.3-en.md) ·
[English (PDF)](README/Whitepaper/myelith-whitepaper-v0.3-en.pdf).
Every technical term, from the bisection game to fixed-point arithmetic,
is explained in the **[glossary](README/Glossary.en.md)**, with pointers
to the corresponding implementation.

---

## Where the project stands

Three weeks turned a planning phase in early August into **seventeen crates,
over 1,800 tests, and a running node.**

| | |
|---|---|
| **Core thesis proven** | Integer inference costs **+1.1 % perplexity at 7B**; the bar was ≤ 5 %, and it succeeds also at **MoE Model with 30B** |
| **And it is fast** | At 7B the integer path is **faster than bf16** on the same machine |
| **The network runs** | Nodes find each other over QUIC, work behind home routers, build blocks, let latecomers catch up |
| **State converges** | Three processes, thirteen blocks, **identical state roots at every height** |
| **Security** | 13 attack classes reviewed: **8 defended, 4 with a named residual condition** |
| **Cost** | **1.9× a centralized provider at 7B**, and nearly all of that is redundancy |

---

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
| Qwen3-4B | 19.95 | 19.63 | **+1.6 %**, criterion ≤5 % met |
| Qwen2.5-7B | **8.78** | 8.68 | **+1.1 %**, criterion ≤5 % met |
| Qwen3-30B-A3B (MoE) | 10.42 | 10.48 | **no measurable gap**, criterion met |

*The metric is perplexity on WikiText-2 under teacher forcing, on identical
sequences for both paths; lower is better. "Gap" is the relative premium the
integer path pays over its own BF16 reference. On 7B that figure stood at
**+377 %** before the bug hunts (perplexity 41.42); today it is **+1.1 %**,
which puts it **0.3 percentage points above the theoretical floor of the
quantisation scheme itself** (+0.84 %, measured independently).

**Bit-identity here is not a side effect, it is the product.** What matters
is the agreement of the integer path with itself: across independent runs,
nodes, and hardware. No tolerance windows, no "reproducible within
measurement error", no trust in individual operators. Bit for bit or not
at all.

Closeness to the floating-point reference comes out better than the
percentage suggests. In the
[qualitative benchmark](INTEGER_LLM/README/README.md#qualitative-benchmark)
over eight real prompts, 7B produces word-for-word the same text as BF16
in five of eight cases, at 73.8 % matching tokens. That is a quality
figure, not a target: 8/8 would not be a success but a hint that the
quantisation does nothing. Details in the
[whitepaper (Chap. 6.9)](README/Whitepaper/myelith-whitepaper-v0.3-en.md)
and in [INTEGER_LLM](INTEGER_LLM/README/README.md).

## Architecture

Four layers (Whitepaper Chap. 3.2), with tokenomics, training, and
governance cutting across them:

**Three of four layers are running, with caveats.**

| Layer | Task | Status |
|---|---|---|
| **L3 Agent Layer** | Agentic workflows, tool use, session contracts | Contracts stand, separation still missing |
| **L2 Compute Layer** | Model shards, pods, pipeline routing, redundancy | **running**, bit-identical across 1 to 24 shards |
| **L1 Consensus Layer** | BFT, PoI aggregation, staking, slashing | **running**, all four phases complete. Driving consensus over the wire comes with validator registration |
| **L0 Networking Layer** | P2P gossip, latency topology, NAT traversal | **running**, including relays and QUIC. Encrypted activation streams are the next build-out |

Also: **TOKENOMICS** complete, **GOVERNANCE** with the parameter
registry, **TRAINING** with data provenance and the growth operator.

## Components

Every component has its own folder with design decisions and tests.
The short version here:

| Component | What it delivers |
|---|---|
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | **The core thesis, measured.** Integer inference at **+1.14 % at 7B** (bar: ≤ 5 %), 0.3 points above the floor of the quantisation scheme; for the **30B mixture of experts** (128 experts per layer) no measurable gap. Throughput **+419 % at 7B**, faster than bf16. Since 28 August the training side of the mixture too: backward pass, bit-identical across two runs, saturation guard, expert growth. The [scale pack](INTEGER_LLM/scale_packs/README.md) makes artefact builds bit-identical, 1.8 MB instead of 8.8 GB, 40 s instead of 20 min |
| [NODE](NODE/README/README.md) | **The binary that runs the protocol.** Peers over TCP and QUIC, relays behind routers, chained blocks from a mempool, catch-up in milliseconds, signature checks, block height and epoch separated, an analysable operating log. Demonstrated on five independent processes committing the same block and surviving leader failure |
| [NETWORKING](NETWORKING/README/README.md) | **L0 stands.** Gossip, Kademlia, latency topology, NAT traversal with AutoNAT, relays, DCUtR, QUIC. Connection limits with **separate budgets** against Sybil floods. Point-to-point channel with opaque payload: the network layer does not know what a block is. Sessions end-to-end encrypted, key exchange **hybrid** from X25519 and ML-KEM-768: recordings stay safe against later breaking |
| [CONSENSUS](CONSENSUS/README/README.md) | **Four phases built, one gap found.** Signed, stake-weighted BFT with VRF committee selection, double-signing proof and round change, so safety **and** liveness, verified on 21 validators. Plus PoI bundles, epoch close, Reed-Solomon, session contracts in state. A transfer between accounts is still missing. The algorithm change becomes a switch, not a migration |
| [VERIFICATION](VERIFICATION/README/README.md) | **Three stages against fraud.** Redundancy comparison, bisection in O(log L), control segments against the one-off intervention, reserve and observation window as parameters. The paper's security arguments are **measured against the implementation**: collusion bound to three digits, independence within 0.01 %. The instrument for indistinguishability stands, the traffic for it does not |
| [TOKENOMICS](TOKENOMICS/README/README.md) | **Planning complete.** Minting, distribution, credit pricing, stake by capacity, graduated slashing over a violation history, bootstrap, genesis, burn cap per address. Fully integer. "No presale" is not checked but **enforced by how the function works**: it accepts proofs of work and nothing else. Every number in the paper stands as a test |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | **Pods compute bit-identically.** 1 to 24 shards yield the same digest over logits and tokens as a single node, mixtures of experts included. Failover with standby takeover and bit-identical KV cache rebuild, only possible in integers at all. Assignment on the scheduler, not on an assumption |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | **The foundation.** VRF, BLS with proof-of-possession, Merkle, erasure coding over GF(2⁸), verified across **all 495** subsets of 8 from 12. The Merkle root also commits to the leaf count, otherwise two leaf sequences would share a root. The [threat model for all seven signature uses](SHARED_TYPES/README/Signatur-Bedrohungsmodell.md) is written up |
| [TESTCLIENT](TESTCLIENT/README/README.md) | **The tool for the tests ahead.** One program, one menu, three questions: does your machine compute what ours does, does it hold the conformance vectors, and do several machines find each other over the internet? `vergleich` **refuses** a positive verdict when all logs come from the same machine. |
| [GOVERNANCE](GOVERNANCE/README/README.md) | **Parameters in one place, with rank.** 33 parameters with provenance and rank; the constitutional rank from Chap. 10.3 is enforced **technically**. Nine conditions are checked **on the proposal**, not after the vote. Plus voting with quorum, majority and window, a model manifest, and the switch for the algorithm change: one-way, one step |
| [TRAINING](TRAINING/README/README.md) | **The one measurement is done: it holds.** Integer training at **+0.67 %** against floating point, with stochastic rounding, **entirely without floating-point state**. Growth exactly function-preserving, 0.00e+00. For mixtures of experts likewise, with load balancing without randomness |
| [SIMULATION](SIMULATION/README.md) | **Tests the interlocks, not the modules.** Drives a segment through every layer, because almost every serious finding in this project sat between two components and was correct inside each |
| [ETHICS](ETHICS/README/README.md) | Manifesto v1.0.0 in place. **Principle G7 checked for all seven Qwen2.5 sizes:** five of them Apache 2.0 |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | ⚑ **A contract is not a program, it is a blast radius.** Budget, recipients and deadline are fixed and checked by consensus; nobody can change them, because a different contract has a different address |
| [CLIENT](CLIENT/README/README.md) | Concept phase |

## Security status

A [security audit](SIMULATION/Sicherheitsaudit.md) takes up the thirteen
attack classes from Whitepaper Chap. 5.6 and 9.2. **Since 25 August not a
single one is marked open (an external audit follows after the remaining
tests and troubleshooting):**

| Status | Count |
|---|---|
| defended and evidenced | **8** |
| closed, with a named residual condition | **4** |
| never externally reviewed | 1 |

The four residual conditions share one shape. The mechanism is in place
and measured; the last prerequisite depends on validator registration at
genesis.

## What comes next

Four things, ordered by priority:

1. **Bit-identity across two architectures.** It follows from the number
   format and is so far measured on one. The
   [TESTCLIENT](TESTCLIENT/README/README.md) is built for exactly this
   proof and is waiting for an x86_64 machine.
2. **Validator registration at genesis.** Unblocks BFT rounds over the
   wire and the last two residual conditions in the audit.
3. **Chain state on disk.** In memory today, which is fine for rehearsals
   and not for a testnet.
4. **External cryptography review.** Before mainnet, not after.

**What runs today is a dry run, not a testnet.** The state is throwaway,
the MYL in it is play money, and the starting value of the rehearsal
chain says so in plain text. When the testnet begins is a
decision, not a consequence of the code running.

## License

[PolyForm Shield License 1.0.0](LICENSE.md). Using, modifying, and
commercially participating in the Myelith network (mining, validation,
gateways, clients) is permitted; operating a competing network or product
based on the code is not.
