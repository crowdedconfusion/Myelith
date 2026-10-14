# golemos (die Linux-Distribution, die nur Myelith bedient)

> **Version:** 0.3.0 (`golem-einrichten` 0.3.0)
> **Datum:** 2026-10-13
> **Status:** Startet vom Stick und von Platte, findet einen
> Myelith-Ordner auf dem Stick oder einem zweiten Laufwerk und
> installiert sich mit einem Assistenten auf eine ganze Platte. Belegt
> unter QEMU mit UEFI auf x86_64 und aarch64, von Anfang bis Ende. Seit
> 0.3.0 betreibt es einen **Myelith-Knoten** als Dienst (`golem-knoten`,
> unter QEMU belegt) und nimmt eine **Oberflächen-Schicht** dazu
> (`golem-schicht`, das Myelith-Fenster im Vollbild). Offen: ein Lauf
> auf echter Hardware, die A/B-Umschaltung, eine eigene
> Kernkonfiguration.

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
