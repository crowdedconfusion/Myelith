# Die Modelle dieser Kiste

Fremde Gewichte, Lizenzen in `COMPLIANCE/NOTICES.md`. Zwei Sorten:

**ONNX, auf 8 Bit gepackt** (`packen.py`, je Kanal eine Skala; 8 Bit, weil darunter die Lesegüte einzelner Schriften kippt). Lesemodelle und `text-finden.onnx` von PaddlePaddle (PP-OCRv5 mobile) in der ONNX-Fassung des Pakets RapidOCR 3.10.0, Apache-2.0. Je Schrift gehört zur `.onnx` eine `.zeichen`-Liste aus den Metadaten des Modells.

**Zeilennetze** (`netz_packen.py`): aus `.traineddata` nur Netz, Codes und Zeichensatz, ohne Wörterbücher, je rund 1 MB.

| Datei | Größe | Was | Herkunft | Lizenz | SHA-256 (Anfang) |
|---|---|---|---|---|---|
| `text-finden.onnx` | 1,3 MB | Zeilen finden, für alle Schriften | PP-OCRv5 mobile | Apache-2.0 | `a9f430223414d353` |
| `aufbau.onnx` | 4,7 MB | Seitenaufbau, 23 Klassen | PP-DocLayout-S, ONNX-Ausfuhr `stefanj0/PP-DocLayout-S-ONNX`, Stand 7e851b9e | Apache-2.0 | `33688dbee1c23e34` |
| `lesen-latein.onnx` | 2,0 MB | Latein, auch Antiqua und Fraktur | PP-OCRv5 mobile | Apache-2.0 | `0849fcab0821c4cf` |
| `lesen-kyrillisch.onnx` | 2,1 MB | Kyrillisch, auch Ukrainisch | PP-OCRv5 mobile | Apache-2.0 | `f4f59c4e8ae83cf1` |
| `lesen-griechisch.onnx` | 2,0 MB | Neugriechisch | PP-OCRv5 mobile | Apache-2.0 | `f63d7d568de60a8b` |
| `lesen-devanagari.onnx` | 2,1 MB | Devanagari | PP-OCRv5 mobile | Apache-2.0 | `dc7ff9b7d1a3dbb4` |
| `lesen-thai.onnx` | 2,0 MB | Thai | PP-OCRv5 mobile | Apache-2.0 | `3fc6f9e79a343c33` |
| `lesen-tamil.onnx` | 2,0 MB | Tamil | PP-OCRv5 mobile | Apache-2.0 | `51a11243b761fc49` |
| `lesen-telugu.onnx` | 2,0 MB | Telugu | PP-OCRv5 mobile | Apache-2.0 | `85ec5eb420ff088b` |
| `lesen-chinesisch.onnx` | 4,3 MB | Chinesisch (auch Langzeichen, senkrecht) und Japanisch | PP-OCRv5 mobile | Apache-2.0 | `b0686122cfc8026a` |
| `lesen-koreanisch.onnx` | 3,5 MB | Koreanisch | PP-OCRv5 mobile | Apache-2.0 | `c541ec45bbda2903` |
| `netz-fraktur.bin` | 0,4 MB | Fraktur und alte Antiqua | `frak2021_0.905` von Stefan Weil (UB Mannheim), [Zenodo 10125246](https://zenodo.org/records/10125246) | CC0-1.0 | `5eedd54031319c89` |
| `netz-altgriechisch.bin` | 0,4 MB | Altgriechisch (polytonisch) | tessdata_fast `grc` | Apache-2.0 | `eaa62d1a39843462` |
| `netz-hebraeisch.bin` | 0,3 MB | Hebräisch | tessdata_fast `script/Hebrew` | Apache-2.0 | `baa8b939bd84c00f` |
| `netz-bengalisch.bin` | 1,0 MB | Bengalisch | tessdata_fast `script/Bengali` | Apache-2.0 | `b3b2a88e24f35fe0` |
| `netz-tibetisch.bin` | 0,9 MB | Tibetisch | tessdata_fast `bod` | Apache-2.0 | `553ab66f353c963b` |
| `netz-arabisch.bin` | 0,3 MB | Arabisch, Persisch, Urdu | tessdata_fast `script/Arabic` | Apache-2.0 | `aeda6c16ccc6e48a` |
| `netz-syrisch.bin` | 1,3 MB | Syrisch | tessdata_fast `script/Syriac` | Apache-2.0 | `fdb25290cb401a3d` |

⚠️ Die Ausfuhr von `aufbau.onnx` stammt von einem Dritten und nicht vom Hersteller. Geprüft ist nur ihr Verhalten an den Probeseiten.

⚠️ Das Frakturmodell wurde mit GT4HistOCR trainiert, einem Datensatz unter CC-BY; ob die Bedingungen eines Datensatzes für daraus trainierte Modelle gelten, ist ungeklärt. Das Modell selbst steht unter CC0.

⚠️ Ein Werk wird in einer Schrift gelesen. Georgisch, Armenisch, Ge'ez, Koptisch und weitere haben hier kein Modell.
