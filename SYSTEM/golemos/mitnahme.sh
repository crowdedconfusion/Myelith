#!/bin/sh
# Sammelt, was dieser Rechner an Myelith hat, fuer die Datenpartition.
#
#   sh SYSTEM/golemos/mitnahme.sh [--artefakt NAME]... [--alle-artefakte]
#                                  [--sehen] [--hoeren]
#
# ⚑ **Es richtet sich nach dem Arbeitsverzeichnis, nicht nach Git.**
# Die Kiste `1337` steht in `.gitignore` und liegt bewusst nur oertlich;
# ein frischer Klon hat sie nicht, dieser Rechner schon. Der Auftrag
# lautet „was der Nutzer hinzugefuegt hat", also zaehlt, was dasteht.
set -eu

HIER=$(cd "$(dirname "$0")" && pwd)
WURZEL=$(cd "$HIER/../.." && pwd)
ZIEL="$HIER/bau/mitnahme"
ARTEFAKTE=""
ALLE=nein
# ⛔️ **Die Vorgabe packt nichts Grosses ein.** 📌 Der erste Lauf nahm
#    die Sinnesmodelle ungefragt mit und kam auf 18 GB, davon 13 GB
#    Hoermodelle, die niemand angefordert hatte. ⚑ **Was Gigabyte
#    kostet, wird genannt oder bleibt liegen**; eine Vorgabe, die
#    heimlich das Groesste einpackt, ist keine Vorgabe, sondern eine
#    Ueberraschung.
SEHEN=nein
HOEREN=nein

while [ $# -gt 0 ]; do
  case "$1" in
    --artefakt) ARTEFAKTE="$ARTEFAKTE $2"; shift 2 ;;
    --alle-artefakte) ALLE=ja; shift ;;
    --sehen) SEHEN=ja; shift ;;
    --hoeren) HOEREN=ja; shift ;;
    -h|--hilfe) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unbekannt: $1" >&2; exit 2 ;;
  esac
done

rm -rf "$ZIEL"
mkdir -p "$ZIEL"

kopiere() {
  quelle="$1"; name="$2"
  [ -e "$quelle" ] || { printf "  %-16s (nicht da)\n" "$name"; return 0; }
  mkdir -p "$ZIEL/$name"
  cp -a "$quelle/." "$ZIEL/$name/"
  # 📌 Fund 475: Aus `~/.myelith/sinne` kam der Bytecode des Wirts mit
  #    (`cpython-311.pyc`), fuer ein Python, das GolemOS nicht hat. Er
  #    nuetzt dort nichts und machte die Sauberkeitsprobe rot.
  find "$ZIEL/$name" -name __pycache__ -type d -prune -exec rm -rf {} +
  printf "  %-16s %s\n" "$name" "$(du -sh "$ZIEL/$name" | cut -f1)"
}

echo "── Sammeln"
kopiere "$WURZEL/CLIENT/werkzeugkisten" werkzeugkisten
# ⛔️ **`myelith/` und nicht direkt nach `/daten` (Fund 455).** GolemOS
#    setzt `XDG_CONFIG_HOME=/daten`, und der Client sucht darunter
#    `myelith/client.json` und `myelith/skills`. Wer eine Ebene zu hoch
#    ablegt, legt etwas ab, das niemand liest, und das sieht wie Erfolg
#    aus.
kopiere "${XDG_CONFIG_HOME:-$HOME/.config}/myelith/skills" myelith/skills
# ⚠️ **Die Sinne bleiben vorerst unverdrahtet.** Der Client sucht sie
#    unter `$HOME/.myelith/sinne`, also nicht unter der Konfiguration.
#    Sie gehoeren zu Stufe 3, und deren Programme liegen ohnehin nicht
#    im Abbild; es waere eine Verbindung zu etwas, das es nicht gibt.
kopiere "$HOME/.myelith/sinne" sinne
if [ "$SEHEN" = ja ];  then kopiere "$WURZEL/MODELS/vision" sehmodelle
  else echo "  sehmodelle       (nicht gewaehlt, --sehen)"; fi
if [ "$HOEREN" = ja ]; then kopiere "$WURZEL/MODELS/audio" hoermodelle
  else echo "  hoermodelle      (nicht gewaehlt, --hoeren)"; fi

# ⚠️ **Artefakte nur auf Ansage.** Fuenf zusammen sind 77 GB; ein
#    Abbild, das sie alle traegt, passt auf keinen Stick, den jemand
#    einsteckt. Wer keines nennt, bekommt ein System ohne Modell, und
#    das ist eine ehrliche Vorgabe: Es startet, sagt „kein Artefakt",
#    und der Mensch legt eines nach.
mkdir -p "$ZIEL/artefakte"
if [ "$ALLE" = ja ]; then
  ARTEFAKTE=$(ls "$WURZEL/INTEGER_LLM/artifacts" 2>/dev/null)
fi
for a in $ARTEFAKTE; do
  q="$WURZEL/INTEGER_LLM/artifacts/$a"
  [ -d "$q" ] || { echo "  ⛔️ Artefakt $a gibt es nicht" >&2; exit 3; }
  mkdir -p "$ZIEL/artefakte/$a"
  cp -a "$q/." "$ZIEL/artefakte/$a/"
  printf "  %-16s %s\n" "artefakt $a" "$(du -sh "$ZIEL/artefakte/$a" | cut -f1)"
done
[ -n "$ARTEFAKTE" ] || echo "  artefakte        (keines gewaehlt)"

# ── Die Einstellungen: Entscheidungen mitnehmen, Pfade neu setzen ───
#
# ⛔️ **Ein Kopieren waere ein Fehler, der wie Erfolg aussieht.** In der
#    Datei stehen absolute Pfade dieses Rechners
#    (`/Users/.../artifacts/myelith-35b-a3b`). Auf GolemOS gibt es die
#    nicht. Das System startet, sieht richtig aus und findet kein
#    Modell, und die Meldung nennt nicht den Grund. 📌 Dieselbe
#    Fehlerklasse wie der Vorrat mit absolutem Pfad (Fund 446).
#
# ⚑ **Mitgenommen wird, was der Nutzer GEWAEHLT hat** (Sprache, Design,
#    Tokengrenze, Denkmodus, Betriebsart, Schrittzahl, Haekchen), und
#    die Pfade werden auf die Orte gesetzt, die GolemOS wirklich hat.
QUELLE=""
for k in "${XDG_CONFIG_HOME:-$HOME/.config}/myelith/client.json" \
         "$HOME/.myelith/client.json"; do
  [ -f "$k" ] && { QUELLE="$k"; break; }
done

echo "── Einstellungen"
if [ -n "$QUELLE" ]; then
  ERSTES=$(ls "$ZIEL/artefakte" 2>/dev/null | head -1)
  mkdir -p "$ZIEL/myelith"
  python3 - "$QUELLE" "$ZIEL/myelith/client.json" "$ERSTES" <<'PY'
import json, sys
quelle, ziel, erstes = sys.argv[1], sys.argv[2], sys.argv[3]
e = json.load(open(quelle, encoding="utf-8"))

# ⛔️ Jeder Pfad wird neu gesetzt. Was hier NICHT steht, bleibt wie es
#    ist, und das ist die Absicht: Entscheidungen ueberleben.
e.setdefault("agent", {})
e.setdefault("modell", {})
e["agent"]["wurzel"] = "/daten/arbeit"
e["agent"]["kistenordner"] = "/daten/werkzeugkisten/Base"
e["modell"]["artefakt"] = f"/daten/artefakte/{erstes}" if erstes else None

json.dump(e, open(ziel, "w", encoding="utf-8"), ensure_ascii=False, indent=2)
gesetzt = [f"{k}.{u}" for k, us in (("agent", ("wurzel", "kistenordner")),
                                     ("modell", ("artefakt",))) for u in us]
print("  aus    ", quelle)
print("  Pfade neu gesetzt:", ", ".join(gesetzt))
print("  Rest uebernommen: ", ", ".join(sorted(e.keys())))
PY
else
  echo "  (keine gefunden, GolemOS nimmt seine Vorgaben)"
fi

mkdir -p "$ZIEL/arbeit"
echo
echo "── Mitnahme fertig: $(du -sh "$ZIEL" | cut -f1) in $ZIEL"
