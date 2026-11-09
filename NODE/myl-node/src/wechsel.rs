//! **Wechselbelege** (CONSENSUS 7.3 und 7.4, 2026-11-04): wie ein Knoten
//! ohne Zustand dem Stimmsatz von Genesis an folgt.
//!
//! ⚑ **Ein Beleg je Epoche:** ein Pruefpunktzertifikat der Epoche `e`,
//! unterschrieben vom Satz, der dort die Finalitaet traegt, die beiden
//! Saetze fuer `e + 1` und `e + 2`, an die seine Bindung haengt
//! (`myl_ledger::stimmsatz::bindung`), und ⚑ seit 2026-11-04 das Zertifikat
//! des naechsten Pruefpunkts, das ihn als Quelle nennt und damit endgueltig
//! macht ([`Wechselbeleg::abschluss`]). Ein nur
//! gerechtfertigter Pruefpunkt kann auf einem Zweig liegen, den die Kette
//! verlaesst; seine Bindung bewiese dann nichts. Wer den Satz von `e` kennt, kennt nach
//! dem Beleg die beiden naechsten. Je
//! Wechsel ein Beleg, kein Block dazwischen. Am Ende steht ein Zertifikat,
//! gegen dessen Block ein leichter Abzug geprueft wird; das ersetzt den
//! Anker von aussen.
//!
//! ⚑ **Dieselbe Regel wie die Kette** ([`crate::kette::Kette::finalitaetssatz_von`]):
//! Solange kein Satz aus dem Zustand bekannt ist, der nicht nach `e` liegt,
//! traegt der Satz von aussen (Genesis); danach nur der Satz der Epoche, und
//! nur, wenn er tragfaehig ist. ⛔️ Zwei Belege, die fuer dieselbe Epoche
//! verschiedene Saetze binden, brechen die Folge ab: Das waeren zwei
//! Geschichten, und eine davon traegt eine Doppelstimme von mehr als einem
//! Drittel.
//!
//! Geprueft wird nur, was vorgelegt wird; eine Gegenstelle kann Belege
//! zurueckhalten, aber keinen falschen Satz unterschieben.
//!
//! ⚑ **Die Bruecke ueber eine Luecke** (2026-11-07): Ruht die Finalitaet
//! zwei Epochen oder laenger, entsteht fuer sie kein Beleg, und der Satz
//! danach ist nirgends gebunden. 📌 Gefunden im Nachtlauf `nacht-konsens`:
//! Nach fuenf Epochen Ruhe nahm der zustandslose Miner nach seinem Neustart
//! keinen Anker mehr an (`Unbekannt(15)`), fuer immer. Ein Zertifikat einer
//! unbekannten Epoche traegt darum auch, wenn es gegen den juengsten
//! bekannten Satz **aus dem Zustand** davor mit mehr als zwei Dritteln
//! traegt, alle Unterzeichner aus ihm und hoechstens
//! [`VERTRAUENSFRIST_EPOCHEN`] spaeter. Ein falsches braucht dann so viel
//! wie jedes Zertifikat dieses Satzes, und fuer eine zweite Unterschrift
//! haftet er bis zum Ende der Sperrfrist. ⛔️ Verworfen: die Schluessel
//! neuer Mitglieder von der Gegenstelle nehmen (ein Aggregat mit einem
//! Rogue-Key ginge durch, `myl_types::bls`), eine Vertrauensstufe von einem
//! Drittel (schwaecher als ein Zertifikat),
//! eine Bruecke vom Satz von aussen (die Startmitglieder haften nach dem
//! Wechsel fuer nichts). ⚠️ Faellt mehr als ein Drittel des Satzes vor der
//! Luecke fuer immer aus, oder unterschreiben in jedem Zertifikat danach
//! neue Mitglieder mit, traegt keine Bruecke; dann bleibt nur ein Anker von
//! aussen (schwache Subjektivitaet).
//!
//! ⚑ **Der Anker von aussen** (CONSENSUS 7.4a, 2026-11-07): Hoehe, Block und Bindung des
//! Zertifikats eines Belegs, vom Betreiber genannt und vertraut wie Genesis
//! ([`Folge::ab_beleg`]). Die Folge beginnt dann bei diesem Beleg; was davor
//! liegt, zaehlt nicht. Geprueft wird nur, dass der Beleg in sich stimmt
//! (Bindung, Abschluss); seine Unterschriften nicht, denn genau die kann nach
//! einer Ruhe mit Ausschluss niemand mehr pruefen. Volle Knoten halten die
//! Belege der letzten [`WECHSELBELEGE_BEHALTEN`] Epochen, also taugt ein Anker
//! die ganze Vertrauensfrist.
//!
//! ⚑ **Ein vertrauter Punkt altert** (Sicherheitsaudit A43, 2026-11-08): Wer
//! mehr als zwei Drittel eines alten Satzes in der Hand hat, dessen Einsatz
//! laengst frei ist, baut ab dort eine zweite Folge von Belegen, und ein
//! Einsteiger kann sie von der echten nicht unterscheiden. Ein vertrauter
//! Punkt, Genesis wie ein Anker von aussen, traegt deshalb nur, solange er
//! nach der Uhr juenger als die Vertrauensfrist ist
//! ([`einstieg_pruefen`]). Danach
//! nennt der Betreiber einen frischen Anker; Quellen sind volle Knoten (Zeile
//! `wechselbeleg`, Weg `/anker`), am besten mehrere unabhaengige.

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::block::{epoche_fuer_hoehe, BLOECKE_JE_EPOCHE};
use myl_consensus::pruefpunkt::{Pruefpunktzertifikat, Quelle};
use myl_consensus::validator::{VotingMember, VotingSet};
use myl_ledger::einsatz::{SPERRFRIST_EPOCHEN, VERTRAUENSFRIST_EPOCHEN};
use myl_ledger::stimmsatz::{bindung_aus, tragfaehig, Satz, VORLAUF_EPOCHEN};
use myl_types::hash::Hash;

/// ⚑ **Wie viele Epochen ein Knoten Wechselbelege haelt** (Sicherheitsaudit
/// A44): die Sperrfrist. Ein Anker taugt hoechstens die Vertrauensfrist, aeltere
/// Belege fragt also niemand mehr nach; bei 1 000 Mitgliedern waeren es sonst
/// rund 2 GB je Jahr, und alle lagen im Speicher.
pub const WECHSELBELEGE_BEHALTEN: u64 = SPERRFRIST_EPOCHEN;
const _: () = assert!(WECHSELBELEGE_BEHALTEN > VERTRAUENSFRIST_EPOCHEN);

/// **Der Anker von aussen**, wie ihn `--anker` nennt: Hoehe, Block und
/// Bindung des Zertifikats eines Wechselbelegs und der Slot seines Blocks.
///
/// Der Slot sagt, wie alt der Anker ist ([`einstieg_pruefen`]). Er ist so
/// vertraut wie der Rest der Zeile: Wer ihn faelscht, faelscht auch den Block.
/// Die Pruefung fing den ehrlichen, aber veralteten Anker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnkerVonAussen {
    pub hoehe: u64,
    pub block: Hash,
    pub bindung: Hash,
    pub slot: u64,
}

impl AnkerVonAussen {
    /// Liest `<hoehe>:<block>:<bindung>:<slot>`.
    pub fn aus_text(text: &str) -> Result<Self, String> {
        let felder: Vec<&str> = text.split(':').collect();
        let [h, b, g, s] = felder.as_slice() else {
            return Err("erwartet <hoehe>:<block>:<bindung>:<slot>".to_string());
        };
        Ok(Self {
            hoehe: h.parse().map_err(|_| "die Hoehe ist keine Zahl".to_string())?,
            block: Hash::from_hex(b).map_err(|_| "der Block ist kein Hash".to_string())?,
            bindung: Hash::from_hex(g).map_err(|_| "die Bindung ist kein Hash".to_string())?,
            slot: s.parse().map_err(|_| "der Slot ist keine Zahl".to_string())?,
        })
    }

    /// Die Form, die [`Self::aus_text`] liest.
    pub fn als_text(&self) -> String {
        format!("{}:{}:{}:{}", self.hoehe, self.block.to_hex(), self.bindung.to_hex(), self.slot)
    }
}

/// Warum ein Knoten ohne eigenen Stand nicht einsteigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Einstiegsfehler {
    /// Der Anker ist aelter als die Vertrauensfrist.
    AnkerZuAlt,
    /// Das Netz kann aelter als die Vertrauensfrist sein, und kein Anker ist genannt.
    NetzZuAlt,
    /// Die Stimmsatzdatei nennt keine Startzeit, und kein Anker ist genannt.
    OhneStartzeit,
}

impl std::fmt::Display for Einstiegsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let wo = "Einen frischen Anker nennt jeder volle Knoten (Weg /anker, Zeile wechselbeleg); am besten bei mehreren unabhaengigen vergleichen.";
        match self {
            Self::AnkerZuAlt => write!(f, "--anker ist aelter als die Vertrauensfrist und traegt nicht mehr. {wo}"),
            Self::NetzZuAlt => write!(f, "das Netz kann aelter als die Vertrauensfrist sein: Ein Einstieg ab Genesis ist nicht mehr sicher, --anker ist noetig. {wo}"),
            Self::OhneStartzeit => write!(f, "die Stimmsatzdatei nennt keine Startzeit (Zeile `start`): Ein Einstieg ab Genesis ist so nicht pruefbar, --anker ist noetig. {wo}"),
        }
    }
}

/// ⚑ **Traegt der Punkt, an dem ein Knoten ohne eigenen Stand beginnt?**
/// (Sicherheitsaudit A43.) Mit einem Anker dessen Slot, sonst der Beginn des
/// Netzes; beide muessen nach der Uhr juenger als die Vertrauensfrist sein.
///
/// Jeder Block braucht einen Slot, die Uhr ist also eine obere Schranke fuer
/// die Epochen seit dem Punkt, ohne jede Gegenstelle. Ausgefallene Slots
/// ueberzeichnen das Alter: im Zweifel frueher ablehnen, nie spaeter. Fuer
/// einen Anker misst deshalb sein eigener Slot und nicht die Hoehe ab Genesis,
/// sonst summierten sich die Ausfaelle der ganzen Kette auf.
pub fn einstieg_pruefen(start_ms: Option<u64>, anker: Option<&AnkerVonAussen>, jetzt_ms: u64, slot_ms: u64) -> Result<(), Einstiegsfehler> {
    let frist_ms = VERTRAUENSFRIST_EPOCHEN.saturating_mul(BLOECKE_JE_EPOCHE).saturating_mul(slot_ms);
    match (anker, start_ms) {
        (Some(a), _) if jetzt_ms < a.slot.saturating_mul(slot_ms).saturating_add(frist_ms) => Ok(()),
        (Some(_), _) => Err(Einstiegsfehler::AnkerZuAlt),
        (None, Some(s)) if jetzt_ms < s.saturating_add(frist_ms) => Ok(()),
        (None, Some(_)) => Err(Einstiegsfehler::NetzZuAlt),
        (None, None) => Err(Einstiegsfehler::OhneStartzeit),
    }
}

/// Ein Wechselbeleg: das Zertifikat und die Saetze, die es bindet.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Wechselbeleg {
    /// Ein Pruefpunktzertifikat der Epoche.
    pub zertifikat: Pruefpunktzertifikat,
    /// Der Satz fuer die naechste Epoche, wenn der Zustand einen fuehrt.
    pub naechster: Option<Satz>,
    /// Der Satz fuer die uebernaechste.
    pub uebernaechster: Option<Satz>,
    /// ⚑ Das Zertifikat des naechsten Pruefpunkts mit diesem als Quelle.
    pub abschluss: Pruefpunktzertifikat,
}

impl Wechselbeleg {
    /// Die Epoche des Zertifikats.
    pub fn epoche(&self) -> u64 {
        epoche_fuer_hoehe(self.zertifikat.hoehe)
    }
}

/// Warum eine Folge nicht weitergeht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folgefehler {
    /// Fuer die Epoche eines Belegs ist kein Satz bekannt (eine Luecke).
    Unbekannt(u64),
    /// Der Satz der Epoche traegt nicht (nicht tragfaehig).
    Ruht(u64),
    /// Das Zertifikat traegt nicht gegen den Satz seiner Epoche.
    TraegtNicht(u64),
    /// Die Saetze passen nicht zur Bindung des Zertifikats.
    Bindung(u64),
    /// Zwei Belege binden fuer dieselbe Epoche verschiedene Saetze.
    Widerspruch(u64),
    /// Das Zertifikat macht keinen Pruefpunkt endgueltig (keines des
    /// direkten Nachfolgers, oder der Abschluss nennt eine andere Quelle).
    Offen(u64),
}

/// **Was ein Knoten ueber die Saetze weiss**, aus Genesis und den
/// geprueften Belegen.
#[derive(Debug, Clone)]
pub struct Folge {
    von_aussen: VotingSet,
    mindestgewicht: u64,
    /// Je Epoche der gebundene Satz, oder dass keiner gefuehrt wird.
    bekannt: BTreeMap<u64, Option<Satz>>,
    /// Die geprueften Belege, je Epoche einer.
    belege: BTreeMap<u64, Wechselbeleg>,
    /// ⚑ Die beiden Zertifikate des Ankers von aussen ([`Self::ab_beleg`]):
    /// vertraut wie er, denn den Satz ihrer Epoche bindet kein Beleg. Das
    /// Zertifikat ist mit Hoehe, Block und Bindung festgelegt
    /// (`Kette::anker_von_aussen_setzen`); der Abschluss ist es nicht, aber
    /// er nennt nur den Anker selbst als Quelle, und was er sonst traegt,
    /// geht in keine Folge ein.
    vertraut: Vec<Pruefpunktzertifikat>,
}

impl Folge {
    /// **Ab Genesis:** Bis `VORLAUF_EPOCHEN` fuehrt kein Zustand einen Satz
    /// (der erste entsteht am Ende von Epoche 0 fuer Epoche 2 + 1 frühestens),
    /// dort traegt der Satz von aussen.
    pub fn ab_genesis(von_aussen: VotingSet, mindestgewicht: u64) -> Self {
        let bekannt = (0..=VORLAUF_EPOCHEN).map(|e| (e, None)).collect();
        Self { von_aussen, mindestgewicht, bekannt, belege: BTreeMap::new(), vertraut: Vec::new() }
    }

    /// ⚑ **Ab einem Anker von aussen** (siehe Modulkopf): bekannt sind die
    /// Saetze, die der Beleg bindet. Er muss in sich stimmen: die Saetze zur
    /// Bindung seines Zertifikats, der Abschluss zu diesem Zertifikat.
    pub fn ab_beleg(von_aussen: VotingSet, mindestgewicht: u64, beleg: &Wechselbeleg) -> Result<Self, Folgefehler> {
        let e = beleg.epoche();
        let z = &beleg.zertifikat;
        if bindung_aus(beleg.naechster.as_ref(), beleg.uebernaechster.as_ref()) != z.bindung {
            return Err(Folgefehler::Bindung(e));
        }
        let a = &beleg.abschluss;
        if !a.schliesst_ab() || a.quelle != (Quelle { hoehe: z.hoehe, block_hash: z.block_hash }) {
            return Err(Folgefehler::Offen(e));
        }
        let bekannt = [(e + 1, beleg.naechster.clone()), (e + 2, beleg.uebernaechster.clone())].into_iter().collect();
        let belege = [(e, beleg.clone())].into_iter().collect();
        Ok(Self { von_aussen, mindestgewicht, bekannt, belege, vertraut: vec![z.clone(), a.clone()] })
    }

    /// **Der Satz, der die Finalitaet von `epoche` traegt**, soweit
    /// bekannt; dieselbe Regel wie in der Kette.
    pub fn satz_fuer(&self, epoche: u64) -> Result<VotingSet, Folgefehler> {
        match self.bekannt.get(&epoche) {
            None => Err(Folgefehler::Unbekannt(epoche)),
            Some(Some(satz)) if tragfaehig(satz, self.mindestgewicht) => Ok(als_stimmsatz(satz)),
            Some(Some(_)) => Err(Folgefehler::Ruht(epoche)),
            Some(None) if self.bekannt.range(..=epoche).all(|(_, s)| s.is_none()) => Ok(self.von_aussen.clone()),
            Some(None) => Err(Folgefehler::Ruht(epoche)),
        }
    }

    /// Bis zu welcher Epoche ein Satz bekannt ist.
    pub fn bekannt_bis(&self) -> u64 {
        self.bekannt.keys().next_back().copied().unwrap_or(0)
    }

    /// Die geprueften Belege ab `ab`, in Epochen aufsteigend.
    pub fn belege_ab(&self, ab: u64) -> impl Iterator<Item = &Wechselbeleg> {
        self.belege.range(ab..).map(|(_, b)| b)
    }

    /// **Einen Beleg pruefen und aufnehmen.** Ein schon bekannter Beleg
    /// derselben Epoche aendert nichts, solange er dasselbe bindet.
    pub fn aufnehmen(&mut self, beleg: &Wechselbeleg) -> Result<(), Folgefehler> {
        let e = beleg.epoche();
        let z = &beleg.zertifikat;
        self.zertifikat_traegt(z)?;
        if bindung_aus(beleg.naechster.as_ref(), beleg.uebernaechster.as_ref()) != z.bindung {
            return Err(Folgefehler::Bindung(e));
        }
        let a = &beleg.abschluss;
        if !a.schliesst_ab() || a.quelle != (Quelle { hoehe: z.hoehe, block_hash: z.block_hash }) {
            return Err(Folgefehler::Offen(e));
        }
        let gebunden = [(e + 1, &beleg.naechster), (e + 2, &beleg.uebernaechster)];
        // Erst beide pruefen, dann eintragen: Ein Widerspruch laesst nichts
        // zurueck.
        if let Some((k, _)) = gebunden.iter().find(|(k, s)| self.bekannt.get(k).is_some_and(|alt| alt != *s)) {
            return Err(Folgefehler::Widerspruch(*k));
        }
        let neu: Vec<u64> = gebunden.iter().map(|(k, _)| *k).filter(|k| !self.bekannt.contains_key(k)).collect();
        for (k, s) in gebunden {
            self.bekannt.entry(k).or_insert_with(|| s.clone());
        }
        // Der Abschluss kann in der naechsten Epoche liegen; ihr Satz ist
        // erst mit diesem Beleg bekannt. Traegt er nicht, wird zurueckgenommen.
        if let Err(f) = self.zertifikat_traegt(a) {
            for k in neu {
                self.bekannt.remove(&k);
            }
            return Err(f);
        }
        self.belege.entry(e).or_insert_with(|| beleg.clone());
        Ok(())
    }

    /// Belege in Epochen aufsteigend aufnehmen, bis einer nicht traegt.
    pub fn folgen(&mut self, belege: &[Wechselbeleg]) -> Result<(), Folgefehler> {
        let mut sortiert: Vec<&Wechselbeleg> = belege.iter().collect();
        sortiert.sort_by_key(|b| b.zertifikat.hoehe);
        for b in sortiert {
            self.aufnehmen(b)?;
        }
        Ok(())
    }

    /// **Traegt dieses Zertifikat?** Gegen den Satz seiner Epoche, und ist
    /// sie unbekannt, ueber die [`Self::bruecke`]. Traegt es dort nicht,
    /// bleibt die Epoche unbekannt: Es koennen auch nur Belege fehlen.
    ///
    /// ⚑ Die beiden Zertifikate des Ankers von aussen tragen ohne Satz.
    /// 📌 Im Podlauf `kurz-anker` (2026-11-07) bot jede Gegenstelle gleich
    /// nach der Ruhe nur Zertifikate aus der Epoche des Ankers an, darunter
    /// genau seinen Abschluss; ohne diese Regel lehnte der Knoten alle ab
    /// (`Unbekannt`) und kam erst ab Genesis wieder hinein.
    pub fn zertifikat_traegt(&self, z: &Pruefpunktzertifikat) -> Result<(), Folgefehler> {
        if self.vertraut.contains(z) {
            return Ok(());
        }
        let e = epoche_fuer_hoehe(z.hoehe);
        match self.satz_fuer(e) {
            Ok(satz) => z.verify(&satz).map_err(|_| Folgefehler::TraegtNicht(e)),
            Err(Folgefehler::Unbekannt(_)) => match self.bruecke(e) {
                Some(satz) if z.verify(&satz).is_ok() => Ok(()),
                _ => Err(Folgefehler::Unbekannt(e)),
            },
            Err(f) => Err(f),
        }
    }

    /// ⚑ **Der Satz ueber eine Luecke** (siehe Modulkopf): der juengste
    /// bekannte vor `epoche`, nur einer aus dem Zustand, nur tragfaehig, nur
    /// innerhalb der Vertrauensfrist. Die Unterzeichner muessen alle aus ihm
    /// sein, sonst traegt das Aggregat nicht.
    fn bruecke(&self, epoche: u64) -> Option<VotingSet> {
        let (k, satz) = self.bekannt.range(..epoche).next_back()?;
        let satz = satz.as_ref()?;
        (epoche - k <= VERTRAUENSFRIST_EPOCHEN && tragfaehig(satz, self.mindestgewicht)).then(|| als_stimmsatz(satz))
    }

    /// ⚑ **Der endgueltige Pruefpunkt, den dieses Zertifikat belegt**
    /// (2026-11-04): Es traegt, und es nennt als Quelle den Pruefpunkt
    /// direkt vor sich. Der ist dann endgueltig, nicht das Zertifikat selbst.
    pub fn endgueltig_durch(&self, z: &Pruefpunktzertifikat) -> Result<Quelle, Folgefehler> {
        self.zertifikat_traegt(z)?;
        if !z.schliesst_ab() || z.quelle == Quelle::GENESIS {
            return Err(Folgefehler::Offen(epoche_fuer_hoehe(z.hoehe)));
        }
        Ok(z.quelle)
    }
}

/// Ein Satz aus dem Zustand als Stimmsatz.
pub fn als_stimmsatz(satz: &Satz) -> VotingSet {
    VotingSet::from_members(satz.iter().map(|(m, k, w)| (*m, VotingMember { pubkey: *k, weight: *w })).collect())
}

/// **Die Ablage der Belege** neben der Kettendatei: je Beleg Laenge (u32,
/// little endian) und Borsh, nur angehaengt. Gelesen wird bis zum ersten
/// unvollstaendigen Eintrag (ein Absturz beim Schreiben verliert nur ihn).
#[derive(Debug, Clone)]
pub struct Belegablage {
    pfad: std::path::PathBuf,
}

impl Belegablage {
    /// Im Ordner `ordner`, Datei `wechselbelege.dat`.
    pub fn in_ordner(ordner: &std::path::Path) -> Self {
        Self { pfad: ordner.join("wechselbelege.dat") }
    }

    /// Haengt einen Beleg an.
    pub fn anhaengen(&self, beleg: &Wechselbeleg) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(o) = self.pfad.parent() {
            std::fs::create_dir_all(o)?;
        }
        let bytes = borsh::to_vec(beleg)?;
        let laenge = u32::try_from(bytes.len()).map_err(|_| std::io::Error::other("Beleg zu gross"))?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.pfad)?;
        f.write_all(&laenge.to_le_bytes())?;
        f.write_all(&bytes)?;
        Ok(())
    }

    /// ⚑ **Schreibt die Ablage neu**, nur mit diesen Belegen (A44). Erst
    /// vollstaendig daneben, dann umbenannt: Ein Absturz laesst die alte
    /// Ablage stehen. Schreiben und Erzwingen ueber denselben Griff (unter
    /// Windows verlangt das Erzwingen Schreibrecht).
    pub fn neu_schreiben<'a>(&self, belege: impl Iterator<Item = &'a Wechselbeleg>) -> std::io::Result<()> {
        use std::io::Write;
        let mut neu = self.pfad.as_os_str().to_owned();
        neu.push(".neu");
        let neu = std::path::PathBuf::from(neu);
        let mut f = std::fs::File::create(&neu)?;
        for beleg in belege {
            let bytes = borsh::to_vec(beleg)?;
            let laenge = u32::try_from(bytes.len()).map_err(|_| std::io::Error::other("Beleg zu gross"))?;
            f.write_all(&laenge.to_le_bytes())?;
            f.write_all(&bytes)?;
        }
        f.sync_all()?;
        drop(f);
        std::fs::rename(&neu, &self.pfad)
    }

    /// Alle lesbaren Belege, in der Reihenfolge der Ablage.
    pub fn lesen(&self) -> Vec<Wechselbeleg> {
        let Ok(daten) = std::fs::read(&self.pfad) else {
            return Vec::new();
        };
        let mut aus = Vec::new();
        let mut i = 0usize;
        while i + 4 <= daten.len() {
            let laenge = u32::from_le_bytes([daten[i], daten[i + 1], daten[i + 2], daten[i + 3]]) as usize;
            let Some(stueck) = daten.get(i + 4..i + 4 + laenge) else {
                break;
            };
            match borsh::from_slice::<Wechselbeleg>(stueck) {
                Ok(b) => aus.push(b),
                Err(_) => break,
            }
            i += 4 + laenge;
        }
        aus
    }
}

/// Saetze und Belege fuer Tests, auch fuer die der Kette.
#[cfg(test)]
pub(crate) mod probe {
    use super::*;
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_consensus::pruefpunkt::Pruefpunktstimme;
    use myl_types::bls::BlsSecretKey;
    use myl_types::hash::Hash;
    use myl_types::ids::MinerId;

    fn sk(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b; 32]).expect("Schluessel")
    }

    fn id(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Vier Mitglieder ab `von`, je mit dem Gewicht `gewicht`.
    pub(crate) fn satz_mit(von: u8, gewicht: u64) -> Satz {
        (von..von + 4).map(|b| (id(b), sk(b).public_key().expect("pk"), gewicht)).collect()
    }

    /// Ein Zertifikat an `hoehe` mit der Quelle `quelle`, unterschrieben von
    /// den Mitgliedern `wer`.
    pub(crate) fn zertifikat(hoehe: u64, quelle: Quelle, bindung: Hash, wer: &[u8]) -> Pruefpunktzertifikat {
        let block = Hash::sha256(&hoehe.to_le_bytes());
        let stimmen: Vec<_> = wer
            .iter()
            .map(|b| Pruefpunktstimme::unterschreiben(hoehe, block, bindung, quelle, id(*b), &sk(*b)).expect("Stimme"))
            .collect();
        Pruefpunktzertifikat::aus_stimmen(&stimmen).expect("Zertifikat")
    }

    /// Ein Beleg der Epoche `e`, unterschrieben von den Mitgliedern `wer`,
    /// samt dem Abschluss durch dieselben am naechsten Pruefpunkt.
    pub(crate) fn beleg(e: u64, wer: &[u8], naechster: Option<Satz>, uebernaechster: Option<Satz>) -> Wechselbeleg {
        let hoehe = e * BLOECKE_JE_EPOCHE + 150;
        let bindung = bindung_aus(naechster.as_ref(), uebernaechster.as_ref());
        let z = zertifikat(hoehe, Quelle::GENESIS, bindung, wer);
        let abschluss = zertifikat(hoehe + 150, Quelle { hoehe, block_hash: z.block_hash }, Hash::sha256(b"spaeter"), wer);
        Wechselbeleg { zertifikat: z, naechster, uebernaechster, abschluss }
    }
}

#[cfg(test)]
mod tests {
    use super::probe::{beleg, satz_mit, zertifikat};
    use super::*;
    use myl_types::hash::Hash;

    fn satz(von: u8) -> Satz {
        satz_mit(von, 10)
    }

    fn aussen() -> VotingSet {
        als_stimmsatz(&satz(100))
    }

    /// ⚑ Von Genesis ueber den Satz von aussen in die Saetze aus dem
    /// Zustand, ein Beleg je Epoche, und am Ende ein Zertifikat, das nur der
    /// neue Satz tragen kann.
    #[test]
    fn von_genesis_bis_zum_juengsten_satz() {
        let mut f = Folge::ab_genesis(aussen(), 40);
        let a = satz(1);
        let b = satz(11);
        let belege = vec![
            beleg(1, &[100, 101, 102], None, Some(a.clone())),
            beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone())),
            beleg(3, &[1, 2, 3], Some(a.clone()), Some(b.clone())),
            beleg(4, &[1, 2, 4], Some(b.clone()), Some(b.clone())),
        ];
        f.folgen(&belege).expect("die Folge traegt");
        assert_eq!(f.bekannt_bis(), 6);
        let z = beleg(5, &[11, 12, 13], None, None).zertifikat;
        assert_eq!(f.zertifikat_traegt(&z), Ok(()));
        // ⚑ Endgueltig macht erst der Abschluss, und zwar seine Quelle.
        let b5 = beleg(5, &[11, 12, 13], None, None);
        assert_eq!(f.endgueltig_durch(&b5.abschluss), Ok(Quelle { hoehe: b5.zertifikat.hoehe, block_hash: b5.zertifikat.block_hash }));
        assert_eq!(f.endgueltig_durch(&z), Err(Folgefehler::Offen(5)), "von Genesis aus schliesst nichts ab");
        // ⛔️ Der Satz von aussen spricht nach dem Wechsel nicht mehr.
        let alt = beleg(5, &[100, 101, 102], None, None).zertifikat;
        assert_eq!(f.zertifikat_traegt(&alt), Err(Folgefehler::TraegtNicht(5)));
        assert_eq!(f.belege_ab(3).count(), 2);
    }

    /// ⛔️ Gegenproben: falsche Bindung, Luecke, Widerspruch, ein nicht
    /// tragfaehiger Satz, zu wenig Gewicht.
    #[test]
    fn falsche_folgen_brechen_ab() {
        let a = satz(1);
        let mut f = Folge::ab_genesis(aussen(), 40);
        let mut untergeschoben = beleg(1, &[100, 101, 102], None, Some(a.clone()));
        untergeschoben.uebernaechster = Some(satz(50));
        assert_eq!(f.aufnehmen(&untergeschoben), Err(Folgefehler::Bindung(1)));
        assert_eq!(f.aufnehmen(&beleg(5, &[100, 101, 102], None, None)), Err(Folgefehler::Unbekannt(5)));
        assert_eq!(f.aufnehmen(&beleg(1, &[100, 101], None, Some(a.clone()))), Err(Folgefehler::TraegtNicht(1)), "zwei von vier");
        f.aufnehmen(&beleg(1, &[100, 101, 102], None, Some(a.clone()))).expect("traegt");
        assert_eq!(
            f.aufnehmen(&beleg(2, &[100, 101, 102], Some(satz(50)), None)),
            Err(Folgefehler::Widerspruch(3)),
            "Epoche 3 ist schon gebunden"
        );
        // ⛔️ Ohne passenden Abschluss kein Beleg.
        let mut offen = beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone()));
        offen.abschluss = zertifikat(offen.zertifikat.hoehe + 300, offen.abschluss.quelle, Hash::sha256(b"x"), &[100, 101, 102]);
        assert_eq!(f.aufnehmen(&offen), Err(Folgefehler::Offen(2)), "kein direkter Nachfolger");
        let mut fremd = beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone()));
        fremd.abschluss.quelle.block_hash = Hash::sha256(b"anderer Zweig");
        assert_eq!(f.aufnehmen(&fremd), Err(Folgefehler::Offen(2)), "Abschluss eines anderen Blocks");
        let mut schwach = beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone()));
        schwach.abschluss = beleg(2, &[100, 101], Some(a.clone()), Some(a.clone())).abschluss;
        assert_eq!(f.aufnehmen(&schwach), Err(Folgefehler::TraegtNicht(2)), "Abschluss mit zwei von vier");
        let drei: Satz = a[..3].to_vec();
        f.aufnehmen(&beleg(2, &[100, 101, 102], Some(a.clone()), Some(drei))).expect("traegt");
        assert_eq!(f.satz_fuer(4).err(), Some(Folgefehler::Ruht(4)), "drei Mitglieder tragen nicht");
    }

    /// ⚑ **Die Bruecke ueber eine Luecke** (2026-11-07): Nach Epochen ohne
    /// Beleg traegt ein Zertifikat, das mehr als zwei Drittel des juengsten
    /// bekannten Satzes tragen, und von dort geht die Folge weiter.
    /// ⛔️ Gegenproben: zu wenig Gewicht, ein Unterzeichner von ausserhalb,
    /// jenseits der Vertrauensfrist, vom Satz von aussen; und ein Beleg der
    /// Bruecke, der einen gebundenen Satz anders bindet.
    #[test]
    fn eine_luecke_wird_ueberbrueckt() {
        let a = satz(1);
        let b = satz(11);
        let mut f = Folge::ab_genesis(aussen(), 40);
        f.folgen(&[
            beleg(1, &[100, 101, 102], None, Some(a.clone())),
            beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone())),
            beleg(3, &[1, 2, 3], Some(a.clone()), Some(a.clone())),
        ])
        .expect("die Folge traegt");
        assert_eq!(f.bekannt_bis(), 5);
        // Die Epochen 4 bis 8 ohne Beleg, die Finalitaet ruhte.
        assert_eq!(f.aufnehmen(&beleg(9, &[1, 2], Some(b.clone()), Some(b.clone()))), Err(Folgefehler::Unbekannt(9)), "zwei von vier");
        let mut fremd = beleg(9, &[1, 2, 3, 11], Some(b.clone()), Some(b.clone()));
        assert_eq!(f.aufnehmen(&fremd), Err(Folgefehler::Unbekannt(9)), "ein Unterzeichner ausserhalb des Satzes");
        fremd.zertifikat = beleg(9, &[1, 2, 3], Some(b.clone()), Some(b.clone())).zertifikat;
        assert_eq!(f.aufnehmen(&fremd), Err(Folgefehler::Unbekannt(9)), "der Abschluss mit einem Unterzeichner ausserhalb");
        assert_eq!(f.bekannt_bis(), 5, "was der Beleg band, ist zurueckgenommen");
        let fern = 5 + VERTRAUENSFRIST_EPOCHEN + 1;
        assert_eq!(f.aufnehmen(&beleg(fern, &[1, 2, 3], None, None)), Err(Folgefehler::Unbekannt(fern)), "jenseits der Vertrauensfrist");
        f.aufnehmen(&beleg(9, &[1, 2, 3], Some(b.clone()), Some(b.clone()))).expect("die Bruecke traegt");
        assert_eq!(f.bekannt_bis(), 11);
        f.aufnehmen(&beleg(10, &[11, 12, 13], Some(b.clone()), Some(b.clone()))).expect("danach geht es gewoehnlich weiter");
        let z = beleg(12, &[11, 12, 13], None, None).zertifikat;
        assert_eq!(f.zertifikat_traegt(&z), Ok(()));
        assert_eq!(f.zertifikat_traegt(&beleg(12, &[1, 2, 3], None, None).zertifikat), Err(Folgefehler::TraegtNicht(12)), "der alte Satz spricht nicht mehr");
        // ⛔️ Auch ueber eine Bruecke bindet keiner anders, als schon gebunden ist.
        assert_eq!(f.aufnehmen(&beleg(8, &[1, 2, 3], None, Some(satz(50)))), Err(Folgefehler::Widerspruch(10)));
        // ⛔️ Vom Satz von aussen keine Bruecke.
        let g = Folge::ab_genesis(aussen(), 40);
        assert_eq!(g.zertifikat_traegt(&beleg(5, &[100, 101, 102], None, None).zertifikat), Err(Folgefehler::Unbekannt(5)));
    }

    /// ⚑ **Der vertraute Punkt altert** (A43), auf die Millisekunde.
    /// Gegenprobe: `<` statt `<=` fiele an genau diesen Raendern auf.
    #[test]
    fn genesis_und_anker_tragen_nur_die_vertrauensfrist() {
        let slot = 2_000;
        let frist = VERTRAUENSFRIST_EPOCHEN * BLOECKE_JE_EPOCHE * slot;
        let start = 1_000_000;
        assert_eq!(frist, 112 * 3_600_000, "bei 2 s je Slot sind es 112 Stunden");
        assert_eq!(einstieg_pruefen(Some(start), None, start + frist - 1, slot), Ok(()));
        assert_eq!(einstieg_pruefen(Some(start), None, start + frist, slot), Err(Einstiegsfehler::NetzZuAlt));
        assert_eq!(einstieg_pruefen(None, None, 0, slot), Err(Einstiegsfehler::OhneStartzeit));
        let a = AnkerVonAussen { hoehe: 150, block: Hash([1; 32]), bindung: Hash([2; 32]), slot: 5_000 };
        let beim_anker = 5_000 * slot;
        assert_eq!(einstieg_pruefen(Some(0), Some(&a), beim_anker + frist - 1, slot), Ok(()), "ein altes Netz, ein frischer Anker");
        assert_eq!(einstieg_pruefen(None, Some(&a), beim_anker + frist - 1, slot), Ok(()), "der Anker braucht keine Startzeit");
        assert_eq!(einstieg_pruefen(Some(beim_anker), Some(&a), beim_anker + frist, slot), Err(Einstiegsfehler::AnkerZuAlt), "ein junges Netz rettet keinen alten Anker");
    }

    #[test]
    fn der_anker_liest_sich_wie_er_sich_schreibt() {
        let a = AnkerVonAussen { hoehe: 14_400, block: Hash([7; 32]), bindung: Hash([8; 32]), slot: 897_000_123 };
        assert_eq!(AnkerVonAussen::aus_text(&a.als_text()), Ok(a));
        assert!(AnkerVonAussen::aus_text("14400:aa:bb").is_err(), "ohne Slot nicht");
        assert!(AnkerVonAussen::aus_text(&format!("x:{}:{}:1", a.block.to_hex(), a.bindung.to_hex())).is_err());
    }

    /// ⚑ **Ab einem Anker von aussen** (7.4a, 2026-11-07): Die Folge
    /// beginnt bei dem Beleg, dessen Zertifikat niemand pruefen kann (sein
    /// Satz ist unbekannt), und geht von dort gewoehnlich weiter.
    /// ⛔️ Gegenproben: ein Beleg, der in sich nicht stimmt (Bindung,
    /// Abschluss), traegt auch als Anker nicht; vom Anker aus spricht der
    /// alte Satz nicht.
    #[test]
    fn ab_einem_anker_von_aussen() {
        let b = satz(11);
        let anker = beleg(9, &[11, 12, 13], Some(b.clone()), Some(b.clone()));
        let mut falsch = anker.clone();
        falsch.uebernaechster = Some(satz(50));
        assert_eq!(Folge::ab_beleg(aussen(), 40, &falsch).err(), Some(Folgefehler::Bindung(9)));
        let mut offen = anker.clone();
        offen.abschluss.quelle.block_hash = Hash::sha256(b"anderer Zweig");
        assert_eq!(Folge::ab_beleg(aussen(), 40, &offen).err(), Some(Folgefehler::Offen(9)));
        let mut f = Folge::ab_beleg(aussen(), 40, &anker).expect("in sich stimmig");
        assert_eq!(f.bekannt_bis(), 11);
        f.aufnehmen(&beleg(10, &[11, 12, 14], Some(b.clone()), Some(b.clone()))).expect("danach gewoehnlich");
        assert_eq!(f.zertifikat_traegt(&beleg(12, &[11, 12, 13], None, None).zertifikat), Ok(()));
        assert_eq!(f.zertifikat_traegt(&beleg(12, &[1, 2, 3], None, None).zertifikat), Err(Folgefehler::TraegtNicht(12)));
        assert_eq!(f.belege_ab(0).count(), 2, "der Anker und der Beleg danach");
        // ⚑ Sein Abschluss macht den Anker endgueltig, obwohl den Satz seiner
        //   Epoche niemand kennt; ein anderes Zertifikat dieser Epoche nicht.
        let quelle = Quelle { hoehe: anker.zertifikat.hoehe, block_hash: anker.zertifikat.block_hash };
        assert_eq!(f.endgueltig_durch(&anker.abschluss), Ok(quelle));
        let anderes = zertifikat(anker.abschluss.hoehe + 150, Quelle { hoehe: anker.abschluss.hoehe, block_hash: anker.abschluss.block_hash }, Hash::sha256(b"spaeter"), &[11, 12, 13]);
        assert_eq!(f.zertifikat_traegt(&anderes), Err(Folgefehler::Unbekannt(9)));
    }

    #[test]
    fn die_ablage_liest_bis_zum_ersten_bruch() {
        let ordner = std::env::temp_dir().join(format!("myl-wechsel-{}", std::process::id()));
        let a = Belegablage::in_ordner(&ordner);
        let b1 = beleg(1, &[100, 101, 102], None, Some(satz(1)));
        let b2 = beleg(2, &[100, 101, 102], Some(satz(1)), Some(satz(1)));
        a.anhaengen(&b1).expect("schreiben");
        a.anhaengen(&b2).expect("schreiben");
        assert_eq!(a.lesen(), vec![b1.clone(), b2]);
        // Ein abgebrochener Eintrag am Ende kostet nur ihn.
        let mut daten = std::fs::read(ordner.join("wechselbelege.dat")).expect("lesen");
        daten.truncate(daten.len() - 10);
        std::fs::write(ordner.join("wechselbelege.dat"), daten).expect("schreiben");
        assert_eq!(a.lesen(), vec![b1]);
        std::fs::remove_dir_all(&ordner).ok();
    }
}
