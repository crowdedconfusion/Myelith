//! Was ein Gateway bezeugen kann, und was nicht.
//!
//! ## ⚑ Ein Gateway bezeugt eine Beobachtung, niemals eine Wahrheit
//!
//! Kap. 8.1 nennt den Mehrfachabruf durch unabhängige Gateways als
//! **Milderung** für externe Werkzeugergebnisse. Die naheliegende
//! Umsetzung wäre eine Abstimmung: *k* von *n* müssen übereinstimmen.
//! **Sie trägt nicht**, und zwar aus zwei Richtungen:
//!
//! - **Uneinigkeit heißt nicht Bosheit.** Zwei ehrliche Gateways
//!   bekommen verschiedene Antworten bei Geo-Routing, A/B-Tests,
//!   Lastverteilung, oder schlicht weil sich die Welt zwischen zwei
//!   Abrufen geändert hat.
//! - **Einigkeit heißt nicht Wahrheit.** Lügt der Ursprungsserver,
//!   lügen alle Gateways einträchtig und bytegleich.
//!
//! **Die Fälle „böse", „veraltet" und „anders geroutet" sind aus den
//! Attestierungen allein nicht unterscheidbar.** Eine Mehrheitsregel
//! vernichtete deshalb Information und gewänne nichts dafür.
//!
//! ## Was stattdessen geschieht: aufzeichnen statt abstimmen
//!
//! [`beobachte`] löst **nichts** auf. Es liefert drei Zustände, und
//! `Uneinig` behält **alle** Varianten mit ihren Zeugen. Wer daraus
//! etwas macht, ist der Agent oder der Mensch, nicht das Protokoll.
//!
//! Das ist derselbe Grundsatz wie bei
//! [`crate::registratur::Segmentstufe::Unbekannt`] und bei
//! `myl_verifier::Befund::KeinNachweis`: **Ungewissheit wird benannt,
//! nicht versteckt.**
//!
//! ## ⚑ Die Zeitspanne ist das Maß dafür, was Einigkeit wert ist
//!
//! Das Whitepaper sagt, der Mehrfachabruf „versagt bei sich laufend
//! ändernden Daten". Das gehört nicht in eine Fußnote, sondern ins
//! Ergebnis: **Drei übereinstimmende Attestierungen innerhalb von 200
//! Millisekunden bedeuten etwas anderes als dieselben über 30
//! Sekunden.** Die Spanne wird deshalb mitgeliefert.
//!
//! ⚑ **Und sie ist ein Hinweis, kein Beweis.** Die Zeitstempel kommen
//! von den Gateways selbst; wer lügt, kann die Spanne besser aussehen
//! lassen, als sie war. Sie hilft gegen **Trägheit**, nicht gegen
//! **Absicht**, und wer sie liest, soll das wissen.
//!
//! ## Wie viele Zeugen es braucht, steht nicht hier
//!
//! **Es gibt keine Konstante dafür**, und das ist Absicht. Der Aufrufer
//! verlangt eine Zahl, und der Session-Kontrakt kann ein Minimum
//! erzwingen, gekoppelt an den Betrag (Kap. 8.2). Wie viele Zeugen es
//! braucht, skaliert damit mit dem, was auf dem Spiel steht, und nicht
//! mit einer Zahl, die jemand einmal geraten hat.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

/// Domain-Trenner der Attestierung.
pub const DST_ATTESTIERUNG: &[u8] = b"MYELITH_ATTESTIERUNG_v1";

/// Die Bytes, über die ein Gateway unterschreibt.
///
/// ⚑ **Die Anfrage steht mit darin, und das ist kein Beiwerk.** Ohne
/// sie bezeugte eine Attestierung nur „ich habe irgendwann diese Bytes
/// gesehen", und dieselbe Unterschrift ließe sich für eine **andere**
/// Anfrage vorlegen. Mit ihr bezeugt sie „auf **diese** Frage kam
/// **diese** Antwort".
pub fn attestierungsbytes(
    gateway: &MinerId,
    anfrage: &Hash,
    zeitpunkt_ms: u64,
    inhalt: &Hash,
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DST_ATTESTIERUNG.len() + 32 + 32 + 8 + 32);
    msg.extend_from_slice(DST_ATTESTIERUNG);
    msg.extend_from_slice(gateway.as_bytes());
    msg.extend_from_slice(anfrage.as_bytes());
    msg.extend_from_slice(&zeitpunkt_ms.to_le_bytes());
    msg.extend_from_slice(inhalt.as_bytes());
    msg
}

/// Die Aussage eines Gateways über einen externen Abruf.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Attestierung {
    /// Wer bezeugt.
    pub gateway: MinerId,
    /// Wonach gefragt wurde (Hash über die Anfrage).
    pub anfrage: Hash,
    /// Wann, nach der Uhr des Gateways.
    pub zeitpunkt_ms: u64,
    /// Was zurückkam (Hash über die Antwortbytes).
    pub inhalt: Hash,
    /// Unterschrift über [`attestierungsbytes`].
    pub signatur: BlsSignature,
}

/// Eine Attestierung, deren Unterschrift geprüft wurde.
///
/// ⚑ **Es gibt keinen anderen Weg hierher als [`Attestierung::pruefe`].**
/// Das ist Absicht: Eine ungeprüfte Attestierung sieht aus wie eine
/// geprüfte, und ein Aufrufer, der es vergisst, merkt nichts. Der Typ
/// erinnert ihn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GepruefteAttestierung(Attestierung);

impl GepruefteAttestierung {
    /// Die zugrunde liegende Aussage.
    pub fn aussage(&self) -> &Attestierung {
        &self.0
    }
}

/// Warum eine Beobachtung nicht zustande kam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeobachtungsFehler {
    /// Die Unterschrift stimmt nicht.
    SignaturStimmtNicht { gateway: MinerId },
    /// Der öffentliche Schlüssel ist kein gültiger Gruppenpunkt.
    SchluesselUngueltig { gateway: MinerId },
    /// Eine Attestierung gehört zu einer anderen Anfrage.
    AndereAnfrage { gateway: MinerId },
    /// ⚑ Dasselbe Gateway zweimal. Zwei Aussagen desselben Zeugen sind
    /// **eine** Aussage, und wer sie doppelt vorlegt, bläht die
    /// Zeugenzahl auf.
    ZeugeDoppelt { gateway: MinerId },
    /// ⚑ Gar keine Aussage.
    ///
    /// **Aus nichts lässt sich weder Einigkeit noch Uneinigkeit
    /// ablesen.** Bis zum 2026-08-28 lieferte eine leere Liste
    /// `Uneinig` mit null Varianten, also die Meldung „die Zeugen sahen
    /// Verschiedenes" über Zeugen, die es nicht gab. Wer sie las,
    /// erfuhr das Gegenteil von dem, was der Fall war.
    ///
    /// Ein Aufrufer, der ohne Bezeugung auskommen darf, fragt hier
    /// nicht an, sondern weiß es vorher.
    KeineAussagen,
}

impl std::fmt::Display for BeobachtungsFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignaturStimmtNicht { gateway } => {
                write!(f, "Unterschrift von {:?} stimmt nicht", gateway)
            }
            Self::SchluesselUngueltig { gateway } => {
                write!(f, "Schlüssel von {:?} ist kein gültiger Gruppenpunkt", gateway)
            }
            Self::AndereAnfrage { gateway } => write!(
                f,
                "{:?} bezeugt eine andere Anfrage; eine Attestierung gilt nur für die Frage, die in ihr steht",
                gateway
            ),
            Self::KeineAussagen => write!(
                f,
                "keine Aussagen: aus nichts folgt weder Einigkeit noch Uneinigkeit"
            ),
            Self::ZeugeDoppelt { gateway } => write!(
                f,
                "{:?} tritt zweimal auf; zwei Aussagen desselben Zeugen sind eine",
                gateway
            ),
        }
    }
}

impl std::error::Error for BeobachtungsFehler {}

impl Attestierung {
    /// Prüft die Unterschrift gegen den Schlüssel des Gateways.
    pub fn pruefe(self, pubkey: &BlsPublicKey) -> Result<GepruefteAttestierung, BeobachtungsFehler> {
        if pubkey.validate().is_err() {
            return Err(BeobachtungsFehler::SchluesselUngueltig {
                gateway: self.gateway,
            });
        }
        let bytes = attestierungsbytes(
            &self.gateway,
            &self.anfrage,
            self.zeitpunkt_ms,
            &self.inhalt,
        );
        if !pubkey.verify(&bytes, &self.signatur) {
            return Err(BeobachtungsFehler::SignaturStimmtNicht {
                gateway: self.gateway,
            });
        }
        Ok(GepruefteAttestierung(self))
    }
}

/// Was mehrere Gateways zusammen ergeben.
///
/// **Drei Zustände, und keiner davon ist eine Entscheidung.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Beobachtung {
    /// Alle Zeugen sahen dasselbe.
    Einig {
        /// Der übereinstimmende Inhalt.
        inhalt: Hash,
        /// Wer bezeugt hat, sortiert.
        zeugen: Vec<MinerId>,
        /// Zeitspanne zwischen der frühesten und der spätesten Aussage.
        spanne_ms: u64,
    },
    /// Die Zeugen sahen Verschiedenes. ⚑ **Alle Varianten bleiben
    /// erhalten**, und das Protokoll löst nicht auf.
    Uneinig {
        /// Je Inhalt die Zeugen, die ihn sahen. Nach Inhalt sortiert.
        varianten: Vec<(Hash, Vec<MinerId>)>,
        /// Zeitspanne über alle Aussagen.
        spanne_ms: u64,
    },
    /// Weniger Zeugen als verlangt.
    Zuwenig { hatte: usize, verlangt: usize },
}

impl Beobachtung {
    /// Kurzform für Protokoll und Oberfläche.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Einig { .. } => "einig",
            Self::Uneinig { .. } => "uneinig",
            Self::Zuwenig { .. } => "zuwenig",
        }
    }

    /// Wie viele Zeugen ausgesagt haben.
    pub fn zeugenzahl(&self) -> usize {
        match self {
            Self::Einig { zeugen, .. } => zeugen.len(),
            Self::Uneinig { varianten, .. } => varianten.iter().map(|(_, z)| z.len()).sum(),
            Self::Zuwenig { hatte, .. } => *hatte,
        }
    }
}

/// Fasst geprüfte Attestierungen zu einer Beobachtung zusammen.
///
/// ⚑ **Diese Funktion entscheidet nichts.** Sie zählt, gruppiert und
/// misst die Zeitspanne. Bei Uneinigkeit gibt es kein „nimm die
/// häufigste": Bei volatilen Daten ist die Mehrheit bedeutungslos, und
/// bei zwei gegen eins kann die Minderheit die ehrliche sein.
///
/// **Abgewiesen wird nur, was formal nicht zusammengehört:** eine
/// Attestierung für eine andere Anfrage, derselbe Zeuge zweimal, und
/// ⚑ **die leere Liste**: Aus nichts folgt weder Einigkeit noch
/// Uneinigkeit, und wer nichts bezeugt haben will, fragt hier nicht an.
pub fn beobachte(
    anfrage: &Hash,
    attestierungen: &[GepruefteAttestierung],
    verlangt: usize,
) -> Result<Beobachtung, BeobachtungsFehler> {
    use std::collections::{BTreeMap, BTreeSet};

    if attestierungen.is_empty() {
        return Err(BeobachtungsFehler::KeineAussagen);
    }

    let mut gesehen: BTreeSet<MinerId> = BTreeSet::new();
    for a in attestierungen {
        let s = a.aussage();
        if &s.anfrage != anfrage {
            return Err(BeobachtungsFehler::AndereAnfrage { gateway: s.gateway });
        }
        if !gesehen.insert(s.gateway) {
            return Err(BeobachtungsFehler::ZeugeDoppelt { gateway: s.gateway });
        }
    }

    if attestierungen.len() < verlangt {
        return Ok(Beobachtung::Zuwenig {
            hatte: attestierungen.len(),
            verlangt,
        });
    }

    // Zeitspanne über alle Aussagen. Bei einer einzigen ist sie null.
    let zeiten: Vec<u64> = attestierungen.iter().map(|a| a.aussage().zeitpunkt_ms).collect();
    let spanne_ms = match (zeiten.iter().min(), zeiten.iter().max()) {
        (Some(a), Some(b)) => b - a,
        _ => 0,
    };

    // Nach Inhalt gruppieren. `BTreeMap`, damit die Ausgabe
    // deterministisch ist: Sie wandert in die Spur, und zwei ehrliche
    // Knoten müssen dieselbe schreiben.
    let mut nach_inhalt: BTreeMap<Hash, Vec<MinerId>> = BTreeMap::new();
    for a in attestierungen {
        let s = a.aussage();
        nach_inhalt.entry(s.inhalt).or_default().push(s.gateway);
    }
    for zeugen in nach_inhalt.values_mut() {
        zeugen.sort();
    }

    if nach_inhalt.len() == 1 {
        let (inhalt, zeugen) = nach_inhalt.into_iter().next().expect("genau einer");
        return Ok(Beobachtung::Einig {
            inhalt,
            zeugen,
            spanne_ms,
        });
    }

    Ok(Beobachtung::Uneinig {
        varianten: nach_inhalt.into_iter().collect(),
        spanne_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSecretKey;

    fn schluessel(n: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[n; 32]).expect("Schlüsselerzeugung")
    }

    fn gateway(n: u8) -> MinerId {
        MinerId::new([n; 32])
    }

    fn attestiere(n: u8, anfrage: &Hash, zeit: u64, inhalt: &Hash) -> (Attestierung, BlsPublicKey) {
        let sk = schluessel(n);
        let pk = sk.public_key().expect("Schlüssel");
        let bytes = attestierungsbytes(&gateway(n), anfrage, zeit, inhalt);
        let signatur = sk.sign(&bytes).expect("signieren");
        (
            Attestierung {
                gateway: gateway(n),
                anfrage: *anfrage,
                zeitpunkt_ms: zeit,
                inhalt: *inhalt,
                signatur,
            },
            pk,
        )
    }

    fn geprueft(n: u8, anfrage: &Hash, zeit: u64, inhalt: &Hash) -> GepruefteAttestierung {
        let (a, pk) = attestiere(n, anfrage, zeit, inhalt);
        a.pruefe(&pk).expect("prüfen")
    }

    /// Einigkeit wird gemeldet, mit Zeugen und Spanne.
    #[test]
    fn drei_gleiche_aussagen_sind_einig() {
        let anfrage = Hash::sha256(b"wie spaet ist es in Oslo");
        let inhalt = Hash::sha256(b"14:03");
        let a = vec![
            geprueft(1, &anfrage, 1_000, &inhalt),
            geprueft(2, &anfrage, 1_120, &inhalt),
            geprueft(3, &anfrage, 1_200, &inhalt),
        ];
        match beobachte(&anfrage, &a, 3).expect("beobachten") {
            Beobachtung::Einig {
                inhalt: i,
                zeugen,
                spanne_ms,
            } => {
                assert_eq!(i, inhalt);
                assert_eq!(zeugen, vec![gateway(1), gateway(2), gateway(3)]);
                assert_eq!(spanne_ms, 200, "die Spanne ist Ende minus Anfang");
            }
            anderes => panic!("erwartet einig, bekommen {anderes:?}"),
        }
    }

    /// ⚑ **Uneinigkeit wird nicht aufgelöst.**
    ///
    /// Zwei gegen eins ergibt **keine** Mehrheitsentscheidung. Beide
    /// Varianten bleiben mit ihren Zeugen erhalten, denn bei volatilen
    /// Daten ist die Mehrheit bedeutungslos und bei zwei gegen eins kann
    /// die Minderheit die ehrliche sein.
    #[test]
    fn zwei_gegen_eins_wird_nicht_zur_mehrheit() {
        let anfrage = Hash::sha256(b"kurs");
        let (viele, wenige) = (Hash::sha256(b"1,08"), Hash::sha256(b"1,09"));
        let a = vec![
            geprueft(1, &anfrage, 1_000, &viele),
            geprueft(2, &anfrage, 1_050, &viele),
            geprueft(3, &anfrage, 1_900, &wenige),
        ];
        match beobachte(&anfrage, &a, 3).expect("beobachten") {
            Beobachtung::Uneinig {
                varianten,
                spanne_ms,
            } => {
                assert_eq!(varianten.len(), 2, "eine Variante ist verschwunden");
                let gesamt: usize = varianten.iter().map(|(_, z)| z.len()).sum();
                assert_eq!(gesamt, 3, "ein Zeuge ist verschwunden");
                assert_eq!(spanne_ms, 900);
            }
            anderes => panic!("erwartet uneinig, bekommen {anderes:?}"),
        }
    }

    /// Zu wenige Zeugen ist ein eigener Zustand und keine Uneinigkeit.
    #[test]
    fn zu_wenige_zeugen_sind_kein_ergebnis() {
        let anfrage = Hash::sha256(b"x");
        let inhalt = Hash::sha256(b"y");
        let a = vec![geprueft(1, &anfrage, 0, &inhalt)];
        assert_eq!(
            beobachte(&anfrage, &a, 3).expect("beobachten"),
            Beobachtung::Zuwenig {
                hatte: 1,
                verlangt: 3
            }
        );
        // Gegenprobe: mit der passenden Zahl geht dieselbe Aussage durch.
        assert!(matches!(
            beobachte(&anfrage, &a, 1).expect("beobachten"),
            Beobachtung::Einig { .. }
        ));
    }

    /// ⚑ **Die Anfrage steht in der Unterschrift, sonst wäre sie
    /// wiederverwendbar.**
    ///
    /// Eine Attestierung für Frage A gilt nicht für Frage B, auch wenn
    /// die Unterschrift echt ist. Ohne die Bindung ließe sich eine alte
    /// Aussage für eine neue Frage vorlegen.
    #[test]
    fn eine_attestierung_gilt_nur_fuer_ihre_anfrage() {
        let a_frage = Hash::sha256(b"Frage A");
        let b_frage = Hash::sha256(b"Frage B");
        let inhalt = Hash::sha256(b"Antwort");
        let alt = vec![geprueft(1, &a_frage, 0, &inhalt)];
        assert!(matches!(
            beobachte(&b_frage, &alt, 1),
            Err(BeobachtungsFehler::AndereAnfrage { .. })
        ));
        // Gegenprobe: für ihre eigene Frage gilt sie.
        assert!(beobachte(&a_frage, &alt, 1).is_ok());
    }

    /// ⚑ **Derselbe Zeuge zweimal ist ein Zeuge.**
    ///
    /// Wer dieselbe Aussage doppelt vorlegt, bläht die Zeugenzahl auf
    /// und unterläuft damit genau die Zahl, die der Session-Kontrakt
    /// verlangt hat.
    #[test]
    fn derselbe_zeuge_zweimal_wird_abgewiesen() {
        let anfrage = Hash::sha256(b"x");
        let inhalt = Hash::sha256(b"y");
        let a = vec![
            geprueft(1, &anfrage, 0, &inhalt),
            geprueft(1, &anfrage, 500, &inhalt),
        ];
        assert!(matches!(
            beobachte(&anfrage, &a, 2),
            Err(BeobachtungsFehler::ZeugeDoppelt { .. })
        ));
    }

    /// Eine gefälschte Unterschrift kommt nicht durch die Prüfung, und
    /// ohne Prüfung gibt es keinen Weg in `beobachte`.
    #[test]
    fn eine_gefaelschte_unterschrift_wird_abgewiesen() {
        let anfrage = Hash::sha256(b"x");
        let inhalt = Hash::sha256(b"y");
        let (mut a, pk) = attestiere(1, &anfrage, 0, &inhalt);
        // Der Inhalt wird nach dem Unterschreiben verändert.
        a.inhalt = Hash::sha256(b"etwas anderes");
        assert!(matches!(
            a.clone().pruefe(&pk),
            Err(BeobachtungsFehler::SignaturStimmtNicht { .. })
        ));
        // Und mit einem fremden Schlüssel ebenso wenig.
        let fremd = schluessel(9).public_key().expect("Schlüssel");
        let (echt, _) = attestiere(1, &anfrage, 0, &inhalt);
        assert!(matches!(
            echt.pruefe(&fremd),
            Err(BeobachtungsFehler::SignaturStimmtNicht { .. })
        ));
    }

    /// ⚑ Die Ausgabe hängt nicht an der Reihenfolge der Eingabe.
    ///
    /// Sie wandert in die Spur, und zwei ehrliche Knoten müssen dieselbe
    /// schreiben.
    #[test]
    fn die_ausgabe_haengt_nicht_an_der_reihenfolge() {
        let anfrage = Hash::sha256(b"x");
        let (p, q) = (Hash::sha256(b"p"), Hash::sha256(b"q"));
        let vorwaerts = vec![
            geprueft(1, &anfrage, 10, &p),
            geprueft(2, &anfrage, 20, &q),
            geprueft(3, &anfrage, 30, &p),
        ];
        let rueckwaerts = vec![
            geprueft(3, &anfrage, 30, &p),
            geprueft(2, &anfrage, 20, &q),
            geprueft(1, &anfrage, 10, &p),
        ];
        assert_eq!(
            beobachte(&anfrage, &vorwaerts, 3).expect("a"),
            beobachte(&anfrage, &rueckwaerts, 3).expect("b")
        );
    }

    /// Ein einzelner Zeuge hat die Spanne null, und das ist richtig: Es
    /// gibt nichts zu spannen.
    #[test]
    fn ein_einzelner_zeuge_hat_die_spanne_null() {
        let anfrage = Hash::sha256(b"x");
        let inhalt = Hash::sha256(b"y");
        let a = vec![geprueft(1, &anfrage, 12_345, &inhalt)];
        match beobachte(&anfrage, &a, 1).expect("beobachten") {
            Beobachtung::Einig { spanne_ms, .. } => assert_eq!(spanne_ms, 0),
            anderes => panic!("erwartet einig, bekommen {anderes:?}"),
        }
    }

    /// ⚑ Gegenprobe zu einem Fund vom 2026-08-28: Eine leere Liste
    /// meldete `Uneinig` mit null Varianten, also Uneinigkeit unter
    /// Zeugen, die es nicht gab. Jetzt ist sie ein Fehler, und der Test
    /// hält fest, dass sie nicht heimlich zu einer Aussage wird.
    #[test]
    fn aus_keiner_aussage_folgt_keine_beobachtung() {
        let anfrage = Hash::sha256(b"frage");
        assert_eq!(beobachte(&anfrage, &[], 0), Err(BeobachtungsFehler::KeineAussagen));
        assert_eq!(beobachte(&anfrage, &[], 3), Err(BeobachtungsFehler::KeineAussagen));

        // Und eine einzige Aussage ist sehr wohl eine Beobachtung.
        let inhalt = Hash::sha256(b"antwort");
        assert!(matches!(
            beobachte(&anfrage, &[geprueft(1, &anfrage, 0, &inhalt)], 1),
            Ok(Beobachtung::Einig { .. })
        ));
    }
}
