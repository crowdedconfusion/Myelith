# golemos (die Linux-Distribution, die nur Myelith bedient)

> **Version:** 0.6.2 (`golem-einrichten` 0.5.0)
> **Datum:** 2026-11-14
> **Status:** Startet vom Stick und von Platte, findet einen
> Myelith-Ordner auf dem Stick oder einem zweiten Laufwerk und
> installiert sich mit einem Assistenten auf eine ganze Platte. Belegt
> unter QEMU mit UEFI auf x86_64 und aarch64, von Anfang bis Ende. Seit
> 0.3.0 betreibt es einen **Myelith-Knoten** als Dienst (`golem-knoten`,
> unter QEMU belegt) und nimmt eine **Oberflächen-Schicht** dazu
> (`golem-schicht`, das Myelith-Fenster im Vollbild). Seit 0.6.0 mit
> A/B-Umschaltung. Offen: ein Lauf auf echter Hardware, Secure Boot,
> eine eigene Kernkonfiguration.

Wie GolemOS gebaut wird und warum es so aussieht, steht in
[`../README.md`](../README.md). Hier stehen der Weg für Menschen und der
Changelog.

## Vom Abbild zum laufenden Myelith

**Die Anleitung für Menschen ohne Vorwissen:** [`../ANLEITUNG.md`](../ANLEITUNG.md)
(English: [`../ANLEITUNG.en.md`](../ANLEITUNG.en.md)). Kurz:

| Schritt | Was |
|---|---|
| 1 | `abbild/fertig/golemos-<arch>.img.xz` auf einen Stick schreiben (balenaEtcher, Rufus, `spread.sh`) |
| 2 | vom Stick starten, als `root` anmelden; der Assistent erscheint, der erste Start dehnt die Datenpartition |
| 3 | im Assistenten **2**: Anleitung, Abschalten |
| 4 | den Stick am eigenen Rechner einstecken, er erscheint als **Golem**; den **ganzen** Myelith-Ordner daraufziehen |
| 5 | wieder vom Stick starten, im Assistenten **1**: Myelith benutzen, ohne etwas zu installieren |
| 6 | freiwillig **3**: GolemOS auf eine ganze Platte installieren; **4**: einen weiteren GolemOS-Stick schreiben |

⚠️ **Die Installation löscht die gewählte Platte vollständig.** Der
Assistent bietet nur Platten an, von denen GolemOS nicht läuft und die
nirgends eingehängt sind, zeigt vorher, was darauf liegt, und
verlangt den Namen der Platte als Bestätigung.

⚠️ **Auf Macs mit Apple-Chip startet GolemOS nicht**, dort fehlt UEFI.
Gemeint sind PCs (x86_64) und ARM-Rechner mit UEFI (aarch64).

## Der Einrichtungsassistent (`einrichten/`)

| Befehl | Was |
|---|---|
| `golem-einrichten` | der Assistent, zeilenweise, auch über eine serielle Leitung bedienbar |
| `golem-einrichten suchen` | Myelith-Ordner suchen, Modell einstellen; läuft bei jedem Start (`S06myelith`) |
| `golem-einrichten platten` | welche Platte für eine Installation in Frage kommt, und warum nicht |
| `golem-einrichten installieren --ziel NAME --bestaetigung NAME` | dasselbe ohne Fragen, mit denselben Prüfungen |
| `golem-einrichten schicht-eintreten WURZEL PROGRAMM` | in eine Schicht wechseln und alle Vorrechte ablegen, bevor ihr erstes Programm läuft (für `golem-schicht`) |

## Der Knoten (`golem-knoten`)

`myl-node` als Dienst, mit demselben Zuschnitt wie der MYL-SERVER für NixOS
(`NODE/myl-server/modul.nix`): eigenes Konto ohne Vorrechte (900,
`setpriv` aus util-linux), nach außen nur der P2P-Port, Beobachtung und Tür
auf der Rückschleife, nach fünf Abbrüchen in fünf Minuten angehalten statt
endlos neu gestartet. Einstellung (`/etc/golemos/knoten.conf`, root 0600)
und Zustand samt Identität (`/var/lib/myl-knoten`, 0700) liegen auf der
Wurzel und nicht auf der für jeden lesbaren FAT-Datenpartition.

| Befehl | Was |
|---|---|
| `golem-knoten einrichten --name N --rolle teilnehmer\|relais [...]` | einrichten, jeder Wert geprüft |
| `golem-knoten starten` / `stoppen` / `status` / `protokoll` | betreiben |
| `golem-knoten an` / `aus` | beim Hochfahren mitstarten (`S50knoten`) |
| `golem-knoten konsensschluessel DATEI` | den Konsensschlüssel eines stimmberechtigten Knotens übernehmen |
| `golem-knoten befehlszeile` | was gestartet würde, ein Argument je Zeile |

⚑ **Der Knoten entscheidet nichts im Protokoll.** Rolle, Pod-Zuteilung,
Routing und Abstimmungen bestimmt die Kette; `golem-knoten` richtet ein und
betreibt.

## Die Aktualisierung (`golem-aktualisieren`)

Zwei Wurzeln (`golem-wurzelA`, `golem-wurzelB`), jede mit ihrem eigenen
Kern auf der Bootpartition (`Image-A`/`Image-B` bzw. `bzImage-…`). Eine
Aktualisierung schreibt auf die ruhende Seite und startet sie **einmal zur
Probe**, wie RAUC, Mender oder ChromeOS es tun:

| Wer | Was |
|---|---|
| `golem-aktualisieren einspielen PAKET` | prüft `PAKET.sha256`, setzt die Zielseite in `grubenv` auf „unfertig“, schreibt Wurzel und Kern, setzt `golem_probe` und ihren Stand |
| GRUB (`grub.cfg`) | liest `grubenv`; steht eine Probe an, leert er sie (`save_env`) und startet sie, sonst `golem_seite` |
| `S95aktualisierung` | am Ende eines gesunden Starts: `bestaetigen --wenn-erwartet` macht die Probeseite zu `golem_seite` |
| `golem-einrichten installieren` | kopiert die ruhende Seite und verlangt deshalb, dass beide Seiten denselben Stand tragen |

Das Paket `aus/<arch>/golemos-<arch>.aktualisierung` legt `bauen.sh` an
(tar mit `stand`, `wurzel.ext2.gz`, `kern`, daneben `.sha256`). Der Stand
sind die ersten 16 Zeichen der SHA-256 der Wurzel, derselbe, den
`nach-dem-abbild.sh` beiden Seiten des Abbilds ins `grubenv` legt.

⚠️ Die Prüfsumme belegt, dass ein Paket heil ankam, nicht wer es gebaut
hat; eine Signatur fehlt wie bei den Schichten.

## Die Oberfläche (`golem-schicht`)

Das Myelith-Fenster im Vollbild (cage, GTK3, WebKitGTK), als **Schicht**:
eine squashfs-Datei auf der Datenpartition, gebaut mit
`schicht-bauen.sh`, nicht im Repositorium und nicht in der Basis. Die Basis
bleibt klein; wer die Oberfläche nicht will, holt sie nicht.

| Befehl | Was |
|---|---|
| `golem-schicht holen QUELLE` | von einem Laufwerk oder per https holen; ohne passende Prüfsumme wird nichts übernommen |
| `golem-schicht starten` / `stoppen` / `status` | die Oberfläche betreiben |
| `golem-schicht an` / `aus` | beim Hochfahren starten (`S70oberflaeche`) |
| `golem-schicht entfernen` | die Schicht löschen |

⛔️ **Kein Code der Schicht läuft als root.** udev und seatd gehören zur
Basis; die Schicht wird `nosuid,nodev` eingehängt, und
`golem-einrichten schicht-eintreten` legt alle Vorrechte ab, bevor ihr
erstes Programm startet. Das Fenster läuft als `golem` (1000) und sieht den
Zustand des Knotens nicht.

⚑ **Der Agent bedient beides** über die Werkzeugkiste
`AGENT_LAYER/local-toolkits/GolemOS` und den Skill `golemos-einrichten`;
liegt der Myelith-Ordner auf der Datenpartition, stellt die erste
Einrichtung diese Kiste ein.

Die Kiste hat keine einzige Abhängigkeit: Sie löscht Platten, und was
sie tut, soll sich in ihrem eigenen Quelltext lesen lassen.

---

## Changelog

### v0.6.2 (2026-11-14): Wo die Quellen liegen

- **Neu `Quellarchiv/README.md`**: Die Quellen zu den freigegebenen
  Abbildern (`quellen-<arch>.tar`, je rund 0,7 GB) und die Lizenzauskunft
  liegen als Anhang der Freigabe `golemos-v0.6.1` auf GitHub; die Datei nennt
  den Ort, die Dateien und wie man sie prüft. Die Archive selbst nimmt
  `.gitignore` aus (`Quellarchiv/*/`), denn eine Datei darf im Repositorium
  höchstens 100 MiB haben. `HERKUNFT.md` verweist darauf.

### v0.6.1 (2026-11-14): Abbilder unter 100 MiB, exFAT, Lizenztexte im Abbild (`golem-einrichten` 0.5.0)

- ⛔️ **Die Freigabe von 0.6.0 scheiterte an der Größe:** gepackt 128 MB
  (x86_64) und 144 MB (aarch64), über der Grenze von 100 MiB je Datei im
  Repositorium. Zwei Ursachen: Mit der A/B-Umschaltung lag jede Wurzel
  zweimal im Abbild, und die Myelith-Programme lagen mit allen Symbolen
  darin (Buildroot bereinigt seine eigenen Programme und legt das Overlay
  erst danach ein). `freigeben.sh` packt jetzt neben das alte Abbild und
  verweigert ab 100 MiB, statt es zu überschreiben.
- ⚑ **Seite B kommt leer aus dem Abbild** (Stand `leer`) und wird beim
  ersten Start aus A gefüllt: `S00seiteb` ruft `golem-aktualisieren
  fuellen`, vor jedem Dienst, mit kurz schreibgeschützter Wurzel. Ein
  abgebrochenes Kopieren hinterlässt wieder `leer`, der nächste Start
  versucht es erneut. `golem-einrichten` verweigert die Installation von
  einem Stick mit leerer Seite B mit eigenem Grund. ⛔️ Ein größeres
  Wörterbuch für `xz` hätte die doppelte Wurzel auch erkannt (gemessen
  71 MB), aber Rufus und das `xz` von BusyBox entpacken höchstens 64 MiB,
  und balenaEtcher erlaubt 20 % des Arbeitsspeichers.
- **Entschlackt bei gleichem Funktionsumfang:** die Programme bereinigt
  (`nach-dem-bauen.sh`, `--strip-unneeded`), die Kernmodule als `.ko.xz`
  (aarch64 84 MB auf 24 MB je Wurzel), dazu das echte `xz` und `kmod` mit
  seinen Werkzeugen statt der BusyBox-Fassungen, die weder gepackte Module
  laden noch große Wörterbücher entpacken; `depmod` auf dem Bauwirt mit xz.
  📌 `bauen.sh` baut jetzt auch Pakete des Bauwirts (`host-kmod`) und ein
  geändertes Kernfragment neu; beides wäre sonst wortlos alt geblieben.
- ⚑ **Der Datenteil ist exFAT** (Kern `CONFIG_EXFAT_FS`, `exfatprogs`),
  vorher FAT32: an jedem Rechner les- und beschreibbar, ohne Grenze von
  4 GB je Datei; GolemOS liest jetzt auch fremde Laufwerke mit exFAT.
  ⛔️ Ein FAT32-Datenteil aus einem früheren Stand bleibt FAT32:
  `S05daten` liest die Art vorher und versucht ohne Angabe beide, denn
  ein Datenteil, der sich nicht einhängen lässt, wird neu formatiert.
  ⚠️ `exfat` kennt `flush` nicht; Geschriebenes wartet dort bis zu 30 s.
- **Freigegeben nach `abbild/fertig`:** gepackt 70,3 MB (x86_64) und
  79,3 MB (aarch64), statt 128 und 144 MB. Belegt unter QEMU mit UEFI, je
  Architektur 17 Starts: erster Start füllt B bei schreibgeschützter
  Wurzel und nur einmal; exFAT-Datenteil; ein FAT32-Datenteil eines
  früheren Stands bleibt samt Inhalt; `xz` und `kmod` echt, ein gepacktes
  Modul lädt; die A/B-Kette und die Installer-Kette wie in 0.6.0. Auf
  aarch64 dazu „Paris" mit `myl frage` aus den bereinigten Programmen.
  📌 Der Prüfstand verlor auf x86_64 gelegentlich QEMU mitten im
  GRUB-Menü; Ursache ein SIGHUP an den Prozess am Pseudoterminal, nicht
  GolemOS. Nicht geprüft: echte Hardware, der exFAT-Datenteil unter macOS
  und Windows.
- ⛔️ **Die Lizenztexte der fremden Pakete lagen nur neben dem Abbild,
  nicht darin.** Ein Stick, den GolemOS selbst schreibt oder den jemand
  weitergibt, trug Kern, GRUB und BusyBox unter der GPL ohne ihren
  Lizenztext. `bauen.sh` sammelt jetzt vor dem Bau mit `make legal-info`
  das Verzeichnis aller Pakete und ihre Lizenztexte und legt beides nach
  `/usr/share/licenses/` (`manifest.csv`, ein Ordner je Paket); die
  Nachzählung nach dem Bau verlangt das Verzeichnis und die Ordner von
  Kern, GRUB und BusyBox.
- **Neu `/usr/share/golemos/LIZENZEN.txt`**, zweisprachig: GolemOS ist eine
  Zusammenstellung; die Teile des Myelith-Projekts stehen unter PolyForm
  Shield 1.0.0, die fremden unter ihren eigenen Lizenzen, die PolyForm
  nicht einschränkt; wo die Quellen liegen und was die GPL von einem
  verlangt, der GolemOS weitergibt. `README.txt` auf dem Stick und beide
  Anleitungen verweisen darauf.
- ⚠️ **Die Oberflächen-Schicht trägt die Lizenztexte ihrer Pakete noch
  nicht in sich**, nur `lizenzen-<arch>.tar.xz` neben dem Abbild nennt sie.
- In der Liste des Offenen ersetzt **Secure Boot** die Zeile zum
  Bootloader, die seit dem Start von Platte mit GRUB überholt war.

### v0.6.0 (2026-11-13): Die A/B-Umschaltung (`golem-einrichten` 0.4.0)

- **Neu `golem-aktualisieren`** (`status`, `einspielen PAKET`,
  `bestaetigen`): schreibt ein Paket auf die ruhende Seite und startet sie
  beim nächsten Mal **einmal zur Probe**. `S95aktualisierung` bestätigt sie
  am Ende eines gesunden Starts; kommt sie nicht hoch, nimmt der nächste
  Start wieder die alte. Vorbild RAUC, Mender, ChromeOS.
- `grub.cfg` liest `grubenv` (`load_env`), leert eine anstehende Probe vor
  dem Start (`save_env`) und startet je Seite **ihren eigenen Kern**
  (`Image-A`/`-B`, `bzImage-A`/`-B`; Bootpartition 160 MB statt 64). GRUB
  bekommt dafür `loadenv` und `test`; `bauen.sh` baut GRUB neu, wenn sich
  seine Optionen ändern.
- `bauen.sh` legt das Paket `golemos-<arch>.aktualisierung` samt `.sha256`
  an (Stand, gepackte Wurzel, Kern). Jede Seite trägt in `grubenv` ihren
  **Stand** (die ersten 16 Zeichen der SHA-256 ihrer Wurzel); vor dem
  Schreiben gilt die Zielseite als unfertig.
- `golem-einrichten` 0.4.0: kopiert die ruhende Wurzel auf beide Seiten
  der Platte und jetzt auch **ihren Kern**, setzt `grubenv` zurück und
  verweigert die Installation, solange die beiden Seiten der Quelle
  verschiedene Stände tragen oder eine Aktualisierung unbestätigt ist.
  `festnageln` prüft, dass jede Seite ihren eigenen Kern startet.
- 📌 **Drei Befunde aus dem Prüfstand**, alle vor der Freigabe behoben:
  Nach dem ersten Start fehlte `PARTNAME` an jeder Partition außer der
  Wurzel (`partx -u` in `S05daten`); die Namen liest jetzt notfalls
  `sfdisk`. Der Ordner der Erwartungsmarke fehlte beim ersten Start, und
  keine Probe wäre je bestätigt worden. ⛔️ Und ein eigener Fehler beim
  Umstellen: Eine geteilte Variable ließ `dd` die Wurzel auf die
  **Bootpartition** schreiben; jetzt prüft das Skript unmittelbar vor dem
  Schreiben den Namen des Ziels.
- **Belegt unter QEMU mit UEFI (edk2), je Architektur zwei Ketten:**
  einspielen, Probe B bestätigt, B bleibt; eine Probe ohne Bestätigung
  (nach dem Kernstart abgebrochen) fällt auf B zurück; eine kaputte Wurzel
  (der Kern hält an der Wurzel an) fällt auf B zurück, `status` warnt.
  Bei verschiedenen Ständen verweigert die Installation mit Grund; nach
  erneutem Einspielen installiert sie, und die installierte Platte startet
  allein mit demselben Stand. aarch64 und x86_64 grün; auf x86_64 dazu acht Probestarts hintereinander,
  abwechselnd A und B, jeder ohne Neustart der Firmware bis zum Kern. Noch nicht auf
  echter Hardware. ⚠️ Pakete tragen eine Prüfsumme, keine Signatur.

### v0.5.3 (2026-11-13): Knotenprobe auf x86_64, und die Untergrenze des Prozessors

- **Knotenprobe auf x86_64** unter QEMU (`golemos.probe golemos.knotenprobe`,
  Abbild vom 2026-11-11, emuliert): mit `-cpu Nehalem` (x86-64-v2 mit
  SSSE3, ohne AVX) `KNOTEN_OK`: P2P-Port nach 1 s auf `0.0.0.0:4001`, Konto
  900, Beobachtung und Tür nur auf `127.0.0.1`, `stoppen` beendet ihn.
- 📌 **Befund:** Mit der Vorgabe-CPU von QEMU (`qemu64`, ohne SSSE3) brach
  `myl-node` fünfmal mit „invalid opcode“ ab (Rückgabe 132), am Befehl
  `pshufb` in der SHA-256 von blst. Echte Prozessoren haben SSSE3 seit Intel
  Core 2 (2006) und AMD Bulldozer (2011); die Anleitung nennt das jetzt, und
  die Startzeile am Ende von `bauen.sh` nimmt für x86_64 `-cpu Nehalem`.

### v0.5.2 (2026-11-13): Kommt ein Paket dazu, baut `bauen.sh` seine Nutzer neu

- 📌 **Befund vom Oktober:** Als eudev dazukam, blieb util-linux auf aarch64
  ohne udev gebaut und versteckte einen Fehler in `S05daten`. Buildroot
  merkt sich je Paket nur „gebaut“, und das Skript verwarf bisher nur
  Pakete, deren eigene Option sich änderte. eudev kam aber über
  `BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_EUDEV` herein, eine Paketoption stand
  in der Grundkonfiguration nie.
- Jetzt vergleicht `bauen.sh` die **aufgelöste** `.config` mit der des
  letzten Baus (`.golemos-voll` im Bauvolumen). Für jedes neu
  eingeschaltete Paket fragt es Buildroot nach seinen Nutzern
  (`<paket>-show-recursive-rdepends`) und verwirft die, die schon gebaut
  sind. Beim ersten Bau nach dieser Fassung wird nur gemerkt.
- Am echten Baum durchgespielt, ohne zu bauen (alte `.config` ohne eudev):
  busybox, dosfstools und util-linux würden neu gebaut, ohne den Block
  keines. Syntax unter sh und dash geprüft. ⚠️ Ein ganzer Bau damit steht
  aus (beim nächsten benannten Stand).

### v0.5.1 (2026-11-12): Lizenzpflichten der Sinne festgehalten

- `README.md` nennt, was ffmpeg (LGPL ohne `--enable-gpl` und
  `--enable-nonfree`) und piper (GPL 3.0 seit `piper1-gpl`, dazu
  espeak-ng) beim Mitbündeln verlangen, und schlägt Stufe 3 ohne piper vor:
  Gesprochen wird über den eigenen Weg. Nichts am Abbild geändert.

### v0.5.0 (2026-11-11): Lizenzauskunft und Quellen zu jedem Abbild

- 📌 **Fund 686 behoben:** Die Abbilder enthalten Teile unter der GPL
  (Kern, BusyBox, GRUB, Werkzeuge), und weder Skripte noch Doku nannten
  eine Lizenzauskunft oder die Quellen. Neu `auskunft.sh`: sammelt mit
  `make legal-info` ohne Netz für Basis und Schicht das Verzeichnis der
  Pakete, ihre Lizenztexte und ihre Quellen, dazu das Quellarchiv von
  Buildroot, das Buildroot selbst nicht beilegt. Ergebnis
  `aus/<arch>/lizenzen-<arch>.tar.xz` (klein, kommt neben das Abbild) und
  `aus/<arch>/quellen-<arch>.tar` (rund 0,7 GB, Anhang derselben Freigabe).
- `freigeben.sh` verweigert ein Abbild, zu dem die Auskunft fehlt oder
  älter ist als das Abbild (Gegenprobe: Rückgabe 4, nichts angefasst), und
  legt die kleine Auskunft neben das freigegebene Abbild.
- 📌 **Fund 700 behoben:** Die Basis lieferte `myl`, `myelith` und
  `myl-node` ohne `LICENSE.md`, `NOTICES.md` und `DRITTLIZENZEN.txt` aus;
  nur die Schicht trug sie. `bauen.sh` legt sie nach
  `/usr/share/doc/myelith/` und prüft nach dem Bau, dass sie im Abbild
  liegen.
- `ANLEITUNG.md`, `ANLEITUNG.en.md` und `README.md` sagen, wo Lizenzen und
  Quellen liegen. ⚠️ Buildroot nennt seine Auskunft selbst keine fertige
  Lieferung; die Lizenzangaben der Pakete sind nicht einzeln geprüft.
  Keine Rechtsberatung.

### v0.4.2 (2026-11-10): Der zweite Bau braucht kein Netz

- 📌 **Fund 699:** Das README sagte, die Programme für GolemOS bauten aus
  dem Vorrat ohne Netz. Sie taten es nicht: Der Programmschritt holte bei
  jedem Lauf `rust`, `cargo` und `build-base` über `apk add`, und ohne Netz
  brach er dort ab (gemessen: `apk` Rückgabe 3).
- Die Werkzeuge stehen jetzt in einem eigenen Abbild je Architektur
  (`Containerfile.programme`, `golemos-programmbauer-<arch>`), das einmal
  mit Netz entsteht und den Übersetzer bis zum Löschen festhält (Rust
  1.99.1, GCC 15.2.0). `bauen.sh` fährt den Programmschritt mit
  `--network none`.
- Nachgesehen: Die Quellen von Buildroot liegen nach dem ersten Bau
  vollständig im Volumen. `make source` ohne Netz gelingt für Basis und
  Schicht beider Architekturen; mit leerem Quellenlager scheitert es.
- Beide Architekturen damit neu gebaut (aarch64 knapp 4 min, x86_64
  7 min, je Rückgabe 0, kein Download im Protokoll) und unter QEMU mit
  `golemos.probe` gestartet: Kern 7.1.13, `myelith`, `myl`, `myl-node`
  vorhanden, `curl` mit OpenSSL 3.6.5. Die Prüfsummen der Schichten sind
  unverändert. ⚠️ Der Schritt für Buildroot hat weiter Netz: Er braucht
  es nicht, solange die Konfiguration keine neuen Pakete bringt, aber er
  ist nicht abgeriegelt.

### v0.4.1 (2026-11-10): Lizenztexte in der Schicht

- Die Schicht legt `LICENSE.md`, `NOTICES.md` und `DRITTLIZENZEN.txt` mit ins Abbild.

### v0.4.0 (2026-11-09): Buildroot 2026.08.1

- Basis auf Buildroot 2026.08.1 (Kern 7.1.13, GCC 15.3, glibc 2.44): OpenSSL 3.6.5
  statt 3.6.2 und expat 2.9.0 statt 2.8.1, wegen der offenen CVEs. Die Reihe 2026.02
  hat keine neuere Fassung bekommen. ⚠️ Das ist keine Langzeitreihe; die nächste
  Fassung ist eine eigene Entscheidung.
- Beide Architekturen gebaut (rund 22 min je Bau) und unter QEMU gestartet:
  Anmeldung, Netz, `myl-node`, `myelith`, Herunterfahren. Noch nicht erneuert:
  `abbild/fertig` und die Schichten für das Fenster.
- 📌 `schicht-bauen.sh` baut wieder: Die Quelle für das Fenster schloss `MODELS` ganz
  aus, und die Laufzeit bindet inzwischen den Katalog daraus ein.

### v0.3.4 (2026-11-08): Die Anleitung kennt Knoten und Fenster

- `ANLEITUNG.md` und `ANLEITUNG.en.md`: Abschnitte „Den Rechner als Knoten betreiben“ und „Das Fenster dazuholen“. Beide sagen, was noch fehlt: ein öffentliches Netz und eine Schicht zum Herunterladen. Der Knoten braucht den Myelith-Ordner wegen der Selbstprüfung, aber kein Modell.
- **Fund 672:** Der Hinweis von `golem-knoten einrichten --erzeuger ja` behauptete, genau ein Knoten im Netz baue die Blöcke. Das stimmt seit den rotierenden Erzeugern nicht mehr; er nennt jetzt den Stimmsatz der Genesis. Nur Text, die Abbilder ändern sich dadurch erst beim nächsten Bau.

### v0.3.3 (2026-10-26): Die Proben räumen ab (Fund 653; `golem-einrichten` 0.3.3)

- 📌 **Fund 653:** Jede Probe legte über `proben::ordner` ein Verzeichnis
  im Temp an und liess es liegen, 13 je Lauf. Jetzt gibt `ordner` einen
  Wächter zurück (`Probeordner`), der das Verzeichnis beim Verlassen der
  Probe löscht, auch wenn sie scheitert. Nachgeprüft: 78 Proben mit leerem
  Probeordner, danach leer.
- `golem-einrichten` 0.3.3.

### v0.3.2 (2026-10-22): Eine Werkzeugkette für alle Kisten (`golem-einrichten` 0.3.2)

- `rust-version` auf 1.99 in `golem-einrichten`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code; die Abbilder müssen deshalb nicht neu gebaut werden.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

### v0.3.1 – 2026-10-21 (die Modelle heißen nach Tieren; `golem-einrichten` 0.3.1)

Die Projektmodelle heißen seit heute nach Tieren, mit Größe und Speicherform als Anhängsel (Festlegung des Projektinhabers): `myelith-0.6b` ist `sparrow-0.6b-int8`, `myelith-4b` `pigeon-4b-int8`, `myelith-30b-a3b` `magpie-30b-a3b-int8`, `myelith-35b-a3b` `raven-35b-a3b-int8`. In den Dateien eines Artefakts steht kein Name, also bleiben Abdrücke und Register gültig. Anleitungen, Hinweisdatei und Proben mitgezogen.

### v0.3.0 – 2026-10-13 (der Knoten als Dienst, die Oberflächen-Schicht, udev und seatd in der Basis)

**Auftrag des Projektinhabers:** Desktop und Fensterclient auf der
Basisinstallation ohne Vorwissen ermöglichen, ohne das Repositorium zu
belasten, und alles, was mit dem Betrieb eines Knotens zu tun hat, über
GolemOS statt eines Nix-Servers vorbereiten (Entscheidungen: Kiosk,
Schicht auf der Datenpartition, „ohne Vorwissen“, NixOS-Modul bleibt als
Alternative).

- **Neu: `golem-knoten`** (`overlay/usr/sbin/`), `S50knoten`, das
  Dienstkonto `myl-knoten` (900) über `konfigurationen/benutzer.tabelle`,
  und `myl-node` im Abbild, statisch gegen musl wie die übrigen. Belegt mit
  einer neuen Knotenprobe (`golemos.knotenprobe` auf der Kernzeile, in
  `S99probe`): unter QEMU aarch64 hört der Knoten nach einer Sekunde auf
  `0.0.0.0:4001`, läuft unter Konto 900, Beobachtung (4151) und Tür (4160)
  nur auf `127.0.0.1`, `stoppen` beendet ihn ordentlich; `KNOTEN_OK`.
- 📌 **Die erste Knotenprobe fand einen echten Fehler:** BusyBox' `setpriv`
  kann kein Konto wechseln (`--reuid` steht dort nur im Kommentar), der
  Knoten brach fünfmal mit dessen Hilfe ab, und die Startbegrenzung hielt
  ihn an, wie sie soll. Jetzt das `setpriv` aus util-linux
  (`BR2_PACKAGE_UTIL_LINUX_SETPRIV`) mit vollem Pfad, und `bauen.sh`
  verlangt genau dieses statt irgendeines.
- 📌 **`bauen.sh` baut ein Paket neu, dessen Unteroption sich ändert.**
  Bisher wurde bei einer geänderten Einstellung nur der Zielbaum neu
  zusammengesetzt; util-linux blieb ohne `setpriv` und der Kern ohne sein
  neues Fragment, und nichts sagte es. Aus jedem geänderten Namen wird das
  Paket abgeleitet und nur dieses verworfen und neu gebaut (gemessen:
  `linux` und `util-linux`, sonst nichts).
- **Neu: der Kern bringt die Anzeige fest eingebaut mit**
  (`konfigurationen/kern-anzeige.fragment`: `simpledrm` für den Bildschirm
  jeder UEFI-Maschine, `virtio-gpu` und `virtio-input` für virtuelle
  Maschinen). `bauen.sh` prüft nach dem Bau, dass jede Zeile im Kern
  angekommen ist.
- 📌 **Behoben: der Kern für x86_64 konnte die Schicht nicht einhängen.**
  Die Kernvorgabe für aarch64 bringt squashfs mit, die für x86_64 nicht;
  beim Neubau der Basis für x86_64 zeigte die Kernkonfiguration Loop, aber
  kein squashfs. Die Schicht wäre geholt und geprüft worden und dann am
  Einhängen gescheitert. Jetzt stehen `CONFIG_BLK_DEV_LOOP`,
  `CONFIG_SQUASHFS` und `CONFIG_SQUASHFS_ZLIB` im selben Fragment, und
  `bauen.sh` prüft sie mit; gemessen nach dem Neubau: alle drei im Kern.
- **Neu: udev und seatd in der Basis** (`BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_EUDEV`,
  `seatd`). Sie brauchen Vorrechte und gehören deshalb in die geprüfte
  Basis, nicht in eine geholte Schicht. Nebenbei lädt udev Kernmodule für
  erkannte Hardware jetzt von selbst.
- **Neu: die Oberflächen-Schicht.** `schicht-bauen.sh` baut mit demselben
  Buildroot-Baum in eigenem Ausgabeordner ein squashfs (gzip, das kann der
  Kern) mit cage, GTK3, WebKitGTK, Mesa (softpipe, virgl) und dem Fenster,
  das mit rustup gegen den sysroot der Schicht gebaut wird, aus einer Kopie
  der Quellen im Volumen (`tauri-build` schreibt sonst ein Linux-Schema ins
  Repositorium). Es zählt die Einstellung nach (was zuletzt gilt) und
  prüft, dass jede Bibliothek des Fensters in der Schicht liegt.
  `--nur-pruefen` ist der kurze Lauf vor dem langen. 📌 **Gebaut wird mit
  so vielen Fäden, wie der Speicher trägt** (zwei Gigabyte je Faden): Mit
  einem Faden je Kern wurde der Übersetzer in WebKitGTK in einer
  Docker-VM mit 7,75 GiB siebenmal vom Kern abgeschossen, mit drei Fäden
  noch einmal in WebCore, mit zwei ein drittes Mal; jetzt dreieinhalb
  Gigabyte je Faden, und nach einem Abschuss halbiert das Skript die Fäden
  und baut am Stand weiter, bis zu einem. Gemessen: Mit einem Faden lief
  WebKitGTK durch, der ganze erste Bau der Schicht brauchte rund
  fünfeinhalb Stunden; die Schicht wiegt **102 MB**.
- 📌 **Drei Fallen im Querbau des Fensters**, alle drei erst sichtbar,
  nachdem WebKit durch war:
  - **Der Linker steht in `RUSTFLAGS`, nicht in
    `CARGO_TARGET_<ZIEL>_LINKER`.** Bau-Container und Schicht tragen auf
    aarch64 dieselbe Kennung, also galt die Zieleinstellung auch für
    Bauskripte: Sie wurden gegen die glibc der Schicht (2.39) gelinkt und
    liefen im Container nicht (`GLIBC_2.39 not found`). `RUSTFLAGS` gilt
    bei ausdrücklichem `--target` nur für den Zielcode.
  - **pkg-config kommt aus Buildroot** (`host/bin/pkg-config`); der
    Bau-Container hat keines, und dieses kennt die Sysroot von sich aus.
  - **Das Fenster baut ohne X11** (`--no-default-features`,
    `myl-oberflaeche` 0.71.0 mit eigenem Schalter `x11`, CLIENT v0.147.0):
    Die Schicht hat nur Wayland, und Tauri verlangte sonst `gdk-x11-3.0`.
- **Neu: `golem-schicht`** und `S70oberflaeche`, dazu
  `schicht/oberflaeche/usr/bin/golem-kiosk` (Umgebung, Tastatur aus der
  Wahl des Assistenten, Zeichnen in Software) und
  `golem-einrichten schicht-eintreten` (`src/schicht.rs`): `chroot`,
  Gruppen, Gruppe und Konto ablegen, Umgebung leeren, erst dann das erste
  Programm der Schicht.
- **Neu: die Schichtprobe** (`golemos.schichtprobe` auf der Kernzeile, in
  `S99probe`): holt eine Schicht von der Datenpartition
  (`/daten/oberflaeche-<arch>.sqfs` samt Prüfsumme), startet sie, prüft, dass
  Kompositor und Fenster unter Konto 1000 laufen, hält sie nach vier Minuten
  an und verlangt, dass danach nichts eingehängt bleibt. Das Bild nimmt der
  Prüfstand von außen über den QEMU-Monitor auf, solange `SCHICHT_LAEUFT`
  dasteht. 📌 **Sie fand zwei Fehler, die kein Bau und keine Probe auf dem
  Mac gefunden hätte:**
  - **seatd:** Buildroot bringt mit dem Dienst ein eigenes `S70seatd` mit,
    das den Socket beim Start der Gruppe `video` gibt, und `golem-schicht`
    sah nur „läuft schon“. Das Konto der Schicht hat keine Zusatzgruppen,
    cage brach ab (`Could not connect to socket /run/seatd.sock: Permission
    denied`). Und `seatd -u` nimmt nur Namen, ein Konto `golem` gibt es aber
    nur in der Schicht. Jetzt entfernt `nach-dem-bauen.sh` das `S70seatd`,
    `golem-schicht` startet seatd selbst und gibt den Socket per `chown` mit
    der Nummer dem Konto der Schicht (0600); ein fremder seatd wird ersetzt.
  - **OpenGL:** GTK forderte einen Kontext für Desktop-OpenGL an. Mesa kann
    ihn über EGL erzeugen, die Schicht hat aber keine Bibliothek für dessen
    Funktionen (kein GLX, kein glvnd), und libepoxy brach das Fenster ab
    (`Couldn't open libGL.so.1 or libOpenGL.so.0`). Gefunden mit einem
    Diagnoselauf, der vier Zeichenwege in einem Start nacheinander probierte:
    Nur mit `GDK_GL=gles` überlebte das Fenster; damit stand es nach 45 s
    vollständig da, mit Zeichnen auf dem Prozessor in Skia zeigte es nach
    55 s erst den Ladeschirm. `golem-kiosk` setzt `GDK_GL=gles` jetzt immer.
- **Behoben: der Schalter für die Grafikkarte.** `golem-kiosk` las
  `GOLEM_KIOSK_GPU=1` aus der Umgebung, die `schicht-eintreten` vorher
  leert; er kam nie an. Jetzt eine Datei: `/daten/golemos/oberflaeche-gpu`.
- **Neu: die erste Einrichtung stellt die Werkzeugkiste `GolemOS` ein**,
  wenn der Myelith-Ordner auf der Datenpartition liegt (`golemos_kiste`).
- 📌 **Behoben: `S05daten` fand die Datenpartition nicht mehr**, sobald
  util-linux gegen udev gebaut ist. Das Skript läuft vor `udevd` (S10), und
  `lsblk` liest Partitionsname, Kennung und Typ dann nur aus der
  udev-Datenbank: Es sah `vda1`, aber ohne `PARTLABEL`. Gemessen unter QEMU
  x86_64 („Datenpartition: nicht gefunden"); nach dem Hochfahren fand
  dasselbe `lsblk` sie, und mit weggeschobenem `/run/udev` wieder nicht.
  Jetzt `blkid -t PARTUUID=…`, `blkid -t PARTLABEL=…`, die Platte aus sysfs
  und der Typ aus `blkid -p -s PART_ENTRY_TYPE`; alle vier im laufenden
  System ohne udev-Datenbank und ohne blkid-Zwischenspeicher geprüft;
  danach beide Basen neu gebaut und beide Schichtproben grün.
  ⚠️ **Auf aarch64 fiel es nicht auf**, weil util-linux dort aus der Zeit
  vor udev stammte: `bauen.sh` baut ein Paket neu, wenn sich seine eigene
  Einstellung ändert, nicht wenn eine neue Abhängigkeit dazukommt. Ein
  frischer Bau hätte es auf beiden Architekturen gezeigt.
- ⚠️ **Mit nur `console=ttyS0` auf der Kernzeile** liegen die Anmeldung auf
  `console` und die auf `ttyS0` am selben Gerät und teilen sich die
  Eingabe. Über GRUB tritt das nicht auf (dort steht `tty1` zuletzt); wer
  direkt mit QEMU startet, hängt `console=tty0` an.
- ✅ **x86_64 fertig: Basis und Schicht gebaut, Schichtprobe grün.** Die
  Schicht wiegt 104 MB; ihr erster Bau brauchte rund 2 Stunden 40
  Minuten (nach einem Speicherabschuss in WebKit mit einem Faden weiter).
  Unter QEMU x86_64 (q35, `-vga none`, virtio-gpu ohne 3D): Datenpartition
  eingehängt, Schicht geholt und geprüft, nach 3 s laufen cage (Prozess
  316) und Fenster (317) unter Konto 1000, das Bild zeigt nach 30, 110 und
  200 s das vollständige Fenster mit dem KI-Hinweis, `status` „läuft",
  `stoppen` hängt alles aus: **`SCHICHT_OK`**.
- ✅ **Beleg am Bildschirm: die Schichtprobe unter QEMU aarch64**
  (virtio-gpu ohne 3D, Software, 4 Kerne ohne Beschleunigung): Schicht
  geholt und geprüft, nach 4 s laufen cage (Prozess 300) und Fenster
  (Prozess 303) unter Konto 1000, das Bild 1280×800 zeigt nach 30, 110 und
  200 s dasselbe vollständige Myelith-Fenster mit dem KI-Hinweis; `status`
  meldet „läuft“, `stoppen` beendet es, nichts bleibt eingehängt:
  **`SCHICHT_OK`**. ⚠️ Die Meldungen von libEGL zu `fd -1` und
  `/dev/dri/card0` bleiben im Protokoll; sie sind die Suche nach einer
  Grafikkarte, bevor auf Software ausgewichen wird.
- Belege: `golem-einrichten` 78 Proben grün (neu: drei in `schicht.rs`,
  `die_golemos_kiste_nur_auf_der_datenpartition`,
  `jede_karte_kennt_auch_die_oberflaeche`, vier in `tests/schicht.rs`:
  heile Schicht übernommen, falsche Prüfsumme hält alles an und `status`
  meldet eine später veränderte, ohne Prüfsumme oder über http
  abgewiesen, Konto überall 1000), Clippy ohne Befund. Gegenprobe: mit
  abgeschalteter Prüfsumme wird die Probe rot.

### v0.2.1 – 2026-09-30 (die Werkzeugkisten liegen jetzt unter `AGENT_LAYER`)

Die Kisten des örtlichen Agenten sind von `CLIENT/werkzeugkisten` nach
`AGENT_LAYER/local-toolkits` gezogen (AGENT_LAYER v0.25.0). Mitgezogen:
die Vollständigkeitsliste eines Klons (`klon.rs`), die Mitnahme
(`mitnahme.sh`, auf dem Stick heißt der Ordner weiter `werkzeugkisten`) und
die Probe, die die Sprachen des Clients gegen die eigenen hält: Sie las
`einstellungen.rs`, die Sprache liegt jetzt in `agentenwahl.rs` beim
Agenten. Der Kopf von `mitnahme.sh` nennt keine einzelne örtliche Kiste mehr.
**Beleg:** `golem-einrichten` 69 grün, Clippy ohne Befund.

### v0.2.0 – 2026-09-25 (Sprache und Tastatur am Anfang, Myelith ohne Installation benutzen, der ganze Myelith-Ordner kommt immer mit, GolemOS schreibt selbst weitere Sticks, der Stick heißt „Golem", eine Anleitung für Laien)

**Auf Wunsch des Projektinhabers**, drei Dinge:

- **„Golem" statt `GOLEM-DATEN`.** Unter diesem Namen erscheint die
  Datenpartition an jedem Rechner. Kleinbuchstaben in einem FAT-Namen
  meldet `mkfs.fat` als Warnung für alte Systeme; macOS zeigt
  `/Volumes/Golem` (gemessen). Der Name steht in `S05daten` und in
  `golem-einrichten` (`DATEN_NAME`), eine Probe hält beide gleich (rot,
  wenn `S05daten` einen anderen nennt). Ältere Sticks benennt `S05daten`
  beim nächsten Start mit `fatlabel` um, ohne den Inhalt anzufassen.
- **Myelith ohne Installation.** Der Assistent bietet als Punkt **1**
  „Myelith jetzt benutzen, ohne etwas zu installieren" und startet die
  Konsole `myelith`; nach `/ende` kommt das Menü zurück. Ohne Modell
  sagt er, was fehlt. Er markiert den sinnvollen nächsten Schritt mit
  `<- empfohlen`: ohne Modell die Anleitung, mit Modell die Benutzung.
  Neue Reihenfolge: 1 benutzen, 2 Anleitung, 3 installieren, 4 weiteren
  Stick schreiben, 5 suchen, 6 Konsole, 7 Sprache und Tastatur.
- **Eine Anleitung für Laien**, deutsch und englisch
  (`ANLEITUNG.md`, `ANLEITUNG.en.md`): welches Abbild, Stick schreiben
  mit Etcher, Rufus oder `spread.sh`, Startmenü-Tasten je Hersteller,
  Secure Boot, amerikanische Tastatur (`/` auf der Bindestrich-Taste),
  was ohne Installation geht und was nicht, Installation, eine Tabelle
  für Fehlerfälle. Dazu liegt auf jedem Stick nach dem ersten Start
  `README.txt` in **Golem**, deutsch und englisch, damit schon der
  erste Blick am eigenen Rechner erklärt, was hineingehört.
- **Der ganze Myelith-Ordner, immer** (Festlegung des Projektinhabers).
  Anleitung, `README.txt` und Assistent verlangen den ganzen Ordner,
  nicht eine Auswahl; weglassen darf man nur, was ein frischer Klon gar
  nicht hat (`SYSTEM/full-build`, `SYSTEM/crates-lager`, Rohgewichte
  unter `MODELS`, `.venv` mit Verknüpfungen, die FAT32 nicht kann).
  Gemessen: Das Repositorium hat keine Datei über 4 GB. Der Assistent
  zeigt, ob der Ordner **vollständig** ist (`klon::VOLLSTAENDIG`,
  Stichproben; eine Probe prüft, dass das Repositorium selbst
  vollständig ist). **Bei der Installation kommt der Ordner immer
  mit**, auch wenn er auf einem anderen Laufwerk lag; freiwillig sind
  nur die übrigen Daten. Passt er nicht auf die Platte, wird nicht
  installiert. Gegenprobe: Käme er nur mit den übrigen Daten mit,
  werden zwei Proben rot.
- **GolemOS schreibt weitere Sticks**: Punkt **4** ruft `spread.sh` aus
  dem Myelith-Ordner, mit dessen eigenen Sicherungen. `spread.sh` läuft
  dafür jetzt auch ohne `sudo` (unter GolemOS ist man root) und zeigt
  den Fortschritt nur, wo `dd` das kann.

**Sprache und Tastatur am Anfang** (Wunsch des Projektinhabers). Beim
allerersten Start fragt der Assistent vor allem anderen nach der
Sprache (Deutsch, English) und der Tastatur (zehn Belegungen, die
Namen in ihrer eigenen Sprache), mit Ziffern, die fast überall gleich
liegen. Die Wahl liegt unter `/daten/golemos/`; die Sprache geht über
`myl setzen oberflaeche.sprache` auch an Myelith, die Tastatur lädt
`loadkeys` sofort und `S06myelith` bei jedem Start. Punkt **7** ändert
beides. Dafür kommt `kbd` ins Abbild, und `bauen.sh` prüft jede
angebotene Belegung; eine Probe verlangt, dass die Liste im Assistenten
und die in `bauen.sh` übereinstimmen. Alle Texte des Assistenten gibt es
deutsch und englisch; die Meldungen beim Start bleiben deutsch.

📌 **Eine Falle dabei:** Die Wahl der Sprache legt `client.json` an,
bevor ein Myelith-Ordner da ist, und darin steht die Vorgabe des
Clients. `S06myelith` stellte ein Modell nur ein, wenn die Datei
fehlte; der Stick hätte also ein Modell behalten, das es auf ihm nicht
gibt. Jetzt prüft es, ob das eingestellte Modell existiert.

**`spread.sh` schützt mehr als die Systemplatte.** Abgelehnt wird jetzt
auch jede Platte, auf der `/daten` oder das Abbild selbst liegt: Läuft
GolemOS von der Platte und liegt der Myelith-Ordner auf einem
USB-Laufwerk, wäre dieses sonst beschreibbar, während `dd` von ihm
liest. 📌 **Ein Verdacht, zurückgezogen:** `/proc/mounts` nennt die
Wurzel unter GolemOS `/dev/root`, und ich hatte daraus geschlossen,
`findmnt` finde die Systemplatte nicht und der laufende Stick sei
beschreibbar. Gemessen nennt `findmnt` `/dev/sda2`; der alte Schutz
hätte gegriffen. Belegt ist im Durchlauf, dass `spread.sh` den laufenden
Stick ablehnt (Rückgabe 4).

**Belegt:** `golem-einrichten` 69 Proben, Clippy ohne Warnung.
Gegenproben: Der Ordner kommt nur mit dem Rest mit (zwei Proben rot);
`S05daten` nennt einen anderen Namen (Bindungsprobe rot); eine
Tastaturkarte fehlt in `bauen.sh` (Probe rot). Durchlauf unter QEMU mit
UEFI auf **x86_64 und aarch64**, je zehn Phasen grün, mit dem ganzen
Myelith-Ordner auf dem Stick (Tabelle in `abbild/fertig/HERKUNFT.md`).
Die Myelith-Konsole antwortete vom Stick in 205 s (aarch64) und 451 s
(x86_64) unter Emulation. ⚠️ Ihre Antwort mit dem 0,6B-Modell war
falsch („Die Hauptstadt ist Berlin"); das ist die Agentenschleife mit
dem kleinsten Modell, `myl frage` sagt auf demselben Stick „Paris".

**Was ohne Installation geht:** Gespräch, Agent mit Werkzeugen,
Wissensmappen, mit Kabel die Websuche; alles bleibt auf dem Stick.
**Was nicht geht:** Fenster, Sprache, Sehen, WLAN.

**Macs mit Apple-Chip** starten GolemOS nicht vom Stick (kein UEFI).
Der Weg über eine virtuelle Maschine ist offen; die Anleitung sagt das.

### v0.1.0 – 2026-09-25 (GolemOS kommt ins Repositorium: Einrichtungsassistent, Myelith-Ordner auf dem Stick, Installation auf eine ganze Platte; Funde 472, 473 und 475 bis 480)

**Auf Festlegung des Projektinhabers ist GolemOS jetzt versioniert.**
Bis dahin stand der ganze Ordner in `.gitignore`, und ein frischer Klon
hatte weder die Skripte noch die Abbilder. Draußen bleibt nur, was jeder
Bau neu erzeugt (`aus/`, `bau/`, die Programme in `overlay/usr/bin/`).

**Neu: `golem-einrichten`, die 26. Kiste.** Drei Aufgaben:

- **Den Myelith-Ordner finden.** Bei jedem Start sucht `S06myelith` auf
  der Datenpartition, und wenn dort nichts liegt, auf allen übrigen
  Laufwerken (nur lesend eingehängt unter `/medien/`), bis zwei Ebenen
  tief nach der Marke `INTEGER_LLM/scripts/build_artifacts.sh`. Der
  Fund landet in `/run/golemos/wurzel`, und `profile.d/golemos.sh`
  setzt daraus `MYELITH_WURZEL`. Mehr braucht der Client nicht: Er
  kennt die Variable und löst relative Artefaktpfade gegen den Klon auf.
  Gibt es noch keine Einstellungen, stellt `myl setzen` das größte
  Modell ein, das in sechs Zehntel des Arbeitsspeichers passt.
- **Der Assistent.** Erscheint bei der ersten Anmeldung von selbst, bis
  jemand „Zur Konsole" wählt. Zeilenweise statt Vollbild, weil GolemOS
  oft über eine serielle Leitung bedient wird.
- **Die Installation.** Plan und Ausführung sind getrennt: Der Plan ist
  eine reine Rechnung aus `lsblk -P` und wird gezeigt, bevor irgendetwas
  geschrieben wird. Kopiert werden die EFI-Partition und **die ruhende
  Wurzel** (läuft das System von A, kommt B auf beide Zielwurzeln, denn
  eine eingehängte Wurzel kann mitten in einem Schreibvorgang stehen).
  Die Datenpartition wird neu angelegt, bekommt die Marke `.gedehnt`
  (sonst formatierte der erste Start sie neu) und auf Wunsch den Inhalt
  des Sticks. Zum Schluss wird `grub.cfg` der Platte auf deren eigene
  Kennungen umgeschrieben.

**Fund 472: Der Startbefehl nach dem Bau nannte immer x86_64.** Nach
einem aarch64-Bau stand dort `qemu-system-x86_64` mit `aus/bzImage`,
einem Kern, den es dort nicht gibt, in einem Verzeichnis ohne
Architektur. Jetzt aus `$ARCH` gebildet.

**Fund 473: Die Selbstprobe meldete `ANTWORT_OK` ohne Antwort.** Die
Mitnahme übernahm `denken: true`, alle 24 Token waren Überlegung, und
die Rückgabe null galt als Erfolg. Jetzt fragt die Probe mit einer Kopie
der Einstellungen ohne Denken und verlangt „Paris".

**Fund 475: Die Mitnahme kopierte Python-Bytecode des Wirts mit.**
`__pycache__` fällt jetzt beim Kopieren weg.

**Fund 476: macOS und Windows sahen die Datenpartition nicht.** Sie
trug FAT32, damit jeder Rechner daraufschreiben kann, aber den GPT-Typ
„Linux-Dateisystem", und an dem entscheiden macOS und Windows, ob sie
überhaupt nachsehen. macOS meldete „kein Dateisystem". Mit „Microsoft
Basic Data" hängt macOS `GOLEM-DATEN` sofort ein (Gegenprobe an
derselben Platte, nur der Typ geändert). **Ohne diese Behebung hätte der
Weg „Ordner auf den Stick ziehen" nie funktioniert.** `S05daten` stellt
den Typ auf älteren Sticks beim nächsten Start um.

**Fund 477: Auf aarch64 blieb die serielle Leitung nach `init` stumm.**
`grub.cfg` und `inittab` nannten fest `ttyS0`; ARM-Maschinen heißen dort
`ttyAMA0`. Beim Start von Platte kam keine Meldung und keine Anmeldung
mehr an. Jetzt je Architektur eingesetzt. 📌 Der Wächter in
`nach-dem-bauen.sh` übersprang eine vorhandene Zeile, und Buildroot baut
`target/` nicht neu auf; ein bestehender Baubaum hätte `ttyS0` für immer
behalten. Die Zeile wird jetzt ersetzt, dreimal hintereinander geprüft.

**Fund 478: Zwei Stellen setzten voraus, dass unter `SYSTEM/` keine
Kiste liegt.** Die vier CI-Schleifen über `*/*/Cargo.toml` (MSRV,
Clippy, cargo-deny) hätten `SYSTEM/golemos/einrichten` still
übergangen, und `notices.py` nahm alles unter `SYSTEM/` aus, gemeint war
das Kistenlager. Beides berichtigt, bevor es etwas gekostet hat.

**Fund 479: Stick und installierte Platte verwechselten sich.** Die
Wurzel wurde über den Namen `golem-wurzelA` gesucht, und nach einer
Installation trägt die Platte denselben Namen. Gemessen: Vom Stick
gestartet, die Platte zuerst gefunden, kamen Wurzel und Daten **von der
Platte**, und der Assistent hielt die Platte für die Startplatte und bot
den Stick als Ziel an. Jetzt würfelt `nach-dem-abbild.sh` je Bau drei
Partitionskennungen aus und setzt sie in `genimage.cfg` und `grub.cfg`
ein; die Installation schreibt sie auf die neuen der Platte um, und
jede Kernzeile nennt die Datenpartition (`golemos.daten=PARTUUID=…`),
die `S05daten` zuerst nimmt.

**Fund 480: Aus einem frischen Klon wäre der Bau gescheitert.**
`bauen.sh` setzte das ausgepackte Kistenlager und `overlay/usr/bin/`
voraus; beides ist ausgeschlossen und fehlt in einem Klon. Es fiel nie
auf, weil GolemOS bisher nur auf einer Maschine gebaut wurde, auf der
beides längst lag. Jetzt packt `bauen.sh` den Vorrat selbst aus und
legt das Verzeichnis an. Belegt mit einer Kopie genau der Dateien, die
ein Commit enthält: Vorrat ausgepackt, alle drei Programme statisch,
Abbild gebaut. ⚠️ Buildroot kam dabei aus dem vorhandenen Bauvolumen;
ein Bau auf einem Rechner ohne dieses Volumen lädt die Quellen einmal
herunter.

**Freigegeben:** `abbild/fertig/golemos-x86_64.img.xz` (32,5 MB) und
`golemos-aarch64.img.xz` (53,5 MB), genau die beiden Abbilder aus dem
Durchlauf unten, mit Prüfsumme und nachgezogener `HERKUNFT.md`.

**Belegt:** `golem-einrichten` mit `cargo test`, 49 Prüfungen, Clippy
ohne Warnung; vier Gegenproben am jetzigen Code (Bestätigung,
Startplatte, ruhende Wurzel, fremde Kennung) machen je eine Probe rot.
📌 Die für die fremde Kennung biss zuerst nicht: Die Probe verlangte nur
„irgendein Fehler", und den lieferte eine andere Prüfung. Jetzt stehen
gültige Zeilen daneben, und verlangt wird die Meldung.

**Belegt von Anfang bis Ende**, unter QEMU mit UEFI-Firmware, ohne Netz,
bedient ausschließlich über die serielle Konsole, so wie ein Mensch es
täte (Stick 4 GB, Zielplatte 4 GB, 0,6B-Modell):

| Phase | x86_64 | aarch64 |
|---|---|---|
| erster Start vom Stick: Datenpartition gedehnt, Assistent erscheint, Anleitung, Abschalten | ✅ 36 s | ✅ 35 s |
| Myelith-Ordner auf `GOLEM-DATEN` (Marke, Modell, Werkzeugkisten) | ✅ | ✅ |
| zweiter Start: Ordner gefunden, Modell eingestellt, „Paris" vom Stick, Installation über den Assistenten | ✅ 160 s | ✅ 138 s |
| nur die Platte: Kennungen der Platte auf der Kernzeile, Daten mitgenommen, „Paris" | ✅ 64 s | ✅ 58 s |
| Platte, der Stick steckt noch: Wurzel und Daten von der Platte | ✅ 33 s | ✅ 34 s |
| Stick, die Platte wird zuerst gefunden (Fund 479): Wurzel und Daten vom Stick, Startplatte richtig erkannt | ✅ 33 s | ✅ 34 s |

Die Antwort war auf beiden Architekturen dieselbe: „Die Hauptstadt von
Frankreich ist **Paris**."

Vor der Behebung von Fund 479 scheiterte die letzte Zeile: Wurzel `vda2`
und Daten `vda4` kamen von der Platte, und `golem-einrichten platten`
nannte die Platte als Startplatte.
