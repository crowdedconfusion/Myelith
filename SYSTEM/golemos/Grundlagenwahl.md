# GolemOS

Ein Betriebssystem, das genau eine Aufgabe hat: **Myelith bedienen.**

> **Stand:** Entscheidungsgrundlage, noch kein Bau.
> **Ziel:** x86_64 und aarch64.
> **Stufe 1:** nur Konsole, kein Desktop.
> **Offline:** die **Installation**, nicht der Bau.

---

## Die vier Anforderungen, und was sie wirklich bedeuten

| Anforderung | Was daraus folgt |
|---|---|
| **kleinster Speicher** | jedes Megabyte begründet, kein Paket „weil es dazugehört" |
| **offline installieren** | alles Nötige liegt auf dem Datenträger; der **Bau** darf ins Netz |
| **alles, was Myelith braucht** | siehe unten, und das ist erstaunlich wenig |
| **erst ohne Oberfläche** | ⚑ **damit fällt WebKitGTK weg**, und das war der einzige Punkt, der das Vorhaben kippen konnte |

⚑ **„Offline installieren" ist viel billiger als „offline bauen".** Ein
offliner Bau hiesse: Kernquellen, libc, BusyBox und eine ganze
Werkzeugkette im Repositorium, je nach Grundlage 2 bis 10 GB. Eine
offline **Installation** heisst nur, dass auf dem Datenträger alles
liegt, was auf die Platte kommt. Bei einem Abbild ist das von selbst
erfüllt.

---

## Was Myelith wirklich braucht

**Gemessen an den installierten Programmen:**

| Teil | Grösse |
|---|---|
| `myelith` (Konsolenclient) | 6,7 MB |
| `myl` (Kommandozeile) | 6,5 MB |
| `myl-node` | 12,4 MB |
| **zusammen** | **25,6 MB** |

Dazu Kern, libc, BusyBox, Bootloader. ⚑ **Ein solches System liegt bei
40 bis 60 MB.**

**Die Sinne sind jede einzeln freiwillig**, und das ist Projektregel und
nicht Zufall: `ffmpeg` (rund 70 MB), die llama-Werkzeuge, `whisper-cli`,
`piper`, `python3` mit `pypdf`. **Eine fehlende Fremdkiste kostet eine
Funktion, nie den Start.** Sie kommen in eine zweite Stufe.

⚑ **Was NICHT hinein muss:** die Kalibrierung. GolemOS **benutzt**
Artefakte, es baut keine. Damit bleiben torch und transformers draussen,
und das sind mehrere Gigabyte.

⚠️ **Und die Gewichte gehören nicht ins Abbild.** Das kleinste Artefakt
wiegt 0,9 GB, das grösste 34 GB. Sie kommen wie überall sonst über ein
Skript, nicht über das System.

---

## Die Grundlagen im Vergleich

| | Buildroot | Alpine | NixOS | Debian |
|---|---|---|---|---|
| Abbild, nur Konsole | **~50 MB** | ~80 MB | ~1 GB | ~300 MB |
| offline installieren | **von selbst** (Abbild) | lokales Paketverzeichnis | Store auf dem Abbild | nur mit voller DVD |
| libc | **wählbar, also glibc** | musl | glibc | glibc |
| beide Architekturen | ja, je ein `defconfig` | ja | ja | ja |
| eine Datei beschreibt alles | ja | nein | ja | nein |
| Paketverwaltung zur Laufzeit | **nein** | `apk` | `nixos-rebuild` | `apt` |
| Bau braucht | Linux, 1 bis 3 h | Linux | Linux | Linux |

⚑ **Der Nix-Flake im Wurzelverzeichnis gehört zum Kettenserver.** Dass
die eine Frage mit Nix beantwortet ist, beantwortet die andere nicht,
und 1 GB gegen 50 MB ist genau der Punkt, an dem GolemOS etwas anderes
will.

---

## Die Empfehlung: Buildroot

**Nicht weil es das bequemste ist, sondern weil jede Anforderung oben
genau darauf zeigt.**

* **Kleinster Speicher:** das ist sein Zweck, nicht ein Nebeneffekt.
* **Offline installieren:** ein Abbild wird auf die Platte geschrieben.
  Es gibt gar nichts nachzuladen.
* **Alles, was Myelith braucht:** libc ist eine **Wahl**. Mit glibc
  laufen unsere Programme und jedes Fremdprogramm ohne Portierung.
* **Beide Architekturen:** ein Baum, zwei `defconfig`.
* Startprogramm dabei: GRUB 2 für x86_64-EFI, U-Boot für aarch64.

### ⛔️ Der ernsthafte Einwand, und die Antwort darauf

**Ohne Paketverwaltung ist jede Sicherheitslücke ein neues Abbild.**
Bei einem Knoten, der am Netz hängt, ist das kein Randfall.

⚑ **Die Antwort heisst A/B, und sie ist besser als `apt upgrade`.**
Zwei Wurzelpartitionen: Das neue Abbild wird auf die **inaktive**
geschrieben, dann wird umgeschaltet und neu gestartet. Geht dabei etwas
schief, startet die alte. **Eine unterbrochene Aktualisierung kann das
System nicht zerlegen**, und das kann `apt` nicht zusagen. Der Preis
sind zwei Abbilder statt einem, also 100 MB statt 50.

### ⚠️ Was trotzdem Arbeit bleibt

1. **Die Kernkonfiguration ist unsere.** Ein minimaler Kern ohne den
   richtigen Platten-, Netz- oder Grafiktreiber startet auf fremder
   Hardware nicht. Distributionen schleppen Tausende Treiber mit, und
   zwar aus genau diesem Grund.
2. **Der Bau braucht Linux**, ein bis drei Stunden je Architektur und
   einige zehn Gigabyte Platte. Auf einem Mac also ein Container oder
   eine virtuelle Maschine. ⚠️ **Das kann keine Grundlage ändern:** Ein
   Linux-Abbild entsteht unter Linux.
3. **Die zweite Stufe kostet.** `ffmpeg` allein ist grösser als das
   ganze Grundsystem. Ob es hineingehört, ist eine eigene Entscheidung.

---

## Der Zuschnitt in Stufen

| Stufe | Inhalt | erwartete Grösse |
|---|---|---|
| **1** | Kern, libc, BusyBox, Bootloader, `myelith`, `myl`, `curl` | **~55 MB** |
| **2** | dazu `myl-node`, `openssh`, A/B-Umschaltung | ~75 MB |
| **3** | dazu die Sinne (`ffmpeg`, llama, whisper, piper, python3) | ~250 MB |
| **4** | dazu Wayland, ein Kioskkompositor, das Fenster | ~800 MB |

⚑ **Stufe 1 ist das Produkt, alles andere ist Zugabe.** Mit dem
Konsolenclient und einem Terminal ist ein Mensch arbeitsfähig; jede
weitere Stufe ist eine Entscheidung mit einem Preisschild.

---

## Der erste Schritt

⛔️ **Nicht das Abbild, sondern die Frage, die es kippen kann.**

Für Stufe 1 heisst sie: **Laufen unsere Rust-Programme auf einem
Buildroot-System?** Das ist keine Selbstverständlichkeit: Sie werden
hier gegen macOS gebaut, dort gegen ein frisch gebautes glibc, und
`myl-node` bringt Kisten mit, die C übersetzen.

Also: ein `defconfig` für x86_64, darin nur Kern, glibc, BusyBox,
GRUB und unsere drei Programme, gestartet in einer virtuellen Maschine.
**Wenn `myelith --hilfe` dort eine Zeile ausgibt, ist der Rest
Handwerk.**
