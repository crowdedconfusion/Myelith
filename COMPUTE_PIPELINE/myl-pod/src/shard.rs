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
use integer_llm_runtime::model::{Belegung, IntegerModel};
use myl_tokenomics::{ModellProfil, ShardZuschnitt};
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};

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
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
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
    /// ⚑ **Dieselbe Anfrage zweimal heißt einmal rechnen** (Fund 723,
    /// [`Self::einmal_rechnen`]).
    einmal: Mutex<Einmal>,
    einmal_frei: std::sync::Condvar,
}

/// Was [`ShardNode::einmal_rechnen`] je Sitzung führt.
#[derive(Default)]
struct Einmal {
    /// Sitzungen, für die gerade gerechnet wird.
    in_arbeit: std::collections::HashSet<u64>,
    /// Je Sitzung die zuletzt gerechnete Anfrage.
    zuletzt: HashMap<u64, Arc<Zuletzt>>,
}

/// Eine gerechnete Anfrage: ihr Abdruck, die Länge jedes beteiligten
/// Speichers danach und die Antwort.
struct Zuletzt {
    abdruck: [u8; 32],
    laengen: Vec<(u64, usize)>,
    aus: Vec<ShardOut>,
}

/// So lange wartet eine Anfrage höchstens auf eine laufende Rechnung
/// derselben Sitzung (wie die Leitung des Knotens zum eigenen Shard).
pub const EINMAL_WARTEN: std::time::Duration = std::time::Duration::from_secs(120);

/// Gibt die Sitzungen frei, auch wenn die Rechnung in Panik ging.
struct Belegt<'a> {
    shard: &'a ShardNode,
    sitzungen: &'a [u64],
}

impl Drop for Belegt<'_> {
    fn drop(&mut self) {
        let mut e = self.shard.einmal.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for s in self.sitzungen {
            e.in_arbeit.remove(s);
        }
        drop(e);
        self.shard.einmal_frei.notify_all();
    }
}

/// Nimmt eine Sperre und macht aus einer **vergifteten** einen Fehler.
///
/// # ⚑ Warum `.lock().unwrap()` hier die falsche Antwort war
///
/// Ein `Mutex` in Rust merkt sich, ob ein Thread beim Halten der Sperre
/// in Panik ging. Danach liefert **jedes weitere** `lock()` einen
/// Fehler, und `.unwrap()` darauf ist eine Panik. Ein `ShardNode` liegt
/// hinter einem `Arc` und bedient viele Sitzungen: Der Ausfall **einer**
/// Sitzung hätte damit den Shard für **alle** getötet, dauerhaft, und
/// kein Test hätte es gezeigt.
///
/// **Heute ist das nicht auslösbar**, und das gehört dazugesagt: Unter
/// den drei Sperren läuft nichts, was in Panik geht. `DekodierDigest`
/// hasht, `KVCache::for_range` legt Karten an, der Zähler zählt. Das ist
/// aber eine Aussage über den **jetzigen** Rumpf und nicht über den
/// Vertrag, und sie fällt mit der nächsten Zeile, die jemand
/// dazwischenschreibt.
///
/// **Deshalb der Fehler statt der Panik.** Er kostet nichts, solange
/// nichts vergiftet, und wo er greift, ist er die richtige Antwort:
/// [`ShardNode::process`] gibt ohnehin ein `Result` zurück, der
/// Koordinator behandelt es, und ein Shard, der eine Nachricht
/// zurückweist, ist etwas anderes als einer, der stirbt.
///
/// **Was er ausdrücklich nicht tut, ist die Sperre entgiften.**
/// `PoisonError::into_inner` gäbe den Inhalt heraus und damit einen
/// Zustand, den ein abgestürzter Thread halb geschrieben hat. In einem
/// Pfad, dessen Ausgabe bitgleich sein muss, ist ein halber KV-Cache
/// schlimmer als eine abgelehnte Nachricht.
fn sperre<'a, T>(m: &'a Mutex<T>, was: &str) -> Result<std::sync::MutexGuard<'a, T>, String> {
    m.lock()
        .map_err(|_| format!("Sperre {was} ist vergiftet: ein Thread ging beim Halten in Panik"))
}

/// ⚑ **Der Fälscher des Prüfstands** (Merkmal `faelscher`, 2026-10-28): Mit
/// `MYL_SHARD_FAELSCHEN=1` rechnet der Shard auf einer Eingabe, deren erster
/// Wert verschoben ist. Spur und Unterschrift folgen der falschen
/// Rechnung, der Nachfolger nimmt sie also an; nur ein zweiter Pod, der
/// dasselbe Segment ehrlich rechnet, sieht eine andere Ausgabe. Ohne das
/// Merkmal tut diese Funktion nichts.
///
/// ⚑ **Die Kollusion** (2026-11-06): Mit `MYL_SHARD_FAELSCHEN=erste`
/// faelscht der Shard nur, wenn er die Einbettung rechnet (`erster`). Zwei
/// solche Faelscher an der ersten Position zweier Zwillinge rechnen dann
/// gleich falsch, denn die Grenzen dahinter aendern an der Rechnung nichts;
/// die Zwillinge sind einig, und nur das Nachrechnen ab den Tokens findet es.
///
/// 📌 **Um 64, nicht um eins** (2026-11-06): Mit dem 0,6B verschwand eine
/// Verschiebung um eins bei 346 von 400 Tokens schon in der ersten Layer
/// (Requantisierung), an Position 0 wie spaeter; der Pod rechnete dann
/// ehrlich, und das Nachrechnen meldete zu Recht „gleich“. Um 64 verschwand
/// sie bei keinem.
#[cfg(feature = "faelscher")]
fn faelschen(hidden: &mut [i16], erster: bool) {
    static ART: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    let art = *ART.get_or_init(|| match std::env::var("MYL_SHARD_FAELSCHEN").as_deref() {
        Ok("1") => 1,
        Ok("erste") => 2,
        _ => 0,
    });
    if art == 1 || (art == 2 && erster) {
        if let Some(wert) = hidden.first_mut() {
            *wert = wert.wrapping_add(64);
        }
    }
}

#[cfg(not(feature = "faelscher"))]
fn faelschen(_: &mut [i16], _: bool) {}

/// Höchstens so viele Positionen rechnet eine Zeitprobe (gegen eine
/// Probe, die einen Shard minutenlang belegt).
///
/// ⚑ **2 048 seit dem 2026-10-27** (bis dahin 16): Die Probe eines
/// Mitglieds richtet sich nach seiner Zusage, und wer sehr schnell ansagt,
/// bekommt bis zu 2 048 Positionen (Fund 646, NODE `frist::probe_positionen`).
pub const ZEITPROBE_HOECHSTENS: usize = 2_048;

/// Das Token, dessen Einbettung die Zeitprobe rechnet.
pub const ZEITPROBE_TOKEN: usize = 0;

impl ShardNode {
    pub fn new(
        shard_index: usize,
        layer_start: usize,
        layer_end: usize,
        has_embedding: bool,
        has_lm_head: bool,
        model: Arc<IntegerModel>,
        bls_sk: BlsSecretKey,
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
            max_new_tokens,
            gen_count: Mutex::new(HashMap::new()),
            dekodier_digest: Mutex::new(HashMap::new()),
            einmal: Mutex::new(Einmal::default()),
            einmal_frei: std::sync::Condvar::new(),
        }
    }

    /// ⚑ **Eine Anfrage, die schon gerechnet ist, wird wiederholt, nicht
    /// neu gerechnet** (Fund 723, Auftragslauf vom 2026-11-24).
    ///
    /// 📌 Seit Fund 719 lehnt der Shard eine schon gerechnete Position ab,
    /// und der Koordinator gab die Runde nach jedem Transportfehler auf. Unter
    /// Last (35B, Prefill von 6 442 Token, Stapel bis 38 s) lief so jede
    /// Zeitüberschreitung in einen Neubeginn von vorn, während der alte Lauf
    /// weiterrechnete, und die doppelte Last brachte die nächste: Der erste
    /// Auftrag endete mit „kein Pod hat gerechnet“.
    ///
    /// Jetzt merkt sich der Shard je Sitzung die zuletzt gerechnete Anfrage
    /// (Abdruck über alle Nachrichten), die Länge jedes beteiligten Speichers
    /// danach und die Antwort. Kommt **dieselbe** Anfrage wieder und hat
    /// keine ihrer Sitzungen seitdem gerechnet oder vergessen (jede Länge
    /// gleich), geht dieselbe Antwort zurück, ohne den Speicher zu berühren:
    /// Der Zustand schiebt sich nicht weiter (Fund 719 bleibt zu), und die
    /// Unterschrift ist dieselbe. Rechnet der Shard die Anfrage gerade noch
    /// (die Frist des Fragenden lief vorher ab), wartet die Wiederholung
    /// darauf, höchstens [`EINMAL_WARTEN`]. Anfragen verschiedener Sitzungen
    /// warten nicht aufeinander.
    pub fn einmal_rechnen(
        &self,
        msgs: &[PodMessage],
        rechnen: impl FnOnce() -> Result<Vec<ShardOut>, String>,
    ) -> Result<Vec<ShardOut>, String> {
        let mut sitzungen: Vec<u64> = Vec::new();
        for m in msgs {
            if !sitzungen.contains(&m.session_id) {
                sitzungen.push(m.session_id);
            }
        }
        let Some(erste) = sitzungen.first().copied() else {
            return rechnen();
        };
        let bytes = borsh::to_vec(msgs).map_err(|e| format!("Anfrage nicht kodierbar: {e}"))?;
        let abdruck = *myl_types::Hash::sha256(&bytes).as_bytes();
        let ende = std::time::Instant::now() + EINMAL_WARTEN;
        let mut e = sperre(&self.einmal, "Wiederholung")?;
        while sitzungen.iter().any(|s| e.in_arbeit.contains(s)) {
            let rest = ende.saturating_duration_since(std::time::Instant::now());
            if rest.is_zero() {
                return Err(format!("Sitzung {erste} wird noch gerechnet (Shard {})", self.shard_index));
            }
            e = self.einmal_frei.wait_timeout(e, rest).unwrap_or_else(std::sync::PoisonError::into_inner).0;
        }
        if let Some(z) = e.zuletzt.get(&erste).cloned() {
            if z.abdruck == abdruck && self.laengen_unveraendert(&z.laengen)? {
                return Ok(z.aus.clone());
            }
        }
        for s in &sitzungen {
            e.in_arbeit.insert(*s);
        }
        drop(e);
        let belegt = Belegt { shard: self, sitzungen: &sitzungen };
        let ergebnis = rechnen();
        if let Ok(aus) = &ergebnis {
            let laengen = {
                let c = sperre(&self.caches, "KV-Cache")?;
                sitzungen.iter().map(|s| (*s, c.get(s).map_or(0, KVCache::laenge))).collect()
            };
            let z = Arc::new(Zuletzt { abdruck, laengen, aus: aus.clone() });
            let mut e = sperre(&self.einmal, "Wiederholung")?;
            for s in &sitzungen {
                e.zuletzt.insert(*s, Arc::clone(&z));
            }
        }
        drop(belegt);
        ergebnis
    }

    fn laengen_unveraendert(&self, laengen: &[(u64, usize)]) -> Result<bool, String> {
        let c = sperre(&self.caches, "KV-Cache")?;
        Ok(laengen.iter().all(|(s, n)| c.get(s).map(KVCache::laenge) == Some(*n)))
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
    /// ⚑ **Der Fehlerfall ist vom `None` getrennt, und das ist der Grund
    /// für das `Result`.** `None` heißt „hier ist nichts gesampelt
    /// worden" und ist harmlos; eine vergiftete Sperre heißt „ein Thread
    /// ist gestorben" und ist es nicht. Beides in dasselbe `None` zu
    /// legen hieße, dem Feld eine dritte Bedeutung zu geben, die niemand
    /// unterscheiden kann.
    pub fn dekodier_digest(&self, session_id: u64) -> Result<Option<(String, usize)>, String> {
        let d = sperre(&self.dekodier_digest, "Dekodier-Digest")?;
        Ok(d.get(&session_id).map(|x| (x.hex(), x.schritte())))
    }

    /// Unterschreibt die Botschaft eines **fertigen** PoI-Bündels als
    /// Pod-Mitglied (⚑ Fund 52).
    ///
    /// # Warum ein Mitglied prüft, bevor es unterschreibt
    ///
    /// Eine Unterschrift, die ohne Prüfung gegeben wird, ist **keine
    /// Zustimmung**, sondern eine Anwesenheitsnotiz. Kap. 5.5 belegt
    /// falsche PoI-Aggregation mit 100 % Slash des Koordinators, und das
    /// setzt voraus, dass die Mitglieder etwas anderes bezeugen als
    /// „ich war dabei".
    ///
    /// Geprüft wird deshalb der **Anspruch**: `vtfe_claimed` muss zu der
    /// Segmentzahl passen, die dieses Mitglied selbst gesehen hat, und
    /// zwar mit derselben Regel, die der Koordinator anwendet
    /// ([`myl_tokenomics::vtfe_gutschrift`]). Weicht sie ab, wird nicht
    /// unterschrieben.
    ///
    /// **Was ein Mitglied lokal nicht prüfen kann**, gehört genannt: die
    /// Merkle-Wurzel über die Segmentmenge. Es kennt seine eigenen
    /// Segmente, aber nicht zwingend die Reihenfolge, in der der
    /// Koordinator sie eingetragen hat, und eine Wurzel ist ohne die
    /// vollständige Liste nicht nachrechenbar. Der Koordinator liefert
    /// sie deshalb mit; die Prüfung ist dann eine über gelieferte Daten
    /// und nicht über eigene. **Das ist eine schwächere Aussage**, und
    /// sie steht hier, damit niemand die Unterschrift für mehr hält, als
    /// sie ist.
    ///
    /// **Domain-Separation:** Die Botschaft trägt `DST_POI_BUNDLE` und
    /// ist damit von der Übergangssignatur getrennt, die
    /// `DST_SHARD_TRANSITION` trägt. Ein eigenes Rollenbyte wie in
    /// [`crate::trace::Rolle`] braucht es hier nicht: Wo eine Klasse ihre
    /// eigene DST hat, ist die Rolle darin schon enthalten.
    pub fn signiere_buendel(
        &self,
        botschaft: &[u8],
        vtfe_claimed: u64,
        segmente: u64,
        pod_zuschnitte: &[myl_tokenomics::ShardZuschnitt],
    ) -> Result<BlsSignature, String> {
        let profil = self.modell_profil();
        let mut erwartet = 0u64;
        for z in pod_zuschnitte {
            let anteil = myl_tokenomics::vtfe_gutschrift(&profil, z, segmente)
                .map_err(|e| format!("Zuschnitt unbrauchbar: {}", e))?;
            erwartet = erwartet.saturating_add(anteil);
        }
        if vtfe_claimed != erwartet {
            return Err(format!(
                "Shard {} unterschreibt nicht: beansprucht sind {} vTFE, nachgerechnet {} \
                 für {} Segmente",
                self.shard_index, vtfe_claimed, erwartet, segmente
            ));
        }
        self.bls_sk
            .sign(botschaft)
            .map_err(|e| format!("Shard {}: {}", self.shard_index, e))
    }

    pub fn public_key(&self) -> BlsPublicKey {
        self.bls_pk
    }

    /// Die Maße des Modells, soweit sie die Rechenarbeit bestimmen.
    ///
    /// `intermediate_size` steht nicht als Feld am Modell und wird
    /// deshalb aus der Zeilenzahl von `gate_proj` gelesen; ohne Layer
    /// gibt es keine, dann steht dort null.
    ///
    /// ⚑ **Fund 60 (2026-08-25): Bei einer MoE-Layer gibt es kein
    /// `gate_proj`.** Sie hat einen Router und N Experten, die je eines
    /// haben. Wer hier den ersten Experten nähme, bekäme
    /// `moe_intermediate_size` statt `intermediate_size` und spräche dem
    /// Shard bei Qwen3-30B-A3B **ein Achtel** seiner Arbeit zu. Deshalb
    /// liest dieser Zweig die MoE-Größen als das, was sie sind, und
    /// `macs_je_layer` rechnet damit `top_k` Experten plus Router statt
    /// einer breiten MLP.
    pub fn modell_profil(&self) -> ModellProfil {
        use integer_llm_runtime::model::{Feedforward, Mischer};
        let erste = self.model.layers.first();
        // ⚑ **Hybride Modelle** (2026-10-22): Die erste Ebene mit voller
        //   Achtsamkeit gibt den Abstand an (beim 35B Ebene 3, also jede
        //   vierte); die erste Zustandsschicht die Maße; der erste
        //   geteilte Experte seine Breite. Ein Modell ohne Zustandsschicht
        //   hat `voll_alle` null und rechnet wie zuvor.
        let mit_zustand = self.model.layers.iter().find_map(|l| match &l.mischer {
            Mischer::Zustand(z) => Some(z),
            Mischer::Achtsamkeit(_) => None,
        });
        let voll_alle = match mit_zustand {
            None => 0,
            Some(_) => self
                .model
                .layers
                .iter()
                .position(|l| matches!(l.mischer, Mischer::Achtsamkeit(_)))
                .map_or(0, |i| i as u64 + 1),
        };
        let zustand = match (mit_zustand, self.model.zustandsmasse.as_ref()) {
            (Some(z), Some(m)) => myl_tokenomics::vtfe::Zustandsprofil {
                schluessel_koepfe: m.schluessel_koepfe as u64,
                schluessel_dim: m.schluessel_dim as u64,
                wert_koepfe: m.wert_koepfe as u64,
                wert_dim: m.wert_dim as u64,
                faltung: z.conv1d.cols() as u64,
            },
            _ => myl_tokenomics::vtfe::Zustandsprofil::LEER,
        };
        let geteilter_experte = self
            .model
            .layers
            .iter()
            .find_map(|l| match &l.ffn {
                Feedforward::Moe(m) => m.geteilter_experte.as_ref().map(|g| g.zwischen as u64),
                Feedforward::Dense(_) => None,
            })
            .unwrap_or(0);
        let ausgangstor = self.model.layers.iter().find_map(|l| match &l.mischer {
            Mischer::Achtsamkeit(a) => Some(a.q_proj.rows() == 2 * self.model.num_heads * self.model.head_dim),
            Mischer::Zustand(_) => None,
        });
        let (intermediate_size, num_experts, num_experts_per_tok, moe_intermediate_size) =
            match erste.map(|l| &l.ffn) {
                Some(Feedforward::Dense(mlp)) => (mlp.gate_proj.rows() as u64, 0, 0, 0),
                Some(Feedforward::Moe(moe)) => (
                    // Bei MoE ist `intermediate_size` für die Rechnung
                    // bedeutungslos; `macs_je_layer` benutzt es dann nicht.
                    0,
                    moe.experts.len() as u64,
                    moe.top_k as u64,
                    moe.experts
                        .first()
                        .map(|e| e.gate_proj.rows() as u64)
                        .unwrap_or(0),
                ),
                None => (0, 0, 0, 0),
            };
        ModellProfil {
            hidden_size: self.model.hidden_size as u64,
            intermediate_size,
            num_experts,
            num_experts_per_tok,
            moe_intermediate_size,
            num_layers: self.model.num_layers as u64,
            vocab_size: self.model.vocab_size as u64,
            num_heads: self.model.num_heads as u64,
            num_kv_heads: self.model.num_kv_heads as u64,
            head_dim: self.model.head_dim as u64,
            voll_alle,
            zustand,
            geteilter_experte,
            ausgangstor: ausgangstor.unwrap_or(false),
        }
    }

    /// ⚑ **Die Zeitprobe** (Stufe 4, Frist, 2026-10-22): rechnet
    /// `positionen` Positionen durch den eigenen Bereich, mit eigenem
    /// KV-Speicher, und den Kopf, wenn der Shard ihn trägt. Ein Zeuge stellt
    /// sie und nimmt die Zeit; der Abdruck des letzten Ergebnisses kommt
    /// zurück, damit die Antwort an eine Rechnung gebunden ist.
    ///
    /// Die Eingabe ist bei jedem Shard die Einbettung eines festen Tokens:
    /// für einen mittleren Bereich kein Strom, der so ankäme, aber eine
    /// Eingabe der richtigen Länge, und die Arbeit hängt nicht an den
    /// Werten. Keine Sitzung, kein Abdruck, keine Spur bleibt zurück.
    pub fn zeitprobe(&self, positionen: usize) -> [u8; 32] {
        self.zeitprobe_bis(positionen, None, None)
    }

    /// Wie [`Self::zeitprobe`], und nach `grenze` hört der Shard auf
    /// (2026-10-27, Fund 646): Wer so lange braucht, hat die Frist ohnehin
    /// überschritten, und der Shard ist wieder frei für den nächsten
    /// Messenden. Die Grenze wird vor jeder Position geprüft.
    ///
    /// ⚑ **Mit Saat** (2026-10-28, Fund 669) ist das Token je Position
    /// `sha256(saat ‖ position)` modulo Vokabular, und der Abdruck hängt an der
    /// Saat des Messenden; ohne Saat das feste Token wie bisher.
    pub fn zeitprobe_bis(
        &self,
        positionen: usize,
        grenze: Option<std::time::Duration>,
        saat: Option<&[u8; 32]>,
    ) -> [u8; 32] {
        let positionen = positionen.clamp(1, ZEITPROBE_HOECHSTENS);
        let ab = std::time::Instant::now();
        let mut cache = self.frischer_cache();
        let mut letzte = ZERO_HASH;
        for pos in 0..positionen {
            if grenze.is_some_and(|g| ab.elapsed() >= g) {
                break;
            }
            let token = match saat {
                Some(s) => {
                    let h = myl_types::hash::Hash::sha256(&[s.as_slice(), &(pos as u64).to_le_bytes()].concat());
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&h.0[..8]);
                    (u64::from_le_bytes(b) % self.model.vocab_size.max(1) as u64) as usize
                }
                None => ZEITPROBE_TOKEN,
            };
            let mut hidden = self.model.embed_token(token);
            for i in self.layer_start..self.layer_end {
                hidden = self.model.run_layers(hidden, pos, &mut cache, i, i + 1);
            }
            letzte = if self.has_lm_head {
                let logits = self.model.head_logits(&hidden);
                myl_types::hash::Hash::sha256(&logits.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).0
            } else {
                activation_hash(&hidden)
            };
        }
        letzte
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
    ///
    /// ⚑ **Das Anlegen steht außerhalb der Sperre.** `for_range` legt je
    /// Layer und Kopf eine Karte an; unter der Sperre wäre das ein
    /// Abschnitt, in dem alle anderen Sitzungen dieses Shards warten,
    /// und zwar ausgerechnet beim ersten Zugriff einer neuen Sitzung.
    /// Gebraucht wird die Sperre nur für das Entnehmen.
    /// **Eine Sitzung, die bei Position 0 beginnt, beginnt neu** (Fund 709,
    /// 2026-11-16).
    ///
    /// 📌 Bis hierher nahm ein Shard den Speicher einer bekannten Sitzung
    /// unverändert wieder auf. Wiederholt der Koordinator einen Auftrag und
    /// erreicht das Vergessen den Shard nicht (im Auftragslauf am
    /// Epochenwechsel: „max sub-streams reached“), überschreiben die
    /// Aufmerksamkeitsebenen ihre alten Positionen, aber eine Zustandsschicht
    /// rechnet auf dem alten Zustand weiter. Beim 35B gab so ein ehrlicher
    /// Pod 24 Token aus, sein Zwilling 47, und das Paar war uneinig. Am
    /// 0,6B ohne Zustandsschichten war es unsichtbar.
    fn neu_beginnen_bei_null(&self, session_id: u64, position: usize) {
        if position == 0 {
            self.sitzung_vergessen(session_id);
        }
    }

    fn take_cache(&self, session_id: u64) -> Result<KVCache, String> {
        let vorhanden = sperre(&self.caches, "KV-Cache")?.remove(&session_id);
        Ok(vorhanden.unwrap_or_else(|| {
            KVCache::for_range(self.layer_start, self.layer_end, self.model.num_kv_heads)
        }))
    }

    /// Vergisst alles, was zu dieser Sitzung gehört.
    ///
    /// # ⚑ Warum beim Abschluss und nicht später (Fund 164)
    ///
    /// Bis zum 2026-09-04 wuchsen `caches` und `dekodier_digest` mit
    /// **jeder** Anfrage und wurden nie kleiner. Gemessen an der
    /// Probepipeline: ein KV-Cache und ein Digest je Anfrage, für
    /// immer. Ein Knoten, der Anfragen bedient, hielt jede je gerechnete
    /// Anfrage im Speicher.
    ///
    /// ⚑ **Und niemand hatte etwas davon.** Das Gateway vergibt je
    /// Anfrage eine **neue** Sitzungsnummer, und `run_prompt` füllt
    /// immer ab Position 0 vor; kein Aufrufer hat je eine Sitzung
    /// fortgesetzt. Die Session-Affinität aus Kap. 4.2 stand im Feld und
    /// hatte keinen Nutzer.
    ///
    /// **Das ist die schlichte Regel eines Inferenzdienstes:** Was zu
    /// einer Anfrage gehört, wird frei, sobald sie fertig ist. Wenn eines Tages
    /// jemand eine Sitzung fortsetzt, gehört hierher eine Verdrängung
    /// nach Alter, kein Weglassen.
    pub fn sitzung_vergessen(&self, session_id: u64) {
        if let Ok(mut c) = self.caches.lock() {
            c.remove(&session_id);
        }
        // Fund 723: Was zur Wiederholung gemerkt war, gehört dazu.
        if let Ok(mut e) = self.einmal.lock() {
            e.zuletzt.remove(&session_id);
        }
        if let Ok(mut d) = self.dekodier_digest.lock() {
            d.remove(&session_id);
        }
        // 📌 **Fund 657 (2026-10-27): und der Generierungszähler.** Er blieb
        //   stehen, wenn eine Sitzung vor dem Deckel des Shards endete (das
        //   Podwerk fragt meist weniger Token an), also je Sitzung ein Eintrag
        //   für immer; und eine Wiederholung derselben Sitzung (Fund 654)
        //   bekam nur noch den Rest des Deckels. Gefunden an
        //   `tests/verlorene_runde.rs`.
        if let Ok(mut g) = self.gen_count.lock() {
            g.remove(&session_id);
        }
    }

    /// Wie viele Sitzungen dieser Shard gerade hält.
    ///
    /// ⚑ **Damit eine Zusicherung darauf zeigen kann.** Eine Zahl, die
    /// niemand lesen kann, wächst unbemerkt.
    pub fn gehaltene_sitzungen(&self) -> usize {
        self.caches.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// **Eine Position, die der KV-Speicher dieser Sitzung nicht fortsetzen
    /// kann, wird abgelehnt**, und der Speicher bleibt, wie er war.
    ///
    /// ⚑ **Hinter dem Ende ist eine Luecke**, und die hat der
    /// zusammenhaengende KV-Speicher nicht. Frueher nahm eine Karte sie
    /// stillschweigend an, und die Aufmerksamkeit sah weniger Positionen.
    /// Jetzt bricht der Speicher dort ab, und eine Panik ist im offenen Netz
    /// ein Denial-of-Service (wie bei der Laenge, Fund 41). Deshalb hier ein
    /// `Err`, bevor gerechnet wird.
    ///
    /// ⚑ **Hinter der Kontextgrenze** begann die RoPE-Tabelle frueher von
    /// vorn (Fund 368); jetzt bricht das Modell dort ab, und auch das wird
    /// hier zur abgelehnten Nachricht.
    /// ⛔️ **Fund 724 (2026-11-24): Die Spur sagt, für welche Ebene eine
    /// Nachricht ist.** Ein Token-Eingang hat keine Spur, eine Aktivierung für
    /// einen Shard ab Ebene `L` genau `L` Einträge, einen je Ebene davor. 📌 Im
    /// Auftragslauf vom 24. rechnete der Koordinator einer abgelaufenen Epoche
    /// über seine alten Mitglieder weiter; deren Knoten reichten an den Shard
    /// weiter, den sie **jetzt** hatten, mit anderen Ebenen, und der rechnete
    /// die Nachricht an der falschen Stelle des Modells. Die Antworten wuchsen
    /// über 24 Minuten auf 4 MiB, bis die Leitung sie nicht mehr trug. Bis
    /// hierher fiel das nur auf, wenn zufällig das Embedding fehlte.
    fn spur_passt(&self, m: &PodMessage) -> Result<(), String> {
        let soll = if m.carries_tokens() { 0 } else { self.layer_start };
        if m.trace.len() != soll {
            return Err(format!(
                "die Nachricht ist für Ebene {}, dieser Shard beginnt bei Ebene {} (Shard {}, Position {})",
                m.trace.len(),
                soll,
                self.shard_index,
                m.position
            ));
        }
        Ok(())
    }

    fn position_pruefen(&self, session_id: u64, pos: usize, cache: KVCache) -> Result<KVCache, String> {
        let laenge = cache.laenge();
        let grenze = self.model.kontextgrenze();
        if pos >= grenze {
            self.put_cache(session_id, cache)?;
            return Err(format!(
                "Position {pos} liegt hinter der Kontextgrenze {grenze} des Modells (Shard {})",
                self.shard_index
            ));
        }
        if pos > laenge {
            self.put_cache(session_id, cache)?;
            return Err(format!(
                "Position {pos} liegt hinter dem Ende des KV-Speichers ({laenge} Positionen, Shard {})",
                self.shard_index
            ));
        }
        // ⛔️ **Fund 719 (2026-11-23): Eine schon gerechnete Position wird
        //    abgelehnt.** Bei reiner Attention überschrieb eine Wiederholung
        //    denselben Eintrag mit denselben Werten; die lineare Attention des
        //    Netzmodells führt aber einen rekurrenten Zustand, und eine zweimal
        //    gerechnete Position schob ihn ein zweites Mal weiter, still und
        //    falsch. So rechneten Zwillinge verschieden, nachdem der
        //    Koordinator einen Stapel nach einem Transportfehler einzeln
        //    wiederholt hatte, den der Shard schon gerechnet hatte.
        if pos < laenge {
            self.put_cache(session_id, cache)?;
            return Err(format!(
                "Position {pos} ist schon gerechnet ({laenge} Positionen, Shard {}); eine Wiederholung verschöbe den Zustand",
                self.shard_index
            ));
        }
        Ok(cache)
    }

    fn put_cache(&self, session_id: u64, cache: KVCache) -> Result<(), String> {
        sperre(&self.caches, "KV-Cache")?.insert(session_id, cache);
        Ok(())
    }

    /// Verarbeitet eine Nachricht; liefert die Ausgabe des Shards.
    pub fn process(&self, msg: &PodMessage) -> Result<ShardOut, String> {
        if !msg.is_valid_frame() {
            return Err("ungültiger Rahmen (Magic)".to_string());
        }
        if msg.flags & crate::wire::FLAG_ABORT != 0 {
            return Err("Request abgebrochen".to_string());
        }
        self.spur_passt(msg)?;

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
            // 📌 **Fund 613 (2026-10-23): das Token gegen das Vokabular,
            //   vor dem Cache.** Der Zweig der Aktivierungen prüft seine
            //   Länge seit Fund 41, weil eine Panik im offenen Netz ein
            //   Denial-of-Service ist; dieser Zweig gab die Kennung
            //   ungeprüft an `embed_token`. Eine Kennung hinter dem
            //   Vokabular brachte den Shard-Prozess zum Absturz, und den
            //   Cache der Sitzung hatte er schon herausgenommen.
            if token >= self.model.vocab_size {
                return Err(format!(
                    "Token {token} liegt ausserhalb des Vokabulars ({} Einträge, Shard {}, Position {pos})",
                    self.model.vocab_size, self.shard_index
                ));
            }

            self.neu_beginnen_bei_null(session_id, pos);
            let mut cache = self.position_pruefen(session_id, pos, self.take_cache(session_id)?)?;
            let mut hidden = self.model.embed_token(token);
            faelschen(&mut hidden, true);
            let mut trace = msg.trace.clone();
            let out =
                self.layer_fuer_layer(hidden, pos, &mut cache, &msg.segment_id, &mut trace);
            self.put_cache(session_id, cache)?;

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

            return self.finish(msg, session_id, pos as u64, out, trace, sig, None);
        }

        // Zwischen-/End-Shard: Aktivierungen.
        //
        // **Länge zuerst (Fund 41).** Ein Kernel darf nie mit einer
        // Eingabe laufen, deren Länge nicht zum Modell passt:
        // `rmsnorm_i16` prüft das per `assert_eq!` und **panisch**, und
        // eine Panik ist im offenen Netz ein Denial-of-Service, den jeder
        // auslösen kann, der Bytes schicken darf. Hier wird daraus ein
        // `Err`, also eine abgelehnte Nachricht.
        if msg.payload.len() != self.model.hidden_size {
            return Err(format!(
                "Aktivierungen haben Länge {}, das Modell erwartet {} (Shard {}, \
                 Position {})",
                msg.payload.len(),
                self.model.hidden_size,
                self.shard_index,
                pos
            ));
        }
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
        let mut hidden = msg.payload.clone();
        faelschen(&mut hidden, false);

        self.neu_beginnen_bei_null(session_id, pos);
        let mut cache = self.position_pruefen(session_id, pos, self.take_cache(session_id)?)?;
        let mut trace = msg.trace.clone();
        let out = self.layer_fuer_layer(hidden, pos, &mut cache, &msg.segment_id, &mut trace);
        self.put_cache(session_id, cache)?;

        let out_hash = *trace.last().unwrap_or(&ZERO_HASH);
        let sig = self.sign_transition(&msg.segment_id, pos as u64, &prev_hash, &out_hash)?;

        self.finish(msg, session_id, pos as u64, out, trace, sig, None)
    }

    /// ⚑ **Ein Stapel, auch über mehrere Sitzungen** (Continuous Batching,
    /// 2026-11-15, nach Orca, Yu et al., OSDI 2022): dieselben Prüfungen und
    /// dieselbe Rechnung wie [`Self::process`] je Nachricht, nur rechnen
    /// alle Nachrichten jede Ebene zugleich
    /// ([`IntegerModel::ebenen_stapel`]): Die Gewichte einer Ebene werden
    /// einmal je Stapel gelesen statt einmal je Nachricht. Gemessen am
    /// 2026-11-15 (sparrow, Decode): vier Token kosten 2,81 Schritte.
    ///
    /// ⚑ **Erst alles prüfen, dann rechnen.** Ein Fehler in irgendeiner
    /// Nachricht lehnt den Stapel ab, bevor ein Speicher berührt ist; der
    /// Koordinator kann ihn dann Nachricht für Nachricht wiederholen, und
    /// eine fremde Sitzung bezahlt nicht für eine kaputte.
    ///
    /// Token für Token wie bisher, wenn der Stapel nur eine Nachricht hat,
    /// das Modell vier Ströme führt oder die Positionen einer Sitzung im
    /// Stapel nicht aufsteigen; das Ergebnis ist dasselbe, nur ohne Bündel.
    pub fn process_stapel(&self, msgs: &[PodMessage]) -> Result<Vec<ShardOut>, String> {
        let einzeln = |msgs: &[PodMessage]| msgs.iter().enumerate().map(|(i, m)| self.process(m).map_err(|e| format!("Stapel, Nachricht {i}: {e}"))).collect();
        if msgs.len() <= 1 || self.model.vierstrom.is_some() {
            return einzeln(msgs);
        }
        // 0) Eine Sitzung, deren Stapel bei Position 0 beginnt, beginnt neu
        //    (Fund 709); vor dem Sperren der Speicher, denn das Vergessen
        //    sperrt sie selbst.
        for m in msgs {
            self.neu_beginnen_bei_null(m.session_id, m.position as usize);
        }
        // 1) Prüfen und die Eingänge bilden, ohne einen Speicher anzufassen.
        let grenze = self.model.kontextgrenze();
        let mut eingaenge: Vec<Vec<i16>> = Vec::with_capacity(msgs.len());
        let mut sitzungen: Vec<u64> = Vec::new();
        let mut laenge: HashMap<u64, usize> = HashMap::new();
        let mut zuletzt: HashMap<u64, usize> = HashMap::new();
        {
            let caches = sperre(&self.caches, "KV-Cache")?;
            for (i, m) in msgs.iter().enumerate() {
                let fehler = |e: String| format!("Stapel, Nachricht {i}: {e}");
                if !m.is_valid_frame() {
                    return Err(fehler("ungültiger Rahmen (Magic)".to_string()));
                }
                if m.flags & crate::wire::FLAG_ABORT != 0 {
                    return Err(fehler("Request abgebrochen".to_string()));
                }
                self.spur_passt(m).map_err(fehler)?;
                let pos = m.position as usize;
                if pos >= grenze {
                    return Err(fehler(format!("Position {pos} liegt hinter der Kontextgrenze {grenze} des Modells (Shard {})", self.shard_index)));
                }
                // Die Länge des Speichers, fortgeschrieben über die
                // Nachrichten derselben Sitzung davor im Stapel.
                let n = *laenge.entry(m.session_id).or_insert_with(|| caches.get(&m.session_id).map_or(0, KVCache::laenge));
                if pos > n {
                    return Err(fehler(format!("Position {pos} liegt hinter dem Ende des KV-Speichers ({n} Positionen, Shard {})", self.shard_index)));
                }
                // ⛔️ Fund 719: nur genau die nächste Position.
                if pos < n {
                    return Err(fehler(format!(
                        "Position {pos} ist schon gerechnet ({n} Positionen, Shard {}); eine Wiederholung verschöbe den Zustand",
                        self.shard_index
                    )));
                }
                laenge.insert(m.session_id, pos + 1);
                // ⚠️ Je Sitzung aufsteigend, sonst Token für Token.
                if zuletzt.insert(m.session_id, pos).is_some_and(|vorher| vorher >= pos) {
                    drop(caches);
                    return einzeln(msgs);
                }
                if !sitzungen.contains(&m.session_id) {
                    sitzungen.push(m.session_id);
                }
                if m.carries_tokens() {
                    if !self.has_embedding {
                        return Err(fehler("Token-Eingang an einem Shard ohne Embedding".to_string()));
                    }
                    let tokens = unpack_tokens(&m.payload).map_err(fehler)?;
                    if tokens.len() != 1 {
                        return Err(fehler("erwarte genau ein Token je Nachricht".to_string()));
                    }
                    let token = tokens[0] as usize;
                    if token >= self.model.vocab_size {
                        return Err(fehler(format!(
                            "Token {token} liegt ausserhalb des Vokabulars ({} Einträge, Shard {}, Position {pos})",
                            self.model.vocab_size, self.shard_index
                        )));
                    }
                    let mut hidden = self.model.embed_token(token);
                    faelschen(&mut hidden, true);
                    eingaenge.push(hidden);
                } else {
                    if m.payload.len() != self.model.hidden_size {
                        return Err(fehler(format!(
                            "Aktivierungen haben Länge {}, das Modell erwartet {} (Shard {}, Position {pos})",
                            m.payload.len(),
                            self.model.hidden_size,
                            self.shard_index
                        )));
                    }
                    if !verify_input_hash(&m.payload, &m.trace) {
                        return Err(fehler(format!(
                            "Eingangs-Hash stimmt nicht mit der Spur überein (Shard {}, Position {pos})",
                            self.shard_index
                        )));
                    }
                    let mut hidden = m.payload.clone();
                    faelschen(&mut hidden, false);
                    eingaenge.push(hidden);
                }
            }
        }
        // 2) Die Speicher der Sitzungen, je einer.
        let mut speicher: Vec<KVCache> = Vec::with_capacity(sitzungen.len());
        for s in &sitzungen {
            speicher.push(self.take_cache(*s)?);
        }
        let belegung: Vec<Belegung> = msgs
            .iter()
            .map(|m| Belegung {
                speicher: sitzungen.iter().position(|s| *s == m.session_id).unwrap_or(0),
                position: m.position as usize,
            })
            .collect();
        // 3) Ebene für Ebene, mit dem Hash jedes Tokens in seiner Spur.
        let mut spuren: Vec<Vec<[u8; 32]>> = msgs.iter().map(|m| m.trace.clone()).collect();
        let mut zustaende = eingaenge;
        for i in self.layer_start..self.layer_end {
            zustaende = match self.model.ebenen_stapel(zustaende, &belegung, &mut speicher, i, i + 1) {
                Some(z) => z,
                None => {
                    for (s, c) in sitzungen.iter().zip(speicher) {
                        self.put_cache(*s, c)?;
                    }
                    return Err("der Stapel passt nicht zum Modell".to_string());
                }
            };
            for (spur, z) in spuren.iter_mut().zip(&zustaende) {
                spur.push(activation_hash(z));
            }
        }
        // 📌 Fund 704: Jede Position ist durch alle Ebenen dieses Shards
        //    gelaufen; ohne das hatte ein Bereich, der mit einer
        //    Zustandsschicht beginnt, die Laenge null.
        for b in &belegung {
            speicher[b.speicher].durchgelaufen(b.position);
        }
        for (s, c) in sitzungen.iter().zip(speicher) {
            self.put_cache(*s, c)?;
        }
        // 4) ⚑ Der Kopf für alle gesampelten Nachrichten in einem Zug
        //    (`head_logits_stapel`): am Kopf-Shard die halbe Arbeit.
        let mut koepfe: Vec<Option<Vec<i32>>> = vec![None; msgs.len()];
        if self.has_lm_head {
            let welche: Vec<usize> = (0..msgs.len()).filter(|&b| msgs[b].flags & FLAG_SAMPLE != 0).collect();
            let eingaenge: Vec<&[i16]> = welche.iter().map(|&b| zustaende[b].as_slice()).collect();
            for (b, l) in welche.into_iter().zip(self.model.head_logits_stapel(&eingaenge)) {
                koepfe[b] = Some(l);
            }
        }
        // 5) Je Nachricht unterschreiben und abschliessen, in Stapelfolge.
        let mut aus = Vec::with_capacity(msgs.len());
        for (((m, out), trace), kopf) in msgs.iter().zip(zustaende).zip(spuren).zip(koepfe) {
            let prev = if m.carries_tokens() { ZERO_HASH } else { m.trace.last().copied().unwrap_or(ZERO_HASH) };
            let out_hash = *trace.last().unwrap_or(&ZERO_HASH);
            let sig = self.sign_transition(&m.segment_id, m.position, &prev, &out_hash)?;
            aus.push(self.finish(m, m.session_id, m.position, out, trace, sig, kopf)?);
        }
        Ok(aus)
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
    /// Ein leerer KV-Cache für den Layer-Bereich dieses Shards.
    ///
    /// Für die Nachrechnung von vorn: Wer bei Position 0 beginnt, hat
    /// keinen Vorlauf, und wer bei Position `t` beginnt, baut ihn damit
    /// über die Positionen 0 bis `t-1` auf.
    pub fn frischer_cache(&self) -> KVCache {
        KVCache::for_range(self.layer_start, self.layer_end, self.model.num_kv_heads)
    }

    /// ⚑ **Wie lange ein Stapel von `n` Eingaben durch den eigenen Bereich
    /// braucht** (Fund 705, fuer die Einmessung beim Start): `n` Positionen ab
    /// 0 in einem frischen Speicher, Ebene fuer Ebene wie
    /// [`Self::process_stapel`], ohne Sitzung, Spur und Unterschrift. Die
    /// Eingabe ist wie bei der Zeitprobe die Einbettung eines festen Tokens;
    /// die Arbeit haengt nicht an den Werten.
    pub fn stapelzeit(&self, n: usize) -> std::time::Duration {
        let n = n.clamp(1, crate::shardweg::MAX_STAPEL);
        let mut speicher = vec![self.frischer_cache()];
        let belegung: Vec<Belegung> = (0..n).map(|position| Belegung { speicher: 0, position }).collect();
        let mut zustaende: Vec<Vec<i16>> = (0..n).map(|_| self.model.embed_token(ZEITPROBE_TOKEN)).collect();
        let ab = std::time::Instant::now();
        for i in self.layer_start..self.layer_end {
            match self.model.ebenen_stapel(zustaende, &belegung, &mut speicher, i, i + 1) {
                Some(z) => zustaende = z,
                None => break,
            }
        }
        ab.elapsed()
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
        // ⚑ **Hier wird nichts mehr archiviert** (E10, 2026-08-30).
        //
        // Bis zum selben Tag lag hier erst jede Layer-Ausgabe, dann der
        // Eingang; über die Streitfrist zwischen 65 GiB und 1,8 TiB je
        // Knoten, zusätzlich zur Modellgröße.
        //
        // Nötig war das, weil der **Angeklagte** im Streitfall die
        // Eingabe offenlegen musste. Das tut jetzt der **Ankläger**: Die
        // Bisektion endet an der ersten Abweichung, bei `j-1` sind sich
        // beide einig, und er hat den Wert ohnehin, weil er das Segment
        // gerade nachgerechnet hat.
        //
        // Was bleibt, ist die **Spur**, und die entsteht ohnehin hier:
        // 32 Byte je Layer statt 7 KiB je Eingang, und sie ist die
        // Zusicherung, gegen die geurteilt wird.
        let _ = segment_id;
        for i in self.layer_start..self.layer_end {
            hidden = self.model.run_layers(hidden, pos, cache, i, i + 1);
            trace.push(activation_hash(&hidden));
        }
        hidden
    }

    /// Gemeinsamer Abschluss: weiterreichen (Zwischen-Shard) oder sampeln
    /// (End-Shard). `logits`: schon gestapelt gerechnet
    /// ([`Self::process_stapel`]), sonst rechnet der Kopf hier.
    #[allow(clippy::too_many_arguments)]
    fn finish(
        &self,
        msg: &PodMessage,
        session_id: u64,
        position: u64,
        out: Vec<i16>,
        trace: Vec<[u8; 32]>,
        sig: myl_types::bls::BlsSignature,
        logits: Option<Vec<i32>>,
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
            let logits = logits.unwrap_or_else(|| self.model.head_logits(&out));
            let token = self.model.greedy_next(&logits) as u32;

            // Fund 36, letzter Teil: **die Zahlen** festhalten, aus denen
            // entschieden wurde, nicht nur die Entscheidung. Ein Token ist
            // ein Argmax über `vocab_size` Zahlen und ändert sich erst,
            // wenn deren Rangfolge kippt; gemessen an 0,5B blieb er bei
            // 0,1 % veränderter Modellbytes unverändert.
            sperre(&self.dekodier_digest, "Dekodier-Digest")?
                .entry(session_id)
                .or_insert_with(DekodierDigest::neu)
                .schritt(&logits, token);

            // Generierungs-Buchhaltung + Feedback.
            let mut gen = sperre(&self.gen_count, "Generierungszaehler")?;
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
    ///
    /// Diese Box gibt es seit E10 (2026-08-30) nicht mehr, der Shard
    /// archiviert nichts. **Der Test bleibt trotzdem**: Er bewacht die
    /// Eigenschaft, nicht das damalige Gegenbeispiel, und die naechste
    /// Box ohne Schranken faellt genauso auf.
    #[test]
    fn shardnode_ist_ueber_threads_teilbar() {
        fn ist_send<T: Send>() {}
        fn ist_sync<T: Sync>() {}
        ist_send::<ShardNode>();
        ist_sync::<ShardNode>();
    }
}
