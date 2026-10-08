//! **Module der Konsole**: was ein Modul ist, wie es sich ausweist und wie
//! es mit der Konsole spricht.
//!
//! Ein Modul ist ein Ordner mit einer Beschreibung (`modul.json`), einer
//! Signatur (`modul.sig`) und einem Programm. Die Konsole findet es, prueft
//! es, startet es und spricht mit ihm in Nachrichten, je eine JSON-Zeile
//! ([`nachricht`]). Ohne Modul baut und laeuft die Konsole wie ohne diese
//! Faehigkeit; ein Modul kann nur, was seine Beschreibung nennt und der
//! Mensch genehmigt hat.
//!
//! # ⚑ Das Bedrohungsmodell, kurz
//!
//! Gegen ein Modul, das unterwegs oder auf der Platte veraendert, auf eine
//! alte Fassung zurueckgesetzt oder mit fremden Dateien gemischt wurde,
//! schuetzt [`signatur`]: Signiert ist die Beschreibung samt SHA-256
//! **jeder** Datei, geprueft wird an den Bytes, die ausgefuehrt werden, und
//! eine kleinere Version als die zuletzt gesehene wird abgelehnt. Ohne
//! gueltige Signatur eines vertrauten Schluessels startet nichts.
//!
//! Gegen eine Konsole, die als Werkzeug missbraucht wird, schuetzt die
//! Konsole selbst: Sie zeichnet allein, Text eines Moduls kommt ohne
//! Steuerzeichen an ([`filter`]), und was ein Modul anfragt, ist durch
//! seine Befugnisse und Budgets begrenzt ([`beschreibung::Befugnisse`]).
//!
//! ⛔️ **Ein natives Modul ist keine Sicherheitsgrenze.** Es ist ein
//! Programm mit den Rechten des Nutzers. Deshalb startet ein natives Modul
//! nur mit einem Schluessel, der in der Vertrauensliste ausdruecklich als
//! nativ markiert ist; alle anderen laufen abgeschottet.

pub mod beschreibung;
pub mod filter;
pub mod gast;
pub mod nachricht;
pub mod signatur;
pub mod wirt;

/// **Die Fassung des Protokolls.** Konsole und Modul nennen sie beim
/// Handschlag; weichen sie ab, startet das Modul nicht.
pub const PROTOKOLL: u32 = 1;

/// Der Name der Beschreibung im Modulordner.
pub const BESCHREIBUNG: &str = "modul.json";

/// Der Name der Signatur im Modulordner.
pub const SIGNATUR: &str = "modul.sig";
