//! **Wechselbelege** (CONSENSUS 7.3 und 7.4, 2026-11-04): wie ein Knoten
//! ohne Zustand dem Stimmsatz von Genesis an folgt.
//!
//! ⚑ **Ein Beleg je Epoche:** ein Pruefpunktzertifikat der Epoche `e`,
//! unterschrieben vom Satz, der dort die Finalitaet traegt, die beiden
//! Saetze fuer `e + 1` und `e + 2`, an die seine Bindung haengt
//! (`myl_ledger::stimmsatz::bindung`), und ⚑ seit 2026-11-04 das Zertifikat
//! des naechsten Pruefpunkts, das ihn als Quelle nennt und damit endgueltig
//! macht ([`Wechselbeleg::abschluss`], Casper FFG). Ein nur
//! gerechtfertigter Pruefpunkt kann auf einem Zweig liegen, den die Kette
//! verlaesst; seine Bindung bewiese dann nichts. Wer den Satz von `e` kennt, kennt nach
//! dem Beleg die beiden naechsten. Vorbild ist der Warp-Sync von GRANDPA: je
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
//! Drittel wie im Lichtclient von CometBFT (schwaecher als ein Zertifikat),
//! eine Bruecke vom Satz von aussen (die Startmitglieder haften nach dem
//! Wechsel fuer nichts). ⚠️ Faellt mehr als ein Drittel des Satzes vor der
//! Luecke fuer immer aus, oder unterschreiben in jedem Zertifikat danach
//! neue Mitglieder mit, traegt keine Bruecke; dann bleibt nur ein Anker von
//! aussen (schwache Subjektivitaet).

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::block::epoche_fuer_hoehe;
use myl_consensus::pruefpunkt::{Pruefpunktzertifikat, Quelle};
use myl_consensus::validator::{VotingMember, VotingSet};
use myl_ledger::einsatz::VERTRAUENSFRIST_EPOCHEN;
use myl_ledger::stimmsatz::{bindung_aus, tragfaehig, Satz, VORLAUF_EPOCHEN};

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
}

impl Folge {
    /// **Ab Genesis:** Bis `VORLAUF_EPOCHEN` fuehrt kein Zustand einen Satz
    /// (der erste entsteht am Ende von Epoche 0 fuer Epoche 2 + 1 frühestens),
    /// dort traegt der Satz von aussen.
    pub fn ab_genesis(von_aussen: VotingSet, mindestgewicht: u64) -> Self {
        let bekannt = (0..=VORLAUF_EPOCHEN).map(|e| (e, None)).collect();
        Self { von_aussen, mindestgewicht, bekannt, belege: BTreeMap::new() }
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
    pub fn zertifikat_traegt(&self, z: &Pruefpunktzertifikat) -> Result<(), Folgefehler> {
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
