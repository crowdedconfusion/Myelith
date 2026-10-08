# MODULES: Erweiterungen der Konsole

Ein **Modul** erweitert die Konsole (`myelith`) um eigene Befehle, einen
eigenen Modus mit Fußzeile und Bereich unter dem Eingaberahmen, Fragen an
das geladene Modell und Agentenläufe mit eigener Werkzeugkiste. Die Konsole
baut und läuft ohne jedes Modul; dann gibt es keinen seiner Befehle, weder
in `/help` noch in der Vervollständigung.

Module liegen hier, je eines in einem eigenen Ordner (`MODULES/<name>/`),
oder im Ordner `module/` neben den Einstellungen der Konsole. `/module`
zeigt, welche gefunden und geprüft wurden.

## Was ein Modul ist

Ein Ordner mit drei Dingen:

| Datei | was sie trägt |
|---|---|
| `modul.json` | die Beschreibung: Name, Version, Protokoll, Programm, Befehle, Befugnisse und die SHA-256 jeder Datei des Moduls |
| `modul.sig` | die Signatur (Ed25519) über die Beschreibung |
| das Programm | was die Konsole startet; es spricht mit ihr in JSON-Zeilen über die Standardein- und ausgabe |

Dazu, was das Modul sonst braucht: Quellen, Anleitungen, eine Werkzeugkiste,
Skills, und Ordner, deren Inhalt sich im Betrieb ändert (Daten, Zustand).

## Die Beschreibung (`modul.json`)

```json
{
  "name": "beispiel",
  "version": "0.1.0",
  "protokoll": 1,
  "was": "Ein Satz für /module.",
  "laufzeit": {"art": "nativ", "programme": {"aarch64-macos": "bin/beispiel"}},
  "befehle": [
    {"name": "/beispiel", "was": "startet den Modus"},
    {"name": "/beispiel status", "was": "zeigt den Stand"}
  ],
  "befugnisse": {"modell": true, "agent": false, "unten": 12, "fusszeile": true},
  "kiste": "werkzeuge",
  "skills": "skills",
  "arbeit": "daten/arbeit",
  "veraenderlich": ["daten"],
  "dateien": {}
}
```

- **Streng gelesen**: Ein unbekanntes Feld ist ein Fehler. Pfade sind
  relativ zum Modulordner und können ihn nicht verlassen.
- **Befehle**: Der erste ist der Hauptbefehl, alle weiteren beginnen mit
  ihm. Ein Modul kann keinen Befehl der Konsole belegen, und ein Name gilt
  nur einmal.
- **Befugnisse**: Was nicht genannt ist, ist verboten. `modell` (das
  geladene Modell fragen), `agent` (Agentenläufe mit der eigenen Kiste),
  `netzmodell` (über das Netz statt am örtlichen Modell), `unten` (Zeilen
  unter dem Rahmen, höchstens 24), `fusszeile`, `http` (erreichbare
  Rechner für abgeschottete Module), `modell_token_je_stunde`.
- **Veränderlich**: Ordner, deren Inhalt nicht signiert ist. Programm,
  Werkzeugkiste und Skills liegen nie darin; der Arbeitsordner (`arbeit`)
  eines Agentenlaufs immer.
- **`dateien`** füllt das Signieren aus; von Hand bleibt es leer.

## Signieren und Vertrauen

```sh
myl-module schluessel <datei>                 # ein Schlüsselpaar; geheim in <datei>, öffentlich auf stdout
myl-module signieren MODULES/beispiel <datei> # Prüfsummen eintragen, signieren
myl-module pruefen MODULES/beispiel <vertrauen.json>
```

⛔️ **Der geheime Schlüssel gehört nie ins Repositorium** und nie in einen
Modulordner.

Die Konsole startet nur Module, deren Signatur zu einem Schlüssel ihrer
**Vertrauensliste** passt (`vertrauen.json` neben den Einstellungen):

```json
{
  "schluessel": [
    {"name": "Wer", "oeffentlich": "<64 Hex-Zeichen>", "nativ": false, "widerrufen": false}
  ]
}
```

Abgelehnt wird ein Modul ohne Signatur, mit fremdem oder widerrufenem
Schlüssel, mit einer veränderten, fehlenden oder dazugelegten Datei, mit
einem symbolischen Verweis, und in einer kleineren Version als der zuletzt
gestarteten. Geprüft wird beim Start der Konsole und vor jedem Start eines
Moduls, an genau den Bytes, die danach laufen: Die Konsole führt eine eigene
Kopie aus. Ändert sich die Signatur eines laufenden Moduls, startet die
Konsole es nach erneuter Prüfung neu.

## Installieren

```
/module install <ordner>
```

prüft das Modul, kopiert es (ohne den Inhalt veränderlicher Ordner) in den
Ordner `module/` neben den Einstellungen und prüft die Kopie.

## Was die Prüfung schützt, und was nicht

- Sie schützt davor, dass ein Modul unterwegs oder auf der Platte verändert,
  auf eine alte Fassung zurückgesetzt oder mit fremden Dateien gemischt
  wird, und davor, dass ein Modul über die Konsole mehr tut, als seine
  Befugnisse nennen: Text eines Moduls kommt ohne Steuerzeichen an, Fragen
  an den Menschen stellt die Konsole in ihrem eigenen Rahmen.
- ⛔️ **Ein natives Modul ist keine Sicherheitsgrenze.** Es ist ein Programm
  mit den Rechten des Nutzers und kann auf dem Rechner tun, was er kann.
  Deshalb startet ein natives Modul nur mit einem Schlüssel, der in der
  Vertrauensliste ausdrücklich als `nativ` markiert ist. **Installieren
  heißt vertrauen.**
- Module, die abgeschottet laufen (WebAssembly, ohne eigenes Dateisystem
  und ohne eigenes Netz), folgen; die Beschreibung kennt sie schon
  (`"laufzeit": {"art": "wasm", "datei": "…"}`).
