//! Die Kapazitätszusage: was ein Knoten zu halten anbietet.
//!
//! # Wozu, und warum sie mehr ist als Bequemlichkeit
//!
//! Der Wunsch war ein Schalter, mit dem ein Miner Hardware zu- und
//! abschaltet (Festlegung des Projektinhabers, 2026-08-30). **Der
//! Schalter ist die Eingabe, aus der sich ergibt, wie groß die
//! Wissensdatenbank überhaupt werden darf.**
//!
//! Die Last je speicherndem Knoten ist `W · f / N`, und nichts bindet
//! den Umfang der Daten an die Zahl der Knoten: W wächst mit der
//! Nutzung, N mit dem Ertrag. Ohne Schranke wächst W, bis Knoten gehen,
//! und dann fällt N. **Die Summe der Zusagen ist die Schranke**, gegen
//! die eine Aufnahme geprüft wird.
//!
//! Zugleich ist sie die Voraussetzung für die Zuteilung: Wem das
//! Protokoll ein Teil zuweist, muss vorher gesagt haben, dass er Platz
//! hat.
//!
//! # ⚑ Sie gilt ab der nächsten Epoche, nie ab sofort
//!
//! [`Kapazitaetszusage::ab_epoche`] muss **echt größer** als die
//! laufende Epoche sein. Damit ist „mitten im Auftrag abschalten" durch
//! Konstruktion ausgeschlossen: Wer abschalten will, sagt für die
//! nächste Epoche weniger zu, und bis dahin gilt, was er zugesagt hat.
//! Ein Rückzug innerhalb der laufenden Epoche bleibt ein Ausfall und
//! wird als solcher behandelt.
//!
//! Nebenbei erledigt diese Regel den Wiedereinspielungsangriff: Eine
//! alte, höhere Zusage trägt ihre eigene Epoche im signierten Teil und
//! ist damit für jede spätere Epoche wertlos.
//!
//! # ⚑ Null ist erlaubt, und das ist keine Nachlässigkeit
//!
//! Eine Zusage über null Bytes ist die **Abmeldung**. Wer sie verböte,
//! machte den Beitritt zu einer Falle: Ein Knoten käme aus der
//! Speicherpflicht nur noch durch Verschwinden, und Verschwinden ist im
//! Protokoll ein Ausfall mit Folgen. Der geordnete Ausstieg muss
//! ausdrücklich möglich sein.
//!
//! # ⚑ Warum nur Speicher, obwohl der Schalter vier Größen nennt
//!
//! CPU, GPU und Arbeitsspeicher stehen nicht darin, und das ist eine
//! Entscheidung, keine Auslassung.
//!
//! - **CPU und GPU** sind über die verifizierte Arbeit bereits bezahlt,
//!   und zwar nachgewiesen. Ein zweites Feld dafür bekäme im Protokoll
//!   heute keinen Leser.
//! - **Arbeitsspeicher** ist Voraussetzung, keine eigene Größe: Wer zu
//!   wenig hat, hält seine Zuteilung nicht und verliert sie. Das ist
//!   bereits eingepreist.
//!
//! **Ein Feld, das niemand liest, ist in diesem Repositorium ein
//! benannter Fehler** (Fund 98: `--upstream` wurde gesetzt,
//! durchgereicht und beworben, und gelesen hat es nie jemand). Sobald
//! die Zuteilung Rechenkapazität wirklich auswertet, gehören die Felder
//! dazu; vorher nicht.
//!
//! # Was sie nicht ist
//!
//! **Keine Zusicherung, sondern eine Obergrenze.** Zugesagt heißt: bis
//! hierhin darf zugeteilt werden. Bezahlt wird davon nichts; bezahlt
//! wird, was nachgewiesen ist. Wer viel zusagt und nichts hält, verdient
//! nichts, verzerrt aber das Budget des Netzes. **Deshalb gehört das
//! Budget aus der nachgewiesenen Kapazität berechnet und nicht aus der
//! zugesagten**, und deshalb steht unten eine Obergrenze, die grobe
//! Vertipper abfängt.

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::ids::{EpochId, MinerId};
use crate::uebergang::Rolle;
use borsh::{BorshDeserialize, BorshSerialize};

/// Trennstring der Kapazitätszusage.
pub const DST_KAPAZITAETSZUSAGE: &[u8] = b"MYELITH_KAPAZITAETSZUSAGE_v1";

/// Obergrenze einer einzelnen Zusage: ein Pebibyte.
///
/// **Gegen Vertipper und Überlauf, nicht gegen Angreifer.** Wer
/// absichtlich zu viel zusagt, verdient daran nichts; die Schranke soll
/// verhindern, dass eine verrutschte Einheit die Budgetrechnung des
/// ganzen Netzes über den Haufen wirft. Ein Pebibyte je Knoten ist weit
/// jenseits dessen, was ein einzelner Halter je beiträgt, und weit
/// diesseits eines Überlaufs.
pub const HOECHSTZUSAGE_BYTES: u64 = 1 << 50;

/// Was ein Knoten für eine Epoche und die folgenden zu halten anbietet.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kapazitaetszusage {
    /// Wer zusagt.
    pub halter: MinerId,
    /// Ab welcher Epoche sie gilt. Echt größer als die laufende.
    pub ab_epoche: EpochId,
    /// Angebotener Speicher in Bytes. Null ist die Abmeldung.
    pub speicher_bytes: u64,
    /// Unterschrift des Halters in der Rolle [`Rolle::Store`].
    pub signature: BlsSignature,
}

/// Warum eine Zusage nicht angenommen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zusagefehler {
    /// Sie gilt ab der laufenden Epoche oder früher.
    ///
    /// Eine Zusage wirkt erst zur nächsten Epochengrenze; alles andere
    /// wäre ein Rückzug mitten im Auftrag.
    NichtInDerZukunft {
        /// Die genannte Epoche.
        ab_epoche: EpochId,
        /// Die laufende.
        jetzt: EpochId,
    },
    /// Mehr als [`HOECHSTZUSAGE_BYTES`].
    ZuGross {
        /// Der genannte Wert.
        bytes: u64,
    },
    /// Die Unterschrift stimmt nicht, oder sie stammt von einem anderen.
    Unterschrift,
}

impl Kapazitaetszusage {
    /// Eine unsignierte Zusage. [`Self::signiere`] gehört dazu.
    pub fn neu(halter: MinerId, ab_epoche: EpochId, speicher_bytes: u64) -> Self {
        Self {
            halter,
            ab_epoche,
            speicher_bytes,
            signature: BlsSignature([0u8; crate::bls::BLS_SIG_LEN]),
        }
    }

    /// Die Bytes, über die unterschrieben wird.
    ///
    /// Trennstring, Rollenbyte, dann der Kern. **Die Rolle wird
    /// mitsigniert**, damit eine Zusage in keiner anderen Rolle gilt.
    pub fn signierbotschaft(&self) -> Vec<u8> {
        let kern = (self.halter, self.ab_epoche, self.speicher_bytes);
        let rumpf = borsh::to_vec(&kern).expect("feste Feldbreiten sind stets serialisierbar");
        let mut msg =
            Vec::with_capacity(DST_KAPAZITAETSZUSAGE.len() + 1 + rumpf.len());
        msg.extend_from_slice(DST_KAPAZITAETSZUSAGE);
        msg.push(Rolle::Store.byte());
        msg.extend_from_slice(&rumpf);
        msg
    }

    /// Unterschreibt die Zusage.
    pub fn signiere(&mut self, sk: &BlsSecretKey) -> Result<(), crate::bls::BlsError> {
        self.signature = sk.sign(&self.signierbotschaft())?;
        Ok(())
    }

    /// Stammt sie von dem, der darin steht?
    ///
    /// ⚑ **Zwei Fragen in einem Schritt**, nach dem Muster von Fund 96:
    /// Die Unterschrift muss stimmen **und** der Schlüssel muss zu
    /// [`Self::halter`] gehören. Nur das erste zu prüfen hieße, jeden
    /// Beliebigen im Namen eines anderen zusagen zu lassen.
    pub fn ist_vom_halter(&self, pk: &BlsPublicKey) -> bool {
        MinerId::aus_schluessel(pk) == self.halter
            && pk.verify(&self.signierbotschaft(), &self.signature)
    }

    /// Prüft Form und Unterschrift gegen die laufende Epoche.
    pub fn pruefen(
        &self,
        pk: &BlsPublicKey,
        jetzt: EpochId,
    ) -> Result<(), Zusagefehler> {
        if self.ab_epoche <= jetzt {
            return Err(Zusagefehler::NichtInDerZukunft {
                ab_epoche: self.ab_epoche,
                jetzt,
            });
        }
        if self.speicher_bytes > HOECHSTZUSAGE_BYTES {
            return Err(Zusagefehler::ZuGross {
                bytes: self.speicher_bytes,
            });
        }
        if !self.ist_vom_halter(pk) {
            return Err(Zusagefehler::Unterschrift);
        }
        Ok(())
    }

    /// Ist das die Abmeldung?
    pub fn ist_abmeldung(&self) -> bool {
        self.speicher_bytes == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel")
    }

    /// Zusage samt passendem Schlüsselpaar.
    fn zusage(b: u8, ab: u64, bytes: u64) -> (Kapazitaetszusage, BlsPublicKey) {
        let sk = schluessel(b);
        let pk = sk.public_key().expect("Öffentlicher Schlüssel");
        let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(ab), bytes);
        z.signiere(&sk).expect("signieren");
        (z, pk)
    }

    #[test]
    fn signieren_und_pruefen() {
        let (z, pk) = zusage(1, 11, 2 * 1024 * 1024 * 1024 * 1024);
        assert_eq!(z.pruefen(&pk, EpochId(10)), Ok(()));
        assert!(z.ist_vom_halter(&pk));
    }

    /// ⚑ **Sie gilt ab der nächsten Epoche, nie ab sofort.**
    ///
    /// Damit ist „mitten im Auftrag abschalten" durch Konstruktion
    /// ausgeschlossen. Der Test fährt beide Seiten der Grenze ab, sonst
    /// bliebe offen, ob die Prüfung die Epoche beurteilt oder alles
    /// ablehnt.
    #[test]
    fn eine_zusage_gilt_erst_ab_der_naechsten_epoche() {
        for (ab, erlaubt) in [(9u64, false), (10, false), (11, true), (99, true)] {
            let (z, pk) = zusage(2, ab, 1024);
            let r = z.pruefen(&pk, EpochId(10));
            assert_eq!(
                r.is_ok(),
                erlaubt,
                "ab_epoche {ab} bei laufender 10: {r:?}"
            );
        }
    }

    /// ⚑ **Null ist die Abmeldung, kein Fehler.**
    ///
    /// Wer sie verböte, machte den Beitritt zu einer Falle: Ein Knoten
    /// käme aus der Speicherpflicht nur noch durch Verschwinden, und
    /// Verschwinden ist im Protokoll ein Ausfall mit Folgen.
    #[test]
    fn null_ist_die_abmeldung_und_kein_fehler() {
        let (z, pk) = zusage(3, 11, 0);
        assert_eq!(z.pruefen(&pk, EpochId(10)), Ok(()));
        assert!(z.ist_abmeldung());
    }

    /// ⚑ **Fund-96-Muster: Unterschrift **und** Identität.**
    ///
    /// Hier unterschreibt Schlüssel A eine Zusage, die B als Halter
    /// nennt. Die Unterschrift ist echt; sie gehört nur nicht zu dem,
    /// der darin steht. Wer nur die Unterschrift prüft, lässt jeden
    /// Beliebigen im Namen eines anderen zusagen.
    #[test]
    fn eine_zusage_im_namen_eines_anderen_gilt_nicht() {
        let sk_a = schluessel(4);
        let pk_a = sk_a.public_key().expect("pk");
        let pk_b = schluessel(5).public_key().expect("pk");

        let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk_b), EpochId(11), 4096);
        z.signiere(&sk_a).expect("signieren");

        // Die Unterschrift selbst ist gültig, gegen A geprüft.
        assert!(pk_a.verify(&z.signierbotschaft(), &z.signature));
        // Sie trägt die Zusage trotzdem nicht.
        assert!(!z.ist_vom_halter(&pk_a), "fremder Halter kam durch");
        assert!(!z.ist_vom_halter(&pk_b), "fremde Unterschrift kam durch");
        assert_eq!(z.pruefen(&pk_b, EpochId(10)), Err(Zusagefehler::Unterschrift));
    }

    /// ⚑ **Eine alte, höhere Zusage lässt sich nicht wiedereinspielen.**
    ///
    /// Sie trägt ihre Epoche im signierten Teil. Wer sie später erneut
    /// einreicht, reicht eine Zusage für eine vergangene Epoche ein, und
    /// die gilt nicht.
    #[test]
    fn eine_alte_zusage_laesst_sich_nicht_wiedereinspielen() {
        let (alt, pk) = zusage(6, 5, 999_999_999);
        assert_eq!(alt.pruefen(&pk, EpochId(4)), Ok(()));
        assert_eq!(
            alt.pruefen(&pk, EpochId(20)),
            Err(Zusagefehler::NichtInDerZukunft {
                ab_epoche: EpochId(5),
                jetzt: EpochId(20),
            })
        );
    }

    /// ⚑ **Die Rolle wird mitsigniert.**
    ///
    /// Dieselben Kernbytes mit einem anderen Rollenbyte ergeben eine
    /// andere Botschaft. Eine in einer anderen Rolle abgegebene
    /// Unterschrift gilt hier nicht, und zwar durch Konstruktion.
    #[test]
    fn eine_unterschrift_aus_einer_anderen_rolle_gilt_nicht() {
        let sk = schluessel(7);
        let pk = sk.public_key().expect("pk");
        let z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(11), 4096);

        // Dieselben Kernbytes, aber als Checker unterschrieben.
        let mut fremd = z.signierbotschaft();
        let stelle = DST_KAPAZITAETSZUSAGE.len();
        assert_eq!(fremd[stelle], Rolle::Store.byte());
        fremd[stelle] = Rolle::Checker.byte();

        let mut z2 = z.clone();
        z2.signature = sk.sign(&fremd).expect("signieren");
        assert!(
            !z2.ist_vom_halter(&pk),
            "eine Checker-Unterschrift trug eine Speicherzusage"
        );
    }

    #[test]
    fn eine_masslose_zusage_wird_abgelehnt() {
        let (z, pk) = zusage(8, 11, HOECHSTZUSAGE_BYTES + 1);
        assert_eq!(
            z.pruefen(&pk, EpochId(10)),
            Err(Zusagefehler::ZuGross {
                bytes: HOECHSTZUSAGE_BYTES + 1
            })
        );
        let (gerade_noch, pk2) = zusage(9, 11, HOECHSTZUSAGE_BYTES);
        assert_eq!(gerade_noch.pruefen(&pk2, EpochId(10)), Ok(()));
    }

    /// Über die Leitung und zurück, unverändert.
    #[test]
    fn borsh_haelt_die_zusage() {
        let (z, _) = zusage(10, 11, 123_456);
        let bytes = borsh::to_vec(&z).expect("serialisieren");
        let zurueck: Kapazitaetszusage = borsh::from_slice(&bytes).expect("lesen");
        assert_eq!(z, zurueck);
    }

    /// Das Rollenbyte der neuen Rolle darf die alten nicht verschieben.
    #[test]
    fn die_neue_rolle_verschiebt_keine_alte() {
        assert_eq!(Rolle::Shard.byte(), 1);
        assert_eq!(Rolle::PodMitglied.byte(), 2);
        assert_eq!(Rolle::Validator.byte(), 3);
        assert_eq!(Rolle::Checker.byte(), 4);
        assert_eq!(Rolle::Store.byte(), 5);
    }
}
