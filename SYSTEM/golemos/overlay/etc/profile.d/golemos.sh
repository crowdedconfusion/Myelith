# Wo GolemOS die Einstellungen sucht.
#
# ⛔️ **Fund 455: `mitnahme.sh` schrieb eine Datei, die niemand liest.**
#
# Das Sammelskript legte `client.json` auf die Datenpartition und setzte
# darin sorgfaeltig alle Pfade auf `/daten/...` um. Der Client sucht
# seine Einstellungen aber unter `$XDG_CONFIG_HOME/myelith/client.json`
# oder `$HOME/.config/myelith/client.json`, und **nichts auf GolemOS
# zeigte dorthin**. Die Datei lag richtig da und wurde nie angefasst.
#
# ⚠️ **Der Fehler sah wie Erfolg aus.** Das System startet, die Datei
# ist sichtbar, das Modell wird trotzdem nicht gefunden, und die Meldung
# nennt nicht den Grund. Dieselbe Klasse wie der Vorrat mit absolutem
# Pfad: Was fehlt, ist kein Stueck Arbeit, sondern eine Verbindung
# zwischen zwei Stuecken, die beide fertig sind.
#
# ⚑ **Eine Zeile, ein Mechanismus.** Der Client kennt
# `XDG_CONFIG_HOME` bereits als ersten Suchort; es braucht also kein
# Sinnbild, kein zweites Skript und keine Sonderbehandlung im Client.
# Damit liegen `client.json` und die Wissensmappen unter
# `/daten/myelith/`, und alles andere, worauf `client.json` mit
# absoluten Pfaden zeigt, bleibt offen sichtbar daneben.
#
# ⚠️ **Es gilt fuer angemeldete Sitzungen**, denn `/etc/profile` liest
# dieses Verzeichnis. Das reicht genau deshalb, weil GolemOS **einen**
# Einstieg hat: den Menschen an der Konsole. Es laeuft hier kein Dienst,
# der Myelith im Hintergrund startet. Kaeme je einer dazu, braeuchte er
# die Variable in seiner eigenen Einheit, und **dann waere es eine
# Angabe an zwei Orten**; das gehoert an dem Tag entschieden und nicht
# vorsorglich verdoppelt.
#
# ⚠️ **Ohne Datenpartition zeigt die Variable ins Leere**, und das ist
# richtig so: Der Client geht dann seine naechste Moeglichkeit durch und
# nimmt am Ende seine Vorgaben.
export XDG_CONFIG_HOME=/daten

# ⚑ **Der Klon, den `S06myelith` gefunden hat.** Der Client kennt
#   `MYELITH_WURZEL` als ersten Suchort und loest relative Artefaktpfade
#   dagegen auf; mehr braucht es nicht, damit `myl` und `myelith` den
#   Ordner auf dem Stick benutzen.
if [ -r /run/golemos/wurzel ]; then
  MYELITH_WURZEL=$(cat /run/golemos/wurzel)
  export MYELITH_WURZEL
fi

# ⚑ **Der Assistent kommt bei der ersten Anmeldung von selbst**, bis
#   jemand „Zur Konsole" waehlt; danach startet er mit
#   `golem-einrichten`. Nur an einem Terminal, damit ein Skript, das
#   `/etc/profile` liest, nicht auf eine Eingabe wartet.
#   ⚠️ Ohne Datenpartition kann er sich das nicht merken und kaeme bei
#   jeder Anmeldung wieder; dann gibt es nur den Hinweis.
if [ -t 0 ] && [ "$(id -u)" = 0 ] && [ -z "$GOLEMOS_OHNE_ASSISTENT" ]; then
  if grep -q " /daten " /proc/mounts && [ ! -e /daten/.assistent-gesehen ]; then
    /usr/bin/golem-einrichten
  else
    echo "GolemOS einrichten: golem-einrichten"
  fi
fi
