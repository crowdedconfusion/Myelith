//! Nutzlastprüfung des Knotens: die Schichtgrenze, die nur er auflösen kann.
//!
//! # Warum das hier steht und nicht in `myl-net`
//!
//! `myl-net` ist die Netzschicht (L0). Die Typen von Blöcken und
//! Transaktionen liegen in `myl-consensus` (L1). Würde die Netzschicht
//! sie kennen, wäre die Schichtung umgekehrt, und jeder, der nur ein
//! Netz braucht, zöge den Konsens mit.
//!
//! Deshalb prüft `myl-net` diese beiden Topics nur auf ihre Größe und
//! reicht den Rest über [`myl_net::validation::PayloadValidator`] nach
//! oben. **Der Knoten ist die vorgesehene Stelle**, weil er als einziger
//! beide Seiten kennt.
//!
//! Dass dieser Weg seit dem 2026-08-18 dokumentiert und bis zum
//! 2026-08-24 unbenutzt war, ist Fund 55: Über `run_node` gab es keinen
//! Parameter dafür. Dieses Modul ist der erste Abnehmer.
//!
//! # ⚑ Was hier geprüft wird, und was ausdrücklich nicht
//!
//! **Geprüft wird die Form, nicht die Gültigkeit.** Eine Nachricht muss
//! sich als der Typ lesen lassen, den ihr Topic ankündigt, und sie darf
//! keine Anhängsel tragen.
//!
//! **Nicht geprüft** werden Signaturen, Epochengültigkeit, Stake, Höhe
//! oder Vorgängerblock. Das ist kein Versäumnis, sondern die Grenze des
//! Möglichen an dieser Stelle: Diese Fragen brauchen **Kettenzustand**,
//! und der Knoten hat noch keinen. Ein Urteil über Gültigkeit ohne
//! Zustand wäre ein geratenes Urteil, und ein geratenes `Reject`
//! verwirft ehrliche Nachrichten und bestraft ihren Absender im
//! Gossipsub-Scoring.
//!
//! # ⚑ Fund 57: Auch für Blöcke ist der Parse fast nur eine Längenprüfung
//!
//! Hier stand zuerst: „`Block` und `Transaction` sind **keine** Typen
//! aus lauter festen Feldern, sie tragen Vektoren, deren Längenköpfe
//! stimmen müssen. Für sie hat der Parse also Zähne."
//!
//! **Das ist falsch, und der Test, der es messen sollte, hat es
//! widerlegt:** Von 20 000 verstümmelten Blöcken kommen rund **88 %
//! durch**.
//!
//! Der Grund ist beim Nachrechnen offensichtlich. Ein `Block` mit
//! wenigen Einträgen besteht fast nur aus Feldern fester Breite: die
//! Epochen-Metadaten allein sind 80 Bytes (zwei `u64`, zwei `Hash`),
//! dazu fünf Vektoren, die als leere Längenköpfe je vier Bytes belegen.
//! **Empfindlich sind nur diese Längenköpfe**, also gut ein Siebtel der
//! Bytes. Ein gekipptes Bit trifft mit hoher Wahrscheinlichkeit ein
//! festes Feld, und dann liest sich die Nachricht weiterhin als Block,
//! nur mit anderem Inhalt.
//!
//! Das ist dieselbe Eigenschaft, die Fund 45 für PoI-Bündel festgehalten
//! hat, nur abgeschwächt statt vollständig. **Die Lehre ist, dass die
//! Abschwächung nicht reicht, um von „Prüfung" zu sprechen.**
//!
//! Was folgt daraus? Nicht, diesen Validator wegzulassen: Er fängt
//! Nachrichten ab, die sich gar nicht als Block lesen lassen, er
//! erzwingt „kein Anhängsel", und ein `Reject` bestraft den Absender im
//! Gossipsub-Scoring. Wohl aber, **nicht mit ihm zu rechnen**. Die
//! Verteidigung für Blöcke ist die Signatur des Vorschlagenden gegen die
//! Validator-Registry, und die braucht Kettenzustand, den dieser Knoten
//! noch nicht führt.
//!
//! Der Test `fund_57_verstuemmelte_bloecke_kommen_meist_durch` hält die
//! gemessene Zahl fest. **Er schlägt fehl, sobald jemand eine echte
//! Prüfung ergänzt**, und zwingt damit zur Aktualisierung dieser Stelle.

use borsh::BorshDeserialize;
use myl_consensus::bft::Konsensnachricht;
use myl_consensus::block::{Block, Transaktion};
use myl_net::{GossipTopic, PayloadValidator};

use crate::validatorsatz::{Attesturteil, Validatorsatz};

/// Die Nutzlastprüfung des Knotens.
///
/// Trägt den [`Validatorsatz`], gegen den Latenz-Atteste geprüft
/// werden. **Ohne ihn wäre A10 offen**, siehe unten.
#[derive(Debug, Default, Clone)]
pub struct ProtokollValidator {
    validatoren: Validatorsatz,
}

impl ProtokollValidator {
    /// Mit einem Validatorsatz für die Attest-Prüfung.
    pub fn mit(validatoren: Validatorsatz) -> Self {
        Self { validatoren }
    }

    /// Der Validatorsatz, für Diagnose.
    pub fn validatoren(&self) -> &Validatorsatz {
        &self.validatoren
    }

    /// Prüft ein Attest und gibt das Urteil zurück.
    ///
    /// Getrennt von [`PayloadValidator::validate`], weil der Grund fürs
    /// Protokoll gebraucht wird: „unbekannter Aussteller" und „falsche
    /// Signatur" haben verschiedene Ursachen, und die erste ist im
    /// Probelauf fast immer ein vergessener Name.
    pub fn beurteile_attest(&self, data: &[u8]) -> Attesturteil {
        let mut rest = data;
        match myl_types::LatencyAttest::deserialize(&mut rest) {
            Ok(a) if rest.is_empty() => self.validatoren.pruefe(&a),
            _ => Attesturteil::StrukturFalsch,
        }
    }

    /// Dasselbe für eine Anfechtung (Fund 96).
    pub fn beurteile_anfechtung(&self, data: &[u8]) -> Attesturteil {
        let mut rest = data;
        match myl_types::Challenge::deserialize(&mut rest) {
            Ok(c) if rest.is_empty() => self.validatoren.pruefe_anfechtung(&c),
            _ => Attesturteil::StrukturFalsch,
        }
    }
}

/// Liest einen Typ aus Bytes und verlangt, dass **nichts übrig bleibt**.
///
/// Der Nachsatz ist der wichtigere Teil. `from_slice` allein akzeptiert
/// bei manchen Typen Anhängsel, und ein Anhängsel ist ein Kanal: Zwei
/// Nachrichten mit gleichem Inhalt und verschiedenem Anhang haben
/// verschiedene Nachrichten-Ids und laufen beide durchs Netz.
fn liest_sich_vollstaendig_als<T: BorshDeserialize>(daten: &[u8]) -> bool {
    let mut rest = daten;
    match T::deserialize(&mut rest) {
        Ok(_) => rest.is_empty(),
        Err(_) => false,
    }
}

impl PayloadValidator for ProtokollValidator {
    fn validate(&self, topic: GossipTopic, data: &[u8]) -> bool {
        match topic {
            GossipTopic::Blocks => liest_sich_vollstaendig_als::<Block>(data),
            GossipTopic::Transactions => liest_sich_vollstaendig_als::<Transaktion>(data),
            // ⚑ **A10: Latenz-Atteste werden geprüft.**
            //
            // Bis zum 2026-08-25 stand hier `_ => true`, und damit lief
            // ein Attest mit beliebiger Signatur durch. Das Feld gab es
            // seit dem ersten Entwurf, geprüft hat es nie jemand.
            //
            // Die Latenzwerte gehen ins Geo-Clustering der Pods. Wer sie
            // frei setzen kann, sucht sich seine Pod-Nachbarn aus, und
            // das ist die Vorstufe zur Kollusion beider Pods (A12).
            GossipTopic::LatencyAttests => self.beurteile_attest(data).ist_gueltig(),
            // ⚑ **Fund 96: Anfechtungen werden geprüft, aber nur, wenn
            // der Herausforderer bekannt ist.**
            //
            // Hier fiel eine Anfechtung bis zum 2026-08-29 unter
            // `_ => true`, und bis dahin trug sie auch gar keine
            // Unterschrift. Beides zusammen hieß: Jeder konnte im Namen
            // jedes Miners anfechten. Das kostet den Angeklagten etwas,
            // denn er muss antworten.
            //
            // **Der Unterschied zum Latenz-Attest, und er ist wichtig:**
            // Ein Attest kommt von einem Validator, und die
            // Validatorenliste ist genau die Menge, gegen die hier
            // geprüft wird. Ein Herausforderer ist dagegen ein **Miner**
            // des redundanten Pods, und der muss in dieser Liste nicht
            // stehen. Wer ihn deshalb abwiese, verwürfe aus **geratener
            // Unkenntnis**, und im Gossipsub-Scoring trifft das den
            // ehrlichen Absender, nicht den Angreifer; dieselbe
            // Überlegung, aus der Konsensnachrichten hier nur
            // strukturell geprüft werden.
            //
            // Also: unbekannter Herausforderer geht durch, falsche
            // Unterschrift eines **bekannten** nicht. Eine Anfechtung,
            // deren Absender niemand zuordnen kann, führt trotzdem zu
            // nichts: Die Slash-Entscheidung verlangt einen
            // `Anfechtungsbeleg`, und der braucht den Schlüssel.
            //
            // ⚑ **Die Zuordnung Miner zu Schlüssel gehört in eine
            // Registrierung, die es noch nicht gibt.** Solange die
            // Teilnehmerliste die einzige Quelle ist, prüft dieser
            // Zweig im echten Netz nur die Validatoren unter den
            // Herausforderern.
            GossipTopic::Challenges => !matches!(
                self.beurteile_anfechtung(data),
                Attesturteil::SignaturFalsch | Attesturteil::StrukturFalsch
            ),
            // Konsensnachrichten: Form ja, Gültigkeit nein.
            //
            // ⚑ **Und die Form leistet fast nichts**, weil alle Felder
            // feste Breite haben (gemessen in
            // `myl_consensus::bft::tests`: 99 % der verstümmelten
            // Nachrichten kommen durch). Der Nutzen liegt woanders: Sie
            // erzwingt „kein Anhängsel", und ein Anhängsel ist ein
            // Kanal, weil es die Nachrichten-Id ändert, ohne den Inhalt
            // zu ändern.
            //
            // **Die eigentliche Prüfung ist hier, anders als bei
            // PoI-Bündeln (Fund 45), erreichbar:** Runde,
            // Mitgliedschaft, Duplikat und BLS-Signatur prüft
            // `myl_consensus::bft::BftState`, und der Knoten ruft ihn
            // über `crate::konsens::Konsensrunde` auch auf. Hier wäre
            // sie nicht zu haben: Dieser Validator kennt die laufende
            // Runde nicht und dürfte sie nicht kennen, denn er läuft im
            // Netzfaden, und ein `Reject` aus geratener Unkenntnis
            // bestrafte ehrliche Absender im Gossipsub-Scoring.
            GossipTopic::Consensus => liest_sich_vollstaendig_als::<Konsensnachricht>(data),
            // Die übrigen prüft `myl-net` strukturell; hier nichts zu
            // ergänzen, solange kein Kettenzustand da ist.
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::{Anweisung, BlockHeader};

    fn burn(n: u8, betrag: u64) -> Transaktion {
        Transaktion::signiere(
            &Hash::sha256(b"myelith-testkette-genesis"),
            &myl_types::bls::BlsSecretKey::key_gen(&[n; 32]).expect("Schlüssel"),
            0,
            Anweisung::Burn { betrag },
        )
        .expect("signieren")
    }
    use myl_types::hash::Hash;

    fn beispielblock() -> Block {
        let mut b = Block::new(BlockHeader {
            height: 5_400,
            epoch: 3,
            prev_block_hash: Hash::sha256(b"vorgaenger"),
            timestamp_ms: 1_700_000_000_000,
            state_root: Hash::sha256(b"zustand"),
            saatquelle: None,
        });
        // Ein Block mit Inhalt: Die Vektoren sind der Grund, warum der
        // Borsh-Parse hier mehr leistet als eine Längenprüfung (Fund 45).
        b.txs.push(burn(4, 1_000));
        b
    }

    #[test]
    fn ein_echter_block_kommt_durch() {
        let daten = borsh::to_vec(&beispielblock()).unwrap();
        assert!(ProtokollValidator::default().validate(GossipTopic::Blocks, &daten));
    }

    #[test]
    fn zufallsbytes_werden_abgelehnt() {
        assert!(!ProtokollValidator::default().validate(GossipTopic::Blocks, &[0xAB; 64]));
        assert!(!ProtokollValidator::default().validate(GossipTopic::Blocks, &[]));
    }

    #[test]
    fn ein_block_mit_anhaengsel_wird_abgelehnt() {
        // Ein Anhängsel ändert die Nachrichten-Id, nicht den Inhalt:
        // derselbe Block liefe beliebig oft durchs Netz.
        let mut daten = borsh::to_vec(&beispielblock()).unwrap();
        daten.push(0);
        assert!(
            !ProtokollValidator::default().validate(GossipTopic::Blocks, &daten),
            "ein Anhängsel hinter einem gültigen Block kam durch"
        );
    }

    #[test]
    fn eine_transaktion_wird_nicht_als_block_gelesen() {
        let tx = burn(4, 100);
        let daten = borsh::to_vec(&tx).unwrap();
        assert!(ProtokollValidator::default().validate(GossipTopic::Transactions, &daten));
        assert!(
            !ProtokollValidator::default().validate(GossipTopic::Blocks, &daten),
            "eine Transaktion las sich als Block"
        );
    }

    #[test]
    fn fund_57_verstuemmelte_bloecke_kommen_meist_durch() {
        // ⚑ Dieser Test ist als Gegenprobe geschrieben worden und hat
        // die Doku widerlegt, die er belegen sollte. Er hält jetzt die
        // **gemessene Schwäche** fest, damit niemand aus „der Knoten
        // prüft Blöcke" schließt, das Netz filtere schon.
        //
        // Schlägt er fehl, ist das gute Nachricht: Dann prüft jemand
        // mehr als die Form, und der Modulkopf gehört nachgezogen.
        let gut = borsh::to_vec(&beispielblock()).unwrap();
        let mut zustand: u64 = 0x9E3779B97F4A7C15;
        let mut durch = 0usize;
        const VERSUCHE: usize = 20_000;
        for _ in 0..VERSUCHE {
            zustand ^= zustand << 13;
            zustand ^= zustand >> 7;
            zustand ^= zustand << 17;
            let mut kaputt = gut.clone();
            let pos = (zustand as usize) % kaputt.len();
            kaputt[pos] ^= ((zustand >> 32) as u8) | 1;
            if ProtokollValidator::default().validate(GossipTopic::Blocks, &kaputt) {
                durch += 1;
            }
        }
        let anteil = durch * 100 / VERSUCHE;
        println!(
            "[Messung] {durch} von {VERSUCHE} verstümmelten Blöcken kamen durch ({anteil} %)"
        );
        assert!(
            anteil > 50,
            "nur {anteil} % kamen durch: Wenn die Formprüfung inzwischen mehr \
             als die Hälfte abfängt, hat jemand eine echte Prüfung ergänzt. \
             Dann gehört Fund 57 im Modulkopf nachgezogen, statt diesen Test \
             anzupassen"
        );
        // Und die Gegenrichtung: Ganz zahnlos ist der Parse nicht, sonst
        // wäre er nur Aufwand. Er fängt die Längenköpfe.
        assert!(
            durch < VERSUCHE,
            "kein einziger verstümmelter Block wurde abgelehnt: dann prüft \
             dieser Validator gar nichts"
        );
    }

    #[test]
    fn fremde_topics_bleiben_unberuehrt() {
        // Der Knoten darf hier nicht strenger sein als myl-net, sonst
        // verwirft er, was die Netzschicht bereits geprüft hat.
        let v = ProtokollValidator::default();
        assert!(v.validate(GossipTopic::PoiBundles, &[0xFF; 32]));
    }

    /// ⚑ **Ein unbekannter Herausforderer wird nicht abgewiesen**
    /// (Fund 96).
    ///
    /// Hier stand bis zum 2026-08-29 `validate(Challenges, &[]) == true`,
    /// und das galt nur, weil es den Zweig gar nicht gab. Jetzt gibt es
    /// ihn, und die Regel dahinter ist eine andere geworden: Ein
    /// Herausforderer ist ein Miner des redundanten Pods und muss in der
    /// Teilnehmerliste nicht stehen. Wer ihn deshalb abwiese, verwürfe
    /// aus geratener Unkenntnis, und das trifft im Gossipsub-Scoring den
    /// Ehrlichen.
    #[test]
    fn eine_anfechtung_eines_unbekannten_geht_durch() {
        // Leerer Validatorsatz: niemand ist bekannt.
        let v = ProtokollValidator::default();
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[7u8; 32]).expect("Schlüssel");
        let pk = sk.public_key().expect("Punkt");
        let mut c = myl_types::Challenge {
            segment_id: myl_types::ids::SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: myl_types::ids::MinerId::new([1u8; 32]),
            redundant_miner: myl_types::ids::MinerId::aus_schluessel(&pk),
            primary_hash: Hash::sha256(b"a"),
            redundant_hash: Hash::sha256(b"b"),
            timestamp_ms: 1_700_000_000_000,
            signature: myl_types::bls::BlsSignature([0u8; 96]),
        };
        c.signiere(&sk).expect("signieren");
        assert!(v.validate(GossipTopic::Challenges, &borsh::to_vec(&c).unwrap()));
    }

    /// ⚑ **Ein bekannter Herausforderer mit falscher Unterschrift
    /// dagegen schon.** Das ist keine Unkenntnis, sondern ein Befund.
    #[test]
    fn eine_anfechtung_eines_bekannten_mit_falscher_unterschrift_wird_abgewiesen() {
        let satz = crate::validatorsatz::Validatorsatz::aus_namen(&["alpha"]);
        let v = ProtokollValidator::mit(satz);
        let pk = crate::validatorsatz::probe_schluessel("alpha")
            .expect("Schlüssel")
            .public_key()
            .expect("Punkt");
        let c = myl_types::Challenge {
            segment_id: myl_types::ids::SegmentId::new([1u8; 32]),
            first_divergence: 3,
            primary_miner: myl_types::ids::MinerId::new([1u8; 32]),
            redundant_miner: myl_types::ids::MinerId::aus_schluessel(&pk),
            primary_hash: Hash::sha256(b"a"),
            redundant_hash: Hash::sha256(b"b"),
            timestamp_ms: 1_700_000_000_000,
            // Nicht unterschrieben, obwohl der Aussteller bekannt ist.
            signature: myl_types::bls::BlsSignature([0u8; 96]),
        };
        assert!(!v.validate(GossipTopic::Challenges, &borsh::to_vec(&c).unwrap()));
    }

    /// Und eine echte, unterschriebene Anfechtung eines bekannten
    /// Herausforderers kommt durch. Gegenprobe zum Test darüber: Ohne
    /// sie prüfte er nur, dass irgendetwas abgewiesen wird.
    #[test]
    fn eine_unterschriebene_anfechtung_eines_bekannten_kommt_durch() {
        let satz = crate::validatorsatz::Validatorsatz::aus_namen(&["alpha"]);
        let v = ProtokollValidator::mit(satz);
        let c = crate::probe::probe_challenge("alpha", 3).expect("Probe-Anfechtung");
        assert!(v.validate(GossipTopic::Challenges, &borsh::to_vec(&c).unwrap()));
    }

    #[test]
    fn eine_konsensnachricht_kommt_durch_ein_anhaengsel_nicht() {
        use myl_consensus::bft::Vote;
        use myl_types::bls::BlsSecretKey;
        let sk = BlsSecretKey::key_gen(&[7u8; 32]).unwrap();
        let h = Hash::sha256(b"block");
        let n = Konsensnachricht::Vote(Vote {
            round: 3,
            block_hash: h,
            voter: myl_types::ids::MinerId::new([1u8; 32]),
            signature: sk.sign(b"egal").unwrap(),
        });
        let v = ProtokollValidator::default();
        let mut daten = borsh::to_vec(&n).unwrap();
        assert!(v.validate(GossipTopic::Consensus, &daten));

        // Ein Anhängsel ändert die Nachrichten-Id, nicht den Inhalt:
        // dieselbe Stimme liefe beliebig oft durchs Netz.
        daten.push(0);
        assert!(
            !v.validate(GossipTopic::Consensus, &daten),
            "ein Anhängsel hinter einer gültigen Stimme kam durch"
        );
        assert!(!v.validate(GossipTopic::Consensus, &[]));
    }

    #[test]
    fn ein_block_wird_nicht_als_konsensnachricht_gelesen() {
        let daten = borsh::to_vec(&beispielblock()).unwrap();
        let v = ProtokollValidator::default();
        assert!(
            !v.validate(GossipTopic::Consensus, &daten),
            "ein Block las sich als Konsensnachricht"
        );
    }

    /// **A10: Ein gültiges Attest kommt durch, ein gefälschtes nicht.**
    #[test]
    fn a10_atteste_werden_gegen_den_validatorsatz_geprueft() {
        use myl_types::latency_attest::{BlsSignatureBytes, PeerIdBytes};
        use myl_types::LatencyAttest;

        let v = ProtokollValidator::mit(Validatorsatz::aus_namen(&["alpha"]));
        let mut a = LatencyAttest {
            issuer: crate::validatorsatz::probe_kennung("alpha").unwrap(),
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            latencies: vec![(PeerIdBytes([3u8; 32]), 30)],
            signature: BlsSignatureBytes([0u8; 96]),
        };
        // Unsigniert: durchgefallen.
        assert!(
            !v.validate(GossipTopic::LatencyAttests, &borsh::to_vec(&a).unwrap()),
            "ein unsigniertes Attest kam durch"
        );

        a.sign(&crate::validatorsatz::probe_schluessel("alpha").unwrap()).unwrap();
        assert!(
            v.validate(GossipTopic::LatencyAttests, &borsh::to_vec(&a).unwrap()),
            "das eigene, gültig signierte Attest wurde abgewiesen"
        );

        // Latenzwert gefälscht: durchgefallen.
        a.latencies[0].1 = 1;
        assert!(!v.validate(GossipTopic::LatencyAttests, &borsh::to_vec(&a).unwrap()));
    }

    #[test]
    fn a10_ohne_validatorsatz_kommt_kein_attest_durch() {
        // Der Vorgabezustand: Wer niemanden kennt, kann nichts prüfen
        // und darf nichts annehmen.
        let v = ProtokollValidator::default();
        assert_eq!(v.validatoren().anzahl(), 0);
        assert!(!v.validate(GossipTopic::LatencyAttests, &[0xAB; 40]));
    }
}
