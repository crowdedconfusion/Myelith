//! Eine laufende Kette über mehrere Epochen, in der alles ineinander
//! greift.
//!
//! # ⚑ Was diese Simulation kann, was `gesamtlauf.rs` nicht kann
//!
//! Der Gesamtlauf geht **zwölf Stationen an einem Auftrag** ab und
//! belegt, dass die Kisten zusammenpassen. Er hält die Kette dabei
//! still: eine Epoche, ein Block, ein Bündel.
//!
//! Hier läuft die Kette über **mehrere Epochen**, und die Größen hängen
//! voneinander ab:
//!
//! | Was geschieht | Was davon abhängt |
//! |---|---|
//! | Miner melden sich in drei Zonen | Zonencluster, Restetopf, Podbildung |
//! | Nutzer verbrennen und geben aus | Auslastung, Prägung, Trainingsabgabe |
//! | Die Auslastung fällt | mehr Trainingspods |
//! | Trainingspods liefern ab | Modellfassung, Vergütung aus dem Puffer |
//! | Ein Pod liefert Falsches | keine Bestätigung, keine Vergütung |
//! | Ein Miner rechnet falsch | Slashing, Kopfgeld |
//!
//! ⚑ **Der Punkt ist die Rückkopplung.** Jede dieser Größen ist einzeln
//! geprüft; was hier geprüft wird, ist, dass sie sich **gegenseitig**
//! richtig bewegen. Genau dort saßen die schweren Funde dieses Projekts:
//! Fund 50 zwischen CONSENSUS und der Epochenlänge, Fund 51 zwischen
//! INTEGER_LLM und CONSENSUS, Fund 184 zwischen Kap. 7.1 und dem
//! Kettenzustand. **Fund 190 kam aus diesem Lauf hier**, und zwar nicht
//! aus einer fehlgeschlagenen Zusicherung, sondern daraus, dass beim
//! Aufstellen der zwölf Miner die Frage aufkam, die der Code nie
//! stellte: Stehen die beiden Pods eines Trainingspaars eigentlich
//! getrennt?
//!
//! # ⚑ Wo dieser Lauf aufhört, und warum das hier steht
//!
//! **Die Zeile „Die Auslastung fällt, also mehr Trainingspods" fährt er
//! nicht.** Die Nachfrage wird gezählt und rollt nachweislich in die
//! Vorepoche; dass daraus eine Auslastung über null wird, kann er nicht
//! zeigen. `PodKapazitaet` ist mit 3,6 Milliarden vTFE je Pod und
//! Epoche eine Zahl für ein echtes Netz, und die Ausgabe einer
//! Probekette ist dagegen auf der Festkommaskala exakt null.
//!
//! Geprüft ist damit die **Verdrahtung**: dass die Zahl entsteht,
//! gezählt wird und ankommt. Die Kurve daneben rechnet der Lauf an der
//! Regel und schreibt sie ins Protokoll, **als Rechnung gekennzeichnet
//! und nicht als gefahrener Fall**. Eine Simulation, die ihre Grenze
//! verschweigt, belegt weniger als eine, die sie ausspricht.

use myl_consensus::block::{Anweisung, Transaktion};
use myl_ledger::state::LedgerState;
use myl_ledger::transitions::{apply_verdict, SlashParams};
use myl_consensus::block::BLOECKE_JE_EPOCHE;
use myl_node::kette::{probekonto, probeschluessel, Kette};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MinerId, SegmentId};
use myl_types::miner::{pod_kennung, HardwareClass};
use myl_types::node_metadata::GeoRegion;
use myl_types::trainingssegment::Trainingssegment;

/// Ein Protokoll, das am Ende sagt, was berührt wurde.
struct Bericht {
    zeilen: Vec<String>,
}

impl Bericht {
    fn neu() -> Self {
        Self { zeilen: Vec::new() }
    }
    fn sagt(&mut self, wo: &str, was: impl AsRef<str>) {
        let z = format!("  [{wo:<16}] {}", was.as_ref());
        eprintln!("{z}");
        self.zeilen.push(z);
    }
}

/// Drei Zonen zu je vier Minern.
///
/// ⚑ **Vier je Zone ist unter der Podgröße von sechs.** Damit fällt
/// jede Zone in den Sammeltopf, und die Simulation geht genau den Weg,
/// den ein junges Netz geht: Regionen sind besetzt, aber keine trägt
/// allein einen Pod.
const ZONEN: [GeoRegion; 3] = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];

fn miner_anmelden(k: &mut Kette, nonce: &mut [u64; 12]) {
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: ZONEN[(w as usize) % ZONEN.len()],
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung: MinerId::new(*probekonto(w).as_bytes()),
                    konto: probekonto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
}

fn epoche_weiter(k: &mut Kette) {
    for _ in 0..BLOECKE_JE_EPOCHE {
        k.baue_block();
    }
}

fn schluessel_von(id: MinerId) -> u8 {
    (0..12u8)
        .find(|w| MinerId::new(*probekonto(*w).as_bytes()) == id)
        .expect("Mitglied ist ein Probekonto")
}

/// Baut ein unterschriebenes Trainingssegment für einen Pod.
fn segment_bauen(
    pod: &myl_scheduler::shard_assignment::Pod,
    charge: Hash,
    commitment: Hash,
) -> Trainingssegment {
    segment_mit_wirkung(pod, charge, commitment, 1)
}

/// Wie [`segment_bauen`], aber mit wählbarer Zahl bewegter Gewichte.
fn segment_mit_wirkung(
    pod: &myl_scheduler::shard_assignment::Pod,
    charge: Hash,
    commitment: Hash,
    bewegte: u64,
) -> Trainingssegment {
    let mut seg = Trainingssegment {
        id: SegmentId::new([3u8; 32]),
        modell_version: myl_types::ids::MerkleRoot::new([4u8; 32]),
        charge,
        startschritt: 0,
        schrittzahl: 30,
        folgen: 1,
        lr_zaehler: 1,
        // ⚑ **Die Rate folgt der Budgetregel und ist keine Zahl mehr.**
        // Hier stand `1 << 12`, und seit dem 2026-09-06 weist die Kette
        // das ab: dreissig Schritte zu je einer Folge ueber
        // vierundzwanzig Ebenen verlangen einen weit groesseren Nenner.
        // **Der Test hat das gefunden, nicht ein Codeleser.**
        lr_nenner: myl_types::lernrate::lernrate_nenner(
            myl_tokenomics::vtfe::PROBE_MODELL.num_layers as u32,
            30,
        ),
        delta_commitment: commitment,
        bewegte_gewichte: bewegte,
        pod_pfad: pod.mitglieder().map(|m| m.miner_id).collect(),
        signaturen: Vec::new(),
    };
    let botschaft = seg.botschaft();
    seg.signaturen = pod
        .mitglieder()
        .map(|m| probeschluessel(schluessel_von(m.miner_id)).sign(&botschaft).expect("sig"))
        .collect();
    seg
}

/// ⚑ **Die verzahnte Simulation.**
#[test]
fn eine_laufende_kette_greift_ineinander() {
    let mut b = Bericht::neu();
    let mut k = Kette::probestand();
    let mut nonce = [0u64; 12];

    // ---- 1. Miner in drei Zonen ------------------------------------
    miner_anmelden(&mut k, &mut nonce);
    k.baue_block();
    assert_eq!(k.zustand().miner.len(), 12, "die Anmeldungen kamen nicht an");
    b.sagt("CONSENSUS", "zwoelf Miner in drei Zonen angemeldet");

    // Der Registrierungsschluss liegt zwei Epochen zurueck.
    for _ in 0..3 {
        epoche_weiter(&mut k);
    }

    // ---- 2. Podbildung über Zonen und Restetopf ---------------------
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 2, "zwoelf Miner muessen zwei Pods ergeben");
    let zonen_je_pod: Vec<usize> = zuteilung
        .pods
        .iter()
        .map(|p| {
            p.mitglieder().map(|m| m.zone).collect::<std::collections::BTreeSet<_>>().len()
        })
        .collect();
    // ⚑ **Keine Zone traegt allein einen Pod**, also sind beide gemischt.
    assert!(zonen_je_pod.iter().all(|n| *n > 1), "ein Pod ist zonenrein: {zonen_je_pod:?}");
    b.sagt(
        "CONSENSUS",
        format!("zwei Pods aus dem Sammeltopf, Zonen je Pod {zonen_je_pod:?}"),
    );

    // ---- 3. Nachfrage: verbrennen und ausgeben ----------------------
    let vor_burn = k.zustand().burn_epoche;
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            nonce[0],
            Anweisung::Burn { betrag: 2_000_000 },
        )
        .expect("signieren"),
    );
    nonce[0] += 1;
    k.baue_block();
    assert!(k.zustand().burn_epoche > vor_burn, "der Burn wurde nicht gezaehlt");
    b.sagt("TOKENOMICS", format!("Burn {} in dieser Epoche", k.zustand().burn_epoche));

    // ---- 3b. Nachfrage wird gezaehlt: Sitzung und Ausgabe ----------
    //
    // ⚑ **Ohne diesen Schritt bleibt die Auslastung null**, und dann
    // greift die ganze Rückkopplung nicht: keine Trainingsabgabe, kein
    // Puffer, ein Trainingsanteil von 82 Prozent. Genau das ist der
    // Unterschied zwischen einer Kette, die läuft, und einer, die
    // stillsteht.
    let nutzer = probekonto(0);
    let agent = probekonto(1);
    let betreiber = probekonto(2);
    let kontrakt = myl_types::sitzung::Sitzungskontrakt::neu(
        nutzer,
        agent,
        myl_types::sitzung::Grenzen {
            budget: 100_000,
            einzellimit: 50_000,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        },
        myl_types::sitzung::Grenzen::gesperrt(),
        vec![betreiber],
        EpochId(0),
        EpochId(1_000),
        1_000,
    )
    .expect("gueltiger Kontrakt");
    let s_id = kontrakt.adresse();
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            nonce[0],
            Anweisung::SitzungEroeffnen { kontrakt: kontrakt.clone() },
        )
        .expect("signieren"),
    );
    nonce[0] += 1;
    k.baue_block();
    assert!(k.zustand().sitzung(&s_id).is_some(), "die Sitzung steht nicht im Zustand");

    let vorhaben = myl_types::sitzung::Vorhaben {
        sitzung: s_id,
        handelnder: agent,
        waehrung: myl_types::sitzung::Waehrung::Credits,
        betrag: 18_000,
        empfaenger: betreiber,
        bestaetigt_ausgeliefert: true,
        nummer: 1,
    };
    myl_ledger::transitions::sitzung_ausgeben(k.zustand_mut(), &agent, &vorhaben, None)
        .expect("Ausgabe");
    assert!(k.zustand().vtfe_epoche > 0, "die Nachfrage wurde nicht gezaehlt");
    b.sagt(
        "TOKENOMICS",
        format!("Nachfrage {} vTFE in dieser Epoche gebucht", k.zustand().vtfe_epoche),
    );

    // ---- 4. Trainingsplan: Anteil, Paare, Buendel -------------------
    let plan = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
    assert_eq!(plan.paare.len(), 1, "zwei Trainingspods ergeben ein Paar");
    let (a, c) = plan.paare[0];
    assert_eq!(
        plan.zuweisungen[&a], plan.zuweisungen[&c],
        "das Paar bekam verschiedene Buendel"
    );
    b.sagt(
        "CONSENSUS",
        format!(
            "Auslastung {}, Trainingsanteil {} bp, Paar ({a}, {c}), Buendel ab {}",
            plan.auslastung, plan.anteil_bps, plan.zuweisungen[&a].start
        ),
    );

    // ⚑ **Und was das Paar an Unabhaengigkeit hergibt.** Zwoelf Miner
    // auf drei Zonen ergeben zwei Pods aus dem Sammeltopf, beide
    // zonengemischt: Es **gibt** hier kein zonendiverses Paar. Der Plan
    // sagt das, statt eine Unabhaengigkeit zu behaupten, die er nicht
    // hat, und genau darin liegt der Wert des Feldes.
    assert!(
        !plan.zonendivers,
        "zwei gemischte Pods koennen kein zonendiverses Paar bilden"
    );
    b.sagt(
        "CONSENSUS",
        "Paar nicht zonendivers, und der Plan sagt es: in dieser Besetzung \
         gibt es kein diverses Paar"
            .to_string(),
    );

    // ---- 5. Das Paar liefert übereinstimmend ab ---------------------
    let zuweisung = plan.zuweisungen[&a];
    let charge = k
        .zustand()
        .korpus
        .as_ref()
        .expect("Korpus")
        .charge(zuweisung.start, zuweisung.laenge);
    let epoche = k.zustand().epoch.0;
    let vor_fassung = k.zustand().modell_version;
    for nr in [a, c] {
        let pod = zuteilung.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
        let w = schluessel_von(pod.shards[0].miner.miner_id);
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::TrainingssegmentEinreichen {
                    pod: pod_kennung(epoche, nr),
                    segment: segment_bauen(pod, charge, Hash([5u8; 32])),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    k.baue_block();
    assert_eq!(k.zustand().trainingssegmente.len(), 2, "beide Segmente fehlen");
    b.sagt("TRAINING", "beide Pods des Paars haben abgeliefert");

    // ---- 6. Ein Betrugsversuch: fremde Charge ----------------------
    let pod_a = zuteilung.pods.iter().find(|p| p.pod_index == a).expect("Pod");
    let w = schluessel_von(pod_a.shards[0].miner.miner_id);
    let vorher = k.zustand().trainingssegmente.len();
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(w),
            nonce[w as usize],
            Anweisung::TrainingssegmentEinreichen {
                pod: pod_kennung(epoche, a),
                segment: segment_bauen(pod_a, Hash([99u8; 32]), Hash([5u8; 32])),
            },
        )
        .expect("signieren"),
    );
    nonce[w as usize] += 1;
    k.baue_block();
    assert_eq!(
        k.zustand().trainingssegmente.len(),
        vorher,
        "ein Segment mit fremder Charge kam durch"
    );
    b.sagt("VERIFICATION", "ein Segment mit fremder Charge wurde abgewiesen");

    // ---- 6b. Ein Segment, das nichts bewegt hat -------------------
    //
    // ⚑ **Kein Betrugsversuch, und genau das ist der Punkt** (Fund
    // 191). Der Pod hat ehrlich gerechnet und ist ehrlich auf null
    // gekommen, weil die Lernrate für dieses Modell unter der Auflösung
    // der Übertragungsform lag. Vor dem 2026-09-06 hätte ein zweiter
    // Pod dasselbe gemeldet, das Paar wäre einig gewesen, und beide
    // wären dafür bezahlt worden, dass sich kein Gewicht bewegte.
    let zuweisung_a = plan.zuweisungen[&a];
    let echte_charge = k
        .zustand()
        .korpus
        .as_ref()
        .expect("Korpus")
        .charge(zuweisung_a.start, zuweisung_a.laenge);
    let vorher = k.zustand().trainingssegmente.len();
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(w),
            nonce[w as usize],
            Anweisung::TrainingssegmentEinreichen {
                pod: pod_kennung(epoche, a),
                segment: segment_mit_wirkung(pod_a, echte_charge, Hash([6u8; 32]), 0),
            },
        )
        .expect("signieren"),
    );
    nonce[w as usize] += 1;
    k.baue_block();
    assert_eq!(
        k.zustand().trainingssegmente.len(),
        vorher,
        "ein Segment ohne bewegte Gewichte kam durch"
    );
    b.sagt(
        "VERIFICATION",
        "ein Segment ohne bewegte Gewichte wurde abgewiesen, obwohl es ehrlich gerechnet war",
    );

    // ---- 7. Epochenwechsel: Fassung, Vergütung, Abgabe --------------
    let beobachtet = probekonto(schluessel_von(pod_a.shards[1].miner.miner_id));
    let vor_geld = k.zustand().account(&beobachtet).balance;
    let vor_treasury = k.zustand().account(&myl_types::treasury::treasury_adresse()).balance;
    epoche_weiter(&mut k);

    assert_ne!(k.zustand().modell_version, vor_fassung, "die Modellfassung stand still");
    assert!(
        k.zustand().account(&beobachtet).balance > vor_geld,
        "der Trainingsminer wurde nicht bezahlt"
    );
    assert!(k.zustand().trainingssegmente.is_empty(), "die Segmente blieben liegen");
    let nach_treasury = k.zustand().account(&myl_types::treasury::treasury_adresse()).balance;
    b.sagt(
        "TOKENOMICS",
        format!(
            "Modellfassung geruecktes, Miner bezahlt (+{}), Treasury {vor_treasury} -> {nach_treasury}",
            k.zustand().account(&beobachtet).balance - vor_geld
        ),
    );

    // ---- 7b. Die Nachfrage ist in die Vorepoche gerollt -------------
    //
    // ⚑ **Das ist die Verdrahtung, um die es hier geht.** Was in einer
    // Epoche ausgegeben wurde, steht in der nächsten als Auslastung zur
    // Verfügung, und daraus folgen Trainingsanteil und Abgabe.
    assert_eq!(
        k.zustand().vtfe_vorepoche,
        18_000,
        "die Nachfrage ist nicht in die Vorepoche gerollt"
    );
    assert_eq!(k.zustand().vtfe_epoche, 0, "der laufende Zaehler faengt bei null an");

    // ⚑ **Und die Auslastung bleibt trotzdem null, mit gutem Grund.**
    // `PodKapazitaet` ist eine Zahl für ein echtes Netz: 3,6 Mrd. vTFE
    // je Pod und Epoche. Achtzehntausend gegen 7,2 Milliarden sind auf
    // der Festkommaskala exakt null.
    //
    // **Das ist kein Fehler der Simulation, sondern ihre Grenze**, und
    // sie gehört benannt: Eine Probekette mit zwölf Minern kann keine
    // Auslastung erzeugen, die gegen einen Netzparameter zählt. Die
    // Regel selbst ist an ihren eigenen Tests geprüft; hier wird die
    // **Verdrahtung** geprüft, also dass die Zahl ankommt.
    let plan_danach = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
    assert_eq!(plan_danach.auslastung, 0, "bei Testgroesse ist sie null");
    b.sagt(
        "TOKENOMICS",
        format!(
            "Nachfrage in die Vorepoche gerollt ({} vTFE); Auslastung bleibt 0, \
             weil die Kapazitaet {} vTFE ist",
            k.zustand().vtfe_vorepoche,
            plan_danach.kapazitaet
        ),
    );

    // ⚑ **Die Rückkopplung selbst, an der Regel gezeigt.** Was bei
    // echter Auslastung geschähe, lässt sich hier nur rechnen, nicht
    // fahren; die Zahlen stehen als Beleg, nicht als Behauptung.
    {
        use myl_tokenomics::trainingsabgabe::{abgabe_bps, ABGABE_MAX_BPS};
        let skala = myl_types::auslastung::AUSLASTUNG_SKALA;
        let bei = |anteil: i64| {
            let u = skala * anteil / 100;
            (
                myl_scheduler::trainingszuteilung::anteil_bps(
                    u,
                    &myl_scheduler::trainingszuteilung::Trainingsraten {
                        grundrate_bps: 200,
                        freianteil_bps: 8_000,
                    },
                ),
                abgabe_bps(u, ABGABE_MAX_BPS),
            )
        };
        let (t100, a100) = bei(100);
        let (t70, a70) = bei(70);
        let (t0, a0) = bei(0);
        assert!(t100 < t70 && t70 < t0, "der Trainingsanteil steigt nicht bei fallender Last");
        assert!(a100 > a70 && a70 > a0, "die Abgabe faellt nicht bei fallender Last");
        b.sagt(
            "TOKENOMICS",
            format!(
                "Rueckkopplung: bei 100/70/0 Prozent Last Training {t100}/{t70}/{t0} bp, \
                 Abgabe {a100}/{a70}/{a0} bp"
            ),
        );
    }

    // ---- 8. Ein uneiniges Paar bewegt nichts ------------------------
    let zuteilung2 = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let plan2 = Kette::trainingsplan_der_laufenden_epoche(k.zustand());
    let epoche2 = k.zustand().epoch.0;
    let fassung_vor = k.zustand().modell_version;
    if let Some(&(x, y)) = plan2.paare.first() {
        let zw = plan2.zuweisungen[&x];
        let ch = k.zustand().korpus.as_ref().expect("Korpus").charge(zw.start, zw.laenge);
        for (nr, commitment) in [(x, Hash([7u8; 32])), (y, Hash([8u8; 32]))] {
            let pod = zuteilung2.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
            let w = schluessel_von(pod.shards[0].miner.miner_id);
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    nonce[w as usize],
                    Anweisung::TrainingssegmentEinreichen {
                        pod: pod_kennung(epoche2, nr),
                        segment: segment_bauen(pod, ch, commitment),
                    },
                )
                .expect("signieren"),
            );
            nonce[w as usize] += 1;
        }
        k.baue_block();
        epoche_weiter(&mut k);
        assert_eq!(
            k.zustand().modell_version,
            fassung_vor,
            "ein uneiniges Paar hat die Modellfassung bewegt"
        );
        b.sagt("VERIFICATION", "ein uneiniges Paar bewegt die Modellfassung nicht");
    }

    // ---- 9. Slashing: falsch gerechnet, Einsatz weg -----------------
    //
    // ⚑ **Auf einer Kopie des Zustands, und das gehört gesagt.** Ein
    // geschnittener Miner fiele aus der Zuteilung, und die Stationen
    // danach prüften dann eine andere Besetzung als die davor. Was hier
    // geprüft wird, ist der Übergang selbst auf einem **echten**
    // Kettenzustand mit echten Konten, nicht die Rückwirkung des
    // Schnitts auf die Podbildung. Die steht offen.
    let mut zustand: LedgerState = k.zustand().clone();
    let schuldig = probekonto(3);
    zustand.account_mut(&schuldig).staked = 100_000_000;
    let unschuldig = probekonto(4);
    let verdict = myl_ledger::transitions::Verdict {
        segment_id: SegmentId::new([9u8; 32]),
        miner: schuldig,
        checker: unschuldig,
        outcome: myl_ledger::transitions::VerdictOutcome::SlashMiner,
    };
    let params = SlashParams {
        slash_fraction_num: 3,
        slash_fraction_den: 10,
        bounty_fraction_num: 1,
        bounty_fraction_den: 10,
    };
    let wirkung = apply_verdict(&mut zustand, &verdict, &params).expect("Urteil");
    assert_eq!(wirkung.slashed, 30_000_000, "dreissig Prozent von hundert Millionen");
    assert_eq!(wirkung.bounty, 3_000_000, "zehn Prozent davon als Kopfgeld");
    assert_eq!(zustand.account(&schuldig).staked, 70_000_000);
    b.sagt(
        "VERIFICATION",
        format!("Slashing {} MYL, Kopfgeld {}", wirkung.slashed, wirkung.bounty),
    );

    // ---- 10. Was berührt wurde -------------------------------------
    //
    // ⚑ **Gegen eine feste Liste und nicht gegen eine Untergrenze.**
    // Ein `>= 8` hätte auch dann zugestimmt, wenn fünf Stationen
    // stillschweigend ausgefallen wären, und genau das ist die Sorte
    // Zusicherung, die dieses Projekt sonst aussortiert. Fällt eine
    // Station weg, muss dieser Test fallen.
    eprintln!("\n--- Verzahnte Simulation: {} Stationen ---", b.zeilen.len());
    let erwartet = [
        "zwoelf Miner in drei Zonen angemeldet",
        "aus dem Sammeltopf",
        "Burn",
        "Nachfrage",
        "Trainingsanteil",
        "nicht zonendivers",
        "beide Pods des Paars haben abgeliefert",
        "fremder Charge",
        "ohne bewegte Gewichte",
        "Modellfassung geruecktes",
        "in die Vorepoche gerollt",
        "Rueckkopplung",
        "uneiniges Paar",
        "Slashing",
    ];
    for teil in erwartet {
        assert!(
            b.zeilen.iter().any(|z| z.contains(teil)),
            "die Station {teil:?} fehlt im Protokoll"
        );
    }
    assert_eq!(
        b.zeilen.len(),
        erwartet.len(),
        "die Zahl der Stationen weicht ab; steht eine neue da, gehoert sie in die Liste"
    );
}
