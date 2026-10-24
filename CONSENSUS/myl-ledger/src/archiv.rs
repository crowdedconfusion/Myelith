//! **Die Archivpflicht im Konsens** (Archivknoten, Stufen A5 und A6,
//! 2026-10-23): wer in einer Periode die Geschichte hält, wie er es belegt,
//! was er dafür bekommt, was ihn trifft, wenn er es nicht tut, und wann die
//! Vorgänger loslassen dürfen.
//!
//! # ⚑ Beweis statt Behauptung
//!
//! An jeder Langzeitgrenze zieht der Zustand die Kopien der neuen Periode
//! ([`myl_types::archivwahl`]) und hält die Historienwurzel der Grenze fest.
//! Je Epoche bekommt jede Kopie aus der Epochensaat ihre Stichprobenhöhen
//! und belegt sie mit einer Transaktion: **die Blöcke selbst**, als Bytes,
//! und für jeden einen Beweis gegen diese Wurzel. Der Blockhash ist der
//! Hash dieser Bytes; wer nur Hashes aufhebt, kann nicht belegen.
//!
//! - **Die erste Belegung einer Kopie prüft die Vollständigkeit**: 32
//!   Stichproben über alle Höhen. Danach genügen 4 je Epoche.
//! - **Zwei Epochen in Folge ohne Beleg sind ein Verstoß**: Abzug vom
//!   Einsatz, Vermerk in der Verstoßhistorie, und das nächste Los rückt
//!   nach. Die Frist für die erste Belegung ist länger, denn eine neue
//!   Kopie muss erst nachladen.
//! - **Wer belegt, bekommt eine Gutschrift** aus dem, was die Treasury in
//!   der Epoche erhalten hat. Umgebucht, nicht geprägt: Die Geldmenge folgt
//!   weiter allein der Prägeformel.
//!
//! ⚑ **Der Verstoß wird beim Epochenwechsel festgestellt, nicht gemeldet.**
//! Ein Verstoß, den jemand einreichen muss, bleibt aus, wenn niemand es tut;
//! der Zustand weiss selbst, wer belegt hat.
//!
//! # ⚑ Übergabe nach Beweis
//!
//! Die Kopien einer Periode werden an der nächsten Grenze **Halter** und
//! behalten weiter, bis die Kopien der neuen Periode belegt haben. Erst
//! wenn mindestens [`UEBERGABE_MINDESTENS`] von ihnen die Vollständigkeit
//! belegt haben, lösen die neuen Halter die alten ab; sonst halten die alten
//! weiter, ohne Frist. Vor der ersten Ziehung und nach einer Periode ohne
//! Ziehung hält jeder.
//!
//! # ⚑ Die Splitter (Stufe A7)
//!
//! Die Splitter einer Grenze halten je einen der 64 Reed-Solomon-Teile jeder
//! Gruppe, die in der endenden Periode fertig wurde
//! ([`myl_types::archivteile`]). Gegen welche **Teilwurzel** sie belegen,
//! legen die Kopien fest: Jede rechnet sie aus ihren Blöcken nach und reicht
//! sie ein; sie gilt, sobald [`UEBERGABE_MINDESTENS`] dieselbe genannt haben.
//! Danach belegt jeder Splitter je Epoche einen gezogenen Teil. Splitter
//! werden bei einem Verstoß nicht ersetzt, denn ihr Index ist ihr Teil; die
//! Codierung verträgt 32 fehlende.
//!
//! ⚠️ **Was Stichproben nicht ausschliessen:** Wer die gezogenen Blöcke erst
//! nach dem Ziehen bei anderen holt, besteht ebenfalls. Er hält dann die
//! Pflicht nicht, solange andere sie halten; dagegen stehen die übrigen
//! Kopien, die Halter und die Splitter.

use std::collections::{BTreeMap, BTreeSet};

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::archivwahl::{self, Ziehung};
use myl_types::hash::Hash;
use myl_types::historie::Beweis;
use myl_types::ids::{Address, MinerId};

use crate::state::LedgerState;
use crate::transitions::TransitionError;

/// Stichproben je Epoche, sobald die Vollständigkeit belegt ist.
pub const STICHPROBEN_BELEG: usize = 4;

/// Stichproben der ersten Belegung einer Kopie: die Vollständigkeit.
pub const STICHPROBEN_VOLLSTAENDIG: usize = 32;

/// So viele Epochen in Folge ohne Beleg sind ein Verstoß.
pub const VERFEHLT_HOECHSTENS: u8 = 2;

/// So viele Epochen hat eine neue Kopie für die erste Belegung, bevor
/// verfehlte Epochen zählen: Zeit zum Nachladen.
///
/// ⚑ **Eine knappe halbe Periode** (Fund 628, Festlegung vom 2026-10-24):
/// das Größte, bei dem auch die Nachrückerin einer Kopie, die nie belegt,
/// noch in derselben Periode fertig wird, also Frist, zwei verfehlte
/// Epochen und noch einmal Frist höchstens 64 Epochen. Der Knoten prüft das
/// gegen seine Periodenlänge beim Übersetzen.
///
/// 📌 Bis zum 2026-10-24 waren es acht. Was eine neue Kopie nachladen muss,
/// wächst mit der Geschichte, die Frist nicht: Im Archivlauf brauchte eine
/// Kopie 509 s für 115 200 Blöcke, die acht Epochen und zwei verfehlte
/// waren dort rund 400 s, und sie wurde ersetzt, obwohl sie lud. Bei 2 s je
/// Block sind es jetzt 31 Stunden statt acht.
///
/// ⚠️ **Das verschiebt die Grenze, es hebt sie nicht auf.** Eine volle Kopie
/// holt die ganze Geschichte, und die wächst ohne Ende. Die Frist trägt,
/// solange ein Knoten die Geschichte in 31 Epochen laden kann; auf lange
/// Sicht muss die Haltbarkeit der alten Perioden bei den Splittern liegen.
///
/// ⚑ **Warum nicht eine Periode im Voraus ziehen:** Für die Nachrückerin
/// gälte dieselbe Bedingung, das Fenster rückte nur eine Periode früher.
/// Gewonnen wären belegte Kopien schon zum Beginn ihres Dienstes; die
/// Zwischenzeit decken die Halter, die bis zur Übergabe weiter halten.
/// Bezahlt würde mit einem zweiten Satz Kopien im Zustand, einer neuen
/// Rolle und Kopien, die eine Periode länger bekannt sind.
pub const FRIST_VOLLSTAENDIG_EPOCHEN: u64 = 31;

/// So viele Epochen hat ein Splitter ab der geltenden Teilwurzel, bevor
/// verfehlte Epochen zählen. Er holt nur seine Teile der Gruppen einer
/// Periode; das wächst nicht mit der Geschichte und bleibt deshalb bei acht.
pub const FRIST_TEILE_EPOCHEN: u64 = 8;

/// So viele Kopien einer Periode müssen die Vollständigkeit belegt haben,
/// damit sie die Halter der vorigen ablösen (bei weniger Kopien alle).
pub const UEBERGABE_MINDESTENS: usize = 5;

/// Anteil des Treasury-Eingangs einer Epoche, der an die belegenden Kopien
/// geht, in Basispunkten.
pub const GUTSCHRIFT_BPS: u64 = 500;

/// Anteil des Treasury-Eingangs für die belegenden Splitter, in
/// Basispunkten.
pub const TEIL_GUTSCHRIFT_BPS: u64 = 200;

/// Anteil des haftenden Einsatzes, der bei einem Verstoß an die Treasury
/// geht: ein Zehntel.
pub const ABZUG_NENNER: u64 = 10;

/// **Der Stand der Archivpflicht** im Zustand.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Archivstand {
    /// Höhe der Grenze, an der gezogen wurde; null vor der ersten.
    pub grenze: u64,
    /// Ihre Epoche, für die Wartezeit beim Nachrücken.
    pub grenze_epoche: u64,
    /// Die Saat der Ziehung, für das Nachrücken.
    pub saat: Hash,
    /// Die Historienwurzel bei `grenze` Blöcken: gegen sie wird belegt.
    pub wurzel: Hash,
    /// Ob gezogen wurde. Ohne Ziehung behält jeder alles.
    pub gezogen: bool,
    /// Die Kopien, nach Nachrücken in Folge des Eintritts.
    pub kopien: Vec<MinerId>,
    /// Die Splitter.
    pub splitter: Vec<MinerId>,
    /// Die Epoche, ab der eine Kopie Pflicht hat.
    pub eintritt: BTreeMap<MinerId, u64>,
    /// Wer die Vollständigkeit belegt hat.
    pub vollstaendig: BTreeSet<MinerId>,
    /// Wer in der laufenden Epoche belegt hat.
    pub belegt: BTreeSet<MinerId>,
    /// Epochen in Folge ohne Beleg.
    pub verfehlt: BTreeMap<MinerId, u8>,
    /// Wer in dieser Periode wegen eines Verstoßes ersetzt wurde.
    pub ersetzt: Vec<MinerId>,
    /// Wer die Geschichte bis zur Übergabe weiter hält.
    pub halter: Vec<MinerId>,
    /// Jeder hält: vor der ersten bestätigten Übergabe.
    pub halter_alle: bool,
    /// Die Gruppen, deren Teile die Splitter halten, halboffen.
    pub teil_ab: u64,
    pub teil_bis: u64,
    /// Die geltende Teilwurzel, sobald genug Kopien sie gleich genannt haben.
    pub teilwurzel: Option<Hash>,
    /// Die Epoche, in der sie galt: ab da läuft die Frist der Splitter.
    pub teilwurzel_epoche: u64,
    /// Was die Kopien genannt haben.
    pub teilstimmen: BTreeMap<MinerId, Hash>,
    /// Die Teilwurzeln aller Abschnitte, mit der Grenze: für die
    /// Wiederherstellung aus Teilen.
    pub teilwurzeln: Vec<(u64, Hash)>,
    /// Splitter, die in der laufenden Epoche belegt haben.
    pub teil_belegt: BTreeSet<MinerId>,
    /// Ihre Epochen in Folge ohne Beleg.
    pub teil_verfehlt: BTreeMap<MinerId, u8>,
}

impl Default for Archivstand {
    fn default() -> Self {
        Self {
            grenze: 0,
            grenze_epoche: 0,
            saat: Hash([0; 32]),
            wurzel: Hash([0; 32]),
            gezogen: false,
            kopien: Vec::new(),
            splitter: Vec::new(),
            eintritt: BTreeMap::new(),
            vollstaendig: BTreeSet::new(),
            belegt: BTreeSet::new(),
            verfehlt: BTreeMap::new(),
            ersetzt: Vec::new(),
            halter: Vec::new(),
            halter_alle: true,
            teil_ab: 0,
            teil_bis: 0,
            teilwurzel: None,
            teilwurzel_epoche: 0,
            teilstimmen: BTreeMap::new(),
            teilwurzeln: Vec::new(),
            teil_belegt: BTreeSet::new(),
            teil_verfehlt: BTreeMap::new(),
        }
    }
}

impl Archivstand {
    /// **Muss `wer` die volle Geschichte behalten?** Ja, solange nicht
    /// gezogen ist, solange jeder hält, als Kopie und als Halter.
    pub fn behaelt(&self, wer: Option<&MinerId>) -> bool {
        !self.gezogen
            || self.halter_alle
            || wer.is_some_and(|m| self.kopien.contains(m) || self.halter.contains(m))
    }

    /// Ist `wer` Kopie der laufenden Periode?
    pub fn ist_kopie(&self, wer: &MinerId) -> bool {
        self.gezogen && self.kopien.contains(wer)
    }

    /// Haben genug Kopien die Vollständigkeit belegt, um die Halter
    /// abzulösen?
    pub fn uebergabe_bestaetigt(&self) -> bool {
        self.gezogen
            && !self.kopien.is_empty()
            && self.vollstaendig.iter().filter(|m| self.kopien.contains(m)).count()
                >= UEBERGABE_MINDESTENS.min(self.kopien.len())
    }

    /// Welcher Teil gehört `wer`? Der Index in der Liste der Splitter.
    pub fn teilindex(&self, wer: &MinerId) -> Option<usize> {
        if !self.gezogen {
            return None;
        }
        self.splitter.iter().position(|m| m == wer)
    }

    /// Wie viele Stichproben `wer` in dieser Epoche belegen muss.
    pub fn stichprobenzahl(&self, wer: &MinerId) -> usize {
        if self.vollstaendig.contains(wer) {
            STICHPROBEN_BELEG
        } else {
            STICHPROBEN_VOLLSTAENDIG
        }
    }
}

/// **Die Stichprobenhöhen** einer Kopie in einer Epoche: `anzahl` Höhen
/// in `1 .. grenze`, aus Epochensaat, Epoche und Kennung.
pub fn stichproben(a: &Archivstand, epochensaat: &Hash, epoche: u64, wer: &MinerId, anzahl: usize) -> Vec<u64> {
    if a.grenze < 2 {
        return Vec::new();
    }
    (0..anzahl as u64)
        .map(|j| {
            let mut e = Vec::with_capacity(16 + 32 + 8 + 32 + 8);
            e.extend_from_slice(b"myl-archiv-probe");
            e.extend_from_slice(epochensaat.as_bytes());
            e.extend_from_slice(&epoche.to_le_bytes());
            e.extend_from_slice(wer.as_bytes());
            e.extend_from_slice(&j.to_le_bytes());
            let h = Hash::sha256(&e);
            let mut acht = [0u8; 8];
            acht.copy_from_slice(&h.as_bytes()[..8]);
            1 + u64::from_le_bytes(acht) % (a.grenze - 1)
        })
        .collect()
}

/// **Die Stelle der Gruppe**, deren Teil ein Splitter in dieser Epoche
/// belegt; `None` ohne Gruppen.
pub fn teilprobe(a: &Archivstand, epochensaat: &Hash, epoche: u64, wer: &MinerId) -> Option<u64> {
    let n = a.teil_bis.checked_sub(a.teil_ab).filter(|&n| n > 0)?;
    let mut e = Vec::with_capacity(16 + 32 + 8 + 32);
    e.extend_from_slice(b"myl-archiv-teil");
    e.extend_from_slice(epochensaat.as_bytes());
    e.extend_from_slice(&epoche.to_le_bytes());
    e.extend_from_slice(wer.as_bytes());
    let h = Hash::sha256(&e);
    let mut acht = [0u8; 8];
    acht.copy_from_slice(&h.as_bytes()[..8]);
    Some(u64::from_le_bytes(acht) % n)
}

/// **Die Ziehung an einer Langzeitgrenze**, beim Anwenden des Blocks
/// `hoehe`, nachdem dessen Vorgänger in der Historie steht. Vorher wird
/// über die Übergabe der endenden Periode entschieden.
pub fn grenze_ziehen(state: &mut LedgerState, hoehe: u64, grenze_epoche: u64, warte_epochen: u64) {
    let a = &mut state.archiv;
    if a.uebergabe_bestaetigt() {
        a.halter = a.kopien.clone();
        a.halter_alle = false;
    } else if !a.gezogen {
        a.halter_alle = true;
    } else {
        for k in a.kopien.clone() {
            if !a.halter.contains(&k) {
                a.halter.push(k);
            }
        }
    }
    let ziehung = archivwahl::ziehen(&state.epochensaat, &state.miner, grenze_epoche, warte_epochen);
    let a = &mut state.archiv;
    (a.teil_ab, a.teil_bis) = myl_types::archivteile::abschnitt(a.grenze, hoehe);
    a.teilwurzel = None;
    a.teilwurzel_epoche = 0;
    a.teilstimmen.clear();
    a.teil_belegt.clear();
    a.teil_verfehlt.clear();
    a.grenze = hoehe;
    a.grenze_epoche = grenze_epoche;
    a.saat = state.epochensaat;
    a.wurzel = state.historie.wurzel();
    a.vollstaendig.clear();
    a.belegt.clear();
    a.verfehlt.clear();
    a.ersetzt.clear();
    a.eintritt.clear();
    match ziehung {
        Ziehung::AlleBehalten => {
            a.gezogen = false;
            a.kopien.clear();
            a.splitter.clear();
        }
        Ziehung::Gezogen { kopien, splitter } => {
            a.gezogen = true;
            for k in &kopien {
                a.eintritt.insert(*k, grenze_epoche);
            }
            a.kopien = kopien;
            a.splitter = splitter;
        }
    }
}

/// **Ein Archivbeleg**: die Blöcke der Stichproben des Absenders, als
/// Bytes, und je ein Beweis gegen die Historienwurzel der Grenze.
pub fn archiv_belegen(
    state: &mut LedgerState,
    absender: &Address,
    bloecke: &[Vec<u8>],
    beweise: &[Beweis],
) -> Result<(), TransitionError> {
    let wer = MinerId::new(*absender.as_bytes());
    let a = &state.archiv;
    if !a.ist_kopie(&wer) {
        return Err(TransitionError::NichtArchivkopie);
    }
    if a.belegt.contains(&wer) {
        return Err(TransitionError::ArchivBereitsBelegt);
    }
    let anzahl = a.stichprobenzahl(&wer);
    let hoehen = stichproben(a, &state.epochensaat, state.epoch.0, &wer, anzahl);
    if hoehen.len() != anzahl || bloecke.len() != anzahl || beweise.len() != anzahl {
        return Err(TransitionError::ArchivbelegUngueltig);
    }
    for ((h, roh), bw) in hoehen.iter().zip(bloecke).zip(beweise) {
        if bw.index != *h || bw.anzahl != a.grenze || !bw.pruefen(&a.wurzel, &Hash::sha256(roh)) {
            return Err(TransitionError::ArchivbelegUngueltig);
        }
    }
    let a = &mut state.archiv;
    a.belegt.insert(wer);
    a.verfehlt.remove(&wer);
    if anzahl == STICHPROBEN_VOLLSTAENDIG {
        a.vollstaendig.insert(wer);
    }
    Ok(())
}

/// **Eine Kopie nennt die Teilwurzel** des Abschnitts. Sie gilt, sobald
/// [`UEBERGABE_MINDESTENS`] Kopien (bei weniger alle) dieselbe genannt
/// haben; danach ist nichts mehr zu nennen.
pub fn teilwurzel_nennen(state: &mut LedgerState, absender: &Address, wurzel: Hash) -> Result<(), TransitionError> {
    let wer = MinerId::new(*absender.as_bytes());
    let a = &mut state.archiv;
    if !a.ist_kopie(&wer) {
        return Err(TransitionError::NichtArchivkopie);
    }
    if a.teilwurzel.is_some() || a.teil_ab == a.teil_bis || a.teilstimmen.contains_key(&wer) {
        return Err(TransitionError::ArchivBereitsBelegt);
    }
    a.teilstimmen.insert(wer, wurzel);
    let gleich = a.teilstimmen.iter().filter(|(m, w)| **w == wurzel && a.kopien.contains(m)).count();
    if gleich >= UEBERGABE_MINDESTENS.min(a.kopien.len()) {
        a.teilwurzel = Some(wurzel);
        a.teilwurzel_epoche = state.epoch.0;
        a.teilwurzeln.push((a.grenze, wurzel));
    }
    Ok(())
}

/// **Ein Teilbeleg** eines Splitters: sein Teil der gezogenen Gruppe und
/// der Beweis gegen die Teilwurzel.
pub fn teil_belegen(
    state: &mut LedgerState,
    absender: &Address,
    stueck: &[u8],
    beweis: &myl_types::archivteile::Teilbeweis,
) -> Result<(), TransitionError> {
    let wer = MinerId::new(*absender.as_bytes());
    let a = &state.archiv;
    let Some(index) = a.teilindex(&wer) else {
        return Err(TransitionError::NichtArchivkopie);
    };
    let Some(wurzel) = a.teilwurzel else {
        return Err(TransitionError::ArchivbelegUngueltig);
    };
    if a.teil_belegt.contains(&wer) {
        return Err(TransitionError::ArchivBereitsBelegt);
    }
    let stelle = teilprobe(a, &state.epochensaat, state.epoch.0, &wer);
    if Some(beweis.stelle) != stelle || beweis.teil != index as u64 || !beweis.pruefen(&wurzel, stueck) {
        return Err(TransitionError::ArchivbelegUngueltig);
    }
    state.archiv.teil_belegt.insert(wer);
    Ok(())
}

/// Was ein Epochenabschluss der Archivpflicht bewirkt hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Archivabschluss {
    /// Wer eine Gutschrift bekam, und wie viel je Kopie.
    pub gutgeschrieben: Vec<MinerId>,
    pub je_kopie: u64,
    /// Wer wegen eines Verstoßes ersetzt wurde, und durch wen.
    pub ersetzt: Vec<(MinerId, Option<MinerId>)>,
    /// Je belegendem Splitter.
    pub je_splitter: u64,
    /// Splitter mit Verstoß (sie bleiben, ihr Index ist ihr Teil).
    pub splitter_verstoesse: Vec<MinerId>,
}

/// Bucht `bps` des Treasury-Eingangs gleich verteilt an `wer` um; gibt den
/// Betrag je Empfänger zurück.
fn gutschreiben(state: &mut LedgerState, wer: &[MinerId], eingang: u64, bps: u64, aus: &mut Vec<MinerId>) -> u64 {
    if wer.is_empty() {
        return 0;
    }
    let je = (eingang as u128 * bps as u128 / 10_000) as u64 / wer.len() as u64;
    if je == 0 {
        return 0;
    }
    let treasury = myl_types::treasury::treasury_adresse();
    for m in wer {
        let ziel = crate::transitions::auszahlungskonto(state, m).unwrap_or(Address::new(*m.as_bytes()));
        if crate::transitions::transfer(state, &treasury, &ziel, je).is_ok() {
            aus.push(*m);
        }
    }
    je
}

/// **Der Abschluss einer Epoche**: Gutschrift für die Belegenden, verfehlte
/// Epochen zählen, bei einem Verstoß Abzug, Vermerk und Nachrücken.
/// `treasury_eingang` ist, was die Treasury in der Epoche bekommen hat.
pub fn epoche_abschliessen(
    state: &mut LedgerState,
    alte_epoche: u64,
    treasury_eingang: u64,
    warte_epochen: u64,
) -> Archivabschluss {
    let mut aus = Archivabschluss::default();
    if !state.archiv.gezogen {
        state.archiv.belegt.clear();
        return aus;
    }
    // Gutschrift, umgebucht aus der Treasury: Kopien und Splitter getrennt.
    let belegt: Vec<MinerId> =
        state.archiv.belegt.iter().copied().filter(|m| state.archiv.kopien.contains(m)).collect();
    aus.je_kopie = gutschreiben(state, &belegt, treasury_eingang, GUTSCHRIFT_BPS, &mut aus.gutgeschrieben);
    let teil_belegt: Vec<MinerId> =
        state.archiv.teil_belegt.iter().copied().filter(|m| state.archiv.splitter.contains(m)).collect();
    aus.je_splitter = gutschreiben(state, &teil_belegt, treasury_eingang, TEIL_GUTSCHRIFT_BPS, &mut aus.gutgeschrieben);
    // Verfehlte Epochen der Splitter: erst, wenn die Teilwurzel gilt und
    // die Frist zum Holen verstrichen ist. Kein Nachrücken.
    if state.archiv.teilwurzel.is_some()
        && (alte_epoche + 1).saturating_sub(state.archiv.teilwurzel_epoche) >= FRIST_TEILE_EPOCHEN
    {
        for s in state.archiv.splitter.clone() {
            if state.archiv.teil_belegt.contains(&s) {
                state.archiv.teil_verfehlt.remove(&s);
                continue;
            }
            let n = state.archiv.teil_verfehlt.entry(s).or_insert(0);
            *n = n.saturating_add(1);
            if *n >= VERFEHLT_HOECHSTENS {
                state.archiv.teil_verfehlt.remove(&s);
                verstoss(state, &s);
                aus.splitter_verstoesse.push(s);
            }
        }
    }
    state.archiv.teil_belegt.clear();
    // Verfehlte Epochen.
    for k in state.archiv.kopien.clone() {
        if state.archiv.belegt.contains(&k) {
            continue;
        }
        let eintritt = state.archiv.eintritt.get(&k).copied().unwrap_or(state.archiv.grenze_epoche);
        let pflichtig = state.archiv.vollstaendig.contains(&k)
            || (alte_epoche + 1).saturating_sub(eintritt) >= FRIST_VOLLSTAENDIG_EPOCHEN;
        if !pflichtig {
            continue;
        }
        let n = state.archiv.verfehlt.entry(k).or_insert(0);
        *n = n.saturating_add(1);
        if *n < VERFEHLT_HOECHSTENS {
            continue;
        }
        verstoss(state, &k);
        let a = &mut state.archiv;
        a.kopien.retain(|m| *m != k);
        a.vollstaendig.remove(&k);
        a.verfehlt.remove(&k);
        a.eintritt.remove(&k);
        a.ersetzt.push(k);
        // ⚑ Zuerst unter denen ohne Rolle; gibt es keinen, rückt ein
        //   Splitter nach und hält dann beides: Sein Index ist sein Teil und
        //   bleibt stehen. Eine fehlende Kopie wiegt schwerer als die Mühe.
        let mut ausgeschlossen = a.splitter.clone();
        ausgeschlossen.extend(a.ersetzt.iter().copied());
        let neu = archivwahl::nachruecken(&a.saat, &state.miner, a.grenze_epoche, warte_epochen, &a.kopien, &ausgeschlossen)
            .or_else(|| archivwahl::nachruecken(&a.saat, &state.miner, a.grenze_epoche, warte_epochen, &a.kopien, &a.ersetzt));
        if let Some(n) = neu {
            let a = &mut state.archiv;
            a.kopien.push(n);
            a.eintritt.insert(n, alte_epoche + 1);
        }
        aus.ersetzt.push((k, neu));
    }
    state.archiv.belegt.clear();
    aus
}

/// Abzug eines Zehntels des haftenden Einsatzes an die Treasury, dazu der
/// Vermerk in der Verstoßhistorie, der die Staffel künftiger Urteile hebt.
fn verstoss(state: &mut LedgerState, wer: &MinerId) {
    let adresse = Address::new(*wer.as_bytes());
    let konto = state.account(&adresse);
    let abzug = crate::einsatz::schlachtbar(konto.staked, &konto.gekuendigt) / ABZUG_NENNER;
    if abzug > 0 {
        let k = state.account_mut(&adresse);
        let genommen = crate::einsatz::nimm_aus_der_masse(&mut k.staked, &mut k.gekuendigt, abzug);
        let treasury = myl_types::treasury::treasury_adresse();
        let t = state.account_mut(&treasury);
        t.balance = t.balance.saturating_add(genommen);
    }
    state.verstoss_vermerken(&adresse);
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::historie::{beweis_aus_blaettern, blatt, Historie};
    use myl_types::miner::{HardwareClass, MinerRegistration};
    use myl_types::node_metadata::GeoRegion;

    fn id(i: u8) -> MinerId {
        MinerId::new([i; 32])
    }

    /// Ein Zustand mit `n` Minern aus Epoche 0 und einer Historie über
    /// `grenze` erfundene Blöcke; dazu die Blöcke als Bytes.
    fn zustand(n: u8, grenze: u64) -> (LedgerState, Vec<Vec<u8>>) {
        let mut z = LedgerState::genesis(1);
        for i in 1..=n {
            z.miner.insert(
                id(i),
                MinerRegistration {
                    miner_id: id(i),
                    hardware_class: HardwareClass::MediumGpu,
                    registration_epoch: 0,
                    zone: [GeoRegion::Europe, GeoRegion::Asia, GeoRegion::Africa][i as usize % 3],
                    schluessel: myl_types::bls::BlsPublicKey([0; 48]),
                    netzadresse: myl_types::latency_attest::PeerIdBytes([i; 32]),
                },
            );
        }
        let bloecke: Vec<Vec<u8>> = (0..grenze).map(|h| format!("block {h}").into_bytes()).collect();
        let mut h = Historie::default();
        bloecke.iter().for_each(|b| h.anhaengen(&Hash::sha256(b)));
        z.historie = h;
        z.epochensaat = Hash::sha256(b"saat");
        (z, bloecke)
    }

    fn beleg(z: &LedgerState, bloecke: &[Vec<u8>], wer: &MinerId) -> (Vec<Vec<u8>>, Vec<Beweis>) {
        let a = &z.archiv;
        let blaetter: Vec<Hash> = bloecke.iter().map(|b| blatt(&Hash::sha256(b))).collect();
        let hoehen = stichproben(a, &z.epochensaat, z.epoch.0, wer, a.stichprobenzahl(wer));
        let roh = hoehen.iter().map(|&h| bloecke[h as usize].clone()).collect();
        let bw = hoehen.iter().map(|&h| beweis_aus_blaettern(&blaetter, h).expect("Beweis")).collect();
        (roh, bw)
    }

    /// ⚑ **Belegen, vollständig werden, übergeben**: Die erste Belegung
    /// prüft 32 Stichproben, danach 4; haben fünf Kopien belegt, lösen sie
    /// an der nächsten Grenze die alten Halter ab.
    #[test]
    fn belegen_macht_vollstaendig_und_die_uebergabe_folgt() {
        let (mut z, bloecke) = zustand(10, 500);
        grenze_ziehen(&mut z, 500, 64, 64);
        assert!(z.archiv.gezogen && z.archiv.halter_alle, "vor der ersten Übergabe hält jeder");
        let kopien = z.archiv.kopien.clone();
        assert_eq!(kopien.len(), 7);
        assert!(z.archiv.behaelt(None));
        for k in &kopien[..5] {
            let (roh, bw) = beleg(&z, &bloecke, k);
            assert_eq!(roh.len(), STICHPROBEN_VOLLSTAENDIG);
            archiv_belegen(&mut z, &Address::new(*k.as_bytes()), &roh, &bw).expect("Beleg");
        }
        assert!(z.archiv.uebergabe_bestaetigt());
        assert_eq!(z.archiv.stichprobenzahl(&kopien[0]), STICHPROBEN_BELEG);
        // Zweimal in derselben Epoche: nein.
        let (roh, bw) = beleg(&z, &bloecke, &kopien[0]);
        assert_eq!(
            archiv_belegen(&mut z, &Address::new(*kopien[0].as_bytes()), &roh, &bw),
            Err(TransitionError::ArchivBereitsBelegt)
        );
        epoche_abschliessen(&mut z, 64, 0, 64);
        grenze_ziehen(&mut z, 500, 128, 64);
        assert!(!z.archiv.halter_alle);
        assert_eq!(z.archiv.halter, kopien, "die belegten Kopien halten jetzt");
        let fremd = (1..=10).map(id).find(|m| !kopien.contains(m) && !z.archiv.kopien.contains(m));
        if let Some(f) = fremd {
            assert!(!z.archiv.behaelt(Some(&f)), "ein Unbeteiligter muss nichts halten");
        }
    }

    /// ⛔️ **Ohne Übergabe halten die alten weiter**, auch über eine zweite
    /// Grenze.
    #[test]
    fn ohne_uebergabe_halten_alle_weiter() {
        let (mut z, _) = zustand(10, 500);
        grenze_ziehen(&mut z, 500, 64, 64);
        grenze_ziehen(&mut z, 500, 128, 64);
        assert!(z.archiv.halter_alle, "ohne bestätigte Übergabe durfte jemand loslassen");
    }

    /// ⛔️ **Falsche Belege fallen durch**: Nicht-Kopie, verfälschter Block,
    /// fremder Beweis, zu wenige Stichproben.
    #[test]
    fn falsche_belege_fallen_durch() {
        let (mut z, bloecke) = zustand(10, 500);
        grenze_ziehen(&mut z, 500, 64, 64);
        let k = z.archiv.kopien[0];
        let adr = Address::new(*k.as_bytes());
        let fremd = (1..=10).map(id).find(|m| !z.archiv.kopien.contains(m)).expect("eine Nicht-Kopie");
        let (roh, bw) = beleg(&z, &bloecke, &k);
        assert_eq!(
            archiv_belegen(&mut z, &Address::new(*fremd.as_bytes()), &roh, &bw),
            Err(TransitionError::NichtArchivkopie)
        );
        let mut falsch = roh.clone();
        falsch[3].push(0);
        assert_eq!(archiv_belegen(&mut z, &adr, &falsch, &bw), Err(TransitionError::ArchivbelegUngueltig));
        let mut bw2 = bw.clone();
        bw2.swap(0, 1);
        if bw2[0].index != bw[0].index {
            assert_eq!(archiv_belegen(&mut z, &adr, &roh, &bw2), Err(TransitionError::ArchivbelegUngueltig));
        }
        assert_eq!(
            archiv_belegen(&mut z, &adr, &roh[..4], &bw[..4]),
            Err(TransitionError::ArchivbelegUngueltig),
            "vier Stichproben vor der Vollständigkeit"
        );
        assert!(z.archiv.belegt.is_empty());
        archiv_belegen(&mut z, &adr, &roh, &bw).expect("der echte Beleg");
    }

    /// ⚑ **Zwei verfehlte Epochen nach der Frist sind ein Verstoß**: Abzug
    /// eines Zehntels an die Treasury, Vermerk, ein Nachrücker. Wer belegt,
    /// bekommt seinen Anteil des Treasury-Eingangs.
    #[test]
    fn verfehlen_kostet_und_belegen_bringt() {
        let (mut z, bloecke) = zustand(20, 500);
        grenze_ziehen(&mut z, 500, 64, 64);
        let treasury = myl_types::treasury::treasury_adresse();
        z.account_mut(&treasury).balance = 1_000_000;
        let saeumig = z.archiv.kopien[1];
        let fleissig = z.archiv.kopien[0];
        z.account_mut(&Address::new(*saeumig.as_bytes())).staked = 10_000;
        let mut ersetzt = Vec::new();
        for e in 64..64 + FRIST_VOLLSTAENDIG_EPOCHEN + 1 {
            z.epoch = myl_types::ids::EpochId(e);
            for k in z.archiv.kopien.clone() {
                if k != saeumig {
                    let (roh, bw) = beleg(&z, &bloecke, &k);
                    archiv_belegen(&mut z, &Address::new(*k.as_bytes()), &roh, &bw).expect("Beleg");
                }
            }
            let ab = epoche_abschliessen(&mut z, e, 100_000, 64);
            assert!(ab.gutgeschrieben.contains(&fleissig));
            assert_eq!(ab.je_kopie, 100_000 * GUTSCHRIFT_BPS / 10_000 / ab.gutgeschrieben.len() as u64);
            ersetzt.extend(ab.ersetzt);
        }
        assert_eq!(ersetzt.len(), 1, "genau der Säumige");
        let (weg, neu) = ersetzt[0];
        assert_eq!(weg, saeumig);
        let neu = neu.expect("ein Nachrücker");
        assert!(z.archiv.kopien.contains(&neu) && !z.archiv.kopien.contains(&saeumig));
        assert_eq!(z.account(&Address::new(*saeumig.as_bytes())).staked, 9_000);
        assert_eq!(z.verstoesse_im_fenster(&Address::new(*saeumig.as_bytes()), crate::state::VERSTOSS_FENSTER), 1);
        assert!(z.account(&Address::new(*fleissig.as_bytes())).balance > 0);
    }

    /// ⚑ **Eine Kopie, die lange nachlädt, verliert ihren Platz nicht**
    /// (Fund 628): Belegt sie die Vollständigkeit erst nach dreissig Epochen,
    /// gibt es keinen Verstoß und keinen Nachrücker; danach gilt die Pflicht
    /// je Epoche wie für jede. Die dreissig stehen fest und nicht als
    /// Ableitung aus der Frist: Sonst wanderte der Test mit, wenn jemand sie
    /// kürzt, und das tat er in der ersten Fassung.
    #[test]
    fn eine_kopie_hat_zum_nachladen_eine_knappe_halbe_periode() {
        let (mut z, bloecke) = zustand(20, 500);
        grenze_ziehen(&mut z, 500, 64, 64);
        let langsam = z.archiv.kopien[2];
        let adr = Address::new(*langsam.as_bytes());
        z.account_mut(&adr).staked = 10_000;
        let geladen = 64 + 30;
        let mut ersetzt = Vec::new();
        for e in 64..geladen + 4 {
            z.epoch = myl_types::ids::EpochId(e);
            for k in z.archiv.kopien.clone() {
                if k != langsam || e >= geladen {
                    let (roh, bw) = beleg(&z, &bloecke, &k);
                    archiv_belegen(&mut z, &Address::new(*k.as_bytes()), &roh, &bw).expect("Beleg");
                }
            }
            ersetzt.extend(epoche_abschliessen(&mut z, e, 0, 64).ersetzt);
        }
        assert!(ersetzt.is_empty(), "ersetzt: {ersetzt:?}");
        assert!(z.archiv.kopien.contains(&langsam) && z.archiv.vollstaendig.contains(&langsam));
        assert_eq!(z.account(&adr).staked, 10_000);
        assert_eq!(z.verstoesse_im_fenster(&adr, crate::state::VERSTOSS_FENSTER), 0);
    }

    /// ⚑ **Splitter**: Die Teilwurzel gilt erst bei fünf gleichen Stimmen
    /// der Kopien; danach belegt ein Splitter seinen Teil der gezogenen
    /// Gruppe, ein fremder Teil fällt durch, und wer nach der Frist zweimal
    /// fehlt, verstößt, ohne ersetzt zu werden.
    #[test]
    fn splitter_belegen_ihre_teile_gegen_die_gewaehlte_teilwurzel() {
        use myl_types::archivteile::{gruppenwurzel, kodieren, teilwurzel, Teilbeweis};
        let (mut z, _) = zustand(80, 3 * 1024 + 10);
        grenze_ziehen(&mut z, 3 * 1024 + 10, 64, 64);
        assert_eq!((z.archiv.teil_ab, z.archiv.teil_bis), (0, 3));
        assert_eq!(z.archiv.splitter.len(), 64);
        // Drei Gruppen erfundener Blöcke, ihre Teile und Wurzeln.
        let gruppen: Vec<Vec<Vec<u8>>> =
            (0..3u8).map(|g| (0..1024u32).map(|i| format!("g{g} b{i}").into_bytes()).collect()).collect();
        let teile: Vec<Vec<Vec<u8>>> = gruppen.iter().map(|g| kodieren(g)).collect();
        let wurzeln: Vec<Hash> = teile.iter().map(|t| gruppenwurzel(t).expect("Wurzel")).collect();
        let tw = teilwurzel(&wurzeln).expect("Teilwurzel");
        let kopien = z.archiv.kopien.clone();
        // Die Teilwurzel gilt in Epoche 64: Ab da läuft die Frist der
        // Splitter, und sie muss kürzer sein als die der Kopien.
        z.epoch = myl_types::ids::EpochId(64);
        // Eine abweichende Stimme und vier richtige: noch nicht.
        teilwurzel_nennen(&mut z, &Address::new(*kopien[0].as_bytes()), Hash::sha256(b"falsch")).expect("Stimme");
        for k in &kopien[1..5] {
            teilwurzel_nennen(&mut z, &Address::new(*k.as_bytes()), tw).expect("Stimme");
        }
        assert_eq!(z.archiv.teilwurzel, None, "vier gleiche Stimmen reichen nicht");
        teilwurzel_nennen(&mut z, &Address::new(*kopien[5].as_bytes()), tw).expect("Stimme");
        assert_eq!(z.archiv.teilwurzel, Some(tw));
        assert_eq!(z.archiv.teilwurzeln, vec![(3 * 1024 + 10, tw)]);

        let s = z.archiv.splitter[9];
        let adr = Address::new(*s.as_bytes());
        let stelle = teilprobe(&z.archiv, &z.epochensaat, z.epoch.0, &s).expect("Stelle") as usize;
        let bw = Teilbeweis::bauen(&teile[stelle], 9, &wurzeln, stelle).expect("Beweis");
        let fremd = Teilbeweis::bauen(&teile[stelle], 10, &wurzeln, stelle).expect("Beweis");
        assert_eq!(teil_belegen(&mut z, &adr, &teile[stelle][10], &fremd), Err(TransitionError::ArchivbelegUngueltig), "fremder Teil");
        assert_eq!(
            teil_belegen(&mut z, &adr, &teile[stelle][10], &bw),
            Err(TransitionError::ArchivbelegUngueltig),
            "richtiger Beweis, anderer Inhalt"
        );
        teil_belegen(&mut z, &adr, &teile[stelle][9], &bw).expect("eigener Teil");
        assert_eq!(teil_belegen(&mut z, &adr, &teile[stelle][9], &bw), Err(TransitionError::ArchivBereitsBelegt));

        // Ein Splitter, der nie belegt, verstößt nach Frist und zwei Epochen.
        let saeumig = z.archiv.splitter[20];
        let mut verstoesse = 0;
        for e in 64..64 + 8 + 2 {
            z.epoch = myl_types::ids::EpochId(e);
            verstoesse += epoche_abschliessen(&mut z, e, 0, 64).splitter_verstoesse.iter().filter(|m| **m == saeumig).count();
        }
        assert!(verstoesse >= 1);
        assert!(z.archiv.splitter.contains(&saeumig), "ein Splitter wird nicht ersetzt");
        assert!(z.verstoesse_im_fenster(&Address::new(*saeumig.as_bytes()), crate::state::VERSTOSS_FENSTER) >= 1);
    }
}
