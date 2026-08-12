![Myelith — Ein dezentrales Netzwerk, in dem Konsensarbeit ein agentisches Sprachmodell betreibt](README/Grafiken/myelith-banner.png)

Dieses README ist auch auf [Englisch](README.en.md) verfügbar.

Myelith ist ein dezentrales Netzwerk, in dem dieselbe Rechenarbeit, die den
Konsens sichert, zugleich ein großes agentisches Sprachmodell betreibt
(„Proof of Inference"). Anders als bei klassischem Proof-of-Work wird keine
verworfene Rechenleistung verbrannt, sondern nützliche Inferenz erbracht,
deren Korrektheit über vollständig ganzzahlige, bitgleiche Ausführung
überprüfbar ist. Der native Coin MYL schließt den Kreislauf: Nutzer
verbrennen MYL gegen Inferenz-Credits, Miner erhalten neu geprägte MYL
proportional zur verifizierten Arbeit.

Die vollständige Architektur, Tokenomics, das Verifikationsmodell und die
offenen Forschungsfragen stehen im Whitepaper v0.3:
[Deutsch (MD)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3.md) /
[Deutsch (PDF)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3.pdf) /
[English (MD)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3-en.md) /
[English (PDF)](README/Whitepaper/myelith-v0.3/myelith-whitepaper-v0.3-en.pdf).
Die Simulationsprogramme zu Anhang B liegen unter
[`README/Whitepaper/myelith-v0.3/simulations/`](README/Whitepaper/myelith-v0.3/simulations/).

## Kernthese

Ganzzahladdition ist assoziativ. Wird Inferenz vollständig in
Ganzzahlarithmetik ausgeführt (kein Gleitkomma im Rechenpfad, Division
ausschließlich als arithmetischer Rechtsshift), entsteht Bitgleichheit
zwischen unabhängigen Knoten — die Grundlage der gesamten
Verifikationsarchitektur (Redundanzvergleich, Bisektions-Spiel,
Kontrollsegmente, Whitepaper Kap. 6). Ob ganzzahlig quantisierte Inferenz in
der Zielgrößenordnung qualitativ trägt, ist dabei eine offene Messfrage, die
das Projekt am kleinen Modell beantwortet, bevor Infrastruktur skaliert wird.

## Architektur

Vier Schichten (Whitepaper Kap. 3.2), dazu querliegend Tokenomics, Training
und Governance:

| Schicht | Aufgabe |
|---|---|
| **L3 Agent Layer** | Agentische Workflows, Tool-Use, Sessions, Session-Kontrakte |
| **L2 Compute Layer** | Modell-Shards, Pods, Pipeline-Routing, Redundanzberechnung |
| **L1 Consensus Layer** | BFT-Konsens, Proof-of-Inference-Aggregation, Staking, Slashing |
| **L0 Networking Layer** | P2P-Gossip, Latenz-Topologie, verschlüsselte Aktivierungs-Streams |

## Repositorystruktur

```
├── LICENSE.md                 PolyForm Shield License 1.0.0
├── README.md                  diese Datei (Deutsch)
├── README.en.md               English version of this file
├── README/Whitepaper/         Whitepaper v0.3 (DE/EN, MD+PDF) + Simulationen
├── README/Grafiken/           Titelgrafiken und Abbildungen (DE/EN)
├── INTEGER_LLM/               bit-exakte Ganzzahl-Inferenz (Rust + Python)
│   ├── kernels/               Rechenkerne (RMSNorm, W8A8-Linear, RoPE, Attention, …)
│   ├── runtime/               Modell-Loader, Forward-Pass, KV-Cache, CLI
│   ├── pipeline/              Mehrknoten-Orchestrierung
│   ├── calibrate/             Quantisierung/Kalibrierung (Python, Offline-Phase)
│   └── tests/, eval/, …       Golden Vectors, End-to-End- und Regressionstests
├── SHARED_TYPES/              protokollweite Kern-Datentypen (Implementierung begonnen)
├── NETWORKING/                P2P-Gossip, Latenztopologie (Planungsphase)
├── CONSENSUS/                 BFT, PoI-Abrechnung, Epochen-Zuteilung (Planungsphase)
├── VERIFICATION/              Redundanzvergleich, Bisektions-Spiel (Planungsphase)
├── TOKENOMICS/                Prägefunktion, Burn-and-Mint (Planungsphase)
├── COMPUTE_PIPELINE/          Pod-Orchestrierung über echtes Netz (Planungsphase)
├── AGENT_LAYER/               Session-Kontrakte, Dual-LLM-Trennung (Planungsphase)
├── TRAINING/                  Datenprovenienz, robuste Aggregation (Planungsphase)
├── GOVERNANCE/                Parameter-Registry, Modell-Updates (Planungsphase)
└── CLIENT/                    Nutzer-Client inkl. Wallet (Konzeptphase)
```

Jede Komponente enthält ein `README/` mit Zielbeschreibung und Status.

## Stand

**INTEGER_LLM** ist die einzige Komponente mit laufender Implementierung
(v0.12.33): Fully-Integer-Inferenz auf Qwen2.5-0.5B-Basis (Gewichte int8
mit Per-Channel-Zweierpotenz-Skalen, Aktivierungen int16 mit kalibrierten
Per-Layer-Skalen), mit Loader, Modell-Forward-Pass (inkl. Grouped-Query-
Attention, Q/K/V-Biases und Multi-Frequenz-RoPE), theta_v-
Spezifikationsvalidierung (θ_v 0.10.0), Export-Workflow, echtem
Kalibrierungslauf (314 kalibrierte Skalen, 291
quantisierte Gewichts-Tensoren inkl. eigenem LM-Head in int16) und
qualitativ validierter Inferenz: Der Qualitätsvergleich gegen die
Gleitkomma-Baseline (Entscheidungspunkt 12.21) ist **AKZEPTIERT** —
Perplexität 15,59 vs. FP 14,95 = +4,29 % (Kriterium max. +5 %),
Determinismus bit-exakt. Der Beleg ist als Evidenz-Paket gesichert
(Bit-Identität über 5 × 5 unabhängige Läufe, Top-1-Agreement 89,3 %
gegen die BF16-Referenz, Parallelgenerierung DE/EN;
`INTEGER_LLM/docs/02_empirischer_beleg_bit-exakte-inferenz.md`).

**SHARED_TYPES** (protokollweite Kern-Datentypen, Whitepaper Anhang A.1)
ist die zweite Komponente mit laufender Implementierung — **Phase 1 ist
vollständig** (myl-types v0.1.6): Hash-Newtype, Merkle-Baum, die
VRF-Schnittstelle (ECVRF nach RFC 9381, gegen die offiziellen
RFC-Testvektoren bit-exakt geprüft), BLS12-381-Signaturen (min-pk, inkl.
Signatur-Aggregation und Rogue-Key-Schutz für die PoI-Bündel) sowie die
Kern-Structs `Segment`/`PoIBundle`/`InferenceCredit` exakt nach
Anhang A.1. Die Design-Entscheidungen sind dokumentiert (Rust, SHA-256,
Borsh; Quantum-Hardening als übergreifende Vorgabe mit verankerten
Post-Quantum-Migrationspfaden).

**NETWORKING** (L0-Netzwerkschicht, Whitepaper Kap. 3.2) ist die dritte
Komponente mit laufender Implementierung — **Phase 1 ist vollständig**
(myl-net v0.1.4): Node-Identität und Swarm-Aufbau
(Gossipsub/Identify/Ping/Kademlia über TCP/Noise), Gossip-Topic-
Struktur (Blöcke, Transaktionen, PoI-Bündel, Challenges, Latenz-
Atteste) mit Borsh-Payloads und Nachrichtenvalidierung vor
Weiterverbreitung. Beide Akzeptanzkriterien sind empirisch erfüllt:
ein Testnetz aus 20 Nodes erreicht Voll-Konnektivität über Gossip in
unter 5 Sekunden, und ungültige Nachrichten werden nicht
weiterverbreitet (adversarialer Test).

**CONSENSUS** (L1-Konsensschicht, Whitepaper Kap. 3.5 und Anhang A.5)
hat ebenfalls die Implementierung begonnen: Die Design-Entscheidungen
sind getroffen (malachite-Konsens-Engine hinter schmaler trait-Grenze
mit dokumentiertem Eigenbau-Fallback, Blockzeit 2 s, Komitee aus 21
Blockproduktions-Validatoren und 7 Schiedsrichtern, Streitfrist
7 Tage, Reed-Solomon-Erasure-Coding k=8/m=4), und das Ledger
`myl-ledger` v0.1.5 hat **Phase 1 vollständig** abgeschlossen — alle
Zustandsübergänge aus Anhang A.5 (burn→mint_credits, apply_verdict,
credit_spend) als reine, atomare Ganzzahl-Funktionen, mit erfülltem
Akzeptanzkriterium: Replay derselben Übergangsfolge liefert auf zwei
unabhängigen Läufen bitgleiche Zustände. Alle
übrigen Komponenten sind in der Planungsphase; ihre Umsetzung folgt der
im Whitepaper beschriebenen Abhängigkeitsordnung.

## Lizenz

[PolyForm Shield License 1.0.0](LICENSE.md) — Nutzung, Veränderung und
kommerzielle Teilnahme am Myelith-Netzwerk (Mining, Validierung, Gateways,
Clients) sind erlaubt; ein konkurrierendes Netzwerk oder Produkt auf Basis
des Codes zu betreiben ist es nicht.
