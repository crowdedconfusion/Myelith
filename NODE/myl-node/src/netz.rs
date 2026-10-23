//! **Die Netze, die dieser Knoten kennt** (Stufe 4 des Dispatchers,
//! 2026-10-22): je Netz Startwert, Podmodell und vTFE-Profil.
//!
//! # ⚑ Warum zwei
//!
//! Die **Probekette** rechnet das Probemodell (Sparrow 0.6B, nur in den
//! Proben auf der Netzliste) und ist die Kette jeder Probe. Das **Testnetz**
//! rechnet das int4-35B: Damit beginnt die Testphase (Festlegung des
//! Projektinhabers vom 2026-10-22; über int8 oder int4 wird danach
//! entschieden). Beide sind keine echte Kette, und beide sagen es in ihrem
//! Startwert.
//!
//! # ⚑ Das Netz folgt aus dem Zustand
//!
//! Der Kettenzustand trägt das Podmodell seit dem Beginn der Kette; daraus
//! findet [`Netz::des_zustands`] das Netz zurück, und mit ihm Startwert und
//! Profil. So brauchen die Ableitungen der Kette, die nur den Zustand sehen
//! (Zuteilung, Gutschrift, Anwenden), keinen zweiten Parameter, und zwei
//! Quellen für dasselbe Netz kann es nicht geben.
//!
//! # ⚑ Was ein Netz bindet
//!
//! - **Startwert**: Eine Transaktion für das eine Netz scheitert im anderen
//!   an der Prüfung, eine Kettendatei des einen öffnet das andere nicht.
//! - **Podmodell**: Abdruck, Ebenenzahl und Kopf; gegen den Katalog
//!   gehalten durch Proben unten.
//! - **Profil**: die Rechenarbeit je Ebene, nach der die Kette gutschreibt.

use myl_tokenomics::vtfe::ModellProfil;
use myl_types::hash::Hash;
use myl_types::podmodell::Podmodell;

/// Ein Netz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Netz {
    /// Der Name auf der Kommandozeile (`--netz`).
    pub name: &'static str,
    /// Der Text, dessen SHA-256 der Startwert ist.
    pub startwert: &'static [u8],
    /// Was die Pods rechnen.
    pub podmodell: Podmodell,
    /// Wie die Kette ihre Arbeit gutschreibt.
    pub profil: ModellProfil,
    /// Der Schlüssel des Artefakts im Katalog, für Meldungen und Proben.
    pub artefakt: &'static str,
    /// ⚑ **Die erste Tür** (seit dem 2026-10-22): Adressen langlebiger
    /// Knoten (`/ip4/…/tcp/…/p2p/…`), die jeder Knoten dieses Netzes beim
    /// Start zusätzlich zu seinen eigenen Bootstrap-Angaben kennt. Kein
    /// Server: Sie öffnen nur den Weg ins verteilte Adressbuch, und jeder
    /// andere Knoten kann dasselbe.
    pub einstieg: &'static [&'static str],
    /// Ob Knoten dieses Netzes einander im lokalen Netz suchen (mDNS).
    pub lokale_suche: bool,
    /// ⚑ **Anker für den Zustandsabzug** (2026-10-22): Höhe und Blockhash
    /// dauerhaft abgelegter Epochengrenzen, eingetragen mit einem Release.
    /// Ein Neuling ohne eigene Kette beginnt beim jüngsten statt bei Block 0.
    /// Nur Höhen, die jeder Knoten auf Dauer behält (`abzug::ist_langzeit`).
    pub abzuege: &'static [(u64, [u8; 32])],
}

impl Netz {
    /// Der Startwert als Hash.
    pub fn startwert_hash(&self) -> Hash {
        Hash::sha256(self.startwert)
    }

    /// Der Stand der Pipeline, den die Arbeitsverteilung eines Pods nennt:
    /// der Abdruck des Podmodells.
    pub fn pipeline(&self) -> Hash {
        Hash::from_bytes(self.podmodell.abdruck)
    }

    /// Der jüngste Anker dieses Netzes für den Zustandsabzug.
    pub fn juengster_anker(&self) -> Option<crate::abzug::Anker> {
        self.abzuege
            .iter()
            .max_by_key(|(h, _)| *h)
            .map(|(hoehe, b)| crate::abzug::Anker { hoehe: *hoehe, blockhash: Hash::from_bytes(*b) })
    }

    /// Das Netz zu einem Namen.
    pub fn aus_name(name: &str) -> Option<&'static Netz> {
        NETZE.iter().find(|n| n.name == name)
    }

    /// **Das Netz eines Kettenzustands**, aus seinem Podmodell.
    ///
    /// ⚑ **Ohne Podmodell die Probekette**: Ein Zustand ohne Podmodell
    /// entsteht nur in Proben, die den Ledger unmittelbar bauen, und dort
    /// gilt der Startwert der Probekette wie vor dem 2026-10-22.
    pub fn des_zustands(zustand: &myl_ledger::state::LedgerState) -> &'static Netz {
        zustand
            .podmodell
            .and_then(|pm| NETZE.iter().find(|n| n.podmodell == pm))
            .unwrap_or(&PROBEKETTE)
    }
}

/// Die Probekette: das Probemodell, nur in den Proben freigegeben.
///
/// Kopf in Platz 20 (`lm_head.bin` 311 MB gegen eine mittlere Ebene von
/// 15,7 MB, aufgerundet) und in Zeit 5 (gemessen am 2026-10-21, P1).
pub const PROBEKETTE: Netz = Netz {
    name: "probe",
    startwert: crate::kette::PROBE_STARTWERT,
    podmodell: Podmodell {
        abdruck: [
            0x92, 0x40, 0xfa, 0x59, 0xdd, 0xb4, 0x60, 0xeb,
            0x1b, 0x2e, 0xfc, 0x77, 0x87, 0x9c, 0xc7, 0x5e,
            0x68, 0x88, 0x2a, 0x5c, 0x43, 0x0b, 0x1c, 0xdc,
            0x02, 0x22, 0xcd, 0x02, 0xd4, 0xba, 0x51, 0xbd,
        ],
        ebenen: 28,
        kopf_platz: 20,
        kopf_zeit: 5,
    },
    profil: myl_tokenomics::vtfe::PROBE_MODELL,
    artefakt: "sparrow-0.6b-int8",
    // Proben bauen ihr Netz selbst; eine lokale Suche liesse Proben auf
    // derselben Maschine einander finden.
    einstieg: &[],
    lokale_suche: false,
    abzuege: &[],
};

/// Der Startwert des Testnetzes. Wie bei der Probekette sagt der Text,
/// was die Kette ist und was nicht.
pub const TESTNETZ_STARTWERT: &[u8] = b"MYELITH-TESTNETZ-RAVEN-35B-A3B-INT4-KEINE-ECHTE-KETTE";

/// ⚑ **Das Testnetz mit dem int4-35B** (Festlegung des Projektinhabers vom
/// 2026-10-22).
///
/// Kopf in Platz 4 (beide Köpfe, 1,5 GB gegen eine mittlere Ebene von
/// 0,45 GB, aufgerundet) und in Zeit 1, gemessen am 2026-10-21 (P1).
pub const TESTNETZ_INT4: Netz = Netz {
    name: "test-int4",
    startwert: TESTNETZ_STARTWERT,
    podmodell: Podmodell {
        abdruck: [
            0x92, 0x35, 0xc8, 0x16, 0x2b, 0x39, 0x1d, 0x6f,
            0x97, 0x6d, 0xe2, 0x4c, 0x53, 0xe1, 0x45, 0xa6,
            0x66, 0x0b, 0xe8, 0x2d, 0x46, 0xb0, 0xdf, 0xf4,
            0x3d, 0x13, 0xac, 0x0b, 0x02, 0x5f, 0x01, 0x1a,
        ],
        ebenen: 40,
        kopf_platz: 4,
        kopf_zeit: 1,
    },
    profil: myl_tokenomics::vtfe::RAVEN_35B_A3B,
    artefakt: "raven-35b-a3b-int4",
    // ⚠️ Noch kein langlebiger Knoten: Bis es einen gibt, ist die Liste
    //   leer, und ausserhalb des lokalen Netzes braucht der Einstieg eine
    //   Bootstrap-Angabe.
    einstieg: &[],
    lokale_suche: true,
    // Noch keiner: Das Testnetz hat noch keine dauerhafte Epochengrenze.
    abzuege: &[],
};

/// Alle Netze, die dieser Knoten kennt.
pub const NETZE: [Netz; 2] = [PROBEKETTE, TESTNETZ_INT4];

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Eintrag eines Artefakts im Katalog, als Text bis zu seinem Ende.
    fn eintrag(name: &str) -> &'static str {
        let katalog = include_str!("../../../MODELS/llm/KATALOG.json");
        let ab = katalog.find(&format!("\"{name}\": {{")).unwrap_or_else(|| panic!("{name} fehlt im Katalog"));
        let rest = &katalog[ab..];
        &rest[..rest.find("\n  }").expect("Eintrag endet nicht")]
    }

    /// ⚑ **Jedes Netz gegen den Katalog**: Abdruck auf der Netzliste (beim
    /// Testnetz) oder im Prüfnetz (bei der Probekette), Ebenenzahl und
    /// Breite des Profils wie dort. Ein neu gebautes Artefakt bekommt
    /// einen neuen Abdruck; dann muss das Netz mit.
    /// ⚑ Ein Anker steht nur auf einer Höhe, die jeder Knoten auf Dauer
    /// behält; auf einer gewöhnlichen könnte ihn zwei Epochen später
    /// niemand mehr liefern.
    #[test]
    fn jeder_anker_steht_auf_einer_dauerhaften_hoehe() {
        for n in NETZE {
            for (h, _) in n.abzuege {
                assert!(crate::abzug::ist_langzeit(*h), "{}: Anker bei {h} wird nicht auf Dauer behalten", n.name);
            }
        }
    }

    #[test]
    fn jedes_netz_stimmt_mit_dem_katalog() {
        for (netz, block) in [(PROBEKETTE, "pruefnetz"), (TESTNETZ_INT4, "netz")] {
            let e = eintrag(netz.artefakt);
            let hex: String = netz.podmodell.abdruck.iter().map(|b| format!("{b:02x}")).collect();
            let block_ab = e.find(&format!("\"{block}\": {{")).unwrap_or_else(|| panic!("{}: {block} fehlt", netz.name));
            assert!(e[block_ab..].contains(&format!("\"abdruck\": \"{hex}\"")), "{}: Abdruck weicht ab", netz.name);
            assert!(e.contains(&format!("\"layer\": {},", netz.podmodell.ebenen)), "{}: Ebenenzahl weicht ab", netz.name);
            assert_eq!(netz.profil.num_layers, netz.podmodell.ebenen as u64, "{}: Profil und Podmodell", netz.name);
            assert!(e.contains(&format!("\"hidden_size\": {},", netz.profil.hidden_size)), "{}: Breite weicht ab", netz.name);
        }
    }

    /// Zwei Netze, zwei Startwerte, zwei Podmodelle; jedes findet sich aus
    /// seinem Zustand wieder.
    #[test]
    fn die_netze_sind_verschieden_und_finden_sich_wieder() {
        assert_ne!(PROBEKETTE.startwert_hash(), TESTNETZ_INT4.startwert_hash());
        assert_ne!(PROBEKETTE.podmodell, TESTNETZ_INT4.podmodell);
        for n in NETZE {
            let mut z = myl_ledger::state::LedgerState::genesis(1);
            z.podmodell = Some(n.podmodell);
            assert_eq!(Netz::des_zustands(&z), &n);
            assert_eq!(Netz::aus_name(n.name), Some(&n));
        }
        assert_eq!(Netz::des_zustands(&myl_ledger::state::LedgerState::genesis(1)), &PROBEKETTE);
        assert!(Netz::aus_name("echt").is_none());
    }
}
