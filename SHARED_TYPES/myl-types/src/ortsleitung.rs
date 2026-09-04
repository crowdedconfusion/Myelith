//! Die lokale Leitung zwischen Knoten und Shard-Prozess (GATEWAY Stufe 4).
//!
//! # ⚑ Warum es eine zweite Leitung gibt
//!
//! Am 2026-09-03 wurde entschieden: **Ein Shard läuft in einem eigenen
//! Prozess.** Der heisse Pfad (Aktivierungen je Token) geht direkt zum
//! Nachbarshard über [`crate::uebergang`] und `myl_pod::wire`; der
//! kalte Pfad (Auftrag, Zuteilung, Bündel) geht über **diese** Leitung
//! zum Knoten.
//!
//! **Dieselbe Trennung, die K0 für die Tür verlangt:** Wer rechnet und
//! wer sich einigt, sollte nicht derselbe Prozess sein. Ein Absturz
//! beim Rechnen hält sonst den Konsens an.
//!
//! # Warum das Protokoll hier steht und nicht bei einer der Seiten
//!
//! Es brauchen **beide**: `myl-node` schickt, `myl-pod` nimmt an. Die
//! beiden Kisten kennen einander nicht und sollen es nicht: `myl-pod`
//! zieht die Ganzzahl-Laufzeit und die Kerne nach, `myl-node` zieht
//! libp2p und tokio nach, und keine der beiden Seiten will die Last der
//! anderen. Dieselbe Begründung wie bei [`crate::poi_botschaft`]
//! (Fund 144) und [`crate::inferenzauftrag`].
//!
//! ⚑ **Zwei Kopien des Rahmens wären zwei Quellen für dieselbe
//! Aussage.** Ein Byte Unterschied, und die Leitung schweigt, ohne dass
//! eine Seite sagen könnte, warum.
//!
//! # ⚑ Eine schleifenlokale Tür ist eine Tür
//!
//! Sie hört auf `127.0.0.1`, aber **jeder lokale Prozess kann wählen**.
//! Ohne Ausweis bestimmte irgendein Programm auf demselben Rechner, was
//! der Shard rechnet und was er dabei mit seinem BLS-Schlüssel
//! unterschreibt.
//!
//! **Deshalb eine Schlüsseldatei**, und zwar die eingeführte Bauart für
//! eine lokale Leitung: Der Shard-Prozess legt sie beim Start an, mit Rechten
//! `0600` unter Unix, und der Knoten liest sie. Wer die Datei lesen
//! kann, darf fragen; das ist genau der Kreis, den das Dateisystem
//! ohnehin zieht.
//!
//! **Was das nicht ist:** kein Ersatz für die Versiegelung des Prompts.
//! Der Schlüssel sagt „du darfst fragen", nicht „niemand hat
//! mitgelesen". Die Leitung geht nicht über ein Netz, also ist das die
//! richtige Arbeitsteilung, und sie gehört gesagt statt angenommen.
//!
//! # ⚑ Der Ausweis steht vor der Länge, und die Länge vor der Nutzlast
//!
//! Der Rahmen ist `MAGIE | SCHLUESSEL | LAENGE | NUTZLAST`, und die
//! Reihenfolge ist die Aussage: **Ein Fremder bringt den Empfänger
//! nicht dazu, Speicher zu belegen.** Stünde die Länge vor dem Ausweis,
//! wäre die Prüfung selbst der Angriff, dieselbe Klasse wie beim Deckel
//! auf die Unterschriftsprüfungen im Gateway.

use borsh::{BorshDeserialize, BorshSerialize};
use subtle::ConstantTimeEq;

use crate::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};

/// Der Port der lokalen Leitung.
///
/// Neben 4150 (Netz), 4151 (Beobachtung) und 4160 (die eigene Tür des
/// Knotens). Ein Blick in `netstat` sagt, was wozu gehört.
pub const ORTSLEITUNG_PORT: u16 = 4170;

/// Rahmenerkennung, damit ein falsch verbundener Klient sofort auffliegt.
pub const MAGIE: [u8; 8] = *b"MYLORT01";

/// Länge des Ausweises in Bytes.
///
/// 32 Bytes aus der Zufallsquelle des Betriebssystems. Weniger wäre
/// ratbar, mehr brächte nichts.
pub const SCHLUESSEL_LEN: usize = 32;

/// Der Dateiname des Ausweises, unterhalb des Arbeitsverzeichnisses.
pub const SCHLUESSEL_DATEI: &str = "ortsschluessel";

/// Fester Vorspann: Magie, Ausweis, Länge.
pub const KOPF_LEN: usize = MAGIE.len() + SCHLUESSEL_LEN + 4;

/// Höchstlänge der Nutzlast eines Rahmens.
///
/// ⚑ **Aus dem Auftragsdeckel hergeleitet, nicht gegriffen.** Ein
/// versiegelter Prompt darf [`crate::inferenzauftrag::MAX_PROMPT_BYTES`]
/// gross sein; darauf kommen Bindung, Sitzung, Pipeline und der
/// Borsh-Rahmen. Der Zuschlag ist grosszügig und trotzdem endlich.
pub const MAX_NUTZLAST_BYTES: usize = crate::inferenzauftrag::MAX_PROMPT_BYTES + 64 * 1024;

/// ⚑ Der Deckel muss über den Auftrag passen, sonst wäre ein
/// formgültiger Auftrag unzustellbar, **und zwar erst auf der Leitung**.
const _: () = assert!(
    MAX_NUTZLAST_BYTES > crate::inferenzauftrag::MAX_PROMPT_BYTES,
    "die Nutzlastgrenze liegt unter dem Promptdeckel"
);

/// Was der Knoten den Shard-Prozess fragt.
///
/// **Additiv angehängt, nie eingefügt:** Die Variantenreihenfolge ist
/// Protokollvertrag, genau wie bei der Nachforderung im Knoten.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Ortsfrage {
    /// Rechne diesen Auftrag.
    Inferenz(Inferenzauftrag),
    /// Lebst du, und für welchen Pipeline-Stand?
    ///
    /// ⚑ **Getrennt von der Arbeit.** Wer die Bereitschaft über einen
    /// Probeauftrag misst, misst die Warteschlange mit und bezahlt
    /// jede Messung mit Rechenzeit.
    Lebenszeichen,
    /// Wer bist du, und wofür soll ich versiegeln?
    ///
    /// ⚑ **Angehängt und nicht in `Lebenszeichen` eingebaut.** Die
    /// Variantenreihenfolge ist Protokollvertrag, und die beiden Fragen
    /// sind verschieden: „lebst du" fragt ein Betreiber im Sekundentakt,
    /// „wer bist du" fragt der Knoten einmal je Epoche.
    Gegenstelle,
    /// Hier sind meine angekündigten Punkte, unterschrieben.
    ///
    /// ⚑ **Rohe Bytes, wie bei [`Ortsantwort::Gegenstelle`]**, und aus
    /// demselben Grund: Der Inhalt ist eine `myl_siegel::Epochenankuendigung`,
    /// und `myl-types` kennt `myl-siegel` nicht. Wer sie braucht,
    /// setzt sie selbst zusammen.
    ///
    /// # ⚑ Warum es diese Frage geben muss (Fund 165, 2026-09-03)
    ///
    /// `Umschlag::oeffnen` braucht die Punkte der Gegenstelle **vor**
    /// dem Entsiegeln; sie können also nicht im Umschlag stecken.
    /// [`Ortsfrage::Gegenstelle`] fragt in die andere Richtung: Der
    /// Knoten erfährt, für wen er versiegelt. **Umgekehrt gab es
    /// nichts**, und deshalb konnte kein Shard je einen echten Umschlag
    /// öffnen. Genau deshalb hatte `Ortsweg::neu` keinen
    /// Produktionsaufrufer: Es gab nichts, wo er sich anschloss.
    ///
    /// # ⚑ Warum eine Unterschrift und nicht bloss zwei Punkte
    ///
    /// Der Ausweis der Leitung sagt „du darfst hereinreden", nicht „du
    /// bist der Knoten". Wer nur Punkte schickte, machte jeden, der den
    /// Ausweis lesen kann, zur Gegenstelle, und das Siegel wäre
    /// Theater. Die Ankündigung ist mit dem **Konsensschlüssel**
    /// unterschrieben, und der Endpunkt ist dessen Hash; der Shard
    /// prüft gegen den Endpunkt, den sein Betreiber ihm genannt hat.
    ///
    /// **Zwei Schichten, und beide werden gebraucht:** die Datei mit
    /// `0600` trägt den Zugang, die Unterschrift die Identität. Der
    /// statische Schlüssel trägt sich dabei nicht selbst, er wird von
    /// einer Signatur des Identitätsschlüssels gedeckt.
    Ankuendigung(Vec<u8>),
}

/// Der grösste erlaubte Umfang einer Ankündigung.
///
/// Eine `Epochenankuendigung` ist rund 1 370 Bytes (Epoche, X25519-Punkt,
/// ML-KEM-768-Kapselpunkt, BLS-Schlüssel, BLS-Signatur). ⚑ **Der Deckel
/// steht trotzdem hier und nicht nur im Rahmen:** Ohne ihn liesse sich
/// [`MAX_NUTZLAST_BYTES`] an Unsinn als Ankündigung einreichen, und der
/// Shard buchstabierte sie durch, bevor er sie verwirft.
pub const MAX_ANKUENDIGUNG_BYTES: usize = 2048;

/// Was zurückkommt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Ortsantwort {
    /// Das Ergebnis oder die Ablehnung.
    Inferenz(Inferenzantwort),
    /// Der Shard-Prozess lebt und nennt seinen Stand.
    Lebenszeichen {
        /// Der Pipeline-Stand, den dieser Prozess geladen hat.
        pipeline: crate::hash::Hash,
        /// Wie viele Shards die Pipeline hat.
        shards: u32,
    },
    /// Der Rahmen war lesbar, die Frage nicht zu beantworten.
    ///
    /// ⚑ **Ohne Grund**, aus demselben Grund wie bei
    /// [`Inferenzantwort::Abgelehnt`]: Wer begründet, verrät seinen
    /// Zustand.
    Abgelehnt,
    /// Der Shard nennt sich und seine angekündigten Punkte.
    ///
    /// ⚑ **Rohe Bytes und keine Siegeltypen.** `myl-types` kennt
    /// `myl-siegel` nicht und soll es nicht: Die Vokabelkiste hängt
    /// sonst an ML-KEM, und das bauen dann alle neunzehn Kisten. Wer
    /// die Punkte braucht, setzt sie selbst zusammen.
    Gegenstelle {
        /// Der Endpunkt des Shards, 32 Bytes.
        endpunkt: [u8; 32],
        /// Der X25519-Punkt der Epoche, 32 Bytes.
        punkt: [u8; 32],
        /// Der ML-KEM-Kapselpunkt.
        kapselpunkt: Vec<u8>,
    },
    /// Die Ankündigung wurde geprüft und gilt.
    ///
    /// ⚑ **Ein Bit und keine Begründung.** Eine abgelehnte Ankündigung
    /// bekommt [`Ortsantwort::Abgelehnt`], wie alles andere; wer
    /// begründete, sagte einem Fremden, welcher Endpunkt erwartet wird.
    Angenommen,
}

/// Was an einem Rahmen nicht stimmt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rahmenfehler {
    /// Noch nicht genug Bytes für den Vorspann.
    Unvollstaendig,
    /// Die Magie stimmt nicht: ein falsch verbundener Klient.
    FremdeMagie,
    /// Der Ausweis stimmt nicht.
    FalscherSchluessel,
    /// Die angekündigte Länge übersteigt [`MAX_NUTZLAST_BYTES`].
    ZuLang { bytes: usize },
    /// Der Vorspann ist gut, die Nutzlast noch nicht vollständig da.
    NutzlastFehlt { erwartet: usize },
}

impl std::fmt::Display for Rahmenfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unvollstaendig => f.write_str("der Vorspann ist noch nicht vollstaendig"),
            Self::FremdeMagie => f.write_str("fremde Magie: kein Rahmen dieser Leitung"),
            Self::FalscherSchluessel => f.write_str("falscher Ausweis"),
            Self::ZuLang { bytes } => write!(
                f,
                "{bytes} Bytes angekuendigt, erlaubt sind {MAX_NUTZLAST_BYTES}"
            ),
            Self::NutzlastFehlt { erwartet } => {
                write!(f, "die Nutzlast fehlt noch, erwartet werden {erwartet} Bytes")
            }
        }
    }
}

impl std::error::Error for Rahmenfehler {}

/// Baut den Rahmen einer Frage.
pub fn rahmen(schluessel: &[u8; SCHLUESSEL_LEN], nutzlast: &[u8]) -> Option<Vec<u8>> {
    if nutzlast.len() > MAX_NUTZLAST_BYTES {
        return None;
    }
    let mut aus = Vec::with_capacity(KOPF_LEN + nutzlast.len());
    aus.extend_from_slice(&MAGIE);
    aus.extend_from_slice(schluessel);
    aus.extend_from_slice(&(nutzlast.len() as u32).to_le_bytes());
    aus.extend_from_slice(nutzlast);
    Some(aus)
}

/// Liest einen Rahmen aus einem Puffer und gibt die Nutzlast zurück.
///
/// ⚑ **Der Ausweis wird in gleichbleibender Zeit verglichen.** Ein
/// Vergleich, der beim ersten falschen Byte aufhört, verrät den
/// Schlüssel Byte für Byte; auf einer lokalen Leitung ist die Messung
/// besonders leicht, weil kein Netz dazwischenrauscht.
///
/// ⚑ **Und die Länge wird erst danach gelesen.** Wer den Ausweis nicht
/// hat, bringt den Empfänger nicht dazu, Speicher zu belegen.
pub fn entrahmen(
    puffer: &[u8],
    schluessel: &[u8; SCHLUESSEL_LEN],
) -> Result<(usize, Vec<u8>), Rahmenfehler> {
    if puffer.len() < KOPF_LEN {
        return Err(Rahmenfehler::Unvollstaendig);
    }
    if puffer[..MAGIE.len()] != MAGIE {
        return Err(Rahmenfehler::FremdeMagie);
    }
    let ab = MAGIE.len();
    let gesehen = &puffer[ab..ab + SCHLUESSEL_LEN];
    if gesehen.ct_eq(&schluessel[..]).unwrap_u8() != 1 {
        return Err(Rahmenfehler::FalscherSchluessel);
    }
    let ab = ab + SCHLUESSEL_LEN;
    let laenge = u32::from_le_bytes([puffer[ab], puffer[ab + 1], puffer[ab + 2], puffer[ab + 3]])
        as usize;
    if laenge > MAX_NUTZLAST_BYTES {
        return Err(Rahmenfehler::ZuLang { bytes: laenge });
    }
    if puffer.len() < KOPF_LEN + laenge {
        return Err(Rahmenfehler::NutzlastFehlt {
            erwartet: KOPF_LEN + laenge,
        });
    }
    Ok((
        KOPF_LEN + laenge,
        puffer[KOPF_LEN..KOPF_LEN + laenge].to_vec(),
    ))
}

/// Baut den Rahmen einer Antwort: ohne Ausweis, denn zurück geht sie
/// über dieselbe offene Verbindung.
pub fn antwortrahmen(nutzlast: &[u8]) -> Option<Vec<u8>> {
    if nutzlast.len() > MAX_NUTZLAST_BYTES {
        return None;
    }
    let mut aus = Vec::with_capacity(MAGIE.len() + 4 + nutzlast.len());
    aus.extend_from_slice(&MAGIE);
    aus.extend_from_slice(&(nutzlast.len() as u32).to_le_bytes());
    aus.extend_from_slice(nutzlast);
    Some(aus)
}

/// Liest den Rahmen einer Antwort.
pub fn antwort_entrahmen(puffer: &[u8]) -> Result<(usize, Vec<u8>), Rahmenfehler> {
    let kopf = MAGIE.len() + 4;
    if puffer.len() < kopf {
        return Err(Rahmenfehler::Unvollstaendig);
    }
    if puffer[..MAGIE.len()] != MAGIE {
        return Err(Rahmenfehler::FremdeMagie);
    }
    let ab = MAGIE.len();
    let laenge = u32::from_le_bytes([puffer[ab], puffer[ab + 1], puffer[ab + 2], puffer[ab + 3]])
        as usize;
    if laenge > MAX_NUTZLAST_BYTES {
        return Err(Rahmenfehler::ZuLang { bytes: laenge });
    }
    if puffer.len() < kopf + laenge {
        return Err(Rahmenfehler::NutzlastFehlt {
            erwartet: kopf + laenge,
        });
    }
    Ok((kopf + laenge, puffer[kopf..kopf + laenge].to_vec()))
}

/// Legt den Ausweis ab, unter Unix mit Rechten `0600`.
///
/// ⚑ **Die Rechte sind die ganze Sicherung**, also werden sie beim
/// Anlegen gesetzt und nicht danach: Zwischen `create` und `chmod` läge
/// ein Fenster, in dem die Datei für jeden lesbar wäre. Unter Unix
/// erledigt das `OpenOptions::mode`; wo es das nicht gibt, sagt der
/// Rückgabewert, dass die Datei ungeschützt liegt.
pub fn schluessel_ablegen(
    pfad: &std::path::Path,
    schluessel: &[u8; SCHLUESSEL_LEN],
) -> std::io::Result<bool> {
    use std::io::Write;
    let mut opt = std::fs::OpenOptions::new();
    opt.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opt.mode(0o600);
    }
    let mut datei = opt.open(pfad)?;
    datei.write_all(schluessel)?;
    datei.flush()?;
    Ok(cfg!(unix))
}

/// Liest den Ausweis.
pub fn schluessel_lesen(pfad: &std::path::Path) -> std::io::Result<[u8; SCHLUESSEL_LEN]> {
    let roh = std::fs::read(pfad)?;
    if roh.len() != SCHLUESSEL_LEN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "der Ausweis hat {} Bytes, erwartet werden {SCHLUESSEL_LEN}",
                roh.len()
            ),
        ));
    }
    let mut aus = [0u8; SCHLUESSEL_LEN];
    aus.copy_from_slice(&roh);
    Ok(aus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::Hash;
    use crate::ids::EpochId;
    use crate::sitzung::Anfragebindung;

    fn schluessel() -> [u8; SCHLUESSEL_LEN] {
        let mut k = [0u8; SCHLUESSEL_LEN];
        for (i, b) in k.iter_mut().enumerate() {
            *b = i as u8;
        }
        k
    }

    fn auftrag() -> Inferenzauftrag {
        Inferenzauftrag {
            sitzung: 5,
            bindung: Anfragebindung::neu(5, b"die frage", EpochId(1)),
            prompt_versiegelt: b"versiegelt".to_vec(),
            max_token: 32,
            pipeline: Hash::sha256(b"pipeline"),
        }
    }

    #[test]
    fn eine_frage_ueberlebt_den_rahmen() {
        let k = schluessel();
        let nutzlast = borsh::to_vec(&Ortsfrage::Inferenz(auftrag())).expect("kodieren");
        let roh = rahmen(&k, &nutzlast).expect("rahmen");
        let (verbraucht, zurueck) = entrahmen(&roh, &k).expect("entrahmen");
        assert_eq!(verbraucht, roh.len(), "der Rahmen wurde nicht ganz gelesen");
        assert_eq!(zurueck, nutzlast);
    }

    /// ⚑ **Der Ausweis ist die Tür.** Ohne ihn kommt niemand herein,
    /// auch nicht mit gültiger Magie und gültiger Nutzlast.
    #[test]
    fn ein_falscher_ausweis_kommt_nicht_durch() {
        let k = schluessel();
        let mut falsch = k;
        falsch[31] ^= 1;
        let roh = rahmen(&falsch, b"egal").expect("rahmen");
        assert_eq!(entrahmen(&roh, &k), Err(Rahmenfehler::FalscherSchluessel));
        // Gegenprobe: mit dem richtigen geht derselbe Rahmen durch.
        let roh = rahmen(&k, b"egal").expect("rahmen");
        assert!(entrahmen(&roh, &k).is_ok(), "der richtige Ausweis kam nicht durch");
    }

    /// ⚑ **Ein Fremder belegt keinen Speicher.** Die Länge steht hinter
    /// dem Ausweis, also fliegt ein falscher Klient auf, bevor seine
    /// Zahl gelesen wird.
    #[test]
    fn die_laenge_wird_erst_nach_dem_ausweis_gelesen() {
        let k = schluessel();
        let mut falsch = k;
        falsch[0] ^= 1;
        let mut roh = rahmen(&falsch, b"x").expect("rahmen");
        // Eine masslose Laenge einsetzen.
        let ab = MAGIE.len() + SCHLUESSEL_LEN;
        roh[ab..ab + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            entrahmen(&roh, &k),
            Err(Rahmenfehler::FalscherSchluessel),
            "die Laenge wurde vor dem Ausweis gelesen"
        );
        // Und mit richtigem Ausweis greift der Deckel.
        let mut roh = rahmen(&k, b"x").expect("rahmen");
        roh[ab..ab + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            entrahmen(&roh, &k),
            Err(Rahmenfehler::ZuLang {
                bytes: u32::MAX as usize
            })
        );
    }

    #[test]
    fn fremde_magie_fliegt_auf() {
        let k = schluessel();
        let mut roh = rahmen(&k, b"x").expect("rahmen");
        roh[0] ^= 1;
        assert_eq!(entrahmen(&roh, &k), Err(Rahmenfehler::FremdeMagie));
    }

    #[test]
    fn ein_halber_rahmen_wartet() {
        let k = schluessel();
        let roh = rahmen(&k, b"eine laengere nutzlast").expect("rahmen");
        assert_eq!(entrahmen(&roh[..4], &k), Err(Rahmenfehler::Unvollstaendig));
        assert_eq!(
            entrahmen(&roh[..roh.len() - 1], &k),
            Err(Rahmenfehler::NutzlastFehlt {
                erwartet: roh.len()
            })
        );
    }

    #[test]
    fn eine_antwort_ueberlebt_ihren_rahmen() {
        let a = Ortsantwort::Lebenszeichen {
            pipeline: Hash::sha256(b"p"),
            shards: 4,
        };
        let nutzlast = borsh::to_vec(&a).expect("kodieren");
        let roh = antwortrahmen(&nutzlast).expect("rahmen");
        let (verbraucht, zurueck) = antwort_entrahmen(&roh).expect("entrahmen");
        assert_eq!(verbraucht, roh.len());
        assert_eq!(
            borsh::from_slice::<Ortsantwort>(&zurueck).expect("dekodieren"),
            a
        );
    }

    /// ⚑ **Zwei Rahmen hintereinander im selben Puffer**, denn ein
    /// Strom kennt keine Nachrichtengrenzen. Ohne die verbrauchte Länge
    /// wüsste der Leser nicht, wo der zweite anfängt.
    #[test]
    fn zwei_rahmen_liegen_hintereinander() {
        let k = schluessel();
        let mut strom = rahmen(&k, b"erste").expect("rahmen");
        strom.extend_from_slice(&rahmen(&k, b"zweite").expect("rahmen"));
        let (n, erste) = entrahmen(&strom, &k).expect("erste");
        assert_eq!(erste, b"erste");
        let (_, zweite) = entrahmen(&strom[n..], &k).expect("zweite");
        assert_eq!(zweite, b"zweite");
    }

    /// ⚑ **Die Rechte sind die ganze Sicherung**, also werden sie
    /// geprüft und nicht angenommen.
    #[test]
    fn der_ausweis_liegt_geschuetzt() {
        let verz = std::env::temp_dir().join(format!("myl-ortsleitung-{}", std::process::id()));
        std::fs::create_dir_all(&verz).expect("Verzeichnis");
        let pfad = verz.join(SCHLUESSEL_DATEI);
        let k = schluessel();
        let geschuetzt = schluessel_ablegen(&pfad, &k).expect("ablegen");
        assert_eq!(schluessel_lesen(&pfad).expect("lesen"), k);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(geschuetzt, "unter Unix muss der Ausweis geschuetzt sein");
            let modus = std::fs::metadata(&pfad).expect("metadaten").permissions().mode();
            assert_eq!(modus & 0o777, 0o600, "der Ausweis liegt mit {modus:o} offen");
        }
        #[cfg(not(unix))]
        assert!(!geschuetzt, "ausserhalb von Unix gibt es keine Zusicherung");
        let _ = std::fs::remove_dir_all(&verz);
    }

    /// Ein zu kurzer oder zu langer Ausweis ist keiner.
    #[test]
    fn ein_ausweis_falscher_laenge_wird_abgewiesen() {
        let verz = std::env::temp_dir().join(format!("myl-ortsleitung-l-{}", std::process::id()));
        std::fs::create_dir_all(&verz).expect("Verzeichnis");
        let pfad = verz.join(SCHLUESSEL_DATEI);
        std::fs::write(&pfad, b"zu kurz").expect("schreiben");
        assert!(schluessel_lesen(&pfad).is_err(), "ein kurzer Ausweis kam durch");
        std::fs::write(&pfad, vec![0u8; SCHLUESSEL_LEN + 1]).expect("schreiben");
        assert!(schluessel_lesen(&pfad).is_err(), "ein langer Ausweis kam durch");
        let _ = std::fs::remove_dir_all(&verz);
    }
}
