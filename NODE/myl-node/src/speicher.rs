//! Die Kette auf der Platte: ein anhängendes Blockprotokoll.
//!
//! # Was gespeichert wird, und warum genau das
//!
//! **Nur die Blöcke.** Nicht der Zustand, nicht die Höhe, nicht der
//! letzte Hash. Diese drei sind aus den Blöcken **ableitbar**, und ein
//! abgeleiteter Wert, der zusätzlich gespeichert wird, ist eine zweite
//! Wahrheit: Sobald sie einmal auseinanderlaufen, glaubt der Knoten der
//! falschen.
//!
//! Genau das ist die Fehlerklasse, die dieses Projekt am häufigsten
//! gefunden hat (Fund 44 und die fünf stillen Prüfungen vom
//! 2026-08-25): ein Wert, einmal kopiert und danach nie wieder gegen
//! seine Quelle gehalten.
//!
//! **Der Wiederanlauf ist deshalb ein Nachrechnen, kein Einlesen.** Der
//! Knoten liest die Blöcke und reicht sie durch dieselbe
//! [`crate::kette::Kette::uebernimm`], durch die auch Gossip-Blöcke
//! gehen. Jede Zustandswurzel wird dabei neu gerechnet und geprüft. Ein
//! zweiter Codepfad, der beim Laden andere Regeln anwendete als beim
//! Empfangen, wäre eine Einladung.
//!
//! # Was **nicht** gespeichert wird
//!
//! **Der Mempool.** Wartende Transaktionen sind unbestätigt; sie kommen
//! nach einem Neustart ohnehin wieder über den Gossip. Sie aufzuheben
//! hieße, nach einem Tag Stillstand alte Transaktionen wieder
//! einzuspeisen, deren Absender längst andere geschickt hat.
//!
//! **Der Konsenszustand einer laufenden Runde.** Eine BFT-Runde ist an
//! die Uhr gebunden und an eine stimmberechtigte Menge; sie über einen
//! Neustart zu retten, hieße mit einer Frist weiterzumachen, die
//! inzwischen abgelaufen ist. Was ein neu gestarteter Knoten braucht,
//! ist der **Abgleich** mit dem Netz, nicht die Konserve seines eigenen
//! alten Zustands. Diesen Abgleich gibt es seit dem 2026-08-29: Ein
//! Commit-Zertifikat belegt eine Entscheidung unabhängig von der Runde,
//! in der der Empfänger steht (⚑ Fund 67, siehe
//! `myl_consensus::round_change::Commitzertifikat`).
//!
//! # Das Format
//!
//! ```text
//! Kopf:    "MYLKETTE" (8) | Fassung u16 | Startwert (32)
//! Satz:    Länge u32 | Borsh(Block) | Prüfsumme u32
//! ```
//!
//! Alles Little-Endian, wie im Rest des Protokolls.
//!
//! **Der Startwert im Kopf bindet die Datei an ihre Kette.** Eine Datei
//! aus einem anderen Netz wird abgewiesen, statt eine fremde Historie
//! als eigene auszugeben.
//!
//! # ⚑ Was die Prüfsumme leistet, und was nicht
//!
//! **Sie findet den Abbruch, sie schützt nicht vor Fälschung.** Vier
//! Bytes SHA-256 über den Satz erkennen einen halb geschriebenen Satz
//! nach einem Absturz. Wer die Datei bearbeiten kann, kann auch die
//! Prüfsumme neu rechnen.
//!
//! Das ist kein Mangel, sondern die richtige Arbeitsteilung: **Gegen
//! einen veränderten Block schützt die Kette selbst.** `uebernimm`
//! prüft Vorgänger-Hash und Zustandswurzel; ein manipulierter Block
//! fällt beim Wiederanlauf durch dieselbe Regel wie im Betrieb.
//!
//! # ⚑ Was `flush` zusichert, und was nicht
//!
//! Nach jedem Block wird geschrieben und geleert, aber **nicht
//! `fsync`**. Der Unterschied ist genau benannt:
//!
//! | Ereignis | Blockprotokoll überlebt? |
//! |---|---|
//! | `kill -9` des Prozesses | **ja**, die Bytes liegen im Seitencache |
//! | Absturz des Betriebssystems | vielleicht |
//! | Stromausfall | vielleicht |
//!
//! Der Chaos-Test, für den diese Datei entsteht, beendet Prozesse hart.
//! Dafür genügt `flush`, und ein `fsync` je Block kostete auf einer
//! rotierenden Platte mehr, als der Probelauf wert ist. **Für ein echtes
//! Netz ist das eine offene Entscheidung**, keine Empfehlung.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use myl_consensus::block::Block;
use myl_types::hash::Hash;

/// Kennung am Dateianfang. Steht im Klartext, damit jemand, der die
/// Datei findet, weiß, was er gefunden hat.
pub const MAGIE: &[u8; 8] = b"MYLKETTE";

/// Fassung des Dateiformats.
///
/// Wird hochgezählt, sobald sich die Kodierung ändert. Eine Datei mit
/// unbekannter Fassung wird **abgewiesen**, nicht geraten.
pub const FASSUNG: u16 = 2;
// ⚑ Von 1 auf 2 am 2026-08-27: Der Blockkopf trägt seither ein
// **Höhenfeld**, und damit ändert sich die Borsh-Kodierung jedes Satzes.
// Eine Datei der alten Fassung würde beim Lesen nicht scheitern, sondern
// **falsch geparst** — die Höhe des einen Blocks wäre die Epoche des
// anderen. Genau dafür steht die Zahl im Kopf: Ein Wiederanlauf gegen
// ein altes Protokoll bricht mit einer Meldung ab, statt eine Kette zu
// erfinden.

/// Länge des Kopfes: Magie (8) + Fassung (2) + Startwert (32).
pub const KOPF_BYTES: u64 = 8 + 2 + 32;

/// Obergrenze für einen einzelnen Satz.
///
/// Hergeleitet aus dem Größenlimit des Block-Topics
/// (`myl_net::validation::MAX_BLOCKS_BYTES`, 2 MiB), plus Luft. Ein
/// Längenkopf, der darüber liegt, ist Datenmüll und wird nicht als
/// Anforderung an den Speicher gelesen: Sonst könnte ein einziges
/// gekipptes Bit den Knoten dazu bringen, vier Gigabyte anzufordern.
pub const MAX_SATZ_BYTES: u32 = 4 * 1024 * 1024;

/// Was beim Umgang mit dem Blockprotokoll schiefgehen kann.
#[derive(Debug)]
pub enum SpeicherFehler {
    /// Die Datei ließ sich nicht öffnen, lesen oder schreiben.
    Datei { pfad: PathBuf, grund: String },
    /// Die Datei beginnt nicht mit [`MAGIE`].
    KeineKettendatei { pfad: PathBuf },
    /// Die Fassung ist unbekannt.
    FremdeFassung { pfad: PathBuf, gefunden: u16 },
    /// Der Startwert im Kopf gehört zu einer anderen Kette.
    FremdeKette {
        pfad: PathBuf,
        erwartet: Hash,
        gefunden: Hash,
    },
    /// Ein Block ließ sich nicht als Block lesen.
    UnlesbarerSatz { pfad: PathBuf, nummer: u64 },
}

impl std::fmt::Display for SpeicherFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Datei { pfad, grund } => write!(f, "Kettendatei {}: {grund}", pfad.display()),
            Self::KeineKettendatei { pfad } => write!(
                f,
                "{} beginnt nicht mit {:?} und ist keine Kettendatei",
                pfad.display(),
                std::str::from_utf8(MAGIE).unwrap_or("MYLKETTE")
            ),
            Self::FremdeFassung { pfad, gefunden } => write!(
                f,
                "{} trägt Formatfassung {gefunden}, dieser Knoten kennt {FASSUNG}. \
                 Eine unbekannte Fassung wird abgewiesen und nicht geraten",
                pfad.display()
            ),
            Self::FremdeKette {
                pfad,
                erwartet,
                gefunden,
            } => write!(
                f,
                "{} gehört zu einer anderen Kette: Startwert {gefunden:?}, erwartet \
                 {erwartet:?}. Eine fremde Historie als eigene auszugeben wäre \
                 schlimmer als ein leerer Start",
                pfad.display()
            ),
            Self::UnlesbarerSatz { pfad, nummer } => write!(
                f,
                "{}: Satz {nummer} hat eine gültige Prüfsumme, liest sich aber nicht \
                 als Block. Das ist kein Abbruch, sondern ein Formatfehler",
                pfad.display()
            ),
        }
    }
}

impl std::error::Error for SpeicherFehler {}

/// Was beim Öffnen aus der Datei kam.
#[derive(Debug)]
pub struct Wiederanlauf {
    /// Die gelesenen Blöcke, in Schreibreihenfolge.
    pub bloecke: Vec<Block>,
    /// Wie viele Bytes am Ende verworfen wurden, weil ein Satz
    /// abbrach.
    ///
    /// **Gehört ins Protokoll.** Null heißt sauber beendet; ein Wert
    /// größer null heißt, der Knoten wurde mitten im Schreiben
    /// abgeräumt, und genau das will der Chaos-Test wissen.
    pub abgeschnitten: u64,
    /// Ob die Datei neu angelegt wurde.
    pub neu: bool,
}

/// Ein anhängendes Blockprotokoll auf der Platte.
#[derive(Debug)]
pub struct Kettenspeicher {
    pfad: PathBuf,
    datei: File,
    geschrieben: u64,
}

/// Prüfsumme eines Satzes: die ersten vier Bytes von SHA-256.
///
/// Siehe Modulkopf: ein Abbruchdetektor, kein Fälschungsschutz.
fn pruefsumme(bytes: &[u8]) -> u32 {
    let h = Hash::sha256(bytes);
    let b = h.as_bytes();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

impl Kettenspeicher {
    /// Öffnet die Datei, legt sie an, falls sie fehlt, und liest sie.
    ///
    /// `startwert` ist der Hash, bei dem die Kette dieses Knotens
    /// beginnt. Er bindet die Datei an ihre Kette.
    ///
    /// **Ein abgebrochener letzter Satz ist kein Fehler.** Er wird
    /// verworfen und die Datei auf die letzte vollständige Länge
    /// gekürzt. Genau so sieht eine Datei nach `kill -9` aus, und ein
    /// Knoten, der daran scheiterte, wäre nach jedem Absturz tot.
    pub fn oeffnen(pfad: &Path, startwert: Hash) -> Result<(Self, Wiederanlauf), SpeicherFehler> {
        let fehler = |e: std::io::Error| SpeicherFehler::Datei {
            pfad: pfad.to_path_buf(),
            grund: e.to_string(),
        };
        if let Some(ordner) = pfad.parent() {
            if !ordner.as_os_str().is_empty() {
                std::fs::create_dir_all(ordner).map_err(fehler)?;
            }
        }
        let neu = !pfad.exists() || std::fs::metadata(pfad).map(|m| m.len()).unwrap_or(0) == 0;
        let mut datei = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(pfad)
            .map_err(fehler)?;

        if neu {
            datei.set_len(0).map_err(fehler)?;
            datei.seek(SeekFrom::Start(0)).map_err(fehler)?;
            datei.write_all(MAGIE).map_err(fehler)?;
            datei.write_all(&FASSUNG.to_le_bytes()).map_err(fehler)?;
            datei.write_all(startwert.as_bytes()).map_err(fehler)?;
            datei.flush().map_err(fehler)?;
            return Ok((
                Self {
                    pfad: pfad.to_path_buf(),
                    datei,
                    geschrieben: 0,
                },
                Wiederanlauf {
                    bloecke: Vec::new(),
                    abgeschnitten: 0,
                    neu: true,
                },
            ));
        }

        let mut inhalt = Vec::new();
        datei.seek(SeekFrom::Start(0)).map_err(fehler)?;
        datei.read_to_end(&mut inhalt).map_err(fehler)?;

        if inhalt.len() < KOPF_BYTES as usize || &inhalt[..8] != MAGIE {
            return Err(SpeicherFehler::KeineKettendatei {
                pfad: pfad.to_path_buf(),
            });
        }
        let fassung = u16::from_le_bytes([inhalt[8], inhalt[9]]);
        if fassung != FASSUNG {
            return Err(SpeicherFehler::FremdeFassung {
                pfad: pfad.to_path_buf(),
                gefunden: fassung,
            });
        }
        let mut roh = [0u8; 32];
        roh.copy_from_slice(&inhalt[10..42]);
        let gefunden = Hash::from_bytes(roh);
        if gefunden != startwert {
            return Err(SpeicherFehler::FremdeKette {
                pfad: pfad.to_path_buf(),
                erwartet: startwert,
                gefunden,
            });
        }

        // Sätze lesen, bis einer abbricht.
        let mut bloecke = Vec::new();
        let mut pos = KOPF_BYTES as usize;
        let mut gueltig_bis = pos;
        let mut nummer = 0u64;
        while pos + 4 <= inhalt.len() {
            let laenge = u32::from_le_bytes([
                inhalt[pos],
                inhalt[pos + 1],
                inhalt[pos + 2],
                inhalt[pos + 3],
            ]);
            // Ein unsinniger Längenkopf ist ein Abbruch, keine
            // Speicheranforderung.
            if laenge == 0 || laenge > MAX_SATZ_BYTES {
                break;
            }
            let ende = pos + 4 + laenge as usize + 4;
            if ende > inhalt.len() {
                break;
            }
            let nutz = &inhalt[pos + 4..pos + 4 + laenge as usize];
            let gespeichert = u32::from_le_bytes([
                inhalt[ende - 4],
                inhalt[ende - 3],
                inhalt[ende - 2],
                inhalt[ende - 1],
            ]);
            if pruefsumme(nutz) != gespeichert {
                break;
            }
            let block: Block =
                borsh::from_slice(nutz).map_err(|_| SpeicherFehler::UnlesbarerSatz {
                    pfad: pfad.to_path_buf(),
                    nummer,
                })?;
            bloecke.push(block);
            nummer += 1;
            pos = ende;
            gueltig_bis = ende;
        }

        let abgeschnitten = (inhalt.len() - gueltig_bis) as u64;
        if abgeschnitten > 0 {
            // Der Rest ist ein abgebrochener Satz. Kürzen, damit der
            // nächste Block sauber anhängt.
            datei.set_len(gueltig_bis as u64).map_err(fehler)?;
        }
        datei.seek(SeekFrom::End(0)).map_err(fehler)?;

        let anzahl = bloecke.len() as u64;
        Ok((
            Self {
                pfad: pfad.to_path_buf(),
                datei,
                geschrieben: anzahl,
            },
            Wiederanlauf {
                bloecke,
                abgeschnitten,
                neu: false,
            },
        ))
    }

    /// Hängt einen Block an.
    pub fn anhaengen(&mut self, block: &Block) -> Result<(), SpeicherFehler> {
        let fehler = |e: std::io::Error| SpeicherFehler::Datei {
            pfad: self.pfad.clone(),
            grund: e.to_string(),
        };
        let nutz = borsh::to_vec(block).map_err(|e| SpeicherFehler::Datei {
            pfad: self.pfad.clone(),
            grund: e.to_string(),
        })?;
        // In einem Stück schreiben: Zwei write_all-Aufrufe könnten
        // zwischen Länge und Nutzlast unterbrochen werden, und dann
        // stünde ein Längenkopf ohne Inhalt da. Der Leser käme damit
        // zurecht, aber ein Satz, der gar nicht erst halb entsteht, ist
        // besser als einer, der sauber verworfen wird.
        let mut satz = Vec::with_capacity(nutz.len() + 8);
        satz.extend_from_slice(&(nutz.len() as u32).to_le_bytes());
        satz.extend_from_slice(&nutz);
        satz.extend_from_slice(&pruefsumme(&nutz).to_le_bytes());
        self.datei.write_all(&satz).map_err(fehler)?;
        self.datei.flush().map_err(fehler)?;
        self.geschrieben += 1;
        Ok(())
    }

    /// Wie viele Blöcke die Datei führt.
    pub fn bloecke(&self) -> u64 {
        self.geschrieben
    }

    /// Der Pfad, fürs Protokoll.
    pub fn pfad(&self) -> &Path {
        &self.pfad
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::{Anweisung, BlockHeader, Transaktion};

    fn tempdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "myl-speicher-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Testverzeichnis");
        p
    }

    fn startwert() -> Hash {
        Hash::sha256(b"myelith-testkette-genesis")
    }

    fn block(hoehe: u64) -> Block {
        let mut b = Block::new(BlockHeader {
            height: hoehe,
            epoch: myl_consensus::block::epoche_fuer_hoehe(hoehe),
            prev_block_hash: Hash::sha256(&hoehe.to_le_bytes()),
            timestamp_ms: 1_700_000_000_000 + hoehe,
            state_root: Hash::sha256(b"zustand"),
            saatquelle: None,
        });
        b.txs.push(
            Transaktion::signiere(
                &Hash::sha256(b"myelith-testkette-genesis"),
                &myl_types::bls::BlsSecretKey::key_gen(&[hoehe as u8; 32]).expect("Schlüssel"),
                0,
                Anweisung::Burn { betrag: 1_000 + hoehe },
            )
            .expect("signieren"),
        );
        b
    }

    #[test]
    fn eine_neue_datei_bekommt_einen_kopf_und_bleibt_leer() {
        let d = tempdir("neu");
        let p = d.join("kette.log");
        let (s, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen");
        assert!(w.neu);
        assert!(w.bloecke.is_empty());
        assert_eq!(w.abgeschnitten, 0);
        assert_eq!(s.bloecke(), 0);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), KOPF_BYTES);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn was_geschrieben_wurde_kommt_zurueck() {
        let d = tempdir("rundreise");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen");
            for h in 1..=5 {
                s.anhaengen(&block(h)).expect("anhängen");
            }
            assert_eq!(s.bloecke(), 5);
        }
        let (s, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("wieder öffnen");
        assert!(!w.neu);
        assert_eq!(w.abgeschnitten, 0);
        assert_eq!(s.bloecke(), 5);
        assert_eq!(w.bloecke.len(), 5);
        for (i, b) in w.bloecke.iter().enumerate() {
            assert_eq!(*b, block(i as u64 + 1), "Block {i} kam verändert zurück");
        }
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn nach_dem_wiederoeffnen_laesst_sich_weiter_anhaengen() {
        // Ohne das stünde der zweite Lauf eines Knotens vor einer Datei,
        // die er lesen, aber nicht fortschreiben kann.
        let d = tempdir("fortsetzen");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            s.anhaengen(&block(1)).unwrap();
        }
        {
            let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            assert_eq!(w.bloecke.len(), 1);
            s.anhaengen(&block(2)).unwrap();
            assert_eq!(s.bloecke(), 2);
        }
        let (_, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        assert_eq!(w.bloecke.len(), 2);
        assert_eq!(w.bloecke[1], block(2));
        std::fs::remove_dir_all(&d).ok();
    }

    /// ⚑ **Der Fall, für den diese Datei gebaut ist: `kill -9` mitten im
    /// Schreiben.**
    ///
    /// Nachgestellt, indem hinter einen vollständigen Satz die Hälfte
    /// eines weiteren geschrieben wird. Erwartung: die vollständigen
    /// Sätze kommen zurück, der Rest wird verworfen und die Datei
    /// gekürzt.
    #[test]
    fn ein_abgebrochener_satz_wird_verworfen_und_gekuerzt() {
        let d = tempdir("abbruch");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            s.anhaengen(&block(1)).unwrap();
            s.anhaengen(&block(2)).unwrap();
        }
        let vorher = std::fs::metadata(&p).unwrap().len();

        // Einen halben dritten Satz anhängen.
        let nutz = borsh::to_vec(&block(3)).unwrap();
        let mut halb = Vec::new();
        halb.extend_from_slice(&(nutz.len() as u32).to_le_bytes());
        halb.extend_from_slice(&nutz[..nutz.len() / 2]);
        let angehaengt = halb.len() as u64;
        {
            let mut f = OpenOptions::new().append(true).open(&p).unwrap();
            f.write_all(&halb).unwrap();
        }
        assert_eq!(std::fs::metadata(&p).unwrap().len(), vorher + angehaengt);

        let (s, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen nach Abbruch");
        assert_eq!(w.bloecke.len(), 2, "die vollständigen Sätze müssen bleiben");
        assert_eq!(w.abgeschnitten, angehaengt);
        assert_eq!(s.bloecke(), 2);
        assert_eq!(
            std::fs::metadata(&p).unwrap().len(),
            vorher,
            "die Datei wurde nicht auf die letzte vollständige Länge gekürzt"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn nach_dem_kuerzen_haengt_der_naechste_block_sauber_an() {
        // Der eigentliche Zweck des Kürzens. Ohne es stünde der neue
        // Satz hinter Datenmüll und wäre beim nächsten Lesen unerreichbar.
        let d = tempdir("nach-abbruch");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            s.anhaengen(&block(1)).unwrap();
        }
        {
            let mut f = OpenOptions::new().append(true).open(&p).unwrap();
            f.write_all(&[0xFF, 0x00, 0x00, 0x00, 0xAB]).unwrap();
        }
        {
            let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            assert!(w.abgeschnitten > 0);
            s.anhaengen(&block(2)).unwrap();
        }
        let (_, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        assert_eq!(w.bloecke.len(), 2);
        assert_eq!(w.bloecke[1], block(2));
        assert_eq!(w.abgeschnitten, 0);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn eine_verstuemmelte_pruefsumme_beendet_das_lesen() {
        let d = tempdir("pruefsumme");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            s.anhaengen(&block(1)).unwrap();
            s.anhaengen(&block(2)).unwrap();
        }
        // Ein Byte im **ersten** Satz kippen.
        let mut inhalt = std::fs::read(&p).unwrap();
        let ziel = KOPF_BYTES as usize + 8;
        inhalt[ziel] ^= 0xFF;
        std::fs::write(&p, &inhalt).unwrap();

        let (_, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen");
        assert!(
            w.bloecke.is_empty(),
            "ein verstümmelter erster Satz darf nichts durchlassen"
        );
        assert!(w.abgeschnitten > 0);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn ein_unsinniger_laengenkopf_fordert_keinen_speicher_an() {
        // Ein gekipptes Bit im Längenkopf darf den Knoten nicht dazu
        // bringen, Gigabyte anzufordern.
        let d = tempdir("laenge");
        let p = d.join("kette.log");
        {
            let (_, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        }
        {
            let mut f = OpenOptions::new().append(true).open(&p).unwrap();
            f.write_all(&u32::MAX.to_le_bytes()).unwrap();
        }
        let (_, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen");
        assert!(w.bloecke.is_empty());
        assert_eq!(w.abgeschnitten, 4);
        std::fs::remove_dir_all(&d).ok();
    }

    // ── Bindung an die eigene Kette ─────────────────────────────────

    #[test]
    fn eine_datei_aus_einer_anderen_kette_wird_abgewiesen() {
        // Eine fremde Historie als eigene auszugeben wäre schlimmer als
        // ein leerer Start.
        let d = tempdir("fremd");
        let p = d.join("kette.log");
        {
            let (mut s, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
            s.anhaengen(&block(1)).unwrap();
        }
        let anderer = Hash::sha256(b"ein anderes netz");
        match Kettenspeicher::oeffnen(&p, anderer) {
            Err(SpeicherFehler::FremdeKette { gefunden, .. }) => {
                assert_eq!(gefunden, startwert());
            }
            andere => panic!("erwartet FremdeKette, bekommen {andere:?}"),
        }
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn eine_fremde_datei_wird_nicht_als_kette_gelesen() {
        let d = tempdir("keine");
        let p = d.join("kette.log");
        std::fs::write(&p, b"das hier ist irgendetwas anderes").unwrap();
        assert!(matches!(
            Kettenspeicher::oeffnen(&p, startwert()),
            Err(SpeicherFehler::KeineKettendatei { .. })
        ));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn eine_fremde_fassung_wird_abgewiesen_und_nicht_geraten() {
        let d = tempdir("fassung");
        let p = d.join("kette.log");
        {
            let (_, _) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        }
        let mut inhalt = std::fs::read(&p).unwrap();
        inhalt[8] = 99;
        std::fs::write(&p, &inhalt).unwrap();
        match Kettenspeicher::oeffnen(&p, startwert()) {
            Err(SpeicherFehler::FremdeFassung { gefunden, .. }) => assert_eq!(gefunden, 99),
            andere => panic!("erwartet FremdeFassung, bekommen {andere:?}"),
        }
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn die_pruefsumme_findet_jede_einzelne_bitkippung_im_satz() {
        // Vier Bytes fangen nicht alles, aber sie sollen jede
        // Einzelkippung finden. Gemessen statt behauptet.
        let nutz = borsh::to_vec(&block(7)).unwrap();
        let gut = pruefsumme(&nutz);
        let mut verfehlt = 0usize;
        let mut versuche = 0usize;
        for i in 0..nutz.len() {
            for bit in 0..8u32 {
                let mut kaputt = nutz.clone();
                kaputt[i] ^= 1 << bit;
                versuche += 1;
                if pruefsumme(&kaputt) == gut {
                    verfehlt += 1;
                }
            }
        }
        println!("[Messung] {versuche} Bitkippungen, {verfehlt} unentdeckt");
        assert_eq!(verfehlt, 0, "eine Einzelkippung kam durch");
    }
}
