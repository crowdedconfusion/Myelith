//! Der Nachrechner: ein [`SegmentAuditor`], der ein echtes Modell fährt.
//!
//! # ⚑ Warum er nicht `myl-pod` ruft
//!
//! Naheliegend wäre, die Spur mit dem Code zu rechnen, der sie erzeugt
//! hat. **Dann prüfte der Prüfer den Geprüften mit dessen eigenem
//! Werkzeug**, und ein übereinstimmendes Ergebnis hieße nur, dass
//! dieselbe Funktion zweimal dasselbe tut.
//!
//! Dieser Nachrechner geht deshalb **selbst** durch
//! `integer_llm_runtime`. Was beide teilen, ist der **Spur-Vertrag**
//! [`myl_types::uebergang::activation_hash`], und der ist ein
//! Konsensdatum: Wer ihn anders rechnet, bekommt eine andere Spur, und
//! ein Streit darüber wäre nicht entscheidbar, sondern nur zwei
//! Meinungen.
//!
//! ⚑ **Genau darauf ruht das Versprechen des Projekts.** Verbindlich sind
//! das Quantisierungsschema und wenige arithmetische Festlegungen, nicht
//! die Ausführung (Kap. 6.2). Zwei Wege durch dieselbe Spezifikation
//! müssen dasselbe ergeben, **sonst ist die Bitgleichheit eine
//! Behauptung**.
//!
//! # Was er braucht und was er nicht weiß
//!
//! Er braucht das Modell und den **Layerbereich**, den der beschuldigte
//! Shard gerechnet hat. Beides steht nicht im Segment: Die Kette kennt
//! nur Kennung und Spurwurzel. ⚑ **Der Bereich kommt deshalb vom
//! Aufrufer**, und wer ihn falsch angibt, bekommt eine Abweichung, die
//! keine ist. Das steht hier, weil es eine echte Fehlerquelle ist und
//! keine theoretische.

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

use crate::checker::{CheckError, SegmentAuditor};

/// Rechnet die Spur eines Shards mit einem geladenen Modell nach.
pub struct ModellAuditor {
    modell: std::sync::Arc<integer_llm_runtime::model::IntegerModel>,
    /// Erste Layer des Bereichs (einschließlich).
    von: usize,
    /// Erste Layer **nach** dem Bereich.
    bis: usize,
    /// Die Position in der Sequenz, für die gerechnet wird.
    position: usize,
}

impl ModellAuditor {
    /// Neu, mit Modell und Layerbereich.
    ///
    /// Gibt `None`, wenn der Bereich außerhalb des Modells liegt oder
    /// leer ist: **Ein leerer Bereich ergäbe eine leere Spur**, und die
    /// verglände sich mit allem.
    pub fn neu(
        modell: std::sync::Arc<integer_llm_runtime::model::IntegerModel>,
        von: usize,
        bis: usize,
        position: usize,
    ) -> Option<Self> {
        if von >= bis || bis > modell.num_layers {
            return None;
        }
        Some(Self {
            modell,
            von,
            bis,
            position,
        })
    }

    /// Die Aktivierungen aus der Bytefolge der Anfrage.
    ///
    /// ⚑ **Little-endian `i16`, dieselbe Kodierung wie im Spur-Hash.**
    /// Eine ungerade Byte-Zahl ist kein halber Wert, sondern ein Fehler:
    /// Stillschweigend abzuschneiden hieße, über eine andere Eingabe zu
    /// rechnen als der Beschuldigte.
    fn aktivierungen(roh: &[u8]) -> Result<Vec<i16>, CheckError> {
        if !roh.len().is_multiple_of(2) {
            return Err(CheckError::EmptySegment);
        }
        Ok(roh
            .as_chunks::<2>().0.iter()
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect())
    }
}

impl SegmentAuditor for ModellAuditor {
    fn audit_segment(
        &self,
        _segment_id: SegmentId,
        input_activations: &[u8],
    ) -> Result<Vec<Hash>, CheckError> {
        // ⛔️ **Fund 373 (2026-09-14): hinter Position null rechnete er etwas
        // anderes als der Beweiser.** Der Speicher unten ist leer. Der
        // Shard, der das Segment gerechnet hat, hielt an Position `p` die
        // Keys und Values aller Positionen davor, und die Aufmerksamkeit
        // liest sie. Der Nachrechner las nur den eigenen Eintrag: **Seine
        // Spur wich bei jedem ehrlichen Shard ab**, sobald `p > 0` war, und
        // die Pruefungen liefen nur mit Position null. Aufgefallen, als der
        // KV-Speicher zusammenhaengend wurde und die Luecke nicht mehr
        // annahm.
        //
        // ⚑ **Bis der Verlauf im Streitfall mitkommt, wird abgelehnt statt
        // falsch gerechnet**: Eine Abweichung, die keine ist, schluege einen
        // ehrlichen Shard.
        if self.position > 0 {
            return Err(CheckError::OhneVerlauf { position: self.position });
        }
        let mut hidden = Self::aktivierungen(input_activations)?;
        if hidden.len() != self.modell.hidden_size {
            return Err(CheckError::EmptySegment);
        }
        let mut cache = integer_llm_runtime::kv_cache::KVCache::new(
            self.modell.num_layers,
            self.modell.num_kv_heads,
        );
        let mut spur = Vec::with_capacity(self.bis - self.von);
        for i in self.von..self.bis {
            hidden = self
                .modell
                .run_layers(hidden, self.position, &mut cache, i, i + 1);
            spur.push(Hash(myl_types::uebergang::activation_hash(&hidden)));
        }
        Ok(spur)
    }
}

/// Was das Nachrechnen eines Verlaufs ergab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verlaufsergebnis {
    /// Jeder Eintrag jeder Position stimmt.
    Gleich,
    /// ⚑ **Die erste Abweichung**, nach Position und dann nach Eintrag:
    /// `eintrag` zählt in der Kette `[Eingang] ++ Spur`, also ist Eintrag
    /// `j + 1` die Ausgabe der Layer `j`. Bis dahin stimmte alles, und das
    /// ist genau der Ausgangspunkt der Bisektion (Whitepaper 6.6): Bei
    /// `eintrag - 1` sind sich beide einig.
    Abweichung {
        /// Die Position in der Sitzung.
        position: usize,
        /// Der Eintrag in der Kette dieser Position.
        eintrag: usize,
    },
}

/// ⚑ **Rechnet einen Verlauf ab den Tokens nach** (Stufe 2, T6c,
/// 2026-10-29; Entscheidung vom 2026-10-27: die ganze Pipeline ab den
/// Tokens). `tokens[p]` ist der Eingang der Position `p`, `ketten[p]` ihre
/// Kette `[Eingang] ++ Spur` (eine Ausgabe je Layer), wie
/// `myl_types::verlauf_pruefen` sie geprüft hat.
///
/// ⚑ **Fund 373 ist damit geschlossen**: Der Nachrechner hält jetzt, was der
/// Shard hielt, nämlich Keys und Values aller Positionen davor, weil er sie
/// selbst von Position null an rechnet. Gerechnet wird als Prefill
/// ([`verlauf_als_prefill`]), Ebene für Ebene über alle Positionen; die
/// Konformitätsprobe gegen den schrittweisen Weg ([`verlauf_schrittweise`],
/// wie im Pod) steht im Test mit echtem Modell. 📌 Bis zu dieser Probe
/// rechnete der Nachrechner schrittweise: Ein Unterschied schlüge hier einen
/// ehrlichen Pod.
///
/// ⚑ **Der eigene Weg** (siehe oben): durch `integer_llm_runtime`, nicht
/// durch den Code des Pods. Geteilt ist nur der Spur-Vertrag.
///
/// ⚑ **Dazu der Hash der Logits an der letzten Position** (Festlegung des
/// Projektinhabers vom 2026-10-29, Festlegen vor Aufdecken): Er steht in
/// keiner Spur, also kennt ihn nur, wer bis zum LM-Kopf gerechnet hat; zwei
/// Checker, die ihn gleich aufdecken, haben beide gerechnet. Nach einer
/// Abweichung rechnet der Nachrechner mit seinen eigenen Werten weiter, die
/// Logits sind also immer die ehrlichen.
///
/// Fehler: ein Token ausserhalb des Vokabulars (dann hat der Pod über etwas
/// gerechnet, das kein Token ist; das gehört vor den Aufruf), ungleiche
/// Längen von Tokens und Ketten, oder ein leerer Verlauf.
pub fn verlauf_nachrechnen(
    modell: &integer_llm_runtime::model::IntegerModel,
    tokens: &[u32],
    ketten: &[Vec<Hash>],
) -> Result<Verlaufspruefung, CheckError> {
    match verlauf_als_prefill(modell, tokens, ketten)? {
        Some(p) => Ok(p),
        None => verlauf_schrittweise(modell, tokens, ketten),
    }
}

/// ⚑ **Der Verlauf als Prefill** (Frage 5 vom 2026-10-29, VeriLLM: Prüfung
/// über einen Prefill statt Schritt für Schritt): alle Positionen eines
/// Fensters Ebene für Ebene über `vorbereiten_stapel_mit_ebenen`, nach
/// jeder Ebene alle Ausgänge gegen die Spur. **Dieselbe Rechnung, andere
/// Reihenfolge** (siehe dort); dass sie bitgleich ist, belegt die
/// Konformitätsprobe gegen [`verlauf_schrittweise`] im Test mit echtem
/// Modell. Die erste Abweichung ist die kleinste nach Position und dann
/// Eintrag, wie beim schrittweisen Weg. `None` für ein Modell, das im
/// Stapel keinen Ausgang je Ebene hat.
pub fn verlauf_als_prefill(
    modell: &integer_llm_runtime::model::IntegerModel,
    tokens: &[u32],
    ketten: &[Vec<Hash>],
) -> Result<Option<Verlaufspruefung>, CheckError> {
    if tokens.is_empty() || tokens.len() != ketten.len() {
        return Err(CheckError::Verlauf { position: tokens.len().min(ketten.len()) });
    }
    let mut ids = Vec::with_capacity(tokens.len());
    for (q, t) in tokens.iter().enumerate() {
        if *t as usize >= modell.vocab_size {
            return Err(CheckError::Verlauf { position: q });
        }
        ids.push(*t as usize);
    }
    let fenster = integer_llm_runtime::model::VORBEREITUNGSFENSTER;
    let mut cache = integer_llm_runtime::kv_cache::KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut erste: Option<(usize, usize)> = None;
    let mut letzter = Vec::new();
    for (f, stueck) in ids.chunks(fenster).enumerate() {
        let start = f * fenster;
        let mut blick = |ebene: usize, zustaende: &[Vec<i16>]| {
            for (b, z) in zustaende.iter().enumerate() {
                let stelle = (start + b, ebene + 1);
                let gleich = ketten[stelle.0]
                    .get(stelle.1)
                    .is_some_and(|h| *h.as_bytes() == myl_types::uebergang::activation_hash(z));
                if !gleich && erste.is_none_or(|e| stelle < e) {
                    erste = Some(stelle);
                }
            }
        };
        let Some(aus) = modell.vorbereiten_stapel_mit_ebenen(stueck, start, &mut cache, &mut blick) else {
            return Ok(None);
        };
        letzter = aus.last().cloned().unwrap_or_default();
    }
    // Eine Kette mit mehr Einträgen als Layern behauptet Arbeit, die es
    // nicht gibt.
    for (q, kette) in ketten.iter().enumerate() {
        let stelle = (q, modell.num_layers + 1);
        if kette.len() != modell.num_layers + 1 && erste.is_none_or(|e| stelle < e) {
            erste = Some(stelle);
        }
    }
    let ergebnis = match erste {
        None => Verlaufsergebnis::Gleich,
        Some((position, eintrag)) => Verlaufsergebnis::Abweichung { position, eintrag },
    };
    Ok(Some(Verlaufspruefung { ergebnis, logits: logits_hash(&modell.head_logits(&letzter)) }))
}

/// **Der Verlauf Schritt für Schritt**: Position für Position, Layer für
/// Layer, wie der Pod rechnet. Der Rückfall für Modelle ohne Ausgang je
/// Ebene im Stapel und die Gegenprobe zum Prefill.
pub fn verlauf_schrittweise(
    modell: &integer_llm_runtime::model::IntegerModel,
    tokens: &[u32],
    ketten: &[Vec<Hash>],
) -> Result<Verlaufspruefung, CheckError> {
    if tokens.is_empty() || tokens.len() != ketten.len() {
        return Err(CheckError::Verlauf { position: tokens.len().min(ketten.len()) });
    }
    let mut cache = integer_llm_runtime::kv_cache::KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut ergebnis = Verlaufsergebnis::Gleich;
    let mut hidden = Vec::new();
    for (position, (token, kette)) in tokens.iter().zip(ketten).enumerate() {
        let token = *token as usize;
        if token >= modell.vocab_size {
            return Err(CheckError::Verlauf { position });
        }
        hidden = modell.embed_token(token);
        for layer in 0..modell.num_layers {
            hidden = modell.run_layers(hidden, position, &mut cache, layer, layer + 1);
            let eintrag = layer + 1;
            let gleich = kette
                .get(eintrag)
                .is_some_and(|h| *h.as_bytes() == myl_types::uebergang::activation_hash(&hidden));
            if !gleich && ergebnis == Verlaufsergebnis::Gleich {
                ergebnis = Verlaufsergebnis::Abweichung { position, eintrag };
            }
        }
        // Eine Kette mit mehr Einträgen als Layern behauptet Arbeit, die es
        // nicht gibt.
        if kette.len() != modell.num_layers + 1 && ergebnis == Verlaufsergebnis::Gleich {
            ergebnis = Verlaufsergebnis::Abweichung { position, eintrag: modell.num_layers + 1 };
        }
    }
    Ok(Verlaufspruefung { ergebnis, logits: logits_hash(&modell.head_logits(&hidden)) })
}

/// Was [`verlauf_nachrechnen`] liefert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verlaufspruefung {
    /// Gleich oder die erste Abweichung.
    pub ergebnis: Verlaufsergebnis,
    /// SHA-256 über die Logits an der letzten Position, je Wert `i32` in
    /// Little Endian.
    pub logits: Hash,
}

/// SHA-256 über Logits, je Wert `i32` in Little Endian.
pub fn logits_hash(logits: &[i32]) -> Hash {
    let bytes: Vec<u8> = logits.iter().flat_map(|v| v.to_le_bytes()).collect();
    Hash::sha256(&bytes)
}

/// ⚑ **Der Beleg des Anfechters** (T6d, Whitepaper 6.6, Festlegung des
/// Projektinhabers vom 2026-10-29): die Eingänge der Layer `layer` für die
/// Positionen `0..=position`, aus den Tokens selbst gerechnet. Bis zu
/// dieser Layer und Position war der Anfechter mit dem Pod einig, also trifft
/// jeder Eingang den Hash, den der Pod bezeugt hat; das Komitee prüft es
/// ([`schied_nachrechnen`]). Für `layer` null braucht es keinen Beleg (der
/// Eingang ist die Einbettung des Tokens), dann ist die Liste leer.
pub fn schiedsbeleg(
    modell: &integer_llm_runtime::model::IntegerModel,
    tokens: &[u32],
    position: usize,
    layer: usize,
) -> Result<Vec<Vec<i16>>, CheckError> {
    if layer == 0 {
        return Ok(Vec::new());
    }
    if position >= tokens.len() || layer > modell.num_layers {
        return Err(CheckError::Verlauf { position });
    }
    let mut cache = integer_llm_runtime::kv_cache::KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut aus = Vec::with_capacity(position + 1);
    for (q, token) in tokens.iter().take(position + 1).enumerate() {
        let token = *token as usize;
        if token >= modell.vocab_size {
            return Err(CheckError::Verlauf { position: q });
        }
        let hidden = modell.embed_token(token);
        aus.push(modell.run_layers(hidden, q, &mut cache, 0, layer));
    }
    Ok(aus)
}

/// ⚑ **Das Komitee rechnet eine Layer an einer Position nach** (T6d,
/// Whitepaper 6.6): `eintrag` ist der strittige Eintrag der Kette, also
/// der Ausgang der Layer `eintrag - 1`. Ihre Eingänge sind die Einträge
/// `eintrag - 1` der Positionen `0..=position`: für die Layer null die
/// Einbettungen der Tokens (gebunden über den Eingangshash), sonst der
/// Beleg des Anfechters, jeder gegen den Hash geprüft, **den der Pod selbst
/// bezeugt hat**. Gerechnet wird die Layer über alle diese Positionen (für
/// Keys und Values), verglichen nur der Ausgang an `position`.
///
/// `Ok(true)`: Der Pod rechnete falsch. `Ok(false)`: Sein Ausgang stimmt,
/// **oder der Beleg trägt nicht**; beides fällt auf den Anfechter, der die
/// Beweislast hat. Eine Kette mit falscher Länge (Eintrag hinter der
/// letzten Layer) ist eine Behauptung ohne Arbeit, also falsch.
pub fn schied_nachrechnen(
    modell: &integer_llm_runtime::model::IntegerModel,
    tokens: &[u32],
    ketten: &[Vec<Hash>],
    position: usize,
    eintrag: usize,
    beleg: &[Vec<i16>],
) -> Result<bool, CheckError> {
    if position >= tokens.len() || tokens.len() != ketten.len() || eintrag == 0 {
        return Err(CheckError::Verlauf { position });
    }
    if eintrag > modell.num_layers {
        return Ok(ketten[position].len() != modell.num_layers + 1);
    }
    let layer = eintrag - 1;
    let eingaenge: Vec<Vec<i16>> = if layer == 0 {
        let mut e = Vec::with_capacity(position + 1);
        for (q, t) in tokens.iter().take(position + 1).enumerate() {
            if *t as usize >= modell.vocab_size {
                return Err(CheckError::Verlauf { position: q });
            }
            e.push(modell.embed_token(*t as usize));
        }
        e
    } else {
        if beleg.len() != position + 1 {
            return Ok(false);
        }
        let traegt = beleg.iter().zip(ketten).all(|(eingang, kette)| {
            kette.get(layer).is_some_and(|h| *h.as_bytes() == myl_types::uebergang::activation_hash(eingang))
        });
        if !traegt {
            return Ok(false);
        }
        beleg.to_vec()
    };
    let mut cache = integer_llm_runtime::kv_cache::KVCache::new(modell.num_layers, modell.num_kv_heads);
    let mut ausgang = Vec::new();
    for (q, eingang) in eingaenge.into_iter().enumerate() {
        ausgang = modell.run_layers(eingang, q, &mut cache, layer, layer + 1);
    }
    let bezeugt = ketten[position].get(eintrag).map(|h| *h.as_bytes());
    Ok(bezeugt != Some(myl_types::uebergang::activation_hash(&ausgang)))
}
