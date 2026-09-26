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
| 3 | `golem-wurzelB` | dasselbe, von Anfang an belegt |
| 4 | `golem-daten` | wird beim Einrichten gedehnt |

⚑ **`wurzelB` trägt von Anfang an eine Kopie.** Ein Rückfallweg, den es
erst nach der ersten Aktualisierung gibt, fehlt genau beim ersten Mal,
und das erste Mal ist der wahrscheinlichste Zeitpunkt für einen
Fehlschlag.

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
  Artefakt: myelith-0.6b
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
| 3 | `sh SYSTEM/golemos/mitnahme.sh --artefakt myelith-0.6b`, dann den Inhalt von `bau/mitnahme/` auf Partition 4 kopieren (`mcopy -s` mit dem Versatz aus `sfdisk`) |
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

---

## ⚠️ Was noch offen ist, ehrlich benannt

| Punkt | Stand |
|---|---|
| **Volumenplatz** | rund 13 GB je Architektur, zusammen 27 GB. Kein Problem, aber es überrascht sonst |
| **Installateur** | ✅ `golem-einrichten`: Assistent bei der ersten Anmeldung, Installation auf eine ganze Platte, Myelith-Ordner auf dem Stick statt Mitnahme. Belegt am 2026-09-25 unter QEMU mit UEFI (aarch64), noch nicht auf echter Hardware |
| **Mitnahme** | ✅ `mitnahme.sh` sammelt Werkzeugkisten, Wissensmappen, Artefakte und die Einstellungen mit Pfaden unter `/daten`; belegt am 2026-09-25 mit dem 0,6B-Artefakt |
| **Ohne Netz** | Die Programme für GolemOS bauen aus dem Vorrat ohne Netz. **Das Grundsystem nicht:** Buildroot lädt seine Quellen beim ersten Bau herunter (Kern, BusyBox, `curl`, GRUB). Sie bleiben im Volumen liegen; ob ein zweiter Bau dann ohne Netz durchläuft, ist nicht nachgesehen |
| **A/B-Umschaltung** | die Partitionen sind da, das Umschalten noch nicht |
| **Kernmodule** | 84 MB Treiber für Hardware, die nie da sein wird. Braucht eine eigene Kernkonfiguration und damit eine benannte Zielmaschine |
| **`alpine:edge`** | rollendes Ziel, gehört festgenagelt, sobald eine veröffentlichte Fassung Rust 1.88 trägt. 3.22 hat 1.87 |
| **Bootloader** | kommt mit Stufe 2, zusammen mit GRUB 2 und einem Plattenabbild |
| **Kernkonfiguration** | die Vorgabe der Architektur. Für eine bestimmte Zielmaschine gehört eine eigene her |
| **Macs mit Apple-Chip** | vorgemerkt über Asahi Linux (Wunsch des Projektinhabers, 2026-09-26). Der Asahi-Installer kann nur eine UEFI-Umgebung einrichten (rund 3 GB, dazu hält er 38 GB für macOS frei), die danach jeden USB-Stick mit `EFI/BOOT/BOOTAA64.EFI` startet; das aarch64-Abbild trägt `bootaa64.efi` dort bereits. **Es fehlt:** ein Kern aus dem Asahi-Baum mit Apples Treibern und der Firmware, die der Installer aus macOS zieht, also ein drittes Abbild. **Es hält auf:** Asahi unterstützt M1 bis M3, M4 und M5 noch nicht, und die Maschine hier ist ein M5; getestet werden könnte es nur auf fremder Hardware. Die Alternative ohne Asahi wäre eine virtuelle Maschine über Apples Virtualization-Framework, auf jedem Apple-Chip |
| **Die Sinne** | Stufe 3. `ffmpeg` allein ist grösser als das ganze Grundsystem |

---

## Warum kein Paketverwalter, und was stattdessen kommt

Ohne Paketverwaltung ist jede Sicherheitslücke ein neues Abbild.
⚑ **Die Antwort heisst A/B und ist besser als ein Aktualisierungslauf:**
zwei Wurzelpartitionen, das neue Abbild geht auf die inaktive, dann
wird umgeschaltet und neu gestartet. Geht dabei etwas schief, startet
die alte. **Eine unterbrochene Aktualisierung kann das System nicht
zerlegen**, und das kann kein Paketverwalter zusagen. Der Preis sind
zwei Abbilder statt einem, also 100 MB statt 50.

Das kommt mit Stufe 2.
