//! Sprache und Tastatur, gewaehlt am Anfang des Assistenten.
//!
//! ⚑ **Zwei Sprachen, wie der Client.** Myelith kennt `de` und `en`
//! (`oberflaeche.sprache`); der Assistent gibt seine Wahl mit
//! `myl setzen` dorthin weiter, damit Assistent und Konsole dieselbe
//! Sprache sprechen.
//!
//! ⚑ **Die Tastatur ist eine Belegung aus `kbd`** und wird mit
//! `loadkeys` geladen, sofort und bei jedem Start (`S06myelith`). Sie
//! gilt fuer Bildschirm und Tastatur am Rechner; wer ueber eine serielle
//! Leitung arbeitet, tippt mit der Belegung seines eigenen Rechners.

use std::fs;
use std::path::Path;

/// Wo die Wahl liegt, eine Zeile je Datei, damit auch ein Startskript
/// sie ohne Zerleger lesen kann.
pub const ORDNER: &str = "/daten/golemos";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sprache {
    #[default]
    De,
    En,
}

impl Sprache {
    /// Die Kennung, wie `myl setzen oberflaeche.sprache` sie erwartet.
    pub fn kennung(self) -> &'static str {
        match self {
            Sprache::De => "de",
            Sprache::En => "en",
        }
    }

    pub fn aus(text: &str) -> Option<Sprache> {
        match text.trim() {
            "de" => Some(Sprache::De),
            "en" => Some(Sprache::En),
            _ => None,
        }
    }

    /// Waehlt zwischen dem deutschen und dem englischen Text.
    pub fn t<'a>(self, de: &'a str, en: &'a str) -> &'a str {
        match self {
            Sprache::De => de,
            Sprache::En => en,
        }
    }
}

/// Eine Tastaturbelegung: ihr Name fuer Menschen und fuer `loadkeys`.
pub struct Belegung {
    pub name: &'static str,
    pub karte: &'static str,
}

/// Die angebotenen Belegungen.
///
/// ⚑ **Die Namen in ihrer eigenen Sprache**, wie beim Client: Wer die
/// Oberflaeche nicht versteht, findet seine Tastatur trotzdem.
/// ⚠️ Die Kartennamen muessen im Abbild liegen; `bauen.sh` prueft das.
pub const BELEGUNGEN: [Belegung; 10] = [
    Belegung { name: "Deutsch", karte: "de-latin1-nodeadkeys" },
    Belegung { name: "Deutsch (Schweiz)", karte: "de_CH-latin1" },
    Belegung { name: "English (US)", karte: "us" },
    Belegung { name: "English (UK)", karte: "uk" },
    Belegung { name: "Français", karte: "fr-latin9" },
    Belegung { name: "Español", karte: "es" },
    Belegung { name: "Italiano", karte: "it" },
    Belegung { name: "Nederlands", karte: "nl" },
    Belegung { name: "Português", karte: "pt-latin1" },
    Belegung { name: "Polski", karte: "pl2" },
];

/// Liest eine gespeicherte Wahl (`sprache` oder `tastatur`) unter `ordner`.
pub fn lesen(ordner: &Path, was: &str) -> Option<String> {
    let t = fs::read_to_string(ordner.join(was)).ok()?;
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

pub fn schreiben(ordner: &Path, was: &str, wert: &str) -> std::io::Result<()> {
    fs::create_dir_all(ordner)?;
    fs::write(ordner.join(was), format!("{wert}\n"))
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::klon::proben::ordner;

    #[test]
    fn wahl_hin_und_zurueck() {
        let o = ordner("sprache");
        assert_eq!(lesen(&o, "sprache"), None);
        schreiben(&o, "sprache", "en").unwrap();
        assert_eq!(lesen(&o, "sprache").as_deref().and_then(Sprache::aus), Some(Sprache::En));
    }

    #[test]
    fn kennung_wie_im_client() {
        // Der Client nimmt genau `de` und `en` (`Sprache::aus` dort).
        let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../CLIENT/myl-client/src/einstellungen.rs");
        let text = fs::read_to_string(pfad).expect("einstellungen.rs");
        assert!(text.contains("moeglich sind de, en"), "der Client kennt andere Sprachen");
        assert_eq!(Sprache::De.kennung(), "de");
        assert_eq!(Sprache::En.kennung(), "en");
    }

    /// Jede angebotene Karte steht in der Pruefliste von `bauen.sh`;
    /// sonst boete der Assistent eine Belegung an, die im Abbild fehlt.
    #[test]
    fn jede_karte_wird_beim_bauen_geprueft() {
        let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../bauen.sh");
        let text = fs::read_to_string(pfad).expect("bauen.sh");
        for b in BELEGUNGEN {
            assert!(text.contains(&format!(" {}", b.karte)), "{} fehlt in bauen.sh", b.karte);
        }
    }
}
