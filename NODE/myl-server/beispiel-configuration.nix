# Beispiel-configuration.nix fuer einen MYL-SERVER.
#
# ⚑ **Zweck** (Auftrag des Projektinhabers): Jeder aufgesetzte Server hat
# dieselbe Grundlage. Wer diese Datei als Vorbild nimmt, bekommt einen
# gehaerteten Knoten mit genau einem offenen Port und Schluesseln, die nicht
# im Nix-Store liegen. Anpassen muss man nur, was oben markiert ist.
#
# ⚠️ **Ungeprueft auf NixOS** (siehe modul.nix). Vor dem Ausrollen mit
# `nixos-rebuild build` bzw. `build-vm` pruefen.
#
# Einbinden (mit Flakes): das Modul als `nixosModules.myl-server` aus dem
# Repositorium importieren. Ohne Flakes: den Pfad zu `modul.nix` in die
# `imports` unten setzen.

{ config, pkgs, lib, ... }:

{
  imports = [
    # Ohne Flakes: Pfad zu diesem Repositorium anpassen.
    # /pfad/zum/repo/NODE/myl-server/modul.nix
  ];

  # ─────────────────────────────────────────────────────────────────────
  # 1. Das ANPASSEN, was zu diesem Server gehoert.
  # ─────────────────────────────────────────────────────────────────────

  networking.hostName = "myl-1"; # ← der Name dieses Servers

  services.myl-server = {
    enable = true;

    # ← Das Paket mit `bin/myl-node`. Mit Flakes kommt es direkt aus dem
    #   Repositorium: `myelith.packages.${pkgs.system}.myl-node`, wobei
    #   `myelith` die Eingabe ist, die auf dieses Repositorium zeigt.
    #   Ohne Flakes zeigt dies auf ein selbst gebautes Paket.
    paket = pkgs.myl-node or (throw "services.myl-server.paket setzen: das Paket mit bin/myl-node");

    # ← Der eine Port nach aussen. Genau diesen im Router weiterleiten
    #   (Router-Anleitung.md). Standard 4001.
    p2pPort = 4001;

    # ← Wenn dieser Server von aussen erreichbar ist, seine oeffentliche
    #   Adresse. Bei dynamischer IP ueber DynDNS, siehe Anleitung.
    oeffentlicheAdressen = [
      # "/dns4/mein-server.example/tcp/4001"
    ];

    # ← Ein Bootstrap-Knoten fuer den ersten Einstieg ins Netz.
    bootstrap = [
      # "/dns4/bootstrap.example/tcp/4001/p2p/12D3Koo..."
    ];

    # ← Der Identitaetsschluessel. ⛔️ Diese Datei gehoert Root, Rechte 0400,
    #   und liegt NICHT in dieser configuration.nix und NICHT im Store.
    #   Einmalig offline erzeugen (Router-Anleitung.md, Abschnitt „Schluessel").
    identitaetCredential = "/etc/myl-server/identitaet.key";

    # ← Der Konformitaetsnachweis. Fuer einen echten Betrieb setzen;
    #   `null` laeuft mit --ohne-konformitaet und ist nur fuer einen Probelauf.
    konformitaetspfad = null;

    # ── Nur fuer einen STIMMBERECHTIGTEN Knoten (Governance) ──────────────
    #   ⛔️ Hoechste Sorgfalt. Der Konsensschluessel ist
    #   getrennt von der Identitaet, Root gehoerend, 0400. Ein
    #   stimmberechtigter Knoten laeuft am besten auf einer Maschine, die
    #   sonst nichts tut, und seine Tuer bleibt ausnahmslos auf der
    #   Rueckschleife.
    # konsensCredential = "/etc/myl-server/konsens.key";
    # stimmsatzdatei = "/var/lib/myl-server/stimmsatz.json";
    #
    # ⚠️ **Bloecke erzeugt in der ueblichen Aufstellung genau einer**, die
    #   Anlaufstelle, und nicht jeder stimmberechtigte Knoten. Wer es ohne
    #   Absprache einschaltet, laesst zwei Knoten dieselbe Runde eroeffnen.
    # erzeuger = true;

    # ── Betrieb ───────────────────────────────────────────────────────────
    # ← Der Beobachtungsendpunkt, **immer auf der Rueckschleife**. Wer ihn
    #   sehen will, tunnelt per SSH:
    #       ssh -N -L 4151:127.0.0.1:4151 benutzer@mein-server
    #   `null` schaltet ihn ab. Die Adresse ist nicht einstellbar: Was
    #   dort heraussieht, ist Aufklaerung fuer einen Fremden.
    beobachtungPort = 4151;

    # ← Wie oft eine Zustandsaufnahme ins Protokoll geht, in Sekunden.
    #   Das ist die Rate, mit der `/var/lib/myl-server/protokolle` waechst;
    #   fuer einen Server, der Monate laeuft, ist sie die wichtigere Zahl.
    aufnahmeSekunden = 300;
  };

  # ─────────────────────────────────────────────────────────────────────
  # 2. Grundlage, die auf JEDEM MYL-SERVER gleich ist. Nicht anfassen,
  #    ausser man weiss, warum.
  # ─────────────────────────────────────────────────────────────────────

  # ⚑ Die Firewall ist an, und das Modul oeffnet NUR den P2P-Port.
  networking.firewall.enable = true;

  # ⛔️ KEIN UPnP: Es oeffnet Ports ohne Nachfrage und ist ein bekanntes
  # Einfallstor. Die eine Weiterleitung wird von Hand im Router gesetzt.
  # (NixOS aktiviert UPnP nicht von selbst; dieser Kommentar haelt fest,
  #  dass das Absicht ist.)

  # SSH ist der einzige weitere Zugang, und nur mit Schluessel: Wer den
  # Beobachtungsendpunkt sehen will, tunnelt ihn ueber SSH statt ihn
  # hinauszubinden.
  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      PermitRootLogin = "no";
    };
  };

  # Automatische Sicherheitsaktualisierungen des Systems; die MYL-Version
  # selbst wird ueber die gepinnte Flake-Eingabe aktualisiert.
  system.autoUpgrade = {
    enable = true;
    allowReboot = false;
  };

  # Ein Konto fuer die Fernwartung, nur mit oeffentlichem Schluessel.
  users.users.betreiber = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    openssh.authorizedKeys.keys = [
      # "ssh-ed25519 AAAA... betreiber@laptop"  ← eigenen Schluessel eintragen
    ];
  };

  # Der Server hat keine grafische Oberflaeche noetig.
  # (Bewusst schlank: weniger installiert heisst weniger Angriffsflaeche.)

  system.stateVersion = "24.11"; # ← auf die eigene NixOS-Fassung setzen
}
