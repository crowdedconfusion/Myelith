# Myelith einrichten

Drei Skripte, eines je System. Sie **prüfen zuerst, bauen dann** und
legen die Programme ab. Aufgerufen werden sie aus dem
Wurzelverzeichnis des Klons oder von hier aus, das macht keinen
Unterschied: Jedes findet seinen Klon über den eigenen Ort.

| System | Aufruf |
|---|---|
| **macOS** | `sh INSTALL/installieren-macos.sh` |
| **NixOS** und andere Anlagen mit Nix | `sh INSTALL/installieren-nixos.sh` |
| **Windows** (PowerShell) | `.\INSTALL\installieren-windows.ps1` |

Jedes kennt dieselben Schalter:

| Schalter | Wirkung |
|---|---|
| `--pruefen` (Windows: `-NurPruefen`) | Sagt nur, was fehlt, und baut nicht |
| `--aktualisieren` (Windows: `-Aktualisieren`) | Holt erst den neuen Stand, baut dann |
| `--system` (nur macOS) | Legt nach `/usr/local/bin` und `/Applications` statt unter das eigene Benutzerverzeichnis |
| `--ohne-netz` (nur macOS) | Baut ausschließlich aus dem örtlichen Cargo-Vorrat |

## Was ein frischer Klon kann, und was er braucht

⚑ **Nachgemessen am 2026-09-17** an einer Kopie des Klons, nicht
behauptet.

| Vorhaben | Braucht | Ohne Netz |
|---|---|---|
| **Die fünf Programme bauen** | Rust-Werkzeugkette, Xcode-Werkzeuge | **ja**, der Vorrat liegt im Klon |
| **Den Agenten betreiben** | ein Artefakt unter `INTEGER_LLM/artifacts/` | **ja** |
| **Konformität und Messungen fahren** | dasselbe | **ja** |
| **Bücher zu Korpus oder Mappe** | Python 3.9 | **ja**, die Räder liegen im Klon |
| **Artefakte aus Gewichten bauen** | Gewichte unter `models/`, Python mit torch | **nur wenn beides schon da ist** |
| **Gewichte holen** | Netz zu Hugging Face | **nein** |

**Der gemessene Lauf:** Klon ohne `target-shared`, Netz auf einen toten
Port umgeleitet, `sh INSTALL/installieren-macos.sh --ohne-netz`. Ergebnis
nach **1 Minute 37** (viele Kerne): alle fünf Programme und das
Fensterbündel, Rückgabewert 0.

## Offline einrichten

⛔️ **Es gibt drei Lagen, und sie sind sehr verschieden.** Wer „offline"
sagt, meint meist die zweite.

**1. Frischer Klon, Netz da.** Alles geht. Die Werkzeugkette holt, was
sie braucht.

**2. Frischer Klon, kein Netz, aber der Rechner hat schon einmal
gebaut.** Alles geht bis auf das Holen von Gewichten. Der Cargo-Vorrat
unter `~/.cargo/registry` trägt die Abhängigkeiten; hier sind das 1,4 GB.

```sh
sh INSTALL/installieren-macos.sh --ohne-netz
```

⚑ **`--ohne-netz` gibt `--offline` an cargo weiter**, und gebaut wird
ohnehin immer mit `--locked`: Die Sperrdateien gehören zum Stand, und
ohne sie löst cargo neu auf und baut etwas anderes, als hier geprüft
wurde.

**3. Frischer Klon, kein Netz, frischer Rechner.** ⚑ **Geht auch**,
seit die Archive im Klon liegen. ⛔️ **Ohne sie ginge es nicht**, und das
ist gemessen: mit leerem Vorrat `error: no matching package named blst
found`. **Die Sperrdateien nennen die Fassungen, sie enthalten den
Quelltext nicht**, und genau diese Lücke schließt `vorrat/`.

### Der Vorrat im Baum: ein Klon, der von sich aus offline baut

⚑ **Festlegung des Projektinhabers (2026-09-17):** Die Fremdquellen
liegen als **Archive** unter `vorrat/` im Repositorium. Damit baut jeder
Klon ohne Netz, ohne Schalter und ohne mitgebrachte Datei.

```sh
git clone <url>
sh INSTALL/installieren-macos.sh
```

**Gemessen:** frischer Klon, **leerer** Cargo-Vorrat, Netz auf einen
toten Port umgeleitet: Auspacken der 758 Pakete in **6 Sekunden**, danach
alle fünf Programme samt Fensterbündel in **1 Minute 57**.

#### ⚑ Warum Archive und nicht ausgepackte Quellen

Dieselben 758 Pakete, drei Wege, gemessen:

| Weg | Im Git | Dateien | Wächst je Fassungssprung um |
|---|---|---|---|
| `cargo vendor`, ausgepackt | 202 MB | 36 805 | die geänderten Pakete |
| ein gepacktes Bündel | 122 MB | 1 | **jedes Mal 122 MB**, ein Archiv ändert sich ganz |
| **Archive je Paket** | **113 MB** | **758** | **nur die neuen Archive**, meist wenige MB |

⚑ **Nur der letzte Weg wächst nicht mit.** Jede Paketfassung ist eine
eigene, unveränderliche Datei: Wer `tokio` anhebt, legt ein Archiv dazu,
und die 757 anderen rühren sich nicht. **Ein Bündel dagegen ist bei jeder
Änderung ein neues Bündel**, und die alten bleiben für immer in der
Geschichte.

Das Repositorium wächst damit von **20 MB auf rund 133 MB** und von
1 446 auf 2 204 Dateien. Ausgepackt wird beim Einrichten nach
`.myelith-vorrat/`, und das ist nicht versioniert.

#### ⚠️ Was auf welchem System wirklich geprüft ist

**Der schwierige Teil ist auf allen dreien derselbe Quelltext**
(`INSTALL/vorrat.py`), die Skripte rufen ihn nur auf. Trotzdem gehört
gesagt, was gemessen ist und was nicht:

| System | Stand |
|---|---|
| **macOS** | **Von Anfang bis Ende gemessen:** frischer Klon, leerer Cargo-Vorrat, Netz blockiert, fünf Programme in 1:57 |
| **NixOS/Linux** | Eingebaut, der Block einzeln ausgeführt, das Auspacken geprüft. ⚠️ **Auf einer echten Nix-Anlage nicht gelaufen** |
| **Windows** | Eingebaut, ⚠️ **hier nicht ausführbar** (kein PowerShell auf der Entwicklungsmaschine) |

⚑ **Was für Windows trotzdem belegt ist**, weil es sich hier messen
ließ: Der Vorrat enthält **60 Windows-Pakete** samt `webview2-com-sys`,
also den ganzen Tauri-Unterbau; kein Pfad trägt ein unter Windows
verbotenes Zeichen oder einen reservierten Namen; und der längste Pfad
im ausgepackten Vorrat ist **153 Zeichen**, bleibt mit einem
gewöhnlichen Klonpfad also unter der Grenze von 260.

⚠️ **Windows braucht Python** zum Auspacken. Fehlt es, sagt das Skript
das gerade heraus, statt später an einem fehlenden Paket zu scheitern.

⚠️ **Bei Nix deckt der Vorrat die Fremdkisten, nicht die Nix-Eingaben.**
`nix develop` holt die Werkzeugkette, wenn sie nicht schon im Store
liegt. Wer dort wirklich ohne Netz baut, hat den Store warm oder nimmt
`--in-der-shell` mit einer vorhandenen Werkzeugkette.

⚠️ **Und Tauri braucht auf Linux Systembibliotheken** (webkit2gtk und
Nachbarn). Die kommen aus der Paketverwaltung des Systems und nicht aus
diesem Vorrat.

#### ⛔️ Und der Vorrat ist prüfbar, nicht nur bequem

Jede Sperrdatei nennt zu jedem Paket seine SHA-256. **Die Archive lassen
sich also gegen den Stand prüfen, den das Repositorium ohnehin
festhält:**

```sh
python3 INSTALL/vorrat.py pruefen    # jede Datei gegen die Sperrdateien
python3 INSTALL/vorrat.py sammeln    # nach einer Fassungsanhebung
python3 INSTALL/vorrat.py auspacken  # macht der Installer selbst
```

⚠️ **Die Pflicht dazu:** Nach jeder Änderung an einer Abhängigkeit muss
`sammeln` laufen, sonst fehlt ein Archiv. ⚑ **Der Unterschied zu früher
ist, dass es auffällt**: `pruefen` sagt, welches Paket fehlt, und der
Bau bricht mit dem Namen des fehlenden Pakets ab, statt stillschweigend
etwas Altes zu nehmen.

⚠️ **Was `sammeln` braucht:** Netz **oder** einen Cargo-Vorrat, in dem
das Paket schon liegt. Es holt zuerst von dort und erst dann aus dem
Netz, und es prüft jede Datei sofort gegen die Sperrdatei.

## Artefakte aus Gewichten bauen

**Zwei Schritte, und der erste braucht Netz.**

```sh
sh INTEGER_LLM/scripts/fetch_model.sh     # Gewichte nach models/, feste Revision
sh INTEGER_LLM/scripts/build_artifacts.sh # Kalibrierung und Export nach artifacts/
```

⚠️ **Der Export braucht Python mit `torch` und `transformers`**, denn
die Gewichte kommen als Gleitkomma-Tensoren. Die Umgebung entsteht
einmalig und braucht Netz und einige Gigabyte:

```sh
cd INTEGER_LLM/calibrate
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python3 -r requirements.txt
```

⚑ **Das Skript sucht sie selbst** (`$PYTHON`, dann
`calibrate/.venv/bin/python3`, dann `python3`) und sagt, was fehlt.
📌 **Bis zum 2026-09-17 rief es blankes `python3`**, und auf einem
frischen Klon endete der Artefaktbau in einem nackten
`ModuleNotFoundError`. **Eine Voraussetzung, die erst beim Absturz
sichtbar wird, ist keine Voraussetzung, sondern eine Falle.**

⚑ **Der Bau ist wiederholbar, und das ist gemessen.** Am 2026-09-17
wurde das Artefakt des 0,6B aus denselben Gewichten neu gebaut:
**48/48 Konformitätsvektoren** und **derselbe `decode_digest`**
(`b744c06…`) wie bei dem Stand vom 11. September. Möglich ist das, weil
das Skalenpaket im Repositorium liegt und die Aktivierungsstatistik
deshalb entfällt.

⚠️ **Der Betrieb braucht davon nichts.** Wer ein fertiges Artefakt hat,
braucht weder torch noch Python: Der Ganzzahlpfad ist Rust.

## Sehen, Hören und Sprechen einrichten

⚑ **Der Bau des Repositoriums braucht davon nichts.** Er läuft ohne Netz
und ohne eine einzige dieser Zutaten; die Sinne sind eine Erweiterung,
keine Bedingung. Wer sie will:

```sh
sh INSTALL/sinne-einrichten.sh                 # alles
sh INSTALL/sinne-einrichten.sh --ohne-sprechen # nur Sehen und Hören
sh INSTALL/sinne-einrichten.sh --pruefen       # nur nachsehen
```

`myl sinne` zeigt danach, was geht, und `myl sinne <datei>` schickt eine
Datei hindurch.

| | woher | wohin |
|---|---|---|
| ffmpeg, llama.cpp, whisper.cpp | Paketverwalter des Systems | `/opt/homebrew/bin` und ähnliche |
| Hörmodell (0,6 GB) | whisper.cpp auf Hugging Face | `~/.myelith/sinne/hoeren.bin` |
| Sehmodell schnell (1,7 GB) | SmolVLM2-2.2B-Instruct | `sehen.gguf`, `sehen-mmproj.gguf` |
| Sehmodell genau (3,3 GB) | Qwen2.5-VL-3B-Instruct | `sehen-genau.gguf`, `sehen-genau-mmproj.gguf` |
| Sprechen (4,5 GB) | **Fun-CosyVoice3-0.5B** samt eigener Python-Umgebung | `~/CosyVoice`, verlinkt nach `~/.myelith/sinne/cosyvoice` |

⛔️ **Was hier bewusst nicht im Repositorium liegt:** die Programme
selbst und die Gewichte. llama.cpp, whisper.cpp und ffmpeg sind je
System verschieden und zusammen einige hundert Megabyte; torch ist
allein rund 2,5 GB. Das wäre derselbe Ballast, der beim Vorrat der
Fremdquellen abgelehnt wurde. **Was das Repositorium mitbringt, ist das
Verbindungsstück**: dieses Skript, der Läufer für CosyVoice und die
Namen, unter denen der Client sucht.

⚠️ **Und deshalb braucht dieser eine Schritt Netz.** Alles andere an
einem frischen Klon nicht.

### Freigaben des Betriebssystems

⚑ **Ein installiertes Programm ist noch keine Erlaubnis.** Mikrofon,
Kamera und Bildschirm gibt jedes der drei Systeme erst heraus, wenn ein
Mensch zustimmt, und keine davon lässt sich aus dem Programm heraus
erteilen. **Was fehlt, fehlt deshalb nie als Download, sondern als
Häkchen.**

⚠️ **Zwei Stufen, und beide müssen stimmen.** Der Client hat eine eigene
Scharfstellung für Bildschirm und Kamera (`MYL_BLICK_BILDSCHIRM`,
`MYL_BLICK_KAMERA`, im Fenster zwei Häkchen unter „Agent"); sie ist
**unabhängig** von der Freigabe des Systems. Steht die eine und die
andere nicht, gibt es das Werkzeug und keine Aufnahme.

| Sinn | macOS | Linux | Windows |
|---|---|---|---|
| **Mikrofon** | Systemeinstellungen, Datenschutz und Sicherheit, **Mikrofon** | Gruppe `audio`, sonst nichts | Einstellungen, Datenschutz, **Mikrofon** |
| **Kamera** | dort, **Kamera** | Gruppe `video` (`/dev/video0`) | dort, **Kamera** |
| **Bildschirm** | dort, **Bildschirmaufnahme** | X11: nichts. **Wayland: Portal nötig** | nichts |
| **Dateien** | bei einem Ordner ausserhalb des Benutzerverzeichnisses: **Festplattenvollzugriff** | Dateirechte des Benutzers | Dateirechte des Benutzers |
| **Web-Recherche** | keine Systemfreigabe, aber `curl` muss da sein (ist es) | `curl` aus der Paketverwaltung | `curl.exe` ab Windows 10 dabei |
| **PDF lesen** | `brew install poppler` oder `pip install pypdf` | `apt install poppler-utils` | poppler für Windows, oder `pip install pypdf` |

⚑ **Die Web-Recherche ist der einzige Punkt in dieser Tabelle, bei dem
das System nicht fragt.** Ein Programm darf ins Netz, ohne dass jemand
zustimmt. Deshalb sitzt die Schranke hier im Client selbst: Das Häkchen
„Im Web recherchieren dürfen" unter „Agent" ist aus, bis jemand es
setzt, und es gilt nur für das Gespräch, nicht für den vollen
Agentenbetrieb. Was dabei gelesen wird, kommt gekennzeichnet als
fremder Inhalt zurück und nie als Anweisung; gelesen wird nur, was aus
einem Suchtreffer stammt oder was der Mensch selbst genannt hat.

⚑ **PDF ist eine eigene Zeile, weil es ein eigenes Programm braucht.**
Gesucht wird in dieser Reihenfolge: `MYL_SCHRIFTLESER`, `pdftotext`
(poppler), `mutool` (mupdf), zuletzt ein Python, das `pypdf` oder
`fitz` **wirklich** hat. Fehlt alles, fehlt nur das Lesen von PDF, und
das Werkzeug sagt es statt zu schweigen. `myl sinne` zeigt in der Zeile
`Schrift`, welcher Weg gefunden wurde.

#### macOS

⛔️ **Für die Bildschirmaufnahme gibt es keinen Info.plist-Schlüssel.**
Mikrofon und Kamera kündigt das Bündel an (`NSMicrophoneUsageDescription`,
`NSCameraUsageDescription`), und macOS fragt dann beim ersten Gebrauch.
Die Bildschirmaufnahme wird **einmal von Hand** freigegeben, und erst
danach fragt nichts mehr.

⚠️ **Ohne sie meldet `screencapture`:**

```
could not create image from display
```

und endet mit Rückgabewert eins. Der Client sagt das weiter und nennt
den Weg zur Freigabe.

📌 **Und macOS merkt sich eine erteilte Erlaubnis an der Kennung samt
Signatur.** `Myelith.app` wird ad hoc signiert, damit sie über einen
Neubau hinweg dieselbe Identität behält; ohne das käme die Frage nach
jedem Bau wieder, oder schlimmer, sie käme nicht und die Aufnahme bliebe
leer.

⚑ **Im Terminal gilt die Erlaubnis dem Terminal**, nicht `myelith`. Wer
`myelith` aus Terminal.app oder iTerm startet, gibt dieses Programm
frei; wer `Myelith.app` doppelklickt, gibt das Bündel frei. **Das sind
zwei Einträge in derselben Liste.**

#### Linux

**Mikrofon und Kamera hängen an Gruppen**, nicht an einem Dialog:

```sh
sudo usermod -aG audio,video "$USER"    # danach neu anmelden
```

⚠️ **Der Bildschirm ist die Ausnahme, und sie hängt an der Sitzungsart.**
Unter X11 nimmt `ffmpeg -f x11grab` ohne jede Freigabe auf. **Unter
Wayland tut es das nicht**, und zwar entwurfsgemäss: Dort gibt erst ein
Portal den Bildschirm heraus, mit einem Dialog je Aufnahme.
`MYL_SCHIRMFORMAT` und `MYL_SCHIRMGERAET` sagen ffmpeg, woher es nehmen
soll, wenn die Vorgabe nicht passt.

#### Windows

Mikrofon und Kamera stehen unter Einstellungen, Datenschutz und
Sicherheit; dort muss zusätzlich **„Desktop-Apps den Zugriff erlauben"**
anstehen, sonst greift die Freigabe nur für Store-Anwendungen. Die
Bildschirmaufnahme über `gdigrab` braucht keine.

⚠️ **`myelith --root` ist etwas anderes als eine Freigabe.** Es
verschiebt die Einhängegrenze des Agenten auf das Dateisystem und
startet sich dafür mit Verwalterrechten neu; die Benutzerkontensteuerung
fragt. Siehe den Changelog des Clients.

### ⛔️ Zwei Stolpersteine, beide beim tatsächlichen Einrichten gefunden

**Ein aus dem Finder gestartetes `Myelith.app` erbt den PATH der
Anmeldesitzung**, und darin steht `/opt/homebrew/bin` nicht. Ein mit
`brew` installiertes llama.cpp wäre im Terminal da und im Fenster nicht,
ohne eine Meldung, die das sagt. Der Client sucht deshalb zusätzlich an
den üblichen Orten.

**CosyVoice braucht seine eigene Python-Umgebung.** Der erste Python im
PATH ist unter macOS `/usr/bin/python3`, also 3.9 und ohne torch; damit
gäbe es einen Importfehler statt einer Stimme. Der Client nimmt deshalb
den Interpreter aus der `.venv` **neben** der CosyVoice-Installation.

📌 **Und ein dritter, in `requirements.txt` von CosyVoice:** setuptools
81 hat `pkg_resources` aus der Bauumgebung genommen, woran
`openai-whisper==20231117` beim Bauen scheitert und die ganze
Installation abbricht. Das Skript nimmt setuptools zurück und baut
dieses eine Paket ohne eigene Bauumgebung.

📌 **Ein vierter, und er kostet die meiste Zeit: die Reihenfolge von
pip.** Wer `openai-whisper` zuerst installiert, holt damit unbestimmte
Fassungen von torch und numpy herein; die gepinnten aus
`requirements.txt` passen dann nicht mehr dazu, und **pip läuft
rückwärts durch hunderte Fassungen**, minutenlang bei voller Last, ohne
Ende in Sicht. Erst die gepinnten, dann das eine Paket.

### ⛔️ Deutsch kann erst CosyVoice 3

Die 2.0-Gewichte decken Chinesisch und Englisch ab. Eine deutsche
Stimmprobe ergibt damit Laute, die wie Deutsch klingen und keines sind:
Am 2026-09-17 gemessen, indem whisper vorgelesen bekam, was CosyVoice
gesagt hatte. Vorlage „dies ist die erste gesprochene Antwort", gehört
„dies Go! Ist die Örsteck ist proschein". ⚑ **Deshalb holt das Skript
`Fun-CosyVoice3-0.5B`**, das neun Sprachen abdeckt, und der Läufer nimmt
es, sobald es da ist.

## Was am Ende dasteht

**Fünf Programme**, und welche das sind, sagen die Kisten selbst über
`[package.metadata.myelith]` in ihrer `Cargo.toml`. Kein Skript führt
eine eigene Liste.

| | |
|---|---|
| `myelith` | Der Agent in der Konsole: in ein Verzeichnis gehen, `myelith` tippen |
| `myl` | Das Bedieninstrument: `myl frage`, `myl agent`, `myl einstellungen` |
| `myl-oberflaeche` | Das Fenster. Unter macOS zusätzlich als `Myelith.app` |
| `myl-node` | Der Knoten |
| `myl-test` | Der Testclient für Hardware- und Determinismusläufe |

## macOS

**Braucht:** die Xcode-Befehlszeilenwerkzeuge und die
Rust-Werkzeugkette.

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Das Skript legt die Programme nach `~/.local/bin` und das Bündel nach
`~/Applications`. Steht `~/.local/bin` nicht in deinem `PATH`, sagt es
dir die Zeile für deine Shell-Datei.

⚠️ **Beim ersten Start meldet Gatekeeper einen unbekannten
Entwickler**, denn das Bündel ist nicht signiert: einmal Rechtsklick
auf `Myelith.app`, dann *Öffnen*, bestätigen. Danach nicht mehr.

## NixOS und andere Anlagen mit Nix

**Braucht:** `nix`. Alles Weitere steht in `flake.nix` im
Wurzelverzeichnis, und das Skript holt es sich selbst: Es ruft sich in
`nix develop` noch einmal auf und baut darin.

```sh
sh INSTALL/installieren-nixos.sh
```

⚑ **Es ändert nichts an deiner Systemkonfiguration.** Ein
Installationsskript, das `/etc/nixos/configuration.nix` anfasst, nimmt
einem NixOS-Nutzer genau das weg, wofür er NixOS benutzt. Die
Programme landen unter `~/.local/bin`, der Menüeintrag unter
`~/.local/share/applications`.

⚠️ **Ungeprüft auf einer echten NixOS-Anlage.** Die Paketnamen in
`flake.nix` stammen aus der Dokumentation von Tauri und nixpkgs, nicht
aus einem Lauf. Wer es zuerst benutzt, prüft sie und berichtigt sie
dort.

**Ohne Nix, aber mit Linux:** Der Bau selbst braucht nur die
Rust-Werkzeugkette und die Entwicklungspakete von WebKitGTK, GTK 3,
libsoup 3 und librsvg. Wer sie über seine eigene Paketverwaltung
holt, baut jede Kiste mit `cargo build --release` einzeln.

## Windows

**Braucht:** die Rust-Werkzeugkette und die C++-Buildwerkzeuge von
Visual Studio, denn Rust benutzt deren Linker.

```powershell
winget install --id Rustlang.Rustup
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Verweigert Windows die Ausführung, einmal für diese Sitzung:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
```

Die Programme landen unter `%LOCALAPPDATA%\Programs\Myelith\bin`, dazu
kommt ein Eintrag im Startmenü, und der Pfad wird für deinen Nutzer
ergänzt. Eine **neue** Sitzung sieht ihn.

⚠️ **SmartScreen meldet einen unbekannten Herausgeber**, denn die
Programme sind nicht signiert: *Weitere Informationen*, dann *Trotzdem
ausführen*.

## Was die Skripte ausdrücklich nicht tun

⚑ **Sie laden nichts nach.** Fehlt eine Werkzeugkette, nennen sie den
einen Befehl, mit dem sie zu holen ist, und hören auf. Ein
Installationsskript, das ein zweites aus dem Netz holt und ausführt,
ist die Angriffsfläche, gegen die dieses Projekt an jeder anderen
Stelle argumentiert.

⚑ **Sie bauen aus dem Quelltext und holen kein fertiges Bündel.** Die
Freigabebündel dieses Projekts sind **nicht signiert**; ein Skript,
das eines holt und am Gatekeeper vorbeischiebt, brächte einem Nutzer
bei, genau das zu tun. Wer aus dem Klon baut, baut aus dem, was er
lesen kann.

⚑ **`--aktualisieren` bewegt den Klon mit `git merge --ff-only`.** Geht
es nicht vorwärts, bricht es ab und sagt, warum. **Wer im Klon
gearbeitet hat, soll seine Arbeit nicht durch ein Installationsskript
verlieren.**

## Kein Modell, kein Artefakt

Ein frischer Klon enthält **keine Gewichte**: Sie sind zu gross für ein
Repositorium. Nach dem Einrichten steht deshalb noch kein Modell
bereit, und `myelith` sagt das beim Start.

**Geholt und gebaut wird im Fenster** (`myl-oberflaeche`), auf der
Einstellungsseite unter *Modelle*: Dort steht der Katalog mit Grösse,
Lizenz und Status, und der Balken zeigt, wie weit es ist. ⚠️ **Das
dauert Minuten bis Stunden und braucht Gigabyte**, und dafür braucht es
zusätzlich Python mit `huggingface_hub`; das Fenster prüft es vorher
und sagt, was fehlt.

## Aktualisieren

Aus dem Klon heraus:

```sh
sh INSTALL/installieren-macos.sh --aktualisieren
```

Oder im Fenster, auf der Einstellungsseite unter *Updates*: Es fragt,
ob `origin` Änderungen hat, die dieser Klon nicht hat, und spielt sie
auf Knopfdruck ein. ⚠️ **Ohne Klon geht das nicht**, und der Knopf sagt
es dann.
