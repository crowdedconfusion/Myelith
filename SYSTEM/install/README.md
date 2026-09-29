# Myelith einrichten

Ein Skript je System. Es **prüft zuerst, baut dann** und legt die
Programme ab. **Netz braucht es dafür nicht:** Alle Rust-Abhängigkeiten
liegen im Klon.

## Auf einen Blick

| System | Einmal vorher | Dann |
|---|---|---|
| **macOS** | `xcode-select --install` und [rustup](https://rustup.rs) | `sh SYSTEM/install/installieren-macos.sh` |
| **Linux mit Nix, NixOS** | `nix` | `sh SYSTEM/install/installieren-nixos.sh` |
| **Windows** (PowerShell) | Rust, die C++-Buildwerkzeuge von Visual Studio, Python (Befehle unten) | `.\SYSTEM\install\installieren-windows.ps1` |

Aufgerufen wird aus der Wurzel des Klons. Jedes Skript findet den Klon
über seinen eigenen Ort, ein anderes Verzeichnis geht also auch.

**Danach sind fünf Programme da:**

| Programm | wofür |
|---|---|
| `myelith` | der Agent in der Konsole: in ein Verzeichnis gehen, `myelith` tippen |
| `myl` | das Bedieninstrument: `myl frage`, `myl agent`, `myl einstellungen`, `myl sinne` |
| `myl-oberflaeche` | das Fenster, unter macOS zusätzlich `Myelith.app` |
| `myl-node` | der Knoten |
| `myl-test` | der Testclient für Hardware- und Determinismusläufe |

Welche Programme das sind, sagen die Kisten selbst (`ausliefern` unter
`[package.metadata.myelith]` in ihrer `Cargo.toml`); kein Skript führt
eine eigene Liste.

⚠️ **Ein Modell ist danach noch nicht da.** Ein Klon enthält keine
Gewichte, dafür sind sie zu gross. Wie eines dazukommt, steht unter
[Modelle](#modelle).

---

## Die Schalter

| macOS, Linux | Windows | Wirkung |
|---|---|---|
| `--pruefen` | `-NurPruefen` | nur sagen, was fehlt; nichts bauen |
| `--aktualisieren` | `-Aktualisieren` | erst den neuen Stand holen (`git merge --ff-only`), dann bauen |
| `--system` (nur macOS) | | nach `/usr/local/bin` und `/Applications` statt ins eigene Benutzerverzeichnis |
| `--in-der-shell` (nur Nix) | | nicht in `nix develop` wechseln, sondern die vorhandene Werkzeugkette nehmen |
| | `-Ziel <ordner>` | anderer Zielordner als `%LOCALAPPDATA%\Programs\Myelith` |

`--ohne-netz` (macOS) gibt es noch, es ist aber **nicht mehr nötig**:
Liegt der Vorrat im Klon, baut das Skript von sich aus ohne Netz.

---

## Je System

### macOS

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sh SYSTEM/install/installieren-macos.sh
```

Die Programme landen in `~/.local/bin`, das Bündel in `~/Applications`.
Steht `~/.local/bin` nicht im `PATH`, nennt das Skript die Zeile für
deine Shell-Datei.

⚠️ **Beim ersten Start meldet Gatekeeper einen unbekannten Entwickler**,
denn das Bündel ist nicht signiert: einmal Rechtsklick auf
`Myelith.app`, *Öffnen*, bestätigen.

### Linux mit Nix, NixOS

```sh
sh SYSTEM/install/installieren-nixos.sh
```

Alles Weitere steht in `flake.nix` in der Wurzel; das Skript ruft sich
dafür in `nix develop` noch einmal auf. Die Programme landen unter
`~/.local/bin`, der Menüeintrag unter `~/.local/share/applications`.

⚑ **Es ändert nichts an deiner Systemkonfiguration.** Ein Skript, das
`/etc/nixos/configuration.nix` anfasst, nähme einem NixOS-Nutzer genau
das weg, wofür er NixOS benutzt.

**Linux ohne Nix:** Es braucht die Rust-Werkzeugkette und die
Entwicklungspakete von WebKitGTK 4.1, GTK 3, libsoup 3 und librsvg (unter
Debian und Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev
libayatana-appindicator3-dev librsvg2-dev libxdo-dev`). Dann jede Kiste
einzeln mit `cargo build --release --locked`.

### Windows

```powershell
winget install --id Rustlang.Rustup
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Python.Python.3.12
.\SYSTEM\install\installieren-windows.ps1
```

Verweigert Windows die Ausführung, einmal für diese Sitzung:
`Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`.

Die Programme landen unter `%LOCALAPPDATA%\Programs\Myelith\bin`, dazu
ein Eintrag im Startmenü, und der Pfad wird für deinen Nutzer ergänzt.
Eine **neue** Sitzung sieht ihn.

⚠️ **Python ist nötig**, weil es den Vorrat auspackt; fehlt es, bricht
das Skript vor dem Bauen mit genau diesem Satz ab. ⚠️ **SmartScreen meldet einen unbekannten
Herausgeber**, denn die Programme sind nicht signiert: *Weitere
Informationen*, dann *Trotzdem ausführen*.

---

## Ohne Netz: was geht und was nicht

| Vorhaben | ohne Netz? | braucht |
|---|---|---|
| **Die fünf Programme bauen** | ✅ ja | Werkzeugkette (siehe oben); der Vorrat liegt im Klon |
| **Den Agenten betreiben** | ✅ ja | ein Artefakt unter `INTEGER_LLM/artifacts/` |
| **Konformität und Messungen** | ✅ ja | dasselbe |
| **Bücher zu Korpus oder Mappe** | ✅ ja | Python 3.9; die Räder liegen im Klon |
| **Artefakte aus Gewichten bauen** | nur wenn schon da | Gewichte unter `MODELS/llm/`, Python mit torch |
| **Gewichte holen** | ❌ nein | Netz zu Hugging Face |
| **Sehen, Hören, Sprechen einrichten** | ❌ nein | Netz für Programme und kleine Modelle |

### Wie gut das belegt ist

| | Beleg |
|---|---|
| **Bauen ohne Netz, alle drei Systeme** | ✅ Die CI baut bei **jedem Push** alle fünf Programme aus einem frischen Klon, ohne Netz, auf macOS, Ubuntu und Windows (Job „Kalter Klon ohne Netz“), und startet danach `myl`. Grün seit dem 2026-09-29 |
| **Das macOS-Skript** | ✅ von Anfang bis Ende gemessen (2026-09-17): frischer Klon, leerer Cargo-Vorrat, Netz blockiert, alle fünf Programme samt Bündel in 1 Minute 57 |
| **Das Nix-Skript** | ⚠️ eingebaut und in Teilen ausgeführt, **auf einer echten NixOS-Anlage noch nicht gelaufen**. Die Paketnamen in `flake.nix` stammen aus der Dokumentation von Tauri und nixpkgs |
| **Das Windows-Skript** | ⚠️ **noch nicht auf Windows gelaufen**. Bis zum 2026-09-29 nannte es zwei Ordner, die es seit einem Umzug nicht mehr gab; eine Probe prüft seitdem auf jedem System, dass jeder Pfad in den drei Skripten im Klon liegt |

⚠️ **Die CI prüft den Bau, nicht die Skripte.** Sie packt den Vorrat mit
`vorrat.py` selbst aus und ruft `cargo` direkt. Dass ein Skript auf
seinem System durchläuft, ist damit nicht gesagt.

⚠️ **Was der Vorrat nicht abdeckt:**
- **Unter Linux die Systembibliotheken des Fensters** (WebKitGTK und
  Nachbarn). Die kommen aus der Paketverwaltung.
- **Unter Nix die Nix-Eingaben.** `nix develop` holt die Werkzeugkette,
  wenn sie nicht schon im Store liegt. Ohne Netz also nur mit warmem
  Store oder mit `--in-der-shell`.

---

## Der Vorrat: für alle, die Abhängigkeiten ändern

Die Rust-Abhängigkeiten liegen als **ein Archiv je Paket** unter
`SYSTEM/crates-vorrat/` (rund 760 Dateien, 113 MB). Beim Einrichten werden
sie nach `SYSTEM/crates-lager/` ausgepackt, und das ist nicht versioniert.
Gebaut wird immer mit `--locked`, aus dem Vorrat zusätzlich mit
`--offline`.

```sh
python3 SYSTEM/install/vorrat.py pruefen    # jedes Archiv gegen die SHA-256 in den Sperrdateien
python3 SYSTEM/install/vorrat.py sammeln    # nach einer Änderung an einer Abhängigkeit
python3 SYSTEM/install/vorrat.py auspacken  # macht das Installationsskript selbst
```

⛔️ **Nach jeder Änderung an einer Abhängigkeit muss `sammeln` laufen**,
sonst fehlt ein Archiv. Das fällt dann auf: `pruefen` nennt das
fehlende Paket, und der Bau bricht mit seinem Namen ab, statt still
etwas Altes zu nehmen. `sammeln` holt zuerst aus `~/.cargo/registry` und
erst dann aus dem Netz, und prüft jede Datei sofort.

<details>
<summary>⚑ Warum Archive je Paket und kein ausgepackter Vorrat</summary>

Dieselben 758 Pakete, drei Wege, gemessen am 2026-09-17:

| Weg | im Git | Dateien | wächst je Fassungssprung um |
|---|---|---|---|
| `cargo vendor`, ausgepackt | 202 MB | 36 805 | die geänderten Pakete |
| ein gepacktes Bündel | 122 MB | 1 | **jedes Mal 122 MB** |
| **Archive je Paket** | **113 MB** | **758** | **nur die neuen Archive** |

Nur der letzte Weg wächst nicht mit: Jede Paketfassung ist eine eigene,
unveränderliche Datei. Ein Bündel ist bei jeder Änderung ein neues, und
die alten bleiben für immer in der Geschichte.

Für Windows liegen rund 60 eigene Pakete darin, samt dem Tauri-Unterbau
(`webview2-com-sys`); der längste ausgepackte Pfad hat 153 Zeichen und
bleibt mit einem gewöhnlichen Klonpfad unter der Grenze von 260.

</details>

---

## Modelle

**Im Fenster** (`myl-oberflaeche`), Einstellungen, *Modelle*: der
Katalog mit Grösse, Lizenz und Status. Holen und Bauen gehen dort per
Knopf, mit Ladebalken. ⚠️ Das dauert Minuten bis Stunden, braucht
Gigabyte und Python mit `huggingface_hub`; das Fenster prüft das vorher.

**Auf der Kommandozeile**, zwei Schritte, der erste mit Netz:

```sh
sh INTEGER_LLM/scripts/fetch_model.sh      # Gewichte holen, feste Revision
sh INTEGER_LLM/scripts/build_artifacts.sh  # kalibrieren und exportieren nach INTEGER_LLM/artifacts/
```

Der Export braucht Python mit `torch` und `transformers`, einmalig
eingerichtet (mit Netz, einige Gigabyte):

```sh
cd INTEGER_LLM/calibrate
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python3 -r requirements.txt
```

Das Skript findet die Umgebung selbst (`$PYTHON`, dann
`calibrate/.venv/bin/python3`, dann `python3`) und sagt, was fehlt.

⚑ **Der Betrieb braucht davon nichts.** Wer ein fertiges Artefakt hat,
braucht weder torch noch Python: Der Ganzzahlpfad ist Rust. ⚑ **Und der
Bau ist wiederholbar**, gemessen am 0,6B: dieselben Gewichte ergeben
48/48 Konformitätsvektoren und denselben `decode_digest`, weil das
Skalenpaket im Repositorium liegt.

---

## Sehen, Hören und Sprechen (freiwillig)

Eine Erweiterung, keine Bedingung: Ohne sie baut und läuft alles.

```sh
sh SYSTEM/install/sinne-einrichten.sh                 # alles
sh SYSTEM/install/sinne-einrichten.sh --ohne-sprechen # nur Sehen und Hören
sh SYSTEM/install/sinne-einrichten.sh --pruefen       # nur nachsehen
myl sinne                                             # was dieser Rechner jetzt kann
myl sinne <datei>                                     # eine Datei hindurchschicken
```

| | woher | wohin |
|---|---|---|
| ffmpeg, llama.cpp, whisper.cpp | Paketverwalter des Systems | `/opt/homebrew/bin` und ähnliche |
| Hörmodell (0,6 GB) | whisper.cpp auf Hugging Face | `~/.myelith/sinne/hoeren.bin` |
| Sehmodell schnell (1,7 GB) | SmolVLM2-2.2B-Instruct | `sehen.gguf`, `sehen-mmproj.gguf` |
| Sehmodell genau (2,9 GB) | Qwen3-VL-4B-Instruct | `sehen-genau.gguf`, `sehen-genau-mmproj.gguf` |
| Sprechen (4,5 GB) | Fun-CosyVoice3-0.5B mit eigener Python-Umgebung | `~/CosyVoice`, verlinkt nach `~/.myelith/sinne/cosyvoice` |

⛔️ **Programme und Gewichte liegen bewusst nicht im Repositorium**: je
System verschieden, zusammen einige Gigabyte. Der Klon bringt nur das
Verbindungsstück mit (dieses Skript, den Läufer für CosyVoice, die Namen,
unter denen der Client sucht). **Deshalb braucht dieser eine Schritt
Netz.**

<details>
<summary>📌 Vier Stolpersteine, alle beim Einrichten gefunden</summary>

- **Ein aus dem Finder gestartetes `Myelith.app` erbt einen PATH ohne
  `/opt/homebrew/bin`.** Ein mit `brew` installiertes llama.cpp wäre im
  Terminal da und im Fenster nicht. Der Client sucht deshalb zusätzlich an
  den üblichen Orten.
- **CosyVoice braucht seine eigene Python-Umgebung.** Der erste Python im
  PATH ist unter macOS 3.9 ohne torch. Der Client nimmt den Interpreter aus
  der `.venv` neben der CosyVoice-Installation.
- **setuptools 81 nahm `pkg_resources` aus der Bauumgebung**, woran
  `openai-whisper==20231117` scheitert. Das Skript nimmt setuptools zurück
  und baut dieses Paket ohne eigene Bauumgebung.
- **Die Reihenfolge von pip:** Wer `openai-whisper` zuerst installiert,
  holt unbestimmte Fassungen von torch und numpy herein, und pip läuft
  danach minutenlang rückwärts durch hunderte Fassungen. Erst die
  gepinnten, dann das eine Paket.

⛔️ **Deutsch kann erst CosyVoice 3.** Die 2.0-Gewichte decken Chinesisch
und Englisch ab; eine deutsche Stimmprobe ergab Laute, die wie Deutsch
klingen und keines sind (gemessen am 2026-09-17: vorgegeben „dies ist die
erste gesprochene Antwort“, gehört „dies Go! Ist die Örsteck ist
proschein“).

</details>

---

## Freigaben des Betriebssystems

**Ein installiertes Programm ist noch keine Erlaubnis.** Mikrofon, Kamera
und Bildschirm gibt jedes System erst heraus, wenn ein Mensch zustimmt.
Was fehlt, fehlt nie als Download, sondern als Häkchen.

⚠️ **Zwei Stufen, beide müssen stimmen:** die Freigabe des Systems und
das Häkchen im Client (Einstellungen, *Agent*: „Bildschirm ansehen
dürfen“, „Kamera ansehen dürfen“; als Umgebungsvariable
`MYL_BLICK_BILDSCHIRM`, `MYL_BLICK_KAMERA`).

| | macOS | Linux | Windows |
|---|---|---|---|
| **Mikrofon** | Datenschutz und Sicherheit, *Mikrofon* | Gruppe `audio` | Datenschutz, *Mikrofon* |
| **Kamera** | dort, *Kamera* | Gruppe `video` | dort, *Kamera* |
| **Bildschirm** | dort, *Bildschirmaufnahme*, einmal von Hand | X11: nichts; **Wayland: Portal nötig** | nichts |
| **Dateien ausserhalb des Benutzerverzeichnisses** | *Festplattenvollzugriff* | Dateirechte | Dateirechte |
| **Web-Recherche** | `curl` (ist da) | `curl` aus der Paketverwaltung | `curl.exe` ab Windows 10 |
| **PDF lesen** | `brew install poppler` oder `pip install pypdf` | `apt install poppler-utils` | poppler für Windows oder `pip install pypdf` |

⚑ **Die Web-Recherche ist die einzige Zeile, bei der das System nicht
fragt.** Deshalb sitzt die Schranke im Client: Das Häkchen „Im Web
recherchieren dürfen“ unter *Agent* ist aus, bis jemand es setzt, und
gilt dann für **Chat und Agent**. Fremder Seitentext kommt gekennzeichnet
als Inhalt zurück, nie als Anweisung.

<details>
<summary>Einzelheiten je System</summary>

**macOS.** Mikrofon und Kamera kündigt das Bündel an, macOS fragt beim
ersten Gebrauch. Für die Bildschirmaufnahme gibt es keinen solchen
Schlüssel; ohne Freigabe meldet `screencapture` „could not create image
from display“, und der Client nennt den Weg. **Im Terminal gilt eine
Erlaubnis dem Terminal**, nicht `myelith`; wer `Myelith.app` startet,
gibt das Bündel frei. Das Bündel ist ad hoc signiert, damit macOS es über
einen Neubau hinweg wiedererkennt.

**Linux.** `sudo usermod -aG audio,video "$USER"`, danach neu anmelden.
Unter Wayland gibt erst ein Portal den Bildschirm heraus, mit einem Dialog
je Aufnahme. `MYL_SCHIRMFORMAT` und `MYL_SCHIRMGERAET` sagen ffmpeg,
woher es nehmen soll.

**Windows.** Unter Datenschutz zusätzlich „Desktop-Apps den Zugriff
erlauben“, sonst gilt die Freigabe nur für Store-Anwendungen.
`myelith --root` ist keine Freigabe: Es verschiebt die Einhängegrenze des
Agenten und startet sich dafür mit Verwalterrechten neu.

**PDF:** Gesucht wird `MYL_SCHRIFTLESER`, dann `pdftotext`, dann
`mutool`, zuletzt ein Python mit `pypdf` oder `fitz`. Fehlt alles, fehlt
nur das Lesen von PDF, und `myl sinne` zeigt es in der Zeile `Schrift`.

</details>

---

## Aktualisieren

```sh
sh SYSTEM/install/installieren-macos.sh --aktualisieren
```

Oder im Fenster, Einstellungen, *Updates*: Es fragt, ob `origin`
Änderungen hat, die dieser Klon nicht hat, und spielt sie auf Knopfdruck
ein. ⚠️ Ohne Klon geht das nicht, und der Knopf sagt es.

---

## Was die Skripte bewusst nicht tun

- **Sie laden nichts nach.** Fehlt eine Werkzeugkette, nennen sie den
  Befehl, mit dem sie zu holen ist, und hören auf. Ein Skript, das ein
  zweites aus dem Netz holt und ausführt, ist genau die Angriffsfläche,
  gegen die dieses Projekt anderswo argumentiert.
- **Sie holen kein fertiges Bündel.** Die Freigabebündel sind nicht
  signiert; ein Skript, das eines am Gatekeeper vorbeischiebt, brächte
  genau das bei. Wer aus dem Klon baut, baut aus dem, was er lesen kann.
- **`--aktualisieren` geht nur vorwärts** (`git merge --ff-only`). Wer im
  Klon gearbeitet hat, verliert seine Arbeit nicht durch ein
  Installationsskript.
