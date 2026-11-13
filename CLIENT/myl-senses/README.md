# myl-senses

Sehen, Hören und Sprechen für den Client. **Das Hauptmodell bleibt ein
Textmodell**; hinter jedem Sinn steht ein eigenes, kleines Modell, das
außerhalb des Repositoriums liegt und ausgetauscht werden kann.

## ⚑ Warum das eine eigene Kiste ist

Die Sinne werden an **zwei** Stellen gebraucht, und das ist der ganze
Grund:

- In der **Agentenschleife**, wo ein Werkzeug sie ruft.
- In der **Chatfunktion**, wo es **keine Werkzeugschleife gibt**. Wer im
  Chat ein Bild anhängt, kann auf keinen Werkzeugaufruf hoffen; das Bild
  muss angesehen werden, wenn es hereinkommt, oder nie.

Zwei Umsetzungen wären genau die Fehlerklasse, die dieses Projekt am
häufigsten trifft. Also liegt die Sache hier, einmal.

⛔️ **Der Chat wertet nur aus, was der Nutzer ausdrücklich angehängt
hat.** Kein Blick in den Arbeitsordner, kein Nachladen, keine
Dateiwerkzeuge. Der Pfad muss unter dem Anhangordner liegen, und das
prüft der Client, bevor er hier etwas ruft.

## ⚑ Warum ohne Shell

Die erste Fassung waren `sh`-Skripte in einer Werkzeugkiste. Das war
modular und **unter Windows wirkungslos**, denn dort gibt es kein `sh`,
und der Client wird für Windows ausgeliefert. Jetzt wird das fremde
Programm unmittelbar gestartet: läuft auf allen drei Systemen, und kein
Argument kann eine zweite Kommandozeile einschleusen.

## Die Sinne, und das Aufnehmen dazu

| Sinn | Richtung | Vorgabe | Dateien in der Heimat |
|---|---|---|---|
| **Sehen** | Bild zu Text | llama.cpp (`llama-mtmd-cli`) | `sehen.gguf`, `sehen-mmproj.gguf` |
| | | | wahlfrei: `sehen-genau.gguf`, `sehen-genau-mmproj.gguf` |
| **Hören** | Ton zu Text | whisper.cpp (`whisper-cli`) | `hoeren.bin` |
| **Sprechen** | Text zu Ton | **Qwen3-TTS-12Hz-0.6B-Base im eigenen Rechenweg** (Rückfall: piper) | `stimme.wav` und `stimme.txt`, sonst `sprechen.onnx` |
| **Aufnehmen** | Mikrofon zu Ton | ffmpeg | kein Modell |

Heimat ist `~/.myelith/sinne`, umzustellen über `MYL_SINNE`. Ein
`bin/`-Ordner darunter wird bei der Programmsuche mit angesehen, **nach**
dem `PATH`: Wer llama.cpp nicht systemweit installieren will, legt es
dorthin.

### Gemessen am 2026-09-17, auf einem M5 Pro

| | Modell | warm | Ergebnis |
|---|---|---|---|
| Hören | large-v3-turbo-q5_0 | 0,8 s | wortrichtig bis auf den Eigennamen |
| Sehen, schnell | SmolVLM2-2.2B | 1,6 s | las „HALT", schweifte **englisch** ab |
| Sehen, genau | Qwen2.5-VL-3B (seit 2026-09-25 ersetzt, siehe unten) | 3,8 s | las **beide Zeilen**, antwortete deutsch |
| Sprechen | Fun-CosyVoice3-0.5B | 26,7 s kalt | davon rund 18 s Modellladen, RTF 1,40 |

⚑ **Die zweite Sprosse hat sich sofort bezahlt gemacht**: dasselbe Bild,
dieselbe Frage, und das 3B liest, was das 2,2B überliest.

📌 **CosyVoice war damals langsamer als Echtzeit** (RTF 1,40) und war es
seit dem 2026-09-25 nicht mehr; seit dem 2026-11-11 ist es durch den
eigenen Weg ersetzt und entfernt, siehe unten.

### ⛔️ Jede erzeugte Sprache ist gekennzeichnet (seit dem 2026-09-25)

Artikel 50 Absatz 2 der KI-Verordnung verlangt, dass synthetische
Inhalte maschinenlesbar als KI-erzeugt erkennbar sind. `kennzeichnung.rs`
tut das für jede Tondatei, bevor sie abgespielt oder abgelegt wird:
ein XMP-Block mit dem IPTC-Quellentyp `trainedAlgorithmicMedia`, ein
RIFF-INFO-Kommentar und ein Wasserzeichen im Signal, rund 36 dB unter der
Sprache. **Was sich nicht kennzeichnen lässt, klingt nicht.** Geprüft an
14 Sätzen des Sprechmodells: ohne Kennzeichnung höchstens 2,5
Standardabweichungen, gekennzeichnet mindestens 11,9, nach MP3 mit
128 kbit/s mindestens 10,3. Nachprüfen: `myl kennzeichen <datei.wav>`.

Das Sehmodell bekommt vor jeder Frage eine Regel (`SEHREGEL`): nur
Sichtbares beschreiben, niemanden identifizieren, keine Gefühle oder
sensiblen Merkmale zuschreiben.

### ⛔️ Die genaue Sehstufe ist seit dem 2026-09-25 Qwen3-VL-4B

Vorher stand dort Qwen2.5-VL-3B. Dessen Original steht unter einer
**Forschungslizenz**, die nur Forschung und nicht kommerzielle Nutzung
erlaubt; die GGUF-Umwandlung, die das Einrichtungsskript holte, nannte
Apache 2.0, kann die Lizenz des Originals aber nicht ändern. Ersetzt durch
`Qwen3-VL-4B-Instruct` (Apache 2.0, GGUF vom Hersteller selbst, Q4_K_M
2,5 GB, Projektor Q8_0 0,45 GB). Gleiches Bild, gleiche Frage, dasselbe
llama.cpp: Das neue las Namen und Unterzeile und beschrieb den
Hintergrund, das alte nannte das Projekt ein „Unternehmen"; 4,7 statt
2,9 s samt Laden. Das Einrichtungsskript ersetzt eine alte Datei, die es
an ihrer Größe erkennt, und lässt eine eigene stehen.

### Sprechen, gemessen am 2026-09-25 (mit CosyVoice, inzwischen entfernt)

Die Zahlen zum Gespräch neben dem Hauptmodell (Vorrang, Lücken) gelten
für den Vorleser und bleiben lehrreich; die Stufen darunter betreffen
CosyVoice. Ein Satz von 62 Zeichen, gemessen vom Eingang beim Läufer bis zum
ersten hörbaren Stück, ohne ein Hauptmodell daneben:

| Stand | erster Ton | RTF |
|---|---|---|
| alles auf der CPU, zehn Flussschritte, ganzer Satz auf einmal | 9,1 s | 2,1 |
| Stimme einmal gemerkt statt je Satz vorbereitet | 7,1 s | 2,2 |
| Fluss auf der Grafikeinheit, sechs Schritte, stückweise | 2,1 bis 2,3 s | 0,86 |
| dazu die Stimmprobe an einer Pause auf 4,4 s gekürzt | 1,6 bis 1,8 s | 0,79 |

Stufe für Stufe, zehn Schritte, ein Satz von rund 5,5 s Ton:
Sprachmodell 3,0 s (45 bis 48 Token/s), Fluss 5,4 bis 6,3 s, Vocoder
0,3 s. **Der Fluss war der Engpass**, und auf der Grafikeinheit braucht
er 1,8 s. Das Sprachmodell bleibt auf der CPU: Dort schafft es 48
Token/s, auf der Grafikeinheit 31. Der Vocoder rechnet in `float64`,
das die Grafikeinheit nicht kann.

⚑ **Sechs statt zehn Flussschritte kosten nichts Hörbares.** Geprüft,
indem whisper jeden Satz zurückhörte (bei 10, 6 und 4 Schritten und drei
Probenlängen jedesmal wortrichtig) und die Stimme gegen die Probe
verglichen wurde (Ähnlichkeit 0,80 bis 0,89, ohne Gang mit der
Schrittzahl).

⚠️ **Neben dem Hauptmodell wird alles langsamer**, denn beide teilen
sich Kerne und Speicherbandbreite. Das Sprachmodell des Sprechers fiel
neben dem 30B von 48 auf 13 Token/s. Gemessen im ganzen Gespräch (eine
Frage, Antwort ohne Nachdenken, die Zeit ab dem Abschicken):

| Hauptmodell | erster Text | erster Ton ohne Vorrang | erster Ton mit Vorrang | Lücken ohne / mit |
|---|---|---|---|---|
| 4B | 0,2 bis 0,3 s | 3,7 bis 3,8 s | **3,0 bis 3,3 s** | 2,2 bis 2,3 s / 2,1 bis 2,3 s |
| 8B | 0,4 bis 1,4 s | 5,4 bis 5,5 s | **4,3 bis 4,6 s** | 2,2 bis 2,5 s / 3,4 bis 3,6 s |
| 30B-A3B | 2,7 bis 2,8 s | 8,5 s | **7,8 s** | 2,3 s / 3,4 s |

Je drei Läufe bei 4B und 8B, einer beim 30B. ⚑ **Vorrang** heisst: Ist
das erste Stück beim Sprecher und klingt noch nichts, hält das
Hauptmodell an (höchstens sechs Sekunden). Es ist dem Sprechen ohnehin
weit voraus; der erste Ton kommt damit 0,65 bis 1,0 s früher. Die
Lücken wachsen beim 8B und 30B um rund eine Sekunde, verteilt über eine
halbe Minute Sprechen, denn das Modell ist dann länger neben dem
Sprecher beschäftigt.

⛔️ **Nicht übernommen, weil gemessen schlechter:** das Sprachmodell des
Sprechers auf der Grafikeinheit (4B: erster Ton 4,8 statt 3,8 s, Lücken
13 statt 2 s; das Hauptmodell rechnet dort mit) und ein Rückstau, der das
Hauptmodell bei jedem offenen Satz anhält (30B: zehn Sekunden Lücken).

⚠️ **Mit Nachdenken wartet das Gespräch auf das Nachdenken**: Das 30B
schrieb nach 44 s das erste Wort der Antwort. Ein vorbereiteter Satz wie
„Lass mich kurz darüber nachdenken." kommt nach rund drei Sekunden,
danach ist es still, bis die Antwort beginnt.

### ⚑ Zwei Sprossen beim Sehen

Festlegung des Projektinhabers: ein kleines schnelles Sehmodell und ein
größeres genaues, je nach Anwendungsfall. **Die Wahl trifft der Aufrufer
und nicht das Modell**, denn ein zusätzliches Argument kostet jedes
kleine Modell eine Entscheidung, die es schlecht trifft:

- **Beim Anhängen `Schnell`.** Ein Blick für jeden, auch für den, der
  gar nichts fragen wollte.
- **Beim Werkzeugaufruf `Genau`.** Wer ausdrücklich eine Frage stellt,
  will die bessere Antwort.

⚑ **Wer nur eines hinlegt, bekommt es für beides.** Eine Stufe, die
mangels Gewichten leer ausginge, wäre eine Falle.

### ⚑ Qwen3-TTS im eigenen Rechenweg ist die Vorgabe (seit dem 2026-11-11)

Qwen3-TTS-12Hz-0.6B-Base (Apache 2.0, ohne Zusatz) rechnet im Programm,
ohne Python: Textzerleger, Sprecher in int8, Code-Vorhersage,
Stimmabdruck und Tonzerleger in Rust. Es **klont aus der Stimmprobe**,
mit ihrem mitgeschriebenen Text (`stimme.txt`) etwas genauer. Abgenommen
gegen CosyVoice auf derselben Maschine, fünf Sätze mit derselben
Stimmprobe: Wortfehler gleich (0,141), erster Ton 1,08 statt 1,58 s, eine
Antwort aus fünf Sätzen ohne Lücke, Laden 1,2 statt 7 bis 9 s;
Ähnlichkeit (CAM++) 0,76 statt 0,84, im Blindvergleich 3 zu 2 für den
eigenen Weg.

⚑ **CosyVoice ist seit CLIENT 0.219.0 entfernt** (Festlegung des
Projektinhabers, 2026-11-11). Die Modellkarte nennt neben Apache 2.0 „nur
für akademische Zwecke“, dazu kamen Python und PyTorch als Laufzeit und
ein eigener Läufer. **piper bleibt Rückfall**, eine Binärdatei ohne
Laufzeit, die sofort geht und dafür nur fertige Stimmen kennt.

Die Wege, in dieser Reihenfolge:

| Weg | klont | braucht |
|---|---|---|
| eigenes Skript `bin/sprechen`, je Satz | ja | was immer es selbst will |
| eigenes Skript `bin/sprechen-dauer`, stehend | ja | dasselbe |
| **Qwen3-TTS (eigen)** | ja | die Gewichte unter `qwen3-tts-0.6b-base` |
| piper | **nein** | eine Binärdatei und eine Stimme |

`MYL_SPRECHEN=fremd` lässt den eigenen Weg aus, `eigen` stellt ihn vor
piper (das ist ohnehin die Vorgabe).

### ⚑ Ein Sprechskript, das stehen bleibt

Ein fremdes Modell, das je Aufruf Gewichte lädt, wäre satzweise
langsamer als gar nicht gestreamt. Dafür gibt es `bin/sprechen-dauer`:
gestartet mit `--dauer` (und der Stimmprobe, falls eine liegt; ihr Text
steht in `MYL_STIMMTEXT`), danach Satz für Satz:

| Richtung | Zeile |
|---|---|
| vom Skript, einmal beim Start | `bereit` |
| an das Skript, je Satz | `<textdatei>` Tabulator `<zielwav>` |
| vom Skript, je hörbarem Stück (wahlfrei) | `stueck` Tabulator `<teilwav>` |
| vom Skript, je Satz | `ok` oder `fehler: …` |

⚑ **Die Stücke sind der Unterschied zwischen zwei und neun Sekunden**:
Der Client spielt jedes, sobald es gemeldet ist; das ganze Satz-WAV liegt
am Ende trotzdem da. Wer keine Stücke meldet, klingt satzweise. **Jedes
Stück und jeder Satz wird gekennzeichnet**, bevor es klingt.

⚑ **Das Schließen der Eingabe beendet es**, wie bei der Aufnahme. Und
das Warten auf `bereit` ist Teil der Sache: Wer nicht wartet, schickt
den ersten Satz in ein Programm, das noch lädt.

## Einrichten


```sh
mkdir -p ~/.myelith/sinne

brew install llama.cpp      # Sehen; NixOS: nix-shell -p llama-cpp
brew install whisper-cpp    # Hören
brew install ffmpeg         # Aufnehmen, und für Ogg, Opus, WebM (WAV und FLAC liest Myelith selbst, m4a und MP3 macOS)

# Sprechen: die Gewichte von Qwen3-TTS holt das Einrichtungsskript
# (fester Stand, Prüfsummen) nach MODELS/audio/qwen3-tts-0.6b-base:
sh SYSTEM/install/sinne-einrichten.sh

brew install piper          # nur als Rückfall, kann keine Stimme klonen
```

Dann die Gewichte unter den Namen aus der Tabelle ablegen. Brauchbar und
klein: **SmolVLM2-2.2B-Instruct** fürs schnelle Sehen,
**Qwen3-VL-4B** fürs genaue, **ggml-small.bin** fürs Hören,
**de_DE-thorsten-medium** fürs Sprechen.

⛔️ **Solange etwas fehlt, sagt der Sinn, was fehlt**, mit Pfad und
Befehl, und tut so, als hätte er nichts gesehen. Er stürzt nicht ab und
er erfindet nichts. Eine Voraussetzung, die erst beim Absturz sichtbar
wird, ist keine Voraussetzung, sondern eine Falle.

### Andere Orte

`MYL_SEHER`, `MYL_SEHMODELL`, `MYL_SEHPROJEKTOR`,
`MYL_SEHMODELL_GENAU`, `MYL_SEHPROJEKTOR_GENAU`, `MYL_HOERER`,
`MYL_HOERMODELL`, `MYL_SPRECHER`, `MYL_SPRECHMODELL`, `MYL_AUFNEHMER`,
`MYL_TONFORMAT` und `MYL_TONGERAET` (woher ffmpeg den Ton nimmt); dazu
`MYL_SEHEN_TOKEN` (Länge der Beschreibung, Vorgabe 320) und
`MYL_SINNE_FAEDEN` (Kerne, Vorgabe 4).

⚠️ **Eine gesetzte, aber falsche Angabe fällt nicht still auf die
Vorgabe zurück.** Sonst arbeitete der Client mit einem anderen Modell
als dem genannten, und niemand sähe es.

## ⚠️ Was hier nicht passiert

**Hier rechnet kein Modell**, und **nichts davon geht durch den
Konsens**: keine Bit-Exaktheit, keine Konformitätsvektoren, keine
Nachprüfbarkeit. Die Werkzeugbeschreibungen sagen das dem Modell
ausdrücklich, damit es die Auskunft nicht für sein eigenes Sehen hält.

## Prüfen

```sh
cd CLIENT/myl-senses && cargo test
```

⚑ **Die Proben brauchen kein llama.cpp.** Sie stellen das Programm
selbst und prüfen den Weg: Wird es gefunden, kommt seine Ausgabe durch,
bleibt sein Protokoll draußen, und steht verständlich da, was fehlt,
wenn nichts da ist? **Der letzte Fall ist der wichtige**, er ist der,
den jeder Nutzer zuerst sieht.
