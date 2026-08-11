# Simulationen zu Myelith v0.3

Die Modellrechnungen aus Anhang B des Whitepapers als ausführbare Programme.
Sie benötigen keine Abhängigkeiten über die Python-Standardbibliothek hinaus.

    python3 <programm>.py

Zuordnung zu den Anhangsabschnitten: siehe Anhang B.9 des Whitepapers.

| Programm | Gegenstand |
|---|---|
| `tokenomics_sim.py` | Burn-and-Mint-Gleichgewicht, Self-Dealing |
| `security_sim.py` | Sicherheitsbedingung und Anreizrechnung |
| `tau_sim.py` | Erforderliche Trennschärfe eines Toleranzverfahrens |
| `robustness_sim.py` | Empfindlichkeit gegenüber verletzten Verteilungsannahmen |
| `hardware_noise_sim.py` | Rauschpegel gängiger Beschleunigerklassen |
| `accum_alternatives_sim.py` | Zwischenlösungen bei der Akkumulationsgenauigkeit |
| `topk_stability_sim.py` | Strukturbasierte Commitments und adaptiver Angriff |
| `integer_determinism_sim.py` | Assoziativität, Überlaufreserve, Divisionssemantik |
| `integer_training_sim.py` | Determinismus des Rückwärtspasses, Block-Skalierung |
| `training_capacity_sim.py` | Trainingsdurchsatz, Datenprovenienz, Auswahl-Poisoning |
| `training_integrity_sim.py` | Robuste Aggregation, veraltete Gradienten, Vergessen |
| `model_growth_sim.py` | Kosten und Zeitskala von Wachstumsschritten |
| `training_tokenomics_sim.py` | Training und Burn-and-Mint-Kreislauf |
| `genesis_supply_sim.py` | Anlaufphase, Emissionsverlauf, Frühphasen-Konzentration |
| `latency_sim.py` | Pod-Latenz gegen lokale Kollusionsdichte (geplant) |

Die Referenzimplementierung des Protokolls wird getrennt entwickelt und ist
nicht Teil dieses Pakets.
