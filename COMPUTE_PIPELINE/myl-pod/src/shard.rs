//! Shard-Knoten: der Mining-Loop eines einzelnen Shards (Anhang A.3,
//! `shard_loop`).
//!
//! Verarbeitungsschritte je Nachricht (eine Token-Position):
//! 1. Aktivierungen vom Vorgänger empfangen (oder Token-Embedding für
//!    Shard 0).
//! 2. **Eingangs-Hash gegen die Spur prüfen** (Manipulationserkennung) —
//!    manipulierte Aktivierungen werden verworfen.
//! 3. Forward-Pass über die INTEGER_LLM-Stage-API (deterministisch,
//!    θ_v-konform).
//! 4. Spur fortschreiben (Ausgabe-Hash anhängen), Übergang BLS-signieren,
//!    Aktivierungen weiterreichen.
//! 5. KV-Cache der Session lokal fortschreiben (Session-Affinität).
//! 6. Aktivierungen erasure-codiert für die Streitfrist archivieren.
//!
//! Eine Nachricht trägt genau eine Token-Position; der Prompt wird als
//! Folge von Nachrichten verarbeitet.
// Die Kernel-Signaturen tragen den vollstaendigen Fixed-Point-Vertrag:
// Eingangs- und Ausgangs-frac_bits, Per-Channel-Shifts, LUT-Parameter.
// In eine Parameter-Struct gefasst waere die Entsprechung zu den
// Referenzformeln (Whitepaper Anhang B) beim Nachrechnen nicht mehr
// ablesbar — und genau dieses Nachrechnen ist die Pruefmethode des
// Projekts. Bewusste Abweichung von clippy::too_many_arguments.
#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use integer_llm_runtime::generate::DekodierDigest;
use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::model::IntegerModel;
use myl_tokenomics::{ModellProfil, ShardZuschnitt};
use myl_types::bls::{BlsPublicKey, BlsSecretKey};

use crate::da::DaStore;
use crate::trace::{activation_hash, verify_input_hash, TransitionSig, ZERO_HASH};
use crate::wire::{unpack_tokens, PodMessage, FLAG_FEEDBACK, FLAG_SAMPLE, FLAG_TOKEN_INPUT};

/// Ausgabe eines Shard-Schritts.
///
/// **Alle drei Fälle tragen Spur und Signatur (seit 2026-08-23).** Bis
/// dahin trugen sie nur `Forward`, und der Koordinator übernahm die Spur
/// ausschließlich dort. Der **letzte** Shard endet aber immer mit `Token`
/// oder `Prefill`: Seine Spur-Einträge und seine Signatur landeten
/// deshalb **nie** im abgeschlossenen Segment.
///
/// Die Wirkung war zweifach. Der Redundanzvergleich verglich die Arbeit
/// des Shards nicht, der die Ausgabe erzeugt, also ausgerechnet die des
/// LM-Kopfes. Und bei einem Pod aus einem einzigen Shard gab es gar kein
/// `Forward`, die committete Spur war **leer**, und ein PoI-Bündel
/// darüber hätte nichts belegt.
///
/// Aufgefallen beim Umbau auf Layer-Granularität, weil die
/// vTFE-Zuschreibung bei `k = 1` plötzlich null ergab.
#[derive(Debug)]
pub enum ShardOut {
    /// Aktivierungen an den nächsten Shard weiterreichen.
    Forward(PodMessage),
    /// Letzter Shard hat ein Token gesampelt (autoregressive Ausgabe).
    Token {
        token: u32,
        position: u64,
        /// Feedback-Nachricht an Shard 0 (falls die Generation weitergeht).
        feedback: Option<PodMessage>,
        trace: Vec<[u8; 32]>,
        signature: myl_types::bls::BlsSignature,
    },
    /// Prefill-Position: KV-Cache wurde fortgeschrieben, aber es wird
    /// kein Token emittiert (nur die letzte Prompt-Position und
    /// Feedback-Nachrichten sampeln).
    Prefill {
        trace: Vec<[u8; 32]>,
        signature: myl_types::bls::BlsSignature,
    },
}

/// Ein Shard-Miner im Pod.
pub struct ShardNode {
    pub shard_index: usize,
    pub layer_start: usize,
    pub layer_end: usize,
    pub has_embedding: bool,
    pub has_lm_head: bool,
    model: Arc<IntegerModel>,
    bls_sk: BlsSecretKey,
    bls_pk: BlsPublicKey,
    /// KV-Cache je Session (Session-Affinität, Kap. 4.2).
    caches: Mutex<HashMap<u64, KVCache>>,
    /// DA-Archiv für die Streitfrist.
    da: Mutex<DaStore>,
    /// Gemeinsame Boundary-Skala auf dem Draht.
    /// Budget an zu generierenden Tokens je Request.
    max_new_tokens: u64,
    /// Generierungs-Zähler je Session.
    gen_count: Mutex<HashMap<u64, u64>>,
    /// Dekodier-Digest je Session, nur beim Shard mit dem LM-Head.
    ///
    /// **Warum hier und nicht im Koordinator (Fund 36, letzter Teil):**
    /// Die Logits entstehen im letzten Shard und verlassen ihn nicht; auf
    /// dem Draht steht nur der gewählte Token. Genau deshalb konnte der
    /// Shard-Vergleich bisher nur Token gegen Token halten, also prüfen,
    /// ob die Aufteilung dieselbe *Entscheidung* erzeugt, statt dieselben
    /// *Zahlen*. Die Logits zum Koordinator zu schicken wäre die andere
    /// Lösung und hieße rund 600 KB je Token für einen Messwert; der
    /// Digest sind 32 Bytes.
    ///
    /// Der Wert folgt dem Vertrag aus
    /// [`integer_llm_runtime::generate::DekodierDigest`] und ist damit
    /// unmittelbar gegen den Einzelknotenlauf zu halten.
    dekodier_digest: Mutex<HashMap<u64, DekodierDigest>>,
}

impl ShardNode {
    pub fn new(
        shard_index: usize,
        layer_start: usize,
        layer_end: usize,
        has_embedding: bool,
        has_lm_head: bool,
        model: Arc<IntegerModel>,
        bls_sk: BlsSecretKey,
        da: DaStore,
        max_new_tokens: u64,
    ) -> Self {
        let bls_pk = bls_sk.public_key().expect("BLS Public Key");
        Self {
            shard_index,
            layer_start,
            layer_end,
            has_embedding,
            has_lm_head,
            model,
            bls_sk,
            bls_pk,
            caches: Mutex::new(HashMap::new()),
            da: Mutex::new(da),
            max_new_tokens,
            gen_count: Mutex::new(HashMap::new()),
            dekodier_digest: Mutex::new(HashMap::new()),
        }
    }

    /// Die archivierte Ausgabe-Aktivierung einer Layer, für Prüfer und
    /// für den Angeklagten in der Streitfrist.
    ///
    /// Der Weg nach draußen ist Absicht: Ohne ihn ließe sich die
    /// DA-Pflicht aus Anhang A.3 Schritt 6 nicht prüfen, und genau das
    /// war der Grund, warum das Überschreiben je Position so lange
    /// unbemerkt blieb.
    pub fn archiviert(
        &self,
        segment_id: &myl_types::ids::SegmentId,
        layer: usize,
    ) -> Result<Vec<i16>, String> {
        let da = self.da.lock().unwrap();
        da.get(*segment_id.as_bytes(), layer as u64)
    }

    /// Dekodier-Digest einer Session: Hexwert und Zahl der Schritte.
    ///
    /// Nur der Shard mit dem LM-Head liefert einen Wert; alle anderen
    /// sehen nie Logits. `None` heißt deshalb entweder „falscher Shard"
    /// oder „diese Session hat hier nichts gesampelt", und beides ist
    /// **kein** Befund über Bitgleichheit.
    ///
    /// Die Zahl der Schritte gehört zum Wert: Zwei Digests über
    /// verschiedene viele Schritte sind schlicht verschieden und sähen
    /// wie ein Determinismusfehler aus, was die Verwechslung aus Fund 35
    /// eine Ebene tiefer wäre.
    pub fn dekodier_digest(&self, session_id: u64) -> Option<(String, usize)> {
        let d = self.dekodier_digest.lock().unwrap();
        d.get(&session_id).map(|x| (x.hex(), x.schritte()))
    }

    pub fn public_key(&self) -> BlsPublicKey {
        self.bls_pk
    }

    /// Die Maße des Modells, soweit sie die Rechenarbeit bestimmen.
    ///
    /// `intermediate_size` steht nicht als Feld am Modell und wird
    /// deshalb aus der Zeilenzahl von `gate_proj` gelesen; ohne Layer
    /// gibt es keine, dann steht dort null.
    pub fn modell_profil(&self) -> ModellProfil {
        ModellProfil {
            hidden_size: self.model.hidden_size as u64,
            intermediate_size: self
                .model
                .layers
                .first()
                .map(|l| l.gate_proj.rows() as u64)
                .unwrap_or(0),
            num_layers: self.model.num_layers as u64,
            vocab_size: self.model.vocab_size as u64,
            num_heads: self.model.num_heads as u64,
            num_kv_heads: self.model.num_kv_heads as u64,
            head_dim: self.model.head_dim as u64,
        }
    }

    /// Was dieser Shard vom Modell hält, in der Form, die die
    /// vTFE-Zuschreibung erwartet.
    pub fn zuschnitt(&self) -> ShardZuschnitt {
        ShardZuschnitt {
            layer_start: self.layer_start as u64,
            layer_end: self.layer_end as u64,
            hat_embedding: self.has_embedding,
            hat_lm_kopf: self.has_lm_head,
        }
    }

    /// KV-Cache einer Session entnehmen oder anlegen.
    fn take_cache(&self, session_id: u64) -> KVCache {
        let mut caches = self.caches.lock().unwrap();
        caches.remove(&session_id).unwrap_or_else(|| {
            KVCache::for_range(self.layer_start, self.layer_end, self.model.num_kv_heads)
        })
    }

    fn put_cache(&self, session_id: u64, cache: KVCache) {
        let mut caches = self.caches.lock().unwrap();
        caches.insert(session_id, cache);
    }

    /// Verarbeitet eine Nachricht; liefert die Ausgabe des Shards.
    pub fn process(&self, msg: &PodMessage) -> Result<ShardOut, String> {
        if !msg.is_valid_frame() {
            return Err("ungültiger Rahmen (Magic)".to_string());
        }
        if msg.flags & crate::wire::FLAG_ABORT != 0 {
            return Err("Request abgebrochen".to_string());
        }

        let pos = msg.position as usize;
        let session_id = msg.session_id;

        if msg.carries_tokens() {
            // Shard 0: Token-Embedding.
            if !self.has_embedding {
                return Err("Token-Eingang an einem Shard ohne Embedding".to_string());
            }
            let tokens = unpack_tokens(&msg.payload)?;
            if tokens.len() != 1 {
                return Err("erwarte genau ein Token je Nachricht".to_string());
            }
            let token = tokens[0] as usize;

            let mut cache = self.take_cache(session_id);
            let hidden = self.model.embed_token(token);
            let mut trace = msg.trace.clone();
            let out =
                self.layer_fuer_layer(hidden, pos, &mut cache, &msg.segment_id, &mut trace);
            self.put_cache(session_id, cache);

            // **Eine Signatur je Shard, auch wenn die Spur je Layer
            // wächst.** Die Spur ist der Vergleichsgegenstand und braucht
            // Layer-Granularität; die Signatur ist die Zuschreibung und
            // braucht Shard-Granularität, denn geslasht wird ein Shard und
            // keine Layer. Je Layer zu signieren hieße bei 28 Layern und
            // vier Shards sieben BLS-Signaturen statt einer, ohne dass
            // jemand die zusätzliche Aussage nutzt.
            let out_hash = *trace.last().unwrap_or(&ZERO_HASH);
            let prev = ZERO_HASH;
            let sig = self.sign_transition(&msg.segment_id, pos as u64, &prev, &out_hash)?;

            return self.finish(msg, session_id, pos as u64, out, trace, sig);
        }

        // Zwischen-/End-Shard: Aktivierungen.
        // 2. Eingangs-Hash gegen die Spur prüfen (Manipulationserkennung).
        if !verify_input_hash(&msg.payload, &msg.trace) {
            return Err(format!(
                "Eingangs-Hash stimmt nicht mit der Spur überein (Shard {}, Position {})",
                self.shard_index, pos
            ));
        }
        // Signatur des Vorgängers prüfen.
        let prev_hash = msg.trace.last().copied().unwrap_or(ZERO_HASH);

        // Fund 26/20 behoben (2026-08-19): Die Aktivierungen kommen in
        // ihrer natürlichen Per-Kanal-Skala an — kein Boundary-Umweg
        // mehr. Damit ist der übertragene Nutzdatensatz genau das, was
        // der Sender gehasht hat, und die Spur bindet wieder die
        // ausgelieferte Arbeit.
        let hidden = msg.payload.clone();

        let mut cache = self.take_cache(session_id);
        let mut trace = msg.trace.clone();
        let out = self.layer_fuer_layer(hidden, pos, &mut cache, &msg.segment_id, &mut trace);
        self.put_cache(session_id, cache);

        let out_hash = *trace.last().unwrap_or(&ZERO_HASH);
        let sig = self.sign_transition(&msg.segment_id, pos as u64, &prev_hash, &out_hash)?;

        self.finish(msg, session_id, pos as u64, out, trace, sig)
    }

    /// Signiert den Übergang `(segment_id, prev_hash, next_hash)`.
    fn sign_transition(
        &self,
        segment_id: &myl_types::ids::SegmentId,
        position: u64,
        prev_hash: &[u8; 32],
        next_hash: &[u8; 32],
    ) -> Result<myl_types::bls::BlsSignature, String> {
        let t = TransitionSig {
            segment_id: *segment_id,
            shard_index: self.shard_index as u64,
            position,
            prev_hash: *prev_hash,
            next_hash: *next_hash,
        };
        t.sign(&self.bls_sk)
    }

    /// Archiviert die Ausgabe-Aktivierungen **einer Layer** (DA-Pflicht,
    /// Anhang A.3 Schritt 6).
    fn archive(&self, segment_id: &myl_types::ids::SegmentId, layer: usize, activations: &[i16]) {
        let mut da = self.da.lock().unwrap();
        da.put(*segment_id.as_bytes(), layer as u64, activations);
    }

    /// Rechnet den Layer-Bereich dieses Shards **Layer für Layer** und
    /// hängt je Layer einen Spur-Eintrag an; archiviert wird ebenso.
    ///
    /// **Warum je Layer und nicht je Shard (2026-08-23).** Die Spur war
    /// Shard-granular, und damit hing ihre Länge am Zuschnitt: Zwei Pods
    /// mit verschiedenem `k` hatten verschieden lange Spuren, und
    /// `myl_verifier::compare_commitments` lehnt das zu Recht mit
    /// `LengthMismatch` ab. Redundante Pipelines mussten deshalb
    /// denselben Zuschnitt haben, und der Entwurf für **variable
    /// Knotenzahl** war blockiert.
    ///
    /// Je Layer ist die Spur eine Eigenschaft des **Modells** statt des
    /// Zuschnitts: `num_layers` Einträge, gleichgültig ob ein Shard oder
    /// vierundzwanzig rechnen. Kosten sind zusätzliche SHA-256 je Token,
    /// gegenüber der Matrixarithmetik derselben Layer vernachlässigbar.
    ///
    /// **Nebengewinn:** Die Bisektion grenzt danach die fehlerhafte
    /// **Layer** ein statt der Layer-Gruppe, bei unverändertem O(log L).
    ///
    /// Dass ein Aufruf je Layer dasselbe liefert wie ein Bereichsaufruf,
    /// ist nicht hergeleitet, sondern gemessen:
    /// `tests/layer_granular.rs`, drei Positionen, echtes Modell.
    fn layer_fuer_layer(
        &self,
        mut hidden: Vec<i16>,
        pos: usize,
        cache: &mut KVCache,
        segment_id: &myl_types::ids::SegmentId,
        trace: &mut Vec<[u8; 32]>,
    ) -> Vec<i16> {
        for i in self.layer_start..self.layer_end {
            hidden = self.model.run_layers(hidden, pos, cache, i, i + 1);
            trace.push(activation_hash(&hidden));
            self.archive(segment_id, i, &hidden);
        }
        hidden
    }

    /// Gemeinsamer Abschluss: weiterreichen (Zwischen-Shard) oder sampeln
    /// (End-Shard).
    fn finish(
        &self,
        msg: &PodMessage,
        session_id: u64,
        position: u64,
        out: Vec<i16>,
        trace: Vec<[u8; 32]>,
        sig: myl_types::bls::BlsSignature,
    ) -> Result<ShardOut, String> {
        if self.has_lm_head {
            // Letzter Shard: Nur Positionen mit FLAG_SAMPLE sampeln ein
            // Token (letzte Prompt-Position und Feedback-Positionen).
            // Prefill-Positionen ohne FLAG_SAMPLE schreiben nur den
            // KV-Cache fort.
            if msg.flags & FLAG_SAMPLE == 0 {
                return Ok(ShardOut::Prefill { trace, signature: sig });
            }

            // Norm + LM-Head + Sampling.
            let logits = self.model.head_logits(&out);
            let token = self.model.greedy_next(&logits) as u32;

            // Fund 36, letzter Teil: **die Zahlen** festhalten, aus denen
            // entschieden wurde, nicht nur die Entscheidung. Ein Token ist
            // ein Argmax über `vocab_size` Zahlen und ändert sich erst,
            // wenn deren Rangfolge kippt; gemessen an 0,5B blieb er bei
            // 0,1 % veränderter Modellbytes unverändert.
            self.dekodier_digest
                .lock()
                .unwrap()
                .entry(session_id)
                .or_insert_with(DekodierDigest::neu)
                .schritt(&logits, token);

            // Generierungs-Buchhaltung + Feedback.
            let mut gen = self.gen_count.lock().unwrap();
            let entry = gen.entry(session_id).or_insert(0);
            *entry += 1;
            let feedback = if *entry < self.max_new_tokens {
                let packed = crate::wire::pack_tokens(&[token]);
                Some(PodMessage {
                    magic: crate::wire::MAGIC,
                    segment_id: msg.segment_id,
                    session_id,
                    sender_shard: self.shard_index as u64,
                    position: position + 1,
                    flags: FLAG_FEEDBACK | FLAG_TOKEN_INPUT | FLAG_SAMPLE,
                    trace: Vec::new(),
                    signature: myl_types::bls::BlsSignature([0u8; 96]),
                    payload: packed,
                })
            } else {
                gen.remove(&session_id);
                None
            };
            Ok(ShardOut::Token {
                token,
                position,
                feedback,
                trace,
                signature: sig,
            })
        } else {
            // Zwischen-Shard: Ausgang unverändert weiterreichen.
            // Token- und Feedback-Flags werden entfernt (ab hier fließen
            // Aktivierungen); FLAG_SAMPLE bleibt für den End-Shard
            // erhalten.
            let next = PodMessage {
                magic: crate::wire::MAGIC,
                segment_id: msg.segment_id,
                session_id,
                sender_shard: self.shard_index as u64,
                position,
                flags: msg.flags & !(FLAG_TOKEN_INPUT | FLAG_FEEDBACK),
                trace,
                signature: sig,
                payload: out,
            };
            Ok(ShardOut::Forward(next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::activation_hash;

    /// Dummy-Aktivierungen und eine Spur, die dazu passt.
    #[test]
    fn eingangs_pruefung_in_isolation() {
        let akt = [1i16, 2, 3, 4];
        let h = activation_hash(&akt);
        assert!(verify_input_hash(&akt, &[h]));
        let mut bad = akt;
        bad[0] = 99;
        assert!(!verify_input_hash(&bad, &[h]));
    }

    /// Regression zu Fund A17: Der Pod teilt seine Shards als
    /// `Arc<ShardNode>` und fuehrt sie nebenlaeufig aus (Micro-Batching,
    /// Pipelining). Fehlt irgendwo im Typbaum eine `Send`/`Sync`-Schranke,
    /// ist der Arc wertlos — vorher blockierte ein
    /// `Box<dyn ErasureCoder>` ohne Schranken die gesamte
    /// Nebenlaeufigkeit, ohne dass es auffiel.
    #[test]
    fn shardnode_ist_ueber_threads_teilbar() {
        fn ist_send<T: Send>() {}
        fn ist_sync<T: Sync>() {}
        ist_send::<ShardNode>();
        ist_sync::<ShardNode>();
    }
}
