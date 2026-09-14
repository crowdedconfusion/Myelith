# myl-server

> **Version:** 0.1.0
> **Datum:** 2026-09-14
> **Status:** ⚠️ **Entwurf, ungeprüft auf NixOS.** Modul und
> Beispielkonfiguration stehen; der reproduzierbare Bau des Binaries
> (Entwurf S1) und die VM-Prüfung (`nixosTest`) fehlen noch.

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

### v0.1.0 – 2026-09-14 (Entwurf: Modul, Beispielkonfiguration, Anleitung)

Erste Fassung nach dem abgenommenen Entwurf. `modul.nix` mit
`services.myl-server` und der systemd-Härtung; `beispiel-configuration.nix`
als gemeinsame Grundlage; `Router-Anleitung.md`. ⚠️ Ungeprüft auf NixOS;
der reproduzierbare Bau (S1) und ein `nixosTest` (S2) stehen aus.
