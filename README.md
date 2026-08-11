# Myelith

Myelith ist ein dezentrales Netzwerk, in dem dieselbe Rechenarbeit, die den
Konsens sichert, zugleich ein großes agentisches Sprachmodell betreibt
(„Proof of Inference"). Anders als bei klassischem Proof-of-Work wird keine
verworfene Rechenleistung verbrannt, sondern nützliche Inferenz erbracht,
deren Korrektheit über vollständig ganzzahlige, bitgleiche Ausführung
überprüfbar ist. Der native Coin MYL schließt den Kreislauf: Nutzer
verbrennen MYL gegen Inferenz-Credits, Miner erhalten neu geprägte MYL
proportional zur verifizierten Arbeit.

Die vollständige Architektur, Tokenomics, das Verifikationsmodell und die
offenen Forschungsfragen stehen im
[Whitepaper (v0.2)](README/Whitepaper/myelith-whitepaper-v0.2.md).

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
├── README.md                  diese Datei
├── README/Whitepaper/         Whitepaper v0.2
├── INTEGER_LLM/               bit-exakte Ganzzahl-Inferenz (Rust + Python)
│   ├── kernels/               Rechenkerne (RMSNorm, W8A8-Linear, RoPE, Attention, …)
│   ├── runtime/               Modell-Loader, Forward-Pass, KV-Cache, CLI
│   ├── pipeline/              Mehrknoten-Orchestrierung
│   ├── calibrate/             Quantisierung/Kalibrierung (Python, Offline-Phase)
│   └── tests/, eval/, …       Golden Vectors, End-to-End- und Regressionstests
├── SHARED_TYPES/              protokollweite Kern-Datentypen (Planungsphase)
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
(v0.12.15): Fully-Integer-Inferenz auf Qwen2.5-0.5B-Basis (W8A8,
int32-Akkumulator), mit Loader, Modell-Forward-Pass (inkl. Grouped-Query-
Attention), theta_v-Spezifikationsvalidierung und Export-Workflow für die
Kalibrierungsartefakte. Als Nächstes stehen der erste echte
Kalibrierungslauf und der Qualitätsvergleich gegen eine Gleitkomma-Baseline
an. Alle übrigen Komponenten sind in der Planungsphase; ihre Umsetzung
folgt der im Whitepaper beschriebenen Abhängigkeitsordnung.

## Lizenz

[PolyForm Shield License 1.0.0](LICENSE.md) — Nutzung, Veränderung und
kommerzielle Teilnahme am Myelith-Netzwerk (Mining, Validierung, Gateways,
Clients) sind erlaubt; ein konkurrierendes Netzwerk oder Produkt auf Basis
des Codes zu betreiben ist es nicht.
