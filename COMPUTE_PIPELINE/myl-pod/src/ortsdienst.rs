//! Die lokale Tür des Shard-Prozesses (GATEWAY Stufe 4).
//!
//! # ⚑ Wozu ein Shard eine Tür braucht
//!
//! Bis zum 2026-09-03 bekam ein Pod seine Arbeit von der
//! Kommandozeile: `myl-pod-node --prompt "<text>"`. Ein Auftrag aus dem
//! Netz hatte keinen Weg hierher, auf keiner der beiden Seiten.
//!
//! Seit der Entscheidung vom selben Tag läuft ein Shard in einem
//! **eigenen Prozess**. Der heisse Pfad (Aktivierungen je Token) geht
//! über [`crate::wire`] direkt zum Nachbarshard; der kalte Pfad
//! (Auftrag, Bündel) geht über diese Tür zum Knoten.
//!
//! **Dieselbe Trennung, die K0 für die Tür des Gateways verlangt:** Wer
//! rechnet und wer sich einigt, sollte nicht derselbe Prozess sein. Ein
//! Absturz beim Rechnen hält sonst den Konsens an.
//!
//! # ⚑ Hier steht die Formprüfung, und hier hat sie zwei Ausgänge
//!
//! Am 2026-09-03 stand dieselbe Prüfung schon einmal im Knoten und
//! wurde wieder ausgebaut (Fund 154): Dort lieferten beide Zweige
//! dasselbe `Abgelehnt`, weil der Knoten ohnehin nicht rechnet. **Hier
//! unterscheidet sie etwas**, denn hinter dem einen Zweig liegt ein
//! Rechenwerk und hinter dem anderen nicht. Das ist die Naht, an die
//! sie gehört.
//!
//! # Eine Verbindung nach der anderen
//!
//! ⚑ **Kein Bündel von Fäden, und das ist Absicht.** Ein Shard ist ein
//! Rechenwerk; zwei gleichzeitige Aufträge stritten um dasselbe Modell
//! und denselben KV-Cache und wären langsamer als nacheinander. Was das
//! kostet, gehört gesagt: **Ein langsamer Klient hält die Leitung.**
//! Dagegen stehen Lese- und Schreibfristen, nicht mehr und nicht
//! weniger.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::ortsleitung::{
    antwortrahmen, entrahmen, schluessel_ablegen, Ortsantwort, Ortsfrage, Rahmenfehler,
    SCHLUESSEL_DATEI, SCHLUESSEL_LEN,
};

/// Wie lange auf die Bytes eines Klienten gewartet wird.
///
/// ⚑ **Ohne Frist hielte ein einziger stiller Klient die Tür für
/// immer.** Dreissig Sekunden sind grosszügig für einen lokalen
/// Prozess, der einen fertigen Rahmen schickt.
pub const LESEFRIST: Duration = Duration::from_secs(30);

/// Wie lange auf die Abnahme der Antwort gewartet wird.
pub const SCHREIBFRIST: Duration = Duration::from_secs(30);

/// Was der Dienst hinter der Tür an Arbeit weiterreicht.
///
/// ⚑ **Ein Merkmal und keine feste Verdrahtung**, aus einem Grund, der
/// sich messen lässt: Die Pipeline braucht geladene Artefakte, und ein
/// Test, der jedes Mal ein Modell lädt, wird nicht gefahren. Der Dienst
/// ist gegen dieses Merkmal geprüft, die Pipeline gegen ihre eigenen
/// Tests, und `myl-pod-node` setzt beides zusammen.
pub trait Rechenwerk: Send + Sync {
    /// Rechnet einen formgeprüften Auftrag.
    ///
    /// **Die Form ist beim Aufruf schon geprüft.** Was hier noch
    /// scheitern kann, ist das Entsiegeln, die Bindung und der
    /// Pipeline-Stand; das entscheidet die Umsetzung.
    fn rechne(&self, auftrag: &Inferenzauftrag) -> Inferenzantwort;

    /// Für welchen Pipeline-Stand dieser Prozess geladen ist.
    fn pipeline(&self) -> myl_types::hash::Hash;

    /// Wie viele Shards die Pipeline hat.
    fn shards(&self) -> u32;

    /// Wer dieser Shard ist und wofür für ihn zu versiegeln ist.
    ///
    /// ⚑ **Ohne diese Auskunft kann niemand für ihn versiegeln.** Der
    /// Fragende muss den Kapselpunkt kennen, sonst bildet der Shard
    /// seinen Empfangsschlüssel gar nicht. `None` heisst: Dieser Shard
    /// nimmt nichts Versiegeltes an, und das ist eine ehrliche Antwort
    /// und keine Lücke.
    fn gegenstelle(&self) -> Option<([u8; 32], [u8; 32], Vec<u8>)> {
        None
    }

    /// Nimmt die unterschriebene Ankündigung der Gegenstelle entgegen.
    ///
    /// ⚑ **Vorgabe: nein.** Ein Rechenwerk ohne Siegel hat keine
    /// Gegenstelle, und „nein" ist darauf die ehrliche Antwort. Wer
    /// hier `true` zurückgäbe, ohne etwas zu prüfen, machte aus der
    /// Ankündigung eine Höflichkeitsfloskel.
    fn ankuendigung_annehmen(&self, _roh: &[u8]) -> bool {
        false
    }
}

/// Die lokale Tür.
pub struct Ortsdienst {
    horcher: TcpListener,
    schluessel: [u8; SCHLUESSEL_LEN],
    werk: Box<dyn Rechenwerk>,
}

/// Was beim Öffnen der Tür herauskam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Oeffnungsbefund {
    /// Die Adresse, auf der die Tür hört.
    pub adresse: SocketAddr,
    /// Ob der Ausweis vom Dateisystem geschützt ist (Unix: `0600`).
    pub ausweis_geschuetzt: bool,
    /// ⚑ Ob die Tür **nach aussen** gebunden ist.
    ///
    /// Dann kann sie jeder im Netz wählen, und der Ausweis ist alles,
    /// was zwischen einem Fremden und dem Rechenwerk steht. Das ist
    /// nicht verboten, es gehört nur gesagt.
    pub nach_aussen: bool,
}

impl Ortsdienst {
    /// Öffnet die Tür, legt einen frischen Ausweis ab und nimmt das
    /// Rechenwerk entgegen.
    ///
    /// ⚑ **Der Ausweis ist bei jedem Start neu.** Ein Ausweis, der
    /// einen Neustart überlebt, überlebt auch den Grund für den
    /// Neustart.
    pub fn oeffnen(
        adresse: SocketAddr,
        verzeichnis: &std::path::Path,
        werk: Box<dyn Rechenwerk>,
    ) -> std::io::Result<(Self, Oeffnungsbefund)> {
        let mut schluessel = [0u8; SCHLUESSEL_LEN];
        getrandom::getrandom(&mut schluessel).map_err(|e| {
            std::io::Error::other(format!("keine Zufallsquelle fuer den Ausweis: {e}"))
        })?;
        std::fs::create_dir_all(verzeichnis)?;
        let geschuetzt = schluessel_ablegen(&verzeichnis.join(SCHLUESSEL_DATEI), &schluessel)?;
        let horcher = TcpListener::bind(adresse)?;
        let echte = horcher.local_addr()?;
        let befund = Oeffnungsbefund {
            adresse: echte,
            ausweis_geschuetzt: geschuetzt,
            nach_aussen: !echte.ip().is_loopback(),
        };
        Ok((
            Self {
                horcher,
                schluessel,
                werk,
            },
            befund,
        ))
    }

    /// Die Adresse, auf der die Tür hört.
    pub fn adresse(&self) -> std::io::Result<SocketAddr> {
        self.horcher.local_addr()
    }

    /// Der Ausweis, den ein Klient mitbringen muss.
    ///
    /// **Nur für Tests und für den Prozess selbst.** Wer ihn im Betrieb
    /// braucht, liest die Datei.
    #[doc(hidden)]
    pub fn schluessel(&self) -> [u8; SCHLUESSEL_LEN] {
        self.schluessel
    }

    /// Nimmt eine Verbindung an und beantwortet, was auf ihr kommt.
    ///
    /// Gibt zurück, wie viele Fragen beantwortet wurden.
    pub fn bediene_eine(&self) -> std::io::Result<usize> {
        let (strom, _) = self.horcher.accept()?;
        self.bediene_strom(strom)
    }

    /// Läuft, bis die Tür geschlossen wird.
    pub fn laufen(&self) {
        loop {
            if self.bediene_eine().is_err() {
                // ⚑ **Ein Fehler auf einer Verbindung schliesst nicht
                // die Tür.** Wer sonst den Dienst beenden könnte, wäre
                // jeder lokale Prozess mit einem halben Rahmen.
                continue;
            }
        }
    }

    fn bediene_strom(&self, mut strom: TcpStream) -> std::io::Result<usize> {
        strom.set_read_timeout(Some(LESEFRIST))?;
        strom.set_write_timeout(Some(SCHREIBFRIST))?;
        let mut puffer = Vec::new();
        let mut stueck = [0u8; 8192];
        let mut beantwortet = 0usize;
        loop {
            match entrahmen(&puffer, &self.schluessel) {
                Ok((verbraucht, nutzlast)) => {
                    puffer.drain(..verbraucht);
                    let antwort = self.beantworte(&nutzlast);
                    let roh = borsh::to_vec(&antwort).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })?;
                    let Some(rahmen) = antwortrahmen(&roh) else {
                        // Eine Antwort, die nicht in den Rahmen passt,
                        // kann nicht gesendet werden. Das ist ein Fehler
                        // dieser Seite, kein Grund, still zu bleiben.
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "die Antwort passt nicht in den Rahmen",
                        ));
                    };
                    strom.write_all(&rahmen)?;
                    strom.flush()?;
                    beantwortet += 1;
                    continue;
                }
                // ⚑ **Ein falscher Ausweis oder fremde Magie beendet
                // die Verbindung sofort und stumm.** Wer nicht
                // hereindarf, bekommt keine Auskunft darüber, woran es
                // lag; sonst wäre die Tür ein Ratespiel mit Rückmeldung.
                Err(Rahmenfehler::FalscherSchluessel) | Err(Rahmenfehler::FremdeMagie) => {
                    return Ok(beantwortet)
                }
                Err(Rahmenfehler::ZuLang { .. }) => return Ok(beantwortet),
                Err(Rahmenfehler::Unvollstaendig) | Err(Rahmenfehler::NutzlastFehlt { .. }) => {}
            }
            // ⚑ **Hier steht bewusst kein zweiter Deckel.** Der Puffer
            // ist schon gebunden: `NutzlastFehlt` kommt nur, solange
            // weniger als `KOPF_LEN + laenge` Bytes da sind, und
            // `laenge` liegt unter [`MAX_NUTZLAST_BYTES`], sonst wäre
            // es `ZuLang` und die Verbindung fiele oben heraus. Eine
            // Zeile, die kein Eingabewert je erreicht, ist keine
            // Prüfung, sondern Zierde (Fund 154). Gebunden wird in
            // `entrahmen`, und dort hält ein Test die Grenze fest.
            match strom.read(&mut stueck) {
                Ok(0) => return Ok(beantwortet),
                Ok(n) => puffer.extend_from_slice(&stueck[..n]),
                Err(_) => return Ok(beantwortet),
            }
        }
    }

    fn beantworte(&self, nutzlast: &[u8]) -> Ortsantwort {
        let Ok(frage) = borsh::from_slice::<Ortsfrage>(nutzlast) else {
            return Ortsantwort::Abgelehnt;
        };
        match frage {
            Ortsfrage::Lebenszeichen => Ortsantwort::Lebenszeichen {
                pipeline: self.werk.pipeline(),
                shards: self.werk.shards(),
            },
            // ⚑ **Auch das kostet keine Rechenzeit**, und es ist die
            // Auskunft, ohne die niemand für diesen Shard versiegeln
            // kann.
            Ortsfrage::Gegenstelle => match self.werk.gegenstelle() {
                Some((endpunkt, punkt, kapselpunkt)) => Ortsantwort::Gegenstelle {
                    endpunkt,
                    punkt,
                    kapselpunkt,
                },
                None => Ortsantwort::Abgelehnt,
            },
            // ⚑ **Die Formprüfung mit zwei Ausgängen** (Fund 154): Ein
            // formwidriger Auftrag wird abgelehnt, **ohne dass das
            // Rechenwerk ihn sieht**. Das ist der Deckel vor der teuren
            // Arbeit, und hier steht er richtig.
            // ⚑ **Vor dem Deckel des Rahmens noch ein eigener** (Fund
            // 165): Eine Ankündigung ist rund 1 370 Bytes; alles
            // darüber ist keine, und der Deckel steht in
            // `MAX_ANKUENDIGUNG_BYTES`.
            Ortsfrage::Ankuendigung(roh) => {
                if roh.len() > myl_types::ortsleitung::MAX_ANKUENDIGUNG_BYTES
                    || !self.werk.ankuendigung_annehmen(&roh)
                {
                    Ortsantwort::Abgelehnt
                } else {
                    Ortsantwort::Angenommen
                }
            }
            Ortsfrage::Inferenz(auftrag) => {
                if auftrag.pruefe_form().is_err() {
                    return Ortsantwort::Inferenz(Inferenzantwort::Abgelehnt {
                        sitzung: auftrag.sitzung,
                    });
                }
                Ortsantwort::Inferenz(self.werk.rechne(&auftrag))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::hash::Hash;
    use myl_types::ids::{EpochId, SegmentId};
    use myl_types::ortsleitung::{antwort_entrahmen, rahmen};
    use myl_types::sitzung::Anfragebindung;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Ein Rechenwerk, das zählt statt zu rechnen.
    struct Zaehlwerk {
        gesehen: Arc<AtomicUsize>,
    }

    impl Rechenwerk for Zaehlwerk {
        fn rechne(&self, auftrag: &Inferenzauftrag) -> Inferenzantwort {
            self.gesehen.fetch_add(1, Ordering::SeqCst);
            Inferenzantwort::Ergebnis {
                sitzung: auftrag.sitzung,
                token: vec![7, 8, 9],
                segment: SegmentId::new([3; 32]),
                prompt_token: 1,
                text: "probe".to_string(),
            }
        }
        fn pipeline(&self) -> Hash {
            Hash::sha256(b"probe-pipeline")
        }
        fn shards(&self) -> u32 {
            4
        }
    }

    fn dienst() -> (Ortsdienst, Arc<AtomicUsize>, std::path::PathBuf) {
        let gesehen = Arc::new(AtomicUsize::new(0));
        let verz = std::env::temp_dir().join(format!(
            "myl-ortsdienst-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let (d, befund) = Ortsdienst::oeffnen(
            "127.0.0.1:0".parse().expect("Adresse"),
            &verz,
            Box::new(Zaehlwerk {
                gesehen: Arc::clone(&gesehen),
            }),
        )
        .expect("Tuer geht auf");
        assert!(!befund.nach_aussen, "die Probetuer haengt nach aussen");
        (d, gesehen, verz)
    }

    fn auftrag(max_token: u32) -> Inferenzauftrag {
        Inferenzauftrag {
            sitzung: 11,
            bindung: Anfragebindung::neu(11, b"die frage", EpochId(2)),
            prompt_versiegelt: b"versiegelt".to_vec(),
            max_token,
            pipeline: Hash::sha256(b"probe-pipeline"),
        }
    }

    /// Schickt einen Rahmen über einen echten Socket und liest zurück.
    fn frage(
        adresse: SocketAddr,
        schluessel: &[u8; SCHLUESSEL_LEN],
        frage: &Ortsfrage,
    ) -> Option<Ortsantwort> {
        let nutzlast = borsh::to_vec(frage).expect("kodieren");
        let roh = rahmen(schluessel, &nutzlast).expect("rahmen");
        let mut strom = TcpStream::connect(adresse).expect("verbinden");
        strom.set_read_timeout(Some(Duration::from_secs(10))).expect("Frist");
        strom.write_all(&roh).expect("senden");
        strom.flush().expect("leeren");
        let mut puffer = Vec::new();
        let mut stueck = [0u8; 4096];
        loop {
            if let Ok((_, n)) = antwort_entrahmen(&puffer) {
                return borsh::from_slice::<Ortsantwort>(&n).ok();
            }
            match strom.read(&mut stueck) {
                Ok(0) | Err(_) => return None,
                Ok(n) => puffer.extend_from_slice(&stueck[..n]),
            }
        }
    }

    /// ⚑ **Verdrahtet und nicht nur gebaut**: über einen echten Socket.
    #[test]
    fn ein_auftrag_erreicht_das_rechenwerk() {
        let (d, gesehen, verz) = dienst();
        let adresse = d.adresse().expect("Adresse");
        let k = d.schluessel();
        let faden = std::thread::spawn(move || d.bediene_eine());
        let antwort = frage(adresse, &k, &Ortsfrage::Inferenz(auftrag(32)));
        let _ = faden.join();
        assert_eq!(
            antwort,
            Some(Ortsantwort::Inferenz(Inferenzantwort::Ergebnis {
                sitzung: 11,
                token: vec![7, 8, 9],
                segment: SegmentId::new([3; 32]),
                prompt_token: 1,
                text: "probe".to_string(),
            }))
        );
        assert_eq!(gesehen.load(Ordering::SeqCst), 1, "das Rechenwerk lief nicht");
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// ⚑ **Der Deckel steht vor der Arbeit** (Fund 154, an der Naht, an
    /// die er gehört): Ein formwidriger Auftrag wird abgelehnt, **und
    /// das Rechenwerk sieht ihn nicht**.
    #[test]
    fn ein_formwidriger_auftrag_erreicht_das_rechenwerk_nicht() {
        let (d, gesehen, verz) = dienst();
        let adresse = d.adresse().expect("Adresse");
        let k = d.schluessel();
        let faden = std::thread::spawn(move || d.bediene_eine());
        let antwort = frage(adresse, &k, &Ortsfrage::Inferenz(auftrag(0)));
        let _ = faden.join();
        assert_eq!(
            antwort,
            Some(Ortsantwort::Inferenz(Inferenzantwort::Abgelehnt {
                sitzung: 11
            }))
        );
        assert_eq!(
            gesehen.load(Ordering::SeqCst),
            0,
            "das Rechenwerk hat einen formwidrigen Auftrag gesehen"
        );
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// ⚑ **Ohne Ausweis kommt niemand herein**, und er bekommt auch
    /// keine Auskunft darüber, woran es lag.
    #[test]
    fn ohne_ausweis_bleibt_die_tuer_stumm() {
        let (d, gesehen, verz) = dienst();
        let adresse = d.adresse().expect("Adresse");
        let mut falsch = d.schluessel();
        falsch[0] ^= 1;
        let faden = std::thread::spawn(move || d.bediene_eine());

        // ⚑ **Gezaehlt werden Bytes, nicht Antworten.** Die Zusicherung
        // lautet „keine Auskunft", nicht „keine gueltige Antwort": Auch
        // vier Bytes „nein" sagten einem Fremden, dass er die richtige
        // Tuer und den falschen Ausweis hat. Eine Fassung dieses Tests
        // hat genau das durchgelassen.
        let nutzlast = borsh::to_vec(&Ortsfrage::Inferenz(auftrag(32))).expect("kodieren");
        let roh = rahmen(&falsch, &nutzlast).expect("rahmen");
        let mut strom = TcpStream::connect(adresse).expect("verbinden");
        strom.set_read_timeout(Some(Duration::from_secs(10))).expect("Frist");
        strom.write_all(&roh).expect("senden");
        strom.flush().expect("leeren");
        let mut zurueck = Vec::new();
        let mut stueck = [0u8; 1024];
        loop {
            match strom.read(&mut stueck) {
                Ok(0) | Err(_) => break,
                Ok(n) => zurueck.extend_from_slice(&stueck[..n]),
            }
        }
        let beantwortet = faden.join().expect("Faden").expect("bedienen");
        assert!(
            zurueck.is_empty(),
            "die Tuer hat einem Fremden {} Bytes gesagt: {zurueck:?}",
            zurueck.len()
        );
        assert_eq!(beantwortet, 0, "die Tuer hat eine fremde Frage beantwortet");
        assert_eq!(gesehen.load(Ordering::SeqCst), 0, "das Rechenwerk lief fuer einen Fremden");
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// Das Lebenszeichen kostet keine Rechenzeit.
    #[test]
    fn das_lebenszeichen_geht_am_rechenwerk_vorbei() {
        let (d, gesehen, verz) = dienst();
        let adresse = d.adresse().expect("Adresse");
        let k = d.schluessel();
        let faden = std::thread::spawn(move || d.bediene_eine());
        let antwort = frage(adresse, &k, &Ortsfrage::Lebenszeichen);
        let _ = faden.join();
        assert_eq!(
            antwort,
            Some(Ortsantwort::Lebenszeichen {
                pipeline: Hash::sha256(b"probe-pipeline"),
                shards: 4,
            })
        );
        assert_eq!(gesehen.load(Ordering::SeqCst), 0, "ein Lebenszeichen hat gerechnet");
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// ⚑ **Zwei Fragen auf einer Verbindung.** Ein Klient soll für die
    /// zweite nicht neu wählen müssen, und der Leser muss den zweiten
    /// Rahmen im Puffer finden.
    #[test]
    fn zwei_fragen_auf_einer_verbindung() {
        let (d, gesehen, verz) = dienst();
        let adresse = d.adresse().expect("Adresse");
        let k = d.schluessel();
        let faden = std::thread::spawn(move || d.bediene_eine());

        let n1 = borsh::to_vec(&Ortsfrage::Inferenz(auftrag(32))).expect("kodieren");
        let n2 = borsh::to_vec(&Ortsfrage::Lebenszeichen).expect("kodieren");
        let mut roh = rahmen(&k, &n1).expect("rahmen");
        roh.extend_from_slice(&rahmen(&k, &n2).expect("rahmen"));

        let mut strom = TcpStream::connect(adresse).expect("verbinden");
        strom.set_read_timeout(Some(Duration::from_secs(10))).expect("Frist");
        strom.write_all(&roh).expect("senden");
        strom.flush().expect("leeren");

        let mut puffer = Vec::new();
        let mut stueck = [0u8; 4096];
        let mut antworten = Vec::new();
        while antworten.len() < 2 {
            if let Ok((verbraucht, n)) = antwort_entrahmen(&puffer) {
                antworten.push(borsh::from_slice::<Ortsantwort>(&n).expect("dekodieren"));
                puffer.drain(..verbraucht);
                continue;
            }
            match strom.read(&mut stueck) {
                Ok(0) | Err(_) => break,
                Ok(n) => puffer.extend_from_slice(&stueck[..n]),
            }
        }
        drop(strom);
        let _ = faden.join();
        assert_eq!(antworten.len(), 2, "die zweite Frage blieb unbeantwortet");
        assert!(matches!(antworten[0], Ortsantwort::Inferenz(_)));
        assert!(matches!(antworten[1], Ortsantwort::Lebenszeichen { .. }));
        assert_eq!(gesehen.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// ⚑ **Der Ausweis liegt im Verzeichnis, geschützt**, und der
    /// Knoten findet ihn dort.
    #[test]
    fn der_ausweis_liegt_wo_der_knoten_ihn_sucht() {
        let (d, _, verz) = dienst();
        let vom_datenträger =
            myl_types::ortsleitung::schluessel_lesen(&verz.join(SCHLUESSEL_DATEI))
                .expect("der Ausweis liegt nicht, wo er soll");
        assert_eq!(vom_datenträger, d.schluessel());
        let _ = std::fs::remove_dir_all(&verz);
    }
}
