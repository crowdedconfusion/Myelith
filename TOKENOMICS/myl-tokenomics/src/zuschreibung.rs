//! Zuschreibung: welcher Miner welchen Anteil der Pod-Arbeit hatte.
//!
//! # ⚑ Abgeleitet und nicht erklärt (Festlegung des Projektinhabers, 2026-08-31)
//!
//! Die Frage lautete, ob das PoI-Bündel ein Feld „Anteil je Miner"
//! bekommt oder ob der Anteil aus Vorhandenem folgt. Entschieden ist
//! das Zweite, und der Grund ist nicht Sparsamkeit am Drahtformat:
//!
//! **Ein erklärtes Feld ist eine Behauptung.** Ein Pod, der seinen
//! Ertrag umverteilen will, trüge dort ein, was ihm passt, und niemand
//! außerhalb des Pods könnte widersprechen: Wer welchen Zuschnitt
//! gerechnet hat, sieht von außen niemand. Was **abgeleitet** wird, kann
//! niemand falsch melden. Der Scheduler weist die Positionen
//! deterministisch zu, jede Position trägt genau einen Miner
//! (`myl_scheduler::Shard`), und was ein Zuschnitt an Arbeit kostet,
//! steht in der Modellkonfiguration, die über `theta_v_hash` gebunden
//! ist.
//!
//! Dasselbe Muster wie bei der Speicher-Stichprobe, deren Seed
//! hergeleitet und nicht mitgeschickt wird, und wie bei Ethereum, das
//! Attestierungs-Belohnungen aus dem Beacon-Zustand rechnet statt sie
//! sich melden zu lassen.
//!
//! # Was hier **nicht** geschieht
//!
//! ⚑ **Die Redundanz-Normierung bleibt draußen**, und das ist kein
//! Vergessen. [`crate::redundancy_normalized_weight`] halbiert eine
//! vTFE-Gutschrift, weil jedes Segment von r = 2 Pods gerechnet wird.
//! Als **Gewicht** in [`crate::split_proportional`] ist eine Halbierung
//! aller Werte wirkungslos, denn dort wird ohnehin durch die
//! Gewichtssumme geteilt; sie würde nur bei ungeraden Werten eine
//! Einheit verschlucken. Die Normierung gehört dorthin, wo vTFE eine
//! **absolute** Größe ist, nicht in eine Verhältnisrechnung.
//!
//! Ebenso wenig wird hier ein Auszahlungskonto gesucht. Diese Datei
//! kennt nur Arbeit; wer sie in Geld übersetzt, steht in
//! [`crate::ausschuettung`].

use std::collections::{BTreeMap, BTreeSet};

use myl_types::arbeitsverteilung::Arbeitsverteilung;
use myl_types::ids::MinerId;

use crate::vtfe::{vtfe_gutschrift, ModellProfil, ShardZuschnitt, VtfeError};

/// Eine besetzte Shard-Position: der Miner und der Zuschnitt, den er hielt.
///
/// Entspricht `myl_scheduler::Shard`, ohne dessen Registrierungsdaten.
/// Die Übersetzung geschieht beim Aufrufer: `myl-tokenomics` hängt
/// nicht am Scheduler, und das soll so bleiben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Podposition {
    /// Wer auf dieser Position saß.
    pub miner: MinerId,
    /// Was er vom Modell hielt.
    pub zuschnitt: ShardZuschnitt,
}

/// Was ein Pod in einer Epoche geleistet hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Podleistung {
    /// Die besetzten Positionen, in Indexreihenfolge.
    pub positionen: Vec<Podposition>,
    /// Die Reserve. Sie stand bereit und rechnete nicht.
    pub reserve: Vec<MinerId>,
    /// Wie viele Segmente dieser Pod erzeugt hat.
    pub segmente: u64,
}

/// Was beim Ableiten schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZuschreibungFehler {
    /// Ein Zuschnitt passt nicht zum Modell.
    ZuschnittUnbrauchbar {
        /// Index des Pods in der Eingabe.
        pod: usize,
        /// Index der Position innerhalb des Pods.
        position: usize,
        /// Was `vtfe_gutschrift` beanstandet hat.
        grund: VtfeError,
    },
    /// Ein Pod ohne besetzte Position. Ein halber Pod ist kein Pod.
    PodOhnePositionen {
        /// Index des Pods in der Eingabe.
        pod: usize,
    },
    /// Derselbe Miner steht zweimal in demselben Pod.
    ///
    /// ⚑ **Nicht stillschweigend zusammenführen.** Wer zweimal in einem
    /// Pod sitzt, bekäme zwei Positionen bezahlt und hielte doch nur
    /// eine Maschine bereit; die Redundanz, für die der Pod gebaut ist,
    /// wäre dahin. Am 2026-08-30 sah `pods_are_disjoint` die Reserve
    /// nicht, also ist diese Lage schon einmal durchgerutscht.
    MinerZweimalImPod {
        /// Index des Pods in der Eingabe.
        pod: usize,
    },
}

impl std::fmt::Display for ZuschreibungFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZuschnittUnbrauchbar {
                pod,
                position,
                grund,
            } => write!(
                f,
                "Pod {}, Position {}: Zuschnitt unbrauchbar ({})",
                pod, position, grund
            ),
            Self::PodOhnePositionen { pod } => {
                write!(f, "Pod {} hat keine besetzte Position", pod)
            }
            Self::MinerZweimalImPod { pod } => {
                write!(f, "Pod {}: derselbe Miner steht zweimal darin", pod)
            }
        }
    }
}

impl std::error::Error for ZuschreibungFehler {}

/// Das Ergebnis der Ableitung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Zuschreibung {
    /// vTFE-Einheiten je Miner, über alle Pods aufsummiert.
    ///
    /// Ein Miner kann in mehreren Pods sitzen; seine Gutschriften
    /// addieren sich.
    pub je_miner: BTreeMap<MinerId, u64>,
    /// Wer nur Reserve war und in keinem Pod eine Position hielt.
    ///
    /// ⚑ **Gehört ins Ergebnis und nicht ins Schweigen.** Eine
    /// Zuschreibung, die die Reserve weglässt, sieht aus wie eine, in
    /// der es keine gab; die Betroffenen warten auf eine Gutschrift, die
    /// nie kommt, und niemand kann sagen, warum. Dieselbe Lehre wie bei
    /// `Zuteilung::ohne_pod` im Scheduler.
    pub reserve_ohne_anteil: Vec<MinerId>,
}

impl Zuschreibung {
    /// Summe aller Gutschriften.
    pub fn summe(&self) -> u128 {
        self.je_miner.values().map(|v| *v as u128).sum()
    }
}

/// Leitet die vTFE-Gutschrift je Miner aus den Pod-Besetzungen ab.
///
/// Jede besetzte Position bekommt [`vtfe_gutschrift`] für ihren
/// Zuschnitt und die Segmentzahl ihres Pods. Die Reserve bekommt nichts;
/// sie hat nichts gerechnet.
///
/// # Warum die Reserve leer ausgeht
///
/// Sie hält Gewichte vor und steht bereit, und beides kostet. Bezahlt
/// wird hier aber **erzeugte Arbeit**, gemessen in
/// Multiplikations-Additionen. Bereitschaft ist eine andere Größe und
/// hätte eine eigene Quelle: Wer sie vergüten will, braucht einen
/// Nachweis, dass die Bereitschaft bestand, und den gibt es noch nicht.
/// Sie hier mitzubezahlen hieße, eine unbelegte Größe aus einer
/// belegten zu erfinden.
pub fn zuschreiben(
    profil: &ModellProfil,
    leistungen: &[Podleistung],
) -> Result<Zuschreibung, ZuschreibungFehler> {
    let mut je_miner: BTreeMap<MinerId, u64> = BTreeMap::new();
    let mut reserve: BTreeSet<MinerId> = BTreeSet::new();

    for (pi, pod) in leistungen.iter().enumerate() {
        if pod.positionen.is_empty() {
            return Err(ZuschreibungFehler::PodOhnePositionen { pod: pi });
        }
        let mut gesehen: BTreeSet<MinerId> = BTreeSet::new();
        for m in pod.positionen.iter().map(|p| p.miner).chain(pod.reserve.iter().copied()) {
            if !gesehen.insert(m) {
                return Err(ZuschreibungFehler::MinerZweimalImPod { pod: pi });
            }
        }
        for (qi, pos) in pod.positionen.iter().enumerate() {
            let anteil = vtfe_gutschrift(profil, &pos.zuschnitt, pod.segmente).map_err(|e| {
                ZuschreibungFehler::ZuschnittUnbrauchbar {
                    pod: pi,
                    position: qi,
                    grund: e,
                }
            })?;
            let eintrag = je_miner.entry(pos.miner).or_insert(0);
            *eintrag = eintrag.saturating_add(anteil);
        }
        reserve.extend(pod.reserve.iter().copied());
    }

    // Wer irgendwo eine Position hielt, ist keine reine Reserve.
    let reserve_ohne_anteil = reserve
        .into_iter()
        .filter(|m| !je_miner.contains_key(m))
        .collect();

    Ok(Zuschreibung {
        je_miner,
        reserve_ohne_anteil,
    })
}

/// Was ein Pod in einer Epoche abgerechnet hat, wie es aus einem
/// bestätigten Bündel folgt.
///
/// # ⚑ Der Unterschied zu [`Podleistung`]
///
/// [`Podleistung`] trägt **Zuschnitte** und eine Segmentzahl; daraus
/// rechnet [`zuschreiben`] die Gutschriften mit dem Modellprofil. Das
/// ist der Weg des **Prüfers**, der θ_v hat.
///
/// Diese hier trägt **Positionsnummern** und die vTFE des Pods; die
/// Gewichtung liefert die [`Arbeitsverteilung`] aus dem Konsenszustand.
/// Das ist der Weg der **Kette**, die kein Modellprofil hat und keines
/// haben soll.
///
/// **Beide müssen dasselbe ergeben**, und ein Test hält das fest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Podabrechnung {
    /// Die besetzten Positionen: wer, und auf welcher Nummer.
    pub positionen: Vec<(MinerId, u32)>,
    /// Die Reserve. Sie stand bereit und rechnete nicht.
    pub reserve: Vec<MinerId>,
    /// Was der Pod insgesamt beansprucht.
    pub vtfe_pod: u64,
}

/// Was beim Abrechnen schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abrechnungsfehler {
    /// Ein Pod ohne besetzte Position.
    PodOhnePositionen {
        /// Index des Pods in der Eingabe.
        pod: usize,
    },
    /// Derselbe Miner steht zweimal in demselben Pod.
    MinerZweimalImPod {
        /// Index des Pods in der Eingabe.
        pod: usize,
    },
    /// Eine Positionsnummer, die die Verteilung nicht kennt.
    ///
    /// ⚑ **Ein Fehler und keine Null.** Wer eine Position abrechnet, die
    /// es im gültigen Zuschnitt nicht gibt, rechnet über etwas anderes
    /// ab als die Kette; sie stillschweigend mit null zu gewichten
    /// verstiege den Widerspruch, statt ihn zu melden.
    PositionUnbekannt {
        /// Index des Pods in der Eingabe.
        pod: usize,
        /// Die verlangte Positionsnummer.
        position: u32,
        /// Wie viele es gibt.
        positionen: usize,
    },
}

impl std::fmt::Display for Abrechnungsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PodOhnePositionen { pod } => {
                write!(f, "Pod {pod} hat keine besetzte Position")
            }
            Self::MinerZweimalImPod { pod } => {
                write!(f, "Pod {pod}: derselbe Miner steht zweimal darin")
            }
            Self::PositionUnbekannt {
                pod,
                position,
                positionen,
            } => write!(
                f,
                "Pod {pod}: Position {position} gibt es nicht, die Verteilung kennt {positionen}"
            ),
        }
    }
}

impl std::error::Error for Abrechnungsfehler {}

/// Leitet die Gutschrift je Miner aus Bündeln und Arbeitsverteilung ab.
///
/// **Der Weg der Kette**, ohne Modellprofil: Die vTFE eines Pods wird
/// nach den Gewichten seiner Positionen aufgeteilt, exakt, sodass die
/// Summe je Pod erhalten bleibt.
///
/// Die Reserve bekommt nichts und wird genannt, wie in [`zuschreiben`].
///
/// # ⚑ Unbesetzte Positionen lassen ihren Anteil verfallen
///
/// Aufgeteilt wird über **alle** Positionen der Verteilung, nicht nur
/// über die besetzten. Fehlt eine Position, verfällt ihr Anteil.
///
/// **Die Alternative wäre falsch:** Verteilte man nur auf die
/// Besetzten, bekämen sie Geld für Layer, die sie nicht gerechnet
/// haben. Ein Pod, dem eine Position fehlt, hat weniger geleistet, und
/// der Verfall bildet genau das ab. Was verfällt, wird nicht
/// umverteilt und nicht geprägt; es entsteht schlicht nicht.
pub fn zuschreiben_aus_abrechnung(
    verteilung: &Arbeitsverteilung,
    abrechnungen: &[Podabrechnung],
) -> Result<Zuschreibung, Abrechnungsfehler> {
    let mut je_miner: BTreeMap<MinerId, u64> = BTreeMap::new();
    let mut reserve: BTreeSet<MinerId> = BTreeSet::new();

    for (pi, pod) in abrechnungen.iter().enumerate() {
        if pod.positionen.is_empty() {
            return Err(Abrechnungsfehler::PodOhnePositionen { pod: pi });
        }
        let mut gesehen: BTreeSet<MinerId> = BTreeSet::new();
        for m in pod
            .positionen
            .iter()
            .map(|(m, _)| *m)
            .chain(pod.reserve.iter().copied())
        {
            if !gesehen.insert(m) {
                return Err(Abrechnungsfehler::MinerZweimalImPod { pod: pi });
            }
        }
        for (_, nummer) in &pod.positionen {
            if *nummer as usize >= verteilung.positionen() {
                return Err(Abrechnungsfehler::PositionUnbekannt {
                    pod: pi,
                    position: *nummer,
                    positionen: verteilung.positionen(),
                });
            }
        }
        let anteile = verteilung.aufteilen(pod.vtfe_pod);
        for (miner, nummer) in &pod.positionen {
            let eintrag = je_miner.entry(*miner).or_insert(0);
            *eintrag = eintrag.saturating_add(anteile[*nummer as usize]);
        }
        reserve.extend(pod.reserve.iter().copied());
    }

    let reserve_ohne_anteil = reserve
        .into_iter()
        .filter(|m| !je_miner.contains_key(m))
        .collect();

    Ok(Zuschreibung {
        je_miner,
        reserve_ohne_anteil,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vtfe::vtfe_voll;

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

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Ein Zuschnitt über die Layer `[a, b)`, ohne Embedding und Kopf.
    fn mitte(a: u64, b: u64) -> ShardZuschnitt {
        ShardZuschnitt {
            layer_start: a,
            layer_end: b,
            hat_embedding: false,
            hat_lm_kopf: false,
        }
    }

    /// Ein vollständiger Pod mit vier Positionen: der letzte trägt den
    /// LM-Kopf, der erste das Embedding.
    fn voller_pod(segmente: u64, ab: u8) -> Podleistung {
        Podleistung {
            positionen: vec![
                Podposition {
                    miner: miner(ab),
                    zuschnitt: ShardZuschnitt {
                        layer_start: 0,
                        layer_end: 6,
                        hat_embedding: true,
                        hat_lm_kopf: false,
                    },
                },
                Podposition {
                    miner: miner(ab + 1),
                    zuschnitt: mitte(6, 12),
                },
                Podposition {
                    miner: miner(ab + 2),
                    zuschnitt: mitte(12, 18),
                },
                Podposition {
                    miner: miner(ab + 3),
                    zuschnitt: ShardZuschnitt {
                        layer_start: 18,
                        layer_end: 24,
                        hat_embedding: false,
                        hat_lm_kopf: true,
                    },
                },
            ],
            reserve: vec![miner(ab + 4), miner(ab + 5)],
            segmente,
        }
    }

    /// Was ein vollständiger Pod untereinander verteilt, ist die volle
    /// Gutschrift, bis auf die Abwärtsrundung je Position.
    #[test]
    fn ein_vollstaendiger_pod_verteilt_die_ganze_arbeit() {
        let z = zuschreiben(&qwen05b(), &[voller_pod(1_000, 1)]).expect("Zuschreibung");
        let voll = vtfe_voll(1_000) as u128;
        let summe = z.summe();
        assert!(summe <= voll, "mehr verteilt als es gibt: {summe} > {voll}");
        // Vier Abrundungen, also fehlen höchstens vier Einheiten.
        assert!(
            voll - summe <= 4,
            "zu viel verloren: {} Einheiten",
            voll - summe
        );
    }

    /// ⚑ **Der Kopf-Shard bekommt mehr als ein Mittelstück.** Genau
    /// diese Ungleichheit ist der Grund, aus dem die Zuschreibung an den
    /// Zuschnitt hängt und nicht an der Layer-Zahl: Bei Qwen2.5-0,5B
    /// wiegt der LM-Kopf gut neun Layer.
    #[test]
    fn der_kopf_shard_bekommt_mehr_als_ein_mittelstueck() {
        let z = zuschreiben(&qwen05b(), &[voller_pod(1_000, 1)]).expect("Zuschreibung");
        let kopf = z.je_miner[&miner(4)];
        let mitte_ = z.je_miner[&miner(2)];
        assert!(
            kopf > mitte_ * 2,
            "Kopf {kopf} sollte deutlich ueber dem Mittelstueck {mitte_} liegen"
        );
    }

    /// ⚑ **Die Gegenprobe zur Entscheidung „abgeleitet statt erklärt":**
    /// Ein anderer Zuschnitt ergibt eine andere Gutschrift. Wäre der
    /// Anteil ein erklärtes Feld, bliebe die Zahl gleich, egal was der
    /// Miner tatsächlich hielt.
    #[test]
    fn ein_anderer_zuschnitt_ergibt_eine_andere_gutschrift() {
        let profil = qwen05b();
        let schmal = Podleistung {
            positionen: vec![Podposition {
                miner: miner(1),
                zuschnitt: mitte(0, 2),
            }],
            reserve: vec![],
            segmente: 500,
        };
        let breit = Podleistung {
            positionen: vec![Podposition {
                miner: miner(1),
                zuschnitt: mitte(0, 12),
            }],
            reserve: vec![],
            segmente: 500,
        };
        let a = zuschreiben(&profil, &[schmal]).expect("schmal");
        let b = zuschreiben(&profil, &[breit]).expect("breit");
        assert!(
            b.je_miner[&miner(1)] > a.je_miner[&miner(1)],
            "der breitere Zuschnitt muss mehr bekommen"
        );
    }

    /// Wer in zwei Pods sitzt, bekommt beide Gutschriften.
    #[test]
    fn wer_in_zwei_pods_sitzt_bekommt_beide() {
        let profil = qwen05b();
        let einer = Podleistung {
            positionen: vec![Podposition {
                miner: miner(9),
                zuschnitt: mitte(0, 6),
            }],
            reserve: vec![],
            segmente: 100,
        };
        let allein = zuschreiben(&profil, std::slice::from_ref(&einer)).expect("einer");
        let doppelt = zuschreiben(&profil, &[einer.clone(), einer]).expect("zwei");
        assert_eq!(
            doppelt.je_miner[&miner(9)],
            allein.je_miner[&miner(9)] * 2,
            "zwei Positionen muessen zweimal gutgeschrieben werden"
        );
    }

    /// Die Reserve bekommt nichts und steht trotzdem im Ergebnis.
    #[test]
    fn die_reserve_bekommt_nichts_und_wird_genannt() {
        let z = zuschreiben(&qwen05b(), &[voller_pod(1_000, 1)]).expect("Zuschreibung");
        assert!(!z.je_miner.contains_key(&miner(5)), "Reserve wurde bezahlt");
        assert!(!z.je_miner.contains_key(&miner(6)), "Reserve wurde bezahlt");
        assert_eq!(z.reserve_ohne_anteil, vec![miner(5), miner(6)]);
    }

    /// ⚑ Wer in einem Pod Reserve ist und in einem anderen rechnet,
    /// steht **nicht** unter den Übergangenen.
    #[test]
    fn reserve_hier_und_position_dort_zaehlt_als_bezahlt() {
        let profil = qwen05b();
        let pod_a = voller_pod(1_000, 1); // Reserve: miner(5), miner(6)
        let pod_b = Podleistung {
            positionen: vec![Podposition {
                miner: miner(5),
                zuschnitt: mitte(0, 6),
            }],
            reserve: vec![],
            segmente: 100,
        };
        let z = zuschreiben(&profil, &[pod_a, pod_b]).expect("Zuschreibung");
        assert!(z.je_miner.contains_key(&miner(5)), "die Position fehlt");
        assert_eq!(
            z.reserve_ohne_anteil,
            vec![miner(6)],
            "miner(5) hat gerechnet und gehoert nicht in die Liste"
        );
    }

    /// Die Reihenfolge der Pods ändert das Ergebnis nicht.
    #[test]
    fn die_reihenfolge_der_pods_aendert_nichts() {
        let profil = qwen05b();
        let a = voller_pod(700, 1);
        let b = voller_pod(300, 11);
        let vorwaerts = zuschreiben(&profil, &[a.clone(), b.clone()]).expect("vorwaerts");
        let rueckwaerts = zuschreiben(&profil, &[b, a]).expect("rueckwaerts");
        assert_eq!(vorwaerts, rueckwaerts);
    }

    /// Gegenprobe: derselbe Miner zweimal in einem Pod ist ein Fehler,
    /// keine Summe.
    #[test]
    fn derselbe_miner_zweimal_im_pod_ist_ein_fehler() {
        let profil = qwen05b();
        let pod = Podleistung {
            positionen: vec![
                Podposition {
                    miner: miner(1),
                    zuschnitt: mitte(0, 6),
                },
                Podposition {
                    miner: miner(1),
                    zuschnitt: mitte(6, 12),
                },
            ],
            reserve: vec![],
            segmente: 100,
        };
        assert_eq!(
            zuschreiben(&profil, &[pod]),
            Err(ZuschreibungFehler::MinerZweimalImPod { pod: 0 })
        );
    }

    /// Gegenprobe: auch über Position und Reserve hinweg.
    #[test]
    fn position_und_reserve_derselbe_miner_ist_ein_fehler() {
        let profil = qwen05b();
        let pod = Podleistung {
            positionen: vec![Podposition {
                miner: miner(1),
                zuschnitt: mitte(0, 6),
            }],
            reserve: vec![miner(1)],
            segmente: 100,
        };
        assert_eq!(
            zuschreiben(&profil, &[pod]),
            Err(ZuschreibungFehler::MinerZweimalImPod { pod: 0 })
        );
    }

    /// Gegenprobe: ein Pod ohne Position wird abgelehnt, nicht ignoriert.
    #[test]
    fn ein_pod_ohne_position_wird_abgelehnt() {
        let profil = qwen05b();
        let pod = Podleistung {
            positionen: vec![],
            reserve: vec![miner(1)],
            segmente: 100,
        };
        assert_eq!(
            zuschreiben(&profil, &[pod]),
            Err(ZuschreibungFehler::PodOhnePositionen { pod: 0 })
        );
    }

    /// Gegenprobe: ein Zuschnitt jenseits des Modells wird mit Pod- und
    /// Positionsnummer beanstandet, damit man ihn findet.
    #[test]
    fn ein_zuschnitt_jenseits_des_modells_nennt_seine_stelle() {
        let profil = qwen05b();
        let gut = voller_pod(100, 1);
        let schlecht = Podleistung {
            positionen: vec![
                Podposition {
                    miner: miner(20),
                    zuschnitt: mitte(0, 6),
                },
                Podposition {
                    miner: miner(21),
                    zuschnitt: mitte(6, 999),
                },
            ],
            reserve: vec![],
            segmente: 100,
        };
        match zuschreiben(&profil, &[gut, schlecht]) {
            Err(ZuschreibungFehler::ZuschnittUnbrauchbar { pod, position, .. }) => {
                assert_eq!((pod, position), (1, 1));
            }
            andere => panic!("erwartet war eine Beanstandung, kam: {andere:?}"),
        }
    }

    /// Ohne Segmente gibt es nichts zu verteilen, und das ist kein Fehler.
    #[test]
    fn ohne_segmente_gibt_es_keine_gutschrift() {
        let z = zuschreiben(&qwen05b(), &[voller_pod(0, 1)]).expect("Zuschreibung");
        assert_eq!(z.summe(), 0);
        assert_eq!(z.je_miner.len(), 4, "die Positionen stehen mit null da");
    }

    // --- Der Weg der Kette: aus Bündel und Arbeitsverteilung ---

    fn verteilung(gewichte: Vec<u64>) -> Arbeitsverteilung {
        Arbeitsverteilung::neu(myl_types::Hash::sha256(b"probe"), gewichte)
            .expect("Verteilung")
    }

    fn abrechnung(ab: u8, vtfe: u64) -> Podabrechnung {
        Podabrechnung {
            positionen: (0..4u32).map(|i| (miner(ab + i as u8), i)).collect(),
            reserve: vec![miner(ab + 4), miner(ab + 5)],
            vtfe_pod: vtfe,
        }
    }

    /// Die vTFE eines Pods wird nach den Gewichten aufgeteilt, und die
    /// Summe bleibt erhalten.
    #[test]
    fn die_pod_vtfe_wird_vollstaendig_aufgeteilt() {
        let v = verteilung(vec![3, 1, 1, 5]);
        let z = zuschreiben_aus_abrechnung(&v, &[abrechnung(1, 1_000)]).expect("Abrechnung");
        assert_eq!(z.summe(), 1_000, "es ging etwas verloren");
    }

    /// ⚑ **Die beiden Wege ergeben dasselbe**: der des Prüfers mit
    /// Modellprofil und Zuschnitt, und der der Kette mit Gewichten.
    ///
    /// Sie runden verschieden (der eine je Position ab, der andere
    /// verteilt den Rest), also wird auf **eine Einheit genau**
    /// verglichen. Mehr wäre falsch behauptet, weniger wertlos.
    #[test]
    fn beide_wege_ergeben_dasselbe() {
        let profil = qwen05b();
        let pod = voller_pod(1_000, 1);

        // Weg des Prüfers.
        let a = zuschreiben(&profil, std::slice::from_ref(&pod)).expect("Pruefer");

        // Weg der Kette: Gewichte sind die MAC-Anteile der Zuschnitte.
        let gewichte: Vec<u64> = pod
            .positionen
            .iter()
            .map(|p| p.zuschnitt.macs(&profil) as u64)
            .collect();
        let v = verteilung(gewichte);
        let b = zuschreiben_aus_abrechnung(
            &v,
            &[Podabrechnung {
                positionen: pod
                    .positionen
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.miner, i as u32))
                    .collect(),
                reserve: pod.reserve.clone(),
                vtfe_pod: a.summe() as u64,
            }],
        )
        .expect("Kette");

        assert_eq!(a.je_miner.len(), b.je_miner.len());
        for (m, wert_a) in &a.je_miner {
            let wert_b = b.je_miner[m];
            let abstand = wert_a.abs_diff(wert_b);
            assert!(
                abstand <= 1,
                "Miner {m:?}: Pruefer {wert_a}, Kette {wert_b}, Abstand {abstand}"
            );
        }
        assert_eq!(a.reserve_ohne_anteil, b.reserve_ohne_anteil);
    }

    /// Die Reserve bekommt auch auf diesem Weg nichts und wird genannt.
    #[test]
    fn die_reserve_bekommt_auch_hier_nichts() {
        let v = verteilung(vec![1, 1, 1, 1]);
        let z = zuschreiben_aus_abrechnung(&v, &[abrechnung(1, 400)]).expect("Abrechnung");
        assert_eq!(z.reserve_ohne_anteil, vec![miner(5), miner(6)]);
        assert!(!z.je_miner.contains_key(&miner(5)));
    }

    /// ⚑ Gegenprobe: Eine Position, die die Verteilung nicht kennt, ist
    /// ein Fehler und keine Null.
    #[test]
    fn eine_unbekannte_position_ist_ein_fehler() {
        let v = verteilung(vec![1, 1]);
        let ergebnis = zuschreiben_aus_abrechnung(&v, &[abrechnung(1, 400)]);
        assert!(
            matches!(ergebnis, Err(Abrechnungsfehler::PositionUnbekannt { .. })),
            "kam: {ergebnis:?}"
        );
    }

    /// Gegenprobe: derselbe Miner zweimal im Pod.
    #[test]
    fn derselbe_miner_zweimal_ist_auch_hier_ein_fehler() {
        let v = verteilung(vec![1, 1]);
        let a = Podabrechnung {
            positionen: vec![(miner(1), 0), (miner(1), 1)],
            reserve: vec![],
            vtfe_pod: 100,
        };
        assert_eq!(
            zuschreiben_aus_abrechnung(&v, &[a]),
            Err(Abrechnungsfehler::MinerZweimalImPod { pod: 0 })
        );
    }

    /// Gegenprobe: ein Pod ohne Position.
    #[test]
    fn ein_pod_ohne_position_ist_auch_hier_ein_fehler() {
        let v = verteilung(vec![1, 1]);
        let a = Podabrechnung {
            positionen: vec![],
            reserve: vec![miner(1)],
            vtfe_pod: 100,
        };
        assert_eq!(
            zuschreiben_aus_abrechnung(&v, &[a]),
            Err(Abrechnungsfehler::PodOhnePositionen { pod: 0 })
        );
    }

    /// Wer in zwei Pods sitzt, bekommt auch hier beide Gutschriften.
    #[test]
    fn wer_in_zwei_pods_sitzt_bekommt_auch_hier_beide() {
        let v = verteilung(vec![1, 1, 1, 1]);
        let einer = Podabrechnung {
            positionen: vec![(miner(9), 0)],
            reserve: vec![],
            vtfe_pod: 100,
        };
        let z = zuschreiben_aus_abrechnung(&v, &[einer.clone(), einer]).expect("Abrechnung");
        // Position 0 trägt ein Viertel der Gewichte, also 25 je Pod.
        assert_eq!(z.je_miner[&miner(9)], 50);
    }

    /// ⚑ **Unbesetzte Positionen bekommen nichts, und ihr Anteil
    /// verfällt.** Das sieht nach Verlust aus und ist die einzig
    /// richtige Wahl.
    ///
    /// ⛑ Der erste Entwurf des Tests darüber erwartete, dass die ganze
    /// Pod-vTFE auf die **besetzten** Positionen fällt. Das wäre falsch:
    /// Die Besetzten bekämen dann Geld für Layer, die sie **nicht
    /// gerechnet haben**. Ein Pod, dem eine Position fehlt, hat weniger
    /// geleistet, und genau das bildet der Verfall ab.
    ///
    /// **Die sichere Richtung dazu:** Was verfällt, wird nicht
    /// umverteilt und auch nicht geprägt; es entsteht schlicht nicht.
    #[test]
    fn unbesetzte_positionen_lassen_ihren_anteil_verfallen() {
        let v = verteilung(vec![1, 1, 1, 1]);
        let halb = Podabrechnung {
            positionen: vec![(miner(1), 0), (miner(2), 1)],
            reserve: vec![],
            vtfe_pod: 1_000,
        };
        let z = zuschreiben_aus_abrechnung(&v, &[halb]).expect("Abrechnung");
        assert_eq!(z.summe(), 500, "der Anteil der Unbesetzten wurde umverteilt");
        assert_eq!(z.je_miner[&miner(1)], 250);
        assert_eq!(z.je_miner[&miner(2)], 250);
    }
}
