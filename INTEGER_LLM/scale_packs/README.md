# Skalenpakete

**Warum hier Skalen liegen und keine Gewichte.**

Ein Skalenpaket enthält den Teil des Artefaktbaus, der **nicht**
deterministisch ist. Alles Übrige rechnet jeder selbst — aus Gewichten,
die er sich bei Hugging Face holt.

| | |
|---|---|
| Größe je Modell | 543 KiB (0,5B) · 1,3 MiB (7B) |
| Enthält | `scales.json`, `luts.json`, `*.lut.bin`, `theta_v.json` |
| Enthält **nicht** | Modellgewichte, Tokenizer |

## Der Grund

Der Artefaktbau ist auf **derselben** Maschine bitgleich reproduzierbar —
zwei Läufe von Qwen2.5-0,5B lieferten 593 von 593 Dateien identisch. Über
Maschinengrenzen hinweg ist er es nicht nachweislich, denn die
Aktivierungsstatistik entsteht in Gleitkomma. Der Shift folgt aus
`floor(log2(32767 / absmax))`, und gemessen sitzen **3 von 314**
Skaleneinträgen innerhalb von 0,01 % einer Zweierpotenz-Grenze — der
knappste bei 0,003 %. Eine andere BLAS-Version reicht, um einen davon
umzuwerfen, und ein gekippter Shift ändert die Artefaktbytes, also das
Modell.

Für einen Cross-Hardware-Test wäre das fatal: Er würde nicht die Hardware
messen, sondern die Kalibrierung.

**Der Rest des Baus ist dagegen exakt.** Die Gewichtsquantisierung ist
`round(W · 2^shift)` mit ganzzahligem Shift — die Multiplikation mit einer
Zweierpotenz ist in IEEE-Gleitkomma exakt, und `round` ist
round-half-to-even. Bei **festen** Skalen ist der Bau auf jeder Plattform
bitgleich.

Deshalb wird nicht das Modell verteilt, sondern die Skalen.

## Verwendung

```bash
huggingface-cli download Qwen/Qwen2.5-0.5B --local-dir INTEGER_LLM/models/Qwen2.5-0.5B
INTEGER_LLM_MODEL=qwen2.5-0.5b python -m calibrate.src.main
```

Das Paket wird automatisch gefunden und verwendet. Der Bau dauert damit
**Sekunden statt Minuten** (0,5B: 3 s statt ~3 min; 7B: 40 s statt ~20 min),
weil der gesamte Kalibrierkorpus-Durchlauf entfällt.

`INTEGER_LLM_SCALE_PACK=0` erzwingt eine vollständige Neukalibrierung —
nötig, wenn das Paket selbst neu erzeugt werden soll. `INTEGER_LLM_GPTQ=1`
schaltet ebenfalls auf den vollen Weg, da GPTQ die Aktivierungsstatistik
braucht.

## Prüfen

```bash
myl-test artefakte
```

Der Testclient rechnet einen Digest über die drei Dateien, an denen die
Identität des Modells hängt — `theta_v.json`, `model_config.json`,
`tokenizer.json` — und vergleicht ihn mit `REGISTER.json`. `theta_v.json`
enthält die Hashes von `weights_manifest.json`, `scales.json` und
`luts.json`; jene wiederum den Hash **jeder** Gewichts- und LUT-Datei, und
`loader.rs` prüft diese Kette beim Laden. Wer die drei trifft, hat
dasselbe Modell.

Weicht der Digest ab, sagt der Client ausdrücklich, dass dies **kein
Hardware-Befund** ist — sonst sähe ein abweichendes Artefakt aus wie eine
gescheiterte Bitgleichheit, und der Client würde das Gegenteil dessen
berichten, wofür es ihn gibt.

> **Warum nicht über den ganzen Ordner.** Diese Fassung gab es und hat
> sofort falschen Alarm ausgelöst: Ein Synchronisationswerkzeug hatte 432
> inhaltsgleiche Kopien in den Artefaktordner gelegt (`theta_v 2.json` und
> so fort). Der Lader ignoriert solche Dateien; sie ändern das Modell
> nicht. Ein Anker, der bei belanglosen Streudateien anschlägt, macht den
> echten Befund unglaubwürdig. Nebenbei dauerte das Hashen von 8,7 GB
> Minuten — jetzt sind es **0,4 Sekunden**.

## Paket neu erzeugen

Nach einem θ_v-Sprung:

```bash
INTEGER_LLM_SCALE_PACK=0 INTEGER_LLM_MODEL=qwen2.5-0.5b python -m calibrate.src.main
python tools/skalenpaket_bauen.py qwen2.5-0.5b qwen2.5-7b
```

Ein Paket ist an **eine** Spec-Version gebunden. Passt sie nicht zur
aktuellen `theta_v/spec.json`, bricht der Bau ab, statt Skalen und LUTs aus
verschiedenen Specs zu mischen — das wäre ein stiller Modellwechsel.

## Lizenz

Skalen und LUTs sind Messwerte und Tabellen dieses Projekts, keine
abgeleiteten Modellgewichte. Die Gewichte selbst werden nicht verteilt;
jeder bezieht sie unter der Lizenz von Qwen direkt von Hugging Face
(siehe [`../../ETHICS/Lizenzlage.md`](../../ETHICS/Lizenzlage.md)).
