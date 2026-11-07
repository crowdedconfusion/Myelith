//! Ein Anteil der verteilten Schluesselerzeugung, verschluesselt fuer
//! genau einen Sitz.
//!
//! ⚑ **Wozu** (2026-11-03): Die Austeilung steht offen in der Kette, die
//! Werte fuer die Sitze duerfen es nicht. Jeder Wert geht verschluesselt an
//! den Schluessel des Sitzinhabers; wer ihn nicht entschluesseln kann oder
//! einen falschen findet, beschwert sich ([`crate::schwelle`]).
//!
//! **Das Verfahren:** gehashtes ElGamal ueber ristretto255. Der Austeiler
//! waehlt `k`, schickt `R = k·B` und `c = Wert ⊕ H(R, P, k·P, Kontext,
//! Nummer)`. Der Empfaenger mit `P = x·B` rechnet `x·R = k·P` und
//! entschluesselt. Eine Bindung an Kontext und Sitznummer verhindert, dass
//! eine Chiffre an anderer Stelle wiederverwendet wird.
//!
//! ⚑ **Warum ristretto255 und nicht BLS12-381** (Abweichung vom Entwurf
//! vom selben Tag, der G1 nannte): Entschluesseln heisst, einen
//! **geheimen** Skalar mit einem **fremden** Punkt zu multiplizieren. Fuer
//! G1 bietet `blst` das nur hinter `unsafe` oder ueber die
//! Mehrfachskalierung, und die laeuft nicht in konstanter Zeit.
//! `curve25519-dalek` rechnet in konstanter Zeit, liegt schon im Lager, und
//! ristretto255 hat Primordnung, also keine Punkte kleiner Ordnung. Dazu
//! kommt die Schluesseltrennung: Der Verschluesselungsschluessel ist ein
//! eigener, nicht der Signaturschluessel.
//!
//! **Integritaet** braucht die Chiffre nicht selbst: Die Austeilung ist
//! signiert, und ein entschluesselter Wert wird gegen die Feldman-Bindung
//! geprueft. Ein falscher Wert, aus welchem Grund auch immer, fuehrt zur
//! Beschwerde.

use borsh::{BorshDeserialize, BorshSerialize};
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek::scalar::Scalar;
use sha2::{Digest, Sha256, Sha512};

use crate::skalar::Skalar;

const DST_SCHLUESSEL: &[u8] = b"myl-anteilchiffre-schluessel-v1";
const DST_FLUECHTIG: &[u8] = b"myl-anteilchiffre-fluechtig-v1";
const DST_STROM: &[u8] = b"myl-anteilchiffre-strom-v1";

/// Der oeffentliche Verschluesselungsschluessel eines Mitglieds
/// (komprimierter ristretto255-Punkt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verschluesselungsschluessel(pub [u8; 32]);

/// Der geheime Gegenpart; verlaesst den Knoten nie.
pub struct Entschluesselungsschluessel(Scalar);

impl std::fmt::Debug for Entschluesselungsschluessel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Entschluesselungsschluessel(..)")
    }
}

/// Ein verschluesselter Wert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Anteilchiffre {
    /// `R = k·B`.
    pub r: [u8; 32],
    /// Der Wert (32 Bytes, hoechstwertiges zuerst), verdeckt.
    pub c: [u8; 32],
}

fn breit(teile: &[&[u8]]) -> Scalar {
    let mut h = Sha512::new();
    for t in teile {
        h.update((t.len() as u64).to_le_bytes());
        h.update(t);
    }
    let b: [u8; 64] = h.finalize().into();
    Scalar::from_bytes_mod_order_wide(&b)
}

/// Dekodiert einen Punkt; die Identitaet gilt nicht.
fn punkt(b: &[u8; 32]) -> Option<RistrettoPoint> {
    if *b == [0u8; 32] {
        return None;
    }
    CompressedRistretto(*b).decompress()
}

fn strom(r: &[u8; 32], p: &[u8; 32], geteilt: &RistrettoPoint, kontext: &[u8], nummer: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DST_STROM);
    h.update(r);
    h.update(p);
    h.update(geteilt.compress().as_bytes());
    h.update((kontext.len() as u64).to_le_bytes());
    h.update(kontext);
    h.update(nummer.to_le_bytes());
    h.finalize().into()
}

impl Entschluesselungsschluessel {
    /// Leitet den Schluessel aus dem Geheimnis des Knotens ab, unter eigener
    /// Domaene, also unabhaengig von jedem anderen Schluessel daraus.
    pub fn ableiten(geheimnis: &[u8; 32]) -> Option<Self> {
        let x = breit(&[DST_SCHLUESSEL, geheimnis]);
        if x == Scalar::ZERO {
            return None;
        }
        Some(Self(x))
    }

    /// Der oeffentliche Schluessel.
    pub fn oeffentlich(&self) -> Verschluesselungsschluessel {
        Verschluesselungsschluessel((self.0 * RISTRETTO_BASEPOINT_POINT).compress().to_bytes())
    }

    /// Entschluesselt einen Wert. `None`, wenn `R` ungueltig ist oder der
    /// Klartext keine Zahl kleiner als `r` ist; beides ist ein Grund zur
    /// Beschwerde.
    pub fn entschluesseln(&self, chiffre: &Anteilchiffre, kontext: &[u8], nummer: u64) -> Option<Skalar> {
        let r = punkt(&chiffre.r)?;
        let p = self.oeffentlich();
        let s = strom(&chiffre.r, &p.0, &(self.0 * r), kontext, nummer);
        let mut klar = [0u8; 32];
        for ((k, c), s) in klar.iter_mut().zip(&chiffre.c).zip(&s) {
            *k = c ^ s;
        }
        Skalar::aus_bytes_be(&klar)
    }
}

impl Verschluesselungsschluessel {
    /// Ob der Schluessel ein gueltiger Punkt ungleich der Identitaet ist.
    /// Vor der Aufnahme in die Kette zu pruefen.
    pub fn pruefen(&self) -> bool {
        punkt(&self.0).is_some()
    }
}

/// Verschluesselt einen Wert fuer einen Sitz. Der fluechtige Skalar wird
/// aus dem Geheimnis des Austeilers, dem Empfaenger, dem Kontext und der
/// Nummer abgeleitet: kein Zufallsgenerator, und nie zweimal derselbe fuer
/// verschiedene Empfaenger oder Sitze. `None` bei ungueltigem Empfaenger.
pub fn verschluesseln(
    empfaenger: &Verschluesselungsschluessel,
    wert: &Skalar,
    kontext: &[u8],
    nummer: u64,
    geheimnis_des_austeilers: &[u8; 32],
) -> Option<Anteilchiffre> {
    let p = punkt(&empfaenger.0)?;
    let k = breit(&[DST_FLUECHTIG, geheimnis_des_austeilers, &empfaenger.0, kontext, &nummer.to_le_bytes()]);
    if k == Scalar::ZERO {
        return None;
    }
    let r = (k * RISTRETTO_BASEPOINT_POINT).compress().to_bytes();
    let s = strom(&r, &empfaenger.0, &(k * p), kontext, nummer);
    let klar = wert.als_bytes_be();
    let mut c = [0u8; 32];
    for ((c, k), s) in c.iter_mut().zip(&klar).zip(&s) {
        *c = k ^ s;
    }
    Some(Anteilchiffre { r, c })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schluessel(i: u8) -> Entschluesselungsschluessel {
        Entschluesselungsschluessel::ableiten(&[i; 32]).expect("Schluessel")
    }

    #[test]
    fn hin_und_zurueck() {
        let e = schluessel(1);
        let p = e.oeffentlich();
        assert!(p.pruefen());
        for n in 1..20u64 {
            let w = Skalar::aus_u64(n * 1_000_003);
            let c = verschluesseln(&p, &w, b"dkg e5", n, &[9; 32]).expect("Chiffre");
            assert_eq!(e.entschluesseln(&c, b"dkg e5", n), Some(w));
        }
    }

    #[test]
    fn falscher_empfaenger_kontext_oder_sitz_ergibt_keinen_wert() {
        let (e, f) = (schluessel(1), schluessel(2));
        let w = Skalar::aus_u64(42);
        let c = verschluesseln(&e.oeffentlich(), &w, b"k", 3, &[9; 32]).expect("Chiffre");
        assert_ne!(f.entschluesseln(&c, b"k", 3), Some(w));
        assert_ne!(e.entschluesseln(&c, b"K", 3), Some(w));
        assert_ne!(e.entschluesseln(&c, b"k", 4), Some(w));
    }

    #[test]
    fn der_fluechtige_schluessel_wechselt_je_sitz() {
        let p = schluessel(1).oeffentlich();
        let w = Skalar::aus_u64(1);
        let a = verschluesseln(&p, &w, b"k", 1, &[9; 32]).expect("Chiffre");
        let b = verschluesseln(&p, &w, b"k", 2, &[9; 32]).expect("Chiffre");
        assert_ne!(a.r, b.r);
    }

    #[test]
    fn identitaet_und_muell_werden_abgewiesen() {
        assert!(!Verschluesselungsschluessel([0; 32]).pruefen());
        assert!(!Verschluesselungsschluessel([0xff; 32]).pruefen());
        let w = Skalar::aus_u64(1);
        assert!(verschluesseln(&Verschluesselungsschluessel([0; 32]), &w, b"k", 1, &[9; 32]).is_none());
        let e = schluessel(1);
        let mut c = verschluesseln(&e.oeffentlich(), &w, b"k", 1, &[9; 32]).expect("Chiffre");
        c.r = [0; 32];
        assert!(e.entschluesseln(&c, b"k", 1).is_none());
    }

    /// Ein Klartext ab `r` ist keine gueltige Zahl und wird nicht still
    /// reduziert: Sonst gaebe es zwei Schreibweisen desselben Werts.
    #[test]
    fn ein_klartext_ueber_r_wird_abgewiesen() {
        let e = schluessel(1);
        let p = e.oeffentlich();
        let mut c = verschluesseln(&p, &Skalar::NULL, b"k", 1, &[9; 32]).expect("Chiffre");
        // Klartext null: c ist der Strom selbst; mit 0xff.. verknuepft ergibt
        // er den Klartext 0xff.., und der liegt ueber r.
        for b in c.c.iter_mut() {
            *b ^= 0xff;
        }
        assert!(e.entschluesseln(&c, b"k", 1).is_none());
    }
}
