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
Verweisen auf die jeweilige Implementierung.

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
Bei 7B lag dieser Wert vor den Fehlersuchen bei **+377 %**
(Perplexität 41,42), heute bei **+1,1 %** und damit **0,3 Prozentpunkte
über dem theoretischen Boden des Quantisierungsschemas** (+0,84 %,
unabhängig gemessen).

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
| [INTEGER_LLM](INTEGER_LLM/README/README.md) | bit-exakte Ganzzahl-Inferenz und, neu, der ganzzahlige Rückwärtspass (Rust + Python) | **Akzeptanzkriterium ≤ 5 % auf beiden Modellen erreicht:** 0,5B +2,11 %, 7B **+1,14 %** (vorher +377 %). Das sind 0,30 Punkte über dem Boden des Quantisierungsschemas selbst (+0,84 %). Durchsatz zuletzt **+29 % / +419 %** (0,5B / 7B), weil die Zeilen jetzt über Threads verteilt werden; **bei 7B ist der Integerpfad damit schneller als bf16** auf derselben Maschine. Davor +52 % / +40 % durch den Wegfall einer Gewichtskopie je Token und +27 % / +43 % durch NEON. Beides bei bitgleicher Ausgabe, 30/30 Konformitätsvektoren auf beiden Backends. Das [Skalenpaket](INTEGER_LLM/scale_packs/README.md) macht den Artefaktbau plattformübergreifend bitgleich: 1,8 MB statt 8,8 GB, und aus 20 Minuten werden 40 Sekunden |
| [SHARED_TYPES](SHARED_TYPES/README/README.md) | Kern-Datentypen, Kryptografie (VRF, BLS, Merkle, Erasure) | Phase 2 abgeschlossen. BLS mit Proof-of-Possession gegen Rogue-Key-Angriffe, mit ausführbarer Regression; Erasure-Codierung über GF(2⁸) in Cauchy-Form, geprüft über **alle 495** Teilmengen von 8 aus 12 |
| [NETWORKING](NETWORKING/README/README.md) | P2P-Gossip, Peer-Discovery, Latenztopologie | Phase 2 abgeschlossen: Paarlatenzmessung, LatencyGraph, Geo- und AS-Diversität |
| [CONSENSUS](CONSENSUS/README/README.md) | Ledger, BFT, Slashing | **Alle vier Phasen abgeschlossen.** Signiertes, stimmgewichtetes BFT mit VRF-rotierender Komiteewahl, Double-Signing-Beweis und Rundenwechsel mit Sperrmechanik, also Safety **und** Liveness, geprüft über eine Akzeptanz-Testmatrix mit 21 simulierten Validatoren. Dazu PoI-Bündel-Einreichung, Epochenabschluss und Datenverfügbarkeit (Reed-Solomon k=8/m=4 über die Streitfrist). Seit v0.10.0 ist der **Arbeitsanteil des Stimmgewichts kalibriert und gedeckelt**: Sein bisheriger Bezugswert entsprach dem Vorwärtspass eines einzigen Tokens, womit eine Stunde Arbeit den Stake um das Tausendfache gehoben hätte |
| [TOKENOMICS](TOKENOMICS/README/README.md) | Burn-and-Mint, Verteilung | Phase 2 abgeschlossen: Prägefunktion, Verteilungsschlüssel und Credit-Preisbildung mit eingefrorener exp()-LUT, vollständig ganzzahlig. Seit v0.4.0 liegt die wirtschaftliche Überschlagsrechnung vor (K8). Sie stand zuerst bei 3,6× bis 9,2× gegenüber einem zentralen Anbieter und führte damit auf einen Befund: Der Integerpfad lief **einkernig**, die Vergleichsseite nicht. Nach der Zeilen-Parallelisierung kostet das Netz je Token **3,2× (0,5B) und 1,9× (7B)**, und davon ist bei 7B fast alles Redundanz. Seit v0.3.0 ist außerdem festgelegt, **wie ein Shard zu seiner Gutschrift kommt**: nach seinem Anteil an der Gewichtsarbeit eines Vorwärtspasses, nicht nach Layern, weil der LM-Kopf bei 0,5B über neun Layer wiegt. Damit verteilen Zuschnitte von 1 bis 28 Shards dieselbe Summe |
| [COMPUTE_PIPELINE](COMPUTE_PIPELINE/README/README.md) | Pod-Orchestrierung über echtes Netz | Phase 2.1 abgeschlossen: Micro-Batching und Pipelining. **Pipeline-Determinismus wieder bitgleich mit dem Einzelknoten,** seit der verlustbehaftete Boundary-Schritt zwischen den Stages entfallen ist (Funde 20/26). Die Spur bindet damit wieder die übertragene Aktivierung. Seit v0.3.0 ist „bitgleich" hier über die **gerechneten Zahlen** belegt und nicht mehr über die erzeugten Token: Der Pod gibt einen Digest über Logits und Token heraus, und er stimmt bei 1 bis 24 Shards mit dem Einzelknoten überein (Fund 36). Seit v0.5.0 trägt die **Spur einen Eintrag je Layer statt je Shard**: Ihre Länge hängt damit am Modell und nicht mehr am Zuschnitt, zwei Pods mit verschiedener Knotenzahl sind vergleichbar, und die Bisektion grenzt die fehlerhafte Layer ein statt der Layer-Gruppe |
| [VERIFICATION](VERIFICATION/README/README.md) | Redundanzvergleich, Bisektions-Spiel | Phase 2 abgeschlossen: Bisektion in O(log L), On-Chain-Schiedsrunde über den `ShardExecutor`-Trait, Slash-Entscheidung getrennt von den Beträgen. Stufe 3 (zkML-Anker) ist Aufrüstpfad und nicht begonnen |
| [TESTCLIENT](TESTCLIENT/README/README.md) | Terminal-Testclient: Hardwaretests, geshardete Inferenz, Auswertung | Phase 1 und **Phase 3** abgeschlossen, dazu Punkt 2.1 und 2.4. `vergleich` stellt die Protokolle mehrerer Maschinen gegenüber und fällt das Urteil, und **verweigert** es, wenn alle von derselben Maschine stammen, wenn ein Lauf abgebrochen ist oder wenn zwei Läufe verschiedene Dinge gemessen haben. Der Vergleichswert deckt seit v0.8.0 die **gerechneten Zahlen** ab und nicht nur die erzeugten Token (Fund 36). Testpläne sind nicht mehr an ein Modell gebunden; ein kuratierter Modellkatalog nennt Herkunft, Revision und Lizenz. Seit v0.11.0 begleitet der Client auch Modellwechsel: `--erwarte` lässt einen Lauf fehlschlagen, der einen anderen Vergleichswert erzeugt, und `modellstaende` beantwortet in einem Aufruf, welche Werte ein θ_v-Wechsel verändert hat und welche nicht. Der Nachweis selbst wartet weiter auf heterogene Hardware |
| [TRAINING](TRAINING/README/README.md) | Datenprovenienz, robuste Aggregation | **Die eine Messung ist gemacht (2026-08-22): Es trägt**, mit stochastischem Runden der Gewichte. Volles Ganzzahlschema gegen Gleitkomma auf zurückgehaltenem Text: **+0,67 %** (Kriterium ≤ 10 %); mit Rundung zur nächsten Stufe dagegen +29,9 %, weil ein SGD-Schritt im Median nur 6,4e-6 einer Rasterstufe groß ist. Der Zufall kostet keinen Determinismus: Der Würfel ist eine Funktion über (Ebene, Schritt, Index), kein Zustand. Und der Trainingsschritt kommt **ganz ohne Gleitkommazustand** aus: ganzzahliger Master, exakte Ganzzahladdition, +0,75 %. Wachstum ist **exakt funktionserhaltend** (Abweichung 0,00e+00 durch ganzzahlige Aufteilung statt Halbierung), die Symmetrie der Kopien bricht dabei ohne künstliches Rauschen. Konzept und Fahrplan stehen. **Seit v0.1.0 hat die Komponente auch Code:** die Datenprovenienz aus Kap. 7.3, also über eine Merkle-Wurzel verankerte Korpora, Segmentreferenz per Beweis statt per Rohdaten und eine Zuweisung, die aus dem Epochen-Seed folgt statt aus der Wahl des Miners. Seit v0.2.0 dazu der **Wachstumsoperator**: eine ganzzahlige Aufteilung statt der Halbierung aus der Literatur, exakt funktionserhaltend und über einen Digest geprüft statt über eine Toleranz. Das Bitbudget ist über vier Lernraten gerechnet; die Grenze, ab der der Master 64 statt 32 Bit braucht, liegt zwischen 1e-4 und 1e-5 |
| [ETHICS](ETHICS/README/README.md) | Ethische und rechtliche Standards, Manifest | Manifest v1.0.0 steht, Fahrplan steht, Design-Entscheidungen offen. **Grundsatz G7 (freie Nachnutzbarkeit des Basismodells) ist für alle sieben Qwen2.5-Größen geprüft:** fünf sind Apache 2.0, 3B und 72B fallen durch, und die 72B-Klausel ab 100 Mio. monatlich aktiven Nutzern ist für ein offenes Protokoll strukturell nicht erfüllbar |
| [GOVERNANCE](GOVERNANCE/README/README.md) | Parameter-Registry, Modell-Updates | Planungsphase. Krypto-Agilität für die Post-Quantum-Migration ist verankert, die übrigen Design-Entscheidungen sind offen |
| [AGENT_LAYER](AGENT_LAYER/README/README.md) | Session-Kontrakte, Dual-LLM-Trennung | Planungsphase, blockiert durch die vorgelagerten Schichten |
| [CLIENT](CLIENT/README/README.md) | Nutzer-Client inkl. Wallet | Konzeptphase |

## Lizenz

[PolyForm Shield License 1.0.0](LICENSE.md). Nutzung, Veränderung und
kommerzielle Teilnahme am Myelith-Netzwerk (Mining, Validierung, Gateways,
Clients) sind erlaubt; ein konkurrierendes Netzwerk oder Produkt auf Basis
des Codes zu betreiben ist es nicht.