//! **Die Zeugen auf der Platte** (2026-10-30).
//!
//! Ein zustandsloser Knoten, der zurückfällt, steigt über einen leichten
//! Abzug am jüngsten bestätigten Prüfpunkt wieder ein und braucht dann die
//! Zeugen **ab dort**. Der Prüfpunkt liegt 300 bis 400 Blöcke hinter der
//! Spitze. Im Speicher hielten Erzeuger und Zeugenhalter bis hierher nur
//! 64 MiB; bei grossen Zeugen (unter Last mehrere hundert KB je Block) sind
//! das rund 100 Blöcke.
//!
//! 📌 Im langen Podlauf `nacht-bereit` hielt danach niemand mehr den Zeugen
//! des Blocks nach dem Prüfpunkt: Der zustandslose Miner sprang eine Stunde
//! lang alle 30 s per leichtem Abzug und prüfte keinen Block.
//!
//! ⚑ **Ein Fenster nach Höhe, nicht nach Bytes**: [`ZEUGEN_VORHALT_BLOECKE`],
//! drei Prüfpunktabstände. Vorbild NEAR hält Zeugen nur für die Spitze und
//! setzt beim Rückstand auf State Sync; unser Gegenstück, der leichte
//! Abzug, verankert am Prüfpunkt und braucht deshalb das Fenster bis zur
//! Spitze. [`ZEUGEN_PLATTE_HOECHSTENS_BYTES`] bleibt als Notbremse.
//!
//! ⚑ **Ein Zwischenspeicher**: Beim Start wird der Ordner geleert, eine
//! Datei je Block, ohne Schreibprotokoll. Was hier liegt, prüft der
//! Empfänger wie jedes andere Stück (Unterschrift, Kopf, Wurzel); eine
//! beschädigte Datei kostet nur eine Antwort. Die Dateinamen entstehen
//! allein aus Höhe und Hash.

use borsh::BorshDeserialize;
use myl_consensus::zeuge::Zeugenstueck;
use myl_types::Hash;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// So viele Blöcke hinter dem jüngsten bleiben die Zeugen auf der Platte.
pub const ZEUGEN_VORHALT_BLOECKE: u64 = 3 * myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT;

/// Höchstens so viele Bytes, gleich was das Fenster verlangt.
pub const ZEUGEN_PLATTE_HOECHSTENS_BYTES: u64 = 4 << 30;

/// Die unterschriebenen Stücke der Zeugen eines Fensters, je Block eine
/// Datei.
#[derive(Debug)]
pub struct Zeugenablage {
    ordner: PathBuf,
    eintraege: BTreeMap<(u64, Hash), u64>,
    je_hash: BTreeMap<Hash, u64>,
    bytes: u64,
    hoechstens_bytes: u64,
}

impl Zeugenablage {
    /// Leert `ordner` (oder legt ihn an) und beginnt neu.
    pub fn neu(ordner: PathBuf) -> std::io::Result<Self> {
        Self::mit_grenze(ordner, ZEUGEN_PLATTE_HOECHSTENS_BYTES)
    }

    /// Wie [`Self::neu`] mit einer anderen Bytegrenze (für Tests).
    pub fn mit_grenze(ordner: PathBuf, hoechstens_bytes: u64) -> std::io::Result<Self> {
        if ordner.exists() {
            std::fs::remove_dir_all(&ordner)?;
        }
        std::fs::create_dir_all(&ordner)?;
        Ok(Self { ordner, eintraege: BTreeMap::new(), je_hash: BTreeMap::new(), bytes: 0, hoechstens_bytes })
    }

    fn pfad(&self, hoehe: u64, hash: &Hash) -> PathBuf {
        self.ordner.join(format!("{hoehe:012}-{}.zeuge", hash.to_hex()))
    }

    /// Legt die Stücke eines Blocks ab; danach fällt, was aus dem Fenster
    /// oder über die Grenze ist, das Älteste zuerst.
    pub fn ablegen(&mut self, hoehe: u64, hash: Hash, stuecke: &[Zeugenstueck]) -> std::io::Result<()> {
        if self.je_hash.contains_key(&hash) {
            return Ok(());
        }
        let roh = borsh::to_vec(stuecke)?;
        std::fs::write(self.pfad(hoehe, &hash), &roh)?;
        self.eintraege.insert((hoehe, hash), roh.len() as u64);
        self.je_hash.insert(hash, hoehe);
        self.bytes += roh.len() as u64;
        let neueste = self.eintraege.keys().next_back().map_or(0, |(h, _)| *h);
        while let Some((&(h, k), &n)) = self.eintraege.iter().next() {
            let zu_alt = h.saturating_add(ZEUGEN_VORHALT_BLOECKE) < neueste;
            let zu_viel = self.bytes > self.hoechstens_bytes && self.eintraege.len() > 1;
            if !zu_alt && !zu_viel {
                break;
            }
            self.eintraege.remove(&(h, k));
            self.je_hash.remove(&k);
            self.bytes -= n;
            let _ = std::fs::remove_file(self.pfad(h, &k));
        }
        Ok(())
    }

    /// Die Stücke zu `hash`, wenn sie noch im Fenster liegen und sich ganz
    /// lesen lassen.
    pub fn lesen(&self, hash: &Hash) -> Option<Vec<Zeugenstueck>> {
        let hoehe = *self.je_hash.get(hash)?;
        let roh = std::fs::read(self.pfad(hoehe, hash)).ok()?;
        let mut rest = &roh[..];
        let stuecke = Vec::<Zeugenstueck>::deserialize(&mut rest).ok()?;
        rest.is_empty().then_some(stuecke)
    }

    /// Die belegten Bytes.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Wie viele Blöcke gehalten werden.
    pub fn anzahl(&self) -> usize {
        self.eintraege.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordner(marke: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("myl-zeugenablage-{marke}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn stuecke(hoehe: u64, groesse: usize) -> (Hash, Vec<Zeugenstueck>) {
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[9u8; 32]).unwrap();
        let hash = Hash::sha256(&hoehe.to_le_bytes());
        let s = myl_consensus::zeuge::zerlegen(hoehe, hash, &vec![(hoehe % 251) as u8; groesse], sk.public_key().unwrap(), |m| {
            sk.sign(m).ok()
        })
        .unwrap();
        (hash, s)
    }

    /// ⚑ Das Fenster nach Höhe: Was mehr als [`ZEUGEN_VORHALT_BLOECKE`]
    /// hinter dem jüngsten liegt, fällt; der Rest liest sich unverändert.
    #[test]
    fn das_fenster_folgt_der_hoehe() {
        let o = ordner("fenster");
        let mut a = Zeugenablage::neu(o.clone()).unwrap();
        let mut hashes = Vec::new();
        for h in 1..=ZEUGEN_VORHALT_BLOECKE + 50 {
            let (hash, s) = stuecke(h, 64);
            a.ablegen(h, hash, &s).unwrap();
            hashes.push((h, hash, s));
        }
        assert_eq!(a.anzahl() as u64, ZEUGEN_VORHALT_BLOECKE + 1);
        for (h, hash, s) in &hashes {
            let gelesen = a.lesen(hash);
            if *h < 50 {
                assert!(gelesen.is_none(), "Höhe {h} liegt vor dem Fenster");
            } else {
                assert_eq!(gelesen.as_ref(), Some(s), "Höhe {h}");
            }
        }
        assert_eq!(std::fs::read_dir(&o).unwrap().count(), a.anzahl(), "die Dateien folgen");
        // Ein Neustart leert den Ordner.
        let a = Zeugenablage::neu(o.clone()).unwrap();
        assert_eq!((a.anzahl(), std::fs::read_dir(&o).unwrap().count()), (0, 0));
        std::fs::remove_dir_all(&o).ok();
    }

    /// ⛔️ Die Bytegrenze greift auch innerhalb des Fensters, das Älteste
    /// zuerst; eine beschädigte Datei liest sich als nichts.
    #[test]
    fn die_grenze_und_eine_beschaedigte_datei() {
        let o = ordner("grenze");
        let mut a = Zeugenablage::mit_grenze(o.clone(), 10_000).unwrap();
        let mut letzte = None;
        for h in 1..=20 {
            let (hash, s) = stuecke(h, 2_000);
            a.ablegen(h, hash, &s).unwrap();
            letzte = Some((h, hash));
        }
        assert!(a.bytes() <= 10_000, "{}", a.bytes());
        assert!(a.anzahl() < 20);
        let (h, hash) = letzte.unwrap();
        assert!(a.lesen(&hash).is_some());
        let pfad = o.join(format!("{h:012}-{}.zeuge", hash.to_hex()));
        let mut roh = std::fs::read(&pfad).unwrap();
        roh.push(0);
        std::fs::write(&pfad, &roh).unwrap();
        assert!(a.lesen(&hash).is_none(), "mit Anhängsel");
        std::fs::remove_dir_all(&o).ok();
    }
}
