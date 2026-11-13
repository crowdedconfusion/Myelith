//! Das Gedaechtnis: Whitepaper Kap. 8.4, letzter Absatz.
//!
//! Kap. 8.4 verlangt vom dauerhaften Speicher einer Session dasselbe wie
//! Kap. 7.3 von Trainingsdaten: **Was hineingeht, muss auf ein
//! verifiziertes Segment zurueckfuehrbar sein.** Geprueft wird die
//! Herkunft und nicht der Inhalt, denn ein Urteil ueber Inhalte waere
//! genau der Ermessensspielraum, den das Protokoll ueberall sonst meidet.
//!
//! ## ⚑ Was hier steht: die Herkunft, nicht der Abruf
//!
//! Ein Gedaechtnis hat zwei Haelften, und nur eine davon ist ein Format.
//!
//! - **Die Herkunft**: Ein Eintrag ist die Ausgabe eines Schritts einer
//!   Segmentkette. Er nennt Session, Plan, Stelle und Glied, und gegen
//!   die Kette laesst sich nachrechnen, ob das stimmt. Das steht hier.
//! - **Der Abruf**: Welche Eintraege zu einer Frage zurueckkommen. Er
//!   muss deterministisch sein, sonst melden zwei ehrliche Pods einander
//!   als fehlerhaft. Er steht in [`crate::abruf`] (lexikalisch, in ganzen
//!   Zahlen) und nicht hier.
//!
//! ## Ein Anker, nicht die Daten
//!
//! Wie beim verankerten Korpus traegt der Speicher **Commitments** und
//! keine Inhalte: Ein Eintrag haelt das Ausgabe-Commitment seines
//! Schritts, der Text dazu liegt anderswo. Die Wurzel ueber alle
//! Eintraege ist, was sich verankern laesst, und ein Abruf weist ueber
//! einen Merkle-Beweis nach, dass sein Eintrag dazugehoert.
//!
//! ⚑ **Zwei Pruefungen, und keine ersetzt die andere.**
//! [`pruefe_abruf`] sagt: Der Eintrag steht an dieser Stelle im Speicher
//! mit dieser Wurzel. [`pruefe_herkunft`] sagt: Der Eintrag ist die
//! Ausgabe dieses Schritts dieser Kette. Wer nur die erste faehrt,
//! glaubt dem Betreiber des Speichers, dass er nur Belegtes aufgenommen
//! hat; wer nur die zweite faehrt, weiss nicht, ob der Eintrag je im
//! Speicher stand.
//!
//! ## ⚑ Was „verifiziert" hier heisst, und was nicht
//!
//! Diese Datei rechnet nach, dass ein Eintrag zu einer Kette **gehoert**.
//! Ob die Segmente dieser Kette verifiziert sind, ist die Aussage von
//! VERIFICATION, und der Kettenwert, gegen den geprueft wird, kommt vom
//! Konsens. Der Eintrag nennt das Segment, damit sich dort nachsehen
//! laesst; eine zweite Verifikation daneben waere eine zweite Quelle
//! fuer dieselbe Aussage.
//!
//! ## ⚑ Truebung ueberlebt den Speicher
//!
//! Ein Eintrag aus einem getruebten Schritt ist auch nach Wochen noch
//! getruebt. **Sie steht deshalb im Eintrag, und die Aufnahme prueft
//! sie** ([`pruefe_herkunft`]): Der Wert muss der sein, den der Plan des
//! Eintrags an dieser Stelle ergibt ([`truebung`]). Ein getruebter
//! Eintrag, der sich als rein ausgibt, kommt nicht hinein, und die
//! Wurzel bindet das Feld wie jedes andere.
//!
//! 📌 **Hier stand zuerst nur die Ableitung**, mit dem Argument, ein Feld
//! koenne beim Schreiben verschwinden. Das Feld verschwindet nicht, weil
//! die Aufnahme es prueft; die Ableitung allein liess dagegen eine Luecke
//! offen: Ein Abruf in einem spaeteren Plan hat den Plan des Eintrags
//! nicht zur Hand, und ein deterministisches Abrufwerkzeug galt dort als
//! rein, auch wenn der Eintrag getruebt hineinging. Mit dem Feld traegt
//! der Eintrag seine Truebung mit.
//!
//! ⚑ **Die zweite Haelfte steht im Abruf** ([`crate::abruf::abrufen`]):
//! Ein Abruf ist getruebt, sobald einer seiner Treffer es ist (dieselbe
//! Regel wie [`crate::plan::Plan::truebung`] fuer Argumente). Ohne diese
//! Regel waere das Feld vorhanden und wirkungslos.
//!
//! ## Der Preis, und er wird hier genannt
//!
//! Der Kettenwert ist nur ueber eine **vollstaendige** Kette definiert.
//! Aus einer Session, die vor dem Ende ihres Plans abbricht, laesst sich
//! deshalb kein Eintrag belegen, und es kommt keiner hinein.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::hash::Hash;
use myl_types::ids::SitzungId;
use myl_types::merkle::{MerkleProof, MerkleTree};

use crate::kette::{pruefe_kette, Kettenbefund, Kettenglied};
use crate::plan::{Plan, Truebung};
use crate::registratur::Registratur;

/// Trennzeichen eines Gedaechtniseintrags.
pub const DST_GEDAECHTNISEINTRAG: &[u8] = b"MYELITH_GEDAECHTNISEINTRAG_v1";

/// Ein Eintrag: die Ausgabe eines Schritts, mit dem Weg dorthin.
///
/// **Der Inhalt steht nicht darin**, nur sein Commitment im Glied. Und
/// das Werkzeug steht auch nicht darin: Es folgt aus Plan und Stelle.
/// Die Truebung folgt auch daraus und steht trotzdem darin, damit ein
/// Abruf sie ohne den Plan kennt; die Aufnahme prueft, dass beide gleich
/// sind.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Eintrag {
    /// Die Session, in der der Schritt lief.
    pub sitzung: SitzungId,
    /// Die Adresse des Plans, zu dem der Schritt gehoert.
    pub plan: Hash,
    /// Die Stelle des Schritts im Plan.
    pub stelle: u16,
    /// Das Glied der Kette an dieser Stelle: Segment und Ausgabe.
    pub glied: Kettenglied,
    /// Die Truebung des Schritts, wie sein Plan sie ergibt.
    pub truebung: Truebung,
}

impl Eintrag {
    /// Die Blattdaten des Eintrags im Speicherbaum.
    ///
    /// Mit Trennzeichen, damit ein Eintrag nie mit einem Blatt eines
    /// anderen Baums verwechselt werden kann, der zufaellig dieselben
    /// Bytes traegt.
    pub fn blatt(&self) -> Vec<u8> {
        let mut daten = Vec::with_capacity(DST_GEDAECHTNISEINTRAG.len() + 32 * 4 + 3);
        daten.extend_from_slice(DST_GEDAECHTNISEINTRAG);
        daten.extend_from_slice(&borsh::to_vec(self).expect("Eintrag ist stets serialisierbar"));
        daten
    }
}

/// Warum ein Eintrag nicht belegt ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herkunftsfehler {
    /// Der Eintrag zeigt auf einen anderen Plan als den vorgelegten.
    FremderPlan {
        /// Die Planadresse im Eintrag.
        eintrag: Hash,
        /// Die Adresse des vorgelegten Plans.
        vorgelegt: Hash,
    },
    /// Die vorgelegte Kette gilt nicht fuer Session und Plan des Eintrags.
    ///
    /// **Hier landet auch der Eintrag mit fremder Session**: Die Session
    /// geht in den Anker ein, und unter einer anderen stimmt der Wert
    /// nicht mehr.
    KetteGiltNicht(Kettenbefund),
    /// Der Plan hat an dieser Stelle keinen Schritt.
    StelleFehlt {
        /// Die Stelle im Eintrag.
        stelle: u16,
        /// Schritte im Plan.
        plan: usize,
    },
    /// An der Stelle steht in der Kette ein anderes Glied.
    ///
    /// ⚑ **Der Fall, auf den es ankommt.** Kette, Plan und Session sind
    /// echt, nur ist der Eintrag nicht die Ausgabe des Schritts, als die
    /// er sich ausgibt.
    GliedStimmtNicht {
        /// Die Stelle im Eintrag.
        stelle: u16,
    },
    /// Der Eintrag nennt eine andere Truebung, als sein Plan ergibt.
    ///
    /// ⚑ **Der Weg, ueber den Fremdes rein wuerde:** ein Eintrag aus einem
    /// Schritt, der die Welt befragt, mit dem Feld auf rein.
    TruebungStimmtNicht {
        /// Die Stelle im Eintrag.
        stelle: u16,
        /// Was der Eintrag angibt.
        angegeben: Truebung,
        /// Was der Plan ergibt.
        abgeleitet: Truebung,
    },
}

impl std::fmt::Display for Herkunftsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FremderPlan { .. } => write!(f, "Eintrag zeigt auf einen anderen Plan"),
            Self::KetteGiltNicht(_) => write!(f, "Kette gilt nicht fuer Session und Plan des Eintrags"),
            Self::StelleFehlt { stelle, plan } => {
                write!(f, "Eintrag nennt Stelle {stelle}, der Plan hat {plan} Schritte")
            }
            Self::GliedStimmtNicht { stelle } => {
                write!(f, "an Stelle {stelle} steht in der Kette ein anderes Glied")
            }
            Self::TruebungStimmtNicht { stelle, angegeben, abgeleitet } => {
                write!(f, "Eintrag an Stelle {stelle} gibt sich als {angegeben:?} aus, der Plan ergibt {abgeleitet:?}")
            }
        }
    }
}

impl std::error::Error for Herkunftsfehler {}

/// Prueft, ob ein Eintrag die Ausgabe eines Schritts dieser Kette ist und
/// die Truebung traegt, die sein Plan ergibt.
///
/// `kettenwert` ist der Wert, den der Konsens fuer die Session fuehrt.
/// **Er kommt nicht vom Eintrag und nicht vom Speicher**, sonst pruefte
/// die Funktion eine Behauptung gegen sich selbst. Dasselbe gilt fuer die
/// Registratur: Sie sagt, welches Werkzeug die Welt befragt.
///
/// Die Reihenfolge der Pruefungen geht vom Billigen zum Teuren, und die
/// Stelle wird erst gelesen, wenn die Kette gilt: Davor ist nicht
/// gesichert, dass sie so lang ist wie der Plan.
pub fn pruefe_herkunft(
    eintrag: &Eintrag,
    plan: &Plan,
    glieder: &[Kettenglied],
    kettenwert: &Hash,
    reg: &Registratur,
) -> Result<(), Herkunftsfehler> {
    let vorgelegt = plan.adresse();
    if eintrag.plan != vorgelegt {
        return Err(Herkunftsfehler::FremderPlan { eintrag: eintrag.plan, vorgelegt });
    }
    let befund = pruefe_kette(&eintrag.sitzung, plan, glieder, kettenwert);
    if !befund.heil() {
        return Err(Herkunftsfehler::KetteGiltNicht(befund));
    }
    match glieder.get(eintrag.stelle as usize) {
        None => Err(Herkunftsfehler::StelleFehlt { stelle: eintrag.stelle, plan: plan.len() }),
        Some(glied) if *glied != eintrag.glied => {
            Err(Herkunftsfehler::GliedStimmtNicht { stelle: eintrag.stelle })
        }
        Some(_) => match truebung(eintrag, plan, reg) {
            Some(t) if t == eintrag.truebung => Ok(()),
            Some(abgeleitet) => Err(Herkunftsfehler::TruebungStimmtNicht {
                stelle: eintrag.stelle,
                angegeben: eintrag.truebung,
                abgeleitet,
            }),
            // Die Kette gilt und hat die Stelle, also auch der Plan.
            None => Err(Herkunftsfehler::StelleFehlt { stelle: eintrag.stelle, plan: plan.len() }),
        },
    }
}

/// Die Truebung eines Eintrags, abgeleitet aus seinem Plan.
///
/// `None`, wenn der Plan nicht der des Eintrags ist oder die Stelle
/// fehlt: Ueber einen Eintrag, dessen Plan nicht vorliegt, laesst sich
/// nichts sagen, und „rein" waere dann eine Behauptung.
pub fn truebung(eintrag: &Eintrag, plan: &Plan, reg: &Registratur) -> Option<Truebung> {
    if eintrag.plan != plan.adresse() {
        return None;
    }
    plan.truebung(reg).get(eintrag.stelle as usize).copied()
}

/// Der Speicher: eine Folge belegter Eintraege, nur zum Anhaengen.
///
/// ⚑ **Hinein fuehrt ein einziger Weg, und der prueft.** Die Liste ist
/// privat, es gibt kein Entfernen und kein Aendern, und der Typ laesst
/// sich mit Absicht nicht aus Bytes lesen: Ein eingelesener Speicher
/// waere einer, dessen Eintraege niemand geprueft hat.
///
/// **Das bindet den ehrlichen Betreiber, nicht den Pruefer.** Wer einem
/// fremden Speicher nicht traut, verlaesst sich nicht auf diesen Typ,
/// sondern faehrt [`pruefe_abruf`] und [`pruefe_herkunft`] selbst.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gedaechtnis {
    eintraege: Vec<Eintrag>,
}

impl Gedaechtnis {
    /// Ein leerer Speicher.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Nimmt einen Eintrag auf, wenn er belegt ist, und nennt seine
    /// Stelle im Speicher.
    pub fn nimm(
        &mut self,
        eintrag: Eintrag,
        plan: &Plan,
        glieder: &[Kettenglied],
        kettenwert: &Hash,
        reg: &Registratur,
    ) -> Result<u64, Herkunftsfehler> {
        pruefe_herkunft(&eintrag, plan, glieder, kettenwert, reg)?;
        self.eintraege.push(eintrag);
        Ok(self.eintraege.len() as u64 - 1)
    }

    /// Die Eintraege, nur lesend.
    pub fn eintraege(&self) -> &[Eintrag] {
        &self.eintraege
    }

    /// Wie viele Eintraege.
    pub fn len(&self) -> usize {
        self.eintraege.len()
    }

    /// Ob der Speicher leer ist.
    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty()
    }

    fn baum(&self) -> Option<MerkleTree> {
        let blaetter: Vec<Vec<u8>> = self.eintraege.iter().map(Eintrag::blatt).collect();
        let sichten: Vec<&[u8]> = blaetter.iter().map(Vec::as_slice).collect();
        MerkleTree::new(&sichten).ok()
    }

    /// Die Wurzel ueber alle Eintraege, in der Reihenfolge der Aufnahme.
    ///
    /// `None` fuer den leeren Speicher: Ein Baum ohne Blaetter ist im
    /// Protokoll nicht zulaessig, und eine erfundene Wurzel fuer „nichts"
    /// waere ein Wert, gegen den sich etwas beweisen liesse.
    pub fn wurzel(&self) -> Option<Hash> {
        self.baum().map(|b| b.root())
    }

    /// Der Beweis, dass der Eintrag an `stelle` zum Speicher gehoert.
    pub fn beweis(&self, stelle: u64) -> Option<MerkleProof> {
        let stelle = usize::try_from(stelle).ok()?;
        self.baum()?.proof(stelle).ok()
    }
}

/// Prueft, dass ein Eintrag an dieser Stelle im Speicher mit dieser
/// Wurzel steht.
///
/// **Das ist die halbe Pruefung eines Abrufs.** Sie sagt nichts
/// darueber, ob der Eintrag belegt ist; das sagt [`pruefe_herkunft`].
pub fn pruefe_abruf(wurzel: &Hash, eintrag: &Eintrag, stelle: u64, beweis: &MerkleProof) -> bool {
    beweis.verify(wurzel, &eintrag.blatt(), stelle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kette::{anker, kettenwert};
    use crate::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
    use crate::plan::{Quelle, Schritt};
    use myl_types::ids::{MerkleRoot, SegmentId};

    /// Eine Registratur mit einem rechnenden und einem Werkzeug, das die
    /// Welt befragt.
    fn registratur() -> (Registratur, MerkleRoot, MerkleRoot) {
        let mut r = Registratur::neu();
        let mach = |name: &str, art, herkunft| Werkzeugmanifest {
            name: name.into(),
            anbieter: "pruefstand".into(),
            revision: "1".into(),
            lizenz: "Apache-2.0".into(),
            art,
            herkunft,
        };
        let rein = r
            .nimm_werkzeug(mach("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("gueltig");
        // Ein externes Werkzeug kann nicht verankert sein.
        let welt = r
            .nimm_werkzeug(mach("abrufen", Werkzeugart::Extern, Herkunft::Bibliothek))
            .expect("gueltig");
        (r, rein, welt)
    }

    /// Ein Plan, in dem jeder Schritt die Ausgabe seines Vorgaengers
    /// nimmt, ausser `frei`: Der nimmt nichts.
    fn plan_mit(werkzeuge: &[MerkleRoot], frei: &[usize]) -> Plan {
        Plan::neu(
            werkzeuge
                .iter()
                .enumerate()
                .map(|(i, w)| Schritt {
                    werkzeug: *w,
                    argumente: if i == 0 || frei.contains(&i) {
                        vec![]
                    } else {
                        vec![Quelle::Schritt(i as u16 - 1)]
                    },
                })
                .collect(),
        )
        .expect("gueltiger Plan")
    }

    fn glied(n: u8) -> Kettenglied {
        Kettenglied { segment: SegmentId::new([n; 32]), ausgabe: Hash::sha256(&[n, n, n]) }
    }

    fn sitzung() -> SitzungId {
        SitzungId::new([77u8; 32])
    }

    /// Eine gelaufene Session: Plan, Glieder und der Wert dazu.
    fn gelaufen(werkzeuge: &[MerkleRoot], ab: u8) -> (Plan, Vec<Kettenglied>, Hash) {
        let p = plan_mit(werkzeuge, &[]);
        let g: Vec<Kettenglied> = (0..werkzeuge.len() as u8).map(|i| glied(ab + i)).collect();
        let wert = kettenwert(&anker(&sitzung(), &p.adresse()), &p, &g).expect("Laenge passt");
        (p, g, wert)
    }

    /// Ein Eintrag mit der Truebung, die sein Plan ergibt; die Registratur
    /// ist bei jedem Aufruf dieselbe, weil Werkzeuge nach Inhalt heissen.
    fn eintrag(p: &Plan, g: &[Kettenglied], stelle: u16) -> Eintrag {
        let (reg, _, _) = registratur();
        let truebung = p.truebung(&reg)[stelle as usize];
        Eintrag { sitzung: sitzung(), plan: p.adresse(), stelle, glied: g[stelle as usize].clone(), truebung }
    }

    /// Was die Ausgabe eines Schritts einer geltenden Kette ist, kommt
    /// hinein und bekommt die naechste Stelle.
    #[test]
    fn ein_belegter_eintrag_kommt_hinein() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt, rein], 1);
        let mut s = Gedaechtnis::neu();
        assert!(s.is_empty());
        assert_eq!(s.wurzel(), None, "der leere Speicher hat keine Wurzel");

        assert_eq!(s.nimm(eintrag(&p, &g, 2), &p, &g, &wert, &reg), Ok(0));
        assert_eq!(s.nimm(eintrag(&p, &g, 0), &p, &g, &wert, &reg), Ok(1));
        assert_eq!(s.len(), 2);
        assert_eq!(s.eintraege()[0].stelle, 2);
        assert!(s.wurzel().is_some());
    }

    /// ⚑ **Die Anforderung aus Kap. 8.4, als Test:** Was sich auf keinen
    /// Schritt zurueckfuehren laesst, kommt nicht hinein. Vier Wege, es
    /// zu versuchen, und der Speicher bleibt bei allen leer.
    #[test]
    fn was_sich_nicht_zurueckfuehren_laesst_kommt_nicht_hinein() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt, rein], 1);
        let mut s = Gedaechtnis::neu();

        // Erfunden: Kette, Plan und Session sind echt, die Ausgabe nicht.
        let mut erfunden = eintrag(&p, &g, 1);
        erfunden.glied.ausgabe = Hash::sha256(b"eingeschmuggelt");
        assert_eq!(
            s.nimm(erfunden, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::GliedStimmtNicht { stelle: 1 })
        );

        // Verschoben: eine echte Ausgabe, aber als die eines anderen
        // Schritts ausgegeben.
        let mut verschoben = eintrag(&p, &g, 1);
        verschoben.stelle = 2;
        assert_eq!(
            s.nimm(verschoben, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::GliedStimmtNicht { stelle: 2 })
        );

        // Hinter dem Plan: Die Stelle gibt es nicht.
        let mut dahinter = eintrag(&p, &g, 1);
        dahinter.stelle = 3;
        assert_eq!(
            s.nimm(dahinter, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::StelleFehlt { stelle: 3, plan: 3 })
        );

        // Fremde Session: Die Kette gilt unter ihr nicht.
        let mut fremd = eintrag(&p, &g, 1);
        fremd.sitzung = SitzungId::new([78u8; 32]);
        assert!(matches!(
            s.nimm(fremd, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::KetteGiltNicht(Kettenbefund::WertStimmtNicht { .. }))
        ));

        // Fremder Plan: Der Eintrag zeigt auf einen, der nicht vorliegt.
        let anderer = plan_mit(&[rein, rein, rein], &[]);
        assert!(matches!(
            s.nimm(eintrag(&anderer, &g, 1), &p, &g, &wert, &reg),
            Err(Herkunftsfehler::FremderPlan { .. })
        ));

        assert!(s.is_empty(), "nichts davon ist hineingekommen");
    }

    /// Eine Kette, die selbst nicht gilt, belegt nichts: weder mit
    /// falschem Wert noch zu kurz.
    #[test]
    fn eine_kette_die_nicht_gilt_belegt_nichts() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt, rein], 1);
        let e = eintrag(&p, &g, 0);

        let falsch = Hash::sha256(b"behauptet");
        assert!(matches!(
            pruefe_herkunft(&e, &p, &g, &falsch, &reg),
            Err(Herkunftsfehler::KetteGiltNicht(Kettenbefund::WertStimmtNicht { .. }))
        ));
        // Abgebrochen: Der Schritt lief, die Kette ist aber nicht
        // vollstaendig, und ueber sie ist kein Wert definiert.
        assert_eq!(
            pruefe_herkunft(&e, &p, &g[..2], &wert, &reg),
            Err(Herkunftsfehler::KetteGiltNicht(Kettenbefund::LaengeStimmtNicht {
                plan: 3,
                kette: 2
            }))
        );
        assert_eq!(pruefe_herkunft(&e, &p, &g, &wert, &reg), Ok(()));
    }

    /// Ein Abruf ist an Wurzel, Stelle und Eintrag gebunden.
    #[test]
    fn ein_abruf_ist_an_wurzel_und_stelle_gebunden() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt, rein], 1);
        let mut s = Gedaechtnis::neu();
        for i in 0..3 {
            s.nimm(eintrag(&p, &g, i), &p, &g, &wert, &reg).expect("belegt");
        }
        let wurzel = s.wurzel().expect("nicht leer");

        for i in 0..3u64 {
            let b = s.beweis(i).expect("Stelle vorhanden");
            assert!(pruefe_abruf(&wurzel, &s.eintraege()[i as usize], i, &b));
        }
        assert!(s.beweis(3).is_none());

        let b1 = s.beweis(1).expect("da");
        let e1 = s.eintraege()[1].clone();
        // Andere Stelle, anderer Eintrag, andere Wurzel: jeweils nein.
        assert!(!pruefe_abruf(&wurzel, &e1, 2, &b1));
        assert!(!pruefe_abruf(&wurzel, &s.eintraege()[0], 1, &b1));
        let mut veraendert = e1.clone();
        veraendert.glied.ausgabe = Hash::sha256(b"anders");
        assert!(!pruefe_abruf(&wurzel, &veraendert, 1, &b1));
        assert!(!pruefe_abruf(&Hash::sha256(b"fremd"), &e1, 1, &b1));
    }

    /// Nur anhaengen: Jede Aufnahme aendert die Wurzel, und dieselben
    /// Eintraege in anderer Reihenfolge sind ein anderer Speicher.
    #[test]
    fn die_wurzel_haengt_an_inhalt_und_reihenfolge() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt], 1);

        let mut a = Gedaechtnis::neu();
        a.nimm(eintrag(&p, &g, 0), &p, &g, &wert, &reg).expect("belegt");
        let eins = a.wurzel().expect("da");
        a.nimm(eintrag(&p, &g, 1), &p, &g, &wert, &reg).expect("belegt");
        let zwei = a.wurzel().expect("da");
        assert_ne!(eins, zwei);

        let mut b = Gedaechtnis::neu();
        b.nimm(eintrag(&p, &g, 1), &p, &g, &wert, &reg).expect("belegt");
        b.nimm(eintrag(&p, &g, 0), &p, &g, &wert, &reg).expect("belegt");
        assert_ne!(b.wurzel().expect("da"), zwei);

        // Zweimal gebaut ergibt dasselbe.
        let mut c = Gedaechtnis::neu();
        c.nimm(eintrag(&p, &g, 0), &p, &g, &wert, &reg).expect("belegt");
        c.nimm(eintrag(&p, &g, 1), &p, &g, &wert, &reg).expect("belegt");
        assert_eq!(c.wurzel().expect("da"), zwei);
    }

    /// ⚑ Truebung ueberlebt den Speicher: Sie haengt am Plan, auf den der
    /// Eintrag zeigt, und erbt sich wie dort ueber Argumente.
    #[test]
    fn die_truebung_folgt_aus_dem_plan_des_eintrags() {
        let (reg, rein, welt) = registratur();
        // Schritt 0 rechnet, 1 befragt die Welt, 2 rechnet auf 1, und 3
        // rechnet auf nichts.
        let p = plan_mit(&[rein, welt, rein, rein], &[3]);
        let g: Vec<Kettenglied> = (1..=4).map(glied).collect();

        let t: Vec<Option<Truebung>> =
            (0..4).map(|i| truebung(&eintrag(&p, &g, i), &p, &reg)).collect();
        assert_eq!(
            t,
            vec![
                Some(Truebung::Rein),
                Some(Truebung::Getruebt),
                Some(Truebung::Getruebt),
                Some(Truebung::Rein),
            ]
        );

        // Ohne den richtigen Plan gibt es keine Aussage, auch nicht „rein".
        let anderer = plan_mit(&[rein, rein, rein, rein], &[]);
        assert_eq!(truebung(&eintrag(&p, &g, 1), &anderer, &reg), None);
        let mut dahinter = eintrag(&p, &g, 1);
        dahinter.stelle = 9;
        assert_eq!(truebung(&dahinter, &p, &reg), None);
    }

    /// ⚑ **Fund 695, als Test:** Ein Eintrag aus einem Schritt, der die
    /// Welt befragt, kommt mit dem Feld auf rein nicht hinein, und auch
    /// nicht umgekehrt. Mit der richtigen Truebung kommt er hinein, und
    /// die Wurzel bindet das Feld.
    #[test]
    fn die_truebung_im_eintrag_wird_bei_der_aufnahme_geprueft() {
        let (reg, rein, welt) = registratur();
        let (p, g, wert) = gelaufen(&[rein, welt, rein], 1);
        let mut s = Gedaechtnis::neu();

        let mut reingewaschen = eintrag(&p, &g, 1);
        assert_eq!(reingewaschen.truebung, Truebung::Getruebt);
        reingewaschen.truebung = Truebung::Rein;
        assert_eq!(
            s.nimm(reingewaschen, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::TruebungStimmtNicht {
                stelle: 1,
                angegeben: Truebung::Rein,
                abgeleitet: Truebung::Getruebt
            })
        );
        let mut uebertrieben = eintrag(&p, &g, 0);
        uebertrieben.truebung = Truebung::Getruebt;
        assert!(matches!(
            s.nimm(uebertrieben, &p, &g, &wert, &reg),
            Err(Herkunftsfehler::TruebungStimmtNicht { stelle: 0, .. })
        ));
        assert!(s.is_empty());

        let echt = eintrag(&p, &g, 1);
        assert_eq!(s.nimm(echt.clone(), &p, &g, &wert, &reg), Ok(0));
        let wurzel = s.wurzel().expect("nicht leer");
        let b = s.beweis(0).expect("da");
        assert!(pruefe_abruf(&wurzel, &echt, 0, &b));
        let mut gefaelscht = echt;
        gefaelscht.truebung = Truebung::Rein;
        assert!(!pruefe_abruf(&wurzel, &gefaelscht, 0, &b), "die Wurzel bindet die Truebung");
    }

    /// Der Eintrag ist ein Wire-Typ: Er uebersteht die Kodierung, und
    /// sein Blatt traegt das Trennzeichen.
    #[test]
    fn der_eintrag_uebersteht_die_kodierung() {
        let (_, rein, welt) = registratur();
        let (p, g, _) = gelaufen(&[rein, welt], 1);
        let e = eintrag(&p, &g, 1);
        let bytes = borsh::to_vec(&e).expect("serialisierbar");
        assert_eq!(borsh::from_slice::<Eintrag>(&bytes).expect("lesbar"), e);
        assert!(e.blatt().starts_with(DST_GEDAECHTNISEINTRAG));
        assert_eq!(e.blatt().len(), DST_GEDAECHTNISEINTRAG.len() + bytes.len());
    }
}
