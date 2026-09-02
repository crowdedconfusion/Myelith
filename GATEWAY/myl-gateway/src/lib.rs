//! GATEWAY: die Tür zum Netz (Whitepaper Kap. 3.3, Rolle 6).
//!
//! # ⚑ Die Rolle stand im Papier und hatte keine Komponente (Fund 87)
//!
//! Kap. 3.3 führt Gateways als eine von sechs Rollen: Nutzeranfragen
//! entgegennehmen, an Pods geben, Ergebnisse zurückliefern. **Es gab
//! weder Komponente noch Code noch einen HTTP-Server im ganzen
//! Repositorium**, und zwei gebaute Schutzmechanismen hatten deshalb
//! keinen Betreiber.
//!
//! # Der Schnitt: Stufe 1
//!
//! Das eigene Gateway auf `localhost`. **Der Betreiber ist der
//! Kontoinhaber, also entfällt die ganze Bezahlfrage**, und mit ihr
//! Zugangsschlüsselverwaltung, Ratenbegrenzung und Missbrauchsschutz.
//! Was bleibt, ist die Tür und der Beleg, und der Beleg ist ohnehin das
//! Produkt.
//!
//! # ⚑ Keine HTTP-Bibliothek, und das ist eine Entscheidung
//!
//! Sie ist die größte neue Abhängigkeitsfläche seit `libp2p` und
//! gehört deshalb **vor** den ersten Code entschieden. Sie lautet:
//! keine. Was ein Rahmenwerk mitbrächte, ist Wegewahl und Mittelschicht
//! für Anforderungen, die Stufe 1 nicht hat; `axum` zöge `hyper` und
//! `tower` nach, drei Bäume für eine Tür mit einem Weg.
//!
//! ⚑ **Der Gewinn ist der Zeitpunkt:** Sobald Stufe 2 kommt, also ein
//! öffentliches Gateway mit TLS, Zugang und Ratenbegrenzung, ist die
//! Rahmenwerksfrage eine **echte** Frage mit echten Anforderungen. Wer
//! die Abhängigkeit für die Stufe nimmt, die sie nicht braucht, trifft
//! die Wahl im ungünstigsten Augenblick.
//!
//! # ⚑ Was diese Stufe **nicht** ist
//!
//! Sie nimmt entgegen und schreibt fest. **Sie gibt noch nichts an einen
//! Pod**, denn dafür braucht sie eine Sitzung im Netz, und die ist eigene
//! Arbeit. Diese Grenze steht hier, damit niemand den grünen Test für
//! mehr hält, als er sagt.

//! # Kein `unsafe`, und hier zaehlt es doppelt
//!
//! Die Regel gilt in Myelith ueberall, wo Protokoll gerechnet wird.
//! Diese Komponente ist zusaetzlich die **einzige nach aussen
//! gerichtete**: Was hier Bytes liest, liest sie von jemandem, der
//! nicht im Netz ist. Der Uebersetzer haelt die Regel, nicht der
//! Vorsatz.

#![deny(unsafe_code)]

pub mod annahme;
pub mod http;
pub mod tuer;

pub use annahme::{Annahme, Beleg, Annahmefehler};
pub use http::{antwort, kopf_lesen, Httpfehler, Kopf, MAX_KOPF, MAX_RUMPF};
pub use tuer::{Tuer, WEG};
