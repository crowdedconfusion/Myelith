//! **Wer auf einer Maschine was bekommt**: Arbeitsspeicher, Grafikspeicher,
//! Kerne und Karten, verteilt auf die Arbeitslasten, die dort laufen
//! wollen.
//!
//! # ⚑ Drei Teile, und der mittlere ist rein
//!
//! 1. **Angebot** ([`local::erhebung`]): was die Maschine hat und was davon
//!    frei ist. Gefragt wird die Maschine, geraten wird nichts: Was sich
//!    nicht ermitteln laesst, ist `None`.
//! 2. **Bedarf** ([`local::bedarf`]): was eine Arbeitslast braucht. Fuer
//!    ein eigenes Artefakt auf das Byte genau aus seinen Dateien und seiner
//!    Modellbeschreibung, fuer ein fremdes Modell aus der Dateigroesse; den
//!    Rest, den keine Datei nennt, misst ein kurzer Lauf ([`local::eichung`]).
//! 3. **Plan** ([`local::plan`]): `belegen(angebot, lasten) → Plan`, eine
//!    reine Funktion in ganzen Zahlen. Dieselben Eingaben ergeben auf jeder
//!    Maschine denselben Plan, und das macht jeden Fehler wiederholbar.
//!
//! # ⚑ Warum der Plan keine Zahl des Rechenpfads aendert
//!
//! Zwischen Karten wird nur **zusammengesetzt**, nie verrechnet, und eine
//! Grenze der Kerne aendert keine Zeile einer Matrixmultiplikation. Zwei
//! Knoten mit verschiedenen Plaenen rechnen deshalb bitgleich. Der Plan ist
//! eine Frage der Geschwindigkeit und der Machbarkeit, nie des Ergebnisses;
//! darum steht er ausserhalb des Konsenses.
//!
//! # ⚑ Vorrang: Agent vor Vision vor Sprache
//!
//! Festlegung des Projektinhabers. Passt nicht alles, weicht zuerst die
//! Last mit dem niedrigsten Vorrang, und zwar nur so weit wie noetig; eine
//! Last weicht nie einer gleich wichtigen oder unwichtigeren
//! ([`local::plan::Vorrang`]).
//!
//! # ⚑ Was hier noch nicht steht: das Netz
//!
//! Ein Modul `network` folgt, sobald es Code gibt. Geplant ist: Ein Knoten
//! kuendigt aus seinem Angebot an, welche Ebenen eines Modells er halten
//! kann und wie schnell er sie rechnet; die Verteilung der Shards ist dann
//! eine deterministische Funktion dieser Ankuendigungen, die jeder Knoten
//! gleich rechnet und die an einer Epochengrenze wechselt. Das ist eine
//! Aenderung am Protokoll und kommt deshalb nicht nebenbei.

#![deny(unsafe_code)]

pub mod local;
