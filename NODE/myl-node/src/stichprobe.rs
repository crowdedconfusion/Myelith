//! Die Stichprobe einer Epoche: welche Segmente nachzurechnen sind.
//!
//! # ⚑ Fund 114: Stufe 2 lief in keinem Knoten
//!
//! `myl_scheduler::sample_segments` zieht die Lotterie, seit dem
//! 2026-08-17. `myl_verifier::check_segment` rechnet ein Segment nach,
//! seit demselben Tag. Beide sind geprüft, beide sind abgehakt, und
//! **beide hatten am 2026-09-01 null Aufrufer**: Außerhalb der Tests
//! und einer Determinismus-Vorführung im Testclient rief sie nichts.
//!
//! Damit lief **Stufe 2 der Verifikation nicht**, und das ist keine
//! Lücke neben anderen: Die gesamte Sicherheitsbedingung aus Anhang B.1
//! hängt an `p`, der Wahrscheinlichkeit, dass ein Segment nachgerechnet
//! wird. Ohne Ziehung ist `p = 0`, und `S_min = g/p²` ist keine
//! Schranke mehr.
//!
//! Dieses Modul ist die fehlende Naht. **Es zieht**, mehr noch nicht:
//! Das Nachrechnen braucht die Spur des Segments, und die liegt beim
//! Koordinator und nicht in der Kette. Was fehlt, steht unten unter
//! „Was hier noch nicht geschieht".
//!
//! # ⚑ Fund 115: Die Kette konnte nicht zählen, was sie bezahlt
//!
//! Die Ziehung war nicht nur nicht verdrahtet, sie war **nicht
//! herleitbar**. Ein `PoIBundle` trug bis zum 2026-09-01 nur die
//! **Wurzel** über seine Segmentzeugnisse, und eine Wurzel sagt nichts
//! über die Zahl ihrer Blätter. `sample_segments` braucht
//! `num_segments`, und diese Zahl gab es im Kettenzustand nirgends.
//!
//! Seither trägt das Bündel `segmente`, und zwar **in der signierten
//! Botschaft**: Wer sie nachträglich erhöhte, verdünnte die
//! Stichprobenwahrscheinlichkeit je Segment, ohne das Aggregat ungültig
//! zu machen.
//!
//! # ⚑ Ein gemeinsamer Indexraum, nicht je Pod
//!
//! Alle Bündel der Epoche werden in kanonischer Reihenfolge zu **einem**
//! Indexraum aneinandergelegt, und daraus wird einmal gezogen. Zöge man
//! je Pod, so bekäme ein Pod mit drei Segmenten bei jeder Aufrundung
//! eine viel höhere Rate je Segment als einer mit dreihundert. **`p` in
//! Anhang B.1 ist eine Wahrscheinlichkeit je Segment**, und die ist nur
//! in einem gemeinsamen Raum für alle dieselbe.
//!
//! # Was hier noch nicht geschieht
//!
//! - **Das Nachrechnen selbst.** Es braucht die Spur des gezogenen
//!   Segments, und die liegt beim Koordinator. Dafür fehlt ein
//!   Abruf über das Netz, und der ist eigene Arbeit.
//! - **Die Streitanzeige bei Abweichung.** Das Bisektions-Spiel steht in
//!   VERIFICATION; was fehlt, ist der Weg von „stimmt nicht überein" zu
//!   einer `Challenge` im Gossip.
//! - ⚑ **Die endgültige Saat.** Sie ist hier ein **Argument**, damit die
//!   offene Entscheidung an der Aufrufstelle sichtbar bleibt statt in
//!   dieser Datei zu verschwinden. Heute übergibt der Knoten den
//!   Blockhash; **der ist mahlbar**, denn wer den Abschlussblock
//!   erzeugt, probiert Kandidaten, bis die Ziehung seine eigenen
//!   Segmente verschont. Ziel ist die **Aggregatsignatur des Komitees**
//!   über den Abschlussblock: BLS ist deterministisch, also kann niemand
//!   unter Kandidaten wählen, und kein einzelnes Mitglied kennt sie,
//!   bevor zwei Drittel unterschrieben haben.

use myl_types::core_types::PoIBundle;
use myl_types::hash::Hash;
use myl_types::ids::PodId;

/// Trennstring der Stichproben-Saat.
///
/// Eigener String, damit dieselbe Quelle nicht zweimal dasselbe ergibt:
/// Die Pod-Bildung leitet aus demselben Blockhash ab, und zwei
/// Ableitungen mit gleicher Eingabe und gleichem Trennstring wären
/// dieselbe Zahl.
pub const DST_STICHPROBE: &[u8] = b"MYELITH_STICHPROBE_v1";

/// Ein gezogenes Segment: in welchem Pod, und das wievielte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Segmentstichprobe {
    /// Der Pod, dessen Bündel das Segment bezeugt.
    pub pod: PodId,
    /// Der Index des Segments **innerhalb dieses Bündels**.
    pub segment: u32,
}

/// Die Saat der Stichprobe, aus einer Quelle und dem Trennstring.
///
/// ⚑ **Die Quelle soll die Aggregatsignatur des Komitees sein, nicht der
/// Blockhash** (Punkt 44). Der Blockhash gibt dem Erzeuger des
/// Abschlussblocks **unbegrenzten** Mahlraum: Er variiert den
/// Blockinhalt, bis die Ziehung seine eigenen Segmente verschont.
///
/// ⛑ **Und die Aggregatsignatur ist nicht eindeutig** (Fund 120,
/// 2026-09-01). BLS ist eindeutig für Nachricht **und** Schlüsselmenge;
/// das Commitzertifikat trägt aber eine **variable** Unterzeichnermenge,
/// und wer es zusammenstellt, wählt sie, solange sie das Quorum trägt.
/// Gemessen bei Komitee 21 und Schwelle 15:
///
/// | erhaltene Stimmen | gültige Teilmengen | Mahlraum |
/// |---|---|---|
/// | 16 | 17 | 4,1 Bit |
/// | 18 | 988 | 9,9 Bit |
/// | 21 | 82 160 | 16,3 Bit |
///
/// **Unbegrenzt auf höchstens sechzehn Bit ist ein echter Gewinn, und
/// sechzehn Bit sind nicht null.** Eindeutig wird es erst mit
/// Schwellen-BLS und verteilter Schlüsselerzeugung, wo *irgendwelche*
/// `t` von `n` dasselbe ergeben. Das ist ein eigenes Vorhaben.
pub fn stichprobensaat(quelle: &[u8], epoche: u64) -> [u8; 32] {
    let mut vor = Vec::with_capacity(DST_STICHPROBE.len() + quelle.len() + 8);
    vor.extend_from_slice(DST_STICHPROBE);
    vor.extend_from_slice(quelle);
    vor.extend_from_slice(&epoche.to_le_bytes());
    Hash::sha256(&vor).0
}

/// Die Saat aus einem Commitzertifikat: **die bevorzugte Quelle**.
///
/// ⚑ **Die Unterzeichnermenge geht mit ein, nicht nur das Aggregat.**
/// Sonst ergäben zwei Zertifikate mit verschiedenen Unterzeichnern und
/// zufällig gleichem Aggregat dieselbe Saat; das kann nicht vorkommen,
/// aber die Saat soll **an dem hängen, was das Zertifikat ausmacht**,
/// und nicht an einem Teil davon.
pub fn saat_aus_zertifikat(
    aggregat: &myl_types::bls::BlsAggregateSignature,
    unterzeichner: &[myl_types::ids::MinerId],
    epoche: u64,
) -> [u8; 32] {
    let mut quelle = Vec::with_capacity(96 + unterzeichner.len() * 32);
    quelle.extend_from_slice(&aggregat.0);
    for m in unterzeichner {
        quelle.extend_from_slice(m.as_bytes());
    }
    stichprobensaat(&quelle, epoche)
}

/// Zieht die Stichprobe einer Epoche aus ihren Bündeln.
///
/// Die Bündel werden nach `pod` sortiert, damit die Reihenfolge nicht an
/// der Übergabe hängt: **Zwei Knoten mit verschiedener Reihenfolge zögen
/// verschiedene Segmente**, und wer geprüft wird, ist eine
/// Konsensentscheidung.
///
/// Bündel ohne Segmente werden übergangen. Sie sind kein Fehler, tragen
/// aber auch nichts zum Indexraum bei.
pub fn stichprobe_der_epoche(
    buendel: &[PoIBundle],
    saat: &[u8; 32],
    rate_bp: u32,
) -> Vec<Segmentstichprobe> {
    let mut sortiert: Vec<&PoIBundle> = buendel.iter().filter(|b| b.segmente > 0).collect();
    sortiert.sort_by_key(|b| *b.pod.as_bytes());

    let gesamt: u64 = sortiert.iter().map(|b| u64::from(b.segmente)).sum();
    if gesamt == 0 {
        return Vec::new();
    }
    // ⚑ Mehr Segmente als `u32` fasst, ist kein Betriebsfall, sondern
    // eine Behauptung: Bei 32 Bit waeren es vier Milliarden Segmente in
    // einer Epoche. Gedeckelt statt umgelaufen; ein Umlauf machte den
    // Indexraum kleiner als die Bündel und zöge ins Leere.
    let gesamt = u32::try_from(gesamt).unwrap_or(u32::MAX);

    let gezogen = myl_scheduler::sample_segments(gesamt, rate_bp, saat);

    let mut aus = Vec::with_capacity(gezogen.sampled_segments.len());
    for global in gezogen.sampled_segments {
        let mut rest = global;
        for b in &sortiert {
            if rest < b.segmente {
                aus.push(Segmentstichprobe {
                    pod: b.pod,
                    segment: rest,
                });
                break;
            }
            rest -= b.segmente;
        }
    }
    aus
}

/// An wen ein Checker seine [`myl_types::Spuranfrage`] richtet.
///
/// # ⚑ An den Pod, nicht an den Koordinator
///
/// Naheliegend wäre der Koordinator, denn er sammelt die Spuren ein.
/// **Dann genügte sein Schweigen, um die Prüfung zu vereiteln.** Alle
/// Mitglieder haben das Bündel unterschrieben, also haben alle gesehen,
/// was bezeugt wurde; wer antwortet, ist gleichgültig, denn der
/// Merkle-Beweis bindet die Antwort an die unterschriebene Wurzel und
/// nicht an den Antwortenden.
///
/// **Damit muss ein ganzer Pod schweigen statt einer**, und das ist der
/// Unterschied zwischen einem Schloss mit einem Schlüssel und einem mit
/// zehn.
///
/// # ⚑ Was fehlt, wenn die Liste leer ist
///
/// Ein Pod, von dem keine einzige Adresse bekannt ist, ist nicht
/// prüfbar. **Das ist ein Befund und kein Nichts:** Eine leere Liste
/// muss beim Aufrufer wie eine ausgebliebene Antwort zählen, sonst wäre
/// „ich nenne keine Adresse" die billigste Art, sich der Prüfung zu
/// entziehen.
pub fn adressen_des_pods(
    pod: &myl_scheduler::shard_assignment::Pod,
) -> Vec<myl_types::latency_attest::PeerIdBytes> {
    let leer = myl_types::latency_attest::PeerIdBytes([0; 32]);
    pod.mitglieder()
        .map(|m| m.netzadresse)
        .filter(|a| *a != leer)
        .collect()
}

/// Was ein Checker zu einer gezogenen Stichprobe fragen muss.
///
/// Gibt die Paare `(Adresse, Anfrage)` zurück und daneben die Zahl der
/// Segmente, zu denen **keine** Adresse zu finden war.
///
/// # ⚑ Warum das eine eigene Funktion ist
///
/// Der Knoten schickt nur, was hier herauskommt. **Die Entscheidung ist
/// damit prüfbar, ohne ein Netz zu starten**, und der Versandteil bleibt
/// so dünn, dass an ihm nichts schiefgehen kann. Fund 114 ist entstanden,
/// weil eine gebaute Entscheidung keinen Aufrufer hatte; eine
/// Entscheidung, die niemand prüfen kann, ist der nächste Schritt in
/// dieselbe Richtung.
///
/// # ⚑ Die Zahl ohne Adresse ist ein Befund, kein Rest
///
/// Ein Pod, den niemand erreichen kann, ist nicht prüfbar. Sie
/// mitzugeben zwingt den Aufrufer, es zu sehen; sonst wäre „ich nenne
/// keine Adresse" die billigste Art, sich der Prüfung zu entziehen.
pub fn anfragen_fuer(
    gezogen: &[Segmentstichprobe],
    epoche: u64,
    pod_finden: impl Fn(&PodId) -> Option<myl_scheduler::shard_assignment::Pod>,
) -> (
    Vec<(myl_types::latency_attest::PeerIdBytes, myl_types::Spuranfrage)>,
    usize,
) {
    let mut aus = Vec::new();
    let mut ohne_adresse = 0usize;
    for s in gezogen {
        let Some(pod) = pod_finden(&s.pod) else {
            ohne_adresse += 1;
            continue;
        };
        let adressen = adressen_des_pods(&pod);
        if adressen.is_empty() {
            ohne_adresse += 1;
            continue;
        }
        let anfrage = myl_types::Spuranfrage {
            epoche: myl_types::ids::EpochId(epoche),
            pod: s.pod,
            segment: s.segment,
        };
        // ⚑ **Jedes Mitglied wird gefragt**, nicht nur eines: Sonst
        // genügte das Schweigen eines Einzelnen.
        for a in adressen {
            aus.push((a, anfrage));
        }
    }
    (aus, ohne_adresse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSignature;
    use myl_types::ids::{EpochId, MerkleRoot};

    /// Ein Pod mit `mitglieder` Positionen, wahlweise mit Adressen.
    fn probepod(mitglieder: [u8; 3], mit_adresse: bool) -> myl_scheduler::shard_assignment::Pod {
        use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
        use myl_scheduler::shard_assignment::{Pod, Shard};
        use myl_types::latency_attest::PeerIdBytes;
        Pod {
            pod_index: 0,
            shards: mitglieder
                .iter()
                .enumerate()
                .map(|(i, &b)| Shard {
                    shard_index: i as u32,
                    miner: MinerRegistration {
                        miner_id: myl_types::ids::MinerId::new([b; 32]),
                        hardware_class: HardwareClass::MediumGpu,
                        registration_epoch: 0,
                        zone: myl_types::node_metadata::GeoRegion::Europe,
                        schluessel: myl_types::bls::BlsPublicKey([0; 48]),
                        netzadresse: PeerIdBytes(if mit_adresse { [b; 32] } else { [0; 32] }),
                    },
                })
                .collect(),
            reserve: vec![],
        }
    }

    fn buendel(p: u8, segmente: u32) -> PoIBundle {
        PoIBundle {
            epoch: EpochId(7),
            pod: PodId::new([p; 32]),
            segments_root: MerkleRoot::new([p; 32]),
            vtfe_claimed: 1_000,
            aggregate_sig: BlsSignature([0; 96]),
            segmente,
        }
    }

    /// Die Saat haengt an Quelle **und** Epoche, sonst zoege jede Epoche
    /// dieselben Segmente.
    #[test]
    fn die_saat_haengt_an_quelle_und_epoche() {
        let a = stichprobensaat(Hash::sha256(b"block").as_bytes(), 7);
        assert_eq!(a, stichprobensaat(Hash::sha256(b"block").as_bytes(), 7));
        assert_ne!(a, stichprobensaat(Hash::sha256(b"block").as_bytes(), 8));
        assert_ne!(a, stichprobensaat(Hash::sha256(b"anderer").as_bytes(), 7));
    }

    /// ⚑ **Die Unterzeichnermenge geht in die Saat ein.**
    ///
    /// Sie ist der Teil, den ein Zusammensteller waehlen kann (Fund
    /// 120). Sie **wegzulassen** hiesse nicht, das Mahlen zu
    /// verhindern, sondern es unsichtbar zu machen: Dieselbe Saat kaeme
    /// dann aus zwei verschiedenen Zertifikaten.
    #[test]
    fn die_unterzeichnermenge_geht_in_die_saat_ein() {
        use myl_types::bls::BlsAggregateSignature;
        use myl_types::ids::MinerId;
        let agg = BlsAggregateSignature([3; 96]);
        let a = saat_aus_zertifikat(&agg, &[MinerId::new([1; 32]), MinerId::new([2; 32])], 7);
        let b = saat_aus_zertifikat(&agg, &[MinerId::new([1; 32]), MinerId::new([3; 32])], 7);
        assert_ne!(a, b, "zwei Unterzeichnermengen ergaben dieselbe Saat");
        // Und die Reihenfolge zaehlt ebenfalls: Das Zertifikat fuehrt
        // sie streng aufsteigend, also ist eine andere Reihenfolge ein
        // anderes Zertifikat.
        let c = saat_aus_zertifikat(&agg, &[MinerId::new([2; 32]), MinerId::new([1; 32])], 7);
        assert_ne!(a, c);
    }

    /// Und das Aggregat allein aendert sie auch.
    #[test]
    fn ein_anderes_aggregat_ergibt_eine_andere_saat() {
        use myl_types::bls::BlsAggregateSignature;
        use myl_types::ids::MinerId;
        let u = [MinerId::new([1; 32])];
        assert_ne!(
            saat_aus_zertifikat(&BlsAggregateSignature([3; 96]), &u, 7),
            saat_aus_zertifikat(&BlsAggregateSignature([4; 96]), &u, 7)
        );
    }

    /// **Die Stichprobensaat ist nicht die der Pod-Bildung.**
    ///
    /// ⛑ Hier stand „der Trennstring wirkt", und das prueft dieser Test
    /// **nicht**: Die beiden Ableitungen bauen ihr Urbild ohnehin
    /// verschieden, also blieb er gruen, als der Trennstring
    /// versuchsweise entfiel. Was er zeigt, ist die Aussage, auf die es
    /// ankommt: **Dieselbe Quelle ergibt zwei verschiedene Zahlen**, und
    /// wer in einem Pod sitzt, weiss damit nicht, welche Segmente
    /// gezogen werden.
    #[test]
    fn die_saat_ist_nicht_der_epochenseed_der_podbildung() {
        let block = Hash::sha256(b"block");
        assert_ne!(
            stichprobensaat(block.as_bytes(), 7),
            myl_scheduler::zonenzuteilung::epochenseed(&block, 7)
        );
    }

    /// Zwei Prozent von tausend Segmenten sind zwanzig.
    #[test]
    fn die_rate_trifft_die_gesamtzahl() {
        let b = vec![buendel(1, 400), buendel(2, 600)];
        let s = stichprobe_der_epoche(&b, &[3u8; 32], 200);
        assert_eq!(s.len(), 20, "zwei Prozent von tausend");
    }

    /// ⚑ **Ein gemeinsamer Indexraum, kein Zug je Pod.**
    ///
    /// Ein Pod mit drei Segmenten und einer mit dreihundert: Zöge man je
    /// Pod, bekaeme der kleine bei jeder Aufrundung **genau ein**
    /// Segment und damit eine Rate von 33 Prozent statt 2.
    ///
    /// ⛑ **Hier stand `klein <= 1`, und das liess genau den schlechten
    /// Fall zu:** Der Zug je Pod ergibt eins, also bestand der Test ihn.
    /// Die Gegenprobe hat es gezeigt. Geprueft wird jetzt der **Anteil
    /// ueber viele Saaten**, denn darum geht es: `p` in Anhang B.1 ist
    /// eine Wahrscheinlichkeit je Segment und muss fuer jedes dieselbe
    /// sein.
    #[test]
    fn kleine_pods_bekommen_keine_hoehere_rate() {
        let b = vec![buendel(1, 3), buendel(2, 300)];
        let (mut klein, mut gesamt) = (0usize, 0usize);
        for k in 0..200u8 {
            let s = stichprobe_der_epoche(&b, &[k; 32], 200);
            klein += s.iter().filter(|x| x.pod == PodId::new([1; 32])).count();
            gesamt += s.len();
        }
        // Erwartet ist der Groessenanteil: 3 von 303, also rund 1 Prozent.
        // Der Zug je Pod ergaebe **jedes Mal genau eins**, also rund ein
        // Sechstel aller Ziehungen.
        let anteil = klein as f64 / gesamt as f64;
        assert!(
            anteil < 0.05,
            "der kleine Pod bekam {klein} von {gesamt} Ziehungen (Anteil {anteil:.3}); \
             bei drei von 303 Segmenten waeren rund 0,01 zu erwarten"
        );
    }

    /// Die Reihenfolge der Übergabe darf nichts aendern: Zwei Knoten mit
    /// verschieden sortierten Bündeln zoegen sonst verschiedene Segmente.
    #[test]
    fn die_uebergabereihenfolge_aendert_nichts() {
        let vorwaerts = vec![buendel(1, 50), buendel(2, 50), buendel(3, 50)];
        let mut rueckwaerts = vorwaerts.clone();
        rueckwaerts.reverse();
        assert_eq!(
            stichprobe_der_epoche(&vorwaerts, &[9u8; 32], 500),
            stichprobe_der_epoche(&rueckwaerts, &[9u8; 32], 500)
        );
    }

    /// Jedes gezogene Segment liegt innerhalb seines Bündels.
    #[test]
    fn kein_index_zeigt_ins_leere() {
        let b = vec![buendel(1, 7), buendel(2, 13), buendel(3, 1)];
        for s in stichprobe_der_epoche(&b, &[2u8; 32], 3_000) {
            let eigner = b.iter().find(|x| x.pod == s.pod).expect("Pod aus der Ziehung");
            assert!(
                s.segment < eigner.segmente,
                "Segment {} von {} im Pod",
                s.segment,
                eigner.segmente
            );
        }
    }

    /// ⚑ **Gefragt wird der Pod, nicht der Koordinator.**
    ///
    /// Alle Mitglieder haben das Bündel unterschrieben, also kann jedes
    /// antworten; der Merkle-Beweis bindet die Antwort an die Wurzel und
    /// nicht an den Antwortenden. **So muss ein ganzer Pod schweigen
    /// statt einer.**
    #[test]
    fn jedes_mitglied_kann_gefragt_werden() {
        use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
        use myl_scheduler::shard_assignment::{Pod, Shard};
        use myl_types::latency_attest::PeerIdBytes;

        let reg = |b: u8| MinerRegistration {
            miner_id: myl_types::ids::MinerId::new([b; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 0,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: PeerIdBytes([b; 32]),
        };
        let pod = Pod {
            pod_index: 0,
            shards: (1..=4u8)
                .map(|b| Shard {
                    shard_index: u32::from(b) - 1,
                    miner: reg(b),
                })
                .collect(),
            reserve: vec![reg(9), reg(10)],
        };
        let a = adressen_des_pods(&pod);
        assert_eq!(a.len(), 6, "Positionen und Reserve, alle sechs");
        assert!(a.contains(&PeerIdBytes([9; 32])), "die Reserve fehlt");
    }

    /// ⚑ **Wer keine Adresse nennt, taucht nicht auf.**
    ///
    /// Die Nulladresse ist keine, und sie stillschweigend
    /// weiterzureichen hieße, einen Checker gegen eine Wand zu schicken
    /// und den Fehlschlag dem Netz anzulasten. Bleibt am Ende **keine**
    /// Adresse, ist der Pod nicht prüfbar, und das muss der Aufrufer
    /// sehen.
    #[test]
    fn ein_pod_ohne_adressen_ist_leer_und_das_ist_der_befund() {
        use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
        use myl_scheduler::shard_assignment::{Pod, Shard};
        use myl_types::latency_attest::PeerIdBytes;

        let ohne = MinerRegistration {
            miner_id: myl_types::ids::MinerId::new([1; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 0,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: PeerIdBytes([0; 32]),
        };
        let pod = Pod {
            pod_index: 0,
            shards: vec![Shard { shard_index: 0, miner: ohne }],
            reserve: vec![],
        };
        assert!(adressen_des_pods(&pod).is_empty());
    }

    /// ⚑ **Zu jeder Ziehung geht eine Frage an jedes Mitglied.**
    ///
    /// Ohne diesen Test wäre die Ziehung wieder etwas, das läuft und
    /// nichts bewirkt: genau Fund 114, eine Ebene höher.
    #[test]
    fn jede_ziehung_ergibt_fragen_an_alle_mitglieder() {
        let pod = probepod([1, 2, 3], true);
        let gezogen = vec![
            Segmentstichprobe { pod: PodId::new([7; 32]), segment: 0 },
            Segmentstichprobe { pod: PodId::new([7; 32]), segment: 5 },
        ];
        let (fragen, ohne) = anfragen_fuer(&gezogen, 3, |_| Some(pod.clone()));
        assert_eq!(ohne, 0);
        assert_eq!(fragen.len(), 6, "zwei Ziehungen mal drei Mitglieder");
        assert!(fragen.iter().all(|(_, a)| a.epoche.0 == 3));
        let segmente: std::collections::BTreeSet<u32> =
            fragen.iter().map(|(_, a)| a.segment).collect();
        assert_eq!(segmente.len(), 2, "beide Segmente muessen gefragt werden");
    }

    /// ⚑ **Ein Pod ohne Adresse wird gezaehlt, nicht uebergangen.**
    #[test]
    fn ein_pod_ohne_adresse_zaehlt_als_befund() {
        let pod = probepod([1, 2, 3], false);
        let gezogen = vec![Segmentstichprobe { pod: PodId::new([7; 32]), segment: 0 }];
        let (fragen, ohne) = anfragen_fuer(&gezogen, 3, |_| Some(pod.clone()));
        assert!(fragen.is_empty());
        assert_eq!(ohne, 1, "der unerreichbare Pod muss im Ergebnis stehen");
    }

    /// Und ein Pod, den die Zuteilung nicht kennt, ebenso.
    #[test]
    fn ein_unbekannter_pod_zaehlt_als_befund() {
        let gezogen = vec![Segmentstichprobe { pod: PodId::new([7; 32]), segment: 0 }];
        let (fragen, ohne) = anfragen_fuer(&gezogen, 3, |_| None);
        assert!(fragen.is_empty());
        assert_eq!(ohne, 1);
    }

    /// Ohne Bündel gibt es nichts zu ziehen, und das ist kein Fehler.
    #[test]
    fn ohne_buendel_wird_nichts_gezogen() {
        assert!(stichprobe_der_epoche(&[], &[1u8; 32], 200).is_empty());
    }

    /// Ein Bündel ohne Segmente traegt nichts zum Indexraum bei.
    #[test]
    fn ein_leeres_buendel_wird_uebergangen() {
        let b = vec![buendel(1, 0), buendel(2, 100)];
        let s = stichprobe_der_epoche(&b, &[4u8; 32], 1_000);
        assert_eq!(s.len(), 10);
        assert!(s.iter().all(|x| x.pod == PodId::new([2; 32])));
    }
}
