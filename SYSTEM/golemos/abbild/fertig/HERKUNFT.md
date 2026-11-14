# Woher diese Abbilder kommen

⛔️ **Hier liegt ein Binärblob im Repositorium, und das ist eine
Entscheidung, keine Nachlässigkeit.** Diese Datei macht ihn
nachvollziehbar.

## Warum überhaupt

Ohne ein fertiges Abbild braucht der allererste Schritt **Docker und
eine Stunde Bauzeit**, und genau dort sollte die Kette halten: Ein
frischer Klon auf einem Rechner ohne Entwicklerwerkzeuge soll einen
startfähigen Datenträger erzeugen können.

⚑ **Die Grössenordnung trägt die Entscheidung**, nicht die Art der
Sache. Projektregel 7 hält Gewichte und fremde Laufzeiten aus dem
Repositorium heraus, und sie zielt auf das, was **riesig** ist: Die
Gewichte wiegen 77 bis 165 GB. Dies hier sind **70 und 79 MB** je
Architektur, jede Datei unter der Grenze von 100 MiB. Wer beides in dieselbe Kategorie steckt, hat
die Regel gelesen und nicht verstanden.

## Was das kostet, ausgesprochen

⚠️ **Die Geschichte wächst bei jedem Neubau um rund 150 MB (beide
Architekturen) und schrumpft nie.** Im September waren es 90 MB; die
A/B-Umschaltung, ein Kern je Seite und gewachsene Programme haben das
Abbild größer gemacht. Zwölf Neubauten im Jahr wären 1,8 GB, die für
immer in jedem Klon liegen. ⛔️ **`freigeben.sh` verweigert ein gepacktes
Abbild ab 100 MiB**, darüber nimmt GitHub keine Datei an. ⚑ **Daraus folgt eine Regel:** Ein neues Abbild kommt nur bei
einem **benannten Stand** herein, nicht nach jedem Bau.

⚠️ **Niemand kann einen Blob durch Lesen prüfen.** Dagegen hilft die
Prüfsumme daneben und dieser Zettel: Wer ihn nachbauen will, hat die
Fassung, die Konfiguration und das Datum.

## Dieser Stand

**Grund der Freigabe: GolemOS v0.6.1.** Gegenüber v0.2.0 vom 2026-09-25:

- **A/B-Umschaltung** (`golem-aktualisieren`): ein neues System auf die
  zweite Seite, beim nächsten Start einmal zur Probe, bestätigt am Ende
  eines gesunden Starts, sonst zurück; ein Kern je Seite.
- **Seite B kommt leer** und wird beim ersten Start aus A gefüllt
  (`S00seiteb`), damit das Abbild die Wurzel nicht zweimal trägt.
- **Datenteil exFAT** statt FAT32; ein FAT32-Datenteil eines früheren
  Stands bleibt, was er ist.
- **Lizenztexte aller Pakete im Abbild** (`/usr/share/licenses/`, dazu
  `/usr/share/golemos/LIZENZEN.txt`); das Verzeichnis liegt außerdem als
  `lizenzen-<arch>.tar.xz` daneben.
- Entschlackt: Programme ohne Symbole, Kernmodule gepackt, echtes `xz`
  und `kmod`. Dazu aus den Ständen dazwischen: Knoten als Dienst
  (`golem-knoten`), udev und seatd für die Oberflächen-Schicht.

⛔️ **Die Quellen gehören an dieselbe Freigabe** (`quellen-<arch>.tar`,
je rund 0,7 GB, aus `auskunft.sh`): Das Abbild enthält Teile unter der
GPL. Zu groß fürs Repositorium, deshalb als Anhang der Freigabe
[GolemOS v0.6.1](https://github.com/crowdedconfusion/Myelith/releases/tag/golemos-v0.6.1);
was dort liegt und wie man es prüft, steht in `SYSTEM/golemos/Quellarchiv/README.md`.

| | |
|---|---|
| Gebaut am | 2026-11-14 |
| Buildroot | 2026.08.1 |
| Kern | Linux 7.1.13 |
| GRUB | 2.14 |
| Programme | `myelith`, `myl`, `myl-node` und `golem-einrichten` 0.5.0, statisch gegen musl (Alpine, Rust 1.99.1), ohne Symbole |

| Architektur | Nenngröße | belegt | gepackt |
|---|---|---|---|
| `x86_64` | 1040 MB | 210 MB | **70,3 MB** |
| `aarch64` | 1040 MB | 271 MB | **79,3 MB** |

### Was belegt ist, und wodurch

Genau diese beiden Abbilder, vor dem Packen (entpackt bitgleich, gegen
die SHA-256 geprüft), unter QEMU 11.1.3 mit UEFI (edk2), ohne Netz,
bedient über die serielle Leitung, je Architektur 17 Starts:

| | x86_64 | aarch64 |
|---|---|---|
| erster Start: Wurzel vor jedem Dienst kurz schreibgeschützt, B gefüllt, beide Seiten mit demselben Stand; beim zweiten Start kein zweites Füllen | ✓ | ✓ |
| Datenteil exFAT, eine Datei überlebt den nächsten Start | ✓ | ✓ |
| ein FAT32-Datenteil wie aus einem früheren Stand bleibt FAT32 samt Inhalt | ✓ | ✓ |
| echtes `xz`, `modprobe` von kmod, ein gepacktes Modul geladen | ✓ | ✓ |
| einspielen, Probe bestätigt, Probe abgebrochen und zurück, kaputte Seite und zurück, `status` warnt | ✓ | ✓ |
| Installation verweigert bei verschiedenen Ständen, danach installiert (Datenteil exFAT), die Platte startet allein | ✓ | ✓ |
| Lizenzseite, Verzeichnis und Lizenztexte von Kern, GRUB, BusyBox im Abbild | ✓ (Prüfung im Bau) | ✓ (Prüfung im Bau) |
| „Paris" mit `myl frage`, Modell von einer zweiten Platte | nicht geprüft | ✓ |
| Myelith-Ordner auf dem Stick, Konsole ohne Installation, `spread.sh` | zuletzt mit v0.2.0 | zuletzt mit v0.2.0 |
| Datenteil unter macOS und Windows | nicht geprüft (exFAT ist neu) | nicht geprüft |

⚠️ **Geprüft in QEMU, nicht auf echter Hardware.** Ein Abbild, das
unter Emulation startet, startet meistens auch auf Blech; aber
„meistens" ist kein Beleg, und der Kern trägt die Treibervorgabe seiner
Architektur, nicht die einer bestimmten Maschine.

**Enthält:** Kern, glibc, BusyBox, `curl` mit OpenSSL, util-linux,
eudev, seatd, `kmod`, `xz`, `dosfstools`, `exfatprogs`, `kbd`, GRUB 2.14,
und darauf `myelith`, `myl`, `myl-node` und `golem-einrichten`.

**Enthält nicht:** Modelle, Gewichte, den Myelith-Ordner, die
Oberflächen-Schicht. Der Ordner kommt nach dem ersten Start auf
**Golem**.

## Nachbauen

```sh
sh SYSTEM/golemos/bauen.sh --arch x86_64
xz -9 -c SYSTEM/golemos/aus/x86_64/golemos.img > golemos-x86_64.img.xz
shasum -a 256 golemos-x86_64.img.xz
```

⚠️ **Bitgleich wird das nicht, und seit Fund 479 ausdrücklich nicht:**
Jeder Bau würfelt die Partitionskennungen neu. Dazu bettet Buildroot
Zeitstempel ein, und `alpine:edge` ist ein rollendes Ziel. Dass der Blob **funktional**
dasselbe ist, belegt der Startversuch, nicht die Prüfsumme; die
Prüfsumme belegt nur, dass **dieser** Blob unverändert ist.

## Eine Beobachtung, die niemand versprochen hat

Beim Freigeben meldete `freigeben.sh` für `x86_64`: **bitgleich mit dem
bereits abgelegten Stand.** Zwei Bauläufe desselben Standes ergaben
hier also dasselbe Abbild, obwohl oben ausdrücklich steht, dass ein
Nachbau nicht bitgleich wird.

⚠️ **Das belegt einen Fall, keine Regel**, und es galt für den Stand
vom 2026-09-24; seit den gewürfelten Kennungen (Fund 479) kann es nicht
mehr eintreten. Über eine
`alpine:edge`-Aktualisierung oder eine neue Buildroot-Fassung hinweg
wird es nicht halten. Es ist trotzdem ein gutes Zeichen: Der Bau hängt
an weniger Zufall, als ich angenommen hatte.
