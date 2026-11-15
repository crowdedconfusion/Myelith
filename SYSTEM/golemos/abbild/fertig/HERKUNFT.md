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
Gewichte wiegen 77 bis 165 GB. Dies hier sind **42 und 52 MB** je
Architektur, jede Datei unter der Grenze von 100 MiB. Wer beides in dieselbe Kategorie steckt, hat
die Regel gelesen und nicht verstanden.

## Was das kostet, ausgesprochen

⚠️ **Die Geschichte wächst bei jedem Neubau um rund 95 MB (beide
Architekturen) und schrumpft nie.** Im September waren es 90 MB; die
A/B-Umschaltung, ein Kern je Seite und gewachsene Programme brachten es
auf 150 MB, bis v0.8.0 die Lesemodelle aus dem Abbild nahm. Zwölf
Neubauten im Jahr wären 1,1 GB, die für immer in jedem Klon liegen. ⛔️ **`freigeben.sh` verweigert ein gepacktes
Abbild ab 100 MiB**, darüber nimmt GitHub keine Datei an. ⚑ **Daraus folgt eine Regel:** Ein neues Abbild kommt nur bei
einem **benannten Stand** herein, nicht nach jedem Bau.

⚠️ **Niemand kann einen Blob durch Lesen prüfen.** Dagegen hilft die
Prüfsumme daneben und dieser Zettel: Wer ihn nachbauen will, hat die
Fassung, die Konfiguration und das Datum.

## Dieser Stand

**Grund der Freigabe: GolemOS v0.8.0, `myl` ohne eingebettete
Lesemodelle.** Gegenüber v0.7.0 vom Vortag:

- **`myl` trägt die Modelle der Texterkennung nicht mehr in sich.** Sie
  kommen aus dem Myelith-Ordner auf dem Stick oder einem zweiten
  Laufwerk; `myl` wiegt im Abbild 35 statt 86 MB, gepackt ist jedes
  Abbild rund 29 MB kleiner. Ohne Myelith-Ordner gibt es keine
  Texterkennung, und `myl scan` sagt, was fehlt.
- Startkette, Kern und Pakete sind dieselben wie in v0.7.0, die
  Programme auf dem Stand dieses Tages.

**Seit v0.7.0 mit Secure Boot:**

- **Startet mit eingeschaltetem Secure Boot.** Die Firmware startet
  `shim` 16.1 aus Debian 13, unverändert und von Microsoft unter der
  UEFI CA 2011 und 2023 signiert; shim startet GRUB, GRUB den Kern, beide
  mit dem eigenen Schlüssel **Golem-Emet** signiert. Ein Rechner trägt
  Golem-Emet beim ersten Start einmal über den MokManager ein; das
  Zertifikat liegt als `EFI/GOLEM-EMET.cer` auf der Bootpartition.
  Fingerabdruck SHA-1 `5D:0B:E9:E3:64:B9:BF:8B:B4:F5:37:D6:B6:C7:9C:0C:C0:D2:F8:B9`.
- **GRUB mit SBAT** (`grub,5`, `grub.golemos,1`).
- **Der Kern lädt nur signierte Module** und läuft immer unter Lockdown
  „integrity“, auch ohne Secure Boot.
- Wer Secure Boot ausgeschaltet lässt, merkt keinen Unterschied.

⛔️ **Die Quellen gehören dazu** (`quellen-<arch>.tar`,
je rund 0,7 GB, aus `auskunft.sh`): Das Abbild enthält Teile unter der
GPL. Zu groß fürs Repositorium, deshalb als Anhang der Freigabe
[GolemOS: Quellen](https://github.com/crowdedconfusion/Myelith/releases/tag/golemos-quellen);
dieselben Archive wie zu v0.6.1 und v0.7.0, denn kein Paket hat sich geändert;
was dort liegt und wie man es prüft, steht in `SYSTEM/golemos/Quellarchiv/README.md`.

| | |
|---|---|
| Gebaut am | 2026-11-15 |
| Buildroot | 2026.08.1 |
| Kern | Linux 7.1.13 |
| GRUB | 2.14 |
| shim | 16.1 aus Debian 13 (`shim-signed` 1.51~1+deb13u1), MokManager aus `shim-helpers-*-signed` |
| Programme | `myelith`, `myl`, `myl-node` und `golem-einrichten` 0.5.0, statisch gegen musl (Alpine, Rust 1.99.1), ohne Symbole |

| Architektur | Nenngröße | belegt | gepackt |
|---|---|---|---|
| `x86_64` | 1040 MB | 200 MB | **41,9 MB** |
| `aarch64` | 1040 MB | 255 MB | **52,4 MB** |

### Was belegt ist, und wodurch

Genau diese beiden Abbilder, vor dem Packen (entpackt bitgleich, gegen
die SHA-256 geprüft), unter QEMU 11.1.3 mit UEFI (edk2), ohne Netz,
bedient über die serielle Leitung. Mit Secure Boot: die Firmware von
Debian mit den Schlüsseln von Microsoft, je Architektur 15 Starts; ohne
Secure Boot je 18 Starts.

| | x86_64 | aarch64 |
|---|---|---|
| mit Secure Boot: ein unsigniertes Abbild wird abgewiesen | nicht geprüft | zuletzt mit v0.7.0, die Startkette ist unverändert |
| mit Secure Boot: shim weist GRUB zunächst ab, Golem-Emet über den MokManager eingetragen (Fingerabdruck geprüft), danach GRUB und Kern, `SecureBoot`=1 im System | ✓ | ✓ |
| mit Secure Boot: einspielen, Probe bestätigt, Probe abgebrochen und zurück, kaputte Seite und zurück | ✓ | ✓ |
| mit Secure Boot: Installation, die installierte Platte startet allein | ✓ | ✓ |
| ohne Secure Boot: erster Start mit kurz schreibgeschützter Wurzel, B gefüllt, beide Seiten mit demselben Stand; beim zweiten Start kein zweites Füllen | ✓ | ✓ |
| ohne Secure Boot: Datenteil exFAT, eine Datei überlebt den nächsten Start; ein FAT32-Datenteil eines früheren Stands bleibt FAT32 samt Inhalt | ✓ | ✓ |
| ohne Secure Boot: echtes `xz`, `modprobe` von kmod, ein gepacktes (signiertes) Modul geladen | ✓ | ✓ |
| ohne Secure Boot: A/B-Kette und Installer-Kette wie oben | ✓ | ✓ |
| `myl scan` ohne Myelith-Ordner: Rückgabe 1 mit der Meldung, was fehlt | ✓ | ✓ |
| `myl scan` mit Myelith-Ordner, die Lesemodelle von dort | nicht geprüft (unter macOS belegt) | nicht geprüft (unter macOS belegt) |
| Lizenzseite, Verzeichnis und Lizenztexte von Kern, GRUB, BusyBox und shim im Abbild | ✓ (Prüfung im Bau) | ✓ (Prüfung im Bau) |
| „Paris" mit `myl frage`, Modell von einer zweiten Platte | nicht geprüft | zuletzt mit v0.6.1 |
| Myelith-Ordner auf dem Stick, Konsole ohne Installation, `spread.sh` | zuletzt mit v0.2.0 | zuletzt mit v0.2.0 |
| Datenteil unter macOS und Windows | nicht geprüft | nicht geprüft |

⚠️ **Geprüft in QEMU, nicht auf echter Hardware.** Ein Abbild, das
unter Emulation startet, startet meistens auch auf Blech; aber
„meistens" ist kein Beleg, und der Kern trägt die Treibervorgabe seiner
Architektur, nicht die einer bestimmten Maschine. Für Secure Boot gilt
das doppelt: Manche Firmware verlangt mehr, als die nachgebildete prüft.

**Enthält:** Kern, glibc, BusyBox, `curl` mit OpenSSL, util-linux,
eudev, seatd, `kmod`, `xz`, `dosfstools`, `exfatprogs`, `kbd`, GRUB 2.14,
shim und MokManager, und darauf `myelith`, `myl`, `myl-node` und
`golem-einrichten`.

**Enthält nicht:** Modelle (seit v0.8.0 auch die der Texterkennung), Gewichte, den Myelith-Ordner, die
Oberflächen-Schicht, den geheimen Teil von Golem-Emet. Der Ordner kommt
nach dem ersten Start auf **Golem**.

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
