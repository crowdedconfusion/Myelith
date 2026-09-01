//! Wer minen darf: Hardware-Klasse und Registrierung (Anhang A.2, Schritt 2).
//!
//! # ⚑ Warum die Typen hier stehen und nicht im Scheduler
//!
//! Sie standen bis zum 2026-09-01 in `myl-scheduler`, und der
//! Doc-Kommentar von [`MinerRegistration`] sagte schon damals, sie werde
//! „im Ledger gespeichert". **Das war nie wahr:** `LedgerState` kannte
//! keine Registrierung, und der Scheduler bekam seine Liste vom
//! Aufrufer.
//!
//! Seit die Kette ein Miner-Register führt, brauchen **beide Seiten**
//! denselben Typ: das Kontenbuch, um ihn zu speichern, der Scheduler, um
//! daraus Pods zu bilden. Ein eigener Typ je Seite wären zwei Quellen
//! für dieselbe Aussage, und die laufen auseinander. Derselbe Grund, aus
//! dem das Gegenstandsformat am 2026-08-31 hierher zog.
//!
//! Die **Filterung** selbst bleibt im Scheduler: Sie ist ein
//! Algorithmus, kein Typ, und das Kontenbuch braucht sie nicht.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::ids::MinerId;
use crate::node_metadata::GeoRegion;

/// Hardware-Klasse eines Miners (grob, für Pod-Bildung).
///
/// Die Hardware-Klasse bestimmt, welche Miner zusammen in einem Pod arbeiten können.
/// Pods bestehen aus Minern ähnlicher Hardware, um die Inferenzleistung zu optimieren.
///
/// **Konsens-Feld:** Die Einteilung ist Teil des Konsensvertrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
pub enum HardwareClass {
    /// Kleine GPU (z.B. RTX 3060, 12 GB VRAM) — 1-2 Mrd. Parameter
    SmallGpu,
    /// Mittlere GPU (z.B. RTX 4090, 24 GB VRAM) — 3-7 Mrd. Parameter
    MediumGpu,
    /// Große GPU (z.B. A100, 80 GB VRAM) — 8-13 Mrd. Parameter
    LargeGpu,
    /// Multi-GPU (z.B. 2x A100) — >13 Mrd. Parameter
    MultiGpu,
}

impl HardwareClass {
    /// Alle Hardware-Klassen in kanonischer Reihenfolge.
    pub fn all() -> [HardwareClass; 4] {
        [
            Self::SmallGpu,
            Self::MediumGpu,
            Self::LargeGpu,
            Self::MultiGpu,
        ]
    }

    /// Menschlich lesbare Bezeichnung.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SmallGpu => "Small GPU",
            Self::MediumGpu => "Medium GPU",
            Self::LargeGpu => "Large GPU",
            Self::MultiGpu => "Multi-GPU",
        }
    }
}

impl std::fmt::Display for HardwareClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Miner-Registrierung: enthält MinerId, Hardware-Klasse und Registrierungs-Epoche.
///
/// Wird bei der Miner-Registrierung erstellt und im Ledger gespeichert.
/// Der Scheduler verwendet diese Informationen für die Filterung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct MinerRegistration {
    /// Die MinerId (eindeutige Identifikation).
    pub miner_id: MinerId,
    /// Hardware-Klasse des Miners.
    pub hardware_class: HardwareClass,
    /// Epoche, in der der Miner sich registriert hat.
    pub registration_epoch: u64,
    /// Sein öffentlicher BLS-Schlüssel.
    ///
    /// # ⚑ Er steht hier, weil die Kennung ihn nicht hergibt
    ///
    /// `MinerId` ist `SHA-256` über diesen Schlüssel, und aus einem Hash
    /// folgt kein Urbild. Ohne den Schlüssel im Register kann der
    /// Konsens **keine Aggregatsignatur eines Pods prüfen**, denn er
    /// wüsste nicht, gegen welche Schlüssel.
    ///
    /// # ⚑ Und der Besitz ist damit bewiesen, ohne eigenen Nachweis
    ///
    /// Eine Anmeldung kommt als **unterschriebene Transaktion**, und die
    /// Unterschrift entsteht mit genau diesem Schlüssel. **Wer
    /// unterschreiben kann, besitzt den geheimen Teil** — das ist
    /// dasselbe, was ein `BlsProofOfPossession` belegt, nur bereits
    /// erbracht.
    ///
    /// Das ist keine Feinheit: Ohne Besitznachweis wäre ein
    /// **Rogue-Key-Angriff** möglich, bei dem jemand einen Schlüssel
    /// veröffentlicht, der als Differenz fremder Schlüssel gebildet ist,
    /// und damit Aggregate fälscht. Wer so einen Schlüssel bildet, kann
    /// mit ihm **nicht unterschreiben** und kommt also nicht ins
    /// Register.
    pub schluessel: crate::bls::BlsPublicKey,
    /// Die Zone, in der er rechnet (Entscheidung 3b, 2026-09-01).
    ///
    /// # ⚑ Warum eine Zone und kein Latenzgraph
    ///
    /// Die Pod-Bildung braucht **Nähe**, sonst kostet jeder Token bei
    /// acht Shards acht Sprünge zu hundert Millisekunden und mehr. Der
    /// naheliegende Weg wäre ein gemessener Latenzgraph im Konsens, und
    /// er ist der falsche: **Wer wählt, mit wem er attestiert, formt
    /// mit, in welchem Topf er gemischt wird**, und erhöht damit seine
    /// Chance, beide Seiten eines Redundanzpaars zu besetzen. Dann
    /// verglände Stufe 1 der Verifikation zwei Ergebnisse desselben
    /// Betreibers. **Latenz in den Konsens zu holen, kauft Durchsatz und
    /// verkauft Sicherheit.**
    ///
    /// Eine Zone ist dagegen **eine Angabe je Miner statt einer Matrix
    /// über alle Paare**: O(1) statt O(n²), niemand muss mitzeichnen,
    /// also kann auch niemand jemanden isolieren.
    ///
    /// # ⚑ Und sie ist eine Angabe, keine Messung
    ///
    /// Wer eine falsche Zone nennt, wird nicht ertappt. **Vorwärts
    /// bestraft es sich selbst:** Er landet in einer schnellen Zone,
    /// bremst sie, und die Vergütung folgt der geleisteten Arbeit.
    /// Rückwärts nicht, und das ist Fund 108, unverändert offen.
    ///
    /// **Was sie gegenüber `NodeMetadata::region` besser macht:** Die
    /// steht im Gossip und ist je Leser eine andere; diese steht im
    /// Konsenszustand und ist für alle dieselbe.
    pub zone: GeoRegion,
}

impl MinerRegistration {
    /// Prüft, ob der Miner für Epoche `target_epoch` qualifiziert ist.
    ///
    /// Ein Miner ist qualifiziert, wenn:
    /// - Er sich vor dem Registrierungsschluss (target_epoch - 2) registriert hat
    /// - Seine Hardware-Klasse in `allowed_classes` ist
    pub fn is_qualified(&self, target_epoch: u64, allowed_classes: &[HardwareClass]) -> bool {
        // Registrierungsschluss: Epoche e-2
        let registration_deadline = target_epoch.saturating_sub(2);
        
        // Prüfe Registrierungs-Epoche
        if self.registration_epoch > registration_deadline {
            return false;
        }
        
        // Prüfe Hardware-Klasse
        allowed_classes.contains(&self.hardware_class)
    }
}

/// Trennstring der Pod-Kennung.
pub const DST_PODKENNUNG: &[u8] = b"MYELITH_PODKENNUNG_v1";

/// Die Kennung eines Pods, abgeleitet aus Epoche und Platznummer.
///
/// # ⚑ Fund 109: Das Bündel nannte einen Pod, den die Zuteilung nicht kannte
///
/// `PoIBundle` trägt seit jeher ein Feld `pod: PodId`, und die Zuteilung
/// des Schedulers nummeriert ihre Pods mit `pod_index: u32`. **Zwischen
/// beiden gab es keine Verbindung.** Im ganzen Repositorium entstand
/// eine `PodId` allein über `PodId::new([b; 32])`, und zwar
/// ausschließlich in Tests: Es gab **keine einzige Ableitung**.
///
/// Damit war der Weg vom Bündel zur Besetzung unterbrochen, ohne dass
/// es auffiel, denn beide Seiten waren für sich vollständig und
/// getestet. **Dieselbe Klasse wie Fund 83 und Fund 87:** Zwei Hälften
/// gebaut, die Naht fehlt.
///
/// # Abgeleitet und nicht vergeben
///
/// Die Kennung folgt aus Epoche und Platznummer, wird also von jedem
/// Knoten gleich ausgerechnet und von niemandem vergeben. **Eine
/// vergebene Kennung bräuchte eine Stelle, die vergibt**, und die wäre
/// ein Eintrag im Zustand, eine Reihenfolge und eine Streitfrage dazu.
///
/// Die Epoche gehört hinein, weil Pod 3 der Epoche 7 und Pod 3 der
/// Epoche 8 verschiedene Besetzungen haben. Ohne sie ließe sich ein
/// Bündel aus einer alten Epoche unter neuer Besetzung abrechnen.
pub fn pod_kennung(epoche: u64, pod_index: u32) -> crate::ids::PodId {
    let mut stoff = Vec::with_capacity(DST_PODKENNUNG.len() + 12);
    stoff.extend_from_slice(DST_PODKENNUNG);
    stoff.extend_from_slice(&epoche.to_le_bytes());
    stoff.extend_from_slice(&pod_index.to_le_bytes());
    crate::ids::PodId::new(crate::hash::Hash::sha256(&stoff).0)
}

#[cfg(test)]
mod kennung_tests {
    use super::*;

    /// Fest und wiederholbar: Zwei Knoten rechnen dieselbe Kennung aus.
    #[test]
    fn die_kennung_ist_fest() {
        assert_eq!(pod_kennung(7, 3), pod_kennung(7, 3));
    }

    /// ⚑ Verschiedene Plätze ergeben verschiedene Kennungen.
    #[test]
    fn verschiedene_plaetze_verschiedene_kennungen() {
        assert_ne!(pod_kennung(7, 3), pod_kennung(7, 4));
    }

    /// ⚑ **Und verschiedene Epochen auch.** Ohne die Epoche ließe sich
    /// ein Bündel aus einer alten Epoche unter neuer Besetzung
    /// abrechnen.
    #[test]
    fn verschiedene_epochen_verschiedene_kennungen() {
        assert_ne!(pod_kennung(7, 3), pod_kennung(8, 3));
    }

    /// Der Trennstring wirkt: Die Kennung ist nicht der nackte Hash der
    /// beiden Zahlen.
    #[test]
    fn der_trennstring_wirkt() {
        let mut ohne = Vec::new();
        ohne.extend_from_slice(&7u64.to_le_bytes());
        ohne.extend_from_slice(&3u32.to_le_bytes());
        assert_ne!(
            pod_kennung(7, 3),
            crate::ids::PodId::new(crate::hash::Hash::sha256(&ohne).0)
        );
    }
}
