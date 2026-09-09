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
# ⛑ **Was dieses Buendel NICHT ist:** signiert und notariell beglaubigt.
# Beim ersten Start meldet Gatekeeper deshalb einen unbekannten
# Entwickler; der Weg darum herum ist ein Rechtsklick und „Oeffnen",
# einmal. Fuer eine Veroeffentlichung reicht das nicht, dafuer braucht
# es ein Entwicklerzertifikat.
set -eu
cd "$(dirname "$0")/../.."
BINAER=target-shared/release/myl-oberflaeche
ZIEL=${1:-target-shared/Myelith.app}

[ -f "$BINAER" ] || { echo "Erst bauen: (cd CLIENT/myl-oberflaeche && cargo build --release)"; exit 1; }

rm -rf "$ZIEL"
mkdir -p "$ZIEL/Contents/MacOS" "$ZIEL/Contents/Resources"
cp "$BINAER" "$ZIEL/Contents/MacOS/Myelith"

# Das Symbol: aus dem 512er PNG die Groessen erzeugen, die macOS will.
SATZ=$(mktemp -d)/icon.iconset
mkdir -p "$SATZ"
for n in 16 32 128 256 512; do
  sips -z $n $n CLIENT/myl-oberflaeche/icons/icon.png --out "$SATZ/icon_${n}x${n}.png" >/dev/null
  z=$((n * 2))
  sips -z $z $z CLIENT/myl-oberflaeche/icons/icon.png --out "$SATZ/icon_${n}x${n}@2x.png" >/dev/null
done
iconutil -c icns "$SATZ" -o "$ZIEL/Contents/Resources/Myelith.icns"

cat > "$ZIEL/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Myelith</string>
  <key>CFBundleDisplayName</key><string>Myelith</string>
  <key>CFBundleIdentifier</key><string>org.myelith.oberflaeche</string>
  <key>CFBundleVersion</key><string>0.4.0</string>
  <key>CFBundleShortVersionString</key><string>0.4.0</string>
  <key>CFBundleExecutable</key><string>Myelith</string>
  <key>CFBundleIconFile</key><string>Myelith</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <!-- ⚑ Ohne diesen Schluessel liegt das Fenster hinter allem anderen
       und erscheint nicht im Dock: macOS haelt das Programm dann fuer
       einen Hintergrunddienst. -->
  <key>LSUIElement</key><false/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

echo "$ZIEL gebaut."
echo "Starten: open $ZIEL"
echo "⛑ Beim ersten Mal meldet Gatekeeper einen unbekannten Entwickler:"
echo "   Rechtsklick auf das Buendel, dann Oeffnen, einmal bestaetigen."
