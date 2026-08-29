# Prüflisten je Komponente (Punkt 1.5)

Was erfüllt sein muss, bevor eine Komponente eine Phase als
abgeschlossen führt. **Ergänzung zu den technischen Akzeptanzkriterien
der Komponenten, kein Ersatz.**

## ⚑ Warum es diese Listen gibt, obwohl es Akzeptanzkriterien gibt

Die technischen Kriterien fragen: *Rechnet es richtig?* Diese Listen
fragen: *Dürfen wir es so betreiben, und wissen die Betroffenen, woran
sie sind?* Beides fällt auseinander. Ein Modul kann bitgleich rechnen
und trotzdem eine Lizenz verletzen oder eine Vertraulichkeit
suggerieren, die es nicht gibt.

## ⚑ Und warum sie nicht automatisch geprüft werden

Drei der Punkte lassen sich als Skript prüfen und werden es auch
(`lizenzprobe.py`, `modellkarte.py --pruefe`, `pruefe_antrag.py`). Der
Rest nicht: „Die Risikoklasse ist dem Nutzer offengelegt" ist keine
Eigenschaft einer Datei, sondern eines Bildschirms.

**Das wird hier ausgeschrieben, statt es zu verschleiern.** Eine Liste,
die vorgibt, maschinell zu sein, und es nicht ist, ist schlechter als
eine, die sich als das ausgibt, was sie ist: **eine Frage, die ein
Mensch vor dem Haken beantworten muss.**

## Für jede Komponente

| Frage | Grundsatz | Prüfbar |
|---|---|---|
| Sind alle benutzten Modelle und Bibliotheken lizenzgeprüft? | G7 | ✅ `lizenzprobe.py` |
| Behauptet die Komponente irgendwo eine Vertraulichkeit, die das Protokoll nicht leistet? | G6, Kap. 9.3 | ❌ Mensch |
| Steht in ihrer Dokumentation, was sie **nicht** leistet? | Manifest Abschnitt 4 | ❌ Mensch |
| Sind neue Governance-Parameter mit Rang und Fundstelle eingetragen? | S1–S5 | ✅ `myl-governance` |

## Zusätzlich je Komponente

Aus Abschnitt 6 des Manifests:

| Komponente | Zusätzlich zu prüfen | Prüfbar |
|---|---|---|
| **INTEGER_LLM** | Quantisierung reproduzierbar und dokumentiert (S2); Lizenz der **konkreten Variante** vor jedem Wechsel (G7); Modellkarte je θ_v-Fassung | ✅ `lizenzprobe.py`, `modellkarte.py --pruefe` |
| **TRAINING** | Provenienzpflicht ohne Ausnahme (G2); vollständiger Aufnahmeantrag je Korpus (G3); **keine inhaltliche Bewertung** (G1) | ✅ `pruefe_antrag.py` für G3, ❌ Mensch für G1 |
| **AGENT_LAYER** | Kontraktgrenzen außerhalb des Modellkontexts (G5); architektonische Trennung; Risikoklasse gegenüber dem Nutzer offengelegt (G6) | ⬤ teils: die Grenzen sind Code, die Offenlegung ist Bildschirm |
| **GOVERNANCE** | G3 und S1–S5 in der Parameter-Registry verankert; Verfassungsrang für das nicht Verhandelbare | ✅ `tests/akzeptanz.rs` |
| **CLIENT** | Risikoklassen-Anzeige **vor** der Nutzung (G6); keine Vertraulichkeitsbehauptung | ❌ Mensch, Quelle ist `../Risikoklassen.toml` |
| **VERIFICATION** | Trägt G4: ohne Verifikation keine Nachvollziehbarkeit | ✅ eigene Akzeptanzkriterien |

## ⚑ Der eine Punkt, der leicht übersehen wird

**„Keine inhaltliche Bewertung" (G1) ist eine Verbotsnorm, und
Verbotsnormen fallen bei Prüfungen durch das Raster:** Man sieht, was
da ist, nicht, was jemand hinzugefügt hat, das nicht da sein darf. Wer
ein Feld für „Qualität" oder „Angemessenheit" in einen Aufnahmeantrag
schreibt, hebt G1 auf, und keine Vollständigkeitsprüfung merkt es.

Diese Zeile steht hier, damit wenigstens ein Mensch danach sucht.
