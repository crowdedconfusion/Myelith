//! Ein vollständiger Protokolldurchlauf, Schicht für Schicht.
//!
//! Der Weg, den ein Segment nimmt:
//!
//! ```text
//!  Scheduler  ─ Komitee, Pods, Redundanzpaare, Stichprobenlotterie
//!      │
//!  Pod        ─ rechnet, führt Spur, signiert Übergänge
//!      │
//!  Koordinator─ bündelt, Mitglieder unterschreiben (Fund 52)
//!      │
//!  Konsens    ─ prüft Aggregat gegen die Mitgliedermenge
//!      │
//!  Verifikation ─ Stufe 1 Vergleich, Kontrollsegmente, Bisektion
//!      │
//!  Epochenabschluss ─ Übereinstimmung, Rückbuchung, Endgültigkeit
//!      │
//!  Ledger     ─ prägt, verteilt, schlachtet
//! ```
//!
//! Jede Stufe wird **gegen die echte Implementierung** gefahren, nicht
//! gegen eine Nachbildung. Wo eine Stufe ohne Modell-Artefakte nicht
//! laufen kann, wird sie ausgelassen und das ausdrücklich vermerkt —
//! nicht durch eine Attrappe ersetzt.

use std::collections::BTreeMap;

use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MinerId, PodId, SegmentId};

/// Wie schwer ein Befund wiegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Schwere {
    /// Läuft, aber jemand sollte es wissen.
    Hinweis,
    /// Eine Zusage des Papiers gilt nicht oder nur eingeschränkt.
    Luecke,
    /// Ein Angriff geht durch oder ein ehrlicher Teilnehmer wird bestraft.
    Schwer,
}

/// Ein Befund der Simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Befund {
    pub schwere: Schwere,
    /// An welcher Naht.
    pub stelle: String,
    /// Was beobachtet wurde.
    pub beobachtung: String,
    /// Woran es hängt.
    pub folge: String,
}

/// Ein Teilnehmer der Simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Teilnehmer {
    pub miner: MinerId,
    pub adresse: Address,
    /// Kapazität in Segmenten je Epoche.
    pub kapazitaet: u64,
    /// Verhält er sich ehrlich?
    pub ehrlich: bool,
}

impl Teilnehmer {
    pub fn neu(n: u64, kapazitaet: u64, ehrlich: bool) -> Self {
        let mut b = [0u8; 32];
        b[..8].copy_from_slice(&n.to_le_bytes());
        Self {
            miner: MinerId::new(b),
            adresse: Address::new(b),
            kapazitaet,
            ehrlich,
        }
    }
}

/// Der Zustand eines Durchlaufs.
pub struct Protokolllauf {
    pub epoche: EpochId,
    pub teilnehmer: Vec<Teilnehmer>,
    pub ledger: myl_ledger::LedgerState,
    pub registry: myl_governance::ParameterRegistry,
    pub befunde: Vec<Befund>,
}

impl Protokolllauf {
    /// Ein Netz aus `ehrliche` ehrlichen und `boese` unehrlichen
    /// Teilnehmern, jeder mit dem Stake, den die Sicherheitsbedingung
    /// verlangt.
    ///
    /// **Der Stake kommt aus `S_min`, nicht aus einer Setzung.** Das ist
    /// die erste Naht, die geprüft wird: Was GOVERNANCE als Parameter
    /// führt, muss TOKENOMICS ausrechnen können, und der Betrag muss im
    /// Ledger buchbar sein.
    pub fn neu(ehrliche: u64, boese: u64) -> Result<Self, String> {
        use myl_governance::registry::Parameter;

        let registry = myl_governance::ParameterRegistry::vorgabe();
        let (pz, pn) = registry
            .wert(Parameter::Stichprobenrate)
            .als_bruch()
            .ok_or("Stichprobenrate ist kein Bruch")?;
        let g = registry
            .wert(Parameter::Betrugsgewinn)
            .als_ganzzahl()
            .ok_or("Betrugsgewinn ist keine Ganzzahl")?;

        let mut teilnehmer = Vec::new();
        for i in 0..(ehrliche + boese) {
            teilnehmer.push(Teilnehmer::neu(i + 1, 1, i < ehrliche));
        }

        let mut ledger = myl_ledger::LedgerState::genesis(
            registry
                .wert(Parameter::MindestStake)
                .als_ganzzahl()
                .unwrap_or(1)
                / 1_000,
        );

        let anspruch = myl_tokenomics::erforderlicher_stake(1, g, pz, pn)
            .map_err(|e| format!("Stake-Anspruch: {e}"))?;
        for t in &teilnehmer {
            let konto = ledger.account_mut(&t.adresse);
            konto.staked = anspruch.gesamt;
            konto.balance = anspruch.gesamt; // Guthaben für Burns
        }

        Ok(Self {
            epoche: EpochId(1),
            teilnehmer,
            ledger,
            registry,
            befunde: Vec::new(),
        })
    }

    fn melde(&mut self, schwere: Schwere, stelle: &str, beobachtung: &str, folge: &str) {
        self.befunde.push(Befund {
            schwere,
            stelle: stelle.to_string(),
            beobachtung: beobachtung.to_string(),
            folge: folge.to_string(),
        });
    }

    /// **Naht 1: Scheduler → Pods.**
    ///
    /// Prüft, dass die Zuteilung überhaupt Pods und Redundanzpaare
    /// liefert, und dass die Paare disjunkt sind. Ohne Disjunktheit ist
    /// Stufe 1 der Verifikation eine Selbstbestätigung.
    pub fn naht_scheduler(&mut self) -> Vec<myl_scheduler::shard_assignment::Pod> {
        use myl_scheduler::geo_clustering::MinerCluster;
        use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
        use myl_scheduler::shard_assignment::assign_shards;

        let k = 2u32;
        // ⚑ **Ein Pod hat k+2 Mitglieder, nicht k** (Entscheidung D3,
        // 2026-08-26). Hier stand `je_pod = k`, und damit entstanden
        // Pods ohne Reserve. Das fiel nicht auf, weil dieser Test die
        // Reserve nie ansah; seit `assign_shards` vollständige Pods
        // verlangt, fällt es auf.
        let je_pod = myl_scheduler::shard_assignment::pod_groesse(k);
        let seed = [7u8; 32];
        let mut pods = Vec::new();
        // Getrennt gezählt und danach gemeldet: Innerhalb der Schleife
        // liegt eine unveränderliche Ausleihe auf `self.teilnehmer`.
        let mut unvollstaendig = 0usize;
        for (i, gruppe) in self.teilnehmer.chunks(je_pod).enumerate() {
            if gruppe.len() < je_pod {
                break;
            }
            let miners: Vec<MinerRegistration> = gruppe
                .iter()
                .map(|t| MinerRegistration {
                    miner_id: t.miner,
                    hardware_class: HardwareClass::MediumGpu,
                    registration_epoch: 0,
                })
                .collect();
            let cluster = MinerCluster {
                miners,
                max_internal_latency: 10,
            };
            if let Some(pod) = assign_shards(&cluster.miners, k, i as u32, &seed) {
                pods.push(pod);
            } else {
                unvollstaendig += 1;
            }
        }

        if unvollstaendig > 0 {
            self.melde(
                Schwere::Luecke,
                "Scheduler → Pods",
                "eine Gruppe ergab keinen vollständigen Pod",
                "ein Pod braucht k+2 Mitglieder; mit weniger blieben Positionen \
                 unbesetzt und die Pipeline liefe ins Leere",
            );
        }

        if pods.len() < 2 {
            self.melde(
                Schwere::Luecke,
                "Scheduler → Pods",
                "weniger als zwei Pods gebildet",
                "ohne zwei disjunkte Pods gibt es keinen Redundanzvergleich, \
                 und Stufe 1 der Verifikation entfällt",
            );
        }
        pods
    }

    /// **Naht 2: Verifikation Stufe 1 → Epochenabschluss.**
    ///
    /// Zwei Pods liefern Spuren; der Vergleich entscheidet, und das
    /// Ergebnis geht in den Epochenabschluss. Geprüft wird, dass ein
    /// **fehlendes** Ergebnis nicht als Übereinstimmung durchgeht.
    pub fn naht_verifikation(&mut self, ehrlich: bool) -> myl_verifier::CompareResult {
        let laenge = 24usize;
        let primaer: Vec<Hash> = (0..laenge)
            .map(|i| Hash::sha256(&(i as u64).to_le_bytes()))
            .collect();
        let redundant = if ehrlich {
            primaer.clone()
        } else {
            let mut r = primaer.clone();
            for h in r.iter_mut().skip(9) {
                *h = Hash::sha256(&[9u8; 8]);
            }
            r
        };
        match myl_verifier::compare_commitments(&primaer, &redundant) {
            Ok(res) => res,
            Err(e) => {
                self.melde(
                    Schwere::Schwer,
                    "Verifikation Stufe 1",
                    &format!("Vergleich schlug fehl: {e}"),
                    "ohne Vergleich gibt es keine Stufe 1",
                );
                myl_verifier::CompareResult::Match
            }
        }
    }

    /// **Naht 3: Urteil → Ledger.**
    ///
    /// Ein Schuldspruch aus VERIFICATION wird mit den Sätzen aus
    /// TOKENOMICS im Ledger gebucht. Drei Komponenten, eine Buchung.
    pub fn naht_slashing(
        &mut self,
        schuldiger: &Teilnehmer,
        checker: &Teilnehmer,
    ) -> Option<myl_ledger::VerdictEffect> {
        use myl_tokenomics::slashing::{urteil_buchen_gestaffelt, Akteur, Grund};

        let verdict = myl_ledger::Verdict {
            segment_id: SegmentId::new([3u8; 32]),
            miner: schuldiger.adresse,
            checker: checker.adresse,
            outcome: myl_ledger::VerdictOutcome::SlashMiner,
        };
        let vorher = self.ledger.account(&schuldiger.adresse).staked;
        let vorverstoesse_vorher = self
            .ledger
            .verstoesse_im_fenster(&schuldiger.adresse, myl_tokenomics::WIEDERHOLUNGSFENSTER);
        // **Über den Weg, der die Reihenfolge festlegt** (seit
        // 2026-08-27). Vorher stand hier `satz_gestaffelt(…, 0)`: eine
        // getippte Null als Vorgeschichte, in einer Naht, deren Zweck
        // gerade das Zusammenspiel dreier Komponenten ist. Die
        // Staffelung wäre damit auch dann grün gewesen, wenn niemand
        // gezählt hätte.
        match urteil_buchen_gestaffelt(
            &mut self.ledger,
            &verdict,
            Akteur::ShardMiner,
            Grund::FalschesErgebnis,
        ) {
            Ok((effekt, _satz)) => {
                let nachher = self.ledger.account(&schuldiger.adresse).staked;
                if effekt.slashed == 0 {
                    self.melde(
                        Schwere::Schwer,
                        "Urteil → Ledger",
                        "Schuldspruch gebucht, aber nichts geschlachtet",
                        "ein Urteil ohne Wirkung ist keine Abschreckung",
                    );
                }
                if nachher + effekt.slashed != vorher {
                    self.melde(
                        Schwere::Schwer,
                        "Urteil → Ledger",
                        &format!("Stake {vorher} → {nachher}, geschlachtet {}", effekt.slashed),
                        "die Buchung stimmt nicht mit dem Effekt überein",
                    );
                }
                // **Gebucht heißt gezählt.** Ohne diese Prüfung liefe
                // die Naht auch dann durch, wenn der Verstoß-Zähler
                // stehen bliebe, und die Slashing-Staffelung wäre wieder
                // eine Tabelle, von der immer die erste Zeile gilt.
                let nachher_verstoesse = self
                    .ledger
                    .verstoesse_im_fenster(&schuldiger.adresse, myl_tokenomics::WIEDERHOLUNGSFENSTER);
                if nachher_verstoesse != vorverstoesse_vorher + 1 {
                    self.melde(
                        Schwere::Schwer,
                        "Urteil → Ledger",
                        &format!(
                            "Verstöße {vorverstoesse_vorher} → {nachher_verstoesse} statt +1"
                        ),
                        "ein gebuchtes Urteil, das nicht zählt, macht die Staffelung wirkungslos",
                    );
                }
                if effekt.vorverstoesse != vorverstoesse_vorher {
                    self.melde(
                        Schwere::Schwer,
                        "Urteil → Ledger",
                        &format!(
                            "gemeldete Vorverstöße {} statt {vorverstoesse_vorher}",
                            effekt.vorverstoesse
                        ),
                        "der Satz gilt gegen den Stand VOR dem Urteil; ein zu hoher \
                         Stand schlägt die nächste Stufe zu früh auf",
                    );
                }
                Some(effekt)
            }
            Err(e) => {
                self.melde(
                    Schwere::Schwer,
                    "Urteil → Ledger",
                    &format!("Buchung schlug fehl: {e}"),
                    "ein Schuldspruch, den der Ledger nicht buchen kann, ist wirkungslos",
                );
                None
            }
        }
    }

    /// **Naht 4: Ledger → Tokenomik.**
    ///
    /// Burn erzeugt Credits, die EMA folgt, die Prägung folgt der EMA,
    /// und die Verteilung gibt genau das aus, was geprägt wurde.
    pub fn naht_praegung(&mut self, epochen: u64) -> BTreeMap<&'static str, u64> {
        use myl_governance::registry::Parameter;

        let (sz, sn) = self
            .registry
            .wert(Parameter::Subventionsrate)
            .als_bruch()
            .unwrap_or((0, 1));
        let m_max = self
            .registry
            .wert(Parameter::PraegeObergrenze)
            .als_ganzzahl()
            .unwrap_or(u64::MAX);
        let params = myl_tokenomics::MintParams {
            subsidy_num: sz,
            subsidy_den: sn,
            m_max,
        };

        let mut ema = 0u64;
        let mut gepraegt_gesamt = 0u128;
        let mut verbrannt_gesamt = 0u128;
        for e in 0..epochen {
            let burn = 1_000 * myl_tokenomics::UNITS_PER_MYL;
            // Burn-Cap je Adresse (Kap. 5.6).
            let spielraum = myl_tokenomics::burn_spielraum(ema, 0);
            let tatsaechlich = burn.min(spielraum);
            verbrannt_gesamt += tatsaechlich as u128;
            ema = myl_tokenomics::ema_update(ema, tatsaechlich);
            let m = myl_tokenomics::mint_amount(ema, &params);
            gepraegt_gesamt += m as u128;

            let d = myl_tokenomics::distribute_mint(m);
            if d.summe() != m {
                self.melde(
                    Schwere::Schwer,
                    "Prägung → Verteilung",
                    &format!("Epoche {e}: verteilt {} statt {m}", d.summe()),
                    "Geld verschwindet oder entsteht",
                );
            }
        }

        if gepraegt_gesamt > verbrannt_gesamt {
            self.melde(
                Schwere::Hinweis,
                "Prägung gegen Verbrennung",
                &format!(
                    "über {epochen} Epochen geprägt {gepraegt_gesamt}, verbrannt {verbrannt_gesamt}"
                ),
                "in der Subventionsphase erwartet; im Zielbetrieb wäre es ein Befund",
            );
        }

        let mut aus = BTreeMap::new();
        aus.insert("gepraegt", gepraegt_gesamt.min(u64::MAX as u128) as u64);
        aus.insert("verbrannt", verbrannt_gesamt.min(u64::MAX as u128) as u64);
        aus.insert("ema", ema);
        aus
    }

    /// Die Befunde, nach Schwere geordnet.
    pub fn bericht(&self) -> Vec<&Befund> {
        let mut b: Vec<&Befund> = self.befunde.iter().collect();
        b.sort_by_key(|b| std::cmp::Reverse(b.schwere));
        b
    }

    /// Gibt es einen schweren Befund?
    pub fn schwerer_befund(&self) -> bool {
        self.befunde.iter().any(|b| b.schwere == Schwere::Schwer)
    }
}

/// Eine Pod-Id aus einer Zahl.
pub fn pod_id(n: u8) -> PodId {
    PodId::new([n; 32])
}

/// Was ein Durchlauf abgedeckt hat und was nicht.
///
/// **Der zweite Teil ist der wichtigere.** Eine Simulation, die nur
/// meldet, was sie geprüft hat, liest sich wie eine Abdeckung; erst die
/// Liste der ausgelassenen Stellen sagt, was ihr grünes Ergebnis wert
/// ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abdeckung {
    /// Nähte, die gefahren wurden.
    pub gefahren: Vec<&'static str>,
    /// Nähte, die ausgelassen wurden, mit Grund.
    pub ausgelassen: Vec<(&'static str, &'static str)>,
}

impl Abdeckung {
    /// Die Abdeckung eines vollständigen Durchlaufs.
    ///
    /// **Fest verdrahtet und nicht gemessen**, und das ist eine
    /// Schwäche: Wer eine Naht hinzufügt und diese Liste vergisst, hat
    /// eine Abdeckung, die zu gut aussieht. Der Test
    /// `die_abdeckung_nennt_jede_naht` hält die Liste gegen die
    /// öffentlichen Nahtfunktionen.
    pub fn vollstaendig() -> Self {
        Self {
            gefahren: vec![
                "Governance → Tokenomik (S_min bestimmt den Stake)",
                "Scheduler → Pods (Zuteilung, Redundanzpaare)",
                "Verifikation Stufe 1 (Commitment-Vergleich)",
                "Urteil → Slashing-Matrix → Ledger",
                "Ledger → EMA → Prägung → Verteilung",
                "Burn-Cap gegen den Verbrauchs-Stoß",
            ],
            ausgelassen: vec![
                (
                    "Pod → Koordinator → Konsens (echte Inferenz)",
                    "braucht Modell-Artefakte; läuft in \
                     myl-pod/tests/koordinator_byzantinisch.rs mit echtem Modell",
                ),
                (
                    "Bisektion und Schiedsrunde",
                    "braucht eine Spur aus echter Rechnung; die Mechanik ist in \
                     myl-verifier/tests/adversarial.rs geprüft",
                ),
                (
                    "Netzschicht (Gossip, Peer-Wahl)",
                    "eigener Testrahmen mit tokio; die Eclipse-Messung steht in \
                     myl-net/tests/eclipse_sybil.rs",
                ),
                (
                    "BFT-Rundenwechsel und Liveness",
                    "geprüft in myl-consensus/tests/liveness.rs über 21 Validatoren",
                ),
                (
                    "Kontrollsegment-Einschleusung im Durchlauf",
                    "die Rate ist in myl-verifier/tests/simulation.rs gemessen; \
                     im Durchlauf fehlt der Auftragsstrom, in den eingeschleust würde",
                ),
                (
                    "Cross-Hardware-Determinismus",
                    "braucht eine zweite Architektur (K1) — der wichtigste offene \
                     Beleg des Projekts",
                ),
            ],
        }
    }

    /// Der Anteil gefahrener Nähte, in Prozent.
    ///
    /// **Eine Zahl mit Vorsicht:** Nähte sind nicht gleich schwer. Sechs
    /// gefahrene gegen sechs ausgelassene sind nicht „die Hälfte
    /// geprüft", denn unter den ausgelassenen ist die teuerste (K1).
    pub fn anteil_prozent(&self) -> u32 {
        let gesamt = self.gefahren.len() + self.ausgelassen.len();
        if gesamt == 0 {
            return 0;
        }
        (self.gefahren.len() * 100 / gesamt) as u32
    }
}
