# GolemOS bauen

> **Stufe 1:** Kern, glibc, BusyBox, `curl`, und darauf `myelith` und
> `myl`. Kein Desktop, kein Bootloader.
> **Stand: gebaut, gestartet, gemessen (2026-09-24), beide Architekturen.
> Myelith antwortet darin, und ein Assistent installiert es auf eine
> Platte (2026-09-25, aarch64). Fassung und Changelog:
> [`README/README.md`](README/README.md).**
>
> **Wer GolemOS nur benutzen will**, liest die
> [Anleitung](ANLEITUNG.md) ([English](ANLEITUNG.en.md)); diese Datei
> erklärt, wie es gebaut wird.

---

## ⛔️ Die Antwort: es läuft

```
[    1.184599] Run /init as init process

════════ GolemOS Selbstprobe ════════
Kern:     Linux 6.19.14 aarch64
System:   GolemOS
Speicher: 965 MB

  myelith  7.9M  vorhanden
  myl  7.7M  vorhanden

── myl ort ──
myl ort: kein Klon gefunden.

── curl ──
curl 8.20.0 (aarch64-buildroot-linux-gnu) libcurl/8.20.0 OpenSSL/3.6.2
═════════════════════════════════════
PROBE_FERTIG
[   17.651537] reboot: Power down
```

Der Kern startet in **1,18 Sekunden**, das Init läuft, beide Programme
sind da und antworten. `myl ort` sagt korrekt „kein Klon gefunden", denn
es liegt keiner im Abbild. `curl` spricht TLS über OpenSSL.

**Und dasselbe auf x86_64:**

```
Kern:     Linux 6.19.14 x86_64
  myelith  8.0M  vorhanden
  myl  7.8M  vorhanden
curl 8.20.0 (x86_64-buildroot-linux-gnu) libcurl/8.20.0 OpenSSL/3.6.2
PROBE_FERTIG
```

⚑ **Für x86_64 braucht es keinen Kreuzübersetzer.** Im README stand
lange, die Docker-VM auf Apple-Silizium sei aarch64 und `blst`
übersetze C, also fehle ein musl-Kreuzübersetzer. **Das war falsch:**
`docker run --platform linux/amd64` gibt einen echten x86_64-Behälter,
und darin ist alles nativ, `g++` eingeschlossen (gemessen: gcc 15.2.0,
Rust-Wirt `x86_64-alpine-linux-musl`). Das Skript wählt die Plattform
aus der Zielarchitektur.

📌 **Ein Fehler traf nur x86_64:** Der x86-Kern baut `objtool`, ein
Wirtswerkzeug, und das verlangt `gelf.h` aus libelf. Nach 85 870 Zeilen
Bau: `compilation terminated`. Der aarch64-Kern kennt `objtool` nicht,
deshalb fiel es dort nicht auf. ⚑ **Die Abhilfe ist eine Zeile
Konfiguration und kein Paket im Behälter**
(`BR2_LINUX_KERNEL_NEEDS_HOST_LIBELF`): Damit hängt der Bau an dem, was
im Repositorium liegt, und nicht an der Ausstattung des Wirts.

| | |
|---|---|
| Kern `Image` | 41,2 MB |
| Dateisystem `rootfs.cpio.gz` | **31,8 MB** |
| entpackt | 112 MB, davon 84 MB Kernmodule |
| unsere Programme | 7,9 MB und 7,7 MB, **statisch** |
| Bau | rund 35 Minuten auf 15 Fäden |

⚠️ **Die 84 MB Kernmodule sind die verbleibende Schuld**, und sie ist
benennbar: Die Architekturvorgabe baut Treiber für alles, was je ein
ARM-Gerät war, `nouveau` und Tegra-Sound eingeschlossen. **Das ist der
Preis dafür, dass das Abbild auf unbekannter Hardware startet.** Wer
eine Zielmaschine benennt, bekommt eine eigene Kernkonfiguration und
damit 30 bis 55 MB; wer keine benennt, zahlt die Treiberbreite. Eine
Entscheidung, keine Nachlässigkeit.

## ⛔️ Stufe 2a: es startet von Platte

```
BdsDxe: starting Boot0002 "UEFI Misc Device"
GNU GRUB  version 2.12
   *GolemOS (A)
    GolemOS (B)
    GolemOS Selbstprobe

EXT4-fs (vda2): mounted filesystem ... ro with ordered data mode
Run /sbin/init as init process
EXT4-fs (vda2): re-mounted ... r/w
```

**EFI → GRUB → Kern → `golem-wurzelA` über seine Kennung → Init → Wurzel
schreibbar.** `golemos.img`: 896 MB Nenngrösse, **98 MB belegt**.

| Partition | Marke | Inhalt |
|---|---|---|
| 1 | (vfat, EFI) | GRUB und der Kern |
| 2 | `golem-wurzelA` | GolemOS |
| 3 | `golem-wurzelB` | im Abbild leer, beim ersten Start aus A gefüllt |
| 4 | `golem-daten` | wird beim ersten Start gedehnt und mit exFAT formatiert |

⚑ **`wurzelB` kommt leer aus dem Abbild** (seit 0.6.1). Belegt lag jede
Wurzel zweimal darin, gepackt über der Grenze von 100 MiB je Datei im
Repositorium. Vor der ersten Aktualisierung braucht B nur die
Installation, die die ruhende Seite kopiert; deshalb füllt `S00seiteb`
sie beim ersten Start aus A, solange die Wurzel noch schreibgeschützt
ist. Den Rückfallweg einer Aktualisierung trägt ohnehin A: Sie schreibt
B neu, und A bleibt, wie es war.

### 📌 Zwei fremde Fehler, die wie eigene aussahen

**Erster Startversuch:** GRUB-Menü, „Booting `GolemOS (A)'", dann
nichts. Das sah aus wie ein Absturz und war keiner: Ohne `console=`
schreibt der Kern auf `tty1`, also auf einen Bildschirm, den eine
Maschine ohne Grafik nicht hat. ⚑ **Linux schreibt auf alle genannten
Konsolen und macht die letzte zu `/dev/console`.** Daraus folgt die
Aufteilung: A und B nennen `tty1` zuletzt (wer einen Bildschirm hat,
bekommt dort die Anmeldung), die Selbstprobe nennt `ttyS0` zuletzt
(eine Diagnose liest man seriell, oft ohne Bildschirm). Auf aarch64
heißt die serielle Schnittstelle `ttyAMA0`; seit Fund 477 setzt der Bau
sie je Architektur ein.

**Zweiter Startversuch:** Kernpanik in `mp_irqdomain_alloc`, also beim
IO-APIC. ⚠️ **Auch das war nicht GolemOS**, sondern QEMUs
Vorgabemaschine `i440fx` von 1996, die sich mit OVMF beim
Interruptcontroller nicht verträgt. Mit `-M q35` startet dieselbe Platte
sauber durch.

⚑ **Zweimal hintereinander sah ein fremdes Problem wie ein eigenes
aus**, und beide Male lag die Abhilfe im Prüfstand und nicht im
Erzeugnis. **Ein Fehler im Prüfstand als Fund am Erzeugnis gebucht wäre
schlimmer als ein übersehener.**

### Von Platte starten

```sh
qemu-system-x86_64 -M q35 -m 1024 -nographic -no-reboot -nic none \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
  -drive file=SYSTEM/golemos/aus/x86_64/golemos.img,format=raw,if=virtio
```

⚠️ **`-M q35` ist nicht optional**, siehe oben.

---

## ⛔️ Stufe 2b: Myelith antwortet in GolemOS

```
── Datenpartition ──
  2.2G gross, 1.3G frei
  artefakte        1 Eintraege
  werkzeugkisten   4 Eintraege
  myelith/skills   0 Eintraege
  client.json      gefunden

── Antwortet das Modell? ──
  Artefakt: sparrow-0.6b-int8
  Frage:    Nenne die Hauptstadt von Frankreich.
  Antwort:  Die Hauptstadt von Frankreich ist **Paris**.
            [myl] 21 Token hinein, 11 heraus, 11.2 s (1.0 Token/s)

  Rueckgabe: 0, Paris genannt, ANTWORT_OK
PROBE_FERTIG
```

Gemessen am 2026-09-25 unter QEMU ohne Beschleunigung (TCG), daher
1 Token/s; auf dem Wirt sind es mit demselben Artefakt 19. Der Weg:

| Schritt | Was |
|---|---|
| 1 | Abbild bauen, die Platte auf 3 GB vergrössern |
| 2 | einmal starten: `S05daten` dehnt `golem-daten` und formatiert sie |
| 3 | `sh SYSTEM/golemos/mitnahme.sh --artefakt sparrow-0.6b-int8`, dann den Inhalt von `bau/mitnahme/` auf Partition 4 kopieren (`mcopy -s` mit dem Versatz aus `sfdisk`) |
| 4 | mit `golemos.probe` starten |

⚑ **Die Antwort ist bitgleich zu der auf dem Wirt.** Dieselbe Frage mit
denselben Einstellungen, einmal nativ auf macOS (NEON), einmal im
statisch gegen musl gebauten `myl` unter emuliertem aarch64: Zeichen für
Zeichen dieselbe Ausgabe, auch bei eingeschaltetem Denken über alle 24
Token. Das ist die Zusage der ganzzahligen Rechnung, hier an einem
fremden Betriebssystem nachgesehen.

⛔️ **Fund 473: Die Probe meldete zuerst `ANTWORT_OK` ohne eine
Antwort.** Die Mitnahme übernimmt `modell.denken` vom Wirt, und dort
stand es an. Alle 24 Token waren Überlegung, die Rückgabe war null, und
die Zeile sagte „gut". Jetzt fragt die Probe mit einer Kopie der
Einstellungen ohne Denken, und sie verlangt „Paris" in der Antwort.
📌 **Eine Rückgabe null belegt, dass ein Programm endete, nicht, dass es
das Richtige sagte.**

### Selbst nachstellen

```sh
sh SYSTEM/golemos/bauen.sh --arch aarch64

qemu-system-aarch64 -M virt -cpu cortex-a57 -m 2048 -nographic -no-reboot \
  -nic none \
  -kernel SYSTEM/golemos/aus/aarch64/Image \
  -initrd SYSTEM/golemos/aus/aarch64/rootfs.cpio.gz \
  -append "console=ttyAMA0 golemos.probe" \
  -drive file=golemos.img,format=raw,if=virtio
```

⚠️ **Die Erzeugnisse liegen je Architektur in `aus/<arch>/`.** Hier und
im Schlusstext von `bauen.sh` stand noch `aus/Image`, dort sogar der
Befehl für x86_64 nach einem aarch64-Bau (Fund 472). Der Schlusstext
wird jetzt aus der gebauten Architektur gebildet.

⚠️ **2 GB Speicher**, sobald ein Modell mitläuft; das 0,6B-Artefakt
allein ist 0,9 GB.

⚑ **`golemos.probe` auf der Kernzeile** lässt `etc/init.d/S99probe`
anspringen: Es druckt den Zustand und schaltet ab. Ohne das Wort startet
dasselbe Abbild normal in eine Konsole.

⚠️ `-nic none`, weil das schlanke QEMU-Paket kein `efi-virtio.rom`
mitbringt. Mit Netz braucht es `ipxe-qemu` dazu.

---

## Der erste Schritt war eine Frage, kein Abbild

⛔️ **Laufen unsere Programme auf einem Buildroot-System?** Das ist keine
Selbstverständlichkeit: Sie werden sonst gegen macOS gebaut, dort gegen
ein frisch übersetztes glibc, und `myelith` bringt eine Kiste mit, die
C übersetzt (`blst`).

Deshalb erzeugt Stufe 1 **kein installierbares Abbild**, sondern Kern
und Speicherdateisystem. Die lassen sich ohne Bootloader in einer
virtuellen Maschine starten, in Minuten statt Stunden. ⚑ **Erst die
Frage, die alles kippen kann, dann das Handwerk.**

```sh
sh SYSTEM/golemos/bauen.sh
```

Und darin:

```
myelith --hilfe
```

Kommt eine Zeile, ist der Rest Handwerk.

---

## Wie gebaut wird

| Schritt | Was |
|---|---|
| 1 | `myelith` und `myl`, **statisch gegen musl** |
| 2 | Buildroot holen, Fassung fest gesetzt |
| 3 | konfigurieren **und nachzählen** |
| 4 | bauen |
| 5 | Kern und Dateisystem nach `aus/` |
| 6 | `auskunft.sh`: Lizenzauskunft und Quellen nach `aus/`, ohne Netz |

⚑ **Statisch gegen musl, obwohl das System glibc trägt.** Dann hängen
unsere Programme an gar nichts: kein Versionsabgleich mit dem frisch
gebauten glibc, kein nachträgliches Brechen, wenn Buildroot seine
Fassung hebt. **Ein Programm, das nichts braucht, kann auch nichts
vermissen.**

⛔️ **Schritt 3 ist der, den sonst niemand macht.** Buildroot verschluckt
unbekannte Namen in einer Konfiguration **wortlos**: Ein Tippfehler ist
dann ein Merkmal, das einfach fehlt, und es fällt erst auf, wenn das
Abbild etwas nicht kann. Das Skript zählt jede Zeile der Konfiguration
gegen die erzeugte `.config` und bricht ab, wenn eine fehlt.
⚑ **Schweigen sieht aus wie Erfolg**, und das ist in diesem Projekt
schon teuer genug gewesen.

⛔️ **Schritt 6 gehört vor jede Freigabe.** Das Abbild enthält Teile unter
der GPL (Kern, BusyBox, GRUB, Werkzeuge), und wer es weitergibt, gibt die
Quellen mit. `auskunft.sh` sammelt mit `make legal-info` ohne Netz das
Verzeichnis der Pakete, ihre Lizenztexte und ihre Quellen (Basis und
Schicht, dazu Buildroot selbst, das seine eigenen Quellen nicht beilegt).
`freigeben.sh` verweigert ein Abbild, zu dem diese Auskunft fehlt oder
älter ist als das Abbild; die kleine Auskunft kommt neben das Abbild, die
Quellen (rund 0,7 GB je Architektur) gehen als Anhang an dieselbe
Freigabe. ⚠️ Buildroot nennt seine Auskunft selbst keine fertige
Lieferung: Die Lizenzangaben der Pakete sind nicht vollständig geprüft.

---

## ⚠️ Was noch offen ist, ehrlich benannt

| Punkt | Stand |
|---|---|
| **Volumenplatz** | rund 13 GB je Architektur, zusammen 27 GB. Kein Problem, aber es überrascht sonst |
| **Installateur** | ✅ `golem-einrichten`: Assistent bei der ersten Anmeldung, Installation auf eine ganze Platte, Myelith-Ordner auf dem Stick statt Mitnahme. Belegt am 2026-09-25 unter QEMU mit UEFI (aarch64), noch nicht auf echter Hardware |
| **Mitnahme** | ✅ `mitnahme.sh` sammelt Werkzeugkisten, Wissensmappen, Artefakte und die Einstellungen mit Pfaden unter `/daten`; belegt am 2026-09-25 mit dem 0,6B-Artefakt |
| **Ohne Netz** | Der erste Bau braucht Netz: die beiden Bauumgebungen (`Containerfile`, `Containerfile.programme`), Buildroot selbst und seine Quellen (Kern, BusyBox, `curl`, GRUB). Danach nicht mehr: Die Programme bauen aus dem Vorrat in einem Behälter ohne Netz, und die Quellen liegen im Volumen (rund 1,7 GB je Architektur). Nachgesehen am 2026-11-10: `make source` ohne Netz gelingt für Basis und Schicht beider Architekturen, mit leerem Quellenlager scheitert es. Eine geänderte Konfiguration mit neuen Paketen braucht wieder Netz |
| **A/B-Umschaltung** | ✅ `golem-aktualisieren`: die neue Seite einmal zur Probe, bestätigt am Ende eines gesunden Starts, sonst zurück zur alten; ein Kern je Seite. Belegt am 2026-11-13 unter QEMU mit UEFI (aarch64 und x86_64), noch nicht auf echter Hardware. ⚠️ Pakete tragen eine Prüfsumme, keine Signatur |
| **Kernmodule** | 84 MB Treiber für Hardware, die nie da sein wird. Braucht eine eigene Kernkonfiguration und damit eine benannte Zielmaschine |
| **`alpine:edge`** | rollendes Ziel; das Abbild `golemos-programmbauer-<arch>` hält eine Fassung fest, bis es gelöscht wird (am 2026-11-10 Rust 1.99.1, GCC 15.2.0). Festnageln, sobald eine veröffentlichte Alpine-Fassung die Rust-Fassung der Kisten trägt |
| **Secure Boot** | GolemOS ist nicht signiert; wer es starten will, schaltet Secure Boot aus. Zwei Wege, beide offen: ein fremdes, von Microsoft signiertes `shim` mit einem eigenen Schlüssel, den jeder Rechner einmal eintragen muss (MOK), oder ein eigenes `shim` über die öffentliche Prüfung (`rhboot/shim-review`), seit dem 2026-06-27 nur noch mit der Microsoft UEFI CA 2023 signiert. Beide brauchen einen signierten Kern, und der schützt nur, wenn er auch nur signierte Module lädt |
| **Kernkonfiguration** | die Vorgabe der Architektur. Für eine bestimmte Zielmaschine gehört eine eigene her |
| **Macs mit Apple-Chip** | vorgemerkt über Asahi Linux (Wunsch des Projektinhabers, 2026-09-26). Der Asahi-Installer kann nur eine UEFI-Umgebung einrichten (rund 3 GB, dazu hält er 38 GB für macOS frei), die danach jeden USB-Stick mit `EFI/BOOT/BOOTAA64.EFI` startet; das aarch64-Abbild trägt `bootaa64.efi` dort bereits. **Es fehlt:** ein Kern aus dem Asahi-Baum mit Apples Treibern und der Firmware, die der Installer aus macOS zieht, also ein drittes Abbild. **Es hält auf:** Asahi unterstützt M1 bis M3, M4 und M5 noch nicht, und die Maschine hier ist ein M5; getestet werden könnte es nur auf fremder Hardware. Die Alternative ohne Asahi wäre eine virtuelle Maschine über Apples Virtualization-Framework, auf jedem Apple-Chip |
| **Die Sinne** | Stufe 3. `ffmpeg` allein ist grösser als das ganze Grundsystem. Die Lizenzpflichten beim Mitbündeln stehen im nächsten Abschnitt |

### ⚑ Die Sinne im Abbild: was die Lizenzen verlangen (festgehalten 2026-11-12)

Myelith ruft ffmpeg und ein fremdes Sprechprogramm als **eigene Programme**
auf, gebunden wird nichts. Die eigene Lizenz berührt das nicht. Wer sie
aber ins Abbild legt, gibt sie weiter, und dann gilt ihre Lizenz für diese
Weitergabe.

| Teil | Lizenz | Was die Weitergabe verlangt | Weg |
|---|---|---|---|
| **ffmpeg** | LGPL 2.1 oder später, **solange ohne `--enable-gpl` und `--enable-nonfree` gebaut**; mit `--enable-gpl` wird das ganze Programm GPL, mit `--enable-nonfree` darf es gar nicht weitergegeben werden | Lizenztext, Hinweise, die Quellen am selben Ort wie das Abbild | als Buildroot-Paket ohne `BR2_PACKAGE_FFMPEG_GPL` und `BR2_PACKAGE_FFMPEG_NONFREE`; dann sammelt `auskunft.sh` Quellen und Lizenztext wie bei jedem anderen Paket. Nur die Leser und Geräte, die der Client braucht (AAC, MP3, Opus, Vorbis, FLAC; ALSA, V4L2), das hält es klein |
| **piper** | GPL 3.0 seit dem Umzug nach `OHF-Voice/piper1-gpl`; das frühere MIT-Repositorium ist seit 2025-10-06 archiviert. Es bringt espeak-ng mit (GPL 3.0 oder später) | Quellen oder ein schriftliches Angebot, Lizenztext; bei einem Gerät für Verbraucher auch die Angaben, wie man eine geänderte Fassung installiert (GPL 3.0 §6). GolemOS sperrt nichts, das ist also erfüllt | **kein** Buildroot-Paket: Es bräuchte ein eigenes, sonst erfasst `legal-info` es nicht. ⚠️ Die Stimmen haben je eine eigene Lizenz, manche nur für nicht kommerzielle Nutzung; jede einzeln prüfen |

⚑ **Vorschlag:** Stufe 3 ohne piper. Gesprochen wird im Abbild über den
eigenen Weg (Qwen3-TTS, Apache 2.0, Gewichte vom Einrichtungsskript
nachgeladen, nicht im Abbild). piper erst, wenn schwache Hardware es
braucht, dann als eigenes Buildroot-Paket und nur mit Stimmen unter freier
Lizenz. Für Formate und Aufnahme braucht der Client ffmpeg immer weniger
(ein WAV wandelt er selbst, die Aufnahme hat eine eigene Anbindung je
System); jedes Format, das er selbst liest, macht ffmpeg im Abbild kleiner.
Keine Rechtsberatung: Geprüft sind die Angaben der Projekte selbst
(ffmpeg.org/legal.html, Repositorium `piper1-gpl`).

---

## Warum kein Paketverwalter, und was stattdessen kommt

Ohne Paketverwaltung ist jede Sicherheitslücke ein neues Abbild.
⚑ **Die Antwort heisst A/B und ist besser als ein Aktualisierungslauf:**
zwei Wurzelpartitionen, das neue Abbild geht auf die inaktive, dann
wird umgeschaltet und neu gestartet. Geht dabei etwas schief, startet
die alte. **Eine unterbrochene Aktualisierung kann das System nicht
zerlegen**, und das kann kein Paketverwalter zusagen. Der Preis sind
zwei Abbilder statt einem, also 100 MB statt 50.

Umgesetzt mit `golem-aktualisieren` (2026-11-13), beschrieben in
[`README/README.md`](README/README.md).
