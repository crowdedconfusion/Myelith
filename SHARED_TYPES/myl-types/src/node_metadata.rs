//! Geo-/AS-Diversitäts-Metadaten für Pods (Whitepaper Kap. 4.4).
//!
//! Jeder Node veröffentlicht seine geografische Region und sein
//! Autonomous System (AS), damit das Netzwerk sicherstellen kann,
//! dass Pods aus Nodes in verschiedenen Zonen bestehen. Das erhöht
//! die Resilienz gegen regionale Ausfälle (Naturkatastrophen,
//! Netzwerkpartitionen, AS-Ausfälle).
//!
//! **Konsens-Feld:** Die Region/AS-Zuordnung ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! **Design:** Metadaten werden zusammen mit den Latenz-Attesten
//! verbreitet (Phase 2.2). Jeder Node signiert seine eigenen Metadaten.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::ids::MinerId;

/// Geografische Region eines Nodes (grob, für Diversitätsprüfung).
///
/// Die Regionen sind bewusst grob gehalten (Kontinente + große Regionen),
/// um Privacy zu schützen und gleichzeitig sinnvolle Diversität zu ermöglichen.
/// Fine-grained Locations (Städte, Koordinaten) werden nicht gespeichert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
pub enum GeoRegion {
    /// Nordamerika (USA, Kanada, Mexiko)
    NorthAmerica,
    /// Südamerika (Brasilien, Argentinien, Chile, etc.)
    SouthAmerica,
    /// Europa (EU, UK, Schweiz, Norwegen, etc.)
    Europe,
    /// Afrika (alle afrikanischen Länder)
    Africa,
    /// Asien (China, Japan, Indien, Südostasien, etc.)
    Asia,
    /// Ozeanien (Australien, Neuseeland, Pazifikinseln)
    Oceania,
    /// Middle East (Türkei, Saudi-Arabien, Iran, etc.)
    MiddleEast,
}

impl GeoRegion {
    /// Alle Regionen in kanonischer Reihenfolge.
    pub fn all() -> [GeoRegion; 7] {
        [
            Self::NorthAmerica,
            Self::SouthAmerica,
            Self::Europe,
            Self::Africa,
            Self::Asia,
            Self::Oceania,
            Self::MiddleEast,
        ]
    }

    /// Menschlich lesbare Bezeichnung.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NorthAmerica => "North America",
            Self::SouthAmerica => "South America",
            Self::Europe => "Europe",
            Self::Africa => "Africa",
            Self::Asia => "Asia",
            Self::Oceania => "Oceania",
            Self::MiddleEast => "Middle East",
        }
    }
}

impl std::fmt::Display for GeoRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Autonomous System Number (ASN) eines Nodes.
///
/// Das AS identifiziert das Netzwerk, in dem der Node betrieben wird.
/// Nodes im selben AS teilen sich typischerweise die gleiche Infrastruktur
/// (Router, Peering), daher sollten Pods Nodes aus verschiedenen AS
/// enthalten.
///
/// **Format:** 32-bit unsigned integer (ASPLAIN-Format, RFC 6793).
/// Private ASNs (64512-65534, 4200000000-4294967294) sind erlaubt,
/// werden aber für Diversitätsprüfung nicht bevorzugt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
pub struct Asn(pub u32);

impl Asn {
    /// Prüft, ob die ASN ein privates AS ist (RFC 6963).
    pub fn is_private(&self) -> bool {
        (64512..=65534).contains(&self.0) || (4200000000..=4294967294).contains(&self.0)
    }
}

impl std::fmt::Display for Asn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AS{}", self.0)
    }
}

/// Geo-/AS-Metadaten eines Nodes.
///
/// Wird zusammen mit dem Latenz-Attest verbreitet und signiert.
/// Andere Nodes können diese Metadaten verwenden, um die Diversität
/// eines Pods zu prüfen.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct NodeMetadata {
    /// Der Miner, dem diese Metadaten gehören.
    pub miner: MinerId,
    /// Geografische Region (grob, für Diversitätsprüfung).
    pub region: GeoRegion,
    /// Autonomous System Number (ASN).
    pub asn: Asn,
    /// Zeitstempel der Erstellung (Unix-Millisekunden).
    pub timestamp_ms: u64,
}

impl NodeMetadata {
    /// Validiert die Struktur der Metadaten.
    ///
    /// Prüft:
    /// - Zeitstempel ist nicht in der Zukunft (mit 5 min Toleranz)
    pub fn validate_structure(&self) -> Result<(), NodeMetadataError> {
        // Zeitstempel prüfen (nicht mehr als 5 min in der Zukunft)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as u64;
        let max_future_ms = 5 * 60 * 1000; // 5 Minuten
        if self.timestamp_ms > now_ms + max_future_ms {
            return Err(NodeMetadataError::FutureTimestamp {
                metadata_ms: self.timestamp_ms,
                now_ms,
            });
        }

        Ok(())
    }
}

/// Fehler bei der Validierung von Node-Metadaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMetadataError {
    /// Zeitstempel liegt mehr als 5 min in der Zukunft.
    FutureTimestamp { metadata_ms: u64, now_ms: u64 },
}

impl std::fmt::Display for NodeMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FutureTimestamp { metadata_ms, now_ms } => {
                write!(
                    f,
                    "Node-Metadaten Zeitstempel {} ms liegt mehr als 5 min in der Zukunft (jetzt: {} ms)",
                    metadata_ms, now_ms
                )
            }
        }
    }
}

impl std::error::Error for NodeMetadataError {}

/// Diversitätsprüfung für Pods.
///
/// Prüft, ob eine Liste von MinerIds ausreichend divers ist (verschiedene
/// Regionen und AS). Wird von CONSENSUS bei der Pod-Bildung verwendet.
pub struct DiversityChecker {
    /// Minimale Anzahl verschiedener Regionen in einem Pod.
    pub min_regions: usize,
    /// Minimale Anzahl verschiedener AS in einem Pod.
    pub min_asns: usize,
}

impl DiversityChecker {
    /// Standard-Diversitätsanforderungen (Kap. 4.4).
    pub fn new() -> Self {
        Self {
            min_regions: 2, // Mindestens 2 verschiedene Regionen
            min_asns: 3,    // Mindestens 3 verschiedene AS
        }
    }

    /// Prüft, ob die gegebenen Metadaten die Diversitätsanforderungen erfüllen.
    ///
    /// Gibt `true` zurück, wenn mindestens `min_regions` verschiedene Regionen
    /// und `min_asns` verschiedene AS vorhanden sind.
    pub fn check_diversity(&self, metadata: &[NodeMetadata]) -> bool {
        use std::collections::HashSet;

        let regions: HashSet<GeoRegion> = metadata.iter().map(|m| m.region).collect();
        let asns: HashSet<Asn> = metadata.iter().map(|m| m.asn).collect();

        regions.len() >= self.min_regions && asns.len() >= self.min_asns
    }
}

impl Default for DiversityChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metadata(miner_byte: u8, region: GeoRegion, asn: u32) -> NodeMetadata {
        NodeMetadata {
            miner: MinerId::new([miner_byte; 32]),
            region,
            asn: Asn(asn),
            timestamp_ms: 1000,
        }
    }

    #[test]
    fn geo_region_all() {
        assert_eq!(GeoRegion::all().len(), 7);
    }

    #[test]
    fn asn_private_ranges() {
        assert!(Asn(64512).is_private());
        assert!(Asn(65534).is_private());
        assert!(!Asn(65535).is_private());
        assert!(Asn(4200000000).is_private());
        assert!(Asn(4294967294).is_private());
        assert!(!Asn(4294967295).is_private());
        assert!(!Asn(13335).is_private()); // Cloudflare (public)
    }

    #[test]
    fn metadata_validate_structure_ok() {
        let metadata = test_metadata(1, GeoRegion::Europe, 13335);
        assert!(metadata.validate_structure().is_ok());
    }

    #[test]
    fn metadata_validate_structure_future_timestamp() {
        let mut metadata = test_metadata(1, GeoRegion::Europe, 13335);
        metadata.timestamp_ms = u64::MAX; // Weit in der Zukunft

        assert!(matches!(
            metadata.validate_structure(),
            Err(NodeMetadataError::FutureTimestamp { .. })
        ));
    }

    #[test]
    fn diversity_checker_sufficient() {
        let checker = DiversityChecker::new();
        let metadata = vec![
            test_metadata(1, GeoRegion::Europe, 13335),
            test_metadata(2, GeoRegion::Europe, 15169), // Google
            test_metadata(3, GeoRegion::NorthAmerica, 16509), // Amazon
        ];

        // 2 Regionen (Europe, NorthAmerica) >= min_regions (2) ✓
        // 3 AS (13335, 15169, 16509) >= min_asns (3) ✓
        assert!(checker.check_diversity(&metadata));
    }

    #[test]
    fn diversity_checker_insufficient_regions() {
        let checker = DiversityChecker::new();
        let metadata = vec![
            test_metadata(1, GeoRegion::Europe, 13335),
            test_metadata(2, GeoRegion::Europe, 15169),
            test_metadata(3, GeoRegion::Europe, 16509),
        ];

        // Nur 1 Region (Europe) < min_regions (2) ✗
        assert!(!checker.check_diversity(&metadata));
    }

    #[test]
    fn diversity_checker_insufficient_asns() {
        let checker = DiversityChecker::new();
        let metadata = vec![
            test_metadata(1, GeoRegion::Europe, 13335),
            test_metadata(2, GeoRegion::NorthAmerica, 13335), // Gleiches AS
            test_metadata(3, GeoRegion::Asia, 13335), // Gleiches AS
        ];

        // 3 Regionen ✓, aber nur 1 AS < min_asns (3) ✗
        assert!(!checker.check_diversity(&metadata));
    }
}
