# gateway (`myl-gateway`)

> **Version:** 0.1.0
> **Datum:** 2026-09-01
> **Status:** Stufe 1 steht: die Tür auf `localhost` und der Beleg. Sie
> nimmt entgegen und schreibt fest; **an einen Pod gibt sie noch nichts**.

## ⚑ Die Rolle stand im Papier und hatte keine Komponente (Fund 87)

Kap. 3.3 führt Gateways als eine von sechs Rollen: Nutzeranfragen
entgegennehmen, an Pods geben, Ergebnisse zurückliefern. **Es gab weder
Komponente noch Code noch einen HTTP-Server im ganzen Repositorium.**
Zwei gebaute Schutzmechanismen hatten damit keinen Betreiber, und das
Netzmodell war ohne den Agent Layer nicht benutzbar.

## Der Schnitt: Stufe 1

Das eigene Gateway auf `localhost`. **Der Betreiber ist der
Kontoinhaber, also entfällt die ganze Bezahlfrage**, und mit ihr
Zugangsschlüsselverwaltung, Ratenbegrenzung und Missbrauchsschutz. Was
bleibt, ist die Tür und der Beleg, und der Beleg ist ohnehin das
Produkt.

## ⚑ Keine HTTP-Bibliothek, und das ist eine Entscheidung

Sie ist die größte neue Abhängigkeitsfläche seit `libp2p` und gehört
deshalb **vor** den ersten Code entschieden. Sie lautet: keine.

Was ein Rahmenwerk mitbrächte, ist Wegewahl und Mittelschicht für
Anforderungen, die Stufe 1 nicht hat; `axum` zöge `hyper` und `tower`
nach, **drei Bäume für eine Tür mit einem Weg**. `tokio` liegt ohnehin in
NODE und NETWORKING.

⚑ **Der eigentliche Gewinn ist der Zeitpunkt.** Sobald Stufe 2 kommt,
also ein öffentliches Gateway mit TLS, Zugang und Ratenbegrenzung, ist
die Rahmenwerksfrage eine **echte** Frage mit echten Anforderungen. Sie
jetzt zu entscheiden hieße, sie ohne Anforderungen zu entscheiden. **Wer
die Abhängigkeit für die Stufe nimmt, die sie nicht braucht, trifft die
Wahl im ungünstigsten Augenblick.**

**Wird umgestoßen, wenn** Stufe 1 mehr als einen Weg braucht, oder ein
Klient mehr verlangt als `Content-Length` und einen Rumpf.

## ⚑ Handgeschriebenes HTTP: wo die Gefahr liegt und wie sie gefasst ist

Sie liegt dort, wo **Zerlegung auf fremde Eingaben** trifft. Deshalb
steht das Zerlegen als **reine Funktion** ohne Netz und wird einzeln
geprüft; der Teil, der Sockets anfasst, bleibt so dünn, dass an ihm
nichts schiefgehen kann. Dieselbe Bauart wie `anfragen_fuer` im Knoten.

**Abgelehnt wird ausdrücklich, statt geraten:**

- **`Transfer-Encoding: chunked`.** Es stillschweigend als Rumpf zu
  lesen wäre die klassische Schmuggelstelle: zwei Leser, zwei Meinungen
  über die Nachrichtengrenze.
- **Fehlendes oder doppeltes `Content-Length`.** Ohne Länge weiß niemand,
  wo die Nachricht endet; mit zweien wissen es zwei verschieden.
- **Ein Rumpf, der kürzer ankommt als angekündigt.** Ihn als kurze
  Anfrage zu nehmen hieße, **eine andere Frage festzuschreiben als die
  gestellte**.
- **Alles über den Grenzen** (Kopf 8 KiB, Rumpf 1 MiB). Der Deckel ist
  eine Ablehnung und kein Abschneiden.

⚑ **`Tuer::binden` nimmt keine Adresse entgegen**, sondern bindet fest an
`127.0.0.1`. Eine Adresse als Parameter wäre eine Einladung, sie auf
`0.0.0.0` zu setzen, und dann stünde eine Tür ohne Schloss im Netz. **Wer
öffentlich hören will, braucht Stufe 2 und nicht ein anderes Argument.**

## Der Beleg, und warum er zuerst kam

Ein Gateway, das nur eine Antwort liefert, ist ein Weiterleiter. **Was
Myelith anders macht, ist der Beleg.**

⚑ **Und ohne die Bindung der Anfrage geht auch Stufe 2 der Verifikation
nicht.** Der Prompt kam im Konsens nicht vor; ein Checker müsste ihn dem
Pod glauben und prüfte dann, ob der Pod zu seiner **eigenen** Eingabe
passt. Eine Frage, auf die der Gefragte beide Hälften wählt. Deshalb
schreibt das Gateway **zuerst** fest und leitet **dann** weiter.

Gebunden wird `SHA-256` über Trennstring, Sitzungsnummer und Anfrage;
die Sitzungsnummer geht mit ein, damit dieselbe Anfrage in zwei
Sitzungen nicht denselben Wert bekommt und eine Bindung nicht
übertragbar ist.

## Changelog

### v0.1.0 – 2026-09-01 (die Tür und der Beleg, Punkt 39)

Das **neunzehnte Crate**. 21 Tests: 15 auf die reinen Funktionen, 6 über
einen echten Socket.

⛑ **Der erste Testlauf blieb hängen**, und das war lehrreich: Der
Testklient schloss seine Schreibseite nicht, also wartete der Server auf
den angekündigten Rest und der Klient auf die Antwort. **Ein Deadlock,
und er hat gezeigt, dass der Server einen abgebrochenen Rumpf am
Dateiende erkennt und nicht an einer Frist** — die richtige Reihenfolge.
Der Klient schließt jetzt, und eine Fünf-Sekunden-Frist darüber ist der
zweite Riegel: **Ein hängender Test sagt nichts, ein fehlgeschlagener
sagt etwas.**

**Was Stufe 1 nicht tut:** an einen Pod geben. Dafür braucht sie eine
Sitzung im Netz, und die ist eigene Arbeit.
