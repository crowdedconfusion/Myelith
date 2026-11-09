//! **Prüfpunkte**: Endgültigkeit alle [`BLOECKE_JE_PRUEFPUNKT`] Blöcke
//! (Festlegung des Projektinhabers vom 2026-10-27).
//!
//! # ⚑ Was hier steht und warum so
//!
//! Das Ziel ist das Whitepaper (Kap. 3.5.2: BFT je Block,
//! der Erzeuger rotiert per VRF). Stufe 1 sind Prüfpunkte: Der Block an jeder
//! Höhe `150 · n` wird vom Stimmsatz bestätigt, und das Zertifikat geht in
//! einen späteren Block ([`crate::block::BlockHeader::pruefpunkt`]).
//!
//! **Keine Runden, kein Leader** (Festlegung vom selben Tag): Der Block eines Prüfpunkts steht schon fest, es gibt nichts
//! vorzuschlagen. Jedes Mitglied unterschreibt (Höhe, Blockhash) des Blocks,
//! den es an dieser Höhe hat ([`Pruefpunktstimme`]); sobald mehr als zwei
//! Drittel des Gewichts beisammen sind, kann jeder daraus ein
//! [`Pruefpunktzertifikat`] bauen.
//!
//! **Sicherheit:** Zwei Zertifikate für dieselbe Höhe mit verschiedenen
//! Hashes brauchen mindestens ein Drittel Gewicht, das beide unterschrieben
//! hat, und eine solche Doppelstimme ist ein schlachtbarer Beleg. Unterschreibt
//! der Erzeuger zwei Blöcke derselben Höhe, stimmen die Mitglieder gespalten
//! ab, und es entsteht kein Zertifikat: Das kostet Endgültigkeit, nicht
//! Sicherheit.
//!
//! ⚑ **Die Bindung an die kommenden Saetze** (7.3, 2026-11-04): Jede
//! Stimme unterschreibt neben dem Block die [`Pruefpunktstimme::bindung`],
//! den Hash der Stimmsaetze der beiden folgenden Epochen, wie der Zustand sie
//! nach dem Block fuehrt. Ein Zertifikat des Satzes von `e` bestaetigt damit,
//! wer in `e + 1` und `e + 2` traegt. Wer neu einsteigt, folgt so dem Satz
//! von Genesis an, ein Zertifikat je Epoche, ohne einen Block zu holen.
//! 📌 Im Blockkopf haette die Bindung
//! nichts genuetzt: Der Blockhash deckt alle Transaktionen, und ein Beleg
//! haette den ganzen Block tragen muessen.
//!
//! ⚑ **Die Quelle jeder Stimme** (7.7 Schritt 1, 2026-11-04): Jede Stimme nennt neben ihrem Ziel (Hoehe und Block) ihre
//! [`Quelle`], den juengsten gerechtfertigten Pruefpunkt, wie der Zustand
//! ihn am Zielblock vermerkt hat. Ein Zertifikat **rechtfertigt** sein Ziel;
//! **endgueltig** wird ein Pruefpunkt erst, wenn ein Zertifikat des
//! naechsten ihn als Quelle nennt ([`Pruefpunktzertifikat::schliesst_ab`]).
//! 📌 Warum nicht schon das Zertifikat selbst endgueltig macht: Sobald der
//! Erzeuger rotiert und Zweige moeglich sind, kann ein ehrlicher Knoten,
//! der ein Zertifikat an 150 nicht gesehen hat, auf einen laengeren Zweig
//! wechseln und an 300 dort stimmen. Zwei Zertifikate fuer zwei
//! Geschichten entstuenden dann ohne einen schlachtbaren Beleg. Mit der
//! Quelle ist jeder solche Widerspruch einer von zwei Arten, und beide sind
//! schlachtbar ([`widersprechen`]): zwei Stimmen fuer dasselbe Ziel, oder
//! eine, die eine andere umschliesst (`q1 < q2 < z2 < z1`). Der Preis ist
//! ein Pruefpunkt Verzug bis zur Endgueltigkeit, ohne eine Nachricht mehr.
//!
//! **Trennung von den BFT-Nachrichten:** eigenes Trennzeichen
//! [`DST_PRUEFPUNKT`], und die Höhe steht in der Botschaft. Eine Stimme oder
//! ein Commit der Runden lässt sich nicht als Prüfpunktstimme ausgeben und
//! umgekehrt.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsAggregateSignature, BlsSecretKey, BlsSignature, aggregate_signatures};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

use crate::round_change::RoundError;
use crate::validator::VotingSet;

/// Alle wie viele Blöcke ein Prüfpunkt liegt: 150, also fünf Minuten bei
/// 2 s Blockzeit, zwölf je Epoche.
///
/// ⚑ **Ein Konsenswert wie die Epochenlänge**: Wer ihn ändert, ändert den
/// Vertrag. Er teilt die Epoche, damit jede Epochengrenze ein Prüfpunkt ist;
/// an ihr wechselt der Stimmsatz, und aus ihrem Zertifikat kommt die Saat.
pub const BLOECKE_JE_PRUEFPUNKT: u64 = 150;

const _: () = assert!(crate::block::BLOECKE_JE_EPOCHE.is_multiple_of(BLOECKE_JE_PRUEFPUNKT));

/// Trennzeichen einer Prüfpunktstimme.
/// ⚑ v2 seit der Bindung, v3 seit der Quelle (beide 2026-11-04).
pub const DST_PRUEFPUNKT: &[u8] = b"MYELITH_PRUEFPUNKT_v3";

/// Ist diese Höhe ein Prüfpunkt? Genesis (Höhe 0) nicht.
pub fn ist_pruefpunkt(hoehe: u64) -> bool {
    hoehe > 0 && hoehe.is_multiple_of(BLOECKE_JE_PRUEFPUNKT)
}

/// ⚑ **Die Quelle einer Stimme** (7.7 Schritt 1): der juengste
/// gerechtfertigte Pruefpunkt, von dem das Ziel abstammt. Genesis ist die
/// Hoehe null mit dem Hash null und gilt ohne Zertifikat als
/// gerechtfertigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Quelle {
    /// Die Hoehe des Pruefpunkts, null fuer Genesis.
    pub hoehe: u64,
    /// Sein Block, null fuer Genesis.
    pub block_hash: Hash,
}

impl Quelle {
    /// Genesis.
    pub const GENESIS: Quelle = Quelle { hoehe: 0, block_hash: Hash([0u8; 32]) };

    /// Kann sie Quelle einer Stimme mit dem Ziel `ziel` sein? Genesis oder
    /// ein Pruefpunkt, und vor dem Ziel.
    pub fn passt_zu(&self, ziel: u64) -> bool {
        self.hoehe < ziel && (*self == Self::GENESIS || ist_pruefpunkt(self.hoehe))
    }
}

/// Die Botschaft, über die ein Mitglied unterschreibt:
/// `DST_PRUEFPUNKT ‖ u64_le(hoehe) ‖ block_hash ‖ bindung ‖ u64_le(quelle.hoehe) ‖ quelle.block_hash`.
pub fn pruefpunktnachricht(hoehe: u64, block_hash: &Hash, bindung: &Hash, quelle: &Quelle) -> Vec<u8> {
    let mut b = crate::signing::signable_bytes(DST_PRUEFPUNKT, hoehe, block_hash);
    b.extend_from_slice(bindung.as_bytes());
    b.extend_from_slice(&quelle.hoehe.to_le_bytes());
    b.extend_from_slice(quelle.block_hash.as_bytes());
    b
}

/// ⚑ **Widersprechen sich zwei Stimmen desselben Waehlers?**
/// 1. **Dasselbe Ziel, verschiedene Stimmen:** anderer Block, andere
///    Bindung oder andere Quelle.
/// 2. **Eine umschliesst die andere:** `q1 < q2` und `z2 < z1`.
///
/// Ein ehrlicher Knoten tut beides nie: Er stimmt je Hoehe einmal, und nur
/// mit einer Quelle, die nicht unter der einer frueheren Stimme liegt.
/// Unterschriften prueft das nicht.
pub fn widersprechen(a: &Pruefpunktstimme, b: &Pruefpunktstimme) -> bool {
    if a.waehler != b.waehler {
        return false;
    }
    if a.hoehe == b.hoehe {
        return (a.block_hash, a.bindung, a.quelle) != (b.block_hash, b.bindung, b.quelle);
    }
    umschliesst(a, b) || umschliesst(b, a)
}

/// Umschliesst `aussen` die Stimme `innen`: `q_aussen < q_innen < z_innen < z_aussen`?
pub fn umschliesst(aussen: &Pruefpunktstimme, innen: &Pruefpunktstimme) -> bool {
    aussen.quelle.hoehe < innen.quelle.hoehe && innen.hoehe < aussen.hoehe
}

/// Die Stimme eines Mitglieds für den Block an einem Prüfpunkt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefpunktstimme {
    /// Die Höhe des Prüfpunkts.
    pub hoehe: u64,
    /// Der Hash des Blocks an dieser Höhe, wie der Wähler ihn hat.
    pub block_hash: Hash,
    /// ⚑ Die Stimmsaetze der beiden folgenden Epochen, wie der Waehler sie
    /// nach diesem Block fuehrt (`myl_ledger::stimmsatz::bindung`).
    pub bindung: Hash,
    /// ⚑ Der juengste gerechtfertigte Pruefpunkt, wie der Zustand ihn am
    /// Zielblock vermerkt hat (7.7 Schritt 1).
    pub quelle: Quelle,
    /// Wer stimmt.
    pub waehler: MinerId,
    /// Unterschrift über [`pruefpunktnachricht`].
    pub signatur: BlsSignature,
}

impl Pruefpunktstimme {
    /// Unterschreibt den Block an `hoehe`.
    pub fn unterschreiben(
        hoehe: u64,
        block_hash: Hash,
        bindung: Hash,
        quelle: Quelle,
        waehler: MinerId,
        geheim: &BlsSecretKey,
    ) -> Result<Self, RoundError> {
        let signatur = geheim
            .sign(&pruefpunktnachricht(hoehe, &block_hash, &bindung, &quelle))
            .map_err(|_| RoundError::InvalidSignature)?;
        Ok(Self { hoehe, block_hash, bindung, quelle, waehler, signatur })
    }

    /// Die Botschaft dieser Stimme.
    pub fn nachricht(&self) -> Vec<u8> {
        pruefpunktnachricht(self.hoehe, &self.block_hash, &self.bindung, &self.quelle)
    }

    /// Trägt die Stimme? Ein Prüfpunkt, eine passende Quelle, ein Mitglied,
    /// und die Unterschrift unter dem Schlüssel, den der Satz für es führt.
    pub fn traegt(&self, satz: &VotingSet) -> bool {
        ist_pruefpunkt(self.hoehe)
            && self.quelle.passt_zu(self.hoehe)
            && satz.pubkey(&self.waehler).is_some_and(|pk| pk.verify(&self.nachricht(), &self.signatur))
    }
}

/// **Der Beleg, dass mehr als zwei Drittel des Stimmsatzes den Block an
/// einem Prüfpunkt bestätigt haben.**
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefpunktzertifikat {
    /// Die Höhe des Prüfpunkts.
    pub hoehe: u64,
    /// Der bestätigte Block.
    pub block_hash: Hash,
    /// Die bestaetigte Bindung an die Saetze der beiden folgenden Epochen.
    pub bindung: Hash,
    /// Die gemeinsame Quelle aller Stimmen.
    pub quelle: Quelle,
    /// Unterzeichner, streng aufsteigend (kanonisch, ohne Doppelte).
    pub unterzeichner: Vec<MinerId>,
    /// Aggregierte BLS-Signatur aller Unterzeichner.
    pub aggregat: BlsAggregateSignature,
}

impl Pruefpunktzertifikat {
    /// Baut das Zertifikat aus Stimmen derselben Höhe und desselben Hashes.
    ///
    /// Geprüft wird hier nur die Form; Gewicht und Unterschriften prüft
    /// [`Self::verify`], beim Empfänger wie beim Bauenden.
    pub fn aus_stimmen(stimmen: &[Pruefpunktstimme]) -> Result<Self, RoundError> {
        let erste = stimmen.first().ok_or(RoundError::EmptyCertificate)?;
        let mut paare: Vec<(MinerId, BlsSignature)> = Vec::with_capacity(stimmen.len());
        for s in stimmen {
            if s.hoehe != erste.hoehe || s.block_hash != erste.block_hash || s.bindung != erste.bindung || s.quelle != erste.quelle {
                return Err(RoundError::InconsistentCertificate);
            }
            paare.push((s.waehler, s.signatur));
        }
        paare.sort_by_key(|(m, _)| *m);
        if paare.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(RoundError::NonCanonicalCertificate);
        }
        let sigs: Vec<BlsSignature> = paare.iter().map(|(_, s)| *s).collect();
        let aggregat = aggregate_signatures(&sigs).map_err(|_| RoundError::InvalidSignature)?;
        Ok(Self {
            hoehe: erste.hoehe,
            block_hash: erste.block_hash,
            bindung: erste.bindung,
            quelle: erste.quelle,
            unterzeichner: paare.into_iter().map(|(m, _)| m).collect(),
            aggregat,
        })
    }

    /// Prüft das Zertifikat gegen den Stimmsatz: ein Prüfpunkt, kanonische
    /// Unterzeichner, alle im Satz, mehr als zwei Drittel des Gewichts, und
    /// die Sammelsignatur über die Prüfpunktbotschaft. Derselbe Kern wie bei
    /// den Zertifikaten der Runden.
    pub fn verify(&self, satz: &VotingSet) -> Result<(), RoundError> {
        if !ist_pruefpunkt(self.hoehe) || !self.quelle.passt_zu(self.hoehe) {
            return Err(RoundError::InconsistentCertificate);
        }
        crate::round_change::pruefe_aggregat(
            &self.unterzeichner,
            satz,
            &pruefpunktnachricht(self.hoehe, &self.block_hash, &self.bindung, &self.quelle),
            &self.aggregat,
        )
    }

    /// ⚑ **Macht dieses Zertifikat seine Quelle endgueltig?** Genau dann,
    /// wenn das Ziel der naechste Pruefpunkt nach der Quelle ist (die
    /// Verbindung zum direkten Nachfolger). Dass die Quelle
    /// gerechtfertigt ist, prueft, wer das Zertifikat anwendet.
    pub fn schliesst_ab(&self) -> bool {
        self.quelle.hoehe.checked_add(BLOECKE_JE_PRUEFPUNKT) == Some(self.hoehe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::VotingMember;

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b; 32]).expect("Schlüssel")
    }

    fn kennung(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Vier Mitglieder mit Gewicht eins.
    fn satz() -> VotingSet {
        let mut m = std::collections::BTreeMap::new();
        for b in 1..=4u8 {
            m.insert(kennung(b), VotingMember { pubkey: schluessel(b).public_key().expect("pk"), weight: 1 });
        }
        VotingSet::from_members(m)
    }

    fn stimme(b: u8, hoehe: u64, hash: Hash) -> Pruefpunktstimme {
        stimme_mit(b, hoehe, hash, Quelle::GENESIS)
    }

    fn stimme_mit(b: u8, hoehe: u64, hash: Hash, quelle: Quelle) -> Pruefpunktstimme {
        Pruefpunktstimme::unterschreiben(hoehe, hash, Hash::sha256(b"bindung"), quelle, kennung(b), &schluessel(b)).expect("Stimme")
    }

    fn quelle(hoehe: u64) -> Quelle {
        Quelle { hoehe, block_hash: Hash::sha256(&hoehe.to_le_bytes()) }
    }

    #[test]
    fn nur_vielfache_von_150_sind_pruefpunkte() {
        assert!(!ist_pruefpunkt(0));
        assert!(!ist_pruefpunkt(149));
        assert!(ist_pruefpunkt(150));
        assert!(ist_pruefpunkt(crate::block::BLOECKE_JE_EPOCHE), "jede Epochengrenze ist ein Prüfpunkt");
    }

    /// Drei von vier tragen, zwei nicht; eine fremde Stimme, eine andere
    /// Höhe oder ein Commit der Runden tragen nicht.
    #[test]
    fn mehr_als_zwei_drittel_bestaetigen_einen_pruefpunkt() {
        let h = Hash::sha256(b"Block an 150");
        let drei: Vec<_> = (1..=3).map(|b| stimme(b, 150, h)).collect();
        assert!(drei.iter().all(|s| s.traegt(&satz())));
        let z = Pruefpunktzertifikat::aus_stimmen(&drei).expect("Zertifikat");
        assert_eq!(z.verify(&satz()), Ok(()));
        // ⛔️ Gegenproben.
        let zwei = Pruefpunktzertifikat::aus_stimmen(&drei[..2]).expect("Zertifikat");
        assert_eq!(zwei.verify(&satz()), Err(RoundError::CertificateBelowQuorum));
        let mut andere_hoehe = z.clone();
        andere_hoehe.hoehe = 300;
        assert_eq!(andere_hoehe.verify(&satz()), Err(RoundError::InvalidSignature));
        let mut kein_pruefpunkt = z.clone();
        kein_pruefpunkt.hoehe = 151;
        assert!(kein_pruefpunkt.verify(&satz()).is_err());
        assert!(!stimme(9, 150, h).traegt(&satz()), "ein Fremder ist kein Mitglied");
        let mut falsch = stimme(1, 150, h);
        falsch.block_hash = Hash::sha256(b"ein anderer Block");
        assert!(!falsch.traegt(&satz()));
        // Ein Commit der BFT-Runden über denselben Hash ist keine
        // Prüfpunktstimme (eigenes Trennzeichen).
        let commit = schluessel(1).sign(&crate::signing::commit_message(150, &h)).expect("Commit");
        let als_stimme = Pruefpunktstimme {
            hoehe: 150,
            block_hash: h,
            bindung: Hash::sha256(b"bindung"),
            quelle: Quelle::GENESIS,
            waehler: kennung(1),
            signatur: commit,
        };
        assert!(!als_stimme.traegt(&satz()));
    }

    #[test]
    fn stimmen_verschiedener_bloecke_ergeben_kein_zertifikat() {
        let a = stimme(1, 150, Hash::sha256(b"a"));
        let b = stimme(2, 150, Hash::sha256(b"b"));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[a.clone(), b]), Err(RoundError::InconsistentCertificate));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[a.clone(), a]), Err(RoundError::NonCanonicalCertificate));
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[]), Err(RoundError::EmptyCertificate));
    }

    /// ⚑ Die Bindung ist unterschrieben (7.3): Ein Zertifikat mit einer
    /// anderen traegt nicht, und Stimmen mit verschiedenen Bindungen ergeben
    /// keines.
    #[test]
    fn die_bindung_ist_unterschrieben() {
        let h = Hash::sha256(b"Block an 150");
        let drei: Vec<_> = (1..=3).map(|b| stimme(b, 150, h)).collect();
        let mut z = Pruefpunktzertifikat::aus_stimmen(&drei).expect("Zertifikat");
        z.bindung = Hash::sha256(b"andere Saetze");
        assert_eq!(z.verify(&satz()), Err(RoundError::InvalidSignature));
        let anders =
            Pruefpunktstimme::unterschreiben(150, h, Hash::sha256(b"andere Saetze"), Quelle::GENESIS, kennung(4), &schluessel(4))
                .expect("Stimme");
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[drei[0].clone(), anders]), Err(RoundError::InconsistentCertificate));
    }

    /// ⚑ Die Quelle ist unterschrieben (7.7 Schritt 1): Ein Zertifikat mit
    /// einer anderen traegt nicht, Stimmen mit verschiedenen Quellen ergeben
    /// keines, und eine Quelle an oder nach dem Ziel oder neben einem
    /// Pruefpunkt traegt nicht.
    #[test]
    fn die_quelle_ist_unterschrieben() {
        let h = Hash::sha256(b"Block an 300");
        let drei: Vec<_> = (1..=3).map(|b| stimme_mit(b, 300, h, quelle(150))).collect();
        let z = Pruefpunktzertifikat::aus_stimmen(&drei).expect("Zertifikat");
        assert_eq!(z.verify(&satz()), Ok(()));
        assert!(z.schliesst_ab(), "300 nennt 150: 150 ist endgueltig");
        let mut umgeschrieben = z.clone();
        umgeschrieben.quelle = Quelle::GENESIS;
        assert_eq!(umgeschrieben.verify(&satz()), Err(RoundError::InvalidSignature));
        assert!(!umgeschrieben.schliesst_ab(), "von Genesis nach 300 ist kein direkter Nachfolger");
        let andere = stimme_mit(4, 300, h, Quelle::GENESIS);
        assert_eq!(Pruefpunktzertifikat::aus_stimmen(&[drei[0].clone(), andere]), Err(RoundError::InconsistentCertificate));
        // ⛔️ Gegenproben zur Form der Quelle.
        assert!(!stimme_mit(1, 300, h, quelle(300)).traegt(&satz()), "Quelle am Ziel");
        assert!(!stimme_mit(1, 300, h, quelle(450)).traegt(&satz()), "Quelle nach dem Ziel");
        assert!(!stimme_mit(1, 300, h, quelle(151)).traegt(&satz()), "kein Pruefpunkt");
        assert!(!stimme_mit(1, 300, h, Quelle { hoehe: 0, block_hash: h }).traegt(&satz()), "Genesis mit fremdem Hash");
        let unpassend: Vec<_> = (1..=3).map(|b| stimme_mit(b, 300, h, quelle(300))).collect();
        let unpassend = Pruefpunktzertifikat::aus_stimmen(&unpassend).expect("Form");
        assert_eq!(unpassend.verify(&satz()), Err(RoundError::InconsistentCertificate));
    }

    /// ⚑ Die beiden Widersprueche, und was keiner ist.
    #[test]
    fn doppelt_und_umschliessend_widersprechen() {
        let a = Hash::sha256(b"a");
        let b = Hash::sha256(b"b");
        let s = |hoehe, q: u64| stimme_mit(1, hoehe, a, if q == 0 { Quelle::GENESIS } else { quelle(q) });
        // Dasselbe Ziel: anderer Block oder andere Quelle.
        assert!(widersprechen(&s(300, 150), &stimme_mit(1, 300, b, quelle(150))));
        assert!(widersprechen(&s(300, 150), &s(300, 0)));
        assert!(!widersprechen(&s(300, 150), &s(300, 150)), "dieselbe Stimme zweimal");
        // Umschliessen, in beiden Reihenfolgen.
        assert!(widersprechen(&s(600, 0), &s(450, 150)));
        assert!(widersprechen(&s(450, 150), &s(600, 0)));
        // ⛔️ Kein Widerspruch: nacheinander, gleiche Quelle, oder ein
        // anderer Waehler.
        assert!(!widersprechen(&s(300, 150), &s(450, 300)));
        assert!(!widersprechen(&s(300, 0), &s(450, 0)), "gleiche Quelle umschliesst nicht");
        assert!(!widersprechen(&s(450, 150), &s(600, 450)));
        assert!(!widersprechen(&s(600, 0), &stimme_mit(2, 450, a, quelle(150))));
    }

    #[test]
    fn das_zertifikat_ueberlebt_borsh() {
        let h = Hash::sha256(b"x");
        let z = Pruefpunktzertifikat::aus_stimmen(&(1..=3).map(|b| stimme(b, 300, h)).collect::<Vec<_>>()).expect("Zertifikat");
        let bytes = borsh::to_vec(&z).expect("borsh");
        assert_eq!(borsh::from_slice::<Pruefpunktzertifikat>(&bytes).expect("lesen"), z);
    }
}
