//! Der Konsensschlüssel eines Knotens, getrennt vom Netzschlüssel.
//!
//! # Zwei Schlüssel, und warum nicht einer
//!
//! Ein Knoten führt zwei Geheimnisse: die **Netzidentität**
//! (`myl_net::NodeIdentity`, ed25519, bestimmt die PeerId) und den
//! **Konsensschlüssel** (BLS12-381, bestimmt die Stimme). Beide aus
//! einer gemeinsamen Saat abzuleiten wäre ein Geheimnis weniger zu
//! verwalten, war aber am 2026-08-25 ausdrücklich nicht die
//! Entscheidung: **Ein Leck kompromittierte dann beide Ebenen
//! gleichzeitig**, und die Trennung von „wer ist das im Netz" und „wessen
//! Stimme ist das im Konsens" wäre aufgegeben.
//!
//! Die Trennung hat einen zweiten Nutzen: Die Netzidentität darf
//! wechseln, ohne dass die Stimme wechselt. Ein Knoten, der umzieht,
//! bleibt derselbe Validator.
//!
//! # ⚑ Die Herkunft ist die Trennlinie, nicht der Code
//!
//! [`crate::validatorsatz`] hält denselben Gedanken für die
//! Probeschlüssel fest: Dieselbe Prüffunktion arbeitet gegen echte und
//! gegen abgeleitete Schlüssel, der Unterschied liegt allein in der
//! **Herkunft**. Deshalb trägt [`Konsensschluessel`] seine Herkunft mit
//! sich und der Knoten schreibt sie in jede Startzeile des
//! Betriebsprotokolls.
//!
//! **Das ist die Antwort auf einen benannten Einwand.** Ein Schalter,
//! den man später einbaut, ist ein Schalter, den jemand vergisst. Hier
//! gibt es keinen Vorgabewert: [`Konsensschluessel::aus_datei`] und
//! [`Konsensschluessel::probe`] sind zwei verschiedene Aufrufe, und wer
//! den zweiten nimmt, bekommt [`Herkunft::Probelauf`] und damit eine
//! Protokollzeile, die es nennt.
//!
//! # Was in der Datei steht
//!
//! Das **Schlüsselmaterial** (IKM, 32 Bytes hex), nicht der Skalar.
//! `myl_types::BlsSecretKey` gibt seine Bytes bewusst nicht heraus und
//! entsteht nur über `key_gen(ikm)`; das ist deterministisch, also ist
//! die Saat der Schlüssel.
//!
//! Die Datei wird mit Rechten `0600` geschrieben, und
//! [`Konsensschluessel::aus_datei`] **weigert sich**, eine Datei zu
//! lesen, die für Gruppe oder Welt zugänglich ist. Ein geheimer
//! Schlüssel, den das halbe System lesen kann, ist kein Geheimnis, und
//! ein stiller Start mit einem solchen Schlüssel wäre die Art Fehler,
//! die erst im Nachhinein auffällt.

use std::fs;
use std::path::{Path, PathBuf};

use myl_types::bls::{BlsProofOfPossession, BlsPublicKey, BlsSecretKey, BlsSignature};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

/// Länge des Schlüsselmaterials in Bytes.
///
/// 32, weil `draft-irtf-cfrg-bls-signature` §2.3 für `KeyGen`
/// mindestens 32 Bytes IKM verlangt.
pub const IKM_BYTES: usize = 32;

/// Kopfzeile der Schlüsseldatei. Steht im Klartext, damit jemand, der
/// die Datei findet, sofort weiß, was er gefunden hat.
pub const DATEIKOPF: &str = "# Myelith-Konsensschluessel (BLS12-381). GEHEIM. Nicht weitergeben.";

/// Woher der Schlüssel stammt.
///
/// Wird in jede Startzeile des Betriebsprotokolls geschrieben. Siehe
/// Modulkopf: Die Herkunft ist die Trennlinie, nicht der Prüfcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herkunft {
    /// Aus einer Schlüsseldatei. Der einzige Fall, der für ein echtes
    /// Netz taugt.
    Datei,
    /// Frisch erzeugt und geschrieben, weil die Datei noch nicht
    /// existierte.
    NeuErzeugt,
    /// Aus einem Teilnehmernamen abgeleitet.
    ///
    /// **Wer den Namen kennt, kennt den Schlüssel.** Für einen Probelauf
    /// gewollt, für ein echtes Netz unbrauchbar.
    Probelauf,
}

impl Herkunft {
    pub fn als_text(&self) -> &'static str {
        match self {
            Self::Datei => "datei",
            Self::NeuErzeugt => "neu-erzeugt",
            Self::Probelauf => "probelauf",
        }
    }

    /// Taugt diese Herkunft für ein Netz, in dem jemand angreift?
    pub fn ist_geheim(&self) -> bool {
        matches!(self, Self::Datei | Self::NeuErzeugt)
    }
}

/// Was beim Umgang mit der Schlüsseldatei schiefgehen kann.
#[derive(Debug)]
pub enum SchluesselFehler {
    /// Die Datei ließ sich nicht lesen oder schreiben.
    Datei { pfad: PathBuf, grund: String },
    /// Der Inhalt ist kein Hexwort der erwarteten Länge.
    KeinSchluessel { pfad: PathBuf },
    /// Die Datei ist für Gruppe oder Welt zugänglich.
    ZuOffen { pfad: PathBuf, modus: u32 },
    /// Die BLS-Bibliothek lehnte das Material ab.
    Bls(&'static str),
    /// Es stand keine Entropie zur Verfügung.
    KeineEntropie(String),
}

impl std::fmt::Display for SchluesselFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Datei { pfad, grund } => {
                write!(f, "Schlüsseldatei {}: {grund}", pfad.display())
            }
            Self::KeinSchluessel { pfad } => write!(
                f,
                "Schlüsseldatei {} enthält kein Hexwort aus {} Zeichen",
                pfad.display(),
                IKM_BYTES * 2
            ),
            Self::ZuOffen { pfad, modus } => write!(
                f,
                "Schlüsseldatei {} hat Rechte {modus:o} und ist damit für Gruppe \
                 oder Welt lesbar. Ein geheimer Schlüssel, den andere lesen können, \
                 ist kein Geheimnis. Behebung: chmod 600 {}",
                pfad.display(),
                pfad.display()
            ),
            Self::Bls(w) => write!(f, "BLS lehnte das Schlüsselmaterial ab: {w}"),
            Self::KeineEntropie(e) => write!(f, "Keine Entropie für einen neuen Schlüssel: {e}"),
        }
    }
}

impl std::error::Error for SchluesselFehler {}

/// Der Konsensschlüssel eines Knotens.
///
/// Gibt den geheimen Teil nicht heraus. Wer signieren will, ruft
/// [`Konsensschluessel::signiere`].
#[derive(Clone)]
pub struct Konsensschluessel {
    geheim: BlsSecretKey,
    oeffentlich: BlsPublicKey,
    herkunft: Herkunft,
}

impl std::fmt::Debug for Konsensschluessel {
    /// Zeigt **nie** den geheimen Teil.
    ///
    /// Ein abgeleitetes `Debug` schriebe ihn in jede Fehlermeldung und
    /// jede Protokollzeile, die eine Struktur mit diesem Feld ausgibt.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Konsensschluessel")
            .field("kennung", &self.kennung())
            .field("herkunft", &self.herkunft)
            .finish_non_exhaustive()
    }
}

impl Konsensschluessel {
    fn aus_ikm(ikm: &[u8], herkunft: Herkunft) -> Result<Self, SchluesselFehler> {
        let geheim = BlsSecretKey::key_gen(ikm).map_err(|_| SchluesselFehler::Bls("key_gen"))?;
        let oeffentlich = geheim
            .public_key()
            .map_err(|_| SchluesselFehler::Bls("public_key"))?;
        Ok(Self {
            geheim,
            oeffentlich,
            herkunft,
        })
    }

    /// Liest den Schlüssel aus einer Datei, oder legt sie an.
    ///
    /// Prüft die Dateirechte, siehe Modulkopf.
    pub fn aus_datei(pfad: &Path) -> Result<Self, SchluesselFehler> {
        if !pfad.exists() {
            return Self::neu_erzeugen(pfad);
        }
        pruefe_rechte(pfad)?;
        let inhalt = fs::read_to_string(pfad).map_err(|e| SchluesselFehler::Datei {
            pfad: pfad.to_path_buf(),
            grund: e.to_string(),
        })?;
        let wort = inhalt
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .find(|l| !l.is_empty())
            .unwrap_or("");
        let ikm = hex_ikm(wort).ok_or_else(|| SchluesselFehler::KeinSchluessel {
            pfad: pfad.to_path_buf(),
        })?;
        Self::aus_ikm(&ikm, Herkunft::Datei)
    }

    /// Erzeugt einen neuen Schlüssel und schreibt ihn mit Rechten `0600`.
    pub fn neu_erzeugen(pfad: &Path) -> Result<Self, SchluesselFehler> {
        let mut ikm = [0u8; IKM_BYTES];
        getrandom::fill(&mut ikm)
            .map_err(|e| SchluesselFehler::KeineEntropie(e.to_string()))?;

        if let Some(ordner) = pfad.parent() {
            if !ordner.as_os_str().is_empty() {
                fs::create_dir_all(ordner).map_err(|e| SchluesselFehler::Datei {
                    pfad: ordner.to_path_buf(),
                    grund: e.to_string(),
                })?;
            }
        }
        let mut text = String::from(DATEIKOPF);
        text.push('\n');
        for b in ikm.iter() {
            text.push_str(&format!("{b:02x}"));
        }
        text.push('\n');
        fs::write(pfad, &text).map_err(|e| SchluesselFehler::Datei {
            pfad: pfad.to_path_buf(),
            grund: e.to_string(),
        })?;
        setze_enge_rechte(pfad)?;
        Self::aus_ikm(&ikm, Herkunft::NeuErzeugt)
    }

    /// Ein Schlüssel für den Probelauf, aus einem Teilnehmernamen.
    ///
    /// **Wer den Namen kennt, kennt den Schlüssel.** Ein eigener Aufruf
    /// und keine Voreinstellung, damit niemand versehentlich damit ins
    /// Netz geht; die Herkunft steht danach in jeder Startzeile.
    pub fn probe(name: &str) -> Result<Self, SchluesselFehler> {
        let saat = Hash::sha256(
            format!("{}{name}", crate::validatorsatz::PROBE_SCHLUESSEL_PRAEFIX).as_bytes(),
        );
        Self::aus_ikm(saat.as_bytes(), Herkunft::Probelauf)
    }

    /// Der öffentliche Schlüssel.
    pub fn oeffentlich(&self) -> BlsPublicKey {
        self.oeffentlich
    }

    /// Die Kennung: `sha256(pubkey)`, wie in [`crate::genesis`].
    pub fn kennung(&self) -> MinerId {
        MinerId::aus_schluessel(&self.oeffentlich)
    }

    /// Woher dieser Schlüssel stammt.
    pub fn herkunft(&self) -> Herkunft {
        self.herkunft
    }

    /// Der Besitznachweis für die Genesis-Datei (Fund 27).
    pub fn besitznachweis(&self) -> Result<BlsProofOfPossession, SchluesselFehler> {
        self.geheim
            .prove_possession()
            .map_err(|_| SchluesselFehler::Bls("prove_possession"))
    }

    /// Signiert eine kanonische Botschaft aus `myl_consensus::signing`.
    pub fn signiere(&self, botschaft: &[u8]) -> Result<BlsSignature, SchluesselFehler> {
        self.geheim
            .sign(botschaft)
            .map_err(|_| SchluesselFehler::Bls("sign"))
    }

    /// Die Zeile, die dieser Schlüssel in einer Genesis-Datei ergäbe.
    ///
    /// Damit jemand, der ein Probenetz aufsetzt, die Datei aus den
    /// laufenden Knoten zusammensetzen kann, statt Hexketten von Hand
    /// abzuschreiben.
    pub fn genesiszeile(&self, stake: u64) -> Result<String, SchluesselFehler> {
        let pop = self.besitznachweis()?;
        let hex = |b: &[u8]| -> String {
            let mut s = String::with_capacity(b.len() * 2);
            for x in b {
                s.push_str(&format!("{x:02x}"));
            }
            s
        };
        Ok(format!(
            "validator {} {} {}",
            hex(&self.oeffentlich.0),
            hex(&pop.0),
            stake
        ))
    }
}

fn hex_ikm(wort: &str) -> Option<[u8; IKM_BYTES]> {
    if wort.len() != IKM_BYTES * 2 {
        return None;
    }
    let mut roh = [0u8; IKM_BYTES];
    for (i, byte) in roh.iter_mut().enumerate() {
        *byte = u8::from_str_radix(wort.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(roh)
}

#[cfg(unix)]
fn pruefe_rechte(pfad: &Path) -> Result<(), SchluesselFehler> {
    use std::os::unix::fs::PermissionsExt;
    let modus = fs::metadata(pfad)
        .map_err(|e| SchluesselFehler::Datei {
            pfad: pfad.to_path_buf(),
            grund: e.to_string(),
        })?
        .permissions()
        .mode()
        & 0o777;
    if modus & 0o077 != 0 {
        return Err(SchluesselFehler::ZuOffen {
            pfad: pfad.to_path_buf(),
            modus,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn pruefe_rechte(_pfad: &Path) -> Result<(), SchluesselFehler> {
    // Auf Systemen ohne Unix-Rechte gibt es nichts zu prüfen. Das ist
    // keine Entwarnung, sondern eine Auslassung, und sie steht hier,
    // damit sie nicht wie eine Prüfung aussieht.
    Ok(())
}

#[cfg(unix)]
fn setze_enge_rechte(pfad: &Path) -> Result<(), SchluesselFehler> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(pfad, fs::Permissions::from_mode(0o600)).map_err(|e| {
        SchluesselFehler::Datei {
            pfad: pfad.to_path_buf(),
            grund: e.to_string(),
        }
    })
}

#[cfg(not(unix))]
fn setze_enge_rechte(_pfad: &Path) -> Result<(), SchluesselFehler> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::signing::vote_message;

    fn tempdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("myl-schluessel-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("Testverzeichnis");
        p
    }

    #[test]
    fn ein_neuer_schluessel_wird_geschrieben_und_wiedergelesen() {
        let ordner = tempdir("neu");
        let pfad = ordner.join("konsens.key");
        let a = Konsensschluessel::aus_datei(&pfad).expect("erzeugen");
        assert_eq!(a.herkunft(), Herkunft::NeuErzeugt);
        assert!(pfad.exists());

        let b = Konsensschluessel::aus_datei(&pfad).expect("wiederlesen");
        assert_eq!(b.herkunft(), Herkunft::Datei);
        // **Der Punkt der Datei:** Über einen Neustart bleibt die Stimme
        // dieselbe. Sonst wäre nach jedem Neustart ein anderer Validator
        // da, und die Genesis-Datei zeigte ins Leere.
        assert_eq!(a.kennung(), b.kennung());
        assert_eq!(a.oeffentlich(), b.oeffentlich());
        fs::remove_dir_all(&ordner).ok();
    }

    #[test]
    fn zwei_neue_schluessel_sind_verschieden() {
        let ordner = tempdir("zwei");
        let a = Konsensschluessel::aus_datei(&ordner.join("a.key")).expect("a");
        let b = Konsensschluessel::aus_datei(&ordner.join("b.key")).expect("b");
        assert_ne!(a.kennung(), b.kennung());
        fs::remove_dir_all(&ordner).ok();
    }

    #[cfg(unix)]
    #[test]
    fn eine_neue_datei_bekommt_enge_rechte() {
        use std::os::unix::fs::PermissionsExt;
        let ordner = tempdir("rechte");
        let pfad = ordner.join("konsens.key");
        Konsensschluessel::aus_datei(&pfad).expect("erzeugen");
        let modus = fs::metadata(&pfad).unwrap().permissions().mode() & 0o777;
        assert_eq!(modus, 0o600, "Rechte waren {modus:o}");
        fs::remove_dir_all(&ordner).ok();
    }

    #[cfg(unix)]
    #[test]
    fn eine_zu_offene_datei_wird_nicht_gelesen() {
        // Ein geheimer Schlüssel, den andere lesen können, ist kein
        // Geheimnis. Der Knoten startet dann nicht, statt still
        // weiterzumachen.
        use std::os::unix::fs::PermissionsExt;
        let ordner = tempdir("offen");
        let pfad = ordner.join("konsens.key");
        Konsensschluessel::aus_datei(&pfad).expect("erzeugen");
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o644)).unwrap();
        match Konsensschluessel::aus_datei(&pfad) {
            Err(SchluesselFehler::ZuOffen { modus, .. }) => assert_eq!(modus, 0o644),
            andere => panic!("erwartet ZuOffen, bekommen {andere:?}"),
        }
        fs::remove_dir_all(&ordner).ok();
    }

    #[test]
    fn eine_unlesbare_datei_faellt_auf() {
        let ordner = tempdir("kaputt");
        let pfad = ordner.join("konsens.key");
        fs::write(&pfad, "# Kopf\nnicht hex\n").unwrap();
        setze_enge_rechte(&pfad).unwrap();
        assert!(matches!(
            Konsensschluessel::aus_datei(&pfad),
            Err(SchluesselFehler::KeinSchluessel { .. })
        ));
        fs::remove_dir_all(&ordner).ok();
    }

    #[test]
    fn der_probeschluessel_haengt_am_namen_und_sagt_es() {
        let a = Konsensschluessel::probe("alpha").expect("alpha");
        let b = Konsensschluessel::probe("alpha").expect("alpha nochmal");
        let c = Konsensschluessel::probe("beta").expect("beta");
        assert_eq!(a.kennung(), b.kennung());
        assert_ne!(a.kennung(), c.kennung());
        assert_eq!(a.herkunft(), Herkunft::Probelauf);
        assert!(
            !a.herkunft().ist_geheim(),
            "der Probeschlüssel darf sich nicht als geheim ausgeben"
        );
    }

    #[test]
    fn der_probeschluessel_deckt_sich_mit_dem_des_validatorsatzes() {
        // Zwei Ableitungen desselben Schlüssels an zwei Stellen wären
        // zwei Wahrheiten. Dieser Test hält sie zusammen: Fällt eine
        // auseinander, prüft der Validatorsatz gegen einen anderen
        // Schlüssel als der, mit dem hier signiert wird.
        let hier = Konsensschluessel::probe("alpha").expect("hier");
        let dort = crate::validatorsatz::probe_schluessel("alpha").expect("dort");
        assert_eq!(hier.oeffentlich(), dort.public_key().unwrap());
        assert_eq!(
            hier.kennung(),
            crate::validatorsatz::probe_kennung("alpha").unwrap()
        );
    }

    #[test]
    fn die_herkunft_unterscheidet_geheim_von_ableitbar() {
        assert!(Herkunft::Datei.ist_geheim());
        assert!(Herkunft::NeuErzeugt.ist_geheim());
        assert!(!Herkunft::Probelauf.ist_geheim());
    }

    #[test]
    fn signieren_und_pruefen_passen_zusammen() {
        let k = Konsensschluessel::probe("alpha").expect("Schlüssel");
        let botschaft = vote_message(7, &Hash::sha256(b"block"));
        let sig = k.signiere(&botschaft).expect("signieren");
        assert!(k.oeffentlich().verify(&botschaft, &sig));
        // Gegenprobe: eine andere Runde prüft nicht.
        assert!(!k
            .oeffentlich()
            .verify(&vote_message(8, &Hash::sha256(b"block")), &sig));
    }

    #[test]
    fn der_besitznachweis_gilt_fuer_den_eigenen_schluessel() {
        let k = Konsensschluessel::probe("alpha").expect("Schlüssel");
        let pop = k.besitznachweis().expect("pop");
        assert!(k.oeffentlich().verify_possession(&pop));
        // Fund 27: der Nachweis eines anderen zählt nicht.
        let fremd = Konsensschluessel::probe("beta").expect("beta");
        assert!(!fremd.oeffentlich().verify_possession(&pop));
    }

    #[test]
    fn die_genesiszeile_laesst_sich_direkt_einlesen() {
        // Damit niemand Hexketten von Hand abschreibt. Vier Knoten,
        // weil die Ein-Drittel-Schranke darunter nicht erfüllbar ist.
        let mut text = String::from("netz aus-schluesseln\n");
        for (name, stake) in [("a", 250u64), ("b", 230), ("c", 200), ("d", 220)] {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            text.push_str(&k.genesiszeile(stake).expect("Zeile"));
            text.push('\n');
        }
        let g = crate::genesis::Genesis::aus_text(&text).expect("lesbar");
        assert_eq!(g.validatoren.len(), 4);
        assert_eq!(g.gesamtstake(), 900);
        // Und die Kennungen decken sich mit denen der Schlüssel.
        let kennungen = g.kennungen();
        for name in ["a", "b", "c", "d"] {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            assert!(kennungen.contains(&k.kennung()), "{name} fehlt");
        }
    }

    #[test]
    fn debug_zeigt_den_geheimen_teil_nicht() {
        // Ein abgeleitetes Debug schriebe ihn in jede Fehlermeldung.
        let k = Konsensschluessel::probe("alpha").expect("Schlüssel");
        let text = format!("{k:?}");
        assert!(text.contains("Konsensschluessel"));
        assert!(text.contains("herkunft"));
        assert!(
            !text.contains("geheim"),
            "das Debug nannte das geheime Feld: {text}"
        );
    }
}
