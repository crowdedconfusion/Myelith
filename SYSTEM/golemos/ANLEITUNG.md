# GolemOS: Myelith vom USB-Stick

**English:** [ANLEITUNG.en.md](ANLEITUNG.en.md)

GolemOS ist ein kleines Betriebssystem, das nur eine Aufgabe hat:
Myelith laufen zu lassen. Es passt auf einen USB-Stick. Du kannst
Myelith direkt vom Stick benutzen, ohne an deinem Rechner etwas zu
verändern, oder GolemOS später dauerhaft auf eine Platte installieren.

Diese Anleitung setzt kein Vorwissen voraus. Rechne beim ersten Mal mit
einer halben Stunde.

---

## Was du brauchst

| | |
|---|---|
| **Einen USB-Stick** | mindestens 4 GB für das kleinste Modell, besser 16 GB oder mehr. ⚠️ Alles, was darauf liegt, wird gelöscht |
| **Einen Rechner, von dem GolemOS starten soll** | fast jeder PC oder Laptop mit Intel- oder AMD-Prozessor, auch ein Mac mit Intel-Prozessor. Zu Macs mit Apple-Chip siehe [unten](#macs-mit-apple-chip) |
| **Deinen Myelith-Ordner** | den Ordner, in dem deine Modelle liegen (`INTEGER_LLM/artifacts/`) |
| **Einen zweiten Rechner** | um den Stick zu beschreiben und den Ordner daraufzuziehen. Es kann derselbe sein |
| **Ein Netzwerkkabel** | nur, wenn Myelith im Internet suchen soll. WLAN kennt GolemOS noch nicht |

**Welches Abbild?** Im Myelith-Ordner unter `SYSTEM/golemos/abbild/fertig/`:

- `golemos-x86_64.img.xz` für fast alle PCs und Laptops und für Macs mit
  Intel-Prozessor. **Wenn du unsicher bist, ist es dieses.**
- `golemos-aarch64.img.xz` für ARM-Rechner mit UEFI, etwa manche Server.

---

## Schritt 1: Das Abbild auf den Stick schreiben

Am einfachsten mit **balenaEtcher** (kostenlos, für Windows, macOS und
Linux, von `etcher.balena.io`):

1. Etcher öffnen, auf **Flash from file** klicken und
   `golemos-x86_64.img.xz` wählen. Entpacken musst du nichts.
2. **Select target**: deinen USB-Stick wählen. Achte auf die Größe,
   damit du nicht versehentlich eine andere Platte erwischst.
3. **Flash!** und warten, bis Etcher fertig ist.

Unter Windows geht auch **Rufus** (`rufus.ie`): Stick wählen, bei
„Startart" das Abbild wählen, **Start**.

Wer mit dem Terminal arbeitet (macOS, Linux), nimmt aus dem
Myelith-Ordner `sh SYSTEM/golemos/spread.sh`. Es zeigt ohne weitere
Angabe nur, welche Datenträger in Frage kommen, und schreibt erst, wenn
du den Namen des Sticks abtippst.

> Nach dem Schreiben zeigt dein Rechner den Stick vielleicht als
> „nicht lesbar" an oder bietet an, ihn zu formatieren. **Nicht
> formatieren.** Das ist normal; der lesbare Teil entsteht beim ersten
> Start.

---

## Schritt 2: Vom Stick starten

Der Rechner muss einmal vom Stick statt von seiner eigenen Platte
starten. Dafür hat fast jeder Rechner ein **Startmenü**, das du mit
einer Taste direkt nach dem Einschalten öffnest:

| Hersteller | Taste für das Startmenü |
|---|---|
| Dell, Lenovo, Acer, Toshiba | F12 |
| HP | F9 (oder Esc, dann F9) |
| ASUS | F8 oder Esc |
| MSI, ASRock, Gigabyte | F11 oder F12 |
| Mac mit Intel-Prozessor | ⌥ (Alt/Option) gedrückt halten, dann „EFI Boot" |

1. Stick einstecken, Rechner ausschalten.
2. Einschalten und sofort mehrmals die Taste drücken.
3. Im Menü den USB-Stick wählen (oft „UEFI: …" mit dem Namen des
   Sticks).

### ⚠️ Wenn der Stick nicht startet: Secure Boot

Viele Rechner starten nur Systeme mit einer Signatur von Microsoft.
GolemOS hat keine. Dann musst du **Secure Boot ausschalten**:

1. Beim Einschalten die Taste für die Einstellungen drücken, meist
   **F2** oder **Entf** (Del).
2. Unter „Security" oder „Boot" den Eintrag **Secure Boot** suchen und
   auf **Disabled** stellen.
3. Speichern (meist F10) und neu starten.

Auf Macs mit Intel-Prozessor und T2-Chip: im Wiederherstellungsmodus
(⌘R beim Start) das „Startsicherheitsdienstprogramm" öffnen, **Keine
Sicherheit** und **Starten von externen Medien erlauben** wählen.

Dein Windows oder macOS startet danach ganz normal weiter; Secure Boot
lässt sich jederzeit wieder einschalten.

---

## Schritt 3: Der erste Start

1. Ein Menü mit **GolemOS (A)** erscheint und startet nach fünf
   Sekunden von selbst.
2. Nach einigen Sekunden steht da `golemos login:`. Tippe **root** und
   drücke Enter. Ein Passwort gibt es nicht.
3. Der **Assistent** fragt zuerst nach der **Sprache** (1 Deutsch,
   2 English) und dann nach deiner **Tastatur** (1 Deutsch, 2 Schweiz,
   3 US-Englisch …). Enter nimmt die Vorgabe. Beides merkt sich der
   Stick; ändern kannst du es später mit Punkt **7**.
4. Dann zeigt er, was er gefunden hat, und markiert den nächsten
   sinnvollen Schritt mit `<- empfohlen`.

⚠️ **Bis du die Tastatur gewählt hast, ist sie amerikanisch belegt**;
dafür brauchst du aber nur Ziffern und Enter, und die liegen fast
überall gleich. Auf einer französischen Tastatur die Ziffern mit der
Umschalttaste tippen.

Beim ersten Start ist noch kein Modell da. Wähle **2** („Myelith-Ordner
auf diesen Stick bringen"). Der Assistent erklärt den nächsten Schritt
und schaltet den Rechner auf Wunsch aus.

---

## Schritt 4: Den Myelith-Ordner auf den Stick bringen

1. Stick abziehen und an deinen normalen Rechner stecken. Er erscheint
   jetzt als **Golem**, darin eine Datei `README.txt`.
2. Ziehe deinen **ganzen Myelith-Ordner** in **Golem**, samt den
   Modellen unter `INTEGER_LLM/artifacts/`. Der ganze Ordner, weil
   GolemOS daraus auch selbst neue Sticks schreibt und alles bei sich
   haben soll, was Myelith ausmacht. Er ist klein; die Modelle sind der
   große Teil.

   Weglassen darfst du nur, was ein frisch heruntergeladener Ordner gar
   nicht enthält: `SYSTEM/full-build` und `SYSTEM/crates-lager` (entstehen
   beim Bauen), die Rohgewichte unter `MODELS` (daraus werden Modelle
   gebaut, GolemOS braucht die fertigen) und Ordner namens `.venv`. Die
   enthalten Verknüpfungen, die der Stick nicht speichern kann, und
   ließen das Kopieren abbrechen.
3. Den Stick **sicher auswerfen** (Finder: Auswerfen; Windows: „Hardware
   sicher entfernen").

**Welches Modell passt?** GolemOS wählt selbst das größte, das in etwa
sechs Zehntel des Arbeitsspeichers passt:

| Modell | Größe | Arbeitsspeicher ab |
|---|---|---|
| `myelith-0.6b` | 0,9 GB | 2 GB |
| `myelith-4b` | 4,5 GB | 8 GB |
| `myelith-8b` | 8,8 GB | 16 GB |

> Der Ordner darf auch auf einem **zweiten Stick oder einer externen
> Platte** liegen, höchstens zwei Ordner tief. GolemOS sucht beim Start
> auf allen Laufwerken. Das Dateisystem muss FAT32 oder ext4 sein; exFAT
> und NTFS kann GolemOS noch nicht lesen.

---

## Schritt 5: Myelith benutzen, ohne etwas zu installieren

1. Wieder vom Stick starten (Schritt 2) und als **root** anmelden.
2. Der Assistent zeigt deinen Ordner und das Modell. Wähle **1**
   („Myelith jetzt benutzen, ohne etwas zu installieren").
3. Myelith weist dich darauf hin, dass du mit einer KI arbeitest, und
   beim ersten Mal auf ein paar Regeln für seine Werkzeuge. Lies sie
   und drücke jeweils Enter.
4. Myelith fragt nach einem **Design** und nach dem **Modell**. Mit den
   Pfeiltasten wählen und Enter drücken; das Vorgewählte passt, also
   reicht zweimal Enter.
5. Das Modell lädt einige Sekunden. Jetzt kannst du schreiben.
   Beenden mit **/ende** und Enter; danach bist du wieder im
   Assistenten.

Im Assistenten steht, ob dein Ordner **vollständig** ist. Steht dort
„UNVOLLSTAENDIG", nennt er, was fehlt; dann den ganzen Ordner noch
einmal kopieren.

**Was dabei alles geht:** das Gespräch, der Agent mit seinen Werkzeugen
(er arbeitet im Ordner `arbeit` auf dem Stick), die Wissensmappen und,
mit Netzwerkkabel, die Suche im Internet. Alles, was du einstellst und
erarbeitest, bleibt auf dem Stick gespeichert. Die Platte deines
Rechners wird dabei nicht angefasst.

**Was nicht geht:** das Fenster mit der Maus, die Sprachausgabe und das
Sehen. Dafür braucht es Myelith auf deinem normalen System. WLAN kennt
GolemOS noch nicht.

Du kannst GolemOS auf diese Weise dauerhaft benutzen; eine Installation
ist nicht nötig.

---

## Schritt 6 (freiwillig): GolemOS dauerhaft installieren

Das lohnt sich, wenn ein Rechner **nur noch** GolemOS laufen lassen
soll. Eine Platte ist schneller als ein Stick, und der Stick wird wieder
frei.

⛔️ **Die gewählte Platte wird vollständig gelöscht**, mit allem darauf,
auch einem Windows. Sichere vorher, was du behalten willst.

1. Vom Stick starten, im Assistenten **3** wählen.
2. Der Assistent listet alle Platten. Angeboten werden nur die, die in
   Frage kommen; bei den anderen steht, warum nicht (etwa „von dieser
   Platte läuft GolemOS gerade" für den Stick).
3. Nummer der Platte eingeben. Der Assistent zeigt, was er vorhat und
   was auf der Platte gelöscht wird.
4. **Dein Myelith-Ordner kommt immer mit**, damit das installierte
   GolemOS ihn hat. Der Assistent fragt nur, ob auch deine übrigen Daten
   vom Stick mitkommen sollen (Einstellungen, Arbeitsordner). Enter
   heißt ja.
5. **Zur Bestätigung den Namen der Platte eintippen**, so wie er in
   Klammern steht, etwa `sda` oder `nvme0n1`. Ein „j" reicht bewusst
   nicht.
6. Warten, bis „GolemOS ist auf … installiert" erscheint. Ausschalten,
   **den Stick abziehen** und einschalten.

Der Stick bleibt ein vollwertiges GolemOS; du kannst ihn weiter
benutzen oder für einen zweiten Rechner nehmen.

---

## Einen weiteren GolemOS-Stick schreiben

GolemOS kann sich selbst weitergeben, vom Stick wie vom installierten
System, denn dein Myelith-Ordner bringt die Abbilder und das Werkzeug
dafür mit.

1. Im Assistenten **4** wählen.
2. Den neuen Stick einstecken und Enter drücken. ⚠️ Alles darauf wird
   gelöscht.
3. Der Assistent zeigt die Datenträger. Gib den des neuen Sticks ein,
   etwa `/dev/sdb`.
4. Zur Bestätigung den Namen noch einmal abtippen. Das Schreiben dauert
   einige Minuten.

Der Stick, von dem GolemOS gerade läuft, und jede Platte, die kein
Wechseldatenträger ist, werden abgelehnt. Der neue Stick ist ein
frisches GolemOS; deinen Myelith-Ordner ziehst du danach wie in
Schritt 4 darauf.

---

## Wenn etwas nicht klappt

| Was passiert | Was hilft |
|---|---|
| Der Rechner startet einfach Windows oder macOS | Startmenü nicht erwischt: gleich nach dem Einschalten mehrmals die Taste drücken (Schritt 2) |
| Der Stick erscheint im Startmenü nicht, oder es kommt „Security Violation" | Secure Boot ausschalten (Schritt 2) |
| „kein Klon gefunden" | Der Ordner liegt zu tief oder `INTEGER_LLM/scripts/build_artifacts.sh` fehlt darin. Im Assistenten **5** sucht noch einmal |
| „UNVOLLSTAENDIG" | Es wurde nur ein Teil des Ordners kopiert. Den ganzen Myelith-Ordner kopieren (Schritt 4) |
| Das Kopieren auf den Stick bricht ab | Meist an einem Ordner `.venv` oder an `SYSTEM/full-build`; diese weglassen (Schritt 4) |
| „Noch ist kein Modell da" | Im Ordner fehlt `INTEGER_LLM/artifacts/<modell>` |
| Myelith antwortet sehr langsam | Das Modell ist für den Arbeitsspeicher zu groß. Ein kleineres Modell in den Ordner legen |
| „Golem" erscheint am eigenen Rechner nicht | Den Stick einmal von GolemOS starten; erst der erste Start richtet diesen Teil ein |
| Beim Tippen kommen falsche Zeichen | Im Assistenten **7** die richtige Tastatur wählen |
| Der Assistent kommt nicht mehr von selbst | Nach „Zur Konsole" ist das gewollt. Er startet mit `golem-einrichten` |

---

## Macs mit Apple-Chip

GolemOS startet auf Macs mit Apple-Chip **noch nicht vom Stick**: Diese
Macs haben kein UEFI, wie es GolemOS braucht.

Vorgemerkt ist der Weg über **Asahi Linux**: Dessen Installer richtet
auf Wunsch nur eine kleine UEFI-Umgebung ein (rund 3 GB) und startet
danach von selbst jeden USB-Stick mit UEFI, also auch GolemOS. Dafür
fehlt GolemOS noch ein Kern mit den Treibern für Apples Hardware, und
Asahi selbst unterstützt bisher Macs mit M1, M2 und M3; M4 und M5 sind
in Arbeit.

---

## Gut zu wissen

- **Es gibt kein Passwort.** Wer an der Tastatur sitzt, kann alles.
  GolemOS öffnet keinen Zugang über das Netz.
- **Myelith weist bei jedem Start darauf hin**, dass du mit einer KI
  arbeitest; das verlangt die KI-Verordnung der EU.
- **Selbstprobe:** Im Startmenü startet **GolemOS Selbstprobe** das
  System, prüft, ob Myelith antwortet, und schaltet wieder ab.
