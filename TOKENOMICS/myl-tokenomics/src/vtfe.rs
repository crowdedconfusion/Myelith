//! Zuschreibung der vTFE-Gutschrift auf die Shards eines Pods.
//!
//! **Was hier festgelegt wird, und warum es festgelegt gehört.** Bis zum
//! 2026-08-23 nahm diese Komponente vTFE als **Eingabe** entgegen
//! (`redundancy_normalized_weight`, `distribute.rs`); wie ein Shard zu
//! seinem Anteil kommt, stand nirgends. Solange jeder Pod dieselben vier
//! oder acht gleich großen Shards hatte, fiel das nicht auf. Der Entwurf
//! für **variable Knotenzahl je Pipeline** (COMPUTE_PIPELINE) bricht
//! genau diese Annahme: Zwei Pipelines mit verschiedenem `k` rechnen
//! dasselbe Segment, und ein Knoten mit sieben Layern darf nicht dasselbe
//! bekommen wie einer mit zweien.
//!
//! ## Die Regel
//!
//! Eine **vTFE-Einheit** ist 10⁻⁶ eines Token-Forward-Äquivalents, und
//! ein Token-Forward-Äquivalent ist der vollständige Vorwärtspass eines
//! Tokens durch das ganze Modell (Whitepaper Kap. 5). Ein Shard bekommt
//! davon den Anteil, den er **gerechnet** hat, gemessen an den
//! Multiplikations-Additionen der Gewichtsmatrizen, die ihm gehören.
//!
//! Der Anteil folgt damit aus `model_config.json`, und das ist über
//! `theta_v_hash` gebunden: Jeder Prüfer rechnet dieselbe Zahl nach, ohne
//! den Zustand einer Anfrage zu kennen.
//!
//! ## Warum nicht einfach Layer zählen
//!
//! Weil der LM-Kopf keine Layer ist, aber wie viele rechnet. Gemessen an
//! den beiden Modellen des Projekts:
//!
//! | Modell | eine Layer | LM-Kopf | Kopf in Layern | Anteil am Vorwärtspass |
//! |---|---|---|---|---|
//! | Qwen2.5-0,5B | 14,9 M MAC | 136,1 M MAC | **9,13** | 27,6 % |
//! | Qwen2.5-7B | 233,0 M MAC | 545,0 M MAC | **2,34** | 7,7 % |
//!
//! Eine reine Layer-Regel spricht dem letzten Shard bei 0,5B und acht
//! Shards **12,5 %** zu, während er **36,6 %** der Arbeit leistet, also
//! nicht einmal ein Drittel. Bei 7B wären es 10,7 % gegen 17,6 %. Der
//! Fehler wächst, je kleiner das Modell und je feiner der Zuschnitt, also
//! genau dorthin, wo der Entwurf für variable Knotenzahl hinwill.
//!
//! ## Was bewusst nicht mitzählt
//!
//! - **Die Attention-Scores.** Sie wachsen mit dem KV-Cache und hängen
//!   damit an der Kontextlänge der einzelnen Anfrage. Bei 0,5B übersteigen
//!   sie ab rund 4000 Token die Gewichtsarbeit einer Layer. Sie
//!   mitzuzählen wäre ehrlicher gegenüber der tatsächlichen Arbeit, machte
//!   die Gutschrift aber zu einer Größe **je Anfrage** statt je Modell:
//!   Jeder Prüfer müsste die Kontextlänge des Segments kennen und
//!   mitrechnen. Entschieden am 2026-08-23: bleibt draußen, gilt als
//!   benannte Näherung. Lange Kontexte sind dadurch unterbezahlt.
//! - **Das Embedding.** Ein Nachschlag in einer Tabelle, null
//!   Multiplikationen. Shard 0 bekommt dafür nichts, und das ist richtig.
//!   Die Tabelle vorzuhalten kostet Speicher, nicht Rechenzeit; ein
//!   Speicherentgelt wäre eine eigene Größe und keine vTFE.
//! - **RMSNorm, RoPE, SiLU, Residual-Additionen.** Elementweise über
//!   `hidden_size`, also drei Größenordnungen unter den Matrixprodukten
//!   derselben Layer.
//!
//! ## Rundung
//!
//! Abgerundet, wie überall in dieser Komponente: Es wird niemals mehr
//! gutgeschrieben, als geleistet wurde. Die Summe über einen
//! vollständigen Zuschnitt liegt deshalb um weniger als `k` Einheiten
//! unter der vollen Gutschrift, bei 10⁶ Einheiten je Token also um
//! weniger als ein Millionstel Token je Shard. Der Rest verfällt und wird
//! nicht verteilt.

use crate::VTFE_UNITS_PER_TFE;

/// Die Maße eines Modells, soweit sie die Rechenarbeit bestimmen.
///
/// Alle Felder stehen in `model_config.json` und sind über
/// `theta_v_hash` gebunden. Die Struktur trägt bewusst keine Werte, die
/// eine einzelne Anfrage betreffen (Kontextlänge, Batchgröße): Die
/// Gutschrift soll ohne Anfragezustand nachrechenbar sein.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModellProfil {
    pub hidden_size: u64,
    pub intermediate_size: u64,
    /// Zahl der Experten je Layer. **`0` heißt: dichtes Modell**, dann
    /// sind die beiden folgenden Felder bedeutungslos und
    /// [`ModellProfil::macs_je_layer`] rechnet wie zuvor.
    pub num_experts: u64,
    /// Wie viele Experten je Token feuern. Eine Konstante aus der
    /// Modellkonfiguration, **keine Größe je Anfrage**, und genau daran
    /// hängt, dass die Zuschreibung ohne Anfragezustand nachrechenbar
    /// bleibt.
    pub num_experts_per_tok: u64,
    /// Breite eines einzelnen Experten (`moe_intermediate_size`).
    pub moe_intermediate_size: u64,
    pub num_layers: u64,
    pub vocab_size: u64,
    pub num_heads: u64,
    pub num_kv_heads: u64,
    pub head_dim: u64,
}

impl ModellProfil {
    /// Multiplikations-Additionen der Gewichtsmatrizen **einer** Layer
    /// für **ein** Token.
    ///
    /// Sieben Matrizen: q, k, v, o aus der Attention, gate, up, down aus
    /// dem MLP. Bei Grouped-Query-Attention sind k und v schmaler als q,
    /// deshalb `num_kv_heads` statt `num_heads` (bei 0,5B ein Faktor 7).
    ///
    /// ## ⚑ Fund 60 (2026-08-25): Die Regel war dicht gerechnet
    ///
    /// Bis heute stand hier `gate = h·i`, `up = h·i`, `down = i·h`, also
    /// die volle Breite der MLP, plus **kein Router**. Bei einem
    /// Mixture-of-Experts-Modell rechnet je Token `top_k` Experten der Breite
    /// `moe_intermediate_size`, dazu die Router-Projektion.
    ///
    /// **Wo es tatsächlich bricht, ist nicht die Zahl, sondern ihre
    /// Herkunft.** `myl-pod::modell_profil` liest `intermediate_size` aus
    /// `gate_proj.rows()` der ersten Layer, und eine MoE-Layer hat kein
    /// `gate_proj`; sie hat 128 Experten, die je eines haben. Wer dort
    /// den ersten Experten nimmt, bekommt 768 statt 6144 und spricht dem
    /// Shard **ein Achtel** seiner Arbeit zu.
    ///
    /// ⚑ **Und ein Zufall, auf den sich niemand verlassen darf:** Bei
    /// Qwen3-30B-A3B ist `intermediate_size` 6144 und
    /// `top_k · moe_intermediate_size` = 8 · 768 = **ebenfalls 6144**.
    /// Die dichte Formel träfe hier also bis auf den Router-Term
    /// zufällig zu. Das ist eine Eigenschaft dieser einen Konfiguration
    /// und keine Regel; `test_der_zufall_bei_qwen3_30b_ist_keine_regel`
    /// hält beides fest, damit niemand die Übereinstimmung für die
    /// Rechtfertigung hält.
    ///
    /// Gefunden beim Bau des MoE-Modellpfads, an der Naht zwischen
    /// COMPUTE_PIPELINE und TOKENOMICS.
    ///
    /// **Was sich nicht ändert, und das ist der Punkt:** Die Zuschreibung
    /// bleibt **ohne Anfragezustand nachrechenbar**. `top_k` ist eine
    /// Konstante aus `model_config.json`, kein Wert je Anfrage. Welche
    /// Experten feuern, hängt am Token; **wie viele**, hängt es nicht.
    /// Ohne diese Eigenschaft wäre MoE mit der Festlegung vom 2026-08-23
    /// unvereinbar gewesen.
    pub fn macs_je_layer(&self) -> u128 {
        let h = self.hidden_size as u128;
        let q_out = self.num_heads as u128 * self.head_dim as u128;
        let kv_out = self.num_kv_heads as u128 * self.head_dim as u128;

        let q = h * q_out;
        let k = h * kv_out;
        let v = h * kv_out;
        let o = q_out * h;

        let ffn = if self.num_experts > 0 {
            let mi = self.moe_intermediate_size as u128;
            let router = h * self.num_experts as u128;
            let je_experte = 3 * h * mi;
            router + self.num_experts_per_tok as u128 * je_experte
        } else {
            let i = self.intermediate_size as u128;
            3 * h * i
        };

        q + k + v + o + ffn
    }

    /// Multiplikations-Additionen des LM-Kopfes für ein Token.
    pub fn macs_lm_kopf(&self) -> u128 {
        self.hidden_size as u128 * self.vocab_size as u128
    }

    /// Der vollständige Vorwärtspass eines Tokens: alle Layer plus Kopf.
    ///
    /// **Das ist der Nenner der Regel** und damit die Definition eines
    /// Token-Forward-Äquivalents in dieser Komponente.
    pub fn macs_vorwaerts(&self) -> u128 {
        self.num_layers as u128 * self.macs_je_layer() + self.macs_lm_kopf()
    }
}

/// Was ein einzelner Shard vom Modell hält.
///
/// `layer_start` einschließlich, `layer_end` ausschließlich, wie in
/// `myl_pod::shard::ShardNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardZuschnitt {
    pub layer_start: u64,
    pub layer_end: u64,
    pub hat_embedding: bool,
    pub hat_lm_kopf: bool,
}

impl ShardZuschnitt {
    /// Multiplikations-Additionen dieses Shards für ein Token.
    ///
    /// Das Embedding taucht hier nicht auf, obwohl `hat_embedding` im
    /// Zuschnitt steht: Ein Tabellennachschlag rechnet nicht. Das Feld
    /// bleibt, weil der Zuschnitt sonst nicht vollständig beschrieben
    /// wäre und ein späteres Speicherentgelt es brauchen wird.
    pub fn macs(&self, profil: &ModellProfil) -> u128 {
        let layer = self.layer_end.saturating_sub(self.layer_start) as u128;
        let mut m = layer * profil.macs_je_layer();
        if self.hat_lm_kopf {
            m += profil.macs_lm_kopf();
        }
        m
    }
}

/// Fehler der vTFE-Zuschreibung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtfeError {
    /// Ein Modell ohne Rechenarbeit: mindestens ein Maß ist null.
    ModellOhneArbeit,
    /// Der Zuschnitt greift über das Modell hinaus.
    ZuschnittAusserhalb,
}

impl std::fmt::Display for VtfeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModellOhneArbeit => write!(
                f,
                "Modellprofil ergibt null Rechenarbeit; eine Gutschrift daran zu messen \
                 hätte keine Bedeutung"
            ),
            Self::ZuschnittAusserhalb => write!(
                f,
                "Shard-Zuschnitt liegt außerhalb des Modells (layer_end > num_layers \
                 oder layer_start > layer_end)"
            ),
        }
    }
}

impl std::error::Error for VtfeError {}

/// vTFE-Gutschrift eines Shards für `tokens` erzeugte Token.
///
/// `floor(VTFE_UNITS_PER_TFE · macs_shard · tokens / macs_vorwaerts)`,
/// gerechnet in `u128`, abgerundet.
///
/// **Unabhängig vom Zuschnitt:** Zwei Pipelines mit verschiedenem `k`
/// verteilen dieselbe Summe, nur anders. Genau das ist die Bedingung
/// dafür, dass sie gegeneinander rechnen dürfen.
///
/// Die Redundanz-Normierung steckt **nicht** hier drin, sondern bleibt in
/// [`crate::redundancy_normalized_weight`]: Sie halbiert die Gutschrift,
/// weil jedes Segment von r = 2 Pods gerechnet wird, und das ist eine
/// Eigenschaft des Protokolls, nicht des Zuschnitts.
pub fn vtfe_gutschrift(
    profil: &ModellProfil,
    zuschnitt: &ShardZuschnitt,
    tokens: u64,
) -> Result<u64, VtfeError> {
    let gesamt = profil.macs_vorwaerts();
    if gesamt == 0 {
        return Err(VtfeError::ModellOhneArbeit);
    }
    if zuschnitt.layer_start > zuschnitt.layer_end || zuschnitt.layer_end > profil.num_layers {
        return Err(VtfeError::ZuschnittAusserhalb);
    }
    let anteil = zuschnitt.macs(profil);
    let einheiten = (VTFE_UNITS_PER_TFE as u128)
        .saturating_mul(anteil)
        .saturating_mul(tokens as u128)
        / gesamt;
    Ok(einheiten.min(u64::MAX as u128) as u64)
}

/// Die volle Gutschrift für `tokens` Token, also die Summe, die ein
/// vollständiger Zuschnitt untereinander aufteilt.
pub fn vtfe_voll(tokens: u64) -> u64 {
    VTFE_UNITS_PER_TFE.saturating_mul(tokens)
}

#[cfg(test)]
mod moe_tests {
    use super::*;

    /// Qwen3-30B-A3B, gegen die echte `config.json` (Revision `ad44e777`).
    fn qwen3_30b_a3b() -> ModellProfil {
        ModellProfil {
            hidden_size: 2048,
            intermediate_size: 6144,
            num_experts: 128,
            num_experts_per_tok: 8,
            moe_intermediate_size: 768,
            num_layers: 48,
            vocab_size: 151_936,
            num_heads: 32,
            num_kv_heads: 4,
            head_dim: 128,
        }
    }

    #[test]
    fn moe_arbeit_je_layer_von_hand_nachgerechnet() {
        let p = qwen3_30b_a3b();
        // q 2048·4096, k 2048·512, v 2048·512, o 4096·2048
        let attn: u128 = 8_388_608 + 1_048_576 + 1_048_576 + 8_388_608;
        // Router 2048·128, dann 8 Experten à 3·2048·768
        let ffn: u128 = 262_144 + 8 * 3 * 2048 * 768;
        assert_eq!(p.macs_je_layer(), attn + ffn);
    }

    /// ⚑ **Der Zufall, den niemand für eine Regel halten darf.**
    ///
    /// Bei dieser Konfiguration gilt `top_k · moe_intermediate_size ==
    /// intermediate_size`, also 8 · 768 == 6144. Die dichte Formel liegt
    /// damit **bis auf den Router-Term** richtig. Das ist eine
    /// Eigenschaft dieses Modells, keine Eigenschaft von MoE.
    #[test]
    fn test_der_zufall_bei_qwen3_30b_ist_keine_regel() {
        let p = qwen3_30b_a3b();
        assert_eq!(
            p.num_experts_per_tok * p.moe_intermediate_size,
            p.intermediate_size,
            "Voraussetzung des Tests: bei diesem Modell fallen beide zusammen"
        );

        // Dieselbe Konfiguration, aber als dicht gelesen.
        let dicht = ModellProfil { num_experts: 0, ..p };
        let unterschied = p.macs_je_layer() - dicht.macs_je_layer();
        assert_eq!(
            unterschied,
            2048 * 128,
            "genau der Router-Term, mehr trennt die beiden hier nicht"
        );

        // Und ein Modell, bei dem der Zufall nicht gilt: halb so viele
        // gefeuerte Experten bei gleicher Breite. Die dichte Formel
        // ueberschaetzt dann um mehr als ein Drittel.
        let anders = ModellProfil { num_experts_per_tok: 4, ..p };
        assert!(
            dicht.macs_je_layer() * 100 > anders.macs_je_layer() * 140,
            "ohne den Zufall ueberschaetzt die dichte Formel deutlich: {} gegen {}",
            dicht.macs_je_layer(),
            anders.macs_je_layer()
        );
    }

    /// Ein Achtel statt des Ganzen: was passiert, wenn jemand
    /// `intermediate_size` aus **einem einzelnen Experten** liest, also
    /// aus `gate_proj.rows()` des ersten Experten.
    ///
    /// Das ist keine ausgedachte Fehlbedienung: Genau so liest
    /// `myl-pod::modell_profil` heute die Zahl, und bei einer MoE-Layer
    /// ist der erste Experte das Naechstliegende, was dort steht.
    #[test]
    fn ein_experte_als_intermediate_size_unterschlaegt_sieben_achtel() {
        let p = qwen3_30b_a3b();
        let falsch = ModellProfil {
            num_experts: 0,
            intermediate_size: p.moe_intermediate_size,
            ..p
        };
        let attn: u128 = 8_388_608 + 1_048_576 + 1_048_576 + 8_388_608;
        let ein_experte: u128 = 3 * 2048 * 768;

        assert_eq!(
            falsch.macs_je_layer(),
            attn + ein_experte,
            "die falsche Lesart rechnet genau einen Experten"
        );
        assert_eq!(
            p.macs_je_layer() - attn - 2048 * 128,
            8 * ein_experte,
            "richtig sind acht, plus der Router"
        );
    }

    #[test]
    fn dichte_modelle_rechnen_unveraendert() {
        // Qwen2.5-0,5B, wie bisher.
        let p = ModellProfil {
            hidden_size: 896,
            intermediate_size: 4864,
            num_experts: 0,
            num_experts_per_tok: 0,
            moe_intermediate_size: 0,
            num_layers: 24,
            vocab_size: 151_936,
            num_heads: 14,
            num_kv_heads: 2,
            head_dim: 64,
        };
        let q_out: u128 = 14 * 64;
        let kv_out: u128 = 2 * 64;
        let erwartet = 896 * q_out + 896 * kv_out + 896 * kv_out + q_out * 896
            + 3 * 896 * 4864;
        assert_eq!(p.macs_je_layer(), erwartet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qwen05b() -> ModellProfil {
        ModellProfil {
            hidden_size: 896,
            intermediate_size: 4864,
            num_experts: 0,
            num_experts_per_tok: 0,
            moe_intermediate_size: 0,
            num_layers: 24,
            vocab_size: 151_936,
            num_heads: 14,
            num_kv_heads: 2,
            head_dim: 64,
        }
    }

    fn qwen7b() -> ModellProfil {
        ModellProfil {
            hidden_size: 3584,
            intermediate_size: 18_944,
            num_experts: 0,
            num_experts_per_tok: 0,
            moe_intermediate_size: 0,
            num_layers: 28,
            vocab_size: 152_064,
            num_heads: 28,
            num_kv_heads: 4,
            head_dim: 128,
        }
    }

    /// Gleichmäßiger Zuschnitt in `k` Shards, Rest auf die vorderen.
    fn zuschnitt(profil: &ModellProfil, k: u64) -> Vec<ShardZuschnitt> {
        let l = profil.num_layers;
        let basis = l / k;
        let rest = l % k;
        let mut grenzen = vec![0u64];
        for s in 0..k {
            let letzte = *grenzen.last().unwrap();
            grenzen.push(letzte + basis + u64::from(s < rest));
        }
        (0..k as usize)
            .map(|s| ShardZuschnitt {
                layer_start: grenzen[s],
                layer_end: grenzen[s + 1],
                hat_embedding: s == 0,
                hat_lm_kopf: s + 1 == k as usize,
            })
            .collect()
    }

    /// Die Zahlen aus dem Modulkopf, damit sie nicht bloß dastehen.
    #[test]
    fn der_lm_kopf_wiegt_mehrere_layer() {
        let p = qwen05b();
        let in_layern = p.macs_lm_kopf() as f64 / p.macs_je_layer() as f64;
        assert!(
            (9.0..9.3).contains(&in_layern),
            "0,5B: LM-Kopf entspricht {:.2} Layern",
            in_layern
        );

        let p = qwen7b();
        let in_layern = p.macs_lm_kopf() as f64 / p.macs_je_layer() as f64;
        assert!(
            (2.2..2.5).contains(&in_layern),
            "7B: LM-Kopf entspricht {:.2} Layern",
            in_layern
        );
    }

    /// **Der Grund für diese Regel.** Eine reine Layer-Zählung gäbe dem
    /// letzten Shard bei 0,5B und acht Shards 12,5 %; geleistet hat er
    /// über ein Drittel.
    #[test]
    fn reine_layerzaehlung_waere_schief() {
        let p = qwen05b();
        let shards = zuschnitt(&p, 8);
        let letzter = shards.last().unwrap();

        let nach_layern = (letzter.layer_end - letzter.layer_start) as f64 / p.num_layers as f64;
        let nach_arbeit = letzter.macs(&p) as f64 / p.macs_vorwaerts() as f64;

        assert!((nach_layern - 0.125).abs() < 1e-9);
        assert!(
            nach_arbeit > 0.36,
            "letzter Shard leistet {:.3}, Layer-Regel gäbe {:.3}",
            nach_arbeit,
            nach_layern
        );
        assert!(nach_arbeit / nach_layern > 2.8);
    }

    /// Ein vollständiger Zuschnitt verteilt die volle Gutschrift, bis auf
    /// die Abrundung. Der Rest verfällt, er wird nie erfunden.
    #[test]
    fn ein_vollstaendiger_zuschnitt_verteilt_hoechstens_das_ganze() {
        for profil in [qwen05b(), qwen7b()] {
            for k in [1u64, 2, 3, 4, 6, 7, 8, 12, 24] {
                if k > profil.num_layers {
                    continue;
                }
                for tokens in [1u64, 8, 1000] {
                    let summe: u64 = zuschnitt(&profil, k)
                        .iter()
                        .map(|z| vtfe_gutschrift(&profil, z, tokens).unwrap())
                        .sum();
                    let voll = vtfe_voll(tokens);
                    assert!(summe <= voll, "k={k}, tokens={tokens}: {summe} > {voll}");
                    assert!(
                        voll - summe < k,
                        "k={k}, tokens={tokens}: Abrundungsverlust {} ist zu groß",
                        voll - summe
                    );
                }
            }
        }
    }

    /// **Die Bedingung für variable Knotenzahl:** Zwei Pipelines mit
    /// verschiedenem `k` verteilen dieselbe Summe, nur anders. Ohne diese
    /// Eigenschaft wäre die gemischte Paarung aus dem
    /// COMPUTE_PIPELINE-Entwurf ökonomisch nicht neutral.
    #[test]
    fn verschiedene_zuschnitte_verteilen_dieselbe_summe() {
        let p = qwen7b();
        let tokens = 500u64;
        let summe = |k: u64| -> u64 {
            zuschnitt(&p, k)
                .iter()
                .map(|z| vtfe_gutschrift(&p, z, tokens).unwrap())
                .sum()
        };
        let referenz = summe(4);
        for k in [2u64, 7, 8, 14, 28] {
            let abweichung = referenz.abs_diff(summe(k));
            assert!(
                abweichung < 28,
                "k={k} weicht um {abweichung} Einheiten ab, mehr als die Abrundung erklärt"
            );
        }
    }

    /// Ein Shard mit mehr Layern bekommt mehr, und zwar streng.
    #[test]
    fn mehr_layer_ergeben_mehr_gutschrift() {
        let p = qwen7b();
        let zwei = ShardZuschnitt {
            layer_start: 0,
            layer_end: 2,
            hat_embedding: true,
            hat_lm_kopf: false,
        };
        let sieben = ShardZuschnitt {
            layer_start: 2,
            layer_end: 9,
            hat_embedding: false,
            hat_lm_kopf: false,
        };
        let a = vtfe_gutschrift(&p, &zwei, 100).unwrap();
        let b = vtfe_gutschrift(&p, &sieben, 100).unwrap();
        assert!(b > a);
        // 7 gegen 2 Layer, gleiche Größe je Layer: genau Faktor 3,5.
        assert_eq!(b / a, 3);
        assert!((b as f64 / a as f64 - 3.5).abs() < 0.01);
    }

    /// Das Embedding rechnet nicht, also zahlt es nicht.
    #[test]
    fn embedding_allein_ergibt_null() {
        let p = qwen05b();
        let nur_embedding = ShardZuschnitt {
            layer_start: 0,
            layer_end: 0,
            hat_embedding: true,
            hat_lm_kopf: false,
        };
        assert_eq!(vtfe_gutschrift(&p, &nur_embedding, 1000).unwrap(), 0);
    }

    #[test]
    fn zuschnitt_ausserhalb_wird_abgelehnt() {
        let p = qwen05b();
        let zu_weit = ShardZuschnitt {
            layer_start: 0,
            layer_end: 25,
            hat_embedding: true,
            hat_lm_kopf: true,
        };
        assert_eq!(
            vtfe_gutschrift(&p, &zu_weit, 1),
            Err(VtfeError::ZuschnittAusserhalb)
        );
        let verdreht = ShardZuschnitt {
            layer_start: 5,
            layer_end: 2,
            hat_embedding: false,
            hat_lm_kopf: false,
        };
        assert_eq!(
            vtfe_gutschrift(&p, &verdreht, 1),
            Err(VtfeError::ZuschnittAusserhalb)
        );
    }

    #[test]
    fn modell_ohne_arbeit_wird_abgelehnt() {
        let leer = ModellProfil {
            hidden_size: 0,
            intermediate_size: 0,
            num_experts: 0,
            num_experts_per_tok: 0,
            moe_intermediate_size: 0,
            num_layers: 0,
            vocab_size: 0,
            num_heads: 0,
            num_kv_heads: 0,
            head_dim: 0,
        };
        let z = ShardZuschnitt {
            layer_start: 0,
            layer_end: 0,
            hat_embedding: true,
            hat_lm_kopf: true,
        };
        assert_eq!(vtfe_gutschrift(&leer, &z, 1), Err(VtfeError::ModellOhneArbeit));
    }

    /// Grouped-Query-Attention muss durchschlagen: Bei 0,5B sind k und v
    /// siebenmal schmaler als q. Wer das übersieht, überschätzt die
    /// Attention-Gewichte um rund ein Drittel.
    #[test]
    fn gqa_schlaegt_durch() {
        let p = qwen05b();
        let mut ohne_gqa = p;
        ohne_gqa.num_kv_heads = p.num_heads;
        assert!(ohne_gqa.macs_je_layer() > p.macs_je_layer());
    }
}
