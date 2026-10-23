//! **Der Zustandsabzug**: Ein Neuling beginnt bei einer Epochengrenze statt
//! bei Block 0 (Festlegung des Projektinhabers vom 2026-10-22).
//!
//! # ⚑ Was ein Abzug ist
//!
//! Der Block bei Höhe H und die Borsh-Form des Kettenzustands nach ihm. Mehr
//! braucht die Anwendung der folgenden Blöcke nicht: Sie liest außer dem
//! Zustand nur Höhe, Vorgängerhash und die Saatquelle im Kopf des nächsten
//! Blocks. Die Zustandswurzel im Kopf von H ist SHA-256 über genau diese
//! Bytes; der Zustand wird deshalb im Ganzen geprüft, nicht stückweise.
//!
//! # ⚑ Wem der Neuling glaubt: dem Anker, nicht der Gegenstelle
//!
//! Blöcke tragen heute keine Unterschrift, eine Kette von Köpfen belegt ohne
//! Nachrechnen nichts. Geglaubt wird deshalb nur ein **Anker** (Höhe und
//! Blockhash), der von außerhalb des Netzes kommt: aus dem Quelltext
//! ([`crate::netz::Netz::abzuege`]) oder vom Betreiber (`--abzug`). Die
//! Gegenstelle liefert nur Bytes, und die müssen zum Anker passen: Der Block
//! hat genau diesen Hash, der Zustand genau die Wurzel in seinem Kopf. Eine
//! Mehrheit von Gegenstellen wäre kein Anker, denn Gegenstellen kosten
//! nichts.
//!
//! # ⚑ Welche Abzüge ein Knoten behält
//!
//! Die letzten zwei Epochengrenzen, und dazu jede [`LANGZEIT_EPOCHEN`]-te auf
//! Dauer. Ein Anker in einem ausgelieferten Programm zeigt auf eine solche
//! dauerhafte Höhe; zeigte er auf eine gewöhnliche, könnte ihn zwei Epochen
//! später niemand mehr liefern.

use std::path::{Path, PathBuf};

use myl_consensus::block::{Block, BLOECKE_JE_EPOCHE};
use myl_ledger::state::LedgerState;
use myl_types::hash::Hash;

/// Jede so vielte Epochengrenze bleibt als Abzug auf Dauer liegen.
pub const LANGZEIT_EPOCHEN: u64 = 64;
/// So viele Bytes des Zustands trägt ein Stück einer Lieferung. Weit unter
/// der Grenze einer Antwort, damit Block und Rahmen daneben Platz haben.
pub const STUECK_BYTES: u64 = 1024 * 1024;
/// Größer darf ein angekündigter Zustand nicht sein, bevor Speicher belegt
/// wird: Die Größe kommt von einer Gegenstelle.
pub const HOECHSTE_GROESSE: u64 = 1024 * 1024 * 1024;
/// Der Kopf der Datei eines Abzugs.
const MAGIE: &[u8; 8] = b"MYLABZUG";
const FASSUNG: u16 = 1;

/// Woran ein Neuling einen Abzug misst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anker {
    /// Die Höhe des Blocks, nach dem der Zustand gilt.
    pub hoehe: u64,
    /// Der Hash dieses Blocks.
    pub blockhash: Hash,
}

impl Anker {
    /// `<hoehe>:<64 hex>`, wie auf der Kommandozeile.
    pub fn aus_text(text: &str) -> Result<Self, String> {
        let (h, x) = text.split_once(':').ok_or("erwartet <hoehe>:<blockhash>")?;
        let hoehe: u64 = h.parse().map_err(|_| format!("keine Höhe: {h}"))?;
        if hoehe == 0 || !hoehe.is_multiple_of(BLOECKE_JE_EPOCHE) {
            return Err(format!("{hoehe} ist keine Epochengrenze (Vielfaches von {BLOECKE_JE_EPOCHE})"));
        }
        if x.len() != 64 || !x.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("der Blockhash braucht 64 Hexziffern".into());
        }
        let mut b = [0u8; 32];
        for (i, z) in b.iter_mut().enumerate() {
            *z = u8::from_str_radix(&x[2 * i..2 * i + 2], 16).map_err(|_| "keine Hexziffer")?;
        }
        Ok(Self { hoehe, blockhash: Hash::from_bytes(b) })
    }
}

/// Ob an dieser Höhe ein Abzug entsteht: an jeder Epochengrenze.
pub fn ist_grenze(hoehe: u64) -> bool {
    hoehe > 0 && hoehe.is_multiple_of(BLOECKE_JE_EPOCHE)
}

/// Ob ein Abzug dieser Höhe auf Dauer bleibt.
pub fn ist_langzeit(hoehe: u64) -> bool {
    ist_grenze(hoehe) && (hoehe / BLOECKE_JE_EPOCHE).is_multiple_of(LANGZEIT_EPOCHEN)
}

/// Ein Abzug: der Block bei H und der Zustand nach ihm, als Borsh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abzug {
    /// Der Block bei H.
    pub block: Block,
    /// Borsh des Zustands nach H.
    pub zustand: Vec<u8>,
}

/// Warum ein Abzug nicht gilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbzugFehler {
    /// Der Block steht nicht auf der Höhe des Ankers.
    FalscheHoehe { anker: u64, block: u64 },
    /// Der Block hat nicht den Hash des Ankers.
    FremderBlock,
    /// Die Bytes haben nicht die Wurzel im Kopf des Blocks.
    FalscheWurzel,
    /// Die Bytes sind kein Zustand, oder keiner in kanonischer Form.
    KeinZustand,
    /// Die Datei ist kaputt, gehört zu einem anderen Netz oder Format.
    Datei(String),
}

impl std::fmt::Display for AbzugFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FalscheHoehe { anker, block } => write!(f, "Block auf Höhe {block}, der Anker nennt {anker}"),
            Self::FremderBlock => write!(f, "der Block hat nicht den Hash des Ankers"),
            Self::FalscheWurzel => write!(f, "der Zustand hat nicht die Wurzel im Kopf des Blocks"),
            Self::KeinZustand => write!(f, "die Bytes sind kein Zustand in kanonischer Form"),
            Self::Datei(g) => write!(f, "Datei des Abzugs: {g}"),
        }
    }
}

impl std::error::Error for AbzugFehler {}

impl Abzug {
    /// Die Höhe des Abzugs.
    pub fn hoehe(&self) -> u64 {
        self.block.header.height
    }

    /// **Prüft den Abzug gegen den Anker** und gibt den Zustand zurück.
    ///
    /// Der Reihe nach, das Billige vor dem Teuren: Höhe, Blockhash, Wurzel
    /// über die Bytes, erst dann das Lesen. Und gelesen wird nur, was sich
    /// in derselben Form zurückschreibt: Zwei Bytefolgen für einen Zustand
    /// wären zwei Wurzeln für dieselbe Sache.
    pub fn pruefen(&self, anker: &Anker) -> Result<LedgerState, AbzugFehler> {
        if self.hoehe() != anker.hoehe {
            return Err(AbzugFehler::FalscheHoehe { anker: anker.hoehe, block: self.hoehe() });
        }
        if self.block.hash() != anker.blockhash {
            return Err(AbzugFehler::FremderBlock);
        }
        self.pruefen_gegen_block()
    }

    /// Prüft nur Wurzel und Form, gegen den eigenen Block. Für Abzüge, die
    /// dieser Knoten selbst angelegt hat oder deren Block er schon kennt.
    pub fn pruefen_gegen_block(&self) -> Result<LedgerState, AbzugFehler> {
        if Hash::sha256(&self.zustand) != self.block.header.state_root {
            return Err(AbzugFehler::FalscheWurzel);
        }
        let zustand: LedgerState = borsh::from_slice(&self.zustand).map_err(|_| AbzugFehler::KeinZustand)?;
        if zustand.commitment() != self.block.header.state_root {
            return Err(AbzugFehler::KeinZustand);
        }
        Ok(zustand)
    }

    /// Die Datei: Kopf, Startwert, Block, Zustand, und SHA-256 über alles
    /// davor. Die Prüfsumme findet einen halb geschriebenen Abzug; gegen
    /// Fälschung steht der Anker, nicht die Summe.
    pub fn als_datei(&self, startwert: &Hash) -> Vec<u8> {
        let block = borsh::to_vec(&self.block).expect("ein Block ist stets serialisierbar");
        let mut d = Vec::with_capacity(64 + block.len() + self.zustand.len());
        d.extend_from_slice(MAGIE);
        d.extend_from_slice(&FASSUNG.to_le_bytes());
        d.extend_from_slice(startwert.as_bytes());
        d.extend_from_slice(&(block.len() as u64).to_le_bytes());
        d.extend_from_slice(&block);
        d.extend_from_slice(&(self.zustand.len() as u64).to_le_bytes());
        d.extend_from_slice(&self.zustand);
        let summe = Hash::sha256(&d);
        d.extend_from_slice(summe.as_bytes());
        d
    }

    /// Liest eine Datei aus [`Abzug::als_datei`] und prüft Wurzel und Form.
    pub fn aus_datei(d: &[u8], startwert: &Hash) -> Result<(Self, LedgerState), AbzugFehler> {
        let kaputt = |g: &str| AbzugFehler::Datei(g.to_string());
        if d.len() < 8 + 2 + 32 + 8 + 8 + 32 || &d[..8] != MAGIE {
            return Err(kaputt("kein Abzug"));
        }
        let (inhalt, summe) = d.split_at(d.len() - 32);
        if Hash::sha256(inhalt).as_bytes() != summe {
            return Err(kaputt("Prüfsumme passt nicht (abgebrochen geschrieben?)"));
        }
        if u16::from_le_bytes([d[8], d[9]]) != FASSUNG {
            return Err(kaputt("unbekannte Fassung"));
        }
        if &d[10..42] != startwert.as_bytes() {
            return Err(kaputt("Abzug eines anderen Netzes"));
        }
        let mut pos = 42usize;
        let laenge = |pos: &mut usize| -> Result<usize, AbzugFehler> {
            let b: [u8; 8] = inhalt.get(*pos..*pos + 8).ok_or_else(|| kaputt("zu kurz"))?.try_into().expect("8 Bytes");
            *pos += 8;
            usize::try_from(u64::from_le_bytes(b)).map_err(|_| kaputt("Länge"))
        };
        let lb = laenge(&mut pos)?;
        let block: Block = borsh::from_slice(inhalt.get(pos..pos + lb).ok_or_else(|| kaputt("Block zu kurz"))?)
            .map_err(|_| kaputt("kein Block"))?;
        pos += lb;
        let lz = laenge(&mut pos)?;
        let zustand = inhalt.get(pos..pos + lz).ok_or_else(|| kaputt("Zustand zu kurz"))?.to_vec();
        if pos + lz != inhalt.len() {
            return Err(kaputt("Bytes hinter dem Zustand"));
        }
        let abzug = Self { block, zustand };
        let z = abzug.pruefen_gegen_block()?;
        Ok((abzug, z))
    }
}

/// **Die Ablage der Abzüge** in einem Ordner neben der Kettendatei:
/// `abzug-<hoehe>.dat`, geschrieben über eine Zwischendatei, damit ein
/// Absturz keinen halben Abzug hinterlässt, der wie ein ganzer aussieht.
#[derive(Debug, Clone)]
pub struct Ablage {
    ordner: PathBuf,
    startwert: Hash,
}

impl Ablage {
    /// Die Ablage neben einer Kettendatei.
    pub fn neben(kettendatei: &Path, startwert: Hash) -> Self {
        let ordner = kettendatei.with_file_name(format!(
            "{}.abzuege",
            kettendatei.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "kette".into())
        ));
        Self { ordner, startwert }
    }

    fn pfad(&self, hoehe: u64) -> PathBuf {
        self.ordner.join(format!("abzug-{hoehe}.dat"))
    }

    /// Legt einen Abzug ab und räumt nach der Behaltensregel auf.
    pub fn ablegen(&self, abzug: &Abzug) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.ordner)?;
        let ziel = self.pfad(abzug.hoehe());
        let zwischen = ziel.with_extension("teil");
        std::fs::write(&zwischen, abzug.als_datei(&self.startwert))?;
        std::fs::rename(&zwischen, &ziel)?;
        self.aufraeumen()
    }

    /// Die Höhen aller abgelegten Abzüge, aufsteigend.
    pub fn hoehen(&self) -> Vec<u64> {
        let mut h: Vec<u64> = std::fs::read_dir(&self.ordner)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_prefix("abzug-")?.strip_suffix(".dat")?.parse().ok()
            })
            .collect();
        h.sort_unstable();
        h
    }

    /// Der Abzug einer Höhe, geprüft gegen seinen eigenen Block.
    pub fn lesen(&self, hoehe: u64) -> Result<(Abzug, LedgerState), AbzugFehler> {
        let d = std::fs::read(self.pfad(hoehe)).map_err(|e| AbzugFehler::Datei(e.to_string()))?;
        Abzug::aus_datei(&d, &self.startwert)
    }

    /// ⚑ **Die Basis**: der Abzug, aus dem diese Kette lebt. Ohne ihn
    /// lässt sich die Kettendatei nicht nachspielen, denn ihr erster Block
    /// schliesst an ihn an; deshalb räumt [`Ablage::aufraeumen`] ihn nie weg.
    pub fn basis(&self) -> Option<u64> {
        std::fs::read_to_string(self.ordner.join("basis")).ok()?.trim().parse().ok()
    }

    /// Setzt die Basis, nachdem ihr Abzug abgelegt ist.
    pub fn basis_setzen(&self, hoehe: u64) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.ordner)?;
        std::fs::write(self.ordner.join("basis"), format!("{hoehe}\n"))
    }

    /// Ob die Hintergrundprüfung die Basis bestätigt hat.
    pub fn bestaetigt(&self) -> bool {
        self.basis().is_some_and(|h| {
            std::fs::read_to_string(self.ordner.join("bestaetigt")).ok().and_then(|t| t.trim().parse().ok()) == Some(h)
        })
    }

    /// Vermerkt, dass die Hintergrundprüfung die Basis bestätigt hat.
    pub fn bestaetigt_setzen(&self, hoehe: u64) -> std::io::Result<()> {
        std::fs::write(self.ordner.join("bestaetigt"), format!("{hoehe}\n"))
    }

    /// Behält die zwei jüngsten, jeden auf Dauer und die Basis.
    fn aufraeumen(&self) -> std::io::Result<()> {
        let hoehen = self.hoehen();
        let juengste: Vec<u64> = hoehen.iter().rev().take(2).copied().collect();
        let basis = self.basis();
        for h in hoehen {
            if !juengste.contains(&h) && !ist_langzeit(h) && Some(h) != basis {
                std::fs::remove_file(self.pfad(h))?;
            }
        }
        Ok(())
    }
}

/// **Setzt einen Abzug aus Stücken zusammen**, die von einer Gegenstelle
/// kommen. Der Block kommt mit jedem Stück und wird beim ersten gegen den
/// Anker gehalten; die angekündigte Größe ist gedeckelt, bevor Speicher
/// belegt wird; jedes Stück muss genau an der erwarteten Stelle beginnen.
#[derive(Debug)]
pub struct Zusammensetzen {
    anker: Anker,
    block: Option<Block>,
    groesse: u64,
    puffer: Vec<u8>,
}

/// Was nach einem Stück zu tun ist.
#[derive(Debug, PartialEq, Eq)]
pub enum Weiter {
    /// Das nächste Stück ab dieser Stelle anfordern.
    Ab(u64),
    /// Fertig und gegen den Anker geprüft.
    Fertig(Box<(Abzug, LedgerState)>),
}

impl Zusammensetzen {
    /// Ein neues Zusammensetzen für einen Anker.
    pub fn neu(anker: Anker) -> Self {
        Self { anker, block: None, groesse: 0, puffer: Vec::new() }
    }

    /// Der Anker, dem es gilt.
    pub fn anker(&self) -> Anker {
        self.anker
    }

    /// Ab welcher Stelle das nächste Stück zu holen ist.
    pub fn ab(&self) -> u64 {
        self.puffer.len() as u64
    }

    /// Nimmt ein Stück. Ein Fehler heißt: Diese Gegenstelle liefert nicht,
    /// was der Anker verlangt; neu beginnen, bei einer anderen.
    pub fn stueck(&mut self, block: Block, ab: u64, daten: &[u8], groesse: u64) -> Result<Weiter, AbzugFehler> {
        if self.block.is_none() {
            if block.header.height != self.anker.hoehe {
                return Err(AbzugFehler::FalscheHoehe { anker: self.anker.hoehe, block: block.header.height });
            }
            if block.hash() != self.anker.blockhash {
                return Err(AbzugFehler::FremderBlock);
            }
            if groesse == 0 || groesse > HOECHSTE_GROESSE {
                return Err(AbzugFehler::KeinZustand);
            }
            self.groesse = groesse;
            self.block = Some(block);
        } else if self.block.as_ref().is_some_and(|b| b.hash() != block.hash()) || groesse != self.groesse {
            return Err(AbzugFehler::FremderBlock);
        }
        if ab != self.ab() || ab + daten.len() as u64 > self.groesse || daten.is_empty() {
            return Err(AbzugFehler::Datei(format!("Stück ab {ab}, erwartet ab {}", self.ab())));
        }
        self.puffer.extend_from_slice(daten);
        if self.ab() < self.groesse {
            return Ok(Weiter::Ab(self.ab()));
        }
        let abzug = Abzug { block: self.block.clone().expect("gesetzt"), zustand: std::mem::take(&mut self.puffer) };
        let z = abzug.pruefen(&self.anker)?;
        Ok(Weiter::Fertig(Box::new((abzug, z))))
    }
}

/// Das Stück eines Abzugs ab `ab`: höchstens [`STUECK_BYTES`], und so viel
/// weniger, dass Block und Rahmen daneben in eine Antwort passen. Ein Block
/// darf so gross sein wie die ganze Antwort; dann passt kein Stück, und die
/// Antwort ist `None`.
pub fn stueck_von(abzug: &Abzug, ab: u64) -> Option<&[u8]> {
    let ab = usize::try_from(ab).ok()?;
    if ab >= abzug.zustand.len() {
        return None;
    }
    let block = borsh::to_vec(&abzug.block).ok()?.len();
    let platz = myl_types::protocol::MAX_ANFRAGE_BYTES.checked_sub(block + 4096)?;
    let n = (STUECK_BYTES as usize).min(platz);
    if n == 0 {
        return None;
    }
    let bis = (ab + n).min(abzug.zustand.len());
    Some(&abzug.zustand[ab..bis])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kette_bis(hoehe: u64) -> crate::kette::Kette {
        let mut k = crate::kette::Kette::probestand();
        for i in 0..hoehe {
            if i % 7 == 0 {
                let s = crate::kette::probeschluessel((i % 8) as u8);
                let nonce = k.zustand().account(&crate::kette::probekonto((i % 8) as u8)).nonce;
                let tx = myl_consensus::block::Transaktion::signiere(
                    &k.netzwert(),
                    &s,
                    nonce,
                    myl_consensus::block::Anweisung::Burn { betrag: 1_000 + i },
                )
                .expect("signieren");
                k.aufnehmen(tx);
            }
            k.baue_block();
        }
        k
    }

    fn abzug_von(k: &crate::kette::Kette, block: Block) -> Abzug {
        Abzug { block, zustand: borsh::to_vec(k.zustand()).expect("Zustand") }
    }

    /// Der Anker aus dem Text, und nur an Epochengrenzen.
    #[test]
    fn der_anker_steht_an_einer_grenze() {
        let h = "ab".repeat(32);
        assert_eq!(Anker::aus_text(&format!("1800:{h}")).expect("gültig").hoehe, 1800);
        assert!(Anker::aus_text(&format!("1801:{h}")).is_err(), "keine Grenze");
        assert!(Anker::aus_text(&format!("0:{h}")).is_err());
        assert!(Anker::aus_text("1800:abc").is_err());
        assert!(ist_langzeit(64 * 1800) && !ist_langzeit(63 * 1800) && !ist_langzeit(0));
    }

    /// Ein echter Abzug besteht; jede einzelne Abweichung fällt, und zwar
    /// an der Stelle, die sie betrifft.
    #[test]
    fn der_abzug_besteht_nur_gegen_seinen_anker() {
        let mut k = kette_bis(30);
        let block = k.bloecke_von_bis(30, 30).pop().expect("Block 30");
        let abzug = abzug_von(&k, block.clone());
        let anker = Anker { hoehe: 30, blockhash: block.hash() };
        assert_eq!(abzug.pruefen(&anker).expect("gültig").commitment(), k.zustandswurzel());

        let falsch_hoch = Anker { hoehe: 1800, ..anker };
        assert!(matches!(abzug.pruefen(&falsch_hoch), Err(AbzugFehler::FalscheHoehe { .. })));
        let fremd = Anker { blockhash: Hash::sha256(b"fremd"), ..anker };
        assert_eq!(abzug.pruefen(&fremd), Err(AbzugFehler::FremderBlock));
        let mut gefaelscht = abzug.clone();
        let letzte = gefaelscht.zustand.len() - 1;
        gefaelscht.zustand[letzte] ^= 1;
        assert_eq!(gefaelscht.pruefen(&anker), Err(AbzugFehler::FalscheWurzel));
    }

    /// Die Datei hin und zurück; ein abgeschnittener Abzug und einer eines
    /// anderen Netzes fallen.
    #[test]
    fn die_datei_haelt_und_meldet_sich() {
        let mut k = kette_bis(12);
        let block = k.bloecke_von_bis(12, 12).pop().expect("Block");
        let abzug = abzug_von(&k, block);
        let sw = k.netz().startwert_hash();
        let d = abzug.als_datei(&sw);
        let (zurueck, z) = Abzug::aus_datei(&d, &sw).expect("lesbar");
        assert_eq!(zurueck, abzug);
        assert_eq!(z.commitment(), k.zustandswurzel());
        assert!(Abzug::aus_datei(&d[..d.len() - 1], &sw).is_err(), "abgeschnitten");
        assert!(Abzug::aus_datei(&d, &Hash::sha256(b"anderes Netz")).is_err());
    }

    /// Die Ablage behält die zwei jüngsten und die dauerhaften.
    #[test]
    fn die_ablage_behaelt_was_sie_soll() {
        let ordner = std::env::temp_dir().join(format!("myl-abzug-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        let mut k = kette_bis(3);
        let block = k.bloecke_von_bis(3, 3).pop().expect("Block");
        let ablage = Ablage::neben(&ordner.join("kette.dat"), k.netz().startwert_hash());
        for epoche in [1u64, 2, 63, 64, 65, 66] {
            let mut b = block.clone();
            b.header.height = epoche * BLOECKE_JE_EPOCHE;
            ablage.ablegen(&abzug_von(&k, b)).expect("ablegen");
            if epoche == 2 {
                ablage.basis_setzen(2 * BLOECKE_JE_EPOCHE).expect("Basis");
            }
        }
        assert_eq!(ablage.hoehen(), vec![2 * 1800, 64 * 1800, 65 * 1800, 66 * 1800], "die Basis bleibt");
        assert!(!ablage.bestaetigt());
        ablage.bestaetigt_setzen(2 * 1800).expect("bestaetigt");
        assert!(ablage.bestaetigt());
        let _ = std::fs::remove_dir_all(&ordner);
    }

    /// Stücke in der richtigen Folge ergeben den Abzug; ein Stück an der
    /// falschen Stelle, ein anderer Block oder eine übergroße Ankündigung
    /// fallen, bevor etwas belegt wird.
    #[test]
    fn stuecke_setzen_sich_nur_in_ordnung_zusammen() {
        let mut k = kette_bis(20);
        let block = k.bloecke_von_bis(20, 20).pop().expect("Block");
        let abzug = abzug_von(&k, block.clone());
        let anker = Anker { hoehe: 20, blockhash: block.hash() };
        let groesse = abzug.zustand.len() as u64;
        let haelfte = groesse / 2;

        let mut z = Zusammensetzen::neu(anker);
        assert_eq!(z.stueck(block.clone(), 0, &abzug.zustand[..haelfte as usize], groesse), Ok(Weiter::Ab(haelfte)));
        assert!(z.stueck(block.clone(), 0, &abzug.zustand[..1], groesse).is_err(), "doppelt");
        let mut z = Zusammensetzen::neu(anker);
        z.stueck(block.clone(), 0, &abzug.zustand[..haelfte as usize], groesse).expect("erste Hälfte");
        match z.stueck(block.clone(), haelfte, &abzug.zustand[haelfte as usize..], groesse) {
            Ok(Weiter::Fertig(f)) => assert_eq!(f.1.commitment(), k.zustandswurzel()),
            anders => panic!("nicht fertig: {anders:?}"),
        }

        let mut fremd = block.clone();
        fremd.header.timestamp_ms += 1;
        assert_eq!(Zusammensetzen::neu(anker).stueck(fremd, 0, &abzug.zustand, groesse), Err(AbzugFehler::FremderBlock));
        assert!(Zusammensetzen::neu(anker).stueck(block.clone(), 0, b"x", HOECHSTE_GROESSE + 1).is_err(), "Deckel");
        let mut falsch = abzug.zustand.clone();
        falsch[0] ^= 1;
        assert_eq!(Zusammensetzen::neu(anker).stueck(block, 0, &falsch, groesse).map(|_| ()), Err(AbzugFehler::FalscheWurzel));
    }
}
