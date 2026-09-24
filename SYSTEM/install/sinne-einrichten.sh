#!/bin/sh
# Richtet Sehen, Hoeren und Sprechen ein.
#
# ⚑ **Was das Repositorium mitbringt und was nicht** (Festlegung des
# Projektinhabers, 2026-09-17): Ein frischer Klon soll alles dabeihaben,
# **ausser den Modellgewichten**. Dieses Skript ist genau dieses „alles":
# Es holt die Programme ueber den Paketverwalter des Systems, legt die
# Gewichte an die Stellen, an denen der Client sie sucht, und richtet
# CosyVoice samt eigener Python-Umgebung ein.
#
# ⚑ **Wohin die Gewichte gehen, hat sich am 2026-09-21 geaendert**
# (Festlegung des Projektinhabers): in den Klon unter `MODELS/audio` und
# `MODELS/vision`, zu allen anderen fremden Gewichten dieses Projekts.
# **Versioniert wird dort nichts**, nur die Ordner und ihre Doku; das
# regelt `MODELS/.gitignore`.
#
# ⚠️ **In der Sinnesheimat bleiben zwei Dinge, und zwar mit Grund:** der
# `bin`-Ordner mit dem Sprechlaeufer (erzeugt, muss beschreibbar sein)
# und eine abgelegte Stimmprobe (gehoert dem Nutzer). Gewichte sind
# geladen, und nur die haben einen Rubrikordner verdient.
#
# ⛔️ **Die Binaerdateien selbst gehoeren nicht ins Repositorium.**
# llama.cpp, whisper.cpp und ffmpeg sind je System verschieden und
# zusammen einige hundert Megabyte; torch ist allein rund 2,5 GB. Das
# waere derselbe Ballast, der beim Vorrat der Fremdquellen bewusst
# abgelehnt wurde. **Der Bau des Repositoriums braucht nichts davon**:
# Er laeuft ohne Netz, und die Sinne sind eine Zutat, keine Bedingung.
#
# Aufruf:
#   sh SYSTEM/install/sinne-einrichten.sh              alles
#   sh SYSTEM/install/sinne-einrichten.sh --ohne-sprechen   nur Sehen und Hoeren
#   sh SYSTEM/install/sinne-einrichten.sh --pruefen    nur nachsehen, nichts tun
set -u

HIER=$(cd "$(dirname "$0")/.." && pwd)
HEIMAT="${MYL_SINNE:-$HOME/.myelith/sinne}"
# ⚑ **Setzt jemand MYL_SINNE, liegt dort alles zusammen**, so wie der
# Client es auch sucht. Sonst gehen die Gewichte in die Rubriken.
if [ -n "${MYL_SINNE:-}" ]; then
  AUDIO="$HEIMAT"
  VISION="$HEIMAT"
else
  AUDIO="$HIER/MODELS/audio"
  VISION="$HIER/MODELS/vision"
fi
COSY="${MYL_COSYVOICE:-$AUDIO/CosyVoice}"
LAEUFER_QUELLE="$HIER/CLIENT/myl-senses/laeufer/sprechen-cosyvoice.py"

mit_sprechen=1
nur_pruefen=0
for a in "$@"; do
  case "$a" in
    --ohne-sprechen) mit_sprechen=0 ;;
    --pruefen) nur_pruefen=1 ;;
    *) echo "unbekannter Schalter: $a"; exit 2 ;;
  esac
done

sagen() { printf '\n== %s ==\n' "$1"; }
da() { command -v "$1" >/dev/null 2>&1; }

pruefen() {
  sagen "Stand"
  for w in ffmpeg llama-mtmd-cli whisper-cli; do
    printf '  %-16s %s\n' "$w" "$(command -v "$w" 2>/dev/null || echo 'FEHLT')"
  done
  # ⚑ **Der Blick braucht kein eigenes Programm.** Auf macOS nimmt
  #   screencapture den Bildschirm, sonst ffmpeg; die Kamera holt immer
  #   ffmpeg. Was fehlt, ist deshalb nie ein Download, sondern eine
  #   Erlaubnis, und die gibt nur der Mensch.
  if [ "$(uname -s)" = Darwin ]; then
    printf '  %-16s %s\n' "screencapture" "$(command -v screencapture 2>/dev/null || echo 'FEHLT')"
  fi
  for paar in "$AUDIO:hoeren.bin" "$VISION:sehen.gguf" "$VISION:sehen-mmproj.gguf" \
              "$VISION:sehen-genau.gguf" "$VISION:sehen-genau-mmproj.gguf"; do
    ort=${paar%%:*}; d=${paar#*:}
    if [ -s "$ort/$d" ]; then
      printf '  %-24s %s\n' "$d" "$(du -h "$ort/$d" | cut -f1)"
    else
      printf '  %-24s %s\n' "$d" "fehlt ($ort)"
    fi
  done
  printf '  %-24s %s\n' "CosyVoice" "$([ -d "$COSY/cosyvoice" ] && echo "$COSY" || echo fehlt)"
  printf '  %-24s %s\n' "seine Umgebung" "$([ -x "$COSY/.venv/bin/python" ] && echo da || echo fehlt)"
  printf '  %-24s %s\n' "seine Gewichte" "$([ -s "$COSY/pretrained_models/Fun-CosyVoice3-0.5B/llm.pt" ] && echo da || echo fehlt)"
  printf '  %-24s %s\n' "Laeufer" "$([ -f "$HEIMAT/bin/sprechen-cosyvoice.py" ] && echo da || echo fehlt)"
  sagen "Blick auf Bildschirm und Kamera"
  echo "  Beide sind AUS, solange sie nicht ausdruecklich scharf gestellt sind:"
  echo "    MYL_BLICK_BILDSCHIRM=1   der Agent darf den Bildschirm aufnehmen"
  echo "    MYL_BLICK_KAMERA=1       der Agent darf die Kamera aufnehmen"
  echo "  Jede Aufnahme bleibt unter .AGENT/blicke/ liegen, damit nachsehbar ist,"
  echo "  was gesehen wurde."
  if [ "$(uname -s)" = Darwin ]; then
    echo "  macOS gibt die Geraete zusaetzlich erst nach Freigabe heraus:"
    echo "    Systemeinstellungen, Datenschutz und Sicherheit, Bildschirmaufnahme bzw. Kamera."
    echo "  Ohne sie meldet screencapture 'could not create image from display'."
  fi
  echo "  Nimmt ffmpeg die falsche Quelle: MYL_SCHIRMFORMAT/MYL_SCHIRMGERAET,"
  echo "  MYL_KAMERAFORMAT/MYL_KAMERAGERAET."
  # ⚑ Der Verweis wird nur gebraucht, wenn CosyVoice ausserhalb der
  # Rubrik liegt; sonst findet der Client es dort von selbst.
  case "$COSY" in
    "$AUDIO"/*) printf '  %-24s %s\n' "CosyVoice in der Rubrik" "ja, kein Verweis noetig" ;;
    *) printf '  %-24s %s\n' "Verweis auf CosyVoice" "$([ -e "$HEIMAT/cosyvoice" ] && echo da || echo fehlt)" ;;
  esac
}

if [ "$nur_pruefen" = 1 ]; then pruefen; exit 0; fi

echo "Das holt rund 6 GB Gewichte fuer Sehen und Hoeren"
[ "$mit_sprechen" = 1 ] && echo "und noch einmal rund 8 GB fuer Sprechen (CosyVoice 3 samt torch)."
echo "Sehen nach:    $VISION"
echo "Hoeren nach:   $AUDIO"
echo "Sprechen nach: $COSY"
echo "Betrieb in:    $HEIMAT (bin und Stimmprobe)"

# ---------------------------------------------------------------- Programme
sagen "Programme"
if da brew; then
  brew install ffmpeg llama.cpp whisper-cpp || exit 1
elif da nix-env; then
  echo "NixOS: nix-shell -p ffmpeg llama-cpp whisper-cpp, oder in die Konfiguration aufnehmen."
elif da apt-get; then
  echo "Debian/Ubuntu: ffmpeg aus der Paketverwaltung; llama.cpp und whisper.cpp selbst bauen."
else
  echo "Kein bekannter Paketverwalter. Gebraucht werden: ffmpeg, llama-mtmd-cli, whisper-cli."
fi

# ---------------------------------------------------------------- Gewichte
mkdir -p "$HEIMAT/bin" "$AUDIO" "$VISION" || exit 1
# ⚑ **Der Ordner steht jetzt im Aufruf** und nicht in der Funktion: Es
# gibt zwei Rubriken, und eine Funktion, die den Ort selbst waehlt,
# waehlte fuer eine von beiden falsch.
hol() {
  ziel="$1/$2"; shift
  [ -s "$ziel" ] && { echo "liegt schon: $1"; return 0; }
  echo "hole $1 ..."
  # ⚑ Erst daneben, dann umbenennen: Ein Abbruch laesst sonst eine halbe
  # Datei liegen, die beim naechsten Lauf als fertig gilt.
  if curl -fL --retry 3 --retry-delay 2 --progress-bar -o "$ziel.teil" "$2"; then
    mv "$ziel.teil" "$ziel"
  else
    rm -f "$ziel.teil"; echo "FEHLGESCHLAGEN: $1"; return 1
  fi
}
W=https://huggingface.co/ggerganov/whisper.cpp/resolve/main
S=https://huggingface.co/ggml-org/SmolVLM2-2.2B-Instruct-GGUF/resolve/main
Q=https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main

sagen "Hoeren"
hol "$AUDIO" hoeren.bin "$W/ggml-large-v3-turbo-q5_0.bin" || exit 1

sagen "Sehen, schnell (SmolVLM2-2.2B)"
hol "$VISION" sehen.gguf        "$S/SmolVLM2-2.2B-Instruct-Q4_K_M.gguf" || exit 1
hol "$VISION" sehen-mmproj.gguf "$S/mmproj-SmolVLM2-2.2B-Instruct-Q8_0.gguf" || exit 1

sagen "Sehen, genau (Qwen2.5-VL-3B)"
# ⚑ Wahlfrei: Faellt sie aus, bleibt es bei der schnellen Sprosse.
hol "$VISION" sehen-genau.gguf        "$Q/Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf" || echo "(bleibt bei der schnellen Sprosse)"
hol "$VISION" sehen-genau-mmproj.gguf "$Q/mmproj-Qwen2.5-VL-3B-Instruct-f16.gguf" || echo "(bleibt bei der schnellen Sprosse)"

if [ "$mit_sprechen" = 0 ]; then pruefen; exit 0; fi

# ---------------------------------------------------------------- Sprechen
sagen "CosyVoice"
if [ ! -d "$COSY/cosyvoice" ]; then
  git clone --depth 1 --recursive https://github.com/FunAudioLLM/CosyVoice.git "$COSY" || exit 1
else
  echo "liegt schon: $COSY"
fi

sagen "Seine Python-Umgebung"
# ⛔️ **Python 3.10 aufwaerts.** macOS bringt 3.9 mit, und CosyVoice
# laeuft damit nicht.
python_neu=""
for p in python3.12 python3.11 python3.10 python3; do
  if da "$p" && "$p" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3,10) else 1)'; then
    python_neu=$(command -v "$p"); break
  fi
done
if [ -z "$python_neu" ]; then
  echo "Es fehlt ein Python 3.10 oder neuer."
  da brew && echo "  brew install python@3.11"
  exit 1
fi
[ -x "$COSY/.venv/bin/python" ] || "$python_neu" -m venv "$COSY/.venv" || exit 1
P="$COSY/.venv/bin/python"
echo "benutzt: $("$P" -V)"

# ⛔️ **setuptools 81 hat `pkg_resources` aus der Bauumgebung genommen**,
# und `openai-whisper==20231117` braucht es beim Bauen. Ohne diese Zeile
# bricht die ganze Installation an einem einzigen Paket ab, und die
# Meldung nennt nur `ModuleNotFoundError: pkg_resources`.
"$P" -m pip install --upgrade pip >/dev/null || exit 1
"$P" -m pip install "setuptools<81" wheel >/dev/null || exit 1

# ⚑ Die CUDA-Indizes fliegen raus: Auf macOS gibt es dort nichts, und der
# zweite ist langsam bis unerreichbar.
# ⚑ **Die Hilfsdateien liegen im Zwischenordner und nicht in der
# fremden Arbeitskopie.** Wer ein fremdes Repositorium beschreibt, macht
# dessen `git status` schmutzig, und der naechste sucht den Grund bei
# sich.
ARBEIT=$(mktemp -d) || exit 1
trap 'rm -rf "$ARBEIT"' EXIT INT TERM
grep -v '^--extra-index-url' "$COSY/requirements.txt" > "$ARBEIT/requirements.txt"

# ⛔️ **Die Reihenfolge ist keine Geschmacksfrage.** Wer
# `openai-whisper` zuerst installiert, holt damit unbestimmte Fassungen
# von torch und numpy herein; die gepinnten aus `requirements.txt`
# passen dann nicht mehr dazu, und **pip laeuft rueckwaerts durch
# hunderte Fassungen**, minutenlang bei voller Last, ohne Ende in Sicht.
# Also erst die gepinnten, dann das eine Paket in die fertige Umgebung.
grep -v '^openai-whisper' "$ARBEIT/requirements.txt" > "$ARBEIT/ohne-whisper.txt"
"$P" -m pip install -r "$ARBEIT/ohne-whisper.txt" || exit 1
"$P" -m pip install --no-build-isolation "openai-whisper==20231117" || exit 1

sagen "Seine Gewichte"
# ⛔️ **CosyVoice 3 und nicht 2, und das ist der Unterschied zwischen
# brauchbarem und unbrauchbarem Deutsch.** Die 2.0-Gewichte decken
# Chinesisch und Englisch ab; eine deutsche Stimmprobe ergibt damit
# Laute, die wie Deutsch klingen und keines sind. Gemessen am
# 2026-09-17: whisper las aus dem Ergebnis „dies Go! Ist die Örsteck ist
# proschein" statt „dies ist die erste gesprochene Antwort".
Z="$COSY/pretrained_models/Fun-CosyVoice3-0.5B"
R=https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512/resolve/main
mkdir -p "$Z/CosyVoice-BlankEN" || exit 1
for f in config.json configuration.json cosyvoice3.yaml campplus.onnx \
         speech_tokenizer_v3.onnx flow.decoder.estimator.fp32.onnx \
         llm.pt flow.pt hift.pt \
         CosyVoice-BlankEN/config.json CosyVoice-BlankEN/generation_config.json \
         CosyVoice-BlankEN/merges.txt CosyVoice-BlankEN/vocab.json \
         CosyVoice-BlankEN/tokenizer_config.json CosyVoice-BlankEN/model.safetensors; do
  if [ -s "$Z/$f" ]; then echo "liegt schon: $f"; continue; fi
  echo "hole $f ..."
  curl -fL --retry 3 --retry-delay 2 --progress-bar -o "$Z/$f.teil" "$R/$f" \
    && mv "$Z/$f.teil" "$Z/$f" || { rm -f "$Z/$f.teil"; echo "FEHLGESCHLAGEN: $f"; exit 1; }
done
# ⚑ Wahlfrei: Gibt es eingebaute Stimmen, kann ohne Probe gesprochen
# werden. ⛔️ **Und eine leere Datei ist schlimmer als keine**: CosyVoice
# prueft nur, ob `spk2info.pt` **existiert**, und laedt sie dann; eine
# 0-Byte-Datei, die `curl` bei einem Fehlschlag hinterlaesst, bringt es
# beim Start zum Absturz.
if curl -fsSL --retry 2 -o "$Z/spk2info.pt.teil" "$R/spk2info.pt" && [ -s "$Z/spk2info.pt.teil" ]; then
  mv "$Z/spk2info.pt.teil" "$Z/spk2info.pt"
else
  rm -f "$Z/spk2info.pt.teil"
  echo "(kein spk2info.pt: dieses Modell braucht eine Stimmprobe)"
fi

sagen "Verdrahten"
# ⚑ **Ein Verweis statt einer Umgebungsvariablen.** Ein aus dem Finder
# gestartetes `Myelith.app` erbt keine Shell-Umgebung; ein Ordner an der
# erwarteten Stelle wird dagegen immer gefunden.
#
# ⚑ **Und genau deshalb entfaellt er, wenn CosyVoice in der Rubrik
# liegt:** Dort sucht der Client ohnehin, und ein Verweis aus der Heimat
# in den Klon waere ein zweiter Weg zur selben Sache, der beim naechsten
# Umzug des Klons ins Leere zeigt.
case "$COSY" in
  "$AUDIO"/*) echo "CosyVoice liegt in der Rubrik, kein Verweis noetig." ;;
  *) ln -sfn "$COSY" "$HEIMAT/cosyvoice" ;;
esac
cp "$LAEUFER_QUELLE" "$HEIMAT/bin/" && chmod +x "$HEIMAT/bin/sprechen-cosyvoice.py"
echo "Laeufer: $HEIMAT/bin/sprechen-cosyvoice.py"

pruefen
echo
echo "Fertig. Im Fenster erscheinen Sprechtaste und Lautsprecher von selbst."
