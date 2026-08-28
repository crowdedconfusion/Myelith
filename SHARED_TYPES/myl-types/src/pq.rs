//! Der Schalter für den Wechsel des Signaturverfahrens.
//!
//! ## Warum es diese Datei gibt, bevor es ein zweites Verfahren gibt
//!
//! Shors Algorithmus bricht BLS12-381, und ein aggregierbares
//! Post-Quantum-Signaturverfahren ist nicht standardisiert. Der Wechsel
//! steht deshalb nicht an. **Die Vorbereitung dafür schon**, und zwar
//! aus einem Grund, der nichts mit Quantenrechnern zu tun hat:
//!
//! ⚑ **Ein Schalter funktioniert nur, wenn alle Validatoren ihren neuen
//! Schlüssel vorher veröffentlicht haben.** Solange der Validator-Satz
//! kein Feld dafür hat, kann niemand anfangen. Das Feld vor dem
//! Genesis-Block einzuziehen ist eine Zeile; danach ist es eine
//! Kettenmigration. Dieselbe Klasse wie Fund 77: eine Lücke im
//! Konsensformat, deren Behebung mit jedem Betriebstag teurer wird,
//! ohne dass jemand etwas falsch macht.
//!
//! ## Was hier steht und was nicht
//!
//! **Hier steht das Format und der Schalter.** Nicht hier steht die
//! Prüfung einer Post-Quantum-Signatur: Welches Verfahren es einmal
//! wird, ist offen, und ein Verfahren einzubauen, das später gegen ein
//! anderes getauscht wird, wäre Arbeit gegen die eigene Annahme. Die
//! Prüfung schlägt deshalb **fehl**, statt zu behaupten, sie gälte.
//!
//! ## ⚑ Drei Stufen, nicht zwei
//!
//! Der naheliegende Entwurf hat einen Schalter mit zwei Stellungen, und
//! er ist falsch. Ein Sprung von „nur klassisch" auf „nur
//! quantensicher" macht **jeden Validator ungültig**, der seinen
//! zweiten Schlüssel noch nicht veröffentlicht hat, und hält damit die
//! Kette an. Es braucht ein Fenster, in dem beide gelten:
//!
//! ```text
//! NurKlassisch  →  Beide  →  NurQuantensicher
//! ```
//!
//! **Und die Folge ist einbahnig.** Ein Rückschritt von `Beide` auf
//! `NurKlassisch` wäre harmlos; einer von `NurQuantensicher` zurück
//! öffnet das gebrochene Verfahren wieder, und genau dann, wenn jemand
//! es gebrochen hat. Die Governance-Invariante lässt deshalb nur
//! Schritte nach vorn und nur einen auf einmal.
//!
//! ⚑ **Das Fenster `Beide` ist die verwundbarste Stellung**, und das
//! ist unvermeidlich: Wer das klassische Verfahren gebrochen hat, kann
//! darin fälschen. Es gehört deshalb so kurz wie möglich gehalten, und
//! der Übergang nach `NurQuantensicher` gehört **nicht** an eine Frist,
//! sondern an eine Bedingung: dass alle Validatoren bereit sind. Die
//! Bedingung prüft der Konsens, nicht die Registry, denn nur er kennt
//! den Validator-Satz.

use borsh::{BorshDeserialize, BorshSerialize};

/// Welches Signaturverfahren eine Nachricht trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize)]
pub enum Signaturverfahren {
    /// BLS12-381 mit Proof-of-Possession. Das heutige Verfahren, und das
    /// einzige aggregierbare.
    Bls12_381,
    /// ML-DSA-65 (FIPS 204). **Format vorgesehen, Prüfung nicht
    /// gebaut.** Die Wahl ist nicht getroffen; die Variante steht hier,
    /// damit der Schalter eine zweite Stellung hat und die Längen
    /// jemandem auffallen.
    MlDsa65,
}

impl Signaturverfahren {
    /// Länge eines öffentlichen Schlüssels in Byte.
    pub fn pubkey_len(&self) -> usize {
        match self {
            Self::Bls12_381 => 48,
            Self::MlDsa65 => 1952,
        }
    }

    /// Länge einer Signatur in Byte.
    ///
    /// ⚑ **Der Grund, warum der Wechsel nicht ansteht, steht in dieser
    /// Zahl.** Ein Polka-Zertifikat aggregiert heute beliebig viele
    /// Signaturen auf 96 Byte. ML-DSA aggregiert nicht: Bei 21
    /// Validatoren sind es 21 · 3 309 Byte, also rund 68 KB, in jedem
    /// Rundenwechsel.
    pub fn signatur_len(&self) -> usize {
        match self {
            Self::Bls12_381 => 96,
            Self::MlDsa65 => 3309,
        }
    }

    /// Hält es einem Quantenrechner stand?
    pub fn ist_quantensicher(&self) -> bool {
        matches!(self, Self::MlDsa65)
    }

    /// Lassen sich viele Signaturen zu einer zusammenfassen?
    ///
    /// **Genau eine Antwort ist heute `true`, und das ist der Blocker.**
    pub fn aggregierbar(&self) -> bool {
        matches!(self, Self::Bls12_381)
    }
}

/// Ein öffentlicher Schlüssel mit seinem Verfahren.
///
/// Variabler Länge, weil die Verfahren verschieden lange Schlüssel
/// haben. Die Länge wird beim Anlegen gegen das Verfahren geprüft:
/// Ein Schlüssel falscher Länge ist kein Schlüssel, und er soll hier
/// auffallen und nicht erst bei der Prüfung.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PqPublicKey {
    verfahren: Signaturverfahren,
    bytes: Vec<u8>,
}

/// Warum ein Schlüssel oder eine Signatur nicht angenommen wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PqFehler {
    /// Die Bytelänge passt nicht zum Verfahren.
    LaengePasstNicht {
        verfahren: Signaturverfahren,
        erwartet: usize,
        bekommen: usize,
    },
    /// Das Verfahren ist in der geltenden Stufe nicht zugelassen.
    VerfahrenNichtZugelassen {
        verfahren: Signaturverfahren,
        stufe: Signaturstufe,
    },
    /// Für dieses Verfahren gibt es noch keine Prüfung.
    PruefungNichtGebaut { verfahren: Signaturverfahren },
}

impl std::fmt::Display for PqFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaengePasstNicht {
                verfahren,
                erwartet,
                bekommen,
            } => write!(
                f,
                "{:?} erwartet {} Byte, bekommen {}",
                verfahren, erwartet, bekommen
            ),
            Self::VerfahrenNichtZugelassen { verfahren, stufe } => write!(
                f,
                "{:?} ist in Stufe {:?} nicht zugelassen",
                verfahren, stufe
            ),
            Self::PruefungNichtGebaut { verfahren } => write!(
                f,
                "für {:?} gibt es noch keine Prüfung; das Format steht, das Verfahren nicht",
                verfahren
            ),
        }
    }
}

impl std::error::Error for PqFehler {}

impl PqPublicKey {
    /// Aus Verfahren und Bytes, mit Längenprüfung.
    pub fn neu(verfahren: Signaturverfahren, bytes: Vec<u8>) -> Result<Self, PqFehler> {
        let erwartet = verfahren.pubkey_len();
        if bytes.len() != erwartet {
            return Err(PqFehler::LaengePasstNicht {
                verfahren,
                erwartet,
                bekommen: bytes.len(),
            });
        }
        Ok(Self { verfahren, bytes })
    }

    /// Das Verfahren.
    pub fn verfahren(&self) -> Signaturverfahren {
        self.verfahren
    }

    /// Die rohen Bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Eine Signatur mit ihrem Verfahren.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PqSignature {
    verfahren: Signaturverfahren,
    bytes: Vec<u8>,
}

impl PqSignature {
    /// Aus Verfahren und Bytes, mit Längenprüfung.
    pub fn neu(verfahren: Signaturverfahren, bytes: Vec<u8>) -> Result<Self, PqFehler> {
        let erwartet = verfahren.signatur_len();
        if bytes.len() != erwartet {
            return Err(PqFehler::LaengePasstNicht {
                verfahren,
                erwartet,
                bekommen: bytes.len(),
            });
        }
        Ok(Self { verfahren, bytes })
    }

    /// Das Verfahren.
    pub fn verfahren(&self) -> Signaturverfahren {
        self.verfahren
    }

    /// Die rohen Bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Welche Verfahren gerade gelten.
///
/// Die Reihenfolge der Varianten **ist** die zulässige Reihenfolge des
/// Übergangs; `as u64` gibt den Wert, der in der Governance-Registry
/// steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize)]
pub enum Signaturstufe {
    /// Nur BLS12-381. Der heutige Zustand.
    NurKlassisch,
    /// Beide gelten. Das Übergangsfenster, und die verwundbarste
    /// Stellung: Wer BLS gebrochen hat, kann darin fälschen.
    Beide,
    /// Nur das quantensichere Verfahren. Erst zulässig, wenn **alle**
    /// Validatoren einen solchen Schlüssel veröffentlicht haben.
    NurQuantensicher,
}

impl Signaturstufe {
    /// Aus der Zahl in der Registry.
    pub fn aus_zahl(n: u64) -> Option<Self> {
        match n {
            0 => Some(Self::NurKlassisch),
            1 => Some(Self::Beide),
            2 => Some(Self::NurQuantensicher),
            _ => None,
        }
    }

    /// Die Zahl für die Registry.
    pub fn zahl(&self) -> u64 {
        match self {
            Self::NurKlassisch => 0,
            Self::Beide => 1,
            Self::NurQuantensicher => 2,
        }
    }

    /// Gilt dieses Verfahren gerade?
    pub fn akzeptiert(&self, verfahren: Signaturverfahren) -> bool {
        match self {
            Self::NurKlassisch => !verfahren.ist_quantensicher(),
            Self::Beide => true,
            Self::NurQuantensicher => verfahren.ist_quantensicher(),
        }
    }

    /// ⚑ **Die Kernregel des Schalters: ein Schritt nach vorn, sonst
    /// nichts.**
    ///
    /// - **Kein Sprung.** `NurKlassisch` direkt auf `NurQuantensicher`
    ///   macht jeden Validator ungültig, der noch keinen zweiten
    ///   Schlüssel hat, und hält die Kette an.
    /// - **Kein Rückschritt.** Von `NurQuantensicher` zurück öffnet das
    ///   gebrochene Verfahren wieder, und zwar genau dann, wenn jemand
    ///   es gebrochen hat. Der Rückweg wäre der Angriff.
    /// - **Kein Stillstand als Änderung.** Gleich auf gleich ist keine
    ///   Änderung und wird als solche zurückgewiesen, damit ein
    ///   wirkungsloser Vorschlag nicht als Übergang durchgeht.
    pub fn uebergang_erlaubt(von: Self, nach: Self) -> bool {
        nach.zahl() == von.zahl() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Die Kernregel des Schalters, über alle neun Paare.**
    ///
    /// Nicht drei Beispiele, sondern die volle Tabelle: Genau zwei
    /// Übergänge sind erlaubt, die anderen sieben nicht. Ein Test über
    /// Beispiele ließe offen, ob der Rückweg wirklich zu ist.
    #[test]
    fn nur_ein_schritt_nach_vorn_ist_erlaubt() {
        use Signaturstufe::*;
        let alle = [NurKlassisch, Beide, NurQuantensicher];
        let mut erlaubt = Vec::new();
        for von in alle {
            for nach in alle {
                if Signaturstufe::uebergang_erlaubt(von, nach) {
                    erlaubt.push((von, nach));
                }
            }
        }
        assert_eq!(
            erlaubt,
            vec![(NurKlassisch, Beide), (Beide, NurQuantensicher)],
            "die erlaubten Uebergaenge stimmen nicht"
        );
    }

    /// Der Sprung über das Fenster hinweg ist gesperrt, und das ist der
    /// Fall, der die Kette anhalten würde.
    #[test]
    fn der_sprung_ueber_das_fenster_ist_gesperrt() {
        assert!(!Signaturstufe::uebergang_erlaubt(
            Signaturstufe::NurKlassisch,
            Signaturstufe::NurQuantensicher
        ));
    }

    /// Der Rückweg ist gesperrt, und er wäre der Angriff: Wer das
    /// klassische Verfahren gebrochen hat, will genau ihn.
    #[test]
    fn der_rueckweg_ist_gesperrt() {
        assert!(!Signaturstufe::uebergang_erlaubt(
            Signaturstufe::NurQuantensicher,
            Signaturstufe::Beide
        ));
        assert!(!Signaturstufe::uebergang_erlaubt(
            Signaturstufe::Beide,
            Signaturstufe::NurKlassisch
        ));
    }

    /// Welche Stufe welches Verfahren annimmt, vollständig.
    #[test]
    fn jede_stufe_nimmt_genau_das_an_was_sie_soll() {
        use Signaturstufe::*;
        use Signaturverfahren::*;
        assert!(NurKlassisch.akzeptiert(Bls12_381));
        assert!(!NurKlassisch.akzeptiert(MlDsa65));
        assert!(Beide.akzeptiert(Bls12_381));
        assert!(Beide.akzeptiert(MlDsa65));
        assert!(!NurQuantensicher.akzeptiert(Bls12_381));
        assert!(NurQuantensicher.akzeptiert(MlDsa65));
    }

    /// Die Zahl in der Registry und die Stufe gehören zusammen, in
    /// beide Richtungen. Ohne den Rundgang könnte die Registry eine
    /// Stufe führen, die niemand lesen kann.
    #[test]
    fn zahl_und_stufe_passen_in_beide_richtungen() {
        use Signaturstufe::*;
        for s in [NurKlassisch, Beide, NurQuantensicher] {
            assert_eq!(Signaturstufe::aus_zahl(s.zahl()), Some(s));
        }
        assert_eq!(Signaturstufe::aus_zahl(3), None, "3 ist keine Stufe");
        assert_eq!(Signaturstufe::aus_zahl(u64::MAX), None);
    }

    /// Ein Schlüssel oder eine Signatur falscher Länge wird beim Anlegen
    /// abgewiesen, nicht erst bei der Prüfung.
    #[test]
    fn falsche_laengen_fallen_beim_anlegen_auf() {
        use Signaturverfahren::*;
        assert!(PqPublicKey::neu(Bls12_381, vec![0; 48]).is_ok());
        assert!(PqPublicKey::neu(Bls12_381, vec![0; 47]).is_err());
        assert!(PqPublicKey::neu(MlDsa65, vec![0; 1952]).is_ok());
        assert!(PqPublicKey::neu(MlDsa65, vec![0; 1951]).is_err());
        assert!(PqSignature::neu(Bls12_381, vec![0; 96]).is_ok());
        assert!(PqSignature::neu(MlDsa65, vec![0; 3309]).is_ok());
        assert!(PqSignature::neu(MlDsa65, vec![0; 96]).is_err());
    }

    /// ⚑ Der Grund, warum der Wechsel nicht ansteht, als Zahl.
    ///
    /// Genau ein Verfahren aggregiert. Ohne diesen Test stünde die
    /// Aussage nur im Fließtext, und ein späterer Zusatz eines
    /// aggregierbaren Verfahrens fiele nirgends auf.
    #[test]
    fn genau_ein_verfahren_aggregiert_und_es_ist_das_klassische() {
        use Signaturverfahren::*;
        assert!(Bls12_381.aggregierbar() && !Bls12_381.ist_quantensicher());
        assert!(!MlDsa65.aggregierbar() && MlDsa65.ist_quantensicher());
        // Und die Kosten: 21 Validatoren, einmal aggregiert gegen
        // einzeln.
        let n = 21;
        assert_eq!(Bls12_381.signatur_len(), 96, "ein Aggregat, gleich wie viele");
        assert_eq!(n * MlDsa65.signatur_len(), 69_489, "21 einzelne");
    }

    #[test]
    fn schluessel_und_signatur_ueberleben_borsh() {
        let k = PqPublicKey::neu(Signaturverfahren::MlDsa65, vec![7; 1952]).expect("Schlüssel");
        let roh = borsh::to_vec(&k).expect("serialisieren");
        assert_eq!(borsh::from_slice::<PqPublicKey>(&roh).expect("lesen"), k);

        let s = PqSignature::neu(Signaturverfahren::Bls12_381, vec![3; 96]).expect("Signatur");
        let roh = borsh::to_vec(&s).expect("serialisieren");
        assert_eq!(borsh::from_slice::<PqSignature>(&roh).expect("lesen"), s);
    }
}
