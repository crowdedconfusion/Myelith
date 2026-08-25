//! Wer darf attestieren: die Zuordnung von Kennung zu Schlüssel.
//!
//! # ⚑ A10, und warum es dafür dieses Modul braucht
//!
//! `LatencyAttest` trägt ein Signaturfeld, das bis zum 2026-08-25
//! **niemand gesetzt und niemand geprüft** hat. Der Sicherheitsaudit
//! führt das als A10 und sagt scharf, warum das schlimmer ist als ein
//! fehlendes Feld: **Ein ungeprüftes Signaturfeld ist gefährlicher als
//! gar keines, weil ein Leser es für einen Schutz hält.**
//!
//! Die Latenzwerte gehen ins Geo-Clustering der Pods. Wer sie frei
//! setzen kann, sucht sich seine Pod-Nachbarn aus, und das ist die
//! Vorstufe zur Kollusion beider Pods (A12).
//!
//! Prüfen lässt sich eine Signatur nur gegen einen Schlüssel, und ein
//! Attest nennt seinen Aussteller als `MinerId`, nicht als Schlüssel.
//! **Genau diese Zuordnung fehlte**, und deshalb war A10 nicht zu
//! schließen, solange niemand Netz- und Konsensschicht in einem Prozess
//! hatte.
//!
//! # ⚑ Was dieser Satz ist, und was er nicht ist
//!
//! **Ein Probeaufbau, kein Protokollmechanismus.** Die Schlüssel werden
//! aus den Teilnehmernamen abgeleitet, die der Koordinator ohnehin
//! verteilt. Das genügt, um den Prüfpfad zu durchlaufen und eine
//! gefälschte Signatur abzuweisen.
//!
//! **Was es ausdrücklich nicht leistet:** Geheimhaltung. Wer die Namen
//! kennt, kann die Schlüssel ableiten und damit in fremdem Namen
//! signieren. Für einen Probelauf ist das hinnehmbar, weil dort niemand
//! angreift; **für ein echtes Netz ist es das nicht.** Dort kommen die
//! Schlüssel aus der Validator-Registrierung zu Genesis, mit
//! Besitznachweis (Fund 27), und dieselbe Prüffunktion arbeitet dann
//! gegen echte Schlüssel.
//!
//! Die Trennlinie liegt also nicht im Prüfcode, sondern in der Herkunft
//! der Schlüssel. Das ist die richtige Stelle für sie.

use std::collections::BTreeMap;

use myl_types::bls::{BlsPublicKey, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

/// Ableitungspräfix der Probeschlüssel.
///
/// Steht im Klartext in jedem abgeleiteten Schlüssel und sagt, was er
/// ist: kein Schlüssel eines echten Netzes.
pub const PROBE_SCHLUESSEL_PRAEFIX: &str = "MYELITH-PROBELAUF-SCHLUESSEL-";

/// Der geheime Schlüssel eines Probeteilnehmers.
///
/// **Aus dem Namen abgeleitet**, siehe Modulkopf: Wer den Namen kennt,
/// kennt den Schlüssel. Für einen Probelauf gewollt, für ein echtes
/// Netz unbrauchbar.
pub fn probe_schluessel(name: &str) -> Option<BlsSecretKey> {
    let saat = Hash::sha256(format!("{PROBE_SCHLUESSEL_PRAEFIX}{name}").as_bytes());
    let mut ikm = [0u8; 32];
    ikm.copy_from_slice(saat.as_bytes());
    BlsSecretKey::key_gen(&ikm).ok()
}

/// Die Kennung eines Probeteilnehmers.
///
/// **Aus dem Schlüssel abgeleitet, nicht aus dem Namen.** Das ist der
/// Unterschied, der zählt: Eine Kennung, die am Namen hinge, ließe sich
/// von jedem beanspruchen. So hängt sie am Schlüssel, und wer die
/// Kennung führen will, muss den Schlüssel haben. Im echten Netz gilt
/// dasselbe Verhältnis, nur mit einem Schlüssel, den niemand ableiten
/// kann.
pub fn probe_kennung(name: &str) -> Option<MinerId> {
    let pk = probe_schluessel(name)?.public_key().ok()?;
    let h = Hash::sha256(&pk.0);
    let mut roh = [0u8; 32];
    roh.copy_from_slice(h.as_bytes());
    Some(MinerId::new(roh))
}

/// Wer im Probelauf attestieren darf.
#[derive(Debug, Clone, Default)]
pub struct Validatorsatz {
    schluessel: BTreeMap<MinerId, BlsPublicKey>,
}

impl Validatorsatz {
    /// Leer: niemand darf attestieren, jedes Attest wird abgewiesen.
    ///
    /// **Der Vorgabezustand, und das ist Absicht.** Ein Knoten, dem
    /// niemand genannt wurde, kann keine Signatur prüfen und darf
    /// deshalb keine annehmen. Die Alternative wäre, ungeprüfte Atteste
    /// durchzulassen, und damit wäre A10 wieder offen, nur mit mehr
    /// Code.
    pub fn leer() -> Self {
        Self::default()
    }

    /// Aus den Teilnehmernamen, die der Koordinator verteilt.
    pub fn aus_namen<S: AsRef<str>>(namen: &[S]) -> Self {
        let mut schluessel = BTreeMap::new();
        for n in namen {
            let name = n.as_ref();
            if let (Some(id), Some(sk)) = (probe_kennung(name), probe_schluessel(name)) {
                if let Ok(pk) = sk.public_key() {
                    schluessel.insert(id, pk);
                }
            }
        }
        Self { schluessel }
    }

    /// Anzahl bekannter Aussteller.
    pub fn anzahl(&self) -> usize {
        self.schluessel.len()
    }

    /// Ob dieser Aussteller bekannt ist.
    pub fn kennt(&self, id: &MinerId) -> bool {
        self.schluessel.contains_key(id)
    }

    /// Prüft ein Attest.
    ///
    /// Gibt den Grund zurück, damit das Betriebsprotokoll ihn nennen
    /// kann: **„unbekannter Aussteller" und „falsche Signatur" haben
    /// verschiedene Ursachen**, und die erste ist im Probelauf fast
    /// immer ein vergessener Name in der Teilnehmerliste. Ohne die
    /// Unterscheidung suchte jemand nach einem Angriff, wo eine
    /// Kommandozeile unvollständig war.
    pub fn pruefe(&self, attest: &myl_types::LatencyAttest) -> Attesturteil {
        let Some(pk) = self.schluessel.get(&attest.issuer) else {
            return Attesturteil::UnbekannterAussteller;
        };
        if !attest.verify(pk) {
            return Attesturteil::SignaturFalsch;
        }
        if attest.validate_structure().is_err() {
            return Attesturteil::StrukturFalsch;
        }
        Attesturteil::Gueltig
    }
}

/// Das Urteil über ein Attest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attesturteil {
    Gueltig,
    /// Der Aussteller steht nicht im Validatorsatz. Im Probelauf fast
    /// immer ein vergessener Name.
    UnbekannterAussteller,
    /// Die Signatur passt nicht zum Schlüssel des Ausstellers.
    SignaturFalsch,
    /// Zeitstempel oder Latenzwerte sind unplausibel.
    StrukturFalsch,
}

impl Attesturteil {
    pub fn als_text(&self) -> &'static str {
        match self {
            Self::Gueltig => "gueltig",
            Self::UnbekannterAussteller => "unbekannter-aussteller",
            Self::SignaturFalsch => "signatur-falsch",
            Self::StrukturFalsch => "struktur-falsch",
        }
    }

    pub fn ist_gueltig(&self) -> bool {
        matches!(self, Self::Gueltig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::latency_attest::{BlsSignatureBytes, PeerIdBytes};
    use myl_types::LatencyAttest;

    fn attest_von(name: &str) -> LatencyAttest {
        let mut a = LatencyAttest {
            issuer: probe_kennung(name).expect("Kennung"),
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            latencies: vec![(PeerIdBytes([4u8; 32]), 25)],
            signature: BlsSignatureBytes([0u8; 96]),
        };
        a.sign(&probe_schluessel(name).expect("Schlüssel")).expect("Signieren");
        a
    }

    #[test]
    fn ein_leerer_satz_weist_alles_ab() {
        // Der Vorgabezustand. Ungeprüfte Atteste durchzulassen hieße,
        // A10 offen zu halten, nur mit mehr Code.
        assert_eq!(
            Validatorsatz::leer().pruefe(&attest_von("alpha")),
            Attesturteil::UnbekannterAussteller
        );
    }

    #[test]
    fn ein_eigenes_attest_wird_anerkannt() {
        let satz = Validatorsatz::aus_namen(&["alpha", "beta"]);
        assert_eq!(satz.anzahl(), 2);
        assert_eq!(satz.pruefe(&attest_von("alpha")), Attesturteil::Gueltig);
        assert_eq!(satz.pruefe(&attest_von("beta")), Attesturteil::Gueltig);
    }

    #[test]
    fn ein_vergessener_name_faellt_als_solcher_auf() {
        // Der häufigste Fall im Probelauf, und er darf nicht wie ein
        // Angriff aussehen.
        let satz = Validatorsatz::aus_namen(&["alpha", "beta"]);
        assert_eq!(
            satz.pruefe(&attest_von("gamma")),
            Attesturteil::UnbekannterAussteller
        );
    }

    /// **Der Angriff, gegen den A10 gerichtet ist.**
    ///
    /// Jemand behauptet fremde Latenzwerte, um sich seine Pod-Nachbarn
    /// auszusuchen.
    #[test]
    fn gefaelschte_latenzwerte_werden_abgewiesen() {
        let satz = Validatorsatz::aus_namen(&["alpha", "beta"]);
        let mut a = attest_von("alpha");
        a.latencies[0].1 = 1; // „ich bin ganz nah dran"
        assert_eq!(satz.pruefe(&a), Attesturteil::SignaturFalsch);
    }

    #[test]
    fn ein_attest_im_namen_eines_anderen_wird_abgewiesen() {
        // Signatur von Alpha, Aussteller Beta.
        let satz = Validatorsatz::aus_namen(&["alpha", "beta"]);
        let mut a = attest_von("alpha");
        a.issuer = probe_kennung("beta").expect("Kennung");
        assert_eq!(satz.pruefe(&a), Attesturteil::SignaturFalsch);
    }

    #[test]
    fn ein_unsigniertes_attest_wird_abgewiesen() {
        let satz = Validatorsatz::aus_namen(&["alpha"]);
        let mut a = attest_von("alpha");
        a.signature = BlsSignatureBytes([0u8; 96]);
        assert_eq!(satz.pruefe(&a), Attesturteil::SignaturFalsch);
    }

    #[test]
    fn die_kennung_haengt_am_schluessel_nicht_am_namen() {
        // Eine Kennung, die am Namen hinge, ließe sich von jedem
        // beanspruchen. So muss man den Schlüssel haben.
        let id = probe_kennung("alpha").expect("Kennung");
        let pk = probe_schluessel("alpha").unwrap().public_key().unwrap();
        let erwartet = Hash::sha256(&pk.0);
        assert_eq!(id.as_bytes(), erwartet.as_bytes());
    }

    #[test]
    fn verschiedene_namen_ergeben_verschiedene_kennungen() {
        assert_ne!(probe_kennung("alpha"), probe_kennung("beta"));
        assert_eq!(probe_kennung("alpha"), probe_kennung("alpha"));
    }

    #[test]
    fn das_praefix_sagt_im_klartext_was_die_schluessel_sind() {
        assert!(PROBE_SCHLUESSEL_PRAEFIX.contains("PROBELAUF"));
    }
}
