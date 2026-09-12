# models/

Ablageort für das Quellmodell, aus dem die θ_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Der Inhalt wird nicht versioniert (siehe `.gitignore`); nur dieses README,
`KATALOG.json` und die `.gitignore` bleiben im Repository.

> **Diese Datei wird erzeugt.** Quelle sind `models/KATALOG.json`
> (kuratiert: Herkunft, Revision, Lizenz, Status) und
> `scale_packs/REGISTER.json` (erzeugt: Digest, θ_v). Änderungen gehören
> in eine der beiden Dateien, danach `python tools/modelle_liste.py`.

Jede Variante braucht eine **eigene Lizenzprüfung** (Whitepaper Kap. 10.1,
ETHICS-Grundsatz G7: Apache 2.0 oder MIT) und eine **fixierte Revision**:
ohne beides ist der Lauf weder zulässig noch reproduzierbar. Es werden
ausschließlich **Basis-Varianten** verwendet, keine Instruct-Varianten
(Scope-Entscheidung 12.15).


## Modelle

| Modell | Hugging Face | Revision | Lizenz (Gewichte) | Lizenz (Artefakt) | Parameter | Layer | Gewichte | Artefakt | θ_v | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| `myelith-0.6b` | [Qwen/Qwen3-0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) | `c1899de289a0…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 0,6 Mrd. | 28 | rund 1,5 GB | 0,92 GB | 0.18.0 | verifiziert |
| `myelith-30b-a3b` | [Qwen/Qwen3-30B-A3B](https://huggingface.co/Qwen/Qwen3-30B-A3B) | `ad44e777bcd1…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 30,5 Mrd. gesamt, 3,04 Mrd. aktiv je Token | 48 | rund 57 GB | 31,2 GB | 0.18.0 | verifiziert |
| `myelith-4b` | [Qwen/Qwen3-4B](https://huggingface.co/Qwen/Qwen3-4B) | `1cfa9a720891…` | Apache-2.0 | PolyForm Shield License 1.0.0 | 4 Mrd. | 36 | rund 7,5 GB | 4,8 GB | 0.18.0 | verifiziert |

**Status:**

- **verifiziert**: Artefakte gebaut, Perplexität gegen die Gleitkomma-Referenz gemessen, Akzeptanzkriterium erfüllt, Skalenpaket im Repository.
- **erprobt**: Artefakte gebaut und lauffähig, Qualität noch nicht gegen die Referenz gemessen.
- **vorgemerkt**: Lizenz geprüft und Revision festgelegt, aber noch nicht geholt oder gebaut.

**Gemessene Qualität** (Perplexität, WikiText-2):

- `myelith-0.6b`: 33,28 gegen BF16 31,86 (+4,47 %). Gemessen am 2026-09-11 ueber 435 Positionen aus vier WikiText-2-Sequenzen. ⚑ Der groesste Abstand der Reihe, und das passt zur Richtung: Je kleiner das Modell, desto teurer die Quantisierung anteilig (0,6B +4,47 %, 4B +1,64 %, 14B +1,16 %, 30B-A3B kein messbarer Abstand). Kriterium <= 5 % erfuellt, aber von allen vier Modellen am knappsten
- `myelith-30b-a3b`: 10,42 gegen BF16 10,48 (-0,59 %). Das Vorzeichen ist kein Beleg fuer Ueberlegenheit: Zwei der vier Sequenzen sind besser, zwei schlechter, und der Standardfehler des Mittels betraegt 1,66 %. Bei 435 Positionen ist kein Unterschied auflösbar; das Kriterium (<= 5 %) ist mit weitem Abstand erfuellt
- `myelith-4b`: 19,95 gegen BF16 19,63 (+1,64 %)

**Anmerkungen:**

- `myelith-0.6b`: Der kleinste Vertreter und der Anker des Projekts (2026-09-11). Er loest Qwen2.5-0,5B ab; damit liegt die kleinste Groesse in derselben Familie wie 4B und 14B, und die Reihe misst eine Achse (Groesse) statt zweier (Groesse und Familie). ⛑ Fund 336: Dieses Modell traegt in der letzten Ebene ein post_attention_layernorm-Gewicht mit dem Betrag 192, waehrend der Median derselben Zeile bei 3,1 liegt. int8 reicht bis 127. Die Kalibrierung verschiebt den Kanal deshalb in die Folgematrizen (gamma/2, Spalte*2); im Gleitkomma ist das bitgleich, geprueft an den Logits. Es ist das erste Modell des Projekts, das die Umformung braucht.
- `myelith-30b-a3b`: Das erste Mixture-of-Experts-Modell des Projekts: 128 Experten je Layer, Top-8, alle 48 Layer sind MoE (mlp_only_layers ist leer). Kalibriert am 2026-08-25 auf einer 24-GiB-Maschine, obwohl das bf16-Modell 56,9 GiB und das Artefakt 29 GiB gross ist: Die Gewichte werden eingeblendet statt kopiert, und Quantisierung wie Export laufen im Strom. Artefakt: 18 868 Tensoren in 37 747 Dateien. Belegt: Fortsetzung von 'Die Hauptstadt von Frankreich ist' lautet ' Paris. Die Hauptstadt', Token-Hash 99bfc1f64e901811 ueber zwei unabhaengige Laeufe gleich. Perplexitaet am 2026-08-25 gemessen; Einordnung siehe eval/results/. Nach Gesamtparametern setzt das Modell die Reihe fort, in der der Abstand mit der Groesse schrumpft (0,5B +2,11 %, 4B +1,64 %, 7B +1,14 %); nach AKTIVEN Parametern (3,0 Mrd.) tut es das nicht. Welche der beiden Groessen massgeblich ist, ist offen.
- `myelith-4b`: Die erste Qwen3-Variante des Projekts und der Traeger von QK-Norm. Drei Unterschiede zu Qwen2.5, von denen nur einer vorher benannt war: QK-Norm (Q und K je Kopf normiert, vor RoPE), keine Attention-Biases, und head_dim 128 bei hidden_size/num_heads = 80 (Fund 59). Status 'erprobt', nicht 'verifiziert': Das Artefakt laeuft und ist bitgleich ueber Laeufe, der Perplexitaetsabstand ist noch offen.

## Woher die Gewichte kommen

Der Testclient holt sie selbst, wenn er sie braucht: Menüpunkt
**[4] Artefakt wählen** oder beim ersten Lauf, der ein Modell benötigt.
Von Hand geht es auch:

```bash
huggingface-cli download <hf_repo> --revision <hf_revision> \
    --local-dir INTEGER_LLM/models/<hf_verzeichnis>
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

⛑ **Hier stand bis zum 2026-09-10 eine einzelne Spalte „Lizenz"**, und
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
Methode in `ETHICS/Lizenzlage.md`.

Für die Apache-2.0-Varianten ist auch die Lage **quantisierter
Ableitungen** geklärt: §2 erlaubt Bearbeitungen, §4(a) und §4(b) binden
sie an eine Lizenzkopie und an die Kennzeichnung geänderter Dateien, und
eine `NOTICE`-Datei, die §4(d) auslösen würde, enthalten die
Qwen2.5-Repositorien nicht. Das ist eine Lesart des Lizenztextes und
ersetzt vor einem Genesis-Block keine Prüfung durch jemanden, der dafür
haftet.
