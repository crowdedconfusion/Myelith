//! Kontrollsegmente (Phase 3, Whitepaper Kap. 6.7).
//!
//! Die drei Verifikationsstufen sanktionieren Betrug entweder durch
//! Vergleich zweier Berechnungen oder durch nachträgliche Stichprobe.
//! Beide setzen voraus, dass der Zwillings-Pod ehrlich rechnet oder dass
//! ein Angreifer **wiederholt** auffällt. Für den **einmaligen** Eingriff
//! eines Angreifers, der beide Pods kontrolliert, bleibt eine Lücke.
//!
//! Kontrollsegmente verkleinern sie: Das Netz hält einen Vorrat von
//! Segmenten, deren korrektes Ergebnis bereits vorliegt, und schleust sie
//! mit einem Anteil γ in den regulären Auftragsstrom.
//!
//! **Der Sicherheitsgewinn liegt in der Ungewissheit des Angreifers.** Da
//! er bei keinem Segment weiß, ob es eine Kontrolle ist, trägt bereits
//! der **erste** Manipulationsversuch ein Entdeckungsrisiko von γ, und
//! zwar unabhängig davon, ob er beide Pods kontrolliert. Das ist der
//! einzige Mechanismus der Architektur, der gegen den Einzelangriff
//! wirkt.
//!
//! ## ⚑ Was dieses Modul **nicht** leistet
//!
//! Kap. 6.7 nennt drei Konstruktionsanforderungen. Dieses Modul erfüllt
//! zwei davon; die erste kann es nicht erfüllen, und das gehört gesagt,
//! bevor jemand den Haken für die Phase als Beleg liest:
//!
//! 1. **Ununterscheidbarkeit** — „Kontrollsegmente müssen der realen
//!    Prompt-Verteilung entstammen und im Timing-, Längen- und
//!    Kontextprofil unauffällig sein. Erkennt ein Miner sie an
//!    statistischen Merkmalen, ist der Mechanismus wirkungslos."
//!
//!    **Das ist eine Eigenschaft der Daten, nicht des Codes.** Kein
//!    Datentyp kann erzwingen, dass ein Prompt unauffällig ist. Das
//!    Whitepaper führt es selbst als offene Messfrage (Kap. 11, Punkt 5),
//!    und diese Implementierung ändert daran nichts. Sie stellt nur
//!    sicher, dass die **Erneuerung** aus echten Segmenten speist, was
//!    die Verteilung von selbst richtig hält (Anforderung 2).
//!
//!    Wer γ konfiguriert und einen Vorrat füllt, hat damit **keinen**
//!    Schutz gegen einen Miner, der Kontrollsegmente an ihrer Länge
//!    erkennt.
//!
//!    ## ⚑ Fund 58: Ein zu kleiner Vorrat verrät sich von selbst
//!
//!    Gemessen am 2026-08-25 in [`crate::unterscheider`], und das
//!    Ergebnis betrifft nicht die Prompts, sondern die **Mechanik**:
//!
//!    Der Vorrat ist endlich, der Auftragsstrom nicht. Wird öfter
//!    eingeschleust, als der Vorrat verschiedene Segmente hält,
//!    **wiederholen sich Ids**. Echte Arbeit wiederholt sich nie. Also
//!    ist jedes zweite Auftreten einer Id ein **Beweis**, kein
//!    Verdacht, und ein Miner braucht dafür nur ein Gedächtnis.
//!
//!    Bei γ = 2 % über 100 000 Aufträge:
//!
//!    | Vorrat | erkannte Kontrollen | Fehlalarme |
//!    |---|---|---|
//!    | 64 | **96,8 %** | 0 |
//!    | 256 | 87,2 % | 0 |
//!    | 1024 | 48,8 % | 0 |
//!    | 2048 | 0 % | 0 |
//!
//!    **Null Fehlalarme heißt: Der Angreifer geht kein Risiko ein.** Er
//!    rechnet die erkannten Kontrollen ehrlich und manipuliert den Rest.
//!
//!    Die Bedingung ist einfach und steht als
//!    [`KontrollsegmentVorrat::reicht_fuer`] bereit: **Der Vorrat muss
//!    mindestens so viele Segmente halten, wie über das
//!    Beobachtungsfenster eingeschleust wird.** Das ist eine
//!    notwendige, keine hinreichende Bedingung: Sie beseitigt genau
//!    diese eine, sichere Spur, nicht die Unterscheidung an Länge,
//!    Timing oder Inhalt.
//!
//!    ### Seit dem 2026-08-27 ist die Vorratsgröße ein Governance-Parameter
//!
//!    Bis dahin stand hier: „γ ist ein Governance-Parameter, die
//!    Vorratsgröße ist keiner. Die einzigen je geschriebenen Werte
//!    stehen in Tests: 10 und 100." Beides galt, und beides war der
//!    Befund: Bei γ = 2 % trägt ein Vorrat von 100 genau 5 000
//!    Aufträge.
//!
//!    Jetzt führt `myl-governance` **zwei** Parameter — die
//!    Vorratsgröße und das Beobachtungsfenster — und eine Invariante,
//!    die beide an γ bindet: `Vorrat ≥ ⌈Fenster · γ⌉`. Die Formel
//!    dafür ist [`crate::unterscheider::noetiger_vorrat`]; die
//!    Governance benutzt sie, statt sie zu wiederholen.
//!
//!    ⚑ **Der Zug, gegen den die Invariante gebaut ist, ist der, der
//!    harmlos aussieht:** γ erhöhen, ohne den Vorrat mitzuziehen. Das
//!    liest sich wie eine schärfere Kontrolle und ist ihre
//!    Abschaltung, denn eine höhere Rate erschöpft einen gleich großen
//!    Vorrat schneller.
//!
//!    **Der Anfangswert bleibt vorläufig.** [`VORRAT_VORGABE`] ist der
//!    gemessene Wert aus Fund 58, [`BEOBACHTUNGSFENSTER_VORGABE`] die
//!    Stromlänge, über die gemessen wurde. Was fehlt, ist die
//!    **Auftragsrate des Netzes**: Erst mit ihr ließe sich sagen, wie
//!    lange ein Fenster dieser Größe in Sekunden ist.
//!
//! 2. **Vorratserneuerung** — erfüllt durch [`KontrollsegmentVorrat::erneuern`]:
//!    Übernahme abgeschlossener, per Stufe 2 vollständig geprüfter
//!    Echtsegmente, mit Verdrängung der ältesten.
//!
//! 3. **Kostenehrlichkeit** — γ ist ein Governance-Parameter
//!    (`myl_governance::Parameter::Kontrollsegmentanteil`) und geht als
//!    Overhead in die Kostenrechnung ein. Seit dem 2026-08-27 gilt
//!    dasselbe für die Vorratsgröße und ihr Beobachtungsfenster; die
//!    Vorberechnung des Vorrats ist Rechenzeit, seine Vorhaltung
//!    Speicher, und beides wächst mit γ.
//!
//! ## Die Sicherheitsbedingung der Einschleusung
//!
//! [`einschleusungsplan`] ist **deterministisch**: Derselbe Seed ergibt
//! dieselben Positionen. Das ist für die Nachprüfbarkeit nötig — ein
//! Gateway muss belegen können, dass es γ eingehalten hat.
//!
//! **Es ist zugleich die Stelle, an der der Mechanismus bricht, wenn der
//! Seed zu früh bekannt wird.** Wer ihn kennt, weiß, welche Aufträge
//! Kontrollen sind, und manipuliert genau die anderen. Der Seed gehört
//! dem Gateway und darf erst nach Auslieferung offengelegt werden; das
//! ist eine Betriebsbedingung, die dieses Modul nicht erzwingen kann,
//! und sie steht hier, damit sie nicht verlorengeht.

use std::collections::BTreeMap;

use myl_types::hash::Hash;
use myl_types::ids::{EpochId, SegmentId};

/// Das Beobachtungsfenster des Wiederholungsunterscheiders, in Aufträgen.
///
/// **Eine Annahme über den Angreifer, keine Messung.** Ein Gedächtnis
/// kostet ihn nichts als Speicher: 100 000 Segment-Ids sind 3,2 MB, und
/// eine Menge nachzuschlagen ist konstant teuer. Die Zahl sagt deshalb
/// nicht, was er kann, sondern **wogegen das Protokoll sich zu schützen
/// verpflichtet**. Sie ist zugleich die Stromlänge, über die Fund 58
/// gemessen wurde, also die einzige, für die eine Zahl vorliegt.
///
/// ⚑ **Sie ersetzt keine Messung der Auftragsrate.** Wie viele Aufträge
/// das Netz je Zeiteinheit trägt, ist unbekannt; erst damit ließe sich
/// sagen, wie lange ein Fenster dieser Größe in Sekunden ist. Bis dahin
/// ist der Wert eine Setzung, und er gehört genau deshalb in die
/// Governance und nicht in eine Konstante des Rechenpfads.
pub const BEOBACHTUNGSFENSTER_VORGABE: u64 = 100_000;

/// Die Vorgabegröße des Vorrats.
///
/// Der kleinste in der Messung zu Fund 58 geprüfte Wert, bei dem der
/// Wiederholungsunterscheider über 100 000 Aufträge bei γ = 2 % **keine
/// einzige** Kontrolle erkennt (64 → 96,8 %, 256 → 87,2 %,
/// 1024 → 48,8 %, 2048 → 0 %). Die rechnerische Schranke liegt bei
/// [`noetiger_vorrat`]`(100_000, 2, 100)` = 2 000; die 2 048 sind der
/// gemessene Wert und lassen etwas Luft.
///
/// **Der Vorrat ist eine Sicherheitsschranke, keine Puffergröße.** Wer
/// ihn zu klein wählt, hebt die Stichprobenprüfung nicht ab, sondern
/// **um**: Ein Miner mit Gedächtnis erkennt die Kontrollen, rechnet
/// genau die ehrlich und manipuliert den Rest — ohne einen einzigen
/// Fehlalarm einzugehen.
pub const VORRAT_VORGABE: u64 = 2_048;

/// Ein Segment mit bekanntem Soll-Ergebnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kontrollsegment {
    /// Die Id, unter der es im Auftragsstrom läuft.
    pub segment_id: SegmentId,
    /// Das Commitment, das ein ehrlicher Pod liefern muss.
    pub soll_commitment: Hash,
    /// Epoche der Aufnahme in den Vorrat — Grundlage der Erneuerung.
    pub aufgenommen_in: EpochId,
}

/// Ergebnis der Prüfung eines gelieferten Commitments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kontrollergebnis {
    /// Das Segment ist keine Kontrolle; hier ist nichts zu entscheiden.
    ///
    /// **Ausdrücklich nicht „bestanden".** Ein Vorgabewert, der wie ein
    /// Freispruch aussieht, war Fund 41 an anderer Stelle: Dort galt eine
    /// leere Spur als geprüft.
    KeineKontrolle,
    /// Das gelieferte Commitment stimmt mit dem Soll überein.
    Bestanden,
    /// Es weicht ab. Das ist ein Betrugsnachweis ohne Bisektion: Das
    /// richtige Ergebnis lag bereits vor.
    Abgewichen { soll: Hash, ist: Hash },
}

/// Fehler des Vorrats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VorratFehler {
    /// Der Vorrat ist leer, es kann nicht eingeschleust werden.
    VorratLeer,
    /// Die Rate γ ist unbrauchbar (Nenner null oder γ > 1).
    UnbrauchbareRate { zaehler: u64, nenner: u64 },
}

impl std::fmt::Display for VorratFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VorratLeer => write!(
                f,
                "der Kontrollsegment-Vorrat ist leer; ohne Vorrat gibt es keinen Schutz \
                 gegen den einmaligen Eingriff"
            ),
            Self::UnbrauchbareRate { zaehler, nenner } => {
                write!(f, "unbrauchbare Rate {}/{}", zaehler, nenner)
            }
        }
    }
}

impl std::error::Error for VorratFehler {}

/// Der Vorrat an Kontrollsegmenten.
#[derive(Debug, Clone, Default)]
pub struct KontrollsegmentVorrat {
    segmente: BTreeMap<SegmentId, Kontrollsegment>,
    hoechstzahl: usize,
}

impl KontrollsegmentVorrat {
    /// Neuer Vorrat mit einer Obergrenze.
    ///
    /// Die Obergrenze ist nötig, weil die Erneuerung sonst unbegrenzt
    /// wüchse: Jedes geprüfte Echtsegment wäre ein Kandidat.
    pub fn neu(hoechstzahl: usize) -> Self {
        Self {
            segmente: BTreeMap::new(),
            hoechstzahl,
        }
    }

    /// Ein Vorrat, dessen Größe aus Fenster und γ **folgt** statt geraten
    /// zu werden.
    ///
    /// **Warum es diesen Weg gibt:** [`Self::neu`] nimmt jede Zahl an,
    /// und die einzigen je geschriebenen waren 10 und 100 — beide aus
    /// Tests, beide weit unter jeder tragfähigen Schranke (Fund 58). Wer
    /// hier hereinkommt, kann die Größe nicht mehr versehentlich zu
    /// klein wählen; er muss sich dazu über das Fenster äußern.
    ///
    /// Mindestens ein Segment, auch bei γ = 0: Ein leerer Vorrat ist
    /// kein kleiner Vorrat, sondern gar keine Kontrolle.
    pub fn fuer_fenster(fenster: u64, gamma_zaehler: u64, gamma_nenner: u64) -> Self {
        let noetig = crate::unterscheider::noetiger_vorrat(fenster, gamma_zaehler, gamma_nenner);
        let groesse = noetig.max(1).min(usize::MAX as u64) as usize;
        Self::neu(groesse)
    }

    /// Nimmt ein Segment mit bekanntem Soll-Ergebnis auf.
    ///
    /// **Verdrängt das älteste**, wenn die Obergrenze erreicht ist. Bei
    /// gleichem Alter entscheidet die Segment-Id, damit die Verdrängung
    /// auf jedem Knoten dieselbe ist.
    pub fn aufnehmen(&mut self, k: Kontrollsegment) {
        self.segmente.insert(k.segment_id, k);
        while self.segmente.len() > self.hoechstzahl {
            let aeltester = self
                .segmente
                .values()
                .min_by_key(|k| (k.aufgenommen_in.0, k.segment_id))
                .map(|k| k.segment_id)
                .expect("nicht leer");
            self.segmente.remove(&aeltester);
        }
    }

    /// **Vorratserneuerung** (Kap. 6.7, Anforderung 2).
    ///
    /// Übernimmt ein abgeschlossenes, per Stufe 2 vollständig geprüftes
    /// Echtsegment in den Vorrat.
    ///
    /// **„Geprüft" ist Bedingung, nicht Beschreibung.** Ein Segment, das
    /// nur den Stufe-1-Vergleich bestanden hat, taugt nicht: Bestünde es
    /// aus einer Kollusion beider Pods, wäre sein falsches Ergebnis
    /// fortan das **Soll**, und jeder ehrliche Miner fiele daran durch.
    /// Der Aufrufer verantwortet die Bedingung; der Typ kann sie nicht
    /// erzwingen, deshalb steht sie hier und im Parameternamen.
    pub fn erneuern(
        &mut self,
        segment_id: SegmentId,
        stufe2_geprueftes_commitment: Hash,
        epoche: EpochId,
    ) {
        self.aufnehmen(Kontrollsegment {
            segment_id,
            soll_commitment: stufe2_geprueftes_commitment,
            aufgenommen_in: epoche,
        });
    }

    /// Prüft ein geliefertes Commitment gegen das Soll.
    pub fn pruefen(&self, segment_id: &SegmentId, geliefert: &Hash) -> Kontrollergebnis {
        match self.segmente.get(segment_id) {
            None => Kontrollergebnis::KeineKontrolle,
            Some(k) if k.soll_commitment == *geliefert => Kontrollergebnis::Bestanden,
            Some(k) => Kontrollergebnis::Abgewichen {
                soll: k.soll_commitment,
                ist: *geliefert,
            },
        }
    }

    /// Ist dieses Segment eine Kontrolle?
    ///
    /// **Nur für das Gateway und den Prüfer.** Käme diese Auskunft je zum
    /// ausführenden Miner, wäre der ganze Mechanismus wirkungslos.
    pub fn ist_kontrolle(&self, segment_id: &SegmentId) -> bool {
        self.segmente.contains_key(segment_id)
    }

    /// Zahl der vorrätigen Segmente.
    /// Ob dieser Vorrat für `auftraege` Aufträge bei Rate γ reicht,
    /// **ohne dass sich eine Id wiederholt** (Fund 58).
    ///
    /// Wiederholt sich eine, ist sie für einen Miner mit Gedächtnis ein
    /// Beweis, dass es sich um eine Kontrolle handelt, und der
    /// Mechanismus verliert genau dort seine Wirkung.
    ///
    /// **Notwendig, nicht hinreichend:** Auch ein ausreichender Vorrat
    /// schützt nicht gegen Unterscheidung an Länge, Timing oder Inhalt.
    /// ⚑ **Die Formel steht in [`crate::unterscheider::noetiger_vorrat`]
    /// und wird hier benutzt, nicht wiederholt.** Bis zum 2026-08-27
    /// stand sie an beiden Orten ausgeschrieben — zwei Fassungen
    /// derselben Sicherheitsschranke, von denen eine hätte
    /// davonlaufen können, ohne dass ein Test es bemerkt. Dieselbe
    /// Arbeitsteilung wie zwischen `myl-governance` und
    /// `myl_tokenomics::s_min`.
    pub fn reicht_fuer(&self, auftraege: usize, gamma_zaehler: u64, gamma_nenner: u64) -> bool {
        if gamma_nenner == 0 {
            return true;
        }
        let noetig = crate::unterscheider::noetiger_vorrat(
            auftraege as u64,
            gamma_zaehler,
            gamma_nenner,
        );
        self.hoechstzahl as u64 >= noetig
    }

    /// Wie viele Aufträge dieser Vorrat bei Rate γ trägt, bevor sich die
    /// erste Id wiederholt. Siehe [`Self::reicht_fuer`] zur Formel.
    pub fn reichweite(&self, gamma_zaehler: u64, gamma_nenner: u64) -> usize {
        let n = crate::unterscheider::reichweite(
            self.hoechstzahl as u64,
            gamma_zaehler,
            gamma_nenner,
        );
        n.min(usize::MAX as u64) as usize
    }

    pub fn len(&self) -> usize {
        self.segmente.len()
    }

    /// Ist der Vorrat leer?
    pub fn is_empty(&self) -> bool {
        self.segmente.is_empty()
    }

    /// Die Segmente, in Id-Reihenfolge.
    pub fn segmente(&self) -> impl Iterator<Item = &Kontrollsegment> {
        self.segmente.values()
    }
}

/// Welche Auftragspositionen ein Kontrollsegment tragen (Anteil γ).
///
/// **Parameter:**
/// - `anzahl_auftraege`: Länge des Auftragsstroms
/// - `gamma_zaehler`, `gamma_nenner`: der Anteil γ
/// - `seed`: der Gateway-Seed, siehe Sicherheitsbedingung in der Moduldoku
///
/// **Returns:** die Positionen, aufsteigend und ohne Doppelung.
///
/// **Aufgerundet**, wie die Stichproben-Lotterie in `myl-scheduler`: Bei
/// γ = 2 % und 10 Aufträgen ist eine Kontrolle besser als keine. Abrunden
/// hieße, dass kleine Auftragsströme gar nicht kontrolliert werden, und
/// genau dort sitzt der Einzelangriff.
pub fn einschleusungsplan(
    anzahl_auftraege: usize,
    gamma_zaehler: u64,
    gamma_nenner: u64,
    seed: &[u8; 32],
) -> Result<Vec<usize>, VorratFehler> {
    if gamma_nenner == 0 || gamma_zaehler > gamma_nenner {
        return Err(VorratFehler::UnbrauchbareRate {
            zaehler: gamma_zaehler,
            nenner: gamma_nenner,
        });
    }
    if anzahl_auftraege == 0 || gamma_zaehler == 0 {
        return Ok(vec![]);
    }

    let anzahl = ((anzahl_auftraege as u128 * gamma_zaehler as u128)
        .div_ceil(gamma_nenner as u128)) as usize;
    let anzahl = anzahl.min(anzahl_auftraege);

    // Deterministische Auswahl ohne Doppelung: Positionen nach einem
    // Seed-abhängigen Schlüssel sortieren und die ersten `anzahl` nehmen.
    // Dasselbe Muster wie die Stichproben-Lotterie: reproduzierbar für
    // den Prüfer, unvorhersehbar ohne den Seed.
    let mut mit_schluessel: Vec<(Hash, usize)> = (0..anzahl_auftraege)
        .map(|i| {
            let mut daten = Vec::with_capacity(40);
            daten.extend_from_slice(seed);
            daten.extend_from_slice(&(i as u64).to_le_bytes());
            (Hash::sha256(&daten), i)
        })
        .collect();
    mit_schluessel.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()).then(a.1.cmp(&b.1)));

    let mut positionen: Vec<usize> = mit_schluessel.into_iter().take(anzahl).map(|(_, i)| i).collect();
    positionen.sort_unstable();
    Ok(positionen)
}

#[cfg(test)]
mod fund_58 {
    use super::*;

    /// **Die Werte, die im Projekt stehen, reichen nicht.**
    ///
    /// Die einzigen je geschriebenen Vorratsgrößen sind 10 und 100, und
    /// beide stehen in Tests. Bei γ = 2 % trägt 100 genau 5 000
    /// Aufträge; danach wiederholt sich jede eingeschleuste Id, und ein
    /// Miner mit Gedächtnis erkennt sie **sicher**.
    #[test]
    fn ein_vorrat_von_hundert_traegt_fuenftausend_auftraege() {
        let v = KontrollsegmentVorrat::neu(100);
        assert_eq!(v.reichweite(2, 100), 5_000);
        assert!(v.reicht_fuer(5_000, 2, 100));
        assert!(
            !v.reicht_fuer(5_001, 2, 100),
            "ein Vorrat von 100 dürfte nicht für mehr als 5000 Aufträge gelten"
        );
    }

    /// **Die Vorgabewerte erfüllen ihre eigene Schranke.**
    ///
    /// Stünde hier ein Vorrat unter der Schranke, wäre die
    /// Governance-Invariante schon vor der ersten Abstimmung verletzt,
    /// und jeder Ablehnungstest darüber hätte den falschen Grund.
    #[test]
    fn die_vorgabewerte_tragen_ihr_eigenes_fenster() {
        let v = KontrollsegmentVorrat::neu(VORRAT_VORGABE as usize);
        assert!(
            v.reicht_fuer(BEOBACHTUNGSFENSTER_VORGABE as usize, 2, 100),
            "der Vorgabevorrat {} trägt das Vorgabefenster {} bei γ = 2 % nicht",
            VORRAT_VORGABE,
            BEOBACHTUNGSFENSTER_VORGABE
        );
        // **Und er liegt über der Schranke, nicht auf ihr.** Geprüft
        // wird die Regel, nicht die Zahl: Bei den heutigen Vorgaben
        // sind es 2 048 gegen 2 000, aber ein Test gegen die getippte
        // 2 000 schlüge bei jeder richtigen Änderung des Fensters fehl
        // und erzeugte Druck, sie zurückzunehmen.
        let schranke =
            crate::unterscheider::noetiger_vorrat(BEOBACHTUNGSFENSTER_VORGABE, 2, 100);
        assert!(
            VORRAT_VORGABE > schranke,
            "der Vorgabevorrat {VORRAT_VORGABE} liegt auf oder unter der Schranke {schranke}; \
             ohne Luft ist jede Erhöhung des Fensters sofort ein Bruch"
        );
    }

    /// [`KontrollsegmentVorrat::fuer_fenster`] leitet die Größe ab,
    /// statt sie raten zu lassen.
    #[test]
    fn fuer_fenster_waehlt_die_groesse_selbst() {
        let v = KontrollsegmentVorrat::fuer_fenster(100_000, 2, 100);
        assert!(v.reicht_fuer(100_000, 2, 100));
        assert_eq!(v.reichweite(2, 100), 100_000);
        // Auch ohne Einschleusung bleibt ein Segment übrig: Ein leerer
        // Vorrat ist keine kleine Kontrolle, sondern gar keine.
        let ohne = KontrollsegmentVorrat::fuer_fenster(100_000, 0, 100);
        assert!(ohne.reicht_fuer(100_000, 0, 100));
    }

    /// **Gegenprobe zur Zusammenführung:** Die Methode und die freie
    /// Funktion sind dieselbe Rechnung, über den ganzen Bereich, in dem
    /// beide auswertbar sind.
    #[test]
    fn methode_und_freie_funktion_sind_dieselbe_rechnung() {
        for auftraege in [0usize, 1, 999, 10_000, 100_000, 1_000_000] {
            for (gz, gn) in [(0u64, 100u64), (1, 100), (2, 100), (3, 100), (1, 1)] {
                let noetig = crate::unterscheider::noetiger_vorrat(auftraege as u64, gz, gn);
                let knapp = KontrollsegmentVorrat::neu(noetig as usize);
                assert!(
                    knapp.reicht_fuer(auftraege, gz, gn),
                    "genau der nötige Vorrat {noetig} muss reichen ({auftraege}, {gz}/{gn})"
                );
                if noetig > 0 {
                    let zu_klein = KontrollsegmentVorrat::neu(noetig as usize - 1);
                    assert!(
                        !zu_klein.reicht_fuer(auftraege, gz, gn),
                        "einer weniger als {noetig} darf nicht reichen ({auftraege}, {gz}/{gn})"
                    );
                }
            }
        }
    }

    #[test]
    fn ein_ausreichender_vorrat_wird_als_solcher_erkannt() {
        let v = KontrollsegmentVorrat::neu(2_000);
        assert!(v.reicht_fuer(100_000, 2, 100));
        assert_eq!(v.reichweite(2, 100), 100_000);
    }

    #[test]
    fn ohne_einschleusung_reicht_jeder_vorrat() {
        let v = KontrollsegmentVorrat::neu(1);
        assert!(v.reicht_fuer(usize::MAX, 0, 100));
        assert_eq!(v.reichweite(0, 100), usize::MAX);
    }

    #[test]
    fn eine_hoehere_rate_verlangt_einen_groesseren_vorrat() {
        // Der Zusammenhang, den jemand kennen muss, der γ erhöht: Die
        // Vorratsgröße muss mitwachsen, sonst macht eine schärfere
        // Kontrolle den Mechanismus schwächer statt stärker.
        let v = KontrollsegmentVorrat::neu(1_000);
        assert!(v.reicht_fuer(100_000, 1, 100), "bei 1 % reicht er");
        assert!(!v.reicht_fuer(100_000, 2, 100), "bei 2 % nicht mehr");
    }
}
