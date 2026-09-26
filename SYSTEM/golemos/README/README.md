# golemos (die Linux-Distribution, die nur Myelith bedient)

> **Version:** 0.2.0 (`golem-einrichten` 0.2.0)
> **Datum:** 2026-09-25
> **Status:** Startet vom Stick und von Platte, findet einen
> Myelith-Ordner auf dem Stick oder einem zweiten Laufwerk und
> installiert sich mit einem Assistenten auf eine ganze Platte. Belegt
> unter QEMU mit UEFI auf x86_64 und aarch64, von Anfang bis Ende. Offen: ein Lauf
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

Die Kiste hat keine einzige Abhängigkeit: Sie löscht Platten, und was
sie tut, soll sich in ihrem eigenen Quelltext lesen lassen.

---

## Changelog

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
