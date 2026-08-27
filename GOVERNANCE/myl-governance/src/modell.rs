//! Modellversionen: Genesis-Manifest und Vorschlagsphase (Punkte 3.4
//! und 3.1, Kap. 10.1 und 10.2).
//!
//! # Die Anforderung ist wörtlich und ungewöhnlich scharf
//!
//! Kap. 10.1 sagt: „Ausgangsgewichte, Quantisierungsverfahren,
//! Kalibrierungsdaten und Werkzeugversionen gehen in das
//! Genesis-Manifest ein, **sodass jeder Teilnehmer die Ableitung
//! nachvollziehen kann**." Das Akzeptanzkriterium der Phase wiederholt
//! es: vollständig genug, dass ein Dritter die Ableitung unabhängig
//! nachvollzieht.
//!
//! Das ist keine Beschreibung, sondern ein **Rezept**. Der Unterschied
//! entscheidet über den Vertrauensbedarf des ganzen Netzes: Ein
//! verteiltes Artefakt muss man glauben, ein Rezept kann man nachkochen
//! und das Ergebnis vergleichen. Kap. 10.1 zieht daraus, dass der
//! Vertrauensbedarf sich auf die **Auswahl** von Basismodell und
//! Kalibrierungsdaten zusammenzieht.
//!
//! # ⚑ Woran ein Rezept still scheitert
//!
//! An einer fehlenden Revision. „Qwen/Qwen2.5-0.5B" ohne festgenagelten
//! Commit heißt „was gerade dort liegt", und das ist morgen etwas
//! anderes. Ein Manifest mit leerem Revisionsfeld sieht vollständig aus,
//! liest sich vollständig und ist es nicht; der Fehler fällt erst auf,
//! wenn jemand Jahre später nachbaut und einen anderen Digest bekommt.
//!
//! Dasselbe gilt für einen fehlenden Artefakt-Digest: Ohne ihn gibt es
//! nichts zu vergleichen, und aus dem Rezept wird wieder eine
//! Behauptung.
//!
//! [`Modellmanifest::pruefe_vollstaendig`] weist beides zurück. Nicht
//! weil ein leeres Feld hässlich wäre, sondern weil ein Manifest mit
//! leerem Feld die einzige Zusage bricht, die es gibt.
//!
//! # Was hier **nicht** steht
//!
//! Die Punkte 3.2 (Shadow-Phase) und 3.3 (koordinierter Rollout). Beide
//! brauchen laufende Pods, die zwei Modellversionen gleichzeitig
//! fahren, und einen Scheduler, der einen Kapazitätsanteil dafür
//! abzweigt. Das ist Arbeit in COMPUTE_PIPELINE und INTEGER_LLM, nicht
//! hier, und sie ist ohne Betrieb nicht messbar. **Als offen geführt,
//! nicht als erledigt behandelt.**

use std::collections::BTreeSet;

use myl_types::hash::Hash;
use myl_types::ids::MerkleRoot;

/// Woher die Ausgangsgewichte stammen (Kap. 10.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gewichtsherkunft {
    /// Bezeichner des Ausgangsmodells, etwa `Qwen/Qwen2.5-0.5B`.
    pub quelle: String,
    /// Die **festgenagelte** Revision, nicht ein Zweigname.
    pub revision: String,
    /// Lizenz des Ausgangsmodells (Kap. 10.1, Kriterium 1).
    pub lizenz: String,
    /// Digest über die Ausgangsgewichte.
    pub gewichte_digest: Hash,
}

/// Wie aus den Ausgangsgewichten das ganzzahlige Artefakt wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ableitung {
    /// Name des Werkzeugs, das die Quantisierung erzeugt.
    pub werkzeug: String,
    /// Seine Version. Ohne sie ist das Rezept nicht reproduzierbar.
    pub werkzeug_version: String,
    /// Digest über die Kalibrierungsdaten.
    pub kalibrierdaten_digest: Hash,
    /// Fassung der Ausführungsspezifikation θ_v.
    pub theta_v: String,
}

/// Ein vollständiges Manifest einer Modellversion.
///
/// Zu Genesis beschreibt es das Startmodell (Punkt 3.4), bei einem
/// Update den Kandidaten (Punkt 3.1). Es ist **dieselbe Struktur**, und
/// das ist Absicht: Ein Update, das weniger nachweisen müsste als
/// Genesis, wäre der bequeme Weg an der Anforderung vorbei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modellmanifest {
    /// Kurzname der Modellversion.
    pub modell: String,
    /// Herkunft der Ausgangsgewichte.
    pub herkunft: Gewichtsherkunft,
    /// Das Rezept.
    pub ableitung: Ableitung,
    /// Digest, den das nachgebaute Artefakt haben **muss**.
    pub artefakt_digest: Hash,
    /// Zugelassene Kernel (Kap. 10.3, `Parameter::KernelWhitelist`).
    pub kernel_whitelist: BTreeSet<Hash>,
}

/// Warum ein Manifest oder ein Modellvorschlag nicht trägt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestFehler {
    /// Ein Feld ist leer, das die Ableitung festnagelt.
    FeldFehlt { feld: &'static str },
    /// Die Kernel-Whitelist ist leer.
    ///
    /// Kap. 10.3 führt sie als Governance-Parameter; leer hieße, dass
    /// jeder Kernel zugelassen ist, und damit wäre die
    /// Determinismus-Pflicht nicht durchsetzbar.
    KeineKernel,
    /// Der nachgebaute Digest weicht ab.
    NachbauWeichtAb { erwartet: Hash, gemessen: Hash },
    /// Der Vorschlag geht von einem anderen Vorgänger aus als dem
    /// geltenden.
    VorgaengerPasstNicht {
        /// Wovon der Vorschlag ausgeht.
        genannt: MerkleRoot,
        /// Was tatsächlich gilt.
        geltend: MerkleRoot,
    },
    /// Der Kandidat ist die geltende Version.
    KandidatIstUnveraendert,
}

impl std::fmt::Display for ManifestFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeldFehlt { feld } => write!(
                f,
                "das Feld {feld} ist leer: ohne es lässt sich die Ableitung nicht \
                 nachvollziehen, und genau das verlangt Kap. 10.1"
            ),
            Self::KeineKernel => write!(
                f,
                "die Kernel-Whitelist ist leer: dann wäre jeder Kernel zugelassen \
                 und die Determinismus-Pflicht nicht durchsetzbar"
            ),
            Self::NachbauWeichtAb { erwartet, gemessen } => write!(
                f,
                "der Nachbau ergab {gemessen}, das Manifest nennt {erwartet}: \
                 das Rezept führt nicht zu dem, was es verspricht"
            ),
            Self::VorgaengerPasstNicht { genannt, geltend } => write!(
                f,
                "der Vorschlag geht von {genannt} aus, es gilt aber {geltend}"
            ),
            Self::KandidatIstUnveraendert => {
                write!(f, "der Kandidat ist die geltende Version")
            }
        }
    }
}

impl std::error::Error for ManifestFehler {}

/// Kennzeichnet die Bytes, über die die Modellwurzel gebildet wird.
pub const DST_MODELLWURZEL: &[u8] = b"MYELITH_MODELLWURZEL_v1";

impl Modellmanifest {
    /// Prüft, ob das Manifest ein Rezept ist und keine Beschreibung.
    ///
    /// Jedes Feld, das die Ableitung festnagelt, muss belegt sein. Ein
    /// leeres Revisionsfeld heißt „was gerade dort liegt", und das ist
    /// morgen etwas anderes.
    pub fn pruefe_vollstaendig(&self) -> Result<(), ManifestFehler> {
        let felder: [(&'static str, &str); 7] = [
            ("modell", &self.modell),
            ("herkunft.quelle", &self.herkunft.quelle),
            ("herkunft.revision", &self.herkunft.revision),
            ("herkunft.lizenz", &self.herkunft.lizenz),
            ("ableitung.werkzeug", &self.ableitung.werkzeug),
            ("ableitung.werkzeug_version", &self.ableitung.werkzeug_version),
            ("ableitung.theta_v", &self.ableitung.theta_v),
        ];
        for (name, wert) in felder {
            if wert.trim().is_empty() {
                return Err(ManifestFehler::FeldFehlt { feld: name });
            }
        }
        if self.kernel_whitelist.is_empty() {
            return Err(ManifestFehler::KeineKernel);
        }
        Ok(())
    }

    /// Die θ_v-Wurzel: der Hash über alles, was die Version ausmacht.
    ///
    /// Sie ist der Bezeichner, den `Segment::model_version` trägt und
    /// gegen den ein Prüfer eine Berechnung einordnet. Gebildet über
    /// **alle** Felder, damit keine zwei verschiedenen Manifeste
    /// dieselbe Wurzel bekommen können.
    pub fn wurzel(&self) -> MerkleRoot {
        let mut daten = Vec::new();
        daten.extend_from_slice(DST_MODELLWURZEL);
        let mut feld = |s: &[u8]| {
            // Längenpräfix vor jedem Feld: Ohne es ergäben ("ab", "c")
            // und ("a", "bc") dieselben Bytes.
            daten.extend_from_slice(&(s.len() as u64).to_le_bytes());
            daten.extend_from_slice(s);
        };
        feld(self.modell.as_bytes());
        feld(self.herkunft.quelle.as_bytes());
        feld(self.herkunft.revision.as_bytes());
        feld(self.herkunft.lizenz.as_bytes());
        feld(self.herkunft.gewichte_digest.as_bytes());
        feld(self.ableitung.werkzeug.as_bytes());
        feld(self.ableitung.werkzeug_version.as_bytes());
        feld(self.ableitung.kalibrierdaten_digest.as_bytes());
        feld(self.ableitung.theta_v.as_bytes());
        feld(self.artefakt_digest.as_bytes());
        for kernel in &self.kernel_whitelist {
            feld(kernel.as_bytes());
        }
        let h = Hash::sha256(&daten);
        let mut roh = [0u8; 32];
        roh.copy_from_slice(h.as_bytes());
        MerkleRoot::new(roh)
    }

    /// Die Probe aufs Exempel: Stimmt der Nachbau?
    ///
    /// **Das ist die Anforderung aus Kap. 10.1, ausgeführt.** Wer das
    /// Rezept befolgt, bekommt einen Digest; stimmt er nicht mit dem
    /// im Manifest überein, war das Manifest kein Rezept, sondern eine
    /// Behauptung, und der Unterschied fällt hier auf statt später.
    pub fn nachvollzogen(&self, gemessener_digest: Hash) -> Result<(), ManifestFehler> {
        if gemessener_digest != self.artefakt_digest {
            return Err(ManifestFehler::NachbauWeichtAb {
                erwartet: self.artefakt_digest,
                gemessen: gemessener_digest,
            });
        }
        Ok(())
    }
}

/// Ein Vorschlag, die geltende Modellversion abzulösen (Punkt 3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modellvorschlag {
    /// Die Version, von der der Vorschlag ausgeht.
    pub vorgaenger: MerkleRoot,
    /// Der Kandidat.
    pub kandidat: Modellmanifest,
}

/// Prüft einen Modellvorschlag gegen die geltende Version.
///
/// # ⚑ Warum der Vorgänger genannt und geprüft wird
///
/// Ohne ihn wäre ein Vorschlag zeitlos: Ein Kandidat, der vor drei
/// Modellwechseln eingereicht wurde, ließe sich später anwenden und
/// setzte alles dazwischen zurück. Der genannte Vorgänger macht aus dem
/// Vorschlag eine **Aussage über einen Zustand**, und ein veralteter
/// Vorschlag fällt mit Begründung durch, statt still zu wirken.
///
/// Dieselbe Bauart wie die Epochenprüfung an anderen Stellen des
/// Projekts: Ein gültig unterschriebener Wert aus der Vergangenheit ist
/// echt und trotzdem falsch.
pub fn pruefe_modellvorschlag(
    geltend: &Modellmanifest,
    vorschlag: &Modellvorschlag,
) -> Result<MerkleRoot, ManifestFehler> {
    let geltende_wurzel = geltend.wurzel();
    if vorschlag.vorgaenger != geltende_wurzel {
        return Err(ManifestFehler::VorgaengerPasstNicht {
            genannt: vorschlag.vorgaenger,
            geltend: geltende_wurzel,
        });
    }
    vorschlag.kandidat.pruefe_vollstaendig()?;
    let neue_wurzel = vorschlag.kandidat.wurzel();
    if neue_wurzel == geltende_wurzel {
        return Err(ManifestFehler::KandidatIstUnveraendert);
    }
    Ok(neue_wurzel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(s: &str) -> Hash {
        Hash::sha256(s.as_bytes())
    }

    /// Ein Manifest mit den Angaben, die für Qwen2.5-0,5B wirklich im
    /// Projekt stehen.
    ///
    /// Die Werte stammen aus `models/KATALOG.json` und
    /// `scale_packs/REGISTER.json`. Sie sind hier nicht abgeschrieben,
    /// um geprüft zu werden, sondern damit der Testfall zeigt, dass die
    /// Struktur die vorhandenen Angaben wirklich aufnimmt: Ein Format,
    /// das die eigenen Artefakte nicht abbildet, ist keins.
    fn qwen_manifest() -> Modellmanifest {
        Modellmanifest {
            modell: "qwen2.5-0.5b".to_string(),
            herkunft: Gewichtsherkunft {
                quelle: "Qwen/Qwen2.5-0.5B".to_string(),
                revision: "060db6499f32faf8b98477b0a26969ef7d8b9987".to_string(),
                lizenz: "Apache-2.0".to_string(),
                gewichte_digest: hash("weights_manifest"),
            },
            ableitung: Ableitung {
                werkzeug: "tools/skalenpaket_bauen.py".to_string(),
                werkzeug_version: "0.17.0".to_string(),
                kalibrierdaten_digest: hash("kalibrierdaten"),
                theta_v: "0.17.0".to_string(),
            },
            artefakt_digest: hash("artefakt"),
            kernel_whitelist: BTreeSet::from([hash("linear_w8a16"), hash("silu_lut")]),
        }
    }

    #[test]
    fn ein_vollstaendiges_manifest_geht_durch() {
        assert!(qwen_manifest().pruefe_vollstaendig().is_ok());
    }

    #[test]
    fn jedes_feld_das_die_ableitung_festnagelt_wird_verlangt() {
        // ⚑ Ein leeres Revisionsfeld heißt „was gerade dort liegt", und
        // das ist morgen etwas anderes. Der Fehler fällt sonst erst
        // auf, wenn jemand Jahre später nachbaut.
        type Leeren = fn(&mut Modellmanifest);
        let leeren: [(&str, Leeren); 7] = [
            ("modell", |m| m.modell.clear()),
            ("herkunft.quelle", |m| m.herkunft.quelle.clear()),
            ("herkunft.revision", |m| m.herkunft.revision.clear()),
            ("herkunft.lizenz", |m| m.herkunft.lizenz.clear()),
            ("ableitung.werkzeug", |m| m.ableitung.werkzeug.clear()),
            ("ableitung.werkzeug_version", |m| {
                m.ableitung.werkzeug_version.clear()
            }),
            ("ableitung.theta_v", |m| m.ableitung.theta_v.clear()),
        ];
        for (name, leeren_fn) in leeren {
            let mut m = qwen_manifest();
            leeren_fn(&mut m);
            assert!(
                matches!(m.pruefe_vollstaendig(), Err(ManifestFehler::FeldFehlt { .. })),
                "ein leeres {name} wurde angenommen"
            );
        }
    }

    #[test]
    fn ein_feld_aus_leerzeichen_gilt_als_leer() {
        // Sonst wäre die Prüfung mit einem Leerschlag zu umgehen, und
        // das sähe an der Aufrufstelle nach einem gefüllten Feld aus.
        let mut m = qwen_manifest();
        m.herkunft.revision = "   ".to_string();
        assert!(matches!(
            m.pruefe_vollstaendig(),
            Err(ManifestFehler::FeldFehlt { .. })
        ));
    }

    #[test]
    fn eine_leere_kernel_whitelist_faellt_auf() {
        let mut m = qwen_manifest();
        m.kernel_whitelist.clear();
        assert!(matches!(
            m.pruefe_vollstaendig(),
            Err(ManifestFehler::KeineKernel)
        ));
    }

    #[test]
    fn der_nachbau_wird_verglichen() {
        // Die Anforderung aus Kap. 10.1, ausgeführt.
        let m = qwen_manifest();
        assert!(m.nachvollzogen(m.artefakt_digest).is_ok());
        assert!(matches!(
            m.nachvollzogen(hash("etwas anderes")),
            Err(ManifestFehler::NachbauWeichtAb { .. })
        ));
    }

    #[test]
    fn die_wurzel_haengt_an_jedem_feld() {
        // Ein Feld, das nicht in die Wurzel eingeht, ließe sich nach
        // der Abstimmung ändern, ohne dass die Version sich ändert.
        let grund = qwen_manifest().wurzel();
        let aenderungen: [fn(&mut Modellmanifest); 10] = [
            |m| m.modell.push('x'),
            |m| m.herkunft.quelle.push('x'),
            |m| m.herkunft.revision.push('x'),
            |m| m.herkunft.lizenz.push('x'),
            |m| m.herkunft.gewichte_digest = hash("anders"),
            |m| m.ableitung.werkzeug.push('x'),
            |m| m.ableitung.werkzeug_version.push('x'),
            |m| m.ableitung.kalibrierdaten_digest = hash("anders"),
            |m| m.ableitung.theta_v.push('x'),
            |m| m.artefakt_digest = hash("anders"),
        ];
        for (i, aendern) in aenderungen.iter().enumerate() {
            let mut m = qwen_manifest();
            aendern(&mut m);
            assert_ne!(m.wurzel(), grund, "Änderung {i} bewegt die Wurzel nicht");
        }
        // Und die Whitelist ebenso.
        let mut m = qwen_manifest();
        m.kernel_whitelist.insert(hash("noch ein kernel"));
        assert_ne!(m.wurzel(), grund, "die Kernel-Whitelist steht nicht in der Wurzel");
    }

    #[test]
    fn die_laengenpraefixe_verhindern_verwechslung() {
        // Ohne sie ergäben ("ab", "c") und ("a", "bc") dieselben Bytes
        // und damit dieselbe Wurzel: zwei verschiedene Modelle mit
        // einem Bezeichner.
        let mut a = qwen_manifest();
        a.modell = "ab".to_string();
        a.herkunft.quelle = "c".to_string();
        let mut b = qwen_manifest();
        b.modell = "a".to_string();
        b.herkunft.quelle = "bc".to_string();
        assert_ne!(a.wurzel(), b.wurzel());
    }

    #[test]
    fn dieselben_angaben_geben_dieselbe_wurzel() {
        assert_eq!(qwen_manifest().wurzel(), qwen_manifest().wurzel());
    }

    #[test]
    fn ein_vorschlag_gegen_einen_veralteten_vorgaenger_faellt_durch() {
        // ⚑ Ohne diese Prüfung wäre ein Vorschlag zeitlos: Ein
        // Kandidat, der vor drei Wechseln eingereicht wurde, setzte
        // alles dazwischen zurück.
        let geltend = qwen_manifest();
        let mut kandidat = qwen_manifest();
        kandidat.ableitung.theta_v = "0.18.0".to_string();

        let veraltet = Modellvorschlag {
            vorgaenger: MerkleRoot::new([9u8; 32]),
            kandidat: kandidat.clone(),
        };
        assert!(matches!(
            pruefe_modellvorschlag(&geltend, &veraltet),
            Err(ManifestFehler::VorgaengerPasstNicht { .. })
        ));

        // Gegenprobe: Mit dem richtigen Vorgänger geht derselbe
        // Kandidat durch. Sonst hieße der Nachweis oben nur, dass gar
        // nichts durchgeht.
        let richtig = Modellvorschlag {
            vorgaenger: geltend.wurzel(),
            kandidat,
        };
        assert_eq!(
            pruefe_modellvorschlag(&geltend, &richtig).expect("prüfen"),
            richtig.kandidat.wurzel()
        );
    }

    #[test]
    fn ein_unveraenderter_kandidat_faellt_durch() {
        let geltend = qwen_manifest();
        let vorschlag = Modellvorschlag {
            vorgaenger: geltend.wurzel(),
            kandidat: qwen_manifest(),
        };
        assert!(matches!(
            pruefe_modellvorschlag(&geltend, &vorschlag),
            Err(ManifestFehler::KandidatIstUnveraendert)
        ));
    }

    #[test]
    fn ein_unvollstaendiger_kandidat_faellt_durch() {
        // Ein Update muss dasselbe nachweisen wie Genesis. Alles andere
        // wäre der bequeme Weg an der Anforderung vorbei.
        let geltend = qwen_manifest();
        let mut kandidat = qwen_manifest();
        kandidat.ableitung.theta_v = "0.18.0".to_string();
        kandidat.herkunft.revision.clear();
        let vorschlag = Modellvorschlag {
            vorgaenger: geltend.wurzel(),
            kandidat,
        };
        assert!(matches!(
            pruefe_modellvorschlag(&geltend, &vorschlag),
            Err(ManifestFehler::FeldFehlt { .. })
        ));
    }

    #[test]
    fn jeder_fehler_sagt_was_geschehen_ist() {
        let faelle = [
            ManifestFehler::FeldFehlt {
                feld: "herkunft.revision",
            },
            ManifestFehler::KeineKernel,
            ManifestFehler::NachbauWeichtAb {
                erwartet: hash("a"),
                gemessen: hash("b"),
            },
            ManifestFehler::VorgaengerPasstNicht {
                genannt: MerkleRoot::new([1u8; 32]),
                geltend: MerkleRoot::new([2u8; 32]),
            },
            ManifestFehler::KandidatIstUnveraendert,
        ];
        for fall in faelle {
            let text = fall.to_string();
            assert!(text.len() > 20, "zu knapp: {text}");
            assert!(!text.ends_with(' '));
        }
    }
}
