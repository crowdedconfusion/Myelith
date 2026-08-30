//! On-Chain-Schiedsrunde (adjudicate) — Whitepaper Anhang A.4, Kap. 6.6.
//!
//! Die Schiedsrunde wird durchgeführt, wenn das Bisektions-Spiel eine
//! Abweichung identifiziert hat. Das Validatoren-Komitee führt einen
//! Shard-Forward durch und vergleicht den Hash mit dem behaupteten Hash.
//!
//! **Ablauf:**
//! 1. Checker fordert Aktivierung a_{j-1} an — unter Nennung des in der
//!    Spur festgeschriebenen Hashes h(a_{j-1})
//! 2. Angeklagter legt a_{j-1} offen
//! 3. Komitee prüft, dass die offengelegte Aktivierung **genau die aus
//!    der Spur** ist
//! 4. Validatoren-Komitee führt den Shard-Forward durch
//! 5. Hash-Vergleich: Übereinstimmung = unschuldig, Abweichung = schuldig
//!
//! ## Warum Schritt 3 nicht fehlen darf (Fund A11)
//!
//! Bis v0.2.6 prüfte `adjudicate()` die offengelegte Aktivierung nur
//! **gegen sich selbst**:
//!
//! ```text
//! let computed = Hash::sha256(&response.activation);
//! if computed != response.activation_hash { return Guilty; }
//! ```
//!
//! Beide Werte kamen aus derselben Antwort — die Prüfung war
//! tautologisch und stellte nur fest, dass der Angeklagte in sich
//! konsistent geantwortet hat. Der `AdjudicationRequest` trug keinen
//! Hash von a_{j-1}, also gab es nichts, woran die Eingabe gebunden
//! gewesen wäre. Ein Angeklagter, der eine **andere** Eingabe findet,
//! die unter seiner Ausführung den erwarteten Ausgabe-Hash ergibt,
//! wurde freigesprochen.
//!
//! Das untergräbt die zentrale Zusage aus Kap. 6.6 („Die
//! Schuldzuweisung ist eindeutig, weil das Ergebnis kanonisch ist"):
//! kanonisch ist das Ergebnis nur **bezogen auf die committete
//! Eingabe**. Der Request trägt deshalb jetzt `input_hash` aus
//! `Segment.trace[j-1]`, und die offengelegte Aktivierung wird dagegen
//! geprüft.
//!
//! **Konsens-Feld:** Die Schiedsrunden-Logik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MinerId, SegmentId};
use borsh::{BorshDeserialize, BorshSerialize};

/// Eine Schiedsrunden-Anfrage.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AdjudicationRequest {
    /// Segment-ID.
    pub segment_id: SegmentId,
    /// Position der abweichenden **Layer**.
    pub divergence_position: usize,
    /// Checker (Miner-ID).
    pub checker: MinerId,
    /// Angeklagter (Miner-ID).
    pub accused: MinerId,
    /// Hash der **committeten** Eingabe-Aktivierung a_{j-1} aus der
    /// Segment-Spur (`Segment.trace[divergence_position - 1]`, bzw. das
    /// Eingangs-Commitment des Segments bei Position 0).
    ///
    /// Ohne dieses Feld wäre die Offenlegung an nichts gebunden: der
    /// Angeklagte könnte eine beliebige Eingabe liefern, die unter
    /// seiner Ausführung zufällig den erwarteten Ausgabe-Hash ergibt.
    pub input_hash: Hash,
    /// Der Hash, den der **Angeklagte** für a_j zugesichert hat, also
    /// sein `trace[divergence_position]`.
    ///
    /// # ⚑ Warum nicht mehr der Wert des Checkers (Fund 101, 2026-08-30)
    ///
    /// Hier stand `expected_hash`, „erwarteter Hash der Ausgabe (vom
    /// Checker)", und das Urteil verglich die Nachrechnung damit. Wer
    /// dort eine beliebige dritte Zahl eintrug, machte aus einem
    /// **ehrlichen** Angeklagten einen schuldigen: Die Nachrechnung
    /// stimmte mit seiner wahren Ausgabe überein, aber eben nicht mit
    /// der Behauptung des Anklägers.
    ///
    /// Die Frage der Schiedsrunde lautet nicht „hatte der Ankläger
    /// recht", sondern **„hat der Angeklagte gerechnet, was er
    /// zugesichert hat"**. Verglichen wird deshalb gegen seine eigene
    /// Zusicherung, und der Ankläger kommt im Urteil gar nicht mehr vor.
    ///
    /// **Gebunden ist dieser Wert über die Bündelwurzel:** Er ist ein
    /// Blatt der Spurwurzel des Segments, und die steht seit Fund 100
    /// in `segments_root`. Der Beweis dafür ist ein Merkle-Pfad, den
    /// vorlegt, wer sich darauf beruft; siehe
    /// [`zusicherung_ist_belegt`].
    pub zugesichert: Hash,
    /// Die offengelegte Eingabe-Aktivierung a_{j-1}, **vom Ankläger**.
    ///
    /// # ⚑ Warum der Ankläger sie liefert und nicht der Angeklagte (E10)
    ///
    /// Die Bisektion endet an der **ersten** Abweichung. Das heißt per
    /// Definition, dass sich beide Seiten bei `j-1` einig sind, Bit für
    /// Bit. **Der Ankläger hat den strittigen Wert also ohnehin**, denn
    /// er hat das Segment gerade nachgerechnet, das ist die Anfechtung.
    ///
    /// Damit muss der Angeklagte **nichts aufbewahren**. Vorher hielt
    /// jeder Shard je Segment eine Aktivierung vor, über die Streitfrist
    /// zwischen 65 und 260 GiB je Knoten; für niedrigschwellige
    /// Teilhabe zu viel.
    ///
    /// **Lügen hilft nicht:** Der Wert ist über `input_hash` an
    /// `trace[j-1]` gebunden, und das ist ein Eintrag, den der
    /// **Angeklagte selbst** zugesichert hat. Ein untergeschobener Wert
    /// fällt beim Hashvergleich durch.
    ///
    /// Dieselbe Bauart tragen die optimistischen Rollups: Nach der
    /// Bisektion auf einen Schritt legt der Anfechtende den Vorzustand
    /// vor, geprüft gegen den gemeinsam bezeugten Hash. Niemand hält die
    /// ganze Ausführungsspur vor; gehalten werden Zusicherungen, und die
    /// Bytes bringt der mit, der gewinnen will.
    pub aktivierung: Vec<u8>,
    /// Die Spurwurzel des Segments, wie der Pod sie im Bündel bezeugt
    /// hat.
    ///
    /// ⚑ **Der einzige Wert dieser Anfrage, den [`adjudicate`] nicht
    /// selbst prüfen kann.** Er stammt aus dem angenommenen PoI-Bündel,
    /// und dort deckt ihn die Aggregat-Signatur der Pod-Mitglieder. Der
    /// Aufrufer muss ihn von dort nehmen und nicht aus der Anfrage
    /// glauben; alles Weitere hängt dann daran und wird hier geprüft.
    pub spurwurzel: myl_types::ids::MerkleRoot,
    /// Beweis, dass [`Self::input_hash`] in der bezeugten Kette an der
    /// Stelle `divergence_position` steht, also die **Eingabe** der
    /// strittigen Layer ist.
    pub beweis_eingabe: myl_types::MerkleProof,
    /// Beweis, dass [`Self::zugesichert`] an der Stelle
    /// `divergence_position + 1` steht, also die **Ausgabe** derselben
    /// Layer.
    ///
    /// Die Kette ist `[Eingang] ++ Spur` (Fund 102): `kette[j]` ist die
    /// Eingabe der Layer `j`, `kette[j+1]` ihre Ausgabe. Ein Index, und
    /// beide Werte hängen an derselben Wurzel.
    pub beweis_zusicherung: myl_types::MerkleProof,
    /// Epoche, in der die Anfrage gestellt wurde.
    ///
    /// ⚑ Die Frist rechnet sich daraus, und zwar **von jedem
    /// Beteiligten gleich**: `gestellt_in + ANTWORTFRIST_EPOCHEN` ist
    /// eine Zahl, keine Behauptung über eine Uhr. Ohne dieses Feld gäbe
    /// es keinen Bezugspunkt, und die Frist wäre wieder das, was sie
    /// vorher war: das Ermessen dessen, der fragt.
    pub gestellt_in: EpochId,
}

/// Ergebnis der Schiedsrunde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjudicationResult {
    /// Angeklagter ist unschuldig (Hash stimmt überein).
    Innocent,
    /// Angeklagter ist schuldig: Die Nachrechnung weicht von seiner
    /// eigenen Zusicherung ab.
    Guilty,
    /// Die Anfrage taugt nicht, es fällt **kein Urteil**.
    ///
    /// ⚑ **Der Unterschied zu einem Schuldspruch ist der ganze Punkt.**
    /// Seit E10 legt der **Ankläger** die Aktivierung vor. Eine, die
    /// nicht zur Zusicherung bei `j-1` hasht, oder eine, an der die
    /// Ausführung scheitert, sagt etwas über den Ankläger aus und nichts
    /// über den Angeklagten. Wer das als „schuldig" buchte, ließe jeden
    /// verurteilen, der eine kaputte Anfrage geschickt bekommt.
    ///
    /// Hier standen bis zum 2026-08-30 `NoResponse` und `Offen`. Beide
    /// sind entfallen, weil niemand mehr antwortet: Die Anfrage ist
    /// vollständig, das Komitee rechnet und urteilt in einem Zug. Damit
    /// verliert auch kein ehrlicher Knoten mehr seinen Stake, nur weil
    /// er gerade nicht erreichbar war.
    Untauglich,
}

/// Fehler bei der Schiedsrunde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjudicationError {
    /// Segment-ID stimmt nicht überein.
    SegmentMismatch,
    /// Position stimmt nicht überein.
    PositionMismatch,
    /// Hash der Aktivierung stimmt nicht mit dem behaupteten Hash überein.
    HashMismatch,
}

impl std::fmt::Display for AdjudicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SegmentMismatch => write!(f, "Segment-ID stimmt nicht überein"),
            Self::PositionMismatch => write!(f, "Position stimmt nicht überein"),
            Self::HashMismatch => write!(f, "Hash der Aktivierung stimmt nicht überein"),
        }
    }
}

impl std::error::Error for AdjudicationError {}

/// Trait für Shard-Forward-Ausführung.
///
/// Dieser Trait definiert die Schnittstelle für die Ausführung eines
/// Shard-Forwards. Die konkrete Implementierung erfolgt durch Integration
/// mit INTEGER_LLMs Runtime.
pub trait ShardExecutor {
    /// Rechnet **eine Layer** nach.
    ///
    /// **Parameter:**
    /// - `activation`: Eingabe-Aktivierung a_{j-1}
    /// - `layer_group_index`: Index der Layer im Modell
    ///
    /// **Returns:** Ausgabe-Aktivierung a_j
    fn execute_shard(
        &self,
        activation: &[u8],
        layer_group_index: usize,
    ) -> Result<Vec<u8>, AdjudicationError>;
}

/// Prüft, dass eine Zusicherung wirklich in der Spur des Angeklagten
/// steht.
///
/// # ⚑ Wozu, wenn [`adjudicate`] doch dagegen urteilt (Fund 100/101)
///
/// Gegen die Zusicherung zu urteilen ist nur dann etwas wert, wenn sie
/// **die des Angeklagten** ist. Sonst wandert der Fehler nur von einem
/// Feld ins andere: Vorher stand die Behauptung des Anklägers in
/// `expected_hash`, danach stünde sie in `zugesichert`.
///
/// Der Beweis ist ein Merkle-Pfad in der Spurwurzel des Segments, und
/// die Spurwurzel steht seit Fund 100 in der Bündelwurzel, die der Pod
/// unterschrieben eingereicht hat. Wer sich auf eine Zusicherung
/// beruft, legt den Pfad vor; wer ihn nicht vorlegt, hat nichts gesagt.
///
/// **Was diese Funktion nicht leistet:** Sie prüft den Pfad gegen die
/// übergebene Spurwurzel. Dass diese Wurzel zum eingereichten Bündel
/// gehört, ist eine zweite Frage und hängt an der Bündelwurzel und der
/// Kette; sie gehört dorthin, wo das Bündel liegt, und nicht hierher.
pub fn zusicherung_ist_belegt(
    spurwurzel: &myl_types::ids::MerkleRoot,
    position: usize,
    zugesichert: &Hash,
    beweis: &myl_types::MerkleProof,
) -> bool {
    // ⚑ **Die Stelle gehört zur Aussage.** Ein Merkle-Beweis belegt
    // „dieses Blatt steht an Index `leaf_index`". Wer den Index nicht
    // vergleicht, belegt nur „irgendwo in dieser Spur", und dann darf
    // ein Ankläger den Eintrag einer **anderen** Layer als Zusicherung
    // für die strittige ausgeben.
    //
    // Der erste Entwurf dieser Funktion nahm `position` entgegen und
    // warf sie mit `let _ = position;` weg. Beim kritischen Nachlesen
    // gefunden, nicht beim Schreiben.
    if beweis.leaf_index != position as u64 {
        return false;
    }
    beweis.verify_hashed(
        &Hash(*spurwurzel.as_bytes()),
        &myl_types::leaf_hash(zugesichert.as_bytes()),
    )
}

/// Führt die Schiedsrunde durch.
///
/// **Parameter:**
/// - `request`: die Anfrage, samt der vom Ankläger offengelegten
///   Aktivierung
/// - `executor`: Shard-Executor für den Forward-Pass **einer** Layer
///
/// # ⚑ Warum niemand mehr antwortet (E10, 2026-08-30)
///
/// Bis dahin legte der **Angeklagte** die Aktivierung offen, und blieb
/// er stumm, hieß das schuldig. Zwei Dinge waren daran falsch. Erstens
/// musste er dafür je Segment eine Aktivierung aufbewahren, über die
/// Streitfrist zwischen 65 und 260 GiB je Knoten. Zweitens verlor ein
/// **ehrlicher** Knoten mit einem Ausfall seinen Stake.
///
/// Beides fällt weg, weil der Ankläger den Wert ohnehin hat: Die
/// Bisektion endet an der ersten Abweichung, bei `j-1` sind sich beide
/// einig. Der Angeklagte wird gar nicht mehr gefragt; das Komitee
/// rechnet die eine Layer nach und hält das Ergebnis gegen seine
/// Zusicherung.
///
/// **Die Prüfkette, billig vor teuer:**
///
/// 1. Der offengelegte Wert hasht zu `input_hash`, also zu dem, was der
///    Angeklagte bei `j-1` zugesichert hat. Ein untergeschobener Wert
///    fällt hier durch.
/// 2. Die Layer wird nachgerechnet.
/// 3. Das Ergebnis wird gegen `zugesichert` gehalten, also gegen die
///    eigene Aussage des Angeklagten (Fund 101).
///
/// **Was der Aufrufer leisten muss:** `input_hash` und `zugesichert`
/// gegen die Spurwurzel des Segments belegen, siehe
/// [`zusicherung_ist_belegt`]. Ohne das urteilt diese Funktion über
/// zwei Zahlen, die der Ankläger sich ausgedacht hat.
pub fn adjudicate(
    request: &AdjudicationRequest,
    executor: &dyn ShardExecutor,
) -> AdjudicationResult {
    // ⚑ **Zuerst die Bindung, dann alles andere.** Ohne sie urteilte
    // diese Funktion über zwei Zahlen, die der Ankläger sich ausgedacht
    // hat, und der Fehler aus Fund 101 wäre nur ein Feld weitergewandert.
    //
    // Hier stand bis zum 2026-08-30 nur ein Doc-Kommentar, der es dem
    // Aufrufer auftrug. Genau so entstehen die Felder, die niemand
    // prüft; diese Sitzung hat drei davon gefunden.
    let j = request.divergence_position;
    if !zusicherung_ist_belegt(
        &request.spurwurzel,
        j,
        &request.input_hash,
        &request.beweis_eingabe,
    ) {
        return AdjudicationResult::Untauglich;
    }
    if !zusicherung_ist_belegt(
        &request.spurwurzel,
        j + 1,
        &request.zugesichert,
        &request.beweis_zusicherung,
    ) {
        return AdjudicationResult::Untauglich;
    }

    // Der offengelegte Wert muss der zugesicherte Eingang sein.
    if Hash::sha256(&request.aktivierung) != request.input_hash {
        return AdjudicationResult::Untauglich;
    }


    // Shard-Forward durchführen
    match executor.execute_shard(&request.aktivierung, request.divergence_position) {
        Ok(output_activation) => {
            // Hash der Ausgabe-Aktivierung berechnen
            let output_hash = Hash::sha256(&output_activation);

            // ⚑ Gegen die **Zusicherung des Angeklagten**, nicht gegen
            // die Behauptung des Anklägers (Fund 101). Wer gerechnet
            // hat, was er zugesichert hat, ist unschuldig, gleich was
            // ein Dritter darüber sagt.
            if output_hash == request.zugesichert {
                AdjudicationResult::Innocent
            } else {
                AdjudicationResult::Guilty
            }
        }
        Err(_) => {
            // ⚑ Fehlgeschlagene Ausführung ist **kein** Schuldspruch
            // mehr. Vorher legte der Angeklagte die Eingabe vor, und wer
            // eine lieferte, an der die Rechnung abstürzt, sollte daraus
            // keinen Vorteil ziehen. Jetzt legt der **Ankläger** sie vor,
            // und dieselbe Regel verurteilte den Falschen.
            AdjudicationResult::Untauglich
        }
    }
}

/// Mock-Shard-Executor für Tests.
#[cfg(test)]
pub struct MockShardExecutor {
    output: Vec<u8>,
}

#[cfg(test)]
impl MockShardExecutor {
    pub fn new(output: Vec<u8>) -> Self {
        Self { output }
    }
}

#[cfg(test)]
impl ShardExecutor for MockShardExecutor {
    fn execute_shard(
        &self,
        _activation: &[u8],
        _layer_group_index: usize,
    ) -> Result<Vec<u8>, AdjudicationError> {
        Ok(self.output.clone())
    }
}

/// Ein Executor, dessen Ausführung scheitert.
///
/// Für die Frage, was passiert, wenn die vom Ankläger gelieferte
/// Eingabe die Rechnung sprengt.
#[cfg(test)]
struct KaputterExecutor;

#[cfg(test)]
impl ShardExecutor for KaputterExecutor {
    fn execute_shard(
        &self,
        _activation: &[u8],
        _layer_group_index: usize,
    ) -> Result<Vec<u8>, AdjudicationError> {
        Err(AdjudicationError::HashMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_miner(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    fn test_segment_id(byte: u8) -> SegmentId {
        SegmentId::new([byte; 32])
    }

    fn test_hash(byte: u8) -> Hash {
        Hash::sha256(&[byte])
    }

    /// Eine vollständige Anfrage samt Beweisen, wie der Ankläger sie
    /// seit dem 2026-08-30 stellen muss.
    ///
    /// Die Kette ist `[Eingang] ++ Spur`: An Stelle `j` steht die
    /// Eingabe der strittigen Layer, an `j+1` ihre Ausgabe.
    fn anfrage(eingabe: &[u8], zugesichert: Hash) -> AdjudicationRequest {
        use myl_types::merkle::MerkleTree;
        const J: usize = 5;
        let eingang = Hash::sha256(eingabe);
        // Eine Kette, in der die beiden Werte an den richtigen Stellen
        // stehen; der Rest ist beliebig.
        let mut kette: Vec<Hash> = (0..9u8).map(|i| Hash::sha256(&[i])).collect();
        kette[J] = eingang;
        kette[J + 1] = zugesichert;
        let refs: Vec<&[u8]> = kette.iter().map(|h| h.as_bytes().as_slice()).collect();
        let baum = MerkleTree::new(&refs).expect("Baum");
        AdjudicationRequest {
            segment_id: test_segment_id(1),
            divergence_position: J,
            checker: test_miner(2),
            accused: test_miner(3),
            input_hash: eingang,
            zugesichert,
            aktivierung: eingabe.to_vec(),
            spurwurzel: myl_types::ids::MerkleRoot::new(baum.root().0),
            beweis_eingabe: baum.proof(J).expect("Beweis"),
            beweis_zusicherung: baum.proof(J + 1).expect("Beweis"),
            gestellt_in: EpochId(5),
        }
    }

    /// ⚑ **Ohne Beweis kein Urteil** (der eigentliche Punkt).
    ///
    /// Hier stand bis zum 2026-08-30 nur ein Doc-Kommentar, der die
    /// Bindung dem Aufrufer auftrug. Jetzt prüft `adjudicate` sie
    /// selbst, und eine Anfrage ohne tragenden Beweis führt zu keinem
    /// Urteil, weder so noch so.
    #[test]
    fn eine_anfrage_ohne_tragenden_beweis_urteilt_nicht() {
        let executor = MockShardExecutor::new(vec![9, 9]);
        let gut = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        assert_eq!(adjudicate(&gut, &executor), AdjudicationResult::Innocent);

        // Eine andere Wurzel: die Beweise passen nicht mehr.
        let mut fremd = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        fremd.spurwurzel = myl_types::ids::MerkleRoot::new([7u8; 32]);
        assert_eq!(adjudicate(&fremd, &executor), AdjudicationResult::Untauglich);

        // ⚑ Und der Beweis der falschen Stelle: Ohne den Indexvergleich
        // ginge er durch, denn er ist in sich stimmig.
        let mut verschoben = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        verschoben.beweis_zusicherung = verschoben.beweis_eingabe.clone();
        assert_eq!(
            adjudicate(&verschoben, &executor),
            AdjudicationResult::Untauglich
        );
    }

    /// Wer gerechnet hat, was er zugesichert hat, ist unschuldig.
    #[test]
    fn wer_seine_zusicherung_haelt_ist_unschuldig() {
        let executor = MockShardExecutor::new(vec![9, 9]);
        let a = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        assert_eq!(adjudicate(&a, &executor), AdjudicationResult::Innocent);
    }

    /// ⚑ **Fund 101: Geurteilt wird gegen die eigene Zusicherung.**
    ///
    /// Hier hieß das Feld `expected_hash` und trug den Wert **des
    /// Anklägers**; das Urteil verglich die Nachrechnung damit. Wer dort
    /// eine dritte Zahl eintrug, verurteilte den, der genau das
    /// gerechnet hatte, was er zugesichert hatte.
    ///
    /// Der Test führt beide Seiten vor: Dieselbe Nachrechnung, einmal
    /// gegen die eingehaltene und einmal gegen eine gebrochene
    /// Zusicherung.
    #[test]
    fn wer_seine_zusicherung_bricht_ist_schuldig() {
        let executor = MockShardExecutor::new(vec![9, 9]);
        let a = anfrage(&[1, 2], Hash::sha256(b"etwas anderes"));
        assert_eq!(adjudicate(&a, &executor), AdjudicationResult::Guilty);
    }

    /// ⚑ **Eine untergeschobene Eingabe fällt durch, und zwar als
    /// untauglich, nicht als Schuldspruch.**
    ///
    /// Sie ist an `input_hash` gebunden, also an das, was der
    /// **Angeklagte** bei `j-1` zugesichert hat. Der Ankläger kann
    /// deshalb nichts unterschieben. Und weil die Eingabe seit E10 von
    /// ihm kommt, sagt eine falsche etwas über ihn aus und nichts über
    /// den Angeklagten: Sie darf niemanden verurteilen.
    #[test]
    fn eine_untergeschobene_eingabe_verurteilt_niemanden() {
        let executor = MockShardExecutor::new(vec![9, 9]);
        let mut a = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        a.aktivierung = vec![7, 7]; // hasht nicht mehr zu input_hash
        assert_eq!(adjudicate(&a, &executor), AdjudicationResult::Untauglich);
    }

    /// Und ebenso eine Eingabe, an der die Ausführung scheitert.
    ///
    /// ⚑ Vorher war das ein Schuldspruch, und das war richtig, solange
    /// der **Angeklagte** die Eingabe lieferte. Seit sie vom Ankläger
    /// kommt, träfe dieselbe Regel den Falschen.
    #[test]
    fn eine_eingabe_die_die_ausfuehrung_sprengt_verurteilt_niemanden() {
        let a = anfrage(&[1, 2], Hash::sha256(&[9u8, 9]));
        assert_eq!(
            adjudicate(&a, &KaputterExecutor),
            AdjudicationResult::Untauglich
        );
    }

    /// ⚑ **Die Zusicherung muss belegt sein, sonst wandert der Fehler
    /// nur ein Feld weiter** (Fund 100).
    #[test]
    fn eine_zusicherung_ohne_beleg_traegt_nicht() {
        use myl_types::merkle::MerkleTree;
        let spur: Vec<Hash> = (0..7u8).map(|i| Hash::sha256(&[i])).collect();
        let refs: Vec<&[u8]> = spur.iter().map(|h| h.as_bytes().as_slice()).collect();
        let baum = MerkleTree::new(&refs).expect("Baum");
        let wurzel = myl_types::ids::MerkleRoot::new(baum.root().0);
        let beweis = baum.proof(4).expect("Beweis");

        assert!(
            zusicherung_ist_belegt(&wurzel, 4, &spur[4], &beweis),
            "der echte Eintrag an seiner Stelle muss durchgehen"
        );
        assert!(
            !zusicherung_ist_belegt(&wurzel, 4, &spur[5], &beweis),
            "ein anderer Eintrag an derselben Stelle nicht"
        );
        assert!(
            !zusicherung_ist_belegt(&wurzel, 4, &Hash::sha256(b"erfunden"), &beweis),
            "eine erfundene Zusicherung schon gar nicht"
        );
    }

    #[test]
    fn adjudication_request_borsh_roundtrip() {
        let a = anfrage(&[1, 2, 3], test_hash(1));
        let bytes = borsh::to_vec(&a).expect("Serialisierung");
        let zurueck: AdjudicationRequest = borsh::from_slice(&bytes).expect("Rücklesen");
        assert_eq!(zurueck, a);
    }

    #[test]
    fn adjudication_result_variants() {
        assert_ne!(AdjudicationResult::Innocent, AdjudicationResult::Guilty);
        assert_ne!(AdjudicationResult::Guilty, AdjudicationResult::Untauglich);
    }
}
