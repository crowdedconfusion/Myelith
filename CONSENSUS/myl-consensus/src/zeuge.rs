//! **Der Zeuge eines Blocks auf dem Draht** (zustandslose Validierung,
//! Festlegung des Projektinhabers vom 2026-10-29).
//!
//! Ein zustandsloser Knoten übernimmt einen Block nur mit seinem Zeugen
//! (`myl_ledger::state::Zeuge`). Der Zeuge kann bei vollen Blöcken und
//! grossem Zustand grösser sein als eine Gossip-Nachricht; er geht deshalb
//! in **Stücken** von höchstens [`ZEUGENSTUECK_BYTES`].
//!
//! # ⚑ Was die Unterschrift leistet, und was nicht
//!
//! **Die Sicherheit hängt nicht an ihr.** Ein Zeuge belegt sich selbst: Sein
//! Teilbaum muss die Wurzel vor dem Block haben, die der Prüfer selbst hält,
//! und jeder Eintrag muss dazu passen. Ein falscher Zeuge führt nie zu einem
//! falschen Zustand, nur zu einem abgewiesenen Block.
//!
//! **Sie schützt die Lebendigkeit.** Ohne sie könnte jeder ein Stück mit
//! falschem Inhalt verbreiten, schneller als der Erzeuger, und jeder Block
//! wäre für Zustandslose unprüfbar; und jeder könnte das Netz der
//! Zustandslosen mit Stücken fluten. Deshalb unterschreibt der Erzeuger
//! einen [`Zeugenkopf`] mit Höhe, Blockhash und den Hashes aller Stücke;
//! jedes Stück trägt ihn und ist für sich prüfbar. Weitergeleitet wird nur,
//! was ein bekannter Erzeuger unterschrieben hat (das prüft der Knoten).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::{Hash, MinerId};
use std::collections::BTreeMap;

/// Domänentrennung der Unterschrift unter einem Zeugenkopf.
pub const DST_ZEUGENKOPF: &[u8] = b"MYELITH_ZEUGENKOPF_v1";

/// Höchstens so gross ist ein Stück (1 MiB; die Gossip-Grenze liegt bei 4).
pub const ZEUGENSTUECK_BYTES: usize = 1 << 20;

/// Höchstens so viele Stücke hat ein Zeuge (48 MiB). Gemessen: 1,7 MB bei
/// einer Million Konten und 2000 Überweisungen je Block.
pub const ZEUGENSTUECKE_HOECHSTENS: usize = 48;

/// Was der Erzeuger zu einem Zeugen unterschreibt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Zeugenkopf {
    /// Die Höhe des Blocks.
    pub hoehe: u64,
    /// Der Hash des Blocks.
    pub blockhash: Hash,
    /// SHA-256 je Stück, in ihrer Folge.
    pub stuecke: Vec<Hash>,
    /// Der Schlüssel des Erzeugers.
    pub schluessel: BlsPublicKey,
    /// Die Unterschrift über [`Zeugenkopf::nachricht`].
    pub signatur: BlsSignature,
}

impl Zeugenkopf {
    /// `DST_ZEUGENKOPF ‖ Höhe ‖ Blockhash ‖ Borsh(Stückhashes)`.
    pub fn nachricht(hoehe: u64, blockhash: &Hash, stuecke: &[Hash]) -> Vec<u8> {
        let mut m = DST_ZEUGENKOPF.to_vec();
        m.extend_from_slice(&hoehe.to_le_bytes());
        m.extend_from_slice(blockhash.as_bytes());
        m.extend_from_slice(&borsh::to_vec(stuecke).expect("in einen Vec"));
        m
    }

    /// Der Unterzeichner.
    pub fn erzeuger(&self) -> MinerId {
        MinerId::aus_schluessel(&self.schluessel)
    }

    /// Ob der Kopf wohlgeformt ist und die Unterschrift unter seinem
    /// Schlüssel trägt. Ob der Schlüssel der eines Erzeugers ist, prüft der
    /// Aufrufer.
    pub fn traegt(&self) -> bool {
        (1..=ZEUGENSTUECKE_HOECHSTENS).contains(&self.stuecke.len())
            && self.schluessel.verify(&Self::nachricht(self.hoehe, &self.blockhash, &self.stuecke), &self.signatur)
    }

    /// Der Hash des Kopfes (für Zwischenspeicher).
    pub fn kennung(&self) -> Hash {
        Hash::sha256(&borsh::to_vec(self).expect("in einen Vec"))
    }
}

/// Ein Stück eines Zeugen, mit dem Kopf, der es belegt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Zeugenstueck {
    pub kopf: Zeugenkopf,
    /// Die Stelle des Stücks im Kopf.
    pub nr: u16,
    pub bytes: Vec<u8>,
}

impl Zeugenstueck {
    /// Ob das Stück zu seinem Kopf passt (ohne die Unterschrift, siehe
    /// [`Zeugenkopf::traegt`]).
    pub fn passt(&self) -> bool {
        self.bytes.len() <= ZEUGENSTUECK_BYTES
            && self.kopf.stuecke.get(usize::from(self.nr)) == Some(&Hash::sha256(&self.bytes))
    }
}

/// ⚑ **Zerlegt einen Zeugen** in unterschriebene Stücke. `signieren`
/// unterschreibt eine Nachricht mit dem Schlüssel `schluessel`. `None`, wenn
/// der Zeuge zu gross ist oder die Unterschrift misslingt.
pub fn zerlegen(
    hoehe: u64,
    blockhash: Hash,
    zeuge: &[u8],
    schluessel: BlsPublicKey,
    signieren: impl FnOnce(&[u8]) -> Option<BlsSignature>,
) -> Option<Vec<Zeugenstueck>> {
    let teile: Vec<&[u8]> = if zeuge.is_empty() { vec![zeuge] } else { zeuge.chunks(ZEUGENSTUECK_BYTES).collect() };
    if teile.len() > ZEUGENSTUECKE_HOECHSTENS {
        return None;
    }
    let stuecke: Vec<Hash> = teile.iter().map(|t| Hash::sha256(t)).collect();
    let signatur = signieren(&Zeugenkopf::nachricht(hoehe, &blockhash, &stuecke))?;
    let kopf = Zeugenkopf { hoehe, blockhash, stuecke, schluessel, signatur };
    Some(
        teile
            .into_iter()
            .enumerate()
            .map(|(nr, t)| Zeugenstueck { kopf: kopf.clone(), nr: nr as u16, bytes: t.to_vec() })
            .collect(),
    )
}

/// Ein Zeuge im Zusammensetzen.
#[derive(Debug)]
struct Offen {
    kopf: Zeugenkopf,
    stuecke: Vec<Option<Vec<u8>>>,
    bytes: usize,
}

/// ⚑ **Setzt Zeugen aus Stücken zusammen, begrenzt.** Nimmt nur Stücke, die
/// der Aufrufer schon geprüft hat (Unterschrift, bekannter Erzeuger).
/// Begrenzt sind die Bytes insgesamt (das älteste Unfertige fällt zuerst),
/// die Köpfe je Höhe ([`KOEPFE_JE_HOEHE`]) und je Höhe und Unterzeichner
/// einer, auch wenn der Block bekannt ist. **Ist der Block einer Höhe
/// bekannt** ([`Self::erwarten`]), gilt dort nur noch sein Kopf, und der
/// bekommt immer Platz: Wer vorher Köpfe mit Unsinn unterschrieb, verdrängt
/// den echten nicht.
#[derive(Debug)]
pub struct Zeugensammler {
    offen: BTreeMap<Hash, Offen>,
    /// Je Höhe der Hash des bekannten Blocks.
    erwartet: BTreeMap<u64, Hash>,
    /// Folge der Aufnahme, für die Verdrängung.
    folge: Vec<Hash>,
    bytes: usize,
    hoechstens_bytes: usize,
}

/// Höchstens so viele Köpfe je Höhe sammelt ein Knoten zugleich.
pub const KOEPFE_JE_HOEHE: usize = 4;

impl Zeugensammler {
    /// Mit einer Grenze für alles Gesammelte.
    pub fn neu(hoechstens_bytes: usize) -> Self {
        Self { offen: BTreeMap::new(), erwartet: BTreeMap::new(), folge: Vec::new(), bytes: 0, hoechstens_bytes }
    }

    /// Der Block der Höhe `hoehe` ist bekannt (geprüft angekommen): Nur noch
    /// Stücke zu ihm zählen dort.
    pub fn erwarten(&mut self, hoehe: u64, blockhash: Hash) {
        self.erwartet.insert(hoehe, blockhash);
        let weg: Vec<Hash> =
            self.offen.iter().filter(|(_, o)| o.kopf.hoehe == hoehe && o.kopf.blockhash != blockhash).map(|(k, _)| *k).collect();
        for k in weg {
            self.entfernen(&k);
        }
    }

    fn entfernen(&mut self, k: &Hash) {
        if let Some(o) = self.offen.remove(k) {
            self.bytes -= o.bytes;
        }
        self.folge.retain(|f| f != k);
    }

    /// Nimmt ein geprüftes Stück auf; ist der Zeuge damit vollständig, kommt
    /// er heraus (Höhe, Blockhash, Bytes, Unterzeichner) und wird nicht mehr
    /// gehalten.
    pub fn aufnehmen(&mut self, s: Zeugenstueck) -> Option<(u64, Hash, Vec<u8>, BlsPublicKey)> {
        if !s.passt() {
            return None;
        }
        let k = s.kopf.kennung();
        let erwartet = self.erwartet.get(&s.kopf.hoehe).copied();
        if erwartet.is_some_and(|b| b != s.kopf.blockhash) {
            return None;
        }
        if !self.offen.contains_key(&k) {
            let gleiche: Vec<&Offen> = self.offen.values().filter(|o| o.kopf.hoehe == s.kopf.hoehe).collect();
            let derselbe = gleiche.iter().any(|o| o.kopf.schluessel == s.kopf.schluessel);
            // ⚑ Je Unterzeichner ein Kopf, auch für einen bekannten Block
            //   (2026-10-30): Seit auch volle Knoten Zeugen liefern, könnte
            //   einer sonst beliebig viele Köpfe zum echten Blockhash
            //   unterschreiben und den echten aus der Bytegrenze drängen.
            if derselbe || (erwartet.is_none() && gleiche.len() >= KOEPFE_JE_HOEHE) {
                return None;
            }
            let n = s.kopf.stuecke.len();
            self.offen.insert(k, Offen { kopf: s.kopf.clone(), stuecke: vec![None; n], bytes: 0 });
            self.folge.push(k);
        }
        let o = self.offen.get_mut(&k)?;
        let platz = o.stuecke.get_mut(usize::from(s.nr))?;
        if platz.is_none() {
            o.bytes += s.bytes.len();
            self.bytes += s.bytes.len();
            *platz = Some(s.bytes);
        }
        if o.stuecke.iter().all(Option::is_some) {
            let o = self.offen.remove(&k)?;
            self.folge.retain(|f| *f != k);
            self.bytes -= o.bytes;
            self.erwartet.remove(&o.kopf.hoehe);
            let zeuge: Vec<u8> = o.stuecke.into_iter().flatten().flatten().collect();
            return Some((o.kopf.hoehe, o.kopf.blockhash, zeuge, o.kopf.schluessel));
        }
        // Über der Grenze fällt das älteste Unfertige, das nicht erwartet
        // ist; erst wenn es keines mehr gibt, auch Erwartetes.
        while self.bytes > self.hoechstens_bytes && !self.folge.is_empty() {
            let alt = self
                .folge
                .iter()
                .find(|f| self.offen.get(*f).is_some_and(|o| self.erwartet.get(&o.kopf.hoehe) != Some(&o.kopf.blockhash)))
                .copied()
                .unwrap_or(self.folge[0]);
            if alt == k && self.folge.len() == 1 {
                break;
            }
            self.entfernen(&alt);
        }
        None
    }

    /// Vergisst alles bis einschliesslich `hoehe` (übernommen oder zu alt).
    pub fn bis_vergessen(&mut self, hoehe: u64) {
        let weg: Vec<Hash> = self.offen.iter().filter(|(_, o)| o.kopf.hoehe <= hoehe).map(|(k, _)| *k).collect();
        for k in weg {
            self.entfernen(&k);
        }
        self.erwartet.retain(|h, _| *h > hoehe);
    }

    /// Die noch fehlenden Stücke des Zeugen zu `blockhash`, wenn schon eines
    /// da ist (sonst ist die Zahl unbekannt).
    ///
    /// ⚑ **Über alle Köpfe zu diesem Block** (2026-10-30): Seit auch volle
    /// Knoten Zeugen liefern, kann ein Block mehrere Köpfe haben, und ihre
    /// Stückzahl kann abweichen. Nur die Lücken des ersten zu fragen hiesse:
    /// Schweigt sein Unterzeichner, wird ein anderer nie fertig.
    pub fn fehlende(&self, blockhash: &Hash) -> Option<Vec<u16>> {
        let mut alle: Vec<u16> = Vec::new();
        let mut da = false;
        for o in self.offen.values().filter(|o| o.kopf.blockhash == *blockhash) {
            da = true;
            alle.extend(o.stuecke.iter().enumerate().filter(|(_, s)| s.is_none()).map(|(i, _)| i as u16));
        }
        alle.sort_unstable();
        alle.dedup();
        da.then_some(alle)
    }

    /// Die gehaltenen Bytes.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSecretKey;

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b; 32]).expect("Schlüssel")
    }

    fn stuecke(sk: &BlsSecretKey, hoehe: u64, zeuge: &[u8]) -> Vec<Zeugenstueck> {
        zerlegen(hoehe, Hash::sha256(&hoehe.to_le_bytes()), zeuge, sk.public_key().unwrap(), |m| sk.sign(m).ok())
            .expect("zerlegt")
    }

    /// ⚑ **Zerlegt, geprüft, zusammengesetzt**, auch in fremder Folge; ein
    /// verändertes Stück passt nicht, ein Kopf mit fremder Unterschrift
    /// trägt nicht.
    #[test]
    fn zerlegt_geprueft_zusammengesetzt() {
        let sk = schluessel(1);
        let zeuge: Vec<u8> = (0..(ZEUGENSTUECK_BYTES * 2 + 17)).map(|i| (i % 251) as u8).collect();
        let mut s = stuecke(&sk, 5, &zeuge);
        assert_eq!(s.len(), 3);
        assert!(s.iter().all(|t| t.passt() && t.kopf.traegt()));
        assert_eq!(s[0].kopf.erzeuger(), MinerId::aus_schluessel(&sk.public_key().unwrap()));
        let mut falsch = s[1].clone();
        falsch.bytes[0] ^= 1;
        assert!(!falsch.passt());
        let mut fremd = s[1].clone();
        fremd.kopf.signatur = schluessel(2).sign(b"anderes").unwrap();
        assert!(!fremd.kopf.traegt());
        let mut sammler = Zeugensammler::neu(64 << 20);
        s.reverse();
        assert!(sammler.aufnehmen(falsch).is_none());
        let mut fertig = None;
        for t in s {
            fertig = sammler.aufnehmen(t).or(fertig);
        }
        let (hoehe, _, bytes, _) = fertig.expect("vollständig");
        assert_eq!((hoehe, bytes), (5, zeuge));
        assert_eq!(sammler.bytes(), 0);
    }

    /// ⚑ **Begrenzt**: höchstens [`KOEPFE_JE_HOEHE`] Köpfe je Höhe, und über
    /// der Bytegrenze fällt das älteste Unfertige.
    #[test]
    fn der_sammler_ist_begrenzt() {
        let sk = schluessel(3);
        let mut sammler = Zeugensammler::neu(3 * ZEUGENSTUECK_BYTES);
        let gross = vec![7u8; ZEUGENSTUECK_BYTES * 2];
        for i in 0..(KOEPFE_JE_HOEHE as u8 + 2) {
            let mut z = gross.clone();
            z[0] = i;
            let s = zerlegen(9, Hash::sha256(&[i]), &z, sk.public_key().unwrap(), |m| sk.sign(m).ok()).unwrap();
            let _ = sammler.aufnehmen(s[0].clone());
        }
        assert!(sammler.offen.len() <= KOEPFE_JE_HOEHE);
        assert!(sammler.bytes() <= 3 * ZEUGENSTUECK_BYTES, "{}", sammler.bytes());
        sammler.bis_vergessen(9);
        assert_eq!(sammler.bytes(), 0);
        // ⛔️ Je Höhe und Unterzeichner einer; vier fremde Köpfe füllen die
        //    Höhe, doch ist der Block bekannt, bekommt sein Kopf Platz.
        let mut sammler = Zeugensammler::neu(64 << 20);
        let echt = stuecke(&sk, 11, b"echt");
        let zweiteilig = |b: &[u8]| {
            zerlegen(11, Hash::sha256(b), &vec![1; ZEUGENSTUECK_BYTES + 1], sk.public_key().unwrap(), |m| sk.sign(m).ok()).unwrap()
        };
        let _ = sammler.aufnehmen(zweiteilig(b"a")[0].clone());
        let _ = sammler.aufnehmen(zweiteilig(b"b")[0].clone());
        assert_eq!(sammler.offen.len(), 1, "derselbe Unterzeichner, dieselbe Höhe");
        sammler.bis_vergessen(11);
        for b in 20..24u8 {
            let fremd = schluessel(b);
            let mut s = zerlegen(11, Hash::sha256(&[b]), &vec![b; ZEUGENSTUECK_BYTES + 1], fremd.public_key().unwrap(), |m| fremd.sign(m).ok()).unwrap();
            let _ = sammler.aufnehmen(s.remove(0));
        }
        assert_eq!(sammler.offen.len(), KOEPFE_JE_HOEHE);
        sammler.erwarten(11, echt[0].kopf.blockhash);
        assert_eq!(sammler.offen.len(), 0, "die fremden Köpfe fallen");
        assert_eq!(sammler.aufnehmen(echt[0].clone()).map(|(h, _, z, _)| (h, z)), Some((11, b"echt".to_vec())));
        // Auch für den bekannten Block nur ein Kopf je Unterzeichner.
        let bekannt = Hash::sha256(b"bekannt");
        sammler.erwarten(12, bekannt);
        for b in [b"a", b"b"] {
            let mut z = vec![1u8; ZEUGENSTUECK_BYTES + 1];
            z[0] = b[0];
            let _ = sammler.aufnehmen(zerlegen(12, bekannt, &z, sk.public_key().unwrap(), |m| sk.sign(m).ok()).unwrap().remove(0));
        }
        assert_eq!(sammler.offen.len(), 1, "derselbe Unterzeichner, bekannter Block");
        // Ein zweiter Unterzeichner mit drei Stücken: Es fehlen die Lücken
        // beider Köpfe, nicht nur die des ersten.
        let zweiter = schluessel(30);
        let _ = sammler.aufnehmen(
            zerlegen(12, bekannt, &vec![2u8; 2 * ZEUGENSTUECK_BYTES + 1], zweiter.public_key().unwrap(), |m| zweiter.sign(m).ok())
                .unwrap()
                .remove(0),
        );
        assert_eq!(sammler.fehlende(&bekannt), Some(vec![1, 2]));
        assert!(zerlegen(1, Hash::sha256(b"x"), &vec![0u8; ZEUGENSTUECK_BYTES * (ZEUGENSTUECKE_HOECHSTENS + 1)], sk.public_key().unwrap(), |m| sk.sign(m).ok()).is_none());
    }
}
