#!/bin/sh
# Richtet Sehen, Hoeren und Sprechen ein.
#
# ⚑ **Was das Repositorium mitbringt und was nicht** (Festlegung des
# Projektinhabers, 2026-09-17): Ein frischer Klon soll alles dabeihaben,
# **ausser den Modellgewichten**. Dieses Skript ist genau dieses „alles":
# Es holt die Programme ueber den Paketverwalter des Systems, legt die
# Gewichte an die Stellen, an denen der Client sie sucht, darunter
# whisper fuer das Hoeren und Qwen3-TTS fuer das Sprechen im eigenen
# Rechenweg.
#
# ⚑ **CosyVoice wird seit CLIENT 0.219.0 nicht mehr eingerichtet**
# (Festlegung des Projektinhabers, 2026-11-11): Die Modellkarte nennt neben
# Apache 2.0 „nur fuer akademische Zwecke“, und es brauchte Python und
# PyTorch. Eine fruehere Installation bleibt liegen, der Client benutzt sie
# nicht mehr; `--pruefen` sagt, wo sie liegt.
#
# ⚑ **Wohin die Gewichte gehen, hat sich am 2026-09-21 geaendert**
# (Festlegung des Projektinhabers): in den Klon unter `MODELS/audio` und
# `MODELS/vision`, zu allen anderen fremden Gewichten dieses Projekts.
# **Versioniert wird dort nichts**, nur die Ordner und ihre Doku; das
# regelt `MODELS/.gitignore`.
#
# ⚠️ **In der Sinnesheimat bleiben zwei Dinge, und zwar mit Grund:** der
# `bin`-Ordner fuer eigene Skripte (muss beschreibbar sein) und eine
# abgelegte Stimmprobe (gehoert dem Nutzer). Gewichte sind
# geladen, und nur die haben einen Rubrikordner verdient.
#
# ⛔️ **Die Binaerdateien selbst gehoeren nicht ins Repositorium.**
# llama.cpp, whisper.cpp und ffmpeg sind je System verschieden und
# zusammen einige hundert Megabyte. Das
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
# unter `SYSTEM/MODELS` ab.
HIER=$(cd "$(dirname "$0")/../.." && pwd)
if [ ! -f "$HIER/CLIENT/myl-senses/Cargo.toml" ]; then
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
SPRECHER="$AUDIO/qwen3-tts-0.6b-base"

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
  for g in whisper-large-v3-turbo whisper-small whisper-base; do
    if [ -s "$AUDIO/$g/model.safetensors" ]; then
      printf '  %-24s %s\n' "$g" "$(du -h "$AUDIO/$g/model.safetensors" | cut -f1)"
    fi
  done
  for d in model.safetensors speech_tokenizer/model.safetensors; do
    if [ -s "$SPRECHER/$d" ]; then
      printf '  %-24s %s\n' "Qwen3-TTS $d" "$(du -h "$SPRECHER/$d" | cut -f1)"
    else
      printf '  %-24s %s\n' "Qwen3-TTS $d" "fehlt ($SPRECHER)"
    fi
  done
  # ⚑ Eine fruehere CosyVoice-Installation wird nur genannt, nicht
  #   geloescht: Sie kann mehrere Gigabyte belegen, gehoert aber dem Nutzer.
  for alt in "$AUDIO/CosyVoice" "$HEIMAT/cosyvoice" "$HEIMAT/bin/sprechen-cosyvoice.py"; do
    if [ -e "$alt" ]; then
      printf '  %-24s %s\n' "nicht mehr benutzt" "$alt (darf weg)"
    fi
  done
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
}

if [ "$nur_pruefen" = 1 ]; then pruefen; exit 0; fi

echo "Das holt rund 6 GB Gewichte fuer Sehen und Hoeren, dazu 1,6 GB (large-v3-turbo) oder 1 GB (small) fuer das eigene Hoeren"
[ "$mit_sprechen" = 1 ] && echo "und noch einmal rund 2,5 GB fuer Sprechen (Qwen3-TTS)."
echo "Sehen nach:    $VISION"
echo "Hoeren nach:   $AUDIO"
echo "Sprechen nach: $SPRECHER"
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
# Pruefung, darunter damals die Pickle-Dateien von CosyVoice (`llm.pt`,
# `flow.pt`, `hift.pt`), die torch beim Laden auspackt. Wer das fremde Repositorium
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

# ---------------------------------------------------------------- Hoeren, eigener Weg
# ⚑ **Die Originale von whisper fuer den eigenen Hoerweg** (CLIENT 8.2,
# `MYL_HOEREN=eigen`): je Groesse die fuenf Dateien, die `myl_senses::whisper`
# liest, jede mit Revision und Pruefsumme. Abgeglichen am 2026-11-12 mit der
# Dateiliste von Hugging Face zu genau diesen Revisionen.
#
# Welche Groesse, folgt der Regel des Clients (`whisper::groesse_waehlen`):
# large-v3-turbo mit Grafikeinheit oder ab 12 Kernen, sonst small. Auf macOS
# rechnet Metal, also large-v3-turbo. Eine andere Wahl: MYL_HOERGROESSE auf
# whisper-large-v3-turbo, whisper-small oder whisper-base.
# ⚠️ Ohne Grafikeinheit erkennt dieses Skript eine Karte unter Linux nicht;
# wer eine hat und weniger als 12 Kerne, setzt MYL_HOERGROESSE.
kerne=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)
if [ -n "${MYL_HOERGROESSE:-}" ]; then
  groessen="$MYL_HOERGROESSE"
elif [ "$(uname -s)" = Darwin ] || [ "$kerne" -ge 12 ]; then
  groessen="whisper-large-v3-turbo"
else
  groessen="whisper-small"
fi
for g in $groessen; do
  case "$g" in
    whisper-large-v3-turbo) rev=41f01f3fe87f28c78e2fbf8b568835947dd65ed9 ;;
    whisper-small) rev=973afd24965f72e36ca33b3055d56a652f456b4d ;;
    whisper-base) rev=e37978b90ca9030d5170a5c07aadb050351a65bb ;;
    *) echo "unbekannte Groesse: $g"; exit 2 ;;
  esac
  sagen "Hoeren im eigenen Rechenweg ($g)"
  mkdir -p "$AUDIO/$g" || exit 1
  grep "^$g " <<'GEWICHTE' | while read -r _ f soll; do
whisper-large-v3-turbo config.json c5b526b3e3cd64cd8940dabb45e8ba726629e22d8ed389c29b552f9140daf04a
whisper-large-v3-turbo generation_config.json cce11bfe3aaa6ae9e072ea2637caaec8795e68d9b67e655a5af16ee509681a4c
whisper-large-v3-turbo vocab.json e2aa043ef015641d363d8288e7c241c85e36a5c761fb303598e0710233344387
whisper-large-v3-turbo added_tokens.json 3c51f66c4c21f9e126970078f11ae77a78c74aee8df606ee9daba86e467108e0
whisper-large-v3-turbo model.safetensors 542566a422ae4f3fd23f1ba11add198fca01bbf82e66e6a2857b3f608b1eb9d1
whisper-small config.json e6a2b489da1b5aed65a8eb8d1e7466fa867ad5643a8bc138ba708bd56b2875c4
whisper-small generation_config.json 71565b8ef50d0bf7a1193ed4bbed195b94e70c18894d81bba2f1233dcec3ab53
whisper-small vocab.json 8f680bba319e01a653d2e8a5dbc17a9157179e0576e6ce74ce0c06356c6e24f9
whisper-small added_tokens.json 9715fd2243b6f06a5858b5e32950d2853f73dd5bc201aafcf76f5082a2d8acd1
whisper-small model.safetensors 1d7734884874f1a1513ed9aa760a4f8e97aaa02fd6d93a3a85d27b2ae9ca596b
whisper-base config.json a153c53883a6799b6f056b4a8d1a515c9926d03994682ba88a7616618d7da0c1
whisper-base generation_config.json 444b3f636d2fff89dd9ecf549e2a085b61f7ff0fa0246d4628bac6a3b8cc9ba4
whisper-base vocab.json 8f680bba319e01a653d2e8a5dbc17a9157179e0576e6ce74ce0c06356c6e24f9
whisper-base added_tokens.json 9715fd2243b6f06a5858b5e32950d2853f73dd5bc201aafcf76f5082a2d8acd1
whisper-base model.safetensors 07cadb9f25677c8d50df603e66a98fbd842cce45047139baeb16e6219a1e807b
GEWICHTE
    hol "$AUDIO/$g" "$f" "https://huggingface.co/openai/$g/resolve/$rev/$f" "$soll" || exit 1
  done || exit 1
done

if [ "$mit_sprechen" = 0 ]; then pruefen; exit 0; fi

# ---------------------------------------------------------------- Sprechen
sagen "Sprechen (Qwen3-TTS-12Hz-0.6B-Base)"
# ⚑ **Apache 2.0, ohne Zusatz**, und gerechnet im Programm, ohne Python.
# Geholt wird nur, was der eigene Weg liest: Sprecher und Tonzerleger je
# mit `config.json`, dazu Wortschatz und Verschmelzungen des Textzerlegers.
# Die Pruefsummen der beiden Gewichte stehen auch in
# `MODELS/audio/LIZENZEN.json`.
T=https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-Base/resolve/5d83992436eae1d760afd27aff78a71d676296fc
mkdir -p "$SPRECHER/speech_tokenizer" || exit 1
while read -r f soll; do
  hol "$SPRECHER" "$f" "$T/$f" "$soll" || exit 1
done <<'GEWICHTE'
config.json 2e714c787c8edb98b05432685cddb634add2de4d4e645f653d68251ef72ba011
vocab.json ca10d7e9fb3ed18575dd1e277a2579c16d108e32f27439684afa0e10b1440910
merges.txt 599bab54075088774b1733fde865d5bd747cbcc7a547c5bc12610e874e26f5e3
model.safetensors 180b3b10eb1c9f1b4db7806d5475bae3071c0243c299d49926bab1da3b6946f6
speech_tokenizer/config.json ee65bb901c876664ab8707c487157aa1a6ee57c65969b28fb5ec9dc211e68167
speech_tokenizer/model.safetensors 836b7b357f5ea43e889936a3709af68dfe3751881acefe4ecf0dbd30ba571258
GEWICHTE

pruefen
echo
echo "Fertig. Im Fenster erscheinen Sprechtaste und Lautsprecher von selbst."
