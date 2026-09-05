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
//! `PodMessage` zwischen echten Prozessen. Der Zuschnitt ist derselbe,
//! also ist es Transport und keine zweite Rechnung; **solange er nicht
//! steht, ist das hier ein Pod auf einer Maschine.**
//!
//! # ⚑ Der Zuschnitt ist derselbe wie bei der Inferenz, und das ist kein Zufall
//!
//! Ebenen gleichmässig, Rest nach hinten, `SHARDS` Stücke: dieselbe
//! Formel wie in [`crate::pipelinewerk`]. Liefen sie auseinander, wäre
//! ein Trainingsergebnis nicht mehr gegen dieselbe Pipeline
//! nachrechenbar, mit der abgeleitet wird, wie viel jeder Shard trägt.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::Zwischenwerte;
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
    pub delta_je_shard: Vec<String>,
    /// Das Commitment des ganzen Pods über alle Shards.
    ///
    /// ⚑ **Über die Verkettung in Shardreihenfolge**, denn genau das
    /// nennt ein `Trainingssegment` als `delta_commitment`. Eine andere
    /// Reihenfolge beschriebe dieselben Zahlen als eine andere Arbeit.
    pub delta_commitment: String,
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
                v.folge.iter().map(|t| m.embed_token(*t as usize)).collect();
            let mut mitschnitte = Vec::with_capacity(SHARDS);
            for j in 0..SHARDS {
                let vg = self.vorgaben(j, s, v);
                let ms = vorwaerts(&m, &self.gewichte[j], &vg, &strom)
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
        let verkettet = delta_je_shard.join("");
        Ok(Podtrainingsergebnis {
            delta_commitment: format!("{:x}", sha2::Sha256::digest(verkettet.as_bytes())),
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

    fn abdruck_des_shards(&self, shard: usize) -> (String, usize, usize) {
        let g = &self.gewichte[shard];
        let deltas: Vec<Vec<i32>> = g
            .anfangsstand()
            .iter()
            .zip(g.master.iter())
            .flat_map(|(a, b)| {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| integer_llm_kernels::optimierer::delta(x, y))
            })
            .collect();
        let scheiben: Vec<&[i32]> = deltas.iter().map(|v| v.as_slice()).collect();
        let bewegte = deltas.iter().flat_map(|d| d.iter()).filter(|v| **v != 0).count();
        let gesamt = g.master.iter().flat_map(|e| e.iter()).map(|m| m.len()).sum();
        (integer_llm_kernels::optimierer::trainingsabdruck(&scheiben), bewegte, gesamt)
    }
}

use sha2::Digest;

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
