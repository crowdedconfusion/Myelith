//! Pod-Koordinator: Micro-Batching + Pipeline-Dispatch + PoI-Aggregation
//! (Anhang A.3, `coordinator_loop`).
//!
//! Der Koordinator sammelt eingehende Anfragen innerhalb des
//! Micro-Batching-Fensters `WINDOW_MS` (Design-Entscheidung 2026-08-13:
//! Default 250 ms, kalibriert wird in Phase 2.1), weist Session- und
//! Segment-Ids zu, schickt die Token-Nachrichten in die Shard-Pipeline
//! und sammelt die generierten Tokens. Abgeschlossene Segmente werden zu
//! PoI-Bündeln aggregiert.
//!
//! Für Phase 1 läuft die Pipeline in-Prozess (die Shards werden direkt
//! aufgerufen); die Netzwerk-Variante (echte Nodes) folgt in den
//! Härtungs-Phasen. Die Determinismus-Garantie gilt in beiden Fällen:
//! derselbe Prompt ⇒ bitgleiche Token-Sequenz.

use std::sync::Arc;

use myl_types::bls::{aggregate_signatures, BlsSignature};
use myl_types::core_types::{segments_root, PoIBundle};
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};

use crate::shard::{ShardNode, ShardOut};
use crate::wire::{self, PodMessage, FLAG_SAMPLE};

/// Default für das Micro-Batching-Fenster (Design-Entscheidung 2026-08-13).
pub const DEFAULT_WINDOW_MS: u64 = 250;

/// Ein abgeschlossenes Segment mit den gesammelten Übergangs-Signaturen.
#[derive(Debug, Clone)]
pub struct CompletedSegment {
    pub id: SegmentId,
    pub trace: Vec<[u8; 32]>,
    /// Commitment auf die **Eingabe** des Segments.
    ///
    /// # ⚑ Warum es das braucht (Fund 102, 2026-08-30)
    ///
    /// Die Schiedsrunde bindet die strittige Eingabe an
    /// `trace[j-1]`, also an die Ausgabe der Layer davor. **Bei `j = 0`
    /// gibt es keine Layer davor.** Dort ist die Eingabe des Segments
    /// gemeint, und die stand nirgends: `myl_types::Segment` führt ein
    /// `input_commitment`, aber diesen Typ erzeugt niemand.
    ///
    /// Seit E10 legt der **Ankläger** die strittige Eingabe vor. An der
    /// ersten Layer prüfte die Schiedsrunde damit `hash(eingabe)` gegen
    /// einen Hash, den derselbe Ankläger daneben schreibt: eine
    /// tautologische Prüfung, und genau der Fehler, den Fund A11 an
    /// anderer Stelle schon einmal hatte.
    ///
    /// Der Wert ist der Hash der Token-Nutzlast, also klein und von
    /// jedem nachrechenbar, der die Anfrage kennt.
    pub eingangs_commitment: [u8; 32],
    /// Die Merkle-Wurzel über [`Self::zusicherungen`].
    ///
    /// ⚑ **Fund 100:** Bis zum 2026-08-30 ging in die Bündelwurzel nur
    /// die `SegmentId`, und die ist `(Sitzung, Position)` mit Nullen
    /// aufgefüllt. Das Bündel beanspruchte damit Arbeit, **ohne zu
    /// sagen, was gerechnet wurde**; die Spur lag nur örtlich hier und
    /// war an nichts gebunden. Die Schiedsrunde hatte deshalb kein
    /// „behauptet", gegen das sie hätte prüfen können, nur zwei einander
    /// widersprechende Aussagen.
    pub spurwurzel: myl_types::MerkleRoot,
    pub signatures: Vec<BlsSignature>,
    pub pod_path: Vec<MinerId>,
    /// Die Token-Position, die dieses Segment gerechnet hat.
    ///
    /// Ein Segment ist genau ein Vorwärtspass, also genau ein
    /// Token-Forward-Äquivalent. Die vTFE-Gutschrift folgt damit aus der
    /// **Zahl der Segmente**; ein eigenes Feld dafür braucht es nicht
    /// mehr.
    ///
    /// **Prefill zählt mit, und das ist Absicht:** Eine Prompt-Position
    /// emittiert kein Token, rechnet aber denselben vollständigen
    /// Vorwärtspass. Sie nicht zu vergüten hieße, geleistete Arbeit nicht
    /// zu bezahlen.
    pub position: u64,
}

impl CompletedSegment {
    /// Die Kette der Zusicherungen: `[Eingang] ++ Spur`.
    ///
    /// ⚑ **Der erste Eintrag ist der Grund, warum es diese Methode
    /// gibt** (Fund 102). `kette()[j]` ist die **Eingabe** der Layer `j`,
    /// `kette()[j+1]` ihre Ausgabe. Damit ist auch die Eingabe der
    /// **ersten** Layer beweisbar; ohne den Eintrag hinge sie an nichts,
    /// und die Schiedsrunde prüfte dort einen Hash des Anklägers gegen
    /// einen zweiten Hash desselben Anklägers.
    ///
    /// Die Wurzel über diese Kette ist [`Self::spurwurzel`].
    pub fn kette(&self) -> Vec<[u8; 32]> {
        let mut k = Vec::with_capacity(self.trace.len() + 1);
        k.push(self.eingangs_commitment);
        k.extend_from_slice(&self.trace);
        k
    }

    /// Der Beleg in der Form, die das Protokoll dafür vorsieht.
    ///
    /// # ⚑ Warum es diese Umrechnung gibt und keinen zweiten Typ
    ///
    /// `myl_types::Segment` beschreibt seit dem 2026-08-13 genau diesen
    /// Beleg (Anhang A.1), und die Gateway-Planung setzt ihn voraus.
    /// Erzeugt hat ihn bis zum 2026-08-30 **niemand**: Was der Pod wirklich
    /// festhielt, war [`CompletedSegment`], und die beiden führten
    /// verschiedene Felder.
    ///
    /// Zwei Typen für dieselbe Sache sind eine zweite Quelle, und diese
    /// hier hat schon Schaden angerichtet: `Segment` trägt ein
    /// `input_commitment`, `CompletedSegment` hatte keines, und genau
    /// daraus entstand Fund 102, die tautologische Prüfung an der ersten
    /// Layer.
    ///
    /// Deshalb keine zweite Aufzeichnung, sondern eine **Projektion**:
    /// Was hier steht, folgt aus dem, was der Pod ohnehin hat.
    ///
    /// `model_version` ist der einzige Wert von außen, denn er gehört
    /// zum Modell und nicht zum Segment.
    pub fn zu_segment(&self, model_version: myl_types::ids::MerkleRoot) -> myl_types::Segment {
        myl_types::Segment {
            id: self.id,
            input_commitment: myl_types::hash::Hash(self.eingangs_commitment),
            model_version,
            pod_path: self.pod_path.clone(),
            // Die Ausgabe des Segments ist der letzte Spur-Eintrag.
            // `trace` ist beim Abschluss nie leer, das prüft
            // `segment_abschliessen`.
            output_commitment: myl_types::hash::Hash(
                *self.trace.last().expect("ein abgeschlossenes Segment hat eine Spur"),
            ),
            trace: self
                .trace
                .iter()
                .map(|h| myl_types::ids::ActivationHash::new(*h))
                .collect(),
            signatures: self.signatures.clone(),
        }
    }
}


/// Segment-Id aus Session-Id **und Position** (Anhang A.1: `h(session ‖
/// index)`; hier vereinfacht als zwei linksbündig eingetragene Felder).
///
/// **Ein Segment ist eine Position, also genau ein Vorwärtspass**
/// (Festlegung des Projektinhabers, 2026-08-23). Bis dahin trug die
/// Segment-Id nur die Session, ein ganzer Prompt lief also unter einer
/// einzigen Id, und Spur wie Datenarchiv behielten davon **nur die
/// letzte Position**: Beide wurden je Position überschrieben. Ein
/// Angeklagter hätte die Aktivierung jeder früheren Position nicht
/// liefern können, `adjudicate` hätte `NoResponse` gesehen, und das heißt
/// schuldig. Ein ehrlicher Knoten wäre geslasht worden.
///
/// Mit einem Segment je Position entfällt die Positionsachse, statt
/// nachgerüstet zu werden: Spur und Archiv tragen dieselbe Achse wie die
/// Bisektion, nämlich die Layer.
fn segment_id_from(session_id: u64, position: u64) -> SegmentId {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&session_id.to_le_bytes());
    bytes[8..16].copy_from_slice(&position.to_le_bytes());
    SegmentId::new(bytes)
}

/// Pod-Koordinator.
pub struct Coordinator {
    pub pod_id: PodId,
    pub epoch: EpochId,
    pub window_ms: u64,
    /// Die Shard-Pipeline in Reihenfolge (Shard 0 zuerst).
    shards: Vec<Arc<ShardNode>>,
    /// Abgeschlossene Segmente dieser Epoche.
    completed: Vec<CompletedSegment>,
}

impl Coordinator {
    pub fn new(pod_id: PodId, epoch: EpochId, shards: Vec<Arc<ShardNode>>, window_ms: u64) -> Self {
        Self {
            pod_id,
            epoch,
            window_ms,
            shards,
            completed: Vec::new(),
        }
    }

    /// Anzahl der Shards in der Pipeline.
    pub fn num_shards(&self) -> usize {
        self.shards.len()
    }

    /// Führt einen Prompt durch die Shard-Pipeline und liefert die
    /// generierten Tokens (deterministisch).
    ///
    /// `prompt_tokens` ist der Prompt; `max_new_tokens` begrenzt die
    /// Generation. Die Ausgabe ist bei identischem Prompt bitgleich.
    pub fn run_prompt(&mut self, session_id: u64, prompt_tokens: &[u32], max_new_tokens: u64) -> Vec<u32> {
        let mut generated = Vec::new();

        // 1) Prefill: Prompt-Tokens durch die Pipeline schicken (je Token
        //    eine Nachricht, Position 0..P-1). Das letzte Prompt-Token
        //    trägt FLAG_SAMPLE und löst das erste Sampling aus.
        let mut pending_feedback: Option<PodMessage> = None;
        for (i, tok) in prompt_tokens.iter().enumerate() {
            let is_last = i + 1 == prompt_tokens.len();
            let packed = wire::pack_tokens(&[*tok]);
            let flags = if is_last { FLAG_SAMPLE } else { 0 };
            let segment_id = segment_id_from(session_id, i as u64);
            // Das Commitment auf die Eingabe, bevor sie verschickt wird
            // (Fund 102): die Nutzlast dieser Nachricht.
            let eingang = crate::trace::activation_hash(&packed);
            let msg = PodMessage::token_input(segment_id, session_id, i as u64, packed, flags);
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg);
            self.segment_abschliessen(segment_id, i as u64, out_trace, out_sigs, eingang);
            if let Some(t) = token_opt {
                generated.push(t);
            }
            if feedback_opt.is_some() {
                pending_feedback = feedback_opt;
            }
        }

        // 2) Autoregressive Feedback-Schleife: das vom End-Shard
        //    erzeugte Feedback wird durch die Pipeline geschickt, bis das
        //    Budget erschöpft ist oder kein Feedback mehr kommt.
        while (generated.len() as u64) < max_new_tokens {
            let mut msg = match pending_feedback.take() {
                Some(m) => m,
                None => break,
            };
            // Die Feedback-Nachricht trägt die Segment-Id ihres Erzeugers.
            // Mit einem Segment je Position gehört sie auf die neue.
            let segment_id = segment_id_from(session_id, msg.position);
            msg.segment_id = segment_id;
            let position = msg.position;
            let eingang = crate::trace::activation_hash(&msg.payload);
            let (out_trace, out_sigs, token_opt, feedback_opt) = self.pump(&msg);
            self.segment_abschliessen(segment_id, position, out_trace, out_sigs, eingang);
            match token_opt {
                Some(t) => generated.push(t),
                None => break,
            }
            pending_feedback = feedback_opt;
        }

        generated
    }

    /// Vermerkt ein abgeschlossenes Segment, also einen Vorwärtspass.
    ///
    /// Eine leere Spur wird **nicht** aufgenommen: Sie entstünde, wenn
    /// kein Shard gerechnet hat, und ein Segment ohne Arbeit gehört nicht
    /// in ein PoI-Bündel.
    fn segment_abschliessen(
        &mut self,
        id: SegmentId,
        position: u64,
        trace: Vec<[u8; 32]>,
        signatures: Vec<BlsSignature>,
        eingangs_commitment: [u8; 32],
    ) {
        if trace.is_empty() {
            return;
        }
        let pod_path: Vec<MinerId> = (0..self.shards.len())
            .map(|i| MinerId::new([(i as u8) + 1; 32]))
            .collect();
        // ⚑ Die Kette ist `[Eingang] ++ Spur`, und damit ist die Eingabe
        // **jeder** Layer beweisbar, auch die der ersten (Fund 102):
        // `kette[j]` ist die Eingabe der Layer `j`, `kette[j+1]` ihre
        // Ausgabe. Ohne den ersten Eintrag hinge die erste Layer an
        // nichts.
        let mut kette = Vec::with_capacity(trace.len() + 1);
        kette.push(eingangs_commitment);
        kette.extend_from_slice(&trace);

        // Die Wurzel jetzt und nicht erst beim Bündeln: Sie ist die
        // Zusicherung über das Ergebnis, und eine Zusicherung, die erst
        // später entsteht, ist eine, die man sich noch überlegen kann.
        let spurwurzel = match myl_types::spurwurzel(&kette) {
            Ok(w) => w,
            Err(_) => return,
        };
        self.completed.push(CompletedSegment {
            id,
            position,
            trace,
            eingangs_commitment,
            spurwurzel,
            signatures,
            pod_path,
        });
    }

    /// Schickt eine Nachricht durch die Shard-Pipeline und sammelt Spur,
    /// Signaturen, ein evtl. gesampeltes Token und die Feedback-Nachricht.
    fn pump(
        &self,
        first: &PodMessage,
    ) -> (Vec<[u8; 32]>, Vec<BlsSignature>, Option<u32>, Option<PodMessage>) {
        let mut trace = Vec::new();
        let mut signatures = Vec::new();
        let mut token_out = None;
        let mut feedback_out = None;
        let mut current = first.clone();
        loop {
            let shard_idx = if current.carries_tokens() {
                0
            } else {
                (current.sender_shard + 1) as usize
            };
            if shard_idx >= self.shards.len() {
                break;
            }
            let shard = &self.shards[shard_idx];
            match shard.process(&current) {
                Ok(ShardOut::Forward(next)) => {
                    trace = next.trace.clone();
                    signatures.push(next.signature);
                    current = next;
                }
                // **Spur und Signatur auch hier übernehmen.** Der
                // letzte Shard endet immer in einem dieser beiden Zweige;
                // bis 2026-08-23 fielen seine Einträge deshalb unter den
                // Tisch, und bei einem Pod aus einem Shard blieb die Spur
                // ganz leer.
                Ok(ShardOut::Token {
                    token,
                    feedback,
                    trace: t,
                    signature,
                    ..
                }) => {
                    trace = t;
                    signatures.push(signature);
                    token_out = Some(token);
                    feedback_out = feedback;
                    break;
                }
                Ok(ShardOut::Prefill {
                    trace: t,
                    signature,
                }) => {
                    // Prefill-Position: kein Token, kein Feedback, aber
                    // gerechnet wurde sehr wohl.
                    trace = t;
                    signatures.push(signature);
                    break;
                }
                Err(e) => {
                    eprintln!("[coordinator] Shard {} lehnte ab: {}", shard_idx, e);
                    break;
                }
            }
        }
        (trace, signatures, token_out, feedback_out)
    }

    /// Baut ein PoI-Bündel aus den abgeschlossenen Segmenten dieser
    /// Epoche (Anhang A.1, Kap. 4.4).
    ///
    /// # ⚑ Fund 52: Das Aggregat ist **keine** Bündelsignatur
    ///
    /// Aggregiert werden die **Übergangs-Signaturen** der Segmente, also
    /// Unterschriften über `DST_SHARD_TRANSITION ‖ Rolle ‖
    /// Borsh(TransitionSig)`. `myl_consensus::verify_bundle_signature`
    /// prüft dagegen gegen `bundle_message(bundle)`, also über
    /// `DST_POI_BUNDLE ‖ epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`.
    ///
    /// **Zwei verschiedene Botschaften.** Ein Bündel aus dieser Funktion
    /// verifiziert deshalb nie — nicht weil es falsch wäre, sondern weil
    /// die beiden Seiten über verschiedene Dinge reden.
    ///
    /// **Die Richtung war die gute:** Es wurde abgelehnt, nicht
    /// angenommen. Niemand bekam Vergütung, die ihm nicht zusteht — und
    /// niemand sonst auch, denn der Pfad war nicht ungeprüft, sondern
    /// unbenutzbar.
    ///
    /// # Geschlossen am 2026-08-24 durch [`Self::build_signed_poi_bundle`]
    ///
    /// Was fehlte, war ein Protokollschritt und keine Zeile Code: Die
    /// Mitglieder sehen das **fertige** Bündel und unterschreiben seine
    /// Botschaft; erst dann gibt es ein Aggregat, das gegen die
    /// Mitgliedermenge gilt. Der Koordinator kann das nicht allein, denn
    /// sonst wäre die Zustimmung der Mitglieder eine Fiktion — und genau
    /// gegen diese Fiktion ist die Signatur da (Kap. 5.5: 100 % Slash bei
    /// falscher Aggregation).
    ///
    /// **Diese Funktion bleibt trotzdem, und sie bleibt so.** Sie baut den
    /// Inhalt; die Signaturrunde setzt darauf auf. Wer nur den Inhalt
    /// braucht (Inspektion, Tests, ein Koordinator, der noch sammelt),
    /// bekommt ihn ohne die Runde. **Ihr Ergebnis ist aber nicht
    /// einreichbar**, und der Name sagt das nicht; deshalb steht es hier.
    ///
    /// **Warum es niemandem aufgefallen ist:** `myl-pod` hing bis zum
    /// 2026-08-24 nicht an `myl-consensus` und umgekehrt. Beide Seiten
    /// sind für sich getestet; die Naht dazwischen hat nie jemand
    /// zusammengesteckt. Festgehalten in
    /// `tests/koordinator_byzantinisch.rs`.
    pub fn build_poi_bundle(&self) -> Result<PoIBundle, String> {
        if self.completed.is_empty() {
            return Err("keine abgeschlossenen Segmente".to_string());
        }
        let zeugnisse: Vec<myl_types::Segmentzeugnis> = self
            .completed
            .iter()
            .map(|c| myl_types::Segmentzeugnis {
                id: c.id,
                spurwurzel: c.spurwurzel,
            })
            .collect();
        let root = segments_root(&zeugnisse).map_err(|e| e.to_string())?;
        let vtfe = self.beanspruchte_vtfe()?;
        // Aggregat über die Übergangs-Signaturen (alle dieselbe Arbeit).
        let all_sigs: Vec<BlsSignature> = self
            .completed
            .iter()
            .flat_map(|c| c.signatures.clone())
            .collect();
        let agg = if all_sigs.is_empty() {
            BlsSignature([0u8; 96])
        } else {
            let a = aggregate_signatures(&all_sigs).map_err(|e| e.to_string())?;
            BlsSignature(a.0)
        };
        Ok(PoIBundle {
            epoch: self.epoch,
            pod: self.pod_id,
            segments_root: root,
            vtfe_claimed: vtfe,
            aggregate_sig: agg,
            segmente: 1,
        })
    }

    /// Die Botschaft, die die Pod-Mitglieder unterschreiben müssen,
    /// damit aus [`Self::build_poi_bundle`] ein einreichbares Bündel wird
    /// (⚑ Fund 52).
    ///
    /// Sie wird hier **nicht** nachgebaut, sondern aus dem Bündel
    /// abgeleitet, wie es die Gegenseite tut. Eine zweite Fassung der
    /// Kodierung wäre genau die Art Dublette, an der sich das Projekt
    /// schon mehrfach verbrannt hat: Sie liefe irgendwann auseinander,
    /// und der Streit wäre nicht entscheidbar.
    ///
    /// Die Kodierung steht in `myl_consensus::poi::poi_bundle_message`
    /// und lautet `DST_POI_BUNDLE ‖ u64_le(epoch) ‖ pod ‖ segments_root ‖
    /// u64_le(vtfe_claimed) ‖ u32_le(segmente)`.
    ///
    /// ⚑ **Sie steht hier zwangsläufig ein zweites Mal.** `myl-consensus`
    /// ist nur **dev-dependency** dieses Crates, denn `myl-pod` darf zur
    /// Bauzeit nicht am Konsens hängen. Rufen geht also nicht, nur
    /// abschreiben.
    ///
    /// **Was die Kopie zusammenhält, ist der Test
    /// `die_signierbotschaft_des_pods_ist_die_des_konsenses`**, und er
    /// hat sich am 2026-09-01 bezahlt gemacht: Als `PoIBundle` das Feld
    /// `segmente` bekam, band die Konsensfassung es und diese nicht.
    /// **Der Test fiel um, bevor irgendetwas auseinanderlaufen konnte.**
    /// Eine erzwungene Kopie ist tragbar, solange eine Gegenprobe an ihr
    /// hängt; ohne sie wäre sie Fund 111.
    pub fn signierbotschaft(bundle: &PoIBundle) -> Vec<u8> {
        let mut msg = Vec::with_capacity(21 + 8 + 32 + 32 + 8 + 4);
        msg.extend_from_slice(b"MYELITH_POI_BUNDLE_v1");
        msg.extend_from_slice(&bundle.epoch.0.to_le_bytes());
        msg.extend_from_slice(bundle.pod.as_bytes());
        msg.extend_from_slice(bundle.segments_root.as_bytes());
        msg.extend_from_slice(&bundle.vtfe_claimed.to_le_bytes());
        msg.extend_from_slice(&bundle.segmente.to_le_bytes());
        msg
    }

    /// Baut ein PoI-Bündel **und lässt es von den Mitgliedern
    /// unterschreiben** (⚑ Fund 52).
    ///
    /// Das ist die Runde, die [`Self::build_poi_bundle`] fehlte: Erst
    /// steht das Bündel, dann sehen es die Mitglieder, dann
    /// unterschreiben sie **seine** Botschaft, und erst dann aggregiert
    /// der Koordinator. Das Ergebnis verifiziert gegen
    /// `myl_consensus::verify_bundle_signature`.
    ///
    /// **Die Reihenfolge ist die ganze Sicherheit.** Würde der
    /// Koordinator zuerst sammeln und danach das Bündel bauen, könnte er
    /// `vtfe_claimed` nachträglich erhöhen; die Unterschriften lägen dann
    /// über einer Botschaft, die niemand gesehen hat. Genau davor schützt
    /// es, dass die Botschaft das fertige Bündel abbildet.
    ///
    /// **Jedes Mitglied prüft vor der Unterschrift**, ob der Anspruch zu
    /// der Segmentzahl passt, die es selbst gesehen hat
    /// ([`ShardNode::signiere_buendel`]). Verweigert eines, gibt es kein
    /// Bündel: Ein Aggregat gilt gegen **alle** Mitglieder, nicht gegen
    /// eine Mehrheit, und ein unvollständiges wäre wertlos.
    pub fn build_signed_poi_bundle(&self) -> Result<PoIBundle, String> {
        let bundle = self.build_poi_bundle()?;
        let botschaft = Self::signierbotschaft(&bundle);
        let segmente = self.completed.len() as u64;
        let zuschnitte: Vec<myl_tokenomics::ShardZuschnitt> =
            self.shards.iter().map(|s| s.zuschnitt()).collect();

        let mut sigs = Vec::with_capacity(self.shards.len());
        for shard in &self.shards {
            sigs.push(shard.signiere_buendel(
                &botschaft,
                bundle.vtfe_claimed,
                segmente,
                &zuschnitte,
            )?);
        }
        let agg = aggregate_signatures(&sigs).map_err(|e| e.to_string())?;
        Ok(PoIBundle {
            aggregate_sig: BlsSignature(agg.0),
            ..bundle
        })
    }

    /// Die abgeschlossenen Segmente (für Tests/Inspektion).
    pub fn completed_segments(&self) -> &[CompletedSegment] {
        &self.completed
    }

    /// Die beanspruchte Arbeitsmenge dieser Epoche in vTFE-Einheiten.
    ///
    /// Summe über alle Shards der Pipeline, je Shard nach der Regel aus
    /// [`myl_tokenomics::vtfe_gutschrift`]: Anteil an den
    /// Multiplikations-Additionen der Gewichtsmatrizen eines vollen
    /// Vorwärtspasses, mal der Zahl der erzeugten Token.
    ///
    /// **Bis zum 2026-08-23 stand hier die Zahl der Segmente**, mit dem
    /// Kommentar „Platzhalter für die FLOPs-Metrik". Ein Bündel über
    /// tausend Token beanspruchte damit dieselbe eine Einheit wie eines
    /// über zwei, und ein Shard mit sieben Layern dasselbe wie einer mit
    /// zweien. Genau davor warnt die Regel: *„eine Festlegung, bevor
    /// die erste Implementierung sie stillschweigend trifft."* Die
    /// Implementierung hatte sie längst getroffen.
    ///
    /// **Die Redundanz-Normierung steckt nicht hier drin.** Sie halbiert
    /// die Gutschrift, weil jedes Segment von r = 2 Pods gerechnet wird
    /// (`myl_tokenomics::redundancy_normalized_weight`), und gehört an
    /// die Stelle, die über die Pods hinwegsieht, nicht in den einzelnen
    /// Pod.
    pub fn beanspruchte_vtfe(&self) -> Result<u64, String> {
        let Some(erster) = self.shards.first() else {
            return Err("Pod ohne Shards beansprucht keine Arbeit".to_string());
        };
        let profil = erster.modell_profil();
        // Ein Segment ist ein Vorwärtspass, also ein
        // Token-Forward-Äquivalent. Prefill-Positionen zählen mit: Sie
        // emittieren kein Token, rechnen aber denselben Pass.
        let tokens: u64 = self.completed.len() as u64;

        let mut summe = 0u64;
        for shard in &self.shards {
            let anteil = myl_tokenomics::vtfe_gutschrift(&profil, &shard.zuschnitt(), tokens)
                .map_err(|e| format!("Shard {}: {}", shard.shard_index, e))?;
            summe = summe.saturating_add(anteil);
        }
        Ok(summe)
    }

    /// Dekodier-Digest einer Session: Hexwert und Zahl der Schritte.
    ///
    /// Der Wert kommt vom Shard mit dem LM-Head, weil nur dort Logits
    /// entstehen. Er folgt dem Vertrag aus
    /// [`integer_llm_runtime::generate::DekodierDigest`] und ist damit
    /// **unmittelbar** gegen den Einzelknotenlauf zu halten.
    ///
    /// **Wofür er da ist (Fund 36, letzter Teil):** Der Vergleich
    /// „Pod gegen Einzelknoten" hielt bis dahin Token gegen Token und
    /// prüfte damit, ob die Aufteilung dieselbe *Entscheidung* erzeugt.
    /// Das ist zu grob: Ein Token ist ein Argmax über `vocab_size`
    /// Zahlen und ändert sich erst, wenn deren Rangfolge kippt. Genau
    /// die kleinen Abweichungen, gegen die dieses Projekt gebaut ist,
    /// wären durchgerutscht.
    pub fn dekodier_digest(&self, session_id: u64) -> Option<(String, usize)> {
        self.shards.last()?.dekodier_digest(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_default() {
        assert_eq!(DEFAULT_WINDOW_MS, 250);
    }
}
