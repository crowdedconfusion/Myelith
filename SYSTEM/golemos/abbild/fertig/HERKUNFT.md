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
Gewichte wiegen 77 bis 165 GB. Dies hier sind **31 MB**, weniger als
ein Drittel von `vorrat/`. Wer beides in dieselbe Kategorie steckt, hat
die Regel gelesen und nicht verstanden.

## Was das kostet, ausgesprochen

⚠️ **Die Geschichte wächst bei jedem Neubau um 31 MB und schrumpft
nie.** Zwölf Neubauten im Jahr sind 370 MB, die für immer in jedem Klon
liegen. ⚑ **Daraus folgt eine Regel:** Ein neues Abbild kommt nur bei
einem **benannten Stand** herein, nicht nach jedem Bau.

⚠️ **Niemand kann einen Blob durch Lesen prüfen.** Dagegen hilft die
Prüfsumme daneben und dieser Zettel: Wer ihn nachbauen will, hat die
Fassung, die Konfiguration und das Datum.

## Dieser Stand

**Grund der Freigabe: GolemOS v0.2.0.** Beim ersten Anmelden fragt der
Einrichtungsassistent nach Sprache und Tastatur, bietet an, Myelith
ohne Installation zu benutzen, GolemOS auf eine ganze Platte zu
installieren oder selbst einen weiteren Stick zu schreiben. Der
Datenteil des Sticks heißt **Golem** und trägt nach dem ersten Start
eine zweisprachige `README.txt`. Der Myelith-Ordner kommt als Ganzes
auf den Stick und bei jeder Installation mit.

Aus dem Stand davor (v0.1.0, derselbe Tag) weiter enthalten: die
Datenpartition als „Microsoft Basic Data", damit macOS und Windows sie
sehen (Fund 476); die serielle Konsole je Architektur (Fund 477); Wurzel
und Daten über Partitionskennungen, die jeder Bau würfelt (Fund 479).

📌 **Ein früherer Stand behauptete hier „`GOLEM-DATEN` an einem fremden
Rechner sichtbar ✓".** Belegt war das mit `mtools` unter Linux, nicht an
einem Rechner mit macOS oder Windows, und genau dort galt es nicht
(Fund 476). Ein Häkchen gilt für das, womit geprüft wurde.

| | |
|---|---|
| Gebaut am | 2026-09-25 |
| Buildroot | 2026.02.3 |
| Kern | Linux 6.19.14 |
| Programme | `myelith`, `myl` und `golem-einrichten` 0.2.0, statisch gegen musl (Alpine, Rust 1.98.1); `kbd` für die Tastaturbelegungen |

| Architektur | Nenngrösse | belegt | gepackt |
|---|---|---|---|
| `x86_64` | 896 MB | 215 MB | **34,6 MB** |
| `aarch64` | 896 MB | 400 MB | **55,6 MB** |

⚑ **Warum aarch64 grösser ist:** Der ARM-Kern liegt **roh** vor
(`Image`, 39 MB), der x86-Kern **komprimiert** (`bzImage`, 14 MB).
Dieselbe Konfiguration, dieselben Programme.

### Was belegt ist, und wodurch

Genau diese beiden Abbilder, vor dem Packen, unter QEMU mit
UEFI-Firmware (OVMF für x86_64, AAVMF für aarch64), ohne Netz, bedient
nur über die serielle Konsole. Stick und Zielplatte je 4 GB; auf den
Stick kam der ganze Myelith-Ordner (2365 versionierte Dateien, `.git`,
das 0,6B-Modell):

| | x86_64 | aarch64 |
|---|---|---|
| erster Start: Sprache, Tastatur (`loadkeys`), Datenpartition gedehnt, Anleitung, Abschalten | ✓ | ✓ |
| `README.txt` und der Name `Golem` auf der Datenpartition | ✓ | ✓ |
| Myelith ohne Installation: Konsole startet, antwortet, zurück ins Menü | ✓ | ✓ |
| Ordner vollständig erkannt, Modell eingestellt, „Paris" mit `myl frage` | ✓ | ✓ |
| Installation über den Assistenten, der Ordner kommt mit | ✓ | ✓ |
| weiterer Stick vom Stick aus mit `spread.sh`; der laufende Stick wird abgelehnt | ✓ | ✓ |
| nur die Platte: eigene Kennungen, „Paris" | ✓ | ✓ |
| Platte, der Stick steckt noch: alles von der Platte | ✓ | ✓ |
| Stick, die Platte wird zuerst gefunden: alles vom Stick | ✓ | ✓ |
| Datenpartition unter macOS eingehängt | ✓ (am Typ, Fund 476) | ✓ (am Typ) |
| Datenpartition unter Windows | nicht geprüft | nicht geprüft |

⚠️ **Geprüft in QEMU, nicht auf echter Hardware.** Ein Abbild, das
unter Emulation startet, startet meistens auch auf Blech; aber
„meistens" ist kein Beleg, und der Kern trägt die Treibervorgabe seiner
Architektur, nicht die einer bestimmten Maschine.

**Enthält:** Kern, glibc, BusyBox, `curl` mit OpenSSL, `sfdisk`,
`mkfs.vfat`, `kbd`, GRUB 2.12, und darauf `myelith`, `myl` und
`golem-einrichten`.

**Enthält nicht:** Modelle, Gewichte, den Myelith-Ordner. Der kommt nach
dem ersten Start auf **Golem**.

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
