![Myelith: Ein dezentrales Netzwerk, in dem Konsensarbeit ein agentisches Sprachmodell betreibt](README/Grafiken/myelith-banner.png)

Dieses README ist auch auf [Englisch](README.en.md) verfügbar.

Myelith ist ein dezentrales Netzwerk, in dem dieselbe Rechenarbeit, die den
Konsens sichert, zugleich ein großes agentisches Sprachmodell betreibt
(„Proof of Inference"). Anders als bei klassischem Proof-of-Work wird keine
verworfene Rechenleistung verbrannt, sondern nützliche Inferenz erbracht,
und zwar verifizierbar, weil sie vollständig ganzzahlig und damit bitgleich zwischen
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

Alle Fachbegriffe, vom Bisektions-Spiel bis zur Festkomma-Arithmetik,
sind im **[Glossar](README/Glossar.md)** erklärt, mit Beispielen und
Verweisen auf die jeweilige Implementierung
([English edition](README/Glossary.en.md)).

## Kernthese

Ganzzahladdition ist assoziativ. Wird Inferenz vollständig in
Ganzzahlarithmetik ausgeführt, entsteht Bitgleichheit zwischen unabhängigen
Knoten, die Grundlage der gesamten Verifikationsarchitektur (Whitepaper
Kap. 6). Ob das bei realistischer Modellgröße auch qualitativ trägt, ist
eine offene Messfrage; das Projekt beantwortet sie zuerst am kleinen
Modell, bevor Infrastruktur skaliert wird.

**Ergebnisse,** vollständig ganzzahlig ausgeführt und gegen die
Gleitkomma-Referenz desselben Modells gemessen:

| Modell | Integer-Perplexität | BF16-Referenz | Abstand |
|---|---|---|---|
| Qwen2.5-0,5B | 15,27 | 14,95 | **+2,1 %**, Kriterium ≤5 % erfüllt |
| Qwen2.5-7B | **8,78** | 8,68 | **+1,1 %**, Kriterium ≤5 % erfüllt |

*Gemessen wird Perplexität auf WikiText-2 mit Teacher-Forcing, für beide
Pfade auf identischen Sequenzen; niedriger ist besser. „Abstand" ist der
relative Aufschlag des Integer-Pfads auf seine eigene BF16-Referenz.
Bei 7B lag dieser Wert vor den Fehlersuchen bei 41,42, heute bei 1,1
und damit **0,3 Punkte über dem theoretischen Boden des
Quantisierungsschemas** (+0,84 %, unabhängig gemessen). Der Weg dorthin
führte über vier Implementierungsfehler und zehn Instrumentenfehler, die
sämtlich dokumentiert sind: Der zuletzt gefundene klemmte in der
Residual-Addition beide Summanden einzeln auf die Zielskala und
zerstörte damit jede Auslöschung. An einer Stelle stand −0,002, wo 61,6
richtig gewesen wäre.*

**Bitgleichheit ist hier kein Nebeneffekt, sondern das Produkt.** Worauf es
ankommt, ist die Übereinstimmung des Integer-Pfads mit sich selbst: über
unabhängige Läufe, über Knoten, über Hardware hinweg. Das ist die
Konsensbedingung (Whitepaper Kap. 6.2), und sie steht: nachgewiesen über
unabhängige Läufe, über eine echte Mehrknoten-Pipeline und unter künstlich
erzeugter Netzwerklast aus Latenz, Paketverlust und Node-Neustarts. Keine
Toleranzfenster, kein „reproduzierbar im Rahmen der Messgenauigkeit", kein
Vertrauen in einzelne Betreiber. Bit für Bit oder gar nicht.

Die *Nähe zur Gleitkomma-Referenz* ist eine andere Frage, und sie fällt
besser aus, als der Prozentwert vermuten lässt. Der Integer-Pfad ist eine
Quantisierung, er weicht per Konstruktion ab; genau deshalb existiert
überhaupt ein Perplexitätsabstand. Im
[qualitativen Benchmark](INTEGER_LLM/README/README.md#qualitativer-benchmark)
über acht echte Prompts liefert 7B dennoch in fünf von acht Fällen Wort für
Wort denselben Text wie BF16, bei 73,8 % deckungsgleichen Token. Das ist eine
Gütezahl, kein Zielwert: 8/8 wäre kein Erfolg, sondern der Hinweis, dass die
Quantisierung wirkungslos ist. Details im
[Whitepaper (Kap. 6.9)](README/Whitepaper/myelith-whitepaper-v0.3.md) und in
[INTEGER_LLM](INTEGER_LLM/README/README.md).

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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exakte Ganzzahl-Inferenz (Rust + Python) | **Akzeptanzkriterium ≤ 5 % auf beiden Modellen erreicht:** 0,5B +2,11 %, 7B **+1,14 %** (vorher +377 %). Das sind 0,30 Punkte über dem Boden des Quantisierungsschemas selbst (+0,84 %). NEON-Backend **+27 % / +43 %** bei bitgleicher Ausgabe, 30/30 Konformitätsvektoren auf beiden Backends. Das [Skalenpaket](INTEGER_LLM/scale_packs/README.md) macht den Artefaktbau plattformübergreifend bitgleich: 1,8 MB statt 8,8 GB, und aus 20 Minuten werden 40 Sekunden |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | Kern-Datentypen, Kryptografie (VRF, BLS, Merkle, Erasure) | Phase 2 abgeschlossen. BLS mit Proof-of-Possession gegen Rogue-Key-Angriffe, mit ausführbarer Regression; Erasure-Codierung über GF(2⁸) in Cauchy-Form, geprüft über **alle 495** Teilmengen von 8 aus 12 |
| [NETWORKING](NETWORKING/README/README.md) | P2P-Gossip, Peer-Discovery, Latenztopologie | Phase 2 abgeschlossen: Paarlatenzmessung, LatencyGraph, Geo- und AS-Diversität |
| [CONSENSUS](CONSENSUS/README/README.md) | Ledger, BFT, Slashing | **Alle vier Phasen abgeschlossen.** Signiertes, stimmgewichtetes BFT mit VRF-rotierender Komiteewahl, Double-Signing-Beweis und Rundenwechsel mit Sperrmechanik, also Safety **und** Liveness, geprüft über eine Akzeptanz-Testmatrix mit 21 simulierten Validatoren. Dazu PoI-Bündel-Einreichung, Epochenabschluss und Datenverfügbarkeit (Reed-Solomon k=8/m=4 über die Streitfrist) |
| [TOKENOMICS](TOKENOMICS/README/README.md) | Burn-and-Mint, Verteilung | Phase 2 abgeschlossen: Prägefunktion, Verteilungsschlüssel und Credit-Preisbildung mit eingefrorener exp()-LUT, vollständig ganzzahlig |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | Pod-Orchestrierung über echtes Netz | Phase 2.1 abgeschlossen: Micro-Batching und Pipelining. **Pipeline-Determinismus wieder bitgleich mit dem Einzelknoten,** seit der verlustbehaftete Boundary-Schritt zwischen den Stages entfallen ist (Funde 20/26). Die Spur bindet damit wieder die übertragene Aktivierung |
| [VERIFICATION](VERIFICATION/README/README.md) | Redundanzvergleich, Bisektions-Spiel | Phase 2 abgeschlossen: Bisektion in O(log L), On-Chain-Schiedsrunde über den `ShardExecutor`-Trait, Slash-Entscheidung getrennt von den Beträgen. Stufe 3 (zkML-Anker) ist Aufrüstpfad und nicht begonnen |
| [TESTCLIENT](TESTCLIENT/README/README.md) | Terminal-Testclient: Hardwaretests, geshardete Inferenz | Phase 1 abgeschlossen. Alle Starter liegen in `TESTCLIENT/`: App-Bündel für macOS, `.cmd` für Windows, Shell-Skript fürs Terminal. Sie finden die Repository-Wurzel selbst und funktionieren auch verschoben. Sucht Artefakte selbst, lässt bei mehreren wählen und baut sie sonst aus HF-Gewichten samt Skalenpaket. Dabei prüft er den Digest und sagt bei Abweichung ausdrücklich, dass dies **kein** Hardware-Befund ist. Phase 2 wartet auf heterogene Hardware |
| [TRAINING](TRAINING/README/README.md) | Datenprovenienz, robuste Aggregation | Ein einziger Fahrplanpunkt: die Messung, ob das Quantisierungsschema im Rückwärtspass trägt. Der Fahrplan entsteht erst nach ihrem Ergebnis, denn die bisherigen 22 Punkte ruhten auf einer ungeprüften Annahme |
| [ETHICS](ETHICS/README/README.md) | Ethische und rechtliche Standards, Manifest | Manifest v1.0.0 steht, Fahrplan steht, Design-Entscheidungen offen |
| [GOVERNANCE](GOVERNANCE/README/README.md) | Parameter-Registry, Modell-Updates | Planungsphase. Krypto-Agilität für die Post-Quantum-Migration ist verankert, die übrigen Design-Entscheidungen sind offen |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | Session-Kontrakte, Dual-LLM-Trennung | Planungsphase, blockiert durch die vorgelagerten Schichten |
| [CLIENT](CLIENT/README/README.md) | Nutzer-Client inkl. Wallet | Konzeptphase |

## Lizenz

[PolyForm Shield License 1.0.0](LICENSE.md). Nutzung, Veränderung und
kommerzielle Teilnahme am Myelith-Netzwerk (Mining, Validierung, Gateways,
Clients) sind erlaubt; ein konkurrierendes Netzwerk oder Produkt auf Basis
des Codes zu betreiben ist es nicht.