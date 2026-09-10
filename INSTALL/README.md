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

Jedes kennt dieselben drei Schalter:

| Schalter | Wirkung |
|---|---|
| `--pruefen` (Windows: `-NurPruefen`) | Sagt nur, was fehlt, und baut nicht |
| `--aktualisieren` (Windows: `-Aktualisieren`) | Holt erst den neuen Stand, baut dann |
| `--system` (nur macOS) | Legt nach `/usr/local/bin` und `/Applications` statt unter das eigene Benutzerverzeichnis |

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
