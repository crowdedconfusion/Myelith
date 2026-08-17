![Myelith — Ein dezentrales Netzwerk, in dem Konsensarbeit ein agentisches Sprachmodell betreibt](README/Grafiken/myelith-banner.png)

Dieses README ist auch auf [Englisch](README.en.md) verfügbar.

Myelith ist ein dezentrales Netzwerk, in dem dieselbe Rechenarbeit, die den
Konsens sichert, zugleich ein großes agentisches Sprachmodell betreibt
(„Proof of Inference"). Anders als bei klassischem Proof-of-Work wird keine
verworfene Rechenleistung verbrannt, sondern nützliche Inferenz erbracht —
verifizierbar, weil sie vollständig ganzzahlig und damit bitgleich zwischen
unabhängigen Knoten abläuft, statt sich auf Vertrauen in einzelne Betreiber
zu verlassen. Der native Coin MYL (noch nicht im Umlauf) schließt den
Kreislauf: Nutzer verbrennen MYL gegen Inferenz-Credits, Miner erhalten neu
geprägte MYL proportional zur verifizierten Arbeit.

Die vollständige Architektur, Tokenomics und das Verifikationsmodell stehen
im **Whitepaper v0.3**:
[Deutsch (MD)](README/Whitepaper/myelith-whitepaper-v0.3.md) ·
[Deutsch (PDF)](README/Whitepaper/myelith-whitepaper-v0.3.pdf) ·
[English (MD)](README/Whitepaper/myelith-whitepaper-v0.3-en.md) ·
[English (PDF)](README/Whitepaper/myelith-whitepaper-v0.3-en.pdf).

## Kernthese

Ganzzahladdition ist assoziativ. Wird Inferenz vollständig in
Ganzzahlarithmetik ausgeführt, entsteht Bitgleichheit zwischen unabhängigen
Knoten — die Grundlage der gesamten Verifikationsarchitektur (Whitepaper
Kap. 6). Ob das bei realistischer Modellgröße auch qualitativ trägt, ist
eine offene Messfrage; das Projekt beantwortet sie zuerst am kleinen
Modell, bevor Infrastruktur skaliert wird.

**Erstes Ergebnis:** Ein 0,5-Milliarden-Parameter-Modell, vollständig
ganzzahlig ausgeführt, erreicht eine Perplexität von 15,59 gegenüber 14,95
in der Gleitkomma-Referenz (+4,3 %, akzeptiert bei ≤5 %) — bei
nachgewiesener Bitgleichheit über unabhängige Läufe und sogar über eine
echte Mehrknoten-Pipeline unter künstlicher Netzwerklast (Latenz,
Paketverlust, Node-Neustarts). Details dazu im
[Whitepaper (Kap. 6.9)](README/Whitepaper/myelith-whitepaper-v0.3.md) und
in [INTEGER_LLM](INTEGER_LLM/README/README.md).

## Architektur

Vier Schichten (Whitepaper Kap. 3.2), ergänzt durch Tokenomics,
Training und Governance:

| Schicht | Aufgabe |
|---|---|
| **L3 Agent Layer** | Agentische Workflows, Tool-Use, Sessions, Session-Kontrakte |
| **L2 Compute Layer** | Modell-Shards, Pods, Pipeline-Routing, Redundanzberechnung |
| **L1 Consensus Layer** | BFT-Konsens, Proof-of-Inference-Aggregation, Staking, Slashing |
| **L0 Networking Layer** | P2P-Gossip, Latenz-Topologie, verschlüsselte Aktivierungs-Streams |

## Komponenten

Jede Komponente hat einen eigenen Ordner mit Fahrplan, Design-Entscheidungen und Tests, welche nachfolgend aufgeführt sind:

| Komponente | Aufgabe | Status |
|---|---|---|
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exakte Ganzzahl-Inferenz (Rust + Python) | Kernthese empirisch bestätigt, Mehrknoten-Pipeline läuft, alle Backends (AVX2+NEON) |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | Kern-Datentypen, Kryptografie (VRF, BLS, Merkle) | Phase 1 + 2 abgeschlossen (Golden Vectors, Fuzz-Harness, Konformitätspaket) |
| [NETWORKING](NETWORKING/README/README.md) | P2P-Gossip, Peer-Discovery, Latenztopologie | Phase 1 + 2 abgeschlossen (Paarlatenzmessung, LatencyGraph, Geo-/AS-Diversität) |
| [CONSENSUS](CONSENSUS/README/README.md) | Ledger, BFT, Slashing | Phase 1 + 2 + 3 abgeschlossen (BFT-Blockproduktion, Validator-Registrierung, Stimmgewichtung) |
| [TOKENOMICS](TOKENOMICS/README/README.md) | Burn-and-Mint, Verteilung | Phase 1 + 2 abgeschlossen (Credit-Preisbildung mit exp()-LUT) |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | Pod-Orchestrierung über echtes Netz | Phase 1 + 2.1 abgeschlossen (Micro-Batching, Pipelining) |
| [VERIFICATION](VERIFICATION/README/README.md) | Redundanzvergleich, Bisektions-Spiel | Phase 1 + 2 vollständig (Schiedsrunde, Slash-Logik) |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | Session-Kontrakte, Dual-LLM-Trennung | Planungsphase |
| [TRAINING](TRAINING/README/README.md) | Datenprovenienz, robuste Aggregation | Planungsphase |
| [GOVERNANCE](GOVERNANCE/README/README.md) | Parameter-Registry, Modell-Updates | Planungsphase |
| [CLIENT](CLIENT/README/README.md) | Nutzer-Client inkl. Wallet | Konzeptphase |

## Lizenz

[PolyForm Shield License 1.0.0](LICENSE.md) — Nutzung, Veränderung und
kommerzielle Teilnahme am Myelith-Netzwerk (Mining, Validierung, Gateways,
Clients) sind erlaubt; ein konkurrierendes Netzwerk oder Produkt auf Basis
des Codes zu betreiben ist es nicht.