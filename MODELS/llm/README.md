# MODELS/llm/

Ablageort für die Quellmodelle, aus denen die θ_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Die Gewichte werden nicht versioniert (siehe `MODELS/.gitignore`); nur
dieses README, `KATALOG.json` und die Lizenzdatei je Modell bleiben im
Repository. Was hier gilt und welche Rubriken es noch gibt, steht eine
Ebene höher in `MODELS/README.md`.

> **Diese Datei wird erzeugt.** Quelle sind `MODELS/llm/KATALOG.json`
> (kuratiert: Herkunft, Revision, Lizenz, Status) und
> `INTEGER_LLM/scale_packs/REGISTER.json` (erzeugt: Digest, θ_v).
> Änderungen gehören in eine der beiden Dateien, danach
> `python INTEGER_LLM/tools/modelle_liste.py`.

Jede Variante braucht eine **eigene Lizenzprüfung** (Whitepaper Kap. 10.1,
ETHICS-Grundsatz G7: Apache 2.0 oder MIT) und eine **fixierte Revision**:
ohne beides ist der Lauf weder zulässig noch reproduzierbar. Es werden
ausschließlich **Basis-Varianten** verwendet, keine Instruct-Varianten
(Scope-Entscheidung 12.15).


## Modelle

| Modell | Hugging Face | Revision | Lizenz (Gewichte) | Lizenz (Artefakt) | Parameter | Layer | Gewichte | Artefakt | θ_v | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| `myelith-0.6b` | [Qwen/Qwen3-0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) | `c1899de289a0…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 0,6 Mrd. | 28 | rund 1,5 GB | 0,92 GB | 0.22.0 | verifiziert |
| `myelith-30b-a3b` | [Qwen/Qwen3-30B-A3B](https://huggingface.co/Qwen/Qwen3-30B-A3B) | `ad44e777bcd1…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 30,5 Mrd. gesamt, 3,04 Mrd. aktiv je Token | 48 | rund 57 GB | 31,2 GB | 0.22.0 | verifiziert |
| `myelith-35b-a3b` | [Qwen/Qwen3.6-35B-A3B](https://huggingface.co/Qwen/Qwen3.6-35B-A3B) | `995ad96eacd9…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 35 Mrd. gesamt, rund 3 Mrd. aktiv je Token | 40 | rund 67 GB | rund 32 GB | 0.22.0 | erprobt |
| `myelith-4b` | [Qwen/Qwen3-4B](https://huggingface.co/Qwen/Qwen3-4B) | `1cfa9a720891…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 4 Mrd. | 36 | rund 7,5 GB | 4,8 GB | 0.22.0 | verifiziert |
| `myelith-8b` | [Qwen/Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) | `b968826d9c46…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 8,2 Mrd. | 36 | rund 15 GB | 9,5 GB | 0.22.0 | verifiziert |

**Status:**

- **verifiziert**: Artefakte gebaut, Perplexität gegen die Gleitkomma-Referenz gemessen, Akzeptanzkriterium erfüllt, Skalenpaket im Repository.
- **erprobt**: Artefakte gebaut und lauffähig, Qualität noch nicht gegen die Referenz gemessen.
- **vorgemerkt**: Lizenz geprüft und Revision festgelegt, aber noch nicht geholt oder gebaut.
- **geholt**: Gewichte liegen lokal mit fixierter Revision, Artefakt noch nicht gebaut. ⚑ Dieser Wert ist am 2026-09-21 dazugekommen, weil keiner der drei anderen den Zustand traf: `vorgemerkt` heisst ausdruecklich "noch nicht geholt", `erprobt` verlangt ein gebautes Artefakt. Ein Zustand ohne Wort wird sonst auf den naechstbesten gebucht, und dann meldet der Katalog etwas Falsches.

**Gemessene Qualität** (Perplexität, WikiText-2):

- `myelith-0.6b`: ⚠️ Ueber 3558 Positionen (32 Sequenzen) gemessen: 43,49 gegen BF16 42,26, also +2,92 %. Mit den Skalen bis zum 2026-09-21 waren es 45,15 gegen 42,26, also +6,84 % und damit VERFEHLT. Ueber die bisher uebliche kleine Stichprobe von 435 Positionen ergaben dieselben Artefakte +4,48 % beziehungsweise -1,64 %; vier Sequenzen tragen die Aussage also nicht. ⛔️ **Hier stand bis dahin 33,29 (+4,48 %)**, und das war ein Artefakt eines veralteten Skalenpakets: 105 von 422 Skalen wichen von einer Neuberechnung ab, alle nach oben. Die alte Messung stammt vom 2026-09-14 mit theta_v 0.19.0 ueber 435 Positionen aus vier WikiText-2-Sequenzen. ⚑ Der groesste Abstand der Reihe. ⛔️ **Hier stand bis zum 2026-09-21 die Richtung „je kleiner das Modell, desto teurer die Quantisierung“, und das 8B hat sie gebrochen**: +3,75 % bei 8,2 Mrd. gegen +1,65 % bei 4 Mrd. ⚑ **Eine Erklaerung liegt nahe und ist nicht geprueft:** Die Reihe mischt zwei Achsen, denn 0,6B und 4B haben eine gebundene Einbettung und 8B und 30B nicht. Innerhalb jeder Gruppe faellt der Abstand monoton. ⚠️ Bei 435 Positionen ist ein Unterschied dieser Groesse ausserdem nicht sicher aufloesbar. Kriterium <= 5 % erfuellt, aber von allen vier Modellen am knappsten
- `myelith-30b-a3b`: 10,42 gegen BF16 10,48 (-0,59 %). Das Vorzeichen ist kein Beleg fuer Ueberlegenheit: Zwei der vier Sequenzen sind besser, zwei schlechter, und der Standardfehler des Mittels betraegt 1,66 %. Bei 435 Positionen ist kein Unterschied auflösbar; das Kriterium (<= 5 %) ist mit weitem Abstand erfuellt
- `myelith-35b-a3b`: 9060,93 gegen BF16 8613,28 (+5,20 %), gemessen am 2026-09-22 ueber 16 Sequenzen aus WikiText-2 mit 1683 ausgewerteten Positionen. ⚠️ Damit liegt es KNAPP UEBER dem Akzeptanzkriterium von 5 Prozent, deshalb 'erprobt' und nicht 'verifiziert'. Bei 1683 Positionen ist der Abstand nicht gesichert; die Messung gehoert ueber 32 Sequenzen wiederholt. ⚑ Die Referenz ist hier NICHT baseline.py, denn die laedt bewusst ohne Auslagerung und 67 GB passen nicht in 24 GiB; gemessen wurde gegen das offizielle Modell mit ausdruecklicher Auslagerung auf Platte, auf denselben Sequenzen.
- `myelith-4b`: 19,95 gegen BF16 19,63 (+1,65 %)
- `myelith-8b`: 13,27 gegen BF16 12,79 (+3,75 %)

**Anmerkungen:**

- `myelith-0.6b`: Der kleinste Vertreter und der Anker des Projekts (2026-09-11). Er loest Qwen2.5-0,5B ab; damit liegt die kleinste Groesse in derselben Familie wie 4B und 14B, und die Reihe misst eine Achse (Groesse) statt zweier (Groesse und Familie). 📌 Fund 336: Dieses Modell traegt in der letzten Ebene ein post_attention_layernorm-Gewicht mit dem Betrag 192, waehrend der Median derselben Zeile bei 3,1 liegt. int8 reicht bis 127. Die Kalibrierung verschiebt den Kanal deshalb in die Folgematrizen (gamma/2, Spalte*2); im Gleitkomma ist das bitgleich, geprueft an den Logits. Es ist das erste Modell des Projekts, das die Umformung braucht.
- `myelith-30b-a3b`: Das erste Mixture-of-Experts-Modell des Projekts: 128 Experten je Layer, Top-8, alle 48 Layer sind MoE (mlp_only_layers ist leer). Kalibriert am 2026-08-25 auf einer 24-GiB-Maschine, obwohl das bf16-Modell 56,9 GiB und das Artefakt 29 GiB gross ist: Die Gewichte werden eingeblendet statt kopiert, und Quantisierung wie Export laufen im Strom. Artefakt: 18 868 Tensoren in 37 747 Dateien. Belegt: Fortsetzung von 'Die Hauptstadt von Frankreich ist' lautet ' Paris. Die Hauptstadt', Token-Hash 99bfc1f64e901811 ueber zwei unabhaengige Laeufe gleich. Perplexitaet am 2026-08-25 gemessen; Einordnung siehe eval/results/. Nach Gesamtparametern setzt das Modell die Reihe fort, in der der Abstand mit der Groesse schrumpft (0,5B +2,11 %, 4B +1,64 %, 7B +1,14 %); nach AKTIVEN Parametern (3,0 Mrd.) tut es das nicht. Welche der beiden Groessen massgeblich ist, ist offen.
- `myelith-35b-a3b`: Das erste hybride Modell des Projekts: 30 der 40 Ebenen sind rekurrente Zustandsschichten (Gated DeltaNet), 10 sind Achtsamkeit mit Ausgangstor und Teildrehung (rotary_dim 64 von 256). Gemisch mit 256 Experten, Top-8, dazu ein geteilter Experte, der bei jedem Token feuert. ⚑ Es ist ein DENKMODELL: Seine Vorlage oeffnet den Denkblock in der Aufforderung (`<|im_start|>assistant\n<think>\n`), anders als Qwen3, das `<think>` selbst schreibt. Der Client bedient das ueber die Vorlage `ChatMlDenkblock`. ⛔️ Der Bau hat zehn Funde gekostet (417 bis 430). Der entscheidende war 423: `Qwen3_5MoeRMSNorm` rechnet `x * (1 + weight)` mit einem auf null angelegten Gewicht, und der Export nahm das Gewicht so, wie es dastand. Belegt gegen das offizielle Modell: Ebene 0 fiel damit von rel.L2 0,9665 auf 0,0017, und die Top-8 der Logits stimmen Token fuer Token ueberein (' Paris' an eins fuer 'The capital of France is'). Durchsatz 8,8 Token/s Dekodierung auf Apple M5 Pro mit 24 GiB.
- `myelith-4b`: Die erste Qwen3-Variante des Projekts und der Traeger von QK-Norm. Drei Unterschiede zu Qwen2.5, von denen nur einer vorher benannt war: QK-Norm (Q und K je Kopf normiert, vor RoPE), keine Attention-Biases, und head_dim 128 bei hidden_size/num_heads = 80 (Fund 59). Status 'erprobt', nicht 'verifiziert': Das Artefakt laeuft und ist bitgleich ueber Laeufe, der Perplexitaetsabstand ist noch offen.
- `myelith-8b`: Das mittlere dichte Modell, geholt am 2026-09-21 auf Wunsch des Projektinhabers. ⚑ Ein reiner Groessenwechsel gegenueber dem 4B und keine Architekturaenderung: dieselbe Bauart (Qwen3ForCausalLM), 399 Tensoren mit demselben Muster, 36 Ebenen, QK-Norm vorhanden (36 q_norm und 36 k_norm gezaehlt), keine Bias-Tensoren. ⚠️ tie_word_embeddings ist hier False, anders als beim 0,6B und 4B: Das Modell traegt ein eigenes lm_head.weight, also greift Fund 390 (die doppelt abgelegte Matrix) hier nicht. ⚑ Es fuellt die Luecke zwischen 4B und dem Gemisch und macht die dichte Reihe wieder dreipunktig, die seit dem Wegfall des 14B nur zwei Punkte hatte. ⚑ **Artefakt gebaut am 2026-09-21** in 10 min 4 s ohne Skalenpaket; mit dem daraus erzeugten Paket baut es in **36 s und bitgleich**, damit ist der Bau plattformuebergreifend wiederholbar. Laedt und rechnet: 36 Layer-Vektoren und 3 E2E-Vektoren in 6,6 s. ⚑ **Perplexitaet gemessen am 2026-09-21:** 13,27 gegen die BF16-Referenz 12,79, also **+3,75 %** bei einem Kriterium von 5 %, AKZEPTIERT. ⛔️ **Und die Zahl bricht die Reihe:** Das Projekt haelt fest, je kleiner das Modell, desto teurer die Quantisierung (0,6B +4,48 %, 4B +1,65 %). Das 8B liegt mit +3,75 % SCHLECHTER als das kleinere 4B. ⚑ **Eine Erklaerung liegt nahe und ist nicht geprueft:** Die Reihe mischt zwei Achsen. 0,6B und 4B haben eine gebundene Einbettung, 8B und 30B nicht. Innerhalb der gebundenen faellt der Abstand monoton, innerhalb der ungebundenen auch (8B +3,75 %, 30B kein messbarer Abstand). ⚠️ **Bei 435 Positionen und vier Sequenzen ist ein Unterschied dieser Groesse ausserdem nicht sicher aufloesbar**; ein Lauf ueber 128 Sequenzen wuerde es entscheiden.

## Woher die Gewichte kommen

Der Testclient holt sie selbst, wenn er sie braucht: Menüpunkt
**[4] Artefakt wählen** oder beim ersten Lauf, der ein Modell benötigt.
Von Hand geht es auch:

```bash
huggingface-cli download <hf_repo> --revision <hf_revision> \
    --local-dir MODELS/llm/<hf_verzeichnis>
```

## Wie daraus Artefakte werden

```bash
cd INTEGER_LLM
INTEGER_LLM_MODEL=<modell> python -m calibrate.src.main
```

Der Bau nutzt das versionierte Skalenpaket aus `scale_packs/<modell>/` und
ist damit **plattformübergreifend bitgleich**: Die Aktivierungsstatistik,
der einzige nichtdeterministische Schritt (Fund 32), entfällt. Er dauert
Sekunden statt Minuten.

## Zur Lizenzangabe

**Es sind zwei Spalten, und sie gelten für verschiedene Dinge.**
„Lizenz (Gewichte)" nennt, was die jeweilige Modellkarte für die
heruntergeladenen Grundgewichte angibt, ohne eigene Rechtsprüfung.
„Lizenz (Artefakt)" nennt die Lizenz des daraus **gebauten** Artefakts,
und das ist die dieses Repositoriums: Was ausgeliefert wird, ist eine
Bearbeitung nach dem Verfahren dieses Projekts, mit eigenen Skalen und
Nachschlagetabellen.

📌 **Hier stand bis zum 2026-09-10 eine einzelne Spalte „Lizenz"**, und
sie stand in einer Zeile, deren erste Spalte `myelith-4b` heisst. Das
las sich, als stuende das Artefakt unter Apache-2.0. **Eine Angabe ist
nicht dadurch richtig, dass sie stimmt, sondern dadurch, dass sie sich
auf das bezieht, wonebendran sie steht.**

**Alle sieben Qwen2.5-Größen wurden am 2026-08-23 geprüft, zwei fallen
durch:** 3B steht unter der Qwen Research License („FOR NON-COMMERCIAL
PURPOSES ONLY"), 72B unter der Qwen License mit einer Lizenzpflicht ab
100 Mio. monatlich aktiven Nutzern. Beides ist mit ETHICS-Grundsatz G7
unvereinbar. Geeignet sind 0.5B, 1.5B, 7B, 14B und 32B; die nächste
Größe nach 7B ist damit **14B, nicht 72B**. Vollständige Prüfung samt
Methode in `COMPLIANCE/ethics/Lizenzlage.md`.

Für die Apache-2.0-Varianten ist auch die Lage **quantisierter
Ableitungen** geklärt: §2 erlaubt Bearbeitungen, §4(a) und §4(b) binden
sie an eine Lizenzkopie und an die Kennzeichnung geänderter Dateien, und
eine `NOTICE`-Datei, die §4(d) auslösen würde, enthalten die
Qwen2.5-Repositorien nicht. Das ist eine Lesart des Lizenztextes und
ersetzt vor einem Genesis-Block keine Prüfung durch jemanden, der dafür
haftet.
