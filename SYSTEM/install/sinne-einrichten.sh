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

# ⚠️ **`../..` und nicht `..`** (Fund 620): Dieses Skript liegt unter
# `SYSTEM/install/`, zwei Ebenen unter der Wurzel. Beim Umzug aus
# `INSTALL/` blieb hier `..` stehen; danach suchte und legte es die Gewichte
# unter `SYSTEM/MODELS` ab, und das Kopieren des Laeufers scheiterte.
HIER=$(cd "$(dirname "$0")/../.." && pwd)
if [ ! -f "$HIER/CLIENT/myl-senses/laeufer/sprechen-cosyvoice.py" ]; then
  echo "Die Wurzel des Repositoriums ist nicht $HIER."
  exit 1
fi
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
summe() {
  if da sha256sum; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# ⚑ Die Umgebung von CosyVoice kommt aus dieser Liste, mit Fassung und
# Pruefsumme je Paket. Ihr Abdruck liegt nach dem Einrichten in der
# Umgebung; weicht er ab, ist die Umgebung von einem anderen Stand.
LISTE="$HIER/SYSTEM/install/sprechen-fest.txt"
STANDMARKE="$COSY/.venv/myelith-stand"
umgebung_stand() {
  if [ ! -x "$COSY/.venv/bin/python" ]; then echo fehlt
  elif [ "$(cat "$STANDMARKE" 2>/dev/null)" = "$(summe "$LISTE")" ]; then echo da
  else echo "veraltet (wird beim Einrichten ersetzt)"
  fi
}

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
  printf '  %-24s %s\n' "seine Umgebung" "$(umgebung_stand)"
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
#
# ⛔️ **Jede Datei hat einen festen Stand und eine Pruefsumme** (Fund 604,
# 2026-10-23). Bis dahin kam alles vom wandernden Zweig `main` und ohne
# Pruefung, darunter die Pickle-Dateien von CosyVoice (`llm.pt`, `flow.pt`,
# `hift.pt`), die torch beim Laden auspackt. Wer das fremde Repositorium
# uebernimmt oder eine Datei darin austauscht, haette damit Code auf jedem
# Rechner ausgefuehrt, der dieses Skript danach laufen laesst. Jetzt steht
# in jeder Adresse ein Commit, und eine Datei, deren Summe nicht stimmt,
# wird nicht an ihren Platz gelegt.
# hol <ordner> <dateiname> <adresse> <sha256>
hol() {
  ziel="$1/$2"
  if [ -s "$ziel" ]; then
    # ⚑ Eine liegende Datei wird geprueft und nicht ersetzt: Sie kann dem
    # Nutzer gehoeren. Weicht sie ab, steht es da, mit dem Weg zurueck.
    if [ "$(summe "$ziel")" = "$4" ]; then
      echo "liegt schon, Summe stimmt: $2"
    else
      echo "⚠️ $ziel weicht vom festen Stand ab. Ist es nicht deine eigene Datei: loeschen und dieses Skript noch einmal starten."
    fi
    return 0
  fi
  echo "hole $2 ..."
  # ⚑ Erst daneben, dann umbenennen: Ein Abbruch laesst sonst eine halbe
  # Datei liegen, die beim naechsten Lauf als fertig gilt.
  if ! curl -fL --retry 3 --retry-delay 2 --progress-bar -o "$ziel.teil" "$3"; then
    rm -f "$ziel.teil"; echo "FEHLGESCHLAGEN: $2"; return 1
  fi
  ist=$(summe "$ziel.teil")
  if [ "$ist" != "$4" ]; then
    rm -f "$ziel.teil"
    echo "FEHLGESCHLAGEN: $2 hat eine andere Pruefsumme (erwartet $4, bekommen $ist)"
    return 1
  fi
  mv "$ziel.teil" "$ziel"
}
W=https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1
S=https://huggingface.co/ggml-org/SmolVLM2-2.2B-Instruct-GGUF/resolve/1bc3c9f74ceafd4c8d4411cc9cf188bba3798f91
# ⛔️ **Die genaue Sprosse ist seit dem 2026-09-25 Qwen3-VL-4B** (Apache 2.0,
# vom Hersteller selbst umgewandelt). Vorher stand hier Qwen2.5-VL-3B, und
# dessen Original steht unter einer Forschungslizenz, die nur Forschung
# und nicht kommerzielle Nutzung erlaubt; die Umwandlung nannte Apache 2.0,
# kann die Lizenz des Originals aber nicht aendern.
Q=https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266

# ⚑ **Eine alte Datei wird ersetzt, eine fremde nicht.** Erkannt wird die
# alte an ihrer Groesse in Bytes; wer eine eigene Datei unter demselben
# Namen abgelegt hat, behaelt sie.
alt_ersetzen() {
  [ -f "$1" ] || return 0
  if [ "$(wc -c < "$1" | tr -d ' ')" = "$2" ]; then
    echo "ersetze $(basename "$1") (Qwen2.5-VL-3B, Forschungslizenz)"
    rm -f "$1"
  fi
}

sagen "Hoeren"
hol "$AUDIO" hoeren.bin "$W/ggml-large-v3-turbo-q5_0.bin" \
  394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2 || exit 1

sagen "Sehen, schnell (SmolVLM2-2.2B)"
hol "$VISION" sehen.gguf "$S/SmolVLM2-2.2B-Instruct-Q4_K_M.gguf" \
  0cf76814555b8665149075b74ab6b5c1d428ea1d3d01c1918c12012e8d7c9f58 || exit 1
hol "$VISION" sehen-mmproj.gguf "$S/mmproj-SmolVLM2-2.2B-Instruct-Q8_0.gguf" \
  ae07ea1facd07dd3230c4483b63e8cda96c6944ad2481f33d531f79e892dd024 || exit 1

sagen "Sehen, genau (Qwen3-VL-4B)"
alt_ersetzen "$VISION/sehen-genau.gguf" 1929901056
alt_ersetzen "$VISION/sehen-genau-mmproj.gguf" 1338428128
# ⚑ Wahlfrei: Faellt sie aus, bleibt es bei der schnellen Sprosse.
hol "$VISION" sehen-genau.gguf "$Q/Qwen3VL-4B-Instruct-Q4_K_M.gguf" \
  66358cb18bb6b3b1b6675aa412c7a88ef01d228f481184d13668e5201c730a0a || echo "(bleibt bei der schnellen Sprosse)"
hol "$VISION" sehen-genau-mmproj.gguf "$Q/mmproj-Qwen3VL-4B-Instruct-Q8_0.gguf" \
  30ba2c7dd3127a4561b6cba9d13d0f711c91bdb38742e2f56d73c8cb596bd06d || echo "(bleibt bei der schnellen Sprosse)"

if [ "$mit_sprechen" = 0 ]; then pruefen; exit 0; fi

# ---------------------------------------------------------------- Sprechen
sagen "CosyVoice"
# ⚑ Ein fester Commit (Fund 604): der Stand, mit dem der Sprechlaeufer
# erprobt ist. Er legt auch das Untermodul Matcha-TTS fest.
COSY_STAND=074ca6dc9e80a2f424f1f74b48bdd7d3fea531cc
if [ ! -d "$COSY/cosyvoice" ]; then
  { git init -q "$COSY" \
    && git -C "$COSY" remote add origin https://github.com/FunAudioLLM/CosyVoice.git \
    && git -C "$COSY" fetch -q --depth 1 origin "$COSY_STAND" \
    && git -C "$COSY" checkout -q FETCH_HEAD \
    && git -C "$COSY" submodule update -q --init --recursive --depth 1; } || exit 1
else
  echo "liegt schon: $COSY"
fi
if [ "$(git -C "$COSY" rev-parse HEAD 2>/dev/null)" != "$COSY_STAND" ]; then
  echo "⚠️ $COSY steht nicht auf $COSY_STAND. Erprobt ist nur dieser Stand."
fi

sagen "Seine Python-Umgebung"
# ⛔️ **Python 3.11 aufwaerts.** macOS bringt 3.9 mit, und CosyVoice laeuft
# damit nicht; die feste Liste ist fuer 3.11 und neuer aufgeloest, erprobt
# mit 3.11.
python_neu=""
for p in python3.11 python3.12 python3.13 python3; do
  if da "$p" && "$p" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3,11) else 1)'; then
    python_neu=$(command -v "$p"); break
  fi
done
if [ -z "$python_neu" ]; then
  echo "Es fehlt ein Python 3.11 oder neuer."
  da brew && echo "  brew install python@3.11"
  exit 1
fi

# ⚑ **Nicht die `requirements.txt` von CosyVoice** (Fund 604): Die bringt
# Server und Training mit und hing am 2026-10-23 an 248 bekannten Luecken.
# `sprechen-fest.txt` nennt nur, was der Laeufer braucht, je Paket mit
# Fassung und Pruefsumme fuer alle Plattformen; `--require-hashes` laesst
# kein Paket herein, dessen Summe nicht dort steht. Wie sie entsteht, steht
# in `sprechen.in`.
#
# ⚑ **Eine Umgebung von einem anderen Stand wird ersetzt, nicht
# nachgebessert.** pip installiert nur hinzu; Pakete der alten Liste
# (gradio, fastapi, ...) blieben liegen. Die alte wird beiseitegelegt und
# bei einem Abbruch zurueckgelegt. ⚠️ Gebaut wird am endgueltigen Ort und
# nicht daneben: Eine venv traegt ihren Pfad in den Startskripten und
# laesst sich nicht verschieben.
case "$(umgebung_stand)" in
  da) echo "liegt schon, auf dem Stand der Liste" ;;
  *)
    if [ -d "$COSY/.venv" ]; then
      rm -rf "$COSY/.venv.alt" && mv "$COSY/.venv" "$COSY/.venv.alt" || exit 1
    fi
    if "$python_neu" -m venv "$COSY/.venv" \
      && "$COSY/.venv/bin/python" -m pip install --require-hashes -r "$LISTE" \
      && summe "$LISTE" > "$STANDMARKE"; then
      rm -rf "$COSY/.venv.alt"
    else
      rm -rf "$COSY/.venv"
      [ -d "$COSY/.venv.alt" ] && mv "$COSY/.venv.alt" "$COSY/.venv"
      echo "Die Umgebung liess sich nicht bauen; die vorige liegt wieder an ihrem Platz."
      exit 1
    fi
    ;;
esac
P="$COSY/.venv/bin/python"
echo "benutzt: $("$P" -V)"

sagen "Seine Gewichte"
# ⛔️ **CosyVoice 3 und nicht 2, und das ist der Unterschied zwischen
# brauchbarem und unbrauchbarem Deutsch.** Die 2.0-Gewichte decken
# Chinesisch und Englisch ab; eine deutsche Stimmprobe ergibt damit
# Laute, die wie Deutsch klingen und keines sind. Gemessen am
# 2026-09-17: whisper las aus dem Ergebnis „dies Go! Ist die Örsteck ist
# proschein" statt „dies ist die erste gesprochene Antwort".
Z="$COSY/pretrained_models/Fun-CosyVoice3-0.5B"
R=https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512/resolve/29e01c4e8d000f4bcd70751be16fa94bf3d85a18
mkdir -p "$Z/CosyVoice-BlankEN" || exit 1
while read -r f soll; do
  hol "$Z" "$f" "$R/$f" "$soll" || exit 1
done <<'GEWICHTE'
config.json 44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a
configuration.json c502b6328c67638b401df8dd05de89e9e8d1cff9cd0ada10dfbdbe13556c20de
cosyvoice3.yaml f5a6b2c6f05139d0f18861a1fe506f751e787026b77c05f7e8fef9f8a4405965
campplus.onnx a6ac6a63997761ae2997373e2ee1c47040854b4b759ea41ec48e4e42df0f4d73
speech_tokenizer_v3.onnx 23236a74175dbdda47afc66dbadd5bcb41303c467a57c261cb8539ad9db9208d
flow.decoder.estimator.fp32.onnx 9b51b9533a55937762b262bf2cf9c6220ce40760f76d6532cb16a6a6d84059a8
llm.pt 69f43bd545131c30e98947fb360ea8b4dc9916d8e83dded7757c7ea4f5a24970
flow.pt a6fab32a7825e5b0bc855ddd948f8db9370b0a786fbc249caa4595e95b608e4b
hift.pt b279d7641eb97ae55b3b540cfba4f953c26492a2df758328a89a4d007ab87a65
CosyVoice-BlankEN/config.json 168aa1bd401abc3bc262ba15ba4e499627a8b4e006e9d050b47c22de20660185
CosyVoice-BlankEN/generation_config.json e558847a8b4402616f1273797b015104dc266fe4b520056fca88823ba8f8ebe6
CosyVoice-BlankEN/merges.txt ac8ff86a72bee70828fbc1119bc4398c6f3a9a6e490d7b0dbe917be025478bd0
CosyVoice-BlankEN/vocab.json ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910
CosyVoice-BlankEN/tokenizer_config.json 482bd979881423375ca5414e4e0d94cd7c5349dbb17fffd46b4d36d71e62a1bc
CosyVoice-BlankEN/model.safetensors 130282af0dfa9fe5840737cc49a0d339d06075f83c5a315c3372c9a0740d0b96
GEWICHTE
# ⚑ `spk2info.pt` (eingebaute Stimmen) wird nicht mehr geholt. Im festen
# Stand gibt es sie nicht, und vorher kam sie als einzige Pickle-Datei
# ohne jede Pruefung vom wandernden Zweig. Gesprochen wird mit Stimmprobe.

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
