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
        if roh.len() % 2 != 0 {
            return Err(CheckError::EmptySegment);
        }
        Ok(roh
            .chunks_exact(2)
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
