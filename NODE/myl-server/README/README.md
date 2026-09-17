# myl-server

> **Version:** 0.2.0
> **Datum:** 2026-09-16
> **Status:** ⚠️ **Ungeprüft auf NixOS.** Modul, Beispielkonfiguration
> **und der Bauweg** stehen: Die Flake gibt `packages.myl-node` und
> `nixosModules.myl-server` heraus, das Modul ist damit kein Umschlag
> mehr um „bau es dir selbst". Offen bleibt die **VM-Prüfung**
> (`nixosTest`); bis sie gelaufen ist, gilt jede Zeile hier als
> geschrieben und nicht als belegt.

**Was hier liegt:** alles, um barrierefrei einen MYL-SERVER unter NixOS
aufzusetzen: ein gehärtetes NixOS-Modul um `myl-node`, eine
Beispiel-`configuration.nix`, damit jeder Server dieselbe Grundlage hat,
und eine Anleitung zur Portweiterleitung.

⚑ **Kein Rust-Crate.** Dies ist Nix und Dokumentation, kein
Cargo-Projekt; deshalb keine Cargo-Version und keine Sperrdatei. Die
Fassung oben zählt die Entwürfe hier.

| Datei | Inhalt |
|---|---|
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
