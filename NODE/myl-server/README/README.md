# myl-server

> **Version:** 0.3.0
> **Datum:** 2026-11-19
> **Status:** Der Dienst für gewöhnliches Linux (`myl-einstieg.service`)
> ist in einem Debian-Behälter mit systemd gelaufen. ⚠️ **Das NixOS-Modul
> ist ungeprüft auf NixOS.** Modul, Beispielkonfiguration
> **und der Bauweg** stehen: Die Flake gibt `packages.myl-node` und
> `nixosModules.myl-server` heraus, das Modul ist damit kein Umschlag
> mehr um „bau es dir selbst". Offen bleibt die **VM-Prüfung**
> (`nixosTest`); bis sie gelaufen ist, gilt jede Zeile hier als
> geschrieben und nicht als belegt.

**Was hier liegt:** alles, um einen dauerhaft laufenden Knoten als Dienst
aufzusetzen: für Debian, Ubuntu, Fedora, Arch und Verwandte ein Dienst für
systemd samt Einrichtungsskript (der Einstiegsknoten der Anleitung in
`NODE/README/README.md`, „Ein Einstiegsknoten unter Linux, als Dienst“),
für NixOS ein gehärtetes Modul um `myl-node`, eine
Beispiel-`configuration.nix` und eine Anleitung zur Portweiterleitung.

⚑ **Kein Rust-Crate.** Dies ist Nix und Dokumentation, kein
Cargo-Projekt; deshalb keine Cargo-Version und keine Sperrdatei. Die
Fassung oben zählt die Entwürfe hier.

| Datei | Inhalt |
|---|---|
| `myl-einstieg.service` | Dienst für systemd außerhalb von NixOS: derselbe Zuschnitt wie das Modul, Einstellungen in `/etc/myl-einstieg/einstieg.env` |
| `einstieg-einrichten.sh` | richtet ihn ein (Dienstnutzer, Programm, Einstellungen), aktualisiert, gibt die Zeile für den Startsatz und die eigene Adresse aus |
| `modul.nix` | das NixOS-Modul `services.myl-server`: gehärteter systemd-Dienst, nur ein offener Port, Schlüssel aus dem Store heraus |
| `beispiel-configuration.nix` | die gemeinsame Grundlage; als Vorbild nehmen und die markierten Stellen anpassen |
| `README/Router-Anleitung.md` | Schritt für Schritt: Schlüssel, Portweiterleitung im Router, Erreichbarkeit, Aktualisieren |

## Die Sicherheitsvorgaben, in einem Satz

Nach außen erreichbar ist **nur** der P2P-Port; Tür, Beobachtung und
Ortsleitung bleiben auf der Rückschleife. Schlüssel liegen **nie** im
Nix-Store, sondern kommen über systemd-Credentials aus einer Datei, die
Root gehört. Der Dienst läuft als eigener, unprivilegierter Nutzer in
einer systemd-Sandbox. Kein UPnP.

## Die Begründungen

Warum genau dieser Zuschnitt gilt (nur ein offener Port, Schlüssel aus dem
Store heraus, systemd-Sandbox, kein UPnP), steht als Kommentar an Ort und
Stelle in `modul.nix` und in der `Router-Anleitung.md`. Wer an diesen
Dateien arbeitet, liest dort die Begründung.

## Changelog

### v0.3.0 – 2026-11-19 (der Einstiegsknoten unter Linux)

- **Neu: `myl-einstieg.service` und `einstieg-einrichten.sh`** für den
  Einstiegsknoten der Stufe A auf einem Linux-Rechner. Das Skript legt den
  Dienstnutzer `myl-einstieg` an, kopiert `myl-node` und die
  Konformitätsvektoren nach `/usr/local/lib/myelith` (der Knoten sucht
  sie neben dem Programm), schreibt `/etc/myl-einstieg/einstieg.env`
  (Name, Netz, Port, öffentliche Adresse, weitere Schalter) und startet
  den Dienst; dazu `--zeile` (Zeile für den Startsatz, als Dienstnutzer),
  `--adresse` (aus der Zeile `eigene_adresse` des Protokolls),
  `--stand` und `--aktualisieren`. Firewall, Router und DNS bleiben beim
  Betreiber; das Skript nennt die Befehle.
- **Im Behälter mit systemd erprobt** (Debian 12, systemd als PID 1):
  ohne Startsatz fünf Versuche, dann `failed` mit der Meldung des Knotens
  im Journal; mit Startsatz vier Validatoren, drei davon über
  `/dns4/einstieg/tcp/4150/p2p/…` eingestiegen, Prüfpunkte bestätigt,
  Neustart des Dienstes holt auf. `systemd-analyze security`: 1,3, „OK“.
- ⛔️ **Fund, auch im Modul: Netlink war verboten.** Mit
  `RestrictAddressFamilies` nur `AF_INET AF_INET6` endet der Start mit
  „Address family not supported by protocol“, denn libp2p beobachtet die
  Schnittstellen über Netlink. Beide erlauben jetzt `AF_NETLINK`.
- ⛔️ **Fund im Modul: kein `--netz`.** Der Dienst lief auf der
  Probekette, der Vorgabe des Knotens. Neu `netz` (Vorgabe `test-int4`)
  und `zeugenBereithalten` (Vorgabe an); `p2pPort` jetzt 4150 wie der
  Knoten (vorher 4001), ebenso Beispiel und Router-Anleitung. `golem-knoten`
  in GolemOS trug denselben Fund und ist gleichgezogen (GolemOS 0.8.2); die
  Probe `tests/golemknoten.rs`, die beide zusammenhält, hatte ihn gemeldet.
- ✅ **Mit Nix ausgewertet und gebaut** (Behälter `nixos/nix`, aarch64-linux,
  das festgehaltene nixpkgs der Flake; KVM für die VM-Prüfung gibt es auf
  dem Mac nicht): Das Modul wertet ohne Warnung aus, die erzeugte Unit trägt
  Netz, Port 4150, Netlink, die Startgrenze unter `[Unit]` und nur den
  P2P-Port in der Firewall. Das Paket `packages.myl-node` hatte **nie
  gebaut**; drei Funde, behoben in `flake.nix`: ohne `cargoRoot` fand der Bau
  die Sperrdatei nicht (kein Workspace); der Quellfilter verglich den ersten
  Pfadteil mit `SYSTEM/full-build` und traf nie, schloss aber `MODELS` ganz
  aus, auch den eingebetteten Katalog (jetzt nach Präfix, mit Ausnahme für
  `MODELS/llm/KATALOG.json`); das Paket legte keine Konformitätsvektoren bei
  (jetzt `$out/bin/conformance/vectors/op`). Danach gebaut, und der Knoten
  meldet ohne Angabe „konformitaet=bestanden vektoren=6“.
- **Modul:** `konformitaetspfad = null` heißt jetzt „die Vektoren neben dem
  Programm“ statt `--ohne-konformitaet`; ohne Prüfung nur noch ausdrücklich
  mit `ohneKonformitaet = true`.
- **Aufnahmetakt 60 statt 300 Sekunden:** Im Aufnahmetakt setzt der
  Knoten auch `/metriken` und `/bereit` neu; mit 300 hinkten sie im
  Versuch fünf Minuten nach. Das Wachstum der Protokolle deckelt der
  Knoten selbst (128 MiB).
- Fünf neue Proben in `NODE/myl-node/tests/serverzuschnitt.rs`: jedes
  Flag der Unit kennt der Knoten, nach außen nur der P2P-Port und ein
  Netz, Startgrenze in `[Unit]`, Unit und Modul härten gleich und
  erlauben Netlink, Skript und Unit meinen dieselben Pfade und Variablen.
  Gegenprobe (Netlink und `--ohne-tuer` entfernt): zwei rot.

### v0.2.1 – 2026-11-08 (der Hinweis zum Erzeuger)

- **Fund 672:** Die Beschreibung von `erzeuger` behauptete, in der üblichen Aufstellung erzeuge genau einer. Seit den rotierenden Erzeugern trägt der Schalter nur den Anlauf aus dem Stimmsatz der Genesis; danach erzeugt jeder volle Miner in den Slots, für die er gezogen ist. Nur Text.

### v0.2.0 – 2026-09-16 (der Bauweg steht, und der Dienst hält für den Dauerbetrieb)

⚑ **`packages.myl-node` in der Flake**, über `cargoLock.lockFile` und
damit **ohne eine einzige von Hand gepflegte Zahl**: Nix liest die
Sperrdatei, die ohnehin gepflegt wird. Möglich ist das, weil sie keine
Git-Quelle enthält; jede Git-Quelle bräuchte einen eigenen Hash. Dazu
`nixosModules.myl-server`, damit das Modul auffindbar ist.

⛔️ **Ein Quellfilter, und er ist kein Zierrat.** `src = ./.` nähme das
gemeinsame Bauverzeichnis, die Modellgewichte und die gebauten Artefakte
mit in den Store. Gemessen am 2026-09-16: allein das Bauverzeichnis sind
57,5 GiB in 327 115 Dateien. **Ein `nix build` ohne Filter füllt die
Platte, bevor die erste Zeile übersetzt ist.**

⛔️ **Ein Startfehler wird keine Schleife.** Bis hierher stand nur
`Restart=on-failure` da: Ein Knoten mit einem fehlenden Schlüssel
startet, fällt, startet, und das alle zehn Sekunden für immer, während
der Dienst sich als aktiv meldet. Jetzt fünf Versuche in fünf Minuten.
⚑ **An der Unit und nicht am Dienst:** `StartLimitIntervalSec` gehört
seit systemd 229 nach `[Unit]`, in `[Service]` täte es stillschweigend
nichts, und eine Begrenzung, die nichts tut, ist schlimmer als keine.

⚑ **Drei Optionen für den Dauerbetrieb.** `beobachtungPort` wählt den
Port des Beobachtungsendpunkts und `null` schaltet ihn ab; **die Adresse
ist nicht einstellbar**, denn was dort heraussieht, ist Aufklärung für
einen Fremden. Übergeben wird sie jetzt ausdrücklich, damit ein
Betreiber die Zusage in `systemctl cat` sieht statt im Quelltext des
Knotens. `aufnahmeSekunden` steht auf 300 statt der 30 des Knotens: Das
ist die Rate, mit der das Protokollverzeichnis wächst, und ein Server
läuft Monate statt Minuten. `erzeuger` schaltet die Blockerzeugung, ⚠️
und die tut in der üblichen Aufstellung genau einer.

⚑ **Zwei Direktiven mehr in der Sandbox.** `UMask=0077`, damit dem
Dienst gehört, was er anlegt: Das Zustandsverzeichnis steht zwar auf
0700, aber eine Berechtigung, die nur durch den Ordner darüber trägt,
trägt einmal. Und `ProcSubset=pid` als Ergänzung zu `ProtectProc`.

⚑ **Eine Warnung statt einer Zusicherung**, wenn `zusatzflags` die Tür,
die Ortsleitung oder die Beobachtung enthält. `zusatzflags` ist
ausdrücklich der Weg für Fälle, die das Modul nicht abbildet; eine
Zusicherung machte daraus ein Verbot. Sichtbar muss es trotzdem sein.

⚑ **Und fünf Prüfungen, die ohne Nix laufen** und in `myl-node` liegen:
Sie halten Modul, Beispiel, Flake und den Aufrufparser des Knotens
zusammen.

### v0.1.0 – 2026-09-14 (Entwurf: Modul, Beispielkonfiguration, Anleitung)

Erste Fassung nach dem abgenommenen Entwurf. `modul.nix` mit
`services.myl-server` und der systemd-Härtung; `beispiel-configuration.nix`
als gemeinsame Grundlage; `Router-Anleitung.md`. ⚠️ Ungeprüft auf NixOS;
der reproduzierbare Bau (S1) und ein `nixosTest` (S2) stehen aus.
