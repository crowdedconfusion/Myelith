# Bauumgebung für Myelith, insbesondere für NixOS.
#
# ⚑ **Warum es diese Datei gibt.** Auf den meisten Systemen findet
# `pkg-config` die WebKit-Bibliotheken, weil sie an einem bekannten Ort
# liegen. NixOS legt sie nicht dorthin, und dann bricht der Bau der
# Oberfläche in `wry` ab, mit einer Meldung über eine fehlende
# `.pc`-Datei, die nicht nach dem eigentlichen Grund aussieht.
#
#   nix develop                  eine Shell mit allem, was der Bau braucht
#   sh SYSTEM/install/installieren-nixos.sh     baut darin und legt die Programme ab
#
# ⛔️ **Diese Datei bleibt in der Wurzel, auch wenn alles andere
# Systemnahe nach `SYSTEM/` gezogen ist.** `nix develop` sucht sie dort
# und nirgends sonst; ein Flake unter `SYSTEM/` waere ein Flake, das
# niemand findet. 📌 Beim Umzug am 2026-09-24 lag sie kurz falsch, und
# aufgefallen ist es nicht beim Ausprobieren, sondern an einer Probe,
# die genau diesen Satz als Begruendung trug.
#
# ⚑ **Eine Entwicklungsumgebung, und seit dem 2026-09-16 dazu genau
# eine Ableitung.** Eine `buildRustPackage`-Ableitung *mit `cargoHash`*
# bräuchte eine festgeschriebene Zahl je Kiste, und dieses Repositorium
# hat fünfundzwanzig eigene Sperrdateien statt einer: fünfundzwanzig
# Zahlen, die von Hand mitgepflegt werden müssten. **Deshalb gibt es
# weiterhin keine Ableitung je Kiste**, und gebaut wird mit cargo, wie
# überall sonst auch.
#
# ⚑ **Die eine Ausnahme ist `myl-node`, und sie kostet keine Zahl.**
# `cargoLock.lockFile` liest die vorhandene Sperrdatei, statt ihren Hash
# zu verlangen. Es gibt sie, weil ein Server ohne sie nicht aufzusetzen
# ist: Das Modul `services.myl-server` braucht ein Paket, und ohne diese
# Ausgabe wäre es ein Umschlag um „bau es dir selbst".
#
# 📌 **Ungeprüft auf NixOS.** Geschrieben am 2026-09-08 auf macOS und am
# 2026-09-10 erweitert, ebenfalls auf macOS; die Paketnamen und
# Variablen stammen aus der Dokumentation von Tauri und nixpkgs, nicht
# aus einem Lauf. Wer sie zuerst benutzt, prüft sie und berichtigt sie
# hier.
{
  description = "Myelith: dezentrales Netzwerk mit bit-exakter Ganzzahl-Inferenz";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        istLinux = pkgs.stdenv.isLinux;

        # Was jede Kiste braucht, auch ohne Oberfläche.
        #
        # ⚑ `git` gehört dazu, seit `SYSTEM/install/installieren-nixos.sh` und der
        # Aktualisierungsknopf des Klienten darin laufen: Beide bewegen
        # den Klon vorwärts, bevor sie bauen.
        grundwerkzeug = with pkgs; [
          rustc cargo rustfmt clippy
          pkg-config
          git
          python312          # die Proben und die Audits
        ];

        # ⚑ Nur für die Oberfläche, und nur unter Linux: Unter macOS
        # kommt WebKit aus dem System, unter Windows ist es WebView2.
        oberflaeche = with pkgs; lib.optionals istLinux [
          webkitgtk_4_1
          gtk3
          libsoup_3          # webkitgtk 4.1 hängt daran
          librsvg            # für die Symbole
          libayatana-appindicator
          openssl
          glib-networking    # sonst kann die Ansicht kein TLS
        ];
        # ⛔️ **Was NICHT in den Store wandert, und warum das hier steht.**
        #
        # `src = ./.` nähme das ganze Verzeichnis mit, und darin liegen
        # das gemeinsame Bauverzeichnis, die Modellgewichte und die
        # gebauten Artefakte. Gemessen am 2026-09-16: allein
        # `SYSTEM/full-build` sind 57,5 GiB in 327 115 Dateien. **Ein
        # `nix build`, das das kopiert, füllt die Platte, bevor die erste
        # Zeile übersetzt ist**, und die Ableitung wäre obendrein bei
        # jedem Bau eine andere, weil sich dort ständig etwas ändert.
        #
        # ⚑ Gefiltert wird nach Wurzelordnern und Präfixen, nicht nach
        # Dateiendungen: Was ausgeschlossen gehört, ist ein Ort und
        # keine Sorte Datei.
        quelle = pkgs.lib.cleanSourceWith {
          src = ./.;
          name = "myelith-quelle";
          filter = pfad: _typ:
            let
              rel = pkgs.lib.removePrefix (toString ./. + "/") (toString pfad);
              erster = pkgs.lib.head (pkgs.lib.splitString "/" rel);
            in
            !(builtins.elem erster [ "SYSTEM/full-build" "WORK_DIR" "logs" "GENESIS" "MODELS" ])
            && !(pkgs.lib.hasPrefix "INTEGER_LLM/artifacts" rel);
        };

        # ⚑ **Die Fassung wird gelesen, nicht hingeschrieben.** Eine Zahl
        # hier wäre die zweite Stelle, an der die Version von `myl-node`
        # steht, und die zweite Stelle veraltet.
        myl-node = pkgs.rustPlatform.buildRustPackage {
          pname = "myl-node";
          version =
            (builtins.fromTOML (builtins.readFile ./NODE/myl-node/Cargo.toml)).package.version;

          # ⚑ **Das ganze Repositorium als Quelle, und das muss so sein:**
          # `myl-node` zeigt über elf Pfadabhängigkeiten auf
          # Nachbarkisten (`../../SHARED_TYPES/myl-types` und weitere).
          # Nur sein eigener Ordner ergäbe einen Bau ohne die Hälfte
          # seiner Kisten.
          src = quelle;
          buildAndTestSubdir = "NODE/myl-node";

          # ⚑ **`cargoLock.lockFile` statt `cargoHash`.** Nix liest die
          # Sperrdatei, die ohnehin gepflegt wird und deren Aktualität
          # eine eigene Probe hält; ein `cargoHash` wäre eine zweite
          # Zahl daneben. Möglich ist das hier, weil die Sperrdatei
          # **keine** Git-Quelle enthält: Jede Git-Quelle bräuchte einen
          # eigenen `outputHashes`-Eintrag, und dann wäre die Zahl
          # wieder da.
          cargoLock.lockFile = ./NODE/myl-node/Cargo.lock;

          # ⚠️ **Die Prüfsammlung läuft hier nicht.** Sie gehört in die
          # Bauprüfung des Projekts, nicht in eine Paketableitung: Teile
          # von ihr brauchen Netz oder Modellartefakte, und beides gibt
          # es in einer Nix-Sandbox mit Absicht nicht. **Ein Paketbau,
          # der stillschweigend die halbe Prüfsammlung überspringt, wäre
          # ein bestandener Lauf ohne Aussage.**
          doCheck = false;

          meta = {
            description = "Myelith-Knoten: P2P, Konsens, Verifikation";
            mainProgram = "myl-node";
          };
        };
      in
      {
        packages.myl-node = myl-node;
        packages.default = myl-node;

        devShells.default = pkgs.mkShell {
          buildInputs = grundwerkzeug ++ oberflaeche;

          # ⚑ **Zwei Variablen, ohne die das Fenster weiss bleibt.**
          #
          # `WEBKIT_DISABLE_COMPOSITING_MODE` schaltet die
          # GPU-Zusammensetzung ab. Auf NixOS und unter Wayland führt
          # sie regelmässig zu einem Fenster, das aufgeht und leer
          # bleibt: kein Absturz, keine Meldung, nur Weiss. Das ist die
          # unangenehmste Sorte Fehler, weil sie nach einem Fehler im
          # eigenen Quelltext aussieht.
          #
          # `XDG_DATA_DIRS` braucht die GSettings-Schemata, sonst
          # meldet GTK beim Start einen fehlenden Schema-Ordner und
          # bricht ab.
          #
          # 📌 **`PKG_CONFIG_PATH` und `LD_LIBRARY_PATH` dazu.** Auf
          # NixOS liegt keine Bibliothek an einem Ort, den ein Linker
          # rät; `buildInputs` setzt den Suchpfad für den Bau, aber ein
          # `cargo build`, das mitten in der Shell aufgerufen wird,
          # sieht ihn nicht immer.
          shellHook = pkgs.lib.optionalString istLinux ''
            export WEBKIT_DISABLE_COMPOSITING_MODE=1
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS"
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" oberflaeche}:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath oberflaeche}:$LD_LIBRARY_PATH"
            echo "[myelith] Bauumgebung bereit. Weiter mit: sh SYSTEM/install/installieren-nixos.sh --in-der-shell"
          '';
        };
      }) // {
    # ⚑ **Das Modul steht ausserhalb von `eachDefaultSystem`**, denn es
    # ist von keinem System abhängig: Es beschreibt einen Dienst, und
    # welches Paket ihn stellt, sagt der Betreiber in seiner eigenen
    # Konfiguration. Ein Modul je Architektur wäre dieselbe Datei
    # mehrfach.
    #
    # Einbinden mit:
    #   imports = [ myelith.nixosModules.myl-server ];
    #   services.myl-server.paket = myelith.packages.${pkgs.system}.myl-node;
    nixosModules.myl-server = import ./NODE/myl-server/modul.nix;
    nixosModules.default = self.nixosModules.myl-server;
  };
}
