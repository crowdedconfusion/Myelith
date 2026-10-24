//! **Kalte Ausfuhr und Einfuhr der Geschichte** (Archivknoten, Stufe A8,
//! 2026-10-23).
//!
//! Die letzte Schicht gegen den Totalausfall: eine Kopie der Kette
//! **ausserhalb des Netzes**, auf einem Datenträger oder in fremdem
//! Speicher. Der Ort muss nicht vertrauenswürdig sein, denn alles, was
//! zurückkommt, wird geprüft.
//!
//! # ⚑ Das Format
//!
//! Ein Ordner mit je Periode einer Datei `periode-<ende>.bloecke` (die
//! Höhen `ende − P + 1 ..= ende`) und, bei einer Ausfuhr von Hand, einer
//! Datei `offen-<bis>.bloecke` für den Rest. Darin je Block die Länge (u32,
//! little endian) und die Borsh-Bytes. Dazu `verzeichnis.txt`: eine
//! Kopfzeile mit dem Startwert des Netzes, je Datei eine Zeile mit Name,
//! Höhen, SHA-256 der Datei und Hash ihres letzten Blocks.
//!
//! # ⚑ Was die Einfuhr prüft
//!
//! - Summe jeder Datei, lückenlose Höhen ab 1, jeden Vorgängerhash ab dem
//!   Startwert und den letzten Hash jeder Datei.
//! - **In eine leere Kette:** die Anker des Netzes, soweit die Höhen
//!   reichen; danach rechnet der Knoten beim Start alles ab Genesis nach,
//!   Zustandswurzel für Zustandswurzel.
//! - **Vor eine verdichtete Kette:** die Historienwurzel ihrer Basis und den
//!   Hash des Basisblocks; erst dann wird vorangestellt.
//!
//! ⚑ **Ausgeführt wird nur bei angehaltenem Knoten oder vom Knoten selbst**
//! (`--archiv-ausfuhr`, je fertige Periode): Zwei Prozesse an derselben
//! Kettendatei wären einer zu viel.

use std::io::Write;
use std::path::{Path, PathBuf};

use myl_consensus::block::Block;
use myl_types::hash::Hash;
use sha2::Digest;

use crate::archiv::PERIODE_BLOECKE;
use crate::speicher::Kettenspeicher;

/// Der Name des Verzeichnisses im Ordner.
pub const VERZEICHNIS: &str = "verzeichnis.txt";

const KOPF: &str = "myelith-archiv 1";

/// Eine Zeile des Verzeichnisses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datei {
    pub name: String,
    pub ab: u64,
    pub bis: u64,
    pub summe: Hash,
    pub letzter: Hash,
}

/// **Liest das Verzeichnis**: Startwert und Dateien nach Höhe.
pub fn verzeichnis_lesen(ordner: &Path) -> Result<(Hash, Vec<Datei>), String> {
    let text = std::fs::read_to_string(ordner.join(VERZEICHNIS)).map_err(|e| format!("{VERZEICHNIS}: {e}"))?;
    let mut zeilen = text.lines();
    let kopf = zeilen.next().ok_or("leeres Verzeichnis")?;
    let startwert = kopf
        .strip_prefix(KOPF)
        .map(str::trim)
        .ok_or("kein Verzeichnis einer Ausfuhr")?
        .parse::<Hash>()
        .map_err(|e| format!("Startwert: {e}"))?;
    let mut dateien = Vec::new();
    for z in zeilen.filter(|z| !z.trim().is_empty()) {
        let t: Vec<&str> = z.split_whitespace().collect();
        let [name, ab, bis, summe, letzter] = t[..] else {
            return Err(format!("Zeile unlesbar: {z}"));
        };
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return Err(format!("Dateiname ausserhalb des Ordners: {name}"));
        }
        dateien.push(Datei {
            name: name.to_string(),
            ab: ab.parse().map_err(|_| format!("Höhe unlesbar: {z}"))?,
            bis: bis.parse().map_err(|_| format!("Höhe unlesbar: {z}"))?,
            summe: summe.parse().map_err(|e| format!("Summe: {e}"))?,
            letzter: letzter.parse().map_err(|e| format!("Hash: {e}"))?,
        });
    }
    dateien.sort_by_key(|d| d.ab);
    Ok((startwert, dateien))
}

fn verzeichnis_schreiben(ordner: &Path, startwert: &Hash, dateien: &[Datei]) -> Result<(), String> {
    let mut text = format!("{KOPF} {}\n", startwert.to_hex());
    for d in dateien {
        text.push_str(&format!("{} {} {} {} {}\n", d.name, d.ab, d.bis, d.summe.to_hex(), d.letzter.to_hex()));
    }
    let ziel = ordner.join(VERZEICHNIS);
    let zwischen = ordner.join(format!("{VERZEICHNIS}.neu"));
    std::fs::write(&zwischen, text).map_err(|e| e.to_string())?;
    std::fs::rename(zwischen, ziel).map_err(|e| e.to_string())
}

fn summe_der_datei(pfad: &Path) -> Option<Hash> {
    let bytes = std::fs::read(pfad).ok()?;
    Some(Hash(sha2::Sha256::digest(&bytes).into()))
}

/// **Führt aus**, was bis `bis_hoehe` noch fehlt. `holen(ab, bis)` liefert
/// Blöcke aufsteigend ab `ab` (auch weniger). Mit `nur_ganze` nur
/// vollständige Perioden, sonst dazu den Rest als `offen-…`. Vorhandene
/// Dateien mit stimmender Summe bleiben. Gibt die Zahl der neu
/// geschriebenen Dateien zurück.
pub fn ausfuhr(
    ordner: &Path,
    startwert: &Hash,
    bis_hoehe: u64,
    nur_ganze: bool,
    mut holen: impl FnMut(u64, u64) -> Vec<Block>,
) -> Result<usize, String> {
    std::fs::create_dir_all(ordner).map_err(|e| e.to_string())?;
    let mut alt = match verzeichnis_lesen(ordner) {
        Ok((s, d)) if s == *startwert => d,
        Ok(_) => return Err("der Ordner gehört zu einem anderen Netz".into()),
        Err(_) => Vec::new(),
    };
    // Ein offener Rest wird immer neu geschrieben.
    for d in alt.iter().filter(|d| d.name.starts_with("offen-")) {
        let _ = std::fs::remove_file(ordner.join(&d.name));
    }
    alt.retain(|d| !d.name.starts_with("offen-"));
    let mut neu = 0usize;
    let mut dateien = Vec::new();
    let mut letzter = *startwert;
    let mut p = 0u64;
    loop {
        let ab = p * PERIODE_BLOECKE + 1;
        let ende = (p + 1) * PERIODE_BLOECKE;
        p += 1;
        if ab > bis_hoehe {
            break;
        }
        let bis = ende.min(bis_hoehe);
        if bis < ende && nur_ganze {
            break;
        }
        let name = if bis == ende { format!("periode-{ende}.bloecke") } else { format!("offen-{bis}.bloecke") };
        if let Some(d) = alt.iter().find(|d| d.name == name && d.ab == ab && d.bis == bis) {
            if summe_der_datei(&ordner.join(&name)) == Some(d.summe) {
                letzter = d.letzter;
                dateien.push(d.clone());
                continue;
            }
        }
        let zwischen = ordner.join(format!("{name}.neu"));
        let mut datei = std::io::BufWriter::new(std::fs::File::create(&zwischen).map_err(|e| e.to_string())?);
        let mut summe = sha2::Sha256::new();
        let mut h = ab;
        while h <= bis {
            let teil = holen(h, bis);
            if teil.first().map(|b| b.header.height) != Some(h) {
                return Err(format!("Block {h} fehlt in der Kette"));
            }
            for b in teil.into_iter().take_while(|b| b.header.height <= bis) {
                if b.header.height != h || b.header.prev_block_hash != letzter {
                    return Err(format!("die Kette bricht bei {h}"));
                }
                let roh = borsh::to_vec(&b).map_err(|e| e.to_string())?;
                let laenge = (roh.len() as u32).to_le_bytes();
                datei.write_all(&laenge).and_then(|_| datei.write_all(&roh)).map_err(|e| e.to_string())?;
                summe.update(laenge);
                summe.update(&roh);
                letzter = b.hash();
                h += 1;
            }
        }
        datei.flush().map_err(|e| e.to_string())?;
        datei.get_ref().sync_all().map_err(|e| e.to_string())?;
        drop(datei);
        std::fs::rename(&zwischen, ordner.join(&name)).map_err(|e| e.to_string())?;
        dateien.push(Datei { name, ab, bis, summe: Hash(summe.finalize().into()), letzter });
        neu += 1;
    }
    verzeichnis_schreiben(ordner, startwert, &dateien)?;
    Ok(neu)
}

/// **Liest eine Ausfuhr geprüft**: Summen, Höhen, Verkettung ab dem
/// Startwert; `jeder` bekommt die Blöcke in Folge und darf abbrechen.
/// Gibt die letzte Höhe zurück.
pub fn einfuhr_lesen(
    ordner: &Path,
    startwert: &Hash,
    mut jeder: impl FnMut(&Block) -> Result<bool, String>,
) -> Result<u64, String> {
    let (s, dateien) = verzeichnis_lesen(ordner)?;
    if s != *startwert {
        return Err("die Ausfuhr gehört zu einem anderen Netz".into());
    }
    let mut letzter = *startwert;
    let mut h = 0u64;
    for d in &dateien {
        if d.ab != h + 1 || d.bis < d.ab {
            return Err(format!("{}: Lücke vor Höhe {}", d.name, d.ab));
        }
        let bytes = std::fs::read(ordner.join(&d.name)).map_err(|e| format!("{}: {e}", d.name))?;
        if Hash(sha2::Sha256::digest(&bytes).into()) != d.summe {
            return Err(format!("{}: die Summe stimmt nicht", d.name));
        }
        let mut at = 0usize;
        while at < bytes.len() {
            let l = bytes
                .get(at..at + 4)
                .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize)
                .ok_or_else(|| format!("{}: abgeschnitten", d.name))?;
            let roh = bytes.get(at + 4..at + 4 + l).ok_or_else(|| format!("{}: abgeschnitten", d.name))?;
            at += 4 + l;
            let b: Block = borsh::from_slice(roh).map_err(|e| format!("{}: Block unlesbar: {e}", d.name))?;
            if b.header.height != h + 1 || b.header.prev_block_hash != letzter {
                return Err(format!("{}: die Kette bricht bei {}", d.name, h + 1));
            }
            h += 1;
            letzter = b.hash();
            if !jeder(&b)? {
                return Ok(h);
            }
        }
        if h != d.bis || letzter != d.letzter {
            return Err(format!("{}: endet nicht wie verzeichnet", d.name));
        }
    }
    Ok(h)
}

/// **Einfuhr in eine leere Kette** (nach einem Totalausfall): schreibt die
/// Blöcke nach `kette` und prüft die Anker des Netzes. Den Rest prüft der
/// Knoten beim Start, indem er ab Genesis nachrechnet.
pub fn einfuhr_in_leere_kette(ordner: &Path, kette: &Path, netz: &crate::netz::Netz) -> Result<u64, String> {
    let startwert = netz.startwert_hash();
    let (mut speicher, _) = Kettenspeicher::oeffnen(kette, startwert).map_err(|e| e.to_string())?;
    if speicher.bloecke() > 0 {
        return Err(format!("{} ist nicht leer", kette.display()));
    }
    einfuhr_lesen(ordner, &startwert, |b| {
        if let Some((_, a)) = netz.abzuege.iter().find(|(h, _)| *h == b.header.height) {
            if b.hash().as_bytes() != a {
                return Err(format!("Höhe {} trifft den Anker des Netzes nicht", b.header.height));
            }
        }
        speicher.anhaengen(b).map_err(|e| e.to_string())?;
        Ok(true)
    })
}

/// **Einfuhr vor eine verdichtete Kette**: die Blöcke bis zur Basis, geprüft
/// gegen die Historienwurzel des Basiszustands und den Basisblock, dann
/// vorangestellt. Gibt die Zahl der eingefügten Blöcke zurück.
pub fn einfuhr_vor_basis(ordner: &Path, kette: &Path, netz: &crate::netz::Netz) -> Result<u64, String> {
    let startwert = netz.startwert_hash();
    let ablage = crate::abzug::Ablage::neben(kette, startwert);
    let basis = ablage.basis().ok_or("die Kette hat keine Basis")?;
    let (abzug, zustand) = ablage.lesen(basis).map_err(|e| format!("Basis {basis}: {e:?}"))?;
    let mut zwischen = kette.as_os_str().to_owned();
    zwischen.push(".einfuhr");
    let zwischen = PathBuf::from(zwischen);
    let _ = std::fs::remove_file(&zwischen);
    let (mut archiv, _) = Kettenspeicher::oeffnen(&zwischen, startwert).map_err(|e| e.to_string())?;
    let mut historie = myl_types::historie::Historie::default();
    historie.anhaengen(&startwert);
    let mut basis_gesehen = false;
    einfuhr_lesen(ordner, &startwert, |b| {
        archiv.anhaengen(b).map_err(|e| e.to_string())?;
        if b.header.height < basis {
            historie.anhaengen(&b.hash());
            return Ok(true);
        }
        basis_gesehen = b.hash() == abzug.block.hash();
        Ok(false)
    })?;
    drop(archiv);
    if !basis_gesehen || historie != zustand.historie {
        let _ = std::fs::remove_file(&zwischen);
        return Err("die Ausfuhr trifft die Historie der Basis nicht".into());
    }
    let (mut speicher, _) = Kettenspeicher::oeffnen(kette, startwert).map_err(|e| e.to_string())?;
    let n = speicher.vorne_einfuegen(&zwischen).map_err(|e| e.to_string())?;
    ablage.basis_entfernen().map_err(|e| e.to_string())?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kette::Kette;

    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-ausfuhr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    /// ⚑ **Ausfuhr, Löschen, Einfuhr, gleiche Wurzel**: eine Kette über
    /// eine Periode hinaus, ausgeführt mit offenem Rest, in eine leere
    /// Kette eingeführt und dort nachgerechnet.
    #[test]
    fn ausfuhr_und_einfuhr_ergeben_dieselbe_kette() {
        let d = ordner("rund");
        let mut k = Kette::probestand();
        let mut bloecke = Vec::new();
        while k.hoehe() < PERIODE_BLOECKE + 700 {
            bloecke.push(k.baue_block());
        }
        let startwert = k.netz().startwert_hash();
        let holen = |ab: u64, bis: u64| -> Vec<Block> {
            bloecke[(ab - 1) as usize..bis.min(ab + 511) as usize].to_vec()
        };
        let aus = d.join("aus");
        assert_eq!(ausfuhr(&aus, &startwert, k.hoehe(), false, holen).expect("Ausfuhr"), 2);
        assert_eq!(ausfuhr(&aus, &startwert, k.hoehe(), false, holen).expect("zweimal"), 1, "nur der offene Rest neu");
        let ziel = d.join("neu/kette.dat");
        assert_eq!(einfuhr_in_leere_kette(&aus, &ziel, k.netz()).expect("Einfuhr"), k.hoehe());
        let mut neu = Kette::probestand();
        let (mut s, _) = Kettenspeicher::oeffnen(&ziel, startwert).expect("öffnen");
        s.fuer_jeden_satz(|b| neu.uebernimm(&b).expect("nachrechnen")).expect("lesen");
        assert_eq!(neu.zustandswurzel(), k.zustandswurzel());
        // Und die eingeführte Datei liefert aus der Mitte, was dort steht.
        neu.speicher_setzen(s);
        let mitte = neu.bloecke_von_bis(3601, 4112);
        assert_eq!(mitte.first().map(|b| b.header.height), Some(3601));
        assert_eq!(mitte.first().map(Block::hash), Some(bloecke[3600].hash()));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔️ **Eine verfälschte Ausfuhr fällt durch**: ein Byte in einer Datei,
    /// und ein Verzeichnis mit passender Summe, aber fremden Blöcken.
    #[test]
    fn eine_verfaelschte_ausfuhr_faellt_durch() {
        let d = ordner("falsch");
        let mut k = Kette::probestand();
        let bloecke: Vec<Block> = (0..300).map(|_| k.baue_block()).collect();
        let startwert = k.netz().startwert_hash();
        let aus = d.join("aus");
        ausfuhr(&aus, &startwert, 300, false, |ab, bis| bloecke[(ab - 1) as usize..bis as usize].to_vec()).expect("Ausfuhr");
        let datei = aus.join("offen-300.bloecke");
        let mut bytes = std::fs::read(&datei).expect("lesen");
        bytes[1000] ^= 1;
        std::fs::write(&datei, &bytes).expect("schreiben");
        let e = einfuhr_in_leere_kette(&aus, &d.join("a/kette.dat"), k.netz()).expect_err("Byte verfälscht");
        assert!(e.contains("Summe"), "{e}");
        // Ein ausgetauschter Block mit neu gerechneter Summe: Die Summe
        // stimmt, die Verkettung nicht.
        let mut neu = Vec::new();
        for (i, b) in bloecke.iter().enumerate() {
            let mut b = b.clone();
            if i == 150 {
                b.header.timestamp_ms += 1;
            }
            let roh = borsh::to_vec(&b).expect("borsh");
            neu.extend_from_slice(&(roh.len() as u32).to_le_bytes());
            neu.extend_from_slice(&roh);
        }
        std::fs::write(&datei, &neu).expect("schreiben");
        let (_, mut dateien) = verzeichnis_lesen(&aus).expect("Verzeichnis");
        dateien[0].summe = Hash(sha2::Sha256::digest(&neu).into());
        verzeichnis_schreiben(&aus, &startwert, &dateien).expect("Verzeichnis");
        let e = einfuhr_lesen(&aus, &startwert, |_| Ok(true)).expect_err("ausgetauschter Block");
        assert!(e.contains("bricht bei 152"), "{e}");
        // Die Ausfuhr eines anderen Netzes fällt am Startwert.
        let e = einfuhr_lesen(&aus, &Hash::sha256(b"anderes Netz"), |_| Ok(true)).expect_err("anderes Netz");
        assert!(e.contains("anderen Netz"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
