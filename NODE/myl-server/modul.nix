# NixOS-Modul fuer einen MYL-SERVER: ein gehaerteter systemd-Dienst um
# `myl-node`, mit den Sicherheitsvorgaben des Projekts fest eingebaut.
#
# ⚑ **Warum genau dieser Zuschnitt gilt**, steht als Kommentar an jeder
# Direktive unten. Wer hier etwas aendert, liest die Begruendung daneben.
#
# ⚠️ **Ungeprueft auf NixOS.** Geschrieben am 2026-09-14 auf macOS, ohne
# `nix` auf der Maschine. Die Optionsnamen und systemd-Direktiven stammen
# aus der nixpkgs- und systemd-Dokumentation, nicht aus einem Lauf. Wer es
# zuerst benutzt, prueft es mit `nixos-rebuild build-vm` oder einem
# `nixosTest` und berichtigt es hier. Dieselbe Ehrlichkeit wie im
# Wurzel-`flake.nix`.
#
# ⛔️ **Was dieses Modul mit Absicht NICHT tut** :
#   - Es oeffnet KEINE Tuer, KEINEN Beobachtungs- und KEINEN
#     Ortsleitungsport nach aussen. Nach aussen erreichbar ist nur der
#     P2P-Port.
#   - Es legt KEIN Schluesselmaterial in den Nix-Store. Schluessel kommen
#     ueber systemd-`LoadCredential` aus einer Datei, die Root gehoert.
#   - Es erzeugt KEINE Schluessel automatisch. Der Betreiber bringt sie
#     mit (siehe Router-Anleitung.md).
#   - Es schaltet KEIN UPnP.

{ config, lib, pkgs, ... }:

let
  cfg = config.services.myl-server;
in
{
  options.services.myl-server = {
    enable = lib.mkEnableOption "den MYL-SERVER (myl-node als gehaerteter Dienst)";

    paket = lib.mkOption {
      type = lib.types.package;
      description = ''
        Das Paket, das `bin/myl-node` bereitstellt. Baubar aus dem
        Repositorium ueber die Flake-Ausgabe; solange die
        noch nicht steht, zeigt der Betreiber hier auf ein selbst gebautes
        Paket.
      '';
    };

    name = lib.mkOption {
      type = lib.types.str;
      default = config.networking.hostName;
      description = "Der Name des Knotens im Protokoll (--name).";
    };

    rolle = lib.mkOption {
      type = lib.types.enum [ "teilnehmer" "relais" ];
      default = "teilnehmer";
      description = ''
        Die Rolle (--rolle). `teilnehmer` hoert zu und rechnet nach;
        `relais` hilft anderen durch NAT hindurch und braucht eine
        oeffentliche Adresse.
      '';
    };

    p2pPort = lib.mkOption {
      type = lib.types.port;
      default = 4001;
      description = ''
        Der einzige Port, der nach aussen offen ist (--port). Genau diesen
        leitet der Router weiter (siehe Router-Anleitung.md). Ueber 1024,
        damit der Dienst keine Capability braucht.
      '';
    };

    firewallOeffnen = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Oeffnet den P2P-Port in der NixOS-Firewall (TCP und UDP fuer QUIC).
        ⚑ Nur diesen. Tuer, Beobachtung und Ortsleitung bleiben zu.
      '';
    };

    bootstrap = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/dns4/bootstrap.example/tcp/4001/p2p/12D3Koo..." ];
      description = "Bootstrap-Knoten fuer den Einstieg (--bootstrap, mehrfach).";
    };

    oeffentlicheAdressen = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "/dns4/mein-server.example/tcp/4001" ];
      description = "Eigene, von aussen erreichbare Adressen (--oeffentlich).";
    };

    konformitaetspfad = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Pfad zum Konformitaetsnachweis (--konformitaet). Ist er `null`,
        laeuft der Dienst mit `--ohne-konformitaet`; ⚠️ das ist nur fuer
        einen Probelauf gedacht.
      '';
    };

    identitaetCredential = lib.mkOption {
      type = lib.types.path;
      example = "/etc/myl-server/identitaet.key";
      description = ''
        Datei mit dem Identitaetsschluessel des Knotens, **Root gehoerend,
        Rechte 0400**. Wird ueber systemd-`LoadCredential` gereicht und
        landet NICHT im Nix-Store. Der Knoten behaelt damit seine PeerId
        ueber Neustarts.
      '';
    };

    konsensCredential = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Nur fuer einen stimmberechtigten Knoten (Governance): Datei mit dem
        geheimen BLS-Konsensschluessel, getrennt von der Identitaet, ⛔️ das
        Kronjuwel. Root gehoerend, Rechte 0400, nie im Store. `null` heisst:
        der Knoten stimmt nicht mit (der Normalfall).
      '';
    };

    stimmsatzdatei = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Nur mit Konsensschluessel: der Validator-Satz (--stimmsatz).";
    };

    zusatzflags = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        Weitere Flags an `myl-node`, fuer Faelle, die dieses Modul nicht
        abbildet. ⚠️ Wer hier `--tuer 0.0.0.0:...` schreibt, verlaesst den
        Zuschnitt; das ist moeglich, aber nicht die Vorgabe.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = (cfg.stimmsatzdatei != null) -> (cfg.konsensCredential != null);
        message = "services.myl-server: ein Stimmsatz ohne Konsensschluessel stimmt nicht mit; setze konsensCredential.";
      }
    ];

    # ⚑ Ein eigener, unprivilegierter Nutzer (kein DynamicUser, weil das
    # StateDirectory ueber Neustarts stabil bleiben muss: Schluessel-PeerId,
    # Kette, Protokolle).
    users.users.myl-server = {
      isSystemUser = true;
      group = "myl-server";
      description = "MYL-SERVER Dienstkonto";
    };
    users.groups.myl-server = { };

    networking.firewall = lib.mkIf cfg.firewallOeffnen {
      allowedTCPPorts = [ cfg.p2pPort ];
      allowedUDPPorts = [ cfg.p2pPort ];
    };

    systemd.services.myl-server = {
      description = "MYL-SERVER (myl-node)";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      serviceConfig = {
        User = "myl-server";
        Group = "myl-server";

        # Zustand liegt hier, nicht im Store und nicht im Home.
        StateDirectory = "myl-server";
        StateDirectoryMode = "0700";
        WorkingDirectory = "/var/lib/myl-server";

        # ⚑ Schluessel aus dem Store heraus, ueber Credentials. Sie liegen
        # zur Laufzeit unter $CREDENTIALS_DIRECTORY und sind nur dem Dienst
        # sichtbar.
        LoadCredential =
          [ "identitaet:${toString cfg.identitaetCredential}" ]
          ++ lib.optional (cfg.konsensCredential != null) "konsens:${toString cfg.konsensCredential}";

        ExecStart = lib.escapeShellArgs (
          [
            "${cfg.paket}/bin/myl-node"
            "--name" cfg.name
            "--rolle" cfg.rolle
            "--port" (toString cfg.p2pPort)
            # ⚑ Der Identitaetsschluessel kommt aus dem Credential-Verzeichnis.
            "--schluessel" "%d/identitaet"
            # Zustand unter dem StateDirectory.
            "--kette" "/var/lib/myl-server/kette.dat"
            "--protokolle" "/var/lib/myl-server/protokolle"
          ]
          # ⚑ Tuer, Beobachtung und Ortsleitung werden NICHT uebergeben:
          # Der Knoten bindet sie per Vorgabe an die Rueckschleife bzw. laesst
          # sie aus. Genau das ist der Zuschnitt.
          ++ (if cfg.konformitaetspfad != null
              then [ "--konformitaet" (toString cfg.konformitaetspfad) ]
              else [ "--ohne-konformitaet" ])
          ++ lib.concatMap (b: [ "--bootstrap" b ]) cfg.bootstrap
          ++ lib.concatMap (a: [ "--oeffentlich" a ]) cfg.oeffentlicheAdressen
          ++ lib.optionals (cfg.konsensCredential != null) [ "--konsensschluessel" "%d/konsens" ]
          ++ lib.optionals (cfg.stimmsatzdatei != null) [ "--stimmsatz" (toString cfg.stimmsatzdatei) ]
          ++ cfg.zusatzflags
        );

        Restart = "on-failure";
        RestartSec = "10s";

        # --- Haertung: die Betriebssystem-Grenze, die der
        #     Dateizugriff sonst vermisst. `systemd-analyze security` ist
        #     der Beleg. ---
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [ "@system-service" "~@privileged" "~@resources" ];
        # Der Knoten braucht keine Capabilities; der P2P-Port liegt ueber 1024.
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        # Nur der eigene Zustand ist beschreibbar.
        ReadWritePaths = [ "/var/lib/myl-server" ];

        # Ressourcengrenzen, damit ein Ueberlastangriff gegen den P2P-Port
        # den Knoten trifft und nicht die Maschine.
        LimitNOFILE = 65536;
        TasksMax = 512;
      };
    };
  };
}
