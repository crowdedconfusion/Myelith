# Bauumgebung für Myelith, insbesondere für NixOS.
#
# ⚑ **Warum es diese Datei gibt.** Auf den meisten Systemen findet
# `pkg-config` die WebKit-Bibliotheken, weil sie an einem bekannten Ort
# liegen. NixOS legt sie nicht dorthin, und dann bricht der Bau der
# Oberfläche in `wry` ab, mit einer Meldung über eine fehlende
# `.pc`-Datei, die nicht nach dem eigentlichen Grund aussieht.
#
#   nix develop                  eine Shell mit allem, was der Bau braucht
#   sh INSTALL/installieren-nixos.sh     baut darin und legt die Programme ab
#
# ⚑ **Eine Entwicklungsumgebung und keine Ableitung.** Eine
# `buildRustPackage`-Ableitung bräuchte einen festgeschriebenen
# `cargoHash` je Kiste, und dieses Repositorium hat fünfundzwanzig
# eigene Sperrdateien statt einer. Sie wären fünfundzwanzig Zahlen, die
# von Hand mitgepflegt werden müssten, also Fund 271 in Nix. **Was hier
# steht, ist die Umgebung; gebaut wird mit cargo, wie überall sonst
# auch.**
#
# ⛑ **Ungeprüft auf NixOS.** Geschrieben am 2026-09-08 auf macOS und am
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
        # ⚑ `git` gehört dazu, seit `INSTALL/installieren-nixos.sh` und der
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
      in
      {
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
          # ⛑ **`PKG_CONFIG_PATH` und `LD_LIBRARY_PATH` dazu.** Auf
          # NixOS liegt keine Bibliothek an einem Ort, den ein Linker
          # rät; `buildInputs` setzt den Suchpfad für den Bau, aber ein
          # `cargo build`, das mitten in der Shell aufgerufen wird,
          # sieht ihn nicht immer.
          shellHook = pkgs.lib.optionalString istLinux ''
            export WEBKIT_DISABLE_COMPOSITING_MODE=1
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS"
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" oberflaeche}:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath oberflaeche}:$LD_LIBRARY_PATH"
            echo "[myelith] Bauumgebung bereit. Weiter mit: sh INSTALL/installieren-nixos.sh --in-der-shell"
          '';
        };
      });
}
