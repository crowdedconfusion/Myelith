0.9.0

# Myelith: die aktuelle Fassung und was sie ändert

Die erste Zeile dieser Datei ist die Fassung von Myelith, und nur sie. Das
Programm liest sie beim Bauen ein und vergleicht sie beim Nachsehen nach
Updates mit derselben Zeile auf GitHub. Ist die Fassung dort höher, bietet
es das Update an; erst nach einer Bestätigung holt es den neuen Stand, baut
und installiert alles und startet neu.

Darunter steht ein einziger Abschnitt: was sich mit der aktuellen Fassung
ändert. Er wird mit jeder neuen Fassung ersetzt, ältere Fassungen stehen
hier nicht. Das Programm zeigt ihn vor dem Update.

## 0.9.0 (2026-11-12)

⚠️ **Neue Regeln für die Ausschüttung.** Eine Kette aus einer älteren
Fassung lässt sich nicht weiterführen; ein Knoten beginnt sie neu.

- **Bücher lesen, auch gescannte.** `myl scan` erkennt den Text gescannter
  PDFs und Bilder ohne Zusatzprogramm, in sechzehn Schriften (darunter
  Kyrillisch, Arabisch, Hebräisch, Chinesisch und Japanisch auch senkrecht,
  Koreanisch, Fraktur). `myl buch <datei>` und `/book` in der Konsole machen
  aus einem Buch einen Skill; im Fenster öffnet der Skillknopf „Buch lesen“.
- **Eine Bibliothek gelesener Bücher.** Jedes Buch liegt seitenweise unter
  `literature/`, und der Agent findet darin wörtliche Stellen samt Seite.
- **Bücher in deiner Sprache.** Book2Skill übersetzt das Werk in die
  eingestellte Sprache, mit einem eigenen kleinen Übersetzungsmodell.
- **Die Ausschüttung zielt auf eine Auslastung von 80 Prozent**, und die
  Abgabe fürs Training ist entsprechend gedeckelt.
- **Erprobung: Aufnehmen und Mitschreiben ohne Zusatzprogramm.** Mit
  `MYL_AUFNAHME=eigen` nimmt Myelith selbst auf, mit `MYL_HOEREN=eigen`
  schreibt es mit eigenem whisper mit; das Einrichtungsskript holt dessen
  Gewichte mit Prüfsummen. Vorgabe bleiben ffmpeg und whisper.cpp. WAV
  und FLAC, auch als Stimmprobe, brauchen kein ffmpeg mehr, auf dem Mac
  auch m4a und MP3 nicht.
- **Myelith spricht mit eigener Stimme, ohne Python.** Qwen3-TTS rechnet im
  Programm, klont aus einer Stimmprobe und spricht gestreamt; CosyVoice ist
  entfernt. Das Einrichtungsskript holt die Gewichte mit Prüfsummen.
- **Jede erzeugte Sprache ist doppelt gekennzeichnet:** ein unhörbares
  Wasserzeichen, das der Stimme folgt, und ein signiertes Manifest nach
  C2PA, das die Prüfseite der Content Authenticity Initiative als
  KI-erzeugt anzeigt. Nachprüfen mit `myl kennzeichen`.
- **Der Verlauf jedes Wallets** steht auch in Konsole (`v`) und Fenster, und
  die Übersicht des Minings nennt Epoche und Rolle des Knotens.
- **Die zwölf Wörter sind geschützt:** Fensterschutz vor Aufnahmen,
  kein Kopieren, Sperre nach zwei Minuten; ein Keystore mit verändertem
  IV wird beim Entsperren erkannt.
- **Jedes Freigabebündel bringt die Lizenztexte** der mitgelieferten
  Modelle und Bibliotheken mit.
- **Robuster gegen präparierte Dateien und Seiten:** Ton, PDF, Bücher und
  Webseiten mit erfundenen Größen, verschachteltem XML, JSON oder CBOR
  bringen Myelith nicht mehr zum Hängen oder Absturz, eine hängende
  PDF-Seite hält das Lesen eines Buchs nicht fest, und Steuerzeichen aus
  fremdem Text erreichen das Terminal nicht.
