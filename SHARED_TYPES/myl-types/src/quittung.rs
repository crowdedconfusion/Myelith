//! Der Stichprobenlauf einer Epoche: wer hat geliefert, wer nicht.
//!
//! # ⚑ Niemand fragt, und das ist der ganze Entwurf
//!
//! Die Stichprobe wird aus Epochenseed, Gegenstand, Epoche und
//! **Halterkennung** abgeleitet. Jeder kann sie ausrechnen, also auch
//! der Halter selbst: **Er weiß ohne Anfrage, was er diese Epoche
//! schuldet.**
//!
//! Damit entfällt die Anfrage, und mit ihr eine ganze Klasse von
//! Fragen: wer fragt, was gilt, wenn niemand fragt, und wie man einen
//! unterbliebenen Anruf beweist. Es ist dieselbe Wendung wie bei E10,
//! wo die Schiedsrunde keine Antwort mehr braucht, weil der Ankläger
//! alles mitbringt, nur in die andere Richtung: Hier bringt der
//! **Halter** von sich aus.
//!
//! # Was er vorlegt, und warum nicht die Bytes
//!
//! Eine **Quittung**: der Hash über die Bytes des verlangten Teils,
//! gebunden an Epoche, Gegenstand, Teilnummer und Halter, unterschrieben
//! in der Rolle [`Rolle::Store`]. Rund 130 Byte statt eines Mebibytes.
//!
//! ⚑ **Eine Quittung beweist nichts**, und das ist kein Mangel: Hashen
//! kann jeder irgendetwas. Sie **verpflichtet** ihn. Wer sie abgibt,
//! kann später auf die Bytes festgenagelt werden, und wer keine abgibt,
//! ist schon ohne Ankläger auffällig.
//!
//! # ⚑ Die fehlende Quittung braucht keinen Ankläger
//!
//! Die Zuteilung ist deterministisch und aus dem Zustand nachrechenbar.
//! **Jeder sieht dieselbe Liste der Schuldner**, und wessen Quittung
//! fehlt, steht damit objektiv fest. Kein Zeuge, keine Behauptung, kein
//! Beweis eines Negativs.
//!
//! Das ist die Sorte Urteil, die Grundsatz G1 verlangt: über
//! nachrechenbare Tatsachen, nicht über Inhalte. Und es ist der
//! Unterschied zur Bisektion, wo jemand stillstehen kann, ohne dass es
//! jemandem auffiele.
//!
//! # Was hier **nicht** entschieden wird
//!
//! **Ob eine abgegebene Quittung wahr ist.** Dafür müsste jemand das
//! Mebibyte anfordern und nachrechnen; das ist der optimistische Teil
//! und arbeitet wie die Verifikation der Inferenz: Wer nachfragt, findet
//! den Betrug, und wer nie nachfragt, bezahlt ihn. Diese Stufe steht
//! aus (STORAGE 3.2).
//!
//! **Und keine Frist.** Eine Quittung gehört in die Epoche, für die sie
//! gilt; die Epochennummer kommt aus dem Konsens, jeder rechnet
//! dieselbe, und niemand kann sich hinter seiner Uhr verstecken.

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::gegenstand::Manifest;
use crate::hash::Hash;
use crate::ids::{EpochId, MerkleRoot, MinerId};
use crate::uebergang::Rolle;
use crate::zuteilung::Zuteilung;
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::BTreeMap;

/// Trennstring der Speicherquittung.
pub const DST_SPEICHERQUITTUNG: &[u8] = b"MYELITH_SPEICHERQUITTUNG_v1";

/// Was ein Halter über eine Epoche vorlegt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Speicherquittung {
    /// Wer quittiert.
    pub halter: MinerId,
    /// Für welche Epoche.
    pub epoche: EpochId,
    /// Für welchen Gegenstand.
    pub wurzel: MerkleRoot,
    /// Welcher Teil verlangt war.
    pub teil: u32,
    /// Hash über die Bytes dieses Teils.
    pub antwort: Hash,
    /// Unterschrift des Halters in der Rolle [`Rolle::Store`].
    pub signature: BlsSignature,
}

impl Speicherquittung {
    /// Eine unsignierte Quittung. [`Self::signiere`] gehört dazu.
    pub fn neu(
        halter: MinerId,
        epoche: EpochId,
        wurzel: MerkleRoot,
        teil: u32,
        antwort: Hash,
    ) -> Self {
        Self {
            halter,
            epoche,
            wurzel,
            teil,
            antwort,
            signature: BlsSignature([0u8; crate::bls::BLS_SIG_LEN]),
        }
    }

    /// Die Bytes, über die unterschrieben wird.
    pub fn signierbotschaft(&self) -> Vec<u8> {
        let kern = (self.halter, self.epoche, self.wurzel, self.teil, self.antwort);
        let rumpf = borsh::to_vec(&kern).expect("feste Feldbreiten sind stets serialisierbar");
        let mut msg = Vec::with_capacity(DST_SPEICHERQUITTUNG.len() + 1 + rumpf.len());
        msg.extend_from_slice(DST_SPEICHERQUITTUNG);
        msg.push(Rolle::Store.byte());
        msg.extend_from_slice(&rumpf);
        msg
    }

    /// Unterschreibt die Quittung.
    pub fn signiere(&mut self, sk: &BlsSecretKey) -> Result<(), crate::bls::BlsError> {
        self.signature = sk.sign(&self.signierbotschaft())?;
        Ok(())
    }

    /// Unterschrift **und** Identität, wie bei der Zusage (Fund 96).
    pub fn ist_vom_halter(&self, pk: &BlsPublicKey) -> bool {
        MinerId::aus_schluessel(pk) == self.halter
            && pk.verify(&self.signierbotschaft(), &self.signature)
    }
}

/// Warum eine vorgelegte Quittung nicht zählt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Untauglich {
    /// Sie gilt einer anderen Epoche.
    FremdeEpoche,
    /// Der Halter ist diesem Gegenstand nicht zugeteilt.
    NichtZugeteilt,
    /// Der Gegenstand steht nicht im Register.
    UnbekannterGegenstand,
    /// Quittiert wurde ein anderer Teil als der verlangte.
    ///
    /// ⚑ **Der verräterischste Fall.** Die Stichprobe ist aus
    /// öffentlichen Größen ableitbar; wer einen anderen Teil quittiert,
    /// hat entweder falsch gerechnet oder den Teil gewählt, den er noch
    /// hat.
    FalscherTeil {
        /// Verlangt war dieser.
        verlangt: u32,
        /// Quittiert wurde jener.
        quittiert: u32,
    },
    /// Unterschrift falsch, oder von einem anderen.
    Unterschrift,
}

/// Was eine Epoche über die Halter sagt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Epochenbefund {
    /// Wer geliefert hat, aufsteigend geordnet.
    pub geliefert: Vec<(MerkleRoot, MinerId)>,
    /// Wer nichts vorgelegt hat.
    ///
    /// **Ohne Ankläger festgestellt:** Die Zuteilung ist nachrechenbar,
    /// also steht objektiv fest, wer schuldete.
    pub fehlend: Vec<(MerkleRoot, MinerId)>,
    /// Wer etwas vorgelegt hat, das nicht zählt, samt Grund.
    ///
    /// ⚑ **Getrennt von [`Self::fehlend`], und das ist keine
    /// Feinheit.** Schweigen und ein untauglicher Versuch sind
    /// verschiedene Befunde: Das eine kann ein Ausfall sein, das andere
    /// ist eine Handlung. Sie in einen Topf zu werfen hieße, dem
    /// Abgestürzten dasselbe vorzuwerfen wie dem, der es versucht hat.
    pub untauglich: Vec<(MerkleRoot, MinerId, Untauglich)>,
}

impl Epochenbefund {
    /// Wie viele Halter geliefert haben.
    pub fn lieferungen(&self) -> usize {
        self.geliefert.len()
    }
}

/// Wertet die Quittungen einer Epoche gegen die Zuteilung aus.
///
/// `schluessel` liefert den öffentlichen Schlüssel eines Halters; ohne
/// ihn ist eine Quittung nicht prüfbar und gilt als untauglich.
pub fn stichprobenlauf(
    zuteilung: &Zuteilung,
    gegenstaende: &BTreeMap<MerkleRoot, Manifest>,
    quittungen: &[Speicherquittung],
    epoche: EpochId,
    seed: &[u8; 32],
    schluessel: &dyn Fn(&MinerId) -> Option<BlsPublicKey>,
) -> Epochenbefund {
    let mut befund = Epochenbefund::default();
    let mut gutgeschrieben: BTreeMap<(MerkleRoot, MinerId), ()> = BTreeMap::new();

    for q in quittungen {
        let grund = pruefe_quittung(q, zuteilung, gegenstaende, epoche, seed, schluessel);
        match grund {
            None => {
                gutgeschrieben.insert((q.wurzel, q.halter), ());
            }
            Some(g) => befund.untauglich.push((q.wurzel, q.halter, g)),
        }
    }

    // ⚑ **Die Schuldnerliste entsteht aus der Zuteilung, nicht aus den
    // Quittungen.** Wer aus den vorgelegten Quittungen ableitete, wer
    // schuldet, fände nie jemanden, der schweigt.
    for (wurzel, halter) in &zuteilung.je_gegenstand {
        for h in halter {
            if gutgeschrieben.contains_key(&(*wurzel, *h)) {
                befund.geliefert.push((*wurzel, *h));
            } else {
                befund.fehlend.push((*wurzel, *h));
            }
        }
    }

    befund
}

/// `None`, wenn die Quittung zählt.
fn pruefe_quittung(
    q: &Speicherquittung,
    zuteilung: &Zuteilung,
    gegenstaende: &BTreeMap<MerkleRoot, Manifest>,
    epoche: EpochId,
    seed: &[u8; 32],
    schluessel: &dyn Fn(&MinerId) -> Option<BlsPublicKey>,
) -> Option<Untauglich> {
    if q.epoche != epoche {
        return Some(Untauglich::FremdeEpoche);
    }
    let Some(manifest) = gegenstaende.get(&q.wurzel) else {
        return Some(Untauglich::UnbekannterGegenstand);
    };
    if !zuteilung.halter(&q.wurzel).contains(&q.halter) {
        return Some(Untauglich::NichtZugeteilt);
    }
    let verlangt = crate::zuteilung::verlangter_teil(manifest, epoche, &q.halter, seed);
    if q.teil != verlangt {
        return Some(Untauglich::FalscherTeil {
            verlangt,
            quittiert: q.teil,
        });
    }
    let Some(pk) = schluessel(&q.halter) else {
        return Some(Untauglich::Unterschrift);
    };
    if !q.ist_vom_halter(&pk) {
        return Some(Untauglich::Unterschrift);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gegenstand::{Gegenstandsart, Redundanzform};
    use crate::zuteilung::verlangter_teil;

    const SEED: [u8; 32] = [3u8; 32];
    const EPOCHE: EpochId = EpochId(9);

    fn schluesselpaar(b: u8) -> (BlsSecretKey, BlsPublicKey) {
        let sk = BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel");
        let pk = sk.public_key().expect("pk");
        (sk, pk)
    }

    fn manifest(b: u8) -> Manifest {
        Manifest {
            art: Gegenstandsart::Shardgewichte,
            fassung: 1,
            teilzahl: 64,
            wurzel: MerkleRoot::new([b; 32]),
            redundanz: Redundanzform::Kopien { anzahl: 2 },
            laenge: 64 * 1024,
        }
    }

    /// Ein Teilnehmer der Probe: Schlüsselpaar und Kennung.
    type Teilnehmer = (BlsSecretKey, BlsPublicKey, MinerId);

    /// Zwei Halter, ein Gegenstand, beide zugeteilt.
    fn aufbau() -> (BTreeMap<MerkleRoot, Manifest>, Zuteilung, Vec<Teilnehmer>) {
        let m = manifest(1);
        let mut g = BTreeMap::new();
        g.insert(m.wurzel, m.clone());

        let leute: Vec<_> = (0..2u8)
            .map(|i| {
                let (sk, pk) = schluesselpaar(i);
                let id = MinerId::aus_schluessel(&pk);
                (sk, pk, id)
            })
            .collect();

        let mut halter: Vec<MinerId> = leute.iter().map(|(_, _, id)| *id).collect();
        halter.sort_unstable();
        let mut z = Zuteilung::default();
        z.je_gegenstand.insert(m.wurzel, halter);
        (g, z, leute)
    }

    fn quittung(
        m: &Manifest,
        sk: &BlsSecretKey,
        id: MinerId,
        epoche: EpochId,
        teil: Option<u32>,
    ) -> Speicherquittung {
        let t = teil.unwrap_or_else(|| verlangter_teil(m, epoche, &id, &SEED));
        let mut q = Speicherquittung::neu(id, epoche, m.wurzel, t, Hash::sha256(b"bytes"));
        q.signiere(sk).expect("signieren");
        q
    }

    fn lauf(
        g: &BTreeMap<MerkleRoot, Manifest>,
        z: &Zuteilung,
        leute: &[Teilnehmer],
        qs: &[Speicherquittung],
    ) -> Epochenbefund {
        let paare: Vec<(MinerId, BlsPublicKey)> =
            leute.iter().map(|(_, pk, id)| (*id, *pk)).collect();
        stichprobenlauf(z, g, qs, EPOCHE, &SEED, &|m: &MinerId| {
            paare.iter().find(|(id, _)| id == m).map(|(_, pk)| *pk)
        })
    }

    #[test]
    fn eine_gueltige_quittung_zaehlt() {
        let (g, z, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        let qs: Vec<_> = leute
            .iter()
            .map(|(sk, _, id)| quittung(m, sk, *id, EPOCHE, None))
            .collect();
        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(b.lieferungen(), 2);
        assert!(b.fehlend.is_empty());
        assert!(b.untauglich.is_empty());
    }

    /// ⚑ **Wer schweigt, fällt ohne Ankläger auf.**
    ///
    /// Die Schuldnerliste entsteht aus der **Zuteilung**, nicht aus den
    /// vorgelegten Quittungen. Wer sie aus den Quittungen ableitete,
    /// fände nie jemanden, der nichts vorlegt: Genau die, um die es
    /// geht, wären unsichtbar.
    #[test]
    fn wer_schweigt_faellt_ohne_anklaeger_auf() {
        let (g, z, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        // Nur der erste quittiert.
        let (sk, _, id) = &leute[0];
        let qs = vec![quittung(m, sk, *id, EPOCHE, None)];

        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(b.lieferungen(), 1);
        assert_eq!(b.fehlend.len(), 1);
        assert_eq!(b.fehlend[0].1, leute[1].2, "der Schweigende fehlt nicht");
        assert!(b.untauglich.is_empty(), "Schweigen ist kein Versuch");
    }

    /// ⚑ **Ein falscher Teil ist untauglich, nicht fehlend.**
    ///
    /// Schweigen kann ein Ausfall sein; ein Versuch mit dem falschen
    /// Teil ist eine Handlung. Beides in einen Topf zu werfen hieße, dem
    /// Abgestürzten dasselbe vorzuwerfen wie dem, der es versucht hat.
    #[test]
    fn ein_falscher_teil_ist_untauglich_und_nicht_bloss_fehlend() {
        let (g, z, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        let (sk, _, id) = &leute[0];
        let verlangt = verlangter_teil(m, EPOCHE, id, &SEED);
        let falsch = (verlangt + 1) % m.teilzahl;
        let qs = vec![quittung(m, sk, *id, EPOCHE, Some(falsch))];

        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(
            b.untauglich,
            vec![(
                m.wurzel,
                *id,
                Untauglich::FalscherTeil { verlangt, quittiert: falsch }
            )]
        );
        // Er zählt zugleich als fehlend, denn geliefert hat er nicht.
        assert!(b.fehlend.iter().any(|(_, h)| h == id));
        assert_eq!(b.lieferungen(), 0);
    }

    #[test]
    fn eine_fremde_epoche_zaehlt_nicht() {
        let (g, z, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        let (sk, _, id) = &leute[0];
        let qs = vec![quittung(m, sk, *id, EpochId(EPOCHE.0 + 1), None)];
        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(b.untauglich[0].2, Untauglich::FremdeEpoche);
        assert_eq!(b.lieferungen(), 0);
    }

    /// Wer nicht zugeteilt ist, quittiert ins Leere.
    #[test]
    fn ein_nicht_zugeteilter_halter_zaehlt_nicht() {
        let (g, z, mut leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        let (sk, pk) = schluesselpaar(9);
        let fremd = MinerId::aus_schluessel(&pk);
        let qs = vec![quittung(m, &sk, fremd, EPOCHE, None)];
        leute.push((sk, pk, fremd));

        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(b.untauglich[0].2, Untauglich::NichtZugeteilt);
        assert_eq!(b.fehlend.len(), 2, "die beiden Zugeteilten fehlen weiterhin");
    }

    /// Unterschrift und Identität, wie bei der Zusage (Fund 96).
    #[test]
    fn eine_quittung_im_namen_eines_anderen_zaehlt_nicht() {
        let (g, z, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        // Schluessel des ersten, Name des zweiten.
        let (sk_a, _, _) = &leute[0];
        let id_b = leute[1].2;
        let t = verlangter_teil(m, EPOCHE, &id_b, &SEED);
        let mut q = Speicherquittung::neu(id_b, EPOCHE, m.wurzel, t, Hash::sha256(b"x"));
        q.signiere(sk_a).expect("signieren");

        let b = lauf(&g, &z, &leute, &[q]);
        assert_eq!(b.untauglich[0].2, Untauglich::Unterschrift);
        assert_eq!(b.lieferungen(), 0);
    }

    #[test]
    fn ein_unbekannter_gegenstand_zaehlt_nicht() {
        let (g, z, leute) = aufbau();
        let fremd = manifest(7);
        let (sk, _, id) = &leute[0];
        let qs = vec![quittung(&fremd, sk, *id, EPOCHE, None)];
        let b = lauf(&g, &z, &leute, &qs);
        assert_eq!(b.untauglich[0].2, Untauglich::UnbekannterGegenstand);
    }

    /// Die Quittung überlebt die Leitung unverändert.
    #[test]
    fn borsh_haelt_die_quittung() {
        let (g, _, leute) = aufbau();
        let m = &g[&MerkleRoot::new([1u8; 32])];
        let (sk, _, id) = &leute[0];
        let q = quittung(m, sk, *id, EPOCHE, None);
        let bytes = borsh::to_vec(&q).expect("serialisieren");
        assert_eq!(q, borsh::from_slice::<Speicherquittung>(&bytes).expect("lesen"));
    }
}
