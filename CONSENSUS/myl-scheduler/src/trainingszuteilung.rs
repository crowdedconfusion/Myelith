//! Wie viel des Netzes trainiert, und welche Pods (Whitepaper Kap. 7.1).
//!
//! # ⚑ Die eine Regel, und woher ihre beiden Hälften kommen
//!
//! Kap. 7.1 sagt: Die Trainingsmenge bemisst sich an der **freien**
//! Kapazität, nicht an der Gesamtleistung, und sie wird gedrosselt,
//! bevor die Credit-Preise anziehen. „Damit ist ausgeschlossen, dass
//! Training Inferenzkapazität verdrängt."
//!
//! Wörtlich gelesen heisst das: **bei voller Auslastung trainiert
//! niemand.** Training ist dann ein Rest, und ein Netz, das Erfolg hat,
//! hört auf, sein Modell zu verbessern.
//!
//! Deshalb hat die Regel hier zwei Summanden:
//!
//! ```text
//! anteil = grundrate + freianteil · frei
//! ```
//!
//! | Auslastung | frei | Anteil bei 200 / 8000 bp |
//! |---|---|---|
//! | 100 % | 0 % | **2,0 %** |
//! | 80 % | 20 % | 18,0 % |
//! | 40 % | 60 % | 50,0 % |
//! | 0 % | 100 % | **82,0 %** |
//!
//! # ⚑ Der Freianteil ist 8000 und nicht 1000, und warum nicht 10000
//!
//! Der erste Entwurf nahm γ_train aus Kap. 7.1 wörtlich („fünf bis zehn
//! Prozent der freien Kapazität") und kam bei leerem Netz auf zwölf
//! Prozent. **Das war falsch herum gedacht:** Ein Netz ohne Nachfrage
//! hat nichts anderes zu tun. Miner, die dastehen, sollen trainieren.
//!
//! ⚑ **Und trotzdem nicht 10000.** Die Auslastung ist die der
//! **Vorepoche**, gemessen über eine Stunde. Steigt die Nachfrage
//! innerhalb der laufenden Epoche, können die Trainingspods sie nicht
//! bedienen: Sie rechnen etwas anderes. Die fehlenden zwanzig Prozent
//! der freien Kapazität sind **Luft für Nachfragewachstum**, kein
//! Rundungsrest.
//!
//! **Die zwanzig sind nicht gemessen.** Wie viel Luft nötig ist, hängt
//! an der Schwankung der Nachfrage über eine Epoche, und die kennt
//! niemand. Deshalb steht die Zahl als Parameter und nicht als
//! Konstante.
//!
//! ⚑ **Die Grundrate ist eine Abweichung vom Whitepaper und als solche
//! gekennzeichnet.** Sie steht als
//! `myl_governance::Parameter::TrainingsGrundrate` mit der Herkunft
//! „Entwurf", und **null stellt Kap. 7.1 wörtlich wieder her**. Wer die
//! Verdrängungsfreiheit buchstäblich will, setzt sie auf null; dann
//! bleibt genau γ_train.
//!
//! # ⚑ Warum kein Pod sich das aussuchen darf
//!
//! Trainingsvergütung ist auf 70 Prozent der Inferenzvergütung gedeckelt
//! (`myl_tokenomics::training_reward_cap`, Kap. 5.6). Ein Pod, der
//! wählen dürfte, wählte Inferenz, und zwar **jeder**. Die Zuteilung
//! wäre damit nicht bloss verzerrt, sondern leer.
//!
//! Die Auswahl ergibt sich deshalb aus der Epochensaat, genau wie die
//! Podbildung selbst und wie die Korpuszuweisung in
//! `myl_train::zuweisung`. Dort steht dasselbe Argument für den
//! Datenfall: „Wer keine Daten fälschen kann, kann immer noch
//! **auswählen**."
//!
//! ⚑ **Und die Saat kommt aus `e−2`**, ist also bei der Anmeldung noch
//! nicht bekannt. Ein Miner kann sich weder ins Training hinein noch
//! heraus stellen.
//!
//! # ⚑ Reihum, und warum das Gedächtnis am Miner hängt und nicht am Pod
//!
//! „Wer zuletzt trainiert hat, kommt zuletzt wieder dran." Der
//! naheliegende Satz dazu lautet „ein Pod, der zuletzt trainiert hat,
//! wird nicht gewählt", und er hat **keinen Gegenstand**: Pods werden
//! **jede Epoche neu gebildet**. Pod 5 der Epoche 100 und Pod 5 der
//! Epoche 101 sind verschiedene Leute.
//!
//! Was über Epochen hinweg besteht, ist der **Miner**. Das Gedächtnis
//! hängt deshalb an ihm ([`Trainingsstand`]), und die Frische eines Pods
//! ist die seines **zuletzt** herangezogenen Mitglieds.
//!
//! ## Reihum statt Ausschluss
//!
//! Ein Ausschluss („wer letzte Epoche trainiert hat, ist gesperrt")
//! bricht bei hohem Anteil zusammen: Bei 82 Prozent hätten fast alle
//! trainiert, fast kein Pod wäre wählbar, und das Training pendelte
//! zwischen 82 Prozent und null.
//!
//! ⚑ **Deshalb wird geordnet und nicht ausgeschlossen.** Wer nie
//! trainiert hat, steht vorn; danach der am längsten Zurückliegende;
//! zuletzt, wer gerade dran war. Gleichstand entscheidet das Los. Das
//! liefert **immer** genau so viele Pods wie gebraucht und rotiert von
//! selbst.
//!
//! ## ⚑ Vorhersagbar, und das ist hier richtig
//!
//! Die Ordnung ist berechenbar: Wer lange nicht dran war, weiss, dass er
//! bald drankommt. Das war beim Zufall anders, und es ist trotzdem kein
//! Rückschritt.
//!
//! Der Zweck der Unvorhersagbarkeit war, dass niemand dem Training
//! **ausweicht**. Reihum kann niemand ausweichen: Wer sich abmeldet und
//! neu anmeldet, gilt als „nie trainiert" und steht damit **ganz vorn**.
//! Der Ausweichversuch beschleunigt die eigene Heranziehung.
//!
//! # ⚑ Die Nachfrage ist die der **Vorepoche**, und das ist keine Wahl
//!
//! Die Pod-Zuteilung einer Epoche steht fest, **bevor** die Epoche
//! läuft. Wer sie aus der Nachfrage derselben Epoche ableitete, bräuchte
//! eine Zahl, die es noch nicht gibt. Dasselbe Kausalitätsargument wie
//! beim Lastausgleich des MoE-Routers, der die vorige Charge benutzt.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::shard_assignment::Zuteilung;
use myl_types::ids::MinerId;

/// Basispunktbasis (100 Prozent).
pub const BPS: u64 = 10_000;

/// Die beiden Raten, aus denen der Trainingsanteil entsteht.
///
/// Beide kommen aus der Parameter-Registry und stehen hier nur als
/// Argument: Eine zweite Stelle, die sie festlegt, wäre eine zweite
/// Wahrheit über dieselbe Grösse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trainingsraten {
    /// Der Sockel, in Basispunkten der **Gesamtkapazität**.
    pub grundrate_bps: u32,
    /// γ_train, in Basispunkten der **freien** Kapazität.
    pub freianteil_bps: u32,
}

impl Trainingsraten {
    /// Kap. 7.1 wörtlich: kein Sockel, nur γ_train, und γ_train aus dem
    /// dort genannten Bereich statt aus dem Auslastungsgedanken.
    ///
    /// ⚑ **Nicht die Vorgabe, sondern die Gegenprobe.** Ein Test hält
    /// damit fest, dass die Abweichung eine Abweichung ist und sich
    /// abschalten lässt.
    pub const fn nach_whitepaper(freianteil_bps: u32) -> Self {
        Self { grundrate_bps: 0, freianteil_bps }
    }
}

/// Das Ergebnis: wer trainiert, und wie die Zahl zustande kam.
///
/// ⚑ **Die Herleitung gehört ins Ergebnis.** Eine Menge von Pod-Nummern
/// allein liesse sich nicht prüfen, ohne die Rechnung zu wiederholen;
/// wer sie mitliefert, macht den Streitfall entscheidbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trainingsplan {
    /// Die Nummern der Pods, die in dieser Epoche trainieren.
    pub pods: BTreeSet<u32>,
    /// Die gemessene Auslastung der Vorepoche, `1 << 16` = 100 Prozent.
    pub auslastung: i64,
    /// Der daraus gefolgerte Anteil in Basispunkten.
    pub anteil_bps: u32,
    /// Die verfügbare Kapazität, gegen die gemessen wurde.
    pub kapazitaet: u64,
    /// Welches Korpusbündel jeder Trainingspod bearbeitet.
    ///
    /// # ⚑ Leer heisst: Das Netz hat keinen Korpus
    ///
    /// Ein Netz kann Trainingspods bilden und trotzdem nichts zu
    /// trainieren haben, wenn kein
    /// [`Korpusanker`](myl_types::korpusanker::Korpusanker) im Zustand
    /// steht. **Das steht dann hier als leere Karte** und nicht in einem
    /// Protokoll: Ein Plan, der Pods nennt und ihnen nichts zuweist,
    /// wäre eine Falle für jeden Aufrufer.
    ///
    /// ⚑ **Der Pod wählt sein Bündel nicht.** Wer keine Daten fälschen
    /// kann, kann immer noch **auswählen**; ein Angreifer mit vierzig
    /// Prozent Kapazitätsanteil hätte bei freier Wahl vierzig Prozent
    /// Einfluss auf die Datenzusammensetzung, und jedes einzelne Segment
    /// wäre dabei echt.
    pub zuweisungen: BTreeMap<u32, myl_train::zuweisung::Zuweisung>,
}

/// Der Trainingsanteil in Basispunkten, aus der Auslastung.
///
/// `auslastung` ist der Festkommawert aus
/// [`myl_types::auslastung::auslastung`]
/// (`AUSLASTUNG_SKALA = 1 << 16`).
///
/// ⚑ **Eine Auslastung über hundert Prozent ist möglich** und heisst,
/// dass mehr nachgefragt als geliefert wurde. Die freie Kapazität ist
/// dann null und nicht negativ; ein negativer Freianteil zöge sonst am
/// Sockel, und Übernachfrage senkte das Training unter den Sockel, den
/// es garantieren soll.
pub fn anteil_bps(auslastung: i64, raten: &Trainingsraten) -> u32 {
    const SKALA: i64 = myl_types::auslastung::AUSLASTUNG_SKALA;
    let frei = (SKALA - auslastung).clamp(0, SKALA) as u64;
    let zuschlag = (u64::from(raten.freianteil_bps) * frei) / SKALA as u64;
    // Der Deckel ist die volle Kapazität: mehr als alles geht nicht.
    let summe = u64::from(raten.grundrate_bps).saturating_add(zuschlag);
    summe.min(BPS) as u32
}

/// Wie viele der `pods` in dieser Epoche trainieren.
///
/// # ⚑ Kaufmännisch gerundet, nicht abgerundet
///
/// Der erste Entwurf rundete ab, mit dem Argument: Bei zehn Pods und
/// vier Prozent wäre ein einzelner Trainingspod zehn Prozent des Netzes,
/// also das Dreifache dessen, was Anhang B.7.2 verträgt.
///
/// **Das Argument trägt bei kleinem Anteil und kippt bei grossem.** Bei
/// 82 Prozent und **einem** Pod ergibt Abrunden null: Ein Netz mit einem
/// einzigen Pod träfe nie ein Training, auch wenn es vollständig
/// leerläuft. Null liegt dann weiter vom Ziel entfernt als eins.
///
/// Runden zur nächsten Zahl hält beides:
///
/// | Pods | Anteil | abgerundet | gerundet |
/// |---|---|---|---|
/// | 10 | 2 % | 0 | **0** |
/// | 10 | 4 % | 0 | **0** |
/// | 25 | 4 % | 1 | **1** |
/// | 1 | 82 % | **0** | **1** |
///
/// Der Schutz kleiner Netze vor Selbstüberschätzung bleibt also, und ein
/// leerlaufendes Netz trainiert trotzdem.
pub fn anzahl(pods: usize, auslastung: i64, raten: &Trainingsraten) -> usize {
    runden(pods, anteil_bps(auslastung, raten))
}

/// `pods · bps / 10000`, zur nächsten ganzen Zahl.
fn runden(pods: usize, bps: u32) -> usize {
    ((pods as u64 * u64::from(bps) + BPS / 2) / BPS) as usize
}

/// Wann ein Miner zuletzt zum Training herangezogen wurde.
///
/// Fehlt er, hat er nie trainiert, und dann steht er ganz vorn.
///
/// ⚑ **Am Miner und nicht am Pod**, siehe den Modulkopf: Pods werden
/// jede Epoche neu gebildet, der Miner besteht.
pub type Trainingsstand = BTreeMap<MinerId, u64>;

/// Wie frisch ein Pod ist: die Epoche seines **zuletzt** herangezogenen
/// Mitglieds.
///
/// `None` heisst, dass **kein** Mitglied je trainiert hat. `None` ordnet
/// vor jedem `Some`, ein solcher Pod kommt also zuerst dran.
///
/// ⚑ **Das Maximum und nicht der Durchschnitt.** Ein Pod, in dem einer
/// gerade dran war, soll warten, auch wenn die anderen fünf frisch sind:
/// Sonst liesse sich ein Vielrechner immer wieder mit Neulingen
/// zusammen nach vorn tragen.
pub fn frische(pod: &crate::shard_assignment::Pod, stand: &Trainingsstand) -> Option<u64> {
    pod.mitglieder().filter_map(|m| stand.get(&m.miner_id).copied()).max()
}

/// Welche Pods trainieren: reihum, Gleichstand nach Los.
///
/// Ordnet nach `(frische, los)` aufsteigend und nimmt die ersten
/// `anzahl`. Wer nie trainiert hat, steht vorn; wer gerade dran war,
/// hinten.
///
/// ⚑ **Das Los ist eigen abgeleitet und nicht die Saat der Podbildung.**
/// Nähme es dieselbe, hinge die Trainingsauswahl an derselben
/// Permutation wie die Shard-Positionen, und wer seine Position kennt,
/// wüsste etwas über seine Trainingslast.
pub fn trainingspods(
    pods: &[crate::shard_assignment::Pod],
    anzahl: usize,
    stand: &Trainingsstand,
    saat: &[u8; 32],
) -> BTreeSet<u32> {
    if anzahl == 0 || pods.is_empty() {
        return BTreeSet::new();
    }
    let los = trainingssaat(saat);
    let mut reihe: Vec<(Option<u64>, [u8; 32], u32)> = pods
        .iter()
        .map(|p| (frische(p, stand), losnummer(&los, p.pod_index), p.pod_index))
        .collect();
    // `Option<u64>` ordnet `None` vor jedem `Some`, und das ist genau
    // die gewollte Reihenfolge: nie trainiert kommt zuerst.
    reihe.sort_unstable();
    // ⚑ **In eine geordnete Menge**, nicht in die Ziehungsreihenfolge:
    // Das Ergebnis ist eine Konsensgrösse, und zwei Knoten müssen
    // dieselbe Bytefolge sehen.
    reihe.into_iter().take(anzahl.min(pods.len())).map(|(_, _, n)| n).collect()
}

/// Das Los eines Pods: eine Zahl aus Saat und Podnummer.
fn losnummer(los: &[u8; 32], pod_index: u32) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MYELITH_TRAININGSLOS_v1");
    h.update(los);
    h.update(pod_index.to_le_bytes());
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

/// Schreibt den Stand fort: wer in dieser Epoche herangezogen wurde.
///
/// # ⚑ Herangezogen und nicht abgeliefert
///
/// Vermerkt wird die **Zuweisung**, nicht die erbrachte Leistung. Ein
/// Pod, der zugewiesen war und scheiterte, soll nicht sofort wieder
/// vorn stehen; sonst wäre Scheitern ein Weg, immer wieder gewählt zu
/// werden, und bei gedeckelter Trainingsvergütung will das niemand.
///
/// **Ob er geliefert hat, entscheidet die Vergütung**, nicht die
/// Reihenfolge.
pub fn stand_fortschreiben(
    stand: &mut Trainingsstand,
    pods: &[crate::shard_assignment::Pod],
    gewaehlt: &BTreeSet<u32>,
    epoche: u64,
) {
    for pod in pods.iter().filter(|p| gewaehlt.contains(&p.pod_index)) {
        for m in pod.mitglieder() {
            stand.insert(m.miner_id, epoche);
        }
    }
}

/// Der ganze Plan einer Epoche.
///
/// `nachfrage_vtfe` ist die Nachfrage der **Vorepoche**
/// (`LedgerState::vtfe_vorepoche`), `kapazitaet_je_pod` der
/// Governance-Parameter `PodKapazitaet`.
pub fn plane(
    zuteilung: &Zuteilung,
    nachfrage_vtfe: u64,
    kapazitaet_je_pod: u64,
    raten: &Trainingsraten,
    stand: &Trainingsstand,
    korpus: Option<&myl_types::korpusanker::Korpusanker>,
    saat: &[u8; 32],
) -> Trainingsplan {
    let pods = zuteilung.pods.len();
    let kapazitaet = (pods as u64).saturating_mul(kapazitaet_je_pod);
    let auslastung = myl_types::auslastung::auslastung(nachfrage_vtfe, kapazitaet);
    let bps = anteil_bps(auslastung, raten);
    let n = runden(pods, bps);
    let gewaehlt = trainingspods(&zuteilung.pods, n, stand, saat);
    Trainingsplan {
        zuweisungen: buendel_zuweisen(&gewaehlt, korpus, saat),
        pods: gewaehlt,
        auslastung,
        anteil_bps: bps,
        kapazitaet,
    }
}

/// Weist jedem gewählten Pod sein Korpusbündel zu.
///
/// ⚑ **Der erste Aufrufer von `myl-train`** (Fund 183, 2026-09-05). Die
/// Kiste war gebaut, geprüft und unerreicht, und der Grund war genau
/// diese fehlende Stelle: `zuweisen` beantwortet die Frage „welches
/// Bündel bekommt Pod *i*?", und niemand stellte sie.
///
/// ⚑ **Ein Fehler der Zuweisung lässt den Pod aus, statt zu raten.** Ist
/// der Korpus kleiner als ein Bündel, gibt es nichts zuzuweisen; ein
/// gekürztes Bündel wäre einfach und falsch, weil Bündel verschiedener
/// Grösse verschieden viel Arbeit tragen und die Vergütung dann an der
/// Position im Korpus hinge.
fn buendel_zuweisen(
    gewaehlt: &BTreeSet<u32>,
    korpus: Option<&myl_types::korpusanker::Korpusanker>,
    saat: &[u8; 32],
) -> BTreeMap<u32, myl_train::zuweisung::Zuweisung> {
    let Some(k) = korpus.filter(|k| k.traegt_ein_buendel()) else {
        return BTreeMap::new();
    };
    gewaehlt
        .iter()
        .filter_map(|nr| {
            myl_train::zuweisung::zuweisen(saat, &k.kennung, k.segmente, k.buendel, u64::from(*nr))
                .ok()
                .map(|z| (*nr, z))
        })
        .collect()
}

/// Die abgeleitete Saat der Trainingsauswahl.
fn trainingssaat(saat: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MYELITH_TRAININGSPODS_v1");
    h.update(saat);
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shard_assignment::{Pod, Shard};
    use myl_types::node_metadata::GeoRegion;

    const SKALA: i64 = myl_types::auslastung::AUSLASTUNG_SKALA;

    fn raten() -> Trainingsraten {
        Trainingsraten { grundrate_bps: 200, freianteil_bps: 8_000 }
    }

    fn miner(b: u16) -> crate::miner_filter::MinerRegistration {
        let mut bytes = [0u8; 32];
        bytes[..2].copy_from_slice(&b.to_le_bytes());
        crate::miner_filter::MinerRegistration {
            miner_id: MinerId::new(bytes),
            hardware_class: crate::miner_filter::HardwareClass::MediumGpu,
            registration_epoch: 0,
            zone: GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        }
    }

    /// `n` Pods zu je vier Mitgliedern, fortlaufend besetzt.
    fn pods(n: u32) -> Vec<Pod> {
        (0..n)
            .map(|i| Pod {
                pod_index: i,
                shards: (0..4)
                    .map(|j| Shard {
                        shard_index: j,
                        miner: miner((i * 4 + j) as u16),
                    })
                    .collect(),
                reserve: Vec::new(),
            })
            .collect()
    }

    // ------------------------------------------------------ die Kurve

    /// Die Tabelle aus dem Modulkopf, Zeile für Zeile.
    #[test]
    fn der_anteil_folgt_der_auslastung() {
        let r = raten();
        assert_eq!(anteil_bps(SKALA, &r), 200, "voll ausgelastet: nur der Sockel");
        assert_eq!(anteil_bps(SKALA * 4 / 5, &r), 1_800, "80 Prozent");
        assert_eq!(anteil_bps(SKALA * 2 / 5, &r), 5_000, "40 Prozent");
        assert_eq!(anteil_bps(0, &r), 8_200, "leerlaufend");
    }

    /// ⚑ **Die Anforderung in einem Satz:** Ein leerlaufendes Netz
    /// trainiert mit dem Grossteil seiner Pods, nicht mit einer
    /// Handvoll.
    #[test]
    fn ein_leerlaufendes_netz_trainiert_mit_dem_grossteil() {
        assert_eq!(anzahl(1_000, 0, &raten()), 820);
    }

    /// ⚑ **Und trotzdem bleibt Luft.** Bei null Nachfrage stehen
    /// achtzehn Prozent bereit, falls die Nachfrage innerhalb der Epoche
    /// anzieht.
    #[test]
    fn es_bleibt_luft_fuer_nachfragewachstum() {
        let bps = anteil_bps(0, &raten());
        assert!(bps < 10_000, "das Netz trainiert restlos und kann nichts mehr bedienen");
        assert_eq!(10_000 - bps, 1_800);
    }

    /// Weniger Inferenz bedeutet mehr Training, nie weniger.
    #[test]
    fn sinkende_auslastung_hebt_den_anteil_nie_senkt_ihn() {
        let r = raten();
        let mut vorher = anteil_bps(SKALA, &r);
        for teil in (0..=20).rev() {
            let jetzt = anteil_bps(SKALA * teil / 20, &r);
            assert!(jetzt >= vorher, "bei {teil}/20 faellt der Anteil von {vorher} auf {jetzt}");
            vorher = jetzt;
        }
    }

    /// Übernachfrage darf nicht unter den Sockel drücken.
    #[test]
    fn ueber_hundert_prozent_bleibt_der_sockel_stehen() {
        let r = raten();
        assert_eq!(anteil_bps(SKALA * 3, &r), 200);
        assert_eq!(anteil_bps(i64::MAX, &r), 200);
    }

    /// ⚑ **Die Gegenprobe zur Abweichung.** Ohne Sockel gilt Kap. 7.1
    /// wörtlich: volle Auslastung, kein Training.
    #[test]
    fn ohne_sockel_gilt_das_whitepaper_woertlich() {
        let r = Trainingsraten::nach_whitepaper(1_000);
        assert_eq!(anteil_bps(SKALA, &r), 0, "voll ausgelastet: gar kein Training");
        assert_eq!(anteil_bps(SKALA * 7 / 10, &r), 300, "bei 70 Prozent rund drei Prozent");
    }

    /// Die Tabelle aus [`anzahl`], Zeile für Zeile.
    #[test]
    fn gerundet_und_nicht_abgerundet() {
        let r = raten();
        assert_eq!(anzahl(3, SKALA, &r), 0, "zwei Prozent von drei ist null");
        assert_eq!(anzahl(10, SKALA * 4 / 5, &r), 2, "achtzehn Prozent von zehn");
        assert_eq!(anzahl(1_000, SKALA, &r), 20);
    }

    /// ⚑ **Der Fall, an dem Abrunden falsch war.** Ein Netz mit einem
    /// einzigen Pod, vollständig leerlaufend, muss trainieren.
    #[test]
    fn ein_einziger_pod_trainiert_wenn_das_netz_leerlaeuft() {
        let r = raten();
        assert_eq!(anzahl(1, 0, &r), 1, "ein leerlaufendes Netz trainiert nicht");
        assert_eq!(anzahl(1, SKALA, &r), 0, "voll ausgelastet trainiert es doch");
    }

    // -------------------------------------------------- die Rotation

    /// Ohne Gedächtnis entscheidet das Los, und es ist deterministisch.
    #[test]
    fn ohne_gedaechtnis_entscheidet_das_los() {
        let p = pods(100);
        let leer = Trainingsstand::new();
        let a = trainingspods(&p, 4, &leer, &[7u8; 32]);
        let b = trainingspods(&p, 4, &leer, &[7u8; 32]);
        let c = trainingspods(&p, 4, &leer, &[8u8; 32]);
        assert_eq!(a, b, "dieselbe Saat, dieselbe Auswahl");
        assert_eq!(a.len(), 4);
        assert_ne!(a, c, "eine andere Saat waehlt anders");
    }

    /// ⚑ **Die Anforderung: Wer gerade dran war, kommt nicht sofort
    /// wieder.**
    #[test]
    fn wer_zuletzt_trainiert_hat_kommt_zuletzt_wieder() {
        let p = pods(10);
        let mut stand = Trainingsstand::new();
        // Die Pods 0 bis 4 waren in Epoche 5 dran.
        let vorher: BTreeSet<u32> = (0..5).collect();
        stand_fortschreiben(&mut stand, &p, &vorher, 5);

        let jetzt = trainingspods(&p, 5, &stand, &[1u8; 32]);
        assert_eq!(
            jetzt,
            (5..10).collect::<BTreeSet<u32>>(),
            "die frischen Pods wurden nicht bevorzugt"
        );
    }

    /// ⚑ **Und der Reihum-Betrieb schliesst sich.** Über sechs Epochen
    /// mit je zwei von sechs Pods kommt jeder genau zweimal dran.
    #[test]
    fn ueber_sechs_epochen_kommt_jeder_gleich_oft_dran() {
        let p = pods(6);
        let mut stand = Trainingsstand::new();
        let mut wie_oft = [0u32; 6];
        for e in 1..=6u64 {
            let mut saat = [0u8; 32];
            saat[0] = e as u8;
            let gewaehlt = trainingspods(&p, 2, &stand, &saat);
            assert_eq!(gewaehlt.len(), 2);
            for n in &gewaehlt {
                wie_oft[*n as usize] += 1;
            }
            stand_fortschreiben(&mut stand, &p, &gewaehlt, e);
        }
        assert_eq!(wie_oft, [2; 6], "die Reihe ist schief: {wie_oft:?}");
    }

    /// ⚑ **Der Ausschluss waere hier zusammengebrochen.** Bei hohem
    /// Anteil haben fast alle zuletzt trainiert; die Ordnung liefert
    /// trotzdem genau so viele Pods wie verlangt.
    #[test]
    fn bei_hohem_anteil_bricht_die_auswahl_nicht_zusammen() {
        let p = pods(10);
        let mut stand = Trainingsstand::new();
        let alle: BTreeSet<u32> = (0..10).collect();
        stand_fortschreiben(&mut stand, &p, &alle, 5);
        let jetzt = trainingspods(&p, 8, &stand, &[1u8; 32]);
        assert_eq!(jetzt.len(), 8, "die Auswahl ist eingebrochen");
    }

    /// ⚑ **Wer sich neu anmeldet, steht ganz vorn.** Damit ist die
    /// Abmeldung kein Weg, dem Training auszuweichen, sondern das
    /// Gegenteil.
    #[test]
    fn eine_neue_kennung_wird_zuerst_herangezogen() {
        let mut p = pods(4);
        let mut stand = Trainingsstand::new();
        stand_fortschreiben(&mut stand, &p, &(0..4).collect(), 9);
        // Pod 2 tauscht seine Besetzung gegen frische Kennungen aus.
        for (j, sh) in p[2].shards.iter_mut().enumerate() {
            sh.miner = miner(50_000 + j as u16);
        }
        let jetzt = trainingspods(&p, 1, &stand, &[3u8; 32]);
        assert_eq!(jetzt, BTreeSet::from([2]), "die neue Kennung stand nicht vorn");
    }

    /// Die Frische ist das Maximum, nicht der Durchschnitt.
    #[test]
    fn ein_einziges_frisches_mitglied_haelt_den_pod_zurueck() {
        let p = pods(2);
        let mut stand = Trainingsstand::new();
        // Nur ein einziges Mitglied von Pod 0 war gerade dran.
        stand.insert(p[0].shards[3].miner.miner_id, 42);
        assert_eq!(frische(&p[0], &stand), Some(42));
        assert_eq!(frische(&p[1], &stand), None);
        assert_eq!(trainingspods(&p, 1, &stand, &[1u8; 32]), BTreeSet::from([1]));
    }

    // ------------------------------------------------------- Ränder

    #[test]
    fn ein_leeres_netz_faellt_nicht_um() {
        let leer = Trainingsstand::new();
        assert!(trainingspods(&[], 0, &leer, &[0u8; 32]).is_empty());
        assert!(trainingspods(&[], 5, &leer, &[0u8; 32]).is_empty());
        assert!(trainingspods(&pods(10), 0, &leer, &[0u8; 32]).is_empty());
        assert_eq!(trainingspods(&pods(3), 99, &leer, &[0u8; 32]).len(), 3);
    }

    /// Ohne Kapazität ist keine Auslastung messbar, und ohne Pods gibt
    /// es nichts zu planen.
    #[test]
    fn ohne_pods_gibt_es_keinen_plan() {
        let leer = Zuteilung::default();
        let p = plane(
            &leer, 1_000, 3_600_000_000, &raten(), &Trainingsstand::new(), None, &[1u8; 32],
        );
        assert!(p.pods.is_empty());
        assert_eq!(p.kapazitaet, 0);
        assert_eq!(p.auslastung, 0);
        assert!(p.zuweisungen.is_empty());
    }

    // ------------------------------------------------ die Zuweisung

    fn anker(segmente: u64, buendel: u64) -> myl_types::korpusanker::Korpusanker {
        myl_types::korpusanker::Korpusanker {
            kennung: "probekorpus".into(),
            wurzel: myl_types::hash::Hash([2u8; 32]),
            segmente,
            buendel,
        }
    }

    fn zuteilung(n: u32) -> Zuteilung {
        Zuteilung { pods: pods(n), ohne_pod: Vec::new() }
    }

    /// ⚑ **Jeder Trainingspod bekommt ein Bündel, und zwar genau eines.**
    #[test]
    fn jeder_trainingspod_bekommt_ein_buendel() {
        let z = zuteilung(10);
        let k = anker(4_096, 256);
        let p = plane(
            &z, 0, 3_600_000_000, &raten(), &Trainingsstand::new(), Some(&k), &[5u8; 32],
        );
        assert_eq!(p.pods.len(), 8, "82 Prozent von zehn, gerundet");
        assert_eq!(p.zuweisungen.len(), p.pods.len(), "nicht jeder Pod hat ein Buendel");
        for nr in &p.pods {
            let zw = p.zuweisungen[nr];
            assert_eq!(zw.laenge, 256);
            assert_eq!(zw.start % 256, 0, "das Buendel liegt nicht auf dem Raster");
            assert!(zw.start + zw.laenge <= 4_096, "das Buendel ragt aus dem Korpus");
        }
    }

    /// ⚑ **Ohne Korpus keine Zuweisung, und der Plan sagt es.** Die Pods
    /// stehen trotzdem da: Das Netz hat Kapazität, nur nichts zu lernen.
    #[test]
    fn ohne_korpus_bleibt_die_zuweisung_leer() {
        let z = zuteilung(10);
        let p =
            plane(&z, 0, 3_600_000_000, &raten(), &Trainingsstand::new(), None, &[5u8; 32]);
        assert_eq!(p.pods.len(), 8);
        assert!(p.zuweisungen.is_empty(), "woher kaeme ein Buendel ohne Korpus?");
    }

    /// Ein Korpus, der kein volles Bündel hergibt, weist nichts zu,
    /// statt ein gekürztes zu erfinden.
    #[test]
    fn ein_zu_kleiner_korpus_weist_nichts_zu() {
        let z = zuteilung(10);
        let k = anker(100, 256);
        let p = plane(
            &z, 0, 3_600_000_000, &raten(), &Trainingsstand::new(), Some(&k), &[5u8; 32],
        );
        assert!(p.zuweisungen.is_empty(), "ein gekuerztes Buendel wurde erfunden");
    }

    /// ⚑ **Zwei Pods bekommen nicht zwangsläufig dasselbe.** Sonst
    /// rechnete das ganze Netz an einer Stelle des Korpus.
    #[test]
    fn verschiedene_pods_bekommen_verschiedene_buendel() {
        let z = zuteilung(40);
        let k = anker(1_048_576, 256);
        let p = plane(
            &z, 0, 3_600_000_000, &raten(), &Trainingsstand::new(), Some(&k), &[5u8; 32],
        );
        let starts: BTreeSet<u64> = p.zuweisungen.values().map(|z| z.start).collect();
        assert!(
            starts.len() > p.zuweisungen.len() / 2,
            "die Buendel haeufen sich: {} verschiedene bei {} Pods",
            starts.len(),
            p.zuweisungen.len()
        );
    }

    /// Und die Zuweisung ist deterministisch.
    #[test]
    fn dieselbe_saat_weist_dieselben_buendel_zu() {
        let z = zuteilung(10);
        let k = anker(4_096, 256);
        let s = Trainingsstand::new();
        let a = plane(&z, 0, 3_600_000_000, &raten(), &s, Some(&k), &[5u8; 32]);
        let b = plane(&z, 0, 3_600_000_000, &raten(), &s, Some(&k), &[5u8; 32]);
        let c = plane(&z, 0, 3_600_000_000, &raten(), &s, Some(&k), &[6u8; 32]);
        assert_eq!(a.zuweisungen, b.zuweisungen);
        assert_ne!(a.zuweisungen, c.zuweisungen, "die Saat wirkt nicht");
    }
}
