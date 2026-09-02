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
//!
//! # ⚑ Was im Speicher liegt: die Orte, nicht die Blöcke
//!
//! Bis zum 2026-09-02 las `oeffnen` die **ganze Datei** in den
//! Arbeitsspeicher und gab **alle Blöcke** als `Vec<Block>` zurück, also
//! zweimal dieselbe Kette: einmal roh, einmal dekodiert. Bei einer Datei
//! von zehn Gigabyte startete kein Knoten mehr, und das ist keine
//! ferne Größe: Ein Block darf bis zu [`MAX_SATZ_BYTES`] tragen.
//!
//! Jetzt hält der Speicher je Satz **acht Bytes**, nämlich seinen
//! Anfang in der Datei. Wer einen Block braucht, liest ihn. Das ist die
//! Bauart, die sich in den großen Ketten durchgesetzt hat: Bitcoin Core
//! hält die Blöcke in `blk*.dat` und im Arbeitsspeicher nur den
//! Blockindex; go-ethereum hält die Körper in der Datenbank und im
//! Arbeitsspeicher einen Zwischenspeicher von 256 Stück
//! (`bodyCacheLimit`).
//!
//! **Zwei Verweise, mit verschiedenen Aufgaben:**
//!
//! | Verweis | Reihenfolge | Wofür |
//! |---|---|---|
//! | `orte` | wie in der Datei | Wiederanlauf, jeder Satz genau einmal |
//! | `nach_hoehe` | nach Blockhöhe | Nachlieferung an Fragende |
//!
//! ⚑ **Der Wiederanlauf geht über `orte`, nicht über `nach_hoehe`**, und
//! das ist keine Kleinigkeit. Eine Datei mit doppelten oder springenden
//! Höhen fiele in `nach_hoehe` zusammen, und der Wiederanlauf spielte
//! **weniger** Sätze ab, als die Datei enthält. Das wäre eine Auswahl
//! **vor** der Prüfung, also genau die Stelle, an der ein manipulierter
//! Verlauf durchkäme. Über `orte` geht jeder Satz durch `uebernimm`, in
//! Dateireihenfolge, wie bisher.
//!
//! **Und `nach_hoehe` darf ungenau sein**, weil ein falscher Block dort
//! nur an einen Fragenden geht, der ihn selbst über `uebernimm` prüft:
//! Vorgänger-Hash und Zustandswurzel entscheiden, nicht die Herkunft.
//!
//! **Gelesen wird über einen zweiten Dateigriff.** Ein Griff für beides
//! hieße, dass jedes Lesen die Schreibstelle verschiebt, und ein
//! vergessenes Zurückspringen schriebe den nächsten Block **mitten in
//! die Kette**. Der Fehler wäre still und die Datei danach hin. Zwei
//! Griffe können das nicht.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
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
    /// Ein Satz, der beim Öffnen noch las, liest sich jetzt nicht mehr.
    ///
    /// ⚑ **Anderer Fall als [`SpeicherFehler::UnlesbarerSatz`].** Der
    /// steht für eine Datei, die von Anfang an nicht stimmte. Dieser
    /// hier steht für eine, die stimmte und es nicht mehr tut: Die
    /// Platte hat gekippt, oder jemand hat die Datei unter dem
    /// laufenden Knoten bearbeitet. Deshalb nennt er den **Ort in
    /// Bytes** und nicht die Satznummer: Der Ort ist es, an dem
    /// nachzusehen ist.
    SatzNichtLesbar { pfad: PathBuf, ort: u64 },
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
            Self::SatzNichtLesbar { pfad, ort } => write!(
                f,
                "{}: der Satz bei Byte {ort} las sich beim Öffnen und jetzt nicht mehr. \
                 Die Datei hat sich unter dem laufenden Knoten geändert",
                pfad.display()
            ),
        }
    }
}

impl std::error::Error for SpeicherFehler {}

/// Was beim Öffnen aus der Datei kam.
#[derive(Debug)]
pub struct Wiederanlauf {
    /// Wie viele vollständige Sätze die Datei führt.
    ///
    /// ⚑ **Die Blöcke selbst stehen hier nicht mehr** (Fund 124). Sie
    /// kamen bis zum 2026-09-02 als `Vec<Block>` zurück, und damit lag
    /// die ganze Kette im Arbeitsspeicher, bevor der Knoten den ersten
    /// Block geprüft hatte. Wer sie braucht, holt sie über
    /// [`Kettenspeicher::fuer_jeden_satz`], einen nach dem anderen.
    pub anzahl: u64,
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
    /// Der Griff zum Anhängen. Steht immer am Ende der Datei.
    datei: File,
    /// Der Griff zum Lesen. Springt frei umher und stört das Anhängen
    /// deshalb nicht (siehe Modulkopf).
    leser: File,
    /// Wo jeder Satz anfängt, in Dateireihenfolge.
    ///
    /// **Acht Bytes je Block.** Eine Kette von einer Million Blöcken
    /// kostet hier acht Megabyte, die Blöcke selbst kosteten je nach
    /// Größe drei bis vier Zehnerpotenzen mehr.
    orte: Vec<u64>,
    /// Wo der Satz zu einer Höhe anfängt, für die Nachlieferung.
    ///
    /// Bei doppelten Höhen gewinnt der spätere Satz. Das ist zulässig,
    /// weil ein Empfänger jeden gelieferten Block selbst prüft; siehe
    /// Modulkopf.
    nach_hoehe: BTreeMap<u64, u64>,
    /// Die Länge der Datei, also der Anfang des nächsten Satzes.
    laenge: u64,
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
            let leser = File::open(pfad).map_err(fehler)?;
            return Ok((
                Self {
                    pfad: pfad.to_path_buf(),
                    datei,
                    leser,
                    orte: Vec::new(),
                    nach_hoehe: BTreeMap::new(),
                    laenge: KOPF_BYTES,
                    geschrieben: 0,
                },
                Wiederanlauf {
                    anzahl: 0,
                    abgeschnitten: 0,
                    neu: true,
                },
            ));
        }

        // ⚑ **Satzweise lesen, nicht die Datei am Stück** (Fund 124).
        // `read_to_end` legte die ganze Kette in den Arbeitsspeicher,
        // und `borsh::from_slice` legte sie ein zweites Mal daneben.
        // Hier steht immer nur **ein** Satz im Speicher, und was bleibt,
        // sind seine acht Bytes Ort.
        let dateilaenge = datei.metadata().map_err(fehler)?.len();
        let mut leser = BufReader::new(File::open(pfad).map_err(fehler)?);

        let mut kopf = [0u8; KOPF_BYTES as usize];
        leser.read_exact(&mut kopf).map_err(|_| SpeicherFehler::KeineKettendatei {
            pfad: pfad.to_path_buf(),
        })?;
        if &kopf[..8] != MAGIE {
            return Err(SpeicherFehler::KeineKettendatei {
                pfad: pfad.to_path_buf(),
            });
        }
        let fassung = u16::from_le_bytes([kopf[8], kopf[9]]);
        if fassung != FASSUNG {
            return Err(SpeicherFehler::FremdeFassung {
                pfad: pfad.to_path_buf(),
                gefunden: fassung,
            });
        }
        let mut roh = [0u8; 32];
        roh.copy_from_slice(&kopf[10..42]);
        let gefunden = Hash::from_bytes(roh);
        if gefunden != startwert {
            return Err(SpeicherFehler::FremdeKette {
                pfad: pfad.to_path_buf(),
                erwartet: startwert,
                gefunden,
            });
        }

        // Sätze lesen, bis einer abbricht.
        let mut orte: Vec<u64> = Vec::new();
        let mut nach_hoehe: BTreeMap<u64, u64> = BTreeMap::new();
        let mut pos = KOPF_BYTES;
        let mut gueltig_bis = pos;
        let mut nummer = 0u64;
        let mut laengenkopf = [0u8; 4];
        let mut pruefkopf = [0u8; 4];
        loop {
            if leser.read_exact(&mut laengenkopf).is_err() {
                break;
            }
            let laenge = u32::from_le_bytes(laengenkopf);
            // Ein unsinniger Längenkopf ist ein Abbruch, keine
            // Speicheranforderung.
            if laenge == 0 || laenge > MAX_SATZ_BYTES {
                break;
            }
            let mut nutz = vec![0u8; laenge as usize];
            if leser.read_exact(&mut nutz).is_err() {
                break;
            }
            if leser.read_exact(&mut pruefkopf).is_err() {
                break;
            }
            if pruefsumme(&nutz) != u32::from_le_bytes(pruefkopf) {
                break;
            }
            let block: Block =
                borsh::from_slice(&nutz).map_err(|_| SpeicherFehler::UnlesbarerSatz {
                    pfad: pfad.to_path_buf(),
                    nummer,
                })?;
            orte.push(pos);
            nach_hoehe.insert(block.header.height, pos);
            nummer += 1;
            pos += 4 + laenge as u64 + 4;
            gueltig_bis = pos;
        }

        let abgeschnitten = dateilaenge.saturating_sub(gueltig_bis);
        if abgeschnitten > 0 {
            // Der Rest ist ein abgebrochener Satz. Kürzen, damit der
            // nächste Block sauber anhängt.
            datei.set_len(gueltig_bis).map_err(fehler)?;
        }
        datei.seek(SeekFrom::End(0)).map_err(fehler)?;
        // Erst **nach** dem Kürzen öffnen: Ein Lesegriff auf die
        // ungekürzte Datei wäre gleich wieder falsch.
        let lesegriff = File::open(pfad).map_err(fehler)?;

        let anzahl = orte.len() as u64;
        Ok((
            Self {
                pfad: pfad.to_path_buf(),
                datei,
                leser: lesegriff,
                orte,
                nach_hoehe,
                laenge: gueltig_bis,
                geschrieben: anzahl,
            },
            Wiederanlauf {
                anzahl,
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
        // ⚑ **Nach einem Schreibfehler wird die Länge nachgeschlagen,
        // nicht fortgeschrieben.** `write_all` kann mitten im Satz
        // scheitern, und dann stehen schon Bytes in der Datei. Wer
        // `self.laenge` unverändert ließe, verwiese den nächsten Satz
        // auf eine Stelle **vor** diesen Bytes, und der Verweis zeigte
        // ins Leere. Die Datei selbst weiß es besser.
        if let Err(e) = self.datei.write_all(&satz) {
            if let Ok(m) = self.datei.metadata() {
                self.laenge = m.len();
            }
            return Err(fehler(e));
        }
        self.datei.flush().map_err(fehler)?;
        // Der Verweis wird **nach** dem gelungenen Schreiben gesetzt.
        // Andersherum zeigte er nach einem Schreibfehler auf eine
        // Stelle, an der nichts steht.
        self.orte.push(self.laenge);
        self.nach_hoehe.insert(block.header.height, self.laenge);
        self.laenge += satz.len() as u64;
        self.geschrieben += 1;
        Ok(())
    }

    /// Liest den Satz, der bei `ort` anfängt.
    ///
    /// Geht über den **Lesegriff**, verschiebt die Schreibstelle also
    /// nicht (siehe Modulkopf).
    fn satz_lesen(&mut self, ort: u64) -> Result<Block, SpeicherFehler> {
        let unlesbar = || SpeicherFehler::SatzNichtLesbar {
            pfad: self.pfad.clone(),
            ort,
        };
        self.leser
            .seek(SeekFrom::Start(ort))
            .map_err(|_| unlesbar())?;
        let mut laengenkopf = [0u8; 4];
        self.leser
            .read_exact(&mut laengenkopf)
            .map_err(|_| unlesbar())?;
        let laenge = u32::from_le_bytes(laengenkopf);
        // Dieselbe Schranke wie beim Öffnen: Ein gekipptes Bit im
        // Längenkopf darf keine Speicheranforderung werden.
        if laenge == 0 || laenge > MAX_SATZ_BYTES {
            return Err(unlesbar());
        }
        let mut nutz = vec![0u8; laenge as usize];
        self.leser.read_exact(&mut nutz).map_err(|_| unlesbar())?;
        let mut pruefkopf = [0u8; 4];
        self.leser
            .read_exact(&mut pruefkopf)
            .map_err(|_| unlesbar())?;
        if pruefsumme(&nutz) != u32::from_le_bytes(pruefkopf) {
            return Err(unlesbar());
        }
        borsh::from_slice(&nutz).map_err(|_| unlesbar())
    }

    /// Reicht jeden Satz **in Dateireihenfolge** einmal an `f`.
    ///
    /// ⚑ **Der Weg des Wiederanlaufs.** Nicht über die Höhen, sondern
    /// über die Orte: Eine Datei mit doppelten Höhen soll nicht
    /// stillschweigend weniger Sätze abspielen, als sie enthält. Was
    /// nicht anschließt, weist `uebernimm` ab, und das ist die Stelle,
    /// an der das entschieden gehört.
    ///
    /// Immer nur **ein** Block liegt dabei im Arbeitsspeicher.
    pub fn fuer_jeden_satz(
        &mut self,
        mut f: impl FnMut(Block),
    ) -> Result<(), SpeicherFehler> {
        for i in 0..self.orte.len() {
            let ort = self.orte[i];
            f(self.satz_lesen(ort)?);
        }
        Ok(())
    }

    /// Alle Sätze auf einmal, **nur für Tests und Werkzeuge**.
    ///
    /// ⚑ **Nicht im Betrieb benutzen.** Genau diese Form, die ganze
    /// Kette in einem `Vec<Block>`, war Fund 124. Sie steht hier, weil
    /// ein Test die Datei als Ganzes prüfen darf: Er weiß, wie groß sie
    /// ist, weil er sie selbst geschrieben hat. Der Knoten weiß es
    /// nicht und nimmt [`Kettenspeicher::fuer_jeden_satz`].
    #[doc(hidden)]
    pub fn alle_saetze(&mut self) -> Result<Vec<Block>, SpeicherFehler> {
        let mut alle = Vec::new();
        self.fuer_jeden_satz(|b| alle.push(b))?;
        Ok(alle)
    }

    /// Der Block einer Höhe, falls die Datei ihn führt.
    ///
    /// `Ok(None)` heißt „nicht in dieser Datei". Ein `Err` heißt, der
    /// Satz steht da und ließ sich nicht lesen, und **das ist ein
    /// Unterschied**: Der erste Fall ist ein Nachzügler, der zu weit
    /// zurückfragt, der zweite eine beschädigte Platte.
    pub fn block_bei(&mut self, hoehe: u64) -> Result<Option<Block>, SpeicherFehler> {
        let Some(&ort) = self.nach_hoehe.get(&hoehe) else {
            return Ok(None);
        };
        self.satz_lesen(ort).map(Some)
    }

    /// Die kleinste Höhe, die die Datei führt.
    pub fn kleinste_hoehe(&self) -> Option<u64> {
        self.nach_hoehe.keys().next().copied()
    }

    /// Welche der Höhen `ab` bis einschließlich `bis` die Datei führt.
    ///
    /// ⚑ **Über den Verweis, nicht über die Zahlen.** Wer stattdessen
    /// `for h in ab..=bis` liefe und jede Höhe einzeln nachschlüge,
    /// arbeitete so lange wie die **Spanne**, nicht wie das Ergebnis,
    /// und eine Spanne kommt von außen: `Bloecke { ab: 0, bis: u64::MAX }`
    /// wäre eine Anfrage, die einen Knoten für immer beschäftigt. Der
    /// Bereich über die Verweistabelle kostet dagegen nur, was er
    /// findet.
    pub fn hoehen_von_bis(&self, ab: u64, bis: u64) -> Vec<u64> {
        if ab > bis {
            return Vec::new();
        }
        self.nach_hoehe.range(ab..=bis).map(|(h, _)| *h).collect()
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
        let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("öffnen");
        assert!(w.neu);
        assert_eq!(w.anzahl, 0);
        assert!(s.alle_saetze().unwrap().is_empty());
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
        let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).expect("wieder öffnen");
        assert!(!w.neu);
        assert_eq!(w.abgeschnitten, 0);
        assert_eq!(s.bloecke(), 5);
        assert_eq!(w.anzahl, 5);
        let gelesen = s.alle_saetze().expect("zurücklesen");
        assert_eq!(gelesen.len(), 5);
        for (i, b) in gelesen.iter().enumerate() {
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
            assert_eq!(w.anzahl, 1);
            s.anhaengen(&block(2)).unwrap();
            assert_eq!(s.bloecke(), 2);
            // Ein Block, der eben erst angehängt wurde, muss über den
            // Lesegriff schon sichtbar sein: Sonst hätte der
            // Zwischenspeicher in der Kette eine Lücke, die niemand
            // füllt.
            assert_eq!(s.block_bei(2).unwrap(), Some(block(2)));
        }
        let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        assert_eq!(w.anzahl, 2);
        assert_eq!(s.alle_saetze().unwrap()[1], block(2));
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
        assert_eq!(w.anzahl, 2, "die vollständigen Sätze müssen bleiben");
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
        let (mut s, w) = Kettenspeicher::oeffnen(&p, startwert()).unwrap();
        assert_eq!(w.anzahl, 2);
        assert_eq!(s.alle_saetze().unwrap()[1], block(2));
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
        assert_eq!(
            w.anzahl, 0,
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
        assert_eq!(w.anzahl, 0);
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
