//! Spur-Hashes und Übergangs-Signaturen (Anhang A.3, Schritte 2+4).
//!
//! Jeder Shard hasht seine Ausgabe-Aktivierungen (`activation_hash`) und
//! signiert den Übergang `(segment_id, vorheriger Spur-Hash, neuer
//! Spur-Hash)` mit seinem BLS-Schlüssel (`TransitionSig`). Der nächste
//! Shard prüft den Hash der empfangenen Aktivierungen gegen den letzten
//! Spur-Eintrag — das ist die Manipulationserkennung.
//!
//! Alle Operationen sind deterministisch: derselbe Input ergibt denselben
//! Hash und dieselbe Signatur (BLS ohne Zufallsdaten).


/// Der Übergangs-Signaturvertrag liegt seit dem 2026-08-29 in
/// `myl_types::uebergang`, damit die **Schiedsstelle** ihn lesen kann,
/// ohne an dieses Crate und damit an die ganze Inferenz-Laufzeit zu
/// hängen. Hier steht er weiter zur Verfügung, denn dies ist die Stelle,
/// an der er benutzt wird.
pub use myl_types::uebergang::{Rolle, TransitionSig, DST_SHARD_TRANSITION};

/// Der Spur-Eintrag liegt seit dem 2026-09-01 in
/// `myl_types::uebergang`, aus demselben Grund wie `TransitionSig`: Der
/// **Checker** muss ihn nachrechnen können, ohne an dieses Crate und
/// damit an die Beweiserseite zu hängen. Hier steht er weiter zur
/// Verfügung, denn dies ist die Stelle, an der er erzeugt wird.
pub use myl_types::uebergang::activation_hash;


/// Null-Hash (vorheriger Spur-Eintrag für Shard 0).
pub const ZERO_HASH: [u8; 32] = [0u8; 32];


/// Prüft, dass der Hash der empfangenen Aktivierungen zum letzten
/// Spur-Eintrag passt (Manipulationserkennung). Liefert `true`, wenn die
/// Spur leer ist (Shard 0 empfängt Token, keine Aktivierungen) oder der
/// Hash übereinstimmt.
pub fn verify_input_hash(activations: &[i16], trace: &[[u8; 32]]) -> bool {
    match trace.last() {
        // **Eine leere Spur belegt nichts (Fund 41, 2026-08-23).**
        //
        // Bis dahin stand hier `true`, mit der Begründung „Shard 0: noch
        // kein Spur-Eintrag, Token-Eingang". Für Shard 0 stimmt das,
        // aber dieser Zweig wird von dort **gar nicht erreicht**: Der
        // Token-Eingang kehrt in `ShardNode::process` vorher zurück.
        //
        // Erreicht wird er nur auf dem Aktivierungspfad, und dort heißt
        // eine leere Spur: Jemand schickt Aktivierungen ohne jeden
        // Nachweis, woher sie kommen. Die Prüfung ging **vacuously**
        // durch, der Shard rechnete auf fremden Zahlen weiter, und bei
        // unpassender Länge endete das in einer Panik im Kernel, also in
        // einem Absturz, den jeder auslösen kann, der Bytes schicken
        // darf.
        //
        // Gefunden vom adversarialen Test, beim ersten Lauf.
        None => false,
        Some(expected) => activation_hash(activations) == *expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_hash_deterministisch_und_empfindlich() {
        let a = [1i16, 2, 3, -4, 32767, -32768];
        let h1 = activation_hash(&a);
        let h2 = activation_hash(&a);
        assert_eq!(h1, h2);
        // Ein verändertes Byte ⇒ anderer Hash.
        let mut b = a;
        b[3] ^= 1;
        assert_ne!(activation_hash(&b), h1);
        // Leer ⇒ definierter Hash.
        let leer = activation_hash(&[]);
        assert_eq!(leer, activation_hash(&[]));
    }

    #[test]
    fn eingangs_hash_pruefung() {
        let akt = [10i16, 20, 30];
        let h = activation_hash(&akt);
        // Passender Spur-Eintrag ⇒ ok.
        assert!(verify_input_hash(&akt, &[h]));
        // Leere Spur (Shard 0) ⇒ ok.
        // Fund 41: Eine leere Spur belegt nichts. Auf dem
        // Aktivierungspfad, dem einzigen, der hier ankommt, heisst sie
        // "ohne Nachweis geschickt".
        assert!(!verify_input_hash(&akt, &[]));
        // Verfälschte Aktivierungen ⇒ abgelehnt.
        let mut manipuliert = akt;
        manipuliert[0] = 99;
        assert!(!verify_input_hash(&manipuliert, &[h]));
        // Falscher Spur-Eintrag ⇒ abgelehnt.
        assert!(!verify_input_hash(&akt, &[[0xAAu8; 32]]));
    }
}

