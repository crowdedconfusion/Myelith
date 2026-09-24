#!/bin/sh
# Baut Myelith.app aus der fertigen Binaerfassung.
#
# ⚑ **Warum von Hand und nicht mit `cargo tauri build`.** Das Werkzeug
# ist das richtige fuer eine Veroeffentlichung: Es baut zusaetzlich
# `.dmg`, `.deb`, `.appimage` und `.msi`, und es kennt die Signierung.
# Es ist aber ein eigener Download von einigen hundert Paketen, und ein
# `.app` zum Doppelklicken braucht ihn nicht: Es ist ein Verzeichnis mit
# drei Dingen darin, und macOS bringt alles mit, um es zu bauen.
#
# 📌 **Was dieses Buendel NICHT ist:** signiert und notariell beglaubigt.
# Beim ersten Start meldet Gatekeeper deshalb einen unbekannten
# Entwickler; der Weg darum herum ist ein Rechtsklick und „Oeffnen",
# einmal. Fuer eine Veroeffentlichung reicht das nicht, dafuer braucht
# es ein Entwicklerzertifikat.
set -eu
cd "$(dirname "$0")/../.."
BINAER=SYSTEM/full-build/release/myl-oberflaeche
ZIEL=${1:-SYSTEM/full-build/Myelith.app}

# 📌 **Die Fassung wird gelesen, nicht hingeschrieben.** Bis zum
# 2026-09-10 stand hier `0.4.0` als fester Text, waehrend die Kiste bei
# 0.16.0 stand: Jedes doppelgeklickte Buendel meldete zwoelf Anhebungen
# zu wenig, und der Freigabelauf gab sie so weiter. Die Pruefung
# `die_buendelversion_ist_die_kistenversion` band nur `tauri.conf.json`
# an die Kiste, dieses Skript band niemand.
#
# ⚑ Gegenprobe dazu ist `das_buendelskript_liest_die_fassung`: Sie
# faellt, sobald hier wieder eine Zahl steht statt der Ableitung.
FASSUNG=$(grep -m1 '^version' CLIENT/myl-oberflaeche/Cargo.toml | cut -d'"' -f2)
[ -n "$FASSUNG" ] || { echo "Keine Fassung in CLIENT/myl-oberflaeche/Cargo.toml."; exit 1; }

# 📌 **Erst bauen, dann buendeln, und zwar hier drin.**
#
# Bis zum 2026-09-09 setzte dieses Skript ein gebautes Programm voraus
# und kopierte, was gerade dalag. Damit gab es zwei Staende: das
# Programm unter `SYSTEM/full-build/release/` und den im Buendel, und nur
# einer davon wurde beim Uebersetzen erneuert.
#
# Gemessen an diesem Tag: Buendel von 17:37, Programm von 17:44,
# verschiedene Pruefsummen. Drei Fehlerberichte des Projektinhabers in
# Folge betrafen Dinge, die laengst behoben waren; er startete per
# Doppelklick, also das Buendel, und sah den alten Stand. Nichts daran
# sieht nach einem Bauproblem aus.
#
# ⚑ Ein Bauschritt an dieser Stelle kostet ein paar Sekunden, wenn
# nichts zu tun ist, und nimmt dafuer die Frage weg, welcher von zwei
# Staenden gerade laeuft.
cargo build --release --quiet --manifest-path CLIENT/myl-oberflaeche/Cargo.toml
[ -f "$BINAER" ] || { echo "Der Bau hat kein $BINAER hinterlassen."; exit 1; }

rm -rf "$ZIEL"
mkdir -p "$ZIEL/Contents/MacOS" "$ZIEL/Contents/Resources"
cp "$BINAER" "$ZIEL/Contents/MacOS/Myelith"

# 📌 **Hier wurde das Symbol bis zum 2026-09-09 ein zweites Mal
# erzeugt**, mit `sips` und `iconutil`, aus demselben PNG wie der
# Buendler von Tauri. Zwei Ableitungen desselben Bildes an zwei Stellen
# heisst: Wer die Groessen an einer aendert, hat sie an der anderen
# nicht geaendert, und das faellt niemandem auf, weil beide Wege ein
# Symbol liefern. Es gibt jetzt genau eine Ableitung, und ihr Ergebnis
# liegt fertig im Symbolverzeichnis; hier wird nur kopiert.
cp CLIENT/myl-oberflaeche/icons/icon.icns "$ZIEL/Contents/Resources/Myelith.icns"

# ⚑ **Unquotiertes Dokument**, damit ${FASSUNG} eingesetzt wird. Wer
# hier ein literales Dollarzeichen braucht, schreibt es als `\$`.
cat > "$ZIEL/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Myelith</string>
  <key>CFBundleDisplayName</key><string>Myelith</string>
  <key>CFBundleIdentifier</key><string>org.myelith.oberflaeche</string>
  <key>CFBundleVersion</key><string>${FASSUNG}</string>
  <key>CFBundleShortVersionString</key><string>${FASSUNG}</string>
  <key>CFBundleExecutable</key><string>Myelith</string>
  <key>CFBundleIconFile</key><string>Myelith</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <!-- ⚑ Ohne diesen Schluessel liegt das Fenster hinter allem anderen
       und erscheint nicht im Dock: macOS haelt das Programm dann fuer
       einen Hintergrunddienst. -->
  <key>LSUIElement</key><false/>
  <key>NSHighResolutionCapable</key><true/>
  <!-- ⛔️ **Ohne diesen Schluessel gibt es kein Mikrofon**, und zwar
       ohne Fehlermeldung: macOS fragt gar nicht erst, wenn ein Programm
       nicht sagt, wofuer es das Geraet will. Die Sprechtaste laesst
       ffmpeg aufnehmen, und ein Unterprozess erbt die Erlaubnis dieses
       Buendels; fehlt der Satz, kommt eine **leere Aufnahme** zurueck.
       Gemeldet vom Projektinhaber am 2026-09-18. -->
  <key>NSMicrophoneUsageDescription</key>
  <string>Myelith nimmt auf, solange die Sprechtaste gedrückt ist, und schreibt das Gesprochene mit einem Modell auf diesem Rechner mit.</string>
  <!-- ⛔️ **Ohne diesen Schluessel gibt es keine Kamera**, aus demselben
       Grund wie beim Mikrofon: macOS fragt gar nicht erst, wenn ein
       Programm nicht sagt, wofuer es das Geraet will.
       ⚠️ **Die Bildschirmaufnahme steht hier NICHT**, und das ist keine
       Luecke: Fuer sie gibt es keinen Info.plist-Schluessel. Sie wird
       einmal in den Systemeinstellungen unter Datenschutz freigegeben,
       und macOS merkt sie sich an der Kennung samt Signatur (siehe der
       Absatz zum Ad-hoc-Signieren weiter unten). -->
  <key>NSCameraUsageDescription</key>
  <string>Myelith nimmt ein einzelnes Kamerabild auf, wenn du danach fragst, und lässt es von einem Modell auf diesem Rechner ansehen.</string>
</dict>
</plist>
PLIST

# ⚑ **Ad-hoc signieren**, und das ist keine Kosmetik.
#
# macOS merkt sich eine einmal erteilte Erlaubnis (Mikrofon, Ordner) an
# der **Kennung samt Signatur**. Ein unsigniertes Buendel bekommt bei
# jedem Neubau eine andere Identitaet: Die Frage nach dem Mikrofon kommt
# wieder, oder schlimmer, sie kommt nicht und die Aufnahme bleibt leer.
# ⚠️ **Eine Ad-hoc-Signatur ersetzt kein Entwicklerzertifikat**; sie gibt
# dem Buendel nur eine Identitaet, die ueber Neubauten hinweg dieselbe
# bleibt.
if command -v codesign >/dev/null 2>&1; then
  # ⚠️ **Erweiterte Attribute zuerst weg.** `codesign` lehnt ein Buendel
  # mit „resource fork, Finder information, or similar detritus not
  # allowed" ab. Sie haengen sich beim Kopieren an, und macOS haengt
  # `com.apple.macl` nach, sobald das Buendel einmal eine Erlaubnis
  # bekommen hat; deshalb **zweimal**, mit einem zweiten Versuch.
  #
  # ⚑ **Und der Fehler wird gezeigt, nicht verschluckt.** Ein
  # `>/dev/null 2>&1` an dieser Stelle hat mich am 2026-09-18 zweimal
  # raten lassen, was schiefging.
  signieren() {
    command -v xattr >/dev/null 2>&1 && xattr -cr "$ZIEL" 2>/dev/null
    # ⚑ Mit der Kennung aus dem Info.plist: Ohne `--identifier` nimmt
    # `codesign` den Namen des Programms, und macOS haengt die Erlaubnis
    # dann an eine Kennung, die nicht die des Buendels ist.
    codesign --force --sign - --identifier org.myelith.oberflaeche "$ZIEL" 2>&1
  }
  if MELDUNG=$(signieren) || MELDUNG=$(signieren); then
    echo "ad-hoc signiert."
  else
    echo "⚠️ Signieren ging nicht, Erlaubnisse koennen nach jedem Neubau erneut gefragt werden:"
    echo "   $MELDUNG"
  fi
fi

echo "$ZIEL gebaut, Fassung $FASSUNG."
echo "Starten: open $ZIEL"
echo "⚠️ Beim ersten Mal meldet Gatekeeper einen unbekannten Entwickler:"
echo "   Rechtsklick auf das Buendel, dann Oeffnen, einmal bestaetigen."
