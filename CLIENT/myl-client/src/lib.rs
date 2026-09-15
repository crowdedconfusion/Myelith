//! Der Client: was ein Mensch bedient (CLIENT Phase 0).
//!
//! # ⚑ Was hier zusammenkommt, und warum es hier zusammenkommt
//!
//! Drei Teile lagen fertig und unverbunden: das quantisierte Modell,
//! der Ganzzahlpfad, der es rechnet, und das Harness, das einen Plan
//! abarbeitet. **Es fehlte die Verbindung und die Hand daran.**
//!
//! ⚑ **Und die Verbindung gehoert hierher und nicht ins Harness.**
//! `myl-local-agent` traegt eine Vollmacht und hat deshalb bewusst fast
//! keine Abhaengigkeiten, nicht einmal einen HTTP-Klienten. Das
//! Laufwerk dort einzuhaengen kehrte diese Entscheidung um. Das Merkmal
//! `Modellweg` laesst die Umsetzung draussen; **diese Kiste ist das
//! Draussen**.

/// Wie viel Maschine der Rechenpfad nehmen darf.
///
/// # ⚑ Warum das hier durchgereicht wird
///
/// Dieselbe Ueberlegung, aus der `integer-llm-runtime` die Naht zur
/// Kernkiste anbietet: **Ein Aufrufer soll die Kiste darunter nicht
/// kennen muessen.** Die Oberflaeche ruft `myl-client`, und wenn sie
/// dafuer `integer-llm-runtime` in ihr eigenes Manifest schreiben
/// muesste, waere die Naht keine.
pub mod kapazitaet {
    /// Setzt die Obergrenze der Kerne fuer diesen Prozess; `0` gibt die
    /// Maschine wieder frei.
    pub fn kerne_setzen(n: usize) {
        integer_llm_runtime::kapazitaet::kerne_setzen(n)
    }

    /// Wie viele Kerne der Rechenpfad gerade hoechstens nimmt.
    pub fn kerne() -> usize {
        integer_llm_runtime::kapazitaet::kerne()
    }
}

pub mod aktualisierung;
pub mod einstellungen;
pub mod gespraech;
pub mod hardware;
pub mod lauf;
pub mod markdown;
pub mod oertlich;
pub mod ort;
pub mod reservierung;
pub mod strom;
pub mod ruestung;
pub mod werkzeuge;
pub mod kisten;

/// ⚑ **Weitergereicht, damit die Oberflaeche nicht an `myl-local-agent`
/// haengen muss.** Sie braucht die Form nur, um `ruesten` zu rufen; eine
/// eigene Abhaengigkeit dafuer waere eine Kante im Graphen fuer eine
/// Aufzaehlung mit zwei Werten.
pub use myl_local_agent::werkzeug::Ansageform;

/// ⚑ **Ebenfalls weitergereicht, aus demselben Grund.** Wer ein Modell
/// direkt fragen will, braucht die Nachrichtenform und das Merkmal, das
/// `chat` traegt. Die Oberflaeche haengt dafuer nicht an
/// `myl-local-agent`: Jene Kiste traegt eine Vollmacht und haelt ihre
/// Angriffsflaeche klein, und je weniger Stellen sie einbinden, desto
/// weniger Stellen koennen sie falsch benutzen.
pub use myl_local_agent::{Modellweg, Nachricht, Tuerfehler};
/// Warum ein Lauf endete; die Konsole nennt bei vollem Kontext den Ausweg.
pub use myl_local_agent::schleife::Ende;

/// ⚑ **Ebenfalls weitergereicht.** Wer einem Lauf zusehen will, braucht
/// die Form der Meldung; eine eigene Abhaengigkeit auf die
/// Vollmachtskiste dafuer waere eine Kante im Graphen fuer eine
/// Aufzaehlung mit vier Werten.
pub use myl_local_agent::schleife::Meldung;

/// ⚑ **Weitergereicht, damit die Oberflaechen keine eigene Fassung
/// aufnehmen.** Wer eine Nachfrage baut, braucht den Typ der
/// Werkzeugargumente; zwei Kisten mit zwei Fassungen von `serde_json`
/// waeren zwei `Value`, die einander nicht kennen.
pub mod modelle;

pub use serde_json;

pub use einstellungen::Einstellungen;
pub use oertlich::Oertlichesmodell;
