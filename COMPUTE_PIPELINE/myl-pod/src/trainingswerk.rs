//! Ein Trainingspod: derselbe Zuschnitt wie die Inferenzpipeline, aber
//! vorwärts **und** rückwärts (Whitepaper Kap. 7.2).
//!
//! # ⚑ Ein Trainingspod ist dieselbe Pipeline, zweimal
//!
//! Shard *j* hält die Ebenen `[von, bis)`, rechnet vorwärts und behält
//! seinen Mitschnitt. Der letzte Shard bildet den Verlustgradienten,
//! danach läuft der Gradient rückwärts durch dieselben Shards: Was
//! Shard *j+1* an seinem Eingang zurückgibt, ist der Ausgangsgradient
//! von Shard *j*.
//!
//! # ⚑ Was dieses Werk ist und was nicht
//!
//! **Es ist der Pod, nicht das Netz.** Alle Shards laufen hier in einem
//! Prozess, genau wie `Pipelinewerk` es für die Inferenz tut. Das ist
//! der Aufbau, gegen den der Testclient prüft und gegen den ein
//! Einzelknoten nachgerechnet wird.
//!
//! **Was fehlt, ist der Draht:** dieselben Nachrichten über
//! `PodMessage` zwischen echten Prozessen.
//!
//! ⚑ **Und den gibt es auch für die Inferenz nicht** (nachgesehen am
//! 2026-09-06). `PodMessage` ist ein fertiges Format mit Rundlauftest,
//! aber `Coordinator` ruft `shard.process` unmittelbar; `Ortsdienst`
//! bedient ganze Aufträge, nicht Nachrichten zwischen Shards. **Solange
//! das so ist, ist auch ein Inferenzpod ein Pod auf einer Maschine.**
//!
//! # ⚑ Der Zuschnitt ist derselbe wie bei der Inferenz, und das ist kein Zufall
//!
//! Ebenen gleichmässig, Rest nach hinten, `SHARDS` Stücke: dieselbe
//! Formel wie in [`crate::pipelinewerk`]. Liefen sie auseinander, wäre
//! ein Trainingsergebnis nicht mehr gegen dieselbe Pipeline
//! nachrechenbar, mit der abgeleitet wird, wie viel jeder Shard trägt.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::shardtraining::{
    rueckwaerts, vorwaerts, Shardfehler, Shardgewichte, Shardvorgaben,
};
use integer_llm_runtime::trainingsschleife::Trainingsvorgaben;

/// Wie viele Shards ein Trainingspod hat.
///
/// ⚑ **Dieselbe Zahl wie die Inferenzpipeline**, und ein Test hält
/// beide gegeneinander.
pub const SHARDS: usize = crate::pipelinewerk::SHARDS;

/// Was ein Trainingslauf über den Pod ergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Podtrainingsergebnis {
    /// Je Shard sein Δ-Commitment, in Shardreihenfolge.
    ///
    /// ⚑ **Das ist die Spur des Segments**, und sie ist genau das, was
    /// ein Prüfer auf Anforderung sehen will: Weichen zwei Pods
    /// voneinander ab, sagt der erste abweichende Eintrag, **welcher
    /// Shard** es war. Ohne sie liesse sich nur feststellen, dass einer
    /// von beiden falsch liegt.
    pub delta_je_shard: Vec<myl_types::hash::Hash>,
    /// Das Commitment des ganzen Pods über alle Shards.
    ///
    /// ⚑ **Gebildet mit `Trainingssegment::commitment_aus_spur`** und
    /// nicht hier: Der Prüfer rechnet die Shards nach und muss zum
    /// selben Wert kommen. Zwei Rechnungen für dieselbe Aussage liefen
    /// auseinander, dieselbe Lehre wie bei Fund 34.
    pub delta_commitment: myl_types::hash::Hash,
    /// Wie viele Gewichte sich im ganzen Pod bewegt haben.
    pub bewegte_gewichte: usize,
    /// Wie viele Gewichte der Pod insgesamt fortschreibt.
    pub gewichte_gesamt: usize,
    /// Die Logits der letzten Position vor dem ersten Schritt.
    pub logits_erst: Vec<i32>,
    /// Die Logits der letzten Position nach dem letzten Schritt.
    pub logits_letzt: Vec<i32>,
    /// Wenn ein Shard die Übertragungsform verlassen hat: welcher, bei
    /// welchem Schritt, auf welcher globalen Ebene.
    ///
    /// ⚑ **Gemeldet und nicht abgestürzt.** Ein Absturz ist von einem
    /// ausgefallenen Miner nicht zu unterscheiden; die Kette wartete
    /// dann auf ein Ergebnis, das nie kommt.
    pub aus_der_form: Option<(usize, u64, usize)>,
}

/// Ein Trainingspod über einer geladenen Pipeline.
pub struct Trainingswerk {
    modell: Arc<IntegerModel>,
    /// Je Shard seine Gewichte. **Sie überleben die Schritte**, sonst
    /// finge jeder Schritt wieder beim Artefakt an.
    gewichte: Vec<Shardgewichte>,
    grenzen: Vec<usize>,
}

impl Trainingswerk {
    /// Lädt das Modell und schneidet es in `SHARDS` Stücke.
    pub fn laden(artefakte: &Path) -> Result<Self, String> {
        let modell = load_model(artefakte).map_err(|e| format!("Modell laden: {e}"))?;
        Self::aus_modell(Arc::new(modell))
    }

    /// Dasselbe aus einem bereits geladenen Modell.
    pub fn aus_modell(modell: Arc<IntegerModel>) -> Result<Self, String> {
        let layer = modell.num_layers;
        // ⚑ Dieselbe Aufteilung wie `Pipelinewerk::laden`.
        let grenzen: Vec<usize> = (0..=SHARDS).map(|s| layer * s / SHARDS).collect();
        let mut gewichte = Vec::with_capacity(SHARDS);
        for s in 0..SHARDS {
            gewichte.push(
                Shardgewichte::aus_modell(&modell, grenzen[s], grenzen[s + 1])
                    .map_err(|e| format!("Shard {s}: {e}"))?,
            );
        }
        Ok(Self { modell, gewichte, grenzen })
    }

    /// Wie viele Shards.
    pub fn shardzahl(&self) -> u32 {
        SHARDS as u32
    }

    /// Die Ebenengrenzen der Shards.
    pub fn grenzen(&self) -> &[usize] {
        &self.grenzen
    }

    /// Die Gewichte eines Shards, zum Nachrechnen von aussen.
    pub fn gewichte_des_shards(&self, shard: usize) -> &Shardgewichte {
        &self.gewichte[shard]
    }

    /// Fährt ein Trainingssegment: `v.schritte` Schritte über den ganzen
    /// Pod.
    ///
    /// # ⚑ Die Reihenfolge, und warum sie festliegt
    ///
    /// Je Schritt:
    ///
    /// 1. **Einbettung**, dann vorwärts durch alle Shards, jeder behält
    ///    seinen Mitschnitt.
    /// 2. Der **letzte** Shard bildet aus seinem Ausgang die Logits und
    ///    daraus den Verlustgradienten gegen das Ziel.
    /// 3. **Rückwärts** durch die Shards, von hinten nach vorn; jeder
    ///    schreibt seine Gewichte fort und reicht `dL/dX` weiter.
    ///
    /// ⚑ **Schritt 2 gehört zum letzten Shard und nicht zum
    /// Koordinator.** Der Kopf des Modells sitzt dort, und wer den
    /// Verlust anderswo bildete, brauchte die Ausgabeschicht ein zweites
    /// Mal.
    pub fn segment_fahren(
        &mut self,
        v: &Trainingsvorgaben,
    ) -> Result<Podtrainingsergebnis, String> {
        if v.folge.is_empty() {
            return Err("Trainingswerk: die Folge ist leer".to_string());
        }
        if v.ziel >= self.modell.vocab_size {
            return Err(format!(
                "Trainingswerk: Zielwort {} liegt ausserhalb des Vokabulars ({})",
                v.ziel, self.modell.vocab_size
            ));
        }
        let m = Arc::clone(&self.modell);
        let letzte_pos = v.folge.len() - 1;
        let mut logits_erst = Vec::new();
        let mut logits_letzt = Vec::new();
        let mut aus_der_form = None;

        for s in 0..v.schritte {
            // 1. Einbettung und Vorwärtslauf durch alle Shards.
            let mut strom: Vec<Vec<i16>> =
                v.folge.iter().map(|t| m.embed_token(*t)).collect();
            let mut mitschnitte = Vec::with_capacity(SHARDS);
            for j in 0..SHARDS {
                let vg = self.vorgaben(j, s, v);
                let ms = vorwaerts(&m, &mut self.gewichte[j], &vg, &strom)
                    .map_err(|e| format!("Shard {j} vorwaerts: {e}"))?;
                strom = ms.ausgang.clone();
                mitschnitte.push(ms);
            }

            // 2. Der letzte Shard bildet den Verlustgradienten.
            let (logits, g_y) = integer_llm_runtime::trainingsschleife::gradient_vom_ziel(
                &m,
                &strom[letzte_pos],
                v,
            );
            if s == 0 {
                logits_erst = logits.clone();
            }
            logits_letzt = logits;
            let mut g: Vec<Vec<i32>> =
                (0..v.folge.len()).map(|_| vec![0i32; m.hidden_size]).collect();
            g[letzte_pos] = g_y;

            // 3. Rückwärts durch die Shards.
            for j in (0..SHARDS).rev() {
                let vg = self.vorgaben(j, s, v);
                let erg = rueckwaerts(&m, &mut self.gewichte[j], &vg, &mitschnitte[j], &g)
                    .map_err(|e| format!("Shard {j} rueckwaerts: {e}"))?;
                if aus_der_form.is_none() {
                    if let Some(ebene) = erg.aus_der_form {
                        aus_der_form = Some((j, s, ebene));
                    }
                }
                g = erg.eingang;
            }
            // ⚑ **Ein Segment, das aus der Form läuft, endet hier.**
            // Weitere Schritte sind verlorene Rechenzeit, und das
            // Ergebnis wäre ohnehin nicht ins Artefakt zurückzuschreiben.
            if aus_der_form.is_some() {
                break;
            }
        }

        // Die Abdrücke: je Shard einer, und der des Pods über deren
        // Verkettung. ⚑ Ein zweiter Rückwärtslauf nur für die Abdrücke
        // wäre teuer; sie entstehen aus dem gehaltenen Stand.
        let mut delta_je_shard = Vec::with_capacity(SHARDS);
        let mut bewegte = 0usize;
        let mut gesamt = 0usize;
        for j in 0..SHARDS {
            let (d, b, g) = self.abdruck_des_shards(j);
            delta_je_shard.push(d);
            bewegte += b;
            gesamt += g;
        }
        Ok(Podtrainingsergebnis {
            delta_commitment:
                myl_types::trainingssegment::Trainingssegment::commitment_aus_spur(
                    &delta_je_shard,
                ),
            delta_je_shard,
            bewegte_gewichte: bewegte,
            gewichte_gesamt: gesamt,
            logits_erst,
            logits_letzt,
            aus_der_form,
        })
    }

    fn vorgaben(&self, shard: usize, schritt: u64, v: &Trainingsvorgaben) -> Shardvorgaben {
        Shardvorgaben {
            von: self.grenzen[shard],
            bis: self.grenzen[shard + 1],
            schritt,
            // ⚑ **Die Lernrate kommt aus den Vorgaben des Segments**,
            // nicht aus dem Shard: Ein Shard, der sie wählte, wählte
            // seinen eigenen Arbeitsaufwand.
            lr_zaehler: 1,
            lr_nenner: v.lr_nenner,
        }
    }

    fn abdruck_des_shards(&self, shard: usize) -> (myl_types::hash::Hash, usize, usize) {
        let g = &self.gewichte[shard];
        // ⚑ **`Shardgewichte::deltas` und keine eigene Schleife**, damit
        // Pod und Pruefer dieselbe Ordnung nehmen. Bei einer
        // Gemischebene haengt sie an den Expertennummern.
        let deltas = g.deltas();
        let scheiben: Vec<&[i32]> = deltas.iter().map(|v| v.as_slice()).collect();
        let bewegte = deltas.iter().flat_map(|d| d.iter()).filter(|v| **v != 0).count();
        let hex = integer_llm_kernels::optimierer::trainingsabdruck(&scheiben);
        (hex_zu_hash(&hex), bewegte, g.gewichte_gesamt())
    }
}

/// Wandelt einen Abdruck aus `trainingsabdruck` in einen `Hash`.
///
/// ⚑ **Panik bei falscher Form.** Der Abdruck kommt aus dem eigenen
/// Kernel und ist immer 64 Hex-Zeichen; wäre er es nicht, wäre etwas
/// grundlegend anders als angenommen, und ein stillschweigend gefüllter
/// Nullhash verglände sich mit jedem anderen Fehlerfall.
fn hex_zu_hash(hex: &str) -> myl_types::hash::Hash {
    assert_eq!(hex.len(), 64, "ein Trainingsabdruck hat 64 Hex-Zeichen, nicht {}", hex.len());
    let mut b = [0u8; 32];
    for (i, paar) in hex.as_bytes().chunks(2).enumerate() {
        b[i] = u8::from_str_radix(std::str::from_utf8(paar).expect("Hex ist ASCII"), 16)
            .expect("trainingsabdruck liefert Hex");
    }
    myl_types::hash::Hash(b)
}

/// Der Fehlerfall eines Shards, für Aufrufer sichtbar gemacht.
pub type Werkfehler = Shardfehler;

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Der Zuschnitt muss der der Inferenz sein.** Liefen sie
    /// auseinander, wäre ein Trainingsergebnis nicht gegen dieselbe
    /// Pipeline nachrechenbar, mit der abgeleitet wird, wie viel jeder
    /// Shard trägt.
    #[test]
    fn der_zuschnitt_ist_der_der_inferenzpipeline() {
        assert_eq!(SHARDS, crate::pipelinewerk::SHARDS);
    }
}

/// Was ein Trainingsauftrag festlegt, bevor gerechnet wird.
///
/// ⚑ **Alles hier ist Bestellung, nichts ist Ergebnis.** Was der Lauf
/// hervorbringt, kommt aus [`Podtrainingsergebnis`] und kann von keinem
/// Aufrufer gesetzt werden. Genau darin liegt der Zweck dieses Typs.
#[derive(Debug, Clone)]
pub struct Segmentauftrag {
    pub id: myl_types::ids::SegmentId,
    pub modell_version: myl_types::ids::MerkleRoot,
    pub charge: myl_types::hash::Hash,
    pub startschritt: u64,
    pub schrittzahl: u32,
    /// Wie viele Folgen in jeden Schritt eingegangen sind.
    pub folgen: u32,
    pub lr_zaehler: i64,
    pub lr_nenner: i64,
    pub pod_pfad: Vec<myl_types::ids::MinerId>,
}

/// Baut das einzureichende Segment aus Auftrag und Ergebnis, **ohne
/// Signaturen**.
///
/// # ⚑ Warum es diese Funktion gibt, und was ohne sie geschah
///
/// `bewegte_gewichte` ist seit dem 2026-09-06 Teil des Segments und
/// damit Teil der Unterschrift (Fund 191). Die Zahl entsteht im
/// Rechenwerk und darf **nicht** von der Aufrufstelle kommen: Ein
/// Aufrufer, der sie tippt, tippt eine Behauptung, und der
/// Redundanzvergleich hätte zwei ehrliche Pods mit verschiedenen
/// Behauptungen als uneinig gemeldet.
///
/// **Hier kommt sie aus dem Ergebnis und sonst nirgendwoher.** Wer das
/// Segment anders zusammensetzt, kann sie vergessen; wer diese Funktion
/// benutzt, kann es nicht.
pub fn segment_aus_ergebnis(
    auftrag: &Segmentauftrag,
    erg: &Podtrainingsergebnis,
) -> myl_types::trainingssegment::Trainingssegment {
    myl_types::trainingssegment::Trainingssegment {
        id: auftrag.id,
        modell_version: auftrag.modell_version,
        charge: auftrag.charge,
        startschritt: auftrag.startschritt,
        schrittzahl: auftrag.schrittzahl,
        folgen: auftrag.folgen,
        lr_zaehler: auftrag.lr_zaehler,
        lr_nenner: auftrag.lr_nenner,
        delta_commitment: erg.delta_commitment,
        bewegte_gewichte: erg.bewegte_gewichte as u64,
        pod_pfad: auftrag.pod_pfad.clone(),
        signaturen: Vec::new(),
    }
}

/// Was ein Trainingsdurchgang über den Draht ergeben hat.
#[derive(Debug, Clone)]
pub struct Drahtergebnis {
    /// Je Shard sein Δ-Commitment, in Shardreihenfolge.
    pub delta_je_shard: Vec<myl_types::hash::Hash>,
    /// Das Commitment des ganzen Pods.
    pub delta_commitment: myl_types::hash::Hash,
    /// Wie viele Master sich bewegt haben, über alle Shards.
    pub bewegte_gewichte: usize,
    /// Wie viele Master der Pod fortschreibt.
    pub gewichte_gesamt: usize,
    /// Der erste Shard, der die Übertragungsform verlassen hat.
    pub aus_der_form: Option<(usize, u64)>,
}

/// Fährt **einen** Trainingsdurchgang über den Draht: vorwärts durch
/// alle Shards, rückwärts zurück, dann anwenden.
///
/// # ⚑ Warum die Reihenfolge nicht verhandelbar ist
///
/// Vorwärts läuft der Residualstrom von Shard 0 nach `n−1`, rückwärts
/// der Gradient von `n−1` nach 0. Jeder Shard hält seinen Mitschnitt
/// zwischen beiden Aufrufen; er geht **nicht** über den Draht, weil ihn
/// nur sein Shard braucht und er gross ist.
///
/// ⚑ **Angewandt wird erst am Ende, und zwar bei jedem Shard einmal.**
/// Zwischen Rückwärtspass und Anwenden liegt die Sammlung: Über wenige
/// Schritte streut das stochastische Runden stärker, als das
/// Gradientensignal wiegt (Fund 189). Wer nach jeder Folge anwendete,
/// bekäme über vierundzwanzig Ebenen einen Faktor tausend Streuung.
///
/// **Mehrere Folgen je Schritt** entstehen, indem der Aufrufer
/// `folge_ueber_den_draht` mehrfach ruft und erst danach
/// `anwenden_ueber_den_draht`.
pub fn folge_ueber_den_draht(
    weg: &dyn crate::shardweg::Shardweg,
    sitzung: u64,
    hidden: Vec<Vec<i16>>,
    g_aus: Vec<Vec<i32>>,
    lr_nenner: i64,
) -> Result<(), String> {
    use crate::shardweg::{Shardanfrage, Shardantwort};
    let n = weg.shardzahl();
    // --- Vorwärts, Shard 0 bis n−1 --------------------------------
    let mut strom = hidden;
    for s in 0..n {
        let anfrage = Shardanfrage::TrainVorwaerts {
            sitzung,
            hidden: strom,
            schritt: 0,
            lr_nenner,
        };
        strom = match weg.frage(s, &anfrage)? {
            Shardantwort::TrainAusgang(a) => a,
            Shardantwort::Fehler(e) => return Err(format!("Shard {s} vorwaerts: {e}")),
            andere => return Err(format!("Shard {s} antwortete mit {andere:?}")),
        };
    }
    // --- Rückwärts, Shard n−1 bis 0 -------------------------------
    let mut grad = g_aus;
    for s in (0..n).rev() {
        let anfrage = Shardanfrage::TrainRueckwaerts { sitzung, g_aus: grad, lr_nenner };
        grad = match weg.frage(s, &anfrage)? {
            Shardantwort::TrainEingang { eingang, .. } => eingang,
            Shardantwort::Fehler(e) => return Err(format!("Shard {s} rueckwaerts: {e}")),
            andere => return Err(format!("Shard {s} antwortete mit {andere:?}")),
        };
    }
    Ok(())
}

/// Wendet an, was alle Shards gesammelt haben, und bildet den Abdruck
/// des Pods.
///
/// ⚑ **Das Commitment entsteht aus der Spur und nicht neben ihr**, mit
/// `Trainingssegment::commitment_aus_spur`: Ein Prüfer, der die Shards
/// nachrechnet, muss zum selben Wert kommen.
pub fn anwenden_ueber_den_draht(
    weg: &dyn crate::shardweg::Shardweg,
    schritt: u64,
    lr_nenner: i64,
) -> Result<Drahtergebnis, String> {
    use crate::shardweg::{Shardanfrage, Shardantwort};
    let n = weg.shardzahl();
    let mut spur = Vec::with_capacity(n);
    let mut bewegte = 0usize;
    let mut gesamt = 0usize;
    let mut aus_der_form = None;
    for s in 0..n {
        let anfrage = Shardanfrage::TrainAnwenden { schritt, lr_nenner };
        match weg.frage(s, &anfrage)? {
            Shardantwort::TrainAngewandt {
                delta_commitment,
                bewegte: b,
                gesamt: g,
                aus_der_form: form,
            } => {
                spur.push(hex_zu_hash(&delta_commitment));
                bewegte += b as usize;
                gesamt += g as usize;
                if aus_der_form.is_none() {
                    if let Some(e) = form {
                        aus_der_form = Some((s, e));
                    }
                }
            }
            Shardantwort::Fehler(e) => return Err(format!("Shard {s} anwenden: {e}")),
            andere => return Err(format!("Shard {s} antwortete mit {andere:?}")),
        }
    }
    let gesamtabdruck =
        myl_types::trainingssegment::Trainingssegment::commitment_aus_spur(&spur);
    Ok(Drahtergebnis {
        delta_je_shard: spur,
        delta_commitment: gesamtabdruck,
        bewegte_gewichte: bewegte,
        gewichte_gesamt: gesamt,
        aus_der_form,
    })
}
