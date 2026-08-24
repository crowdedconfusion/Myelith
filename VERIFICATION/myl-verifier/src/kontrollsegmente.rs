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
//! 2. **Vorratserneuerung** — erfüllt durch [`KontrollsegmentVorrat::erneuern`]:
//!    Übernahme abgeschlossener, per Stufe 2 vollständig geprüfter
//!    Echtsegmente, mit Verdrängung der ältesten.
//!
//! 3. **Kostenehrlichkeit** — γ ist ein Governance-Parameter
//!    (`myl_governance::Parameter::Kontrollsegmentanteil`) und geht als
//!    Overhead in die Kostenrechnung ein.
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
