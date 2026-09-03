//! Ein HTTP/1.1-Endpunkt ohne Rahmenwerk, für Stufe 1.
//!
//! # ⚑ Warum handgeschrieben, und wo die Grenze liegt
//!
//! Die Entscheidung zu Punkt 39 lautet: **keine HTTP-Bibliothek.** Stufe
//! 1 ist `localhost`, ein Betreiber, ein Weg, kein TLS, kein Zugang.
//! Was ein Rahmenwerk mitbrächte, ist Wegewahl und Mittelschicht für
//! Anforderungen, die es hier nicht gibt.
//!
//! ⚑ **Handgeschriebenes HTTP ist gefährlich, wo es fremde Eingaben mit
//! Zerlegungslogik trifft.** Deshalb steht das Zerlegen hier als
//! **reine Funktion**, ohne Netz, und wird einzeln geprüft; der Teil,
//! der Sockets anfasst, bleibt so dünn, dass an ihm nichts schiefgehen
//! kann. Dieselbe Bauart wie `anfragen_fuer` im Knoten.
//!
//! # Was ausdrücklich abgelehnt wird, statt geraten
//!
//! - **`Transfer-Encoding: chunked`.** Nicht unterstützt und
//!   **ausdrücklich zurückgewiesen**. Es stillschweigend als Rumpf zu
//!   lesen wäre die klassische Schmuggelstelle: Zwei Leser, zwei
//!   Meinungen über die Nachrichtengrenze.
//! - **Fehlendes oder doppeltes `Content-Length`.** Ohne Länge weiß
//!   niemand, wo die Nachricht endet; mit zweien wissen es zwei
//!   verschieden.
//! - **Alles über den Grenzen.** Kopf und Rumpf sind gedeckelt, und der
//!   Deckel ist eine Ablehnung und kein Abschneiden.

/// Höchstlänge des Kopfes, in Bytes.
///
/// Ein Kopf, der größer ist, ist kein Kopf, sondern ein Versuch, den
/// Puffer wachsen zu lassen.
pub const MAX_KOPF: usize = 8 * 1024;

/// Höchstlänge des Rumpfes, in Bytes.
///
/// ⚑ **Gerechnet, nicht geraten:** Eine Anfrage ist ein Prompt. Ein
/// Megabyte sind rund 250 000 Token, weit über jedem Kontextfenster,
/// das dieses Projekt fährt. Wer mehr schickt, schickt keinen Prompt.
pub const MAX_RUMPF: usize = 1024 * 1024;

/// Was an einer Anfrage nicht stimmt.
///
/// ⚑ **Jeder Fall ist eine eigene Aussage.** „Zu groß" und „unlesbar"
/// haben verschiedene Ursachen und verschiedene Antworten; sie in einem
/// Fehler zu bündeln hieße, dem Klienten das Raten zu überlassen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Httpfehler {
    /// Der Kopf ist noch nicht vollständig (kein `\r\n\r\n` gesehen).
    Unvollstaendig,
    /// Der Kopf überschreitet [`MAX_KOPF`].
    KopfZuGross { bytes: usize },
    /// Die Startzeile ist unlesbar.
    Startzeile,
    /// Ein anderes Verfahren als `POST`.
    FalschesVerfahren,
    /// Ein anderer Weg als der erwartete.
    FalscherWeg,
    /// `Content-Length` fehlt, ist doppelt oder keine Zahl.
    Laengenangabe,
    /// `Authorization` ist doppelt, leer oder kein `Bearer`.
    ///
    /// ⚑ **Ein eigener Fall und nicht „unlesbarer Kopf".** Wer zwei
    /// Ausweise schickt, hat ein anderes Problem als wer keinen
    /// schickt, und das Protokoll des Betreibers soll es
    /// unterscheiden können. **Nach draussen sieht man den Unterschied
    /// trotzdem nicht**, siehe `Tuer::abgewiesen`.
    Ausweisangabe,
    /// Der Rumpf überschreitet [`MAX_RUMPF`].
    RumpfZuGross { bytes: usize, grenze: usize },
    /// `Transfer-Encoding` wird nicht unterstützt.
    Stueckweise,
}

impl Httpfehler {
    /// Der Statuscode, den der Klient sehen soll.
    pub fn status(&self) -> u16 {
        match self {
            Self::Unvollstaendig => 400,
            Self::KopfZuGross { .. } => 431,
            Self::Startzeile | Self::Laengenangabe => 400,
            // ⚑ **401 und nicht 400.** Ein fehlerhafter Ausweis ist
            // eine Frage der Berechtigung und keine der Form; ein
            // Klient soll daran erkennen, dass er sich ausweisen muss.
            Self::Ausweisangabe => 401,
            Self::FalschesVerfahren => 405,
            Self::FalscherWeg => 404,
            Self::RumpfZuGross { .. } => 413,
            Self::Stueckweise => 411,
        }
    }
}

/// Eine gelesene Anfrage: der Weg und der Rumpf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anfrage {
    /// Der angefragte Weg.
    pub weg: String,
    /// Der Rumpf, in voller Länge.
    pub rumpf: Vec<u8>,
}

/// Wie viele Bytes der Rumpf haben soll, und wo er beginnt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kopf {
    /// Das Verfahren der Startzeile.
    ///
    /// ⚑ **Es steht seit dem 2026-09-03 hier** (Stufe 3): Die Tür hat
    /// nicht mehr einen Weg, sondern mehrere, und `/v1/models` ist ein
    /// `GET`. Wer nur den Weg vergleicht, verwechselt „Modelle nennen"
    /// mit „Modelle setzen".
    pub verfahren: String,
    /// Der angefragte Weg.
    pub weg: String,
    /// Erstes Byte des Rumpfes in der Eingabe.
    pub rumpf_ab: usize,
    /// Angekündigte Rumpflänge.
    pub laenge: usize,
    /// Der Bearer-Wert aus `Authorization`, falls einer da war.
    ///
    /// ⚑ **Der Weg, den ein Harness gehen kann** (Stufe 2). Jeder
    /// Inferenzanbieter authentifiziert so, und ein Wechsel heisst
    /// Basis-URL und Schlüssel tauschen. Wer signiert statt einen
    /// Bearer zu schicken, geht den anderen Weg.
    ///
    /// **Nur der Wert nach `Bearer `**, nicht die ganze Zeile: Ein
    /// anderes Verfahren (`Basic`, `Digest`) ist keines, das dieses
    /// Gateway kennt, und wird wie ein fehlender Kopf behandelt.
    pub vollmacht: Option<String>,
}

/// Liest den Kopf, ohne auf den Rumpf zu warten.
///
/// Gibt [`Httpfehler::Unvollstaendig`], solange der Kopf nicht ganz da
/// ist: **Das ist kein Fehler, sondern die Aufforderung weiterzulesen**,
/// und der Aufrufer muss die beiden unterscheiden können.
pub fn kopf_lesen(daten: &[u8], erwarteter_weg: &str) -> Result<Kopf, Httpfehler> {
    kopf_lesen_wege(daten, &[("POST", erwarteter_weg)])
}

/// Liest den Kopf und lässt mehrere Verfahren und Wege zu.
///
/// ⚑ **Eine Liste und keine Musterprüfung.** Die Tür kennt genau die
/// Wege, die sie bedient; wer mit Mustern arbeitet, bedient irgendwann
/// einen, den er nicht gemeint hat. Dieselbe Haltung wie beim
/// Beobachtungsendpunkt des Knotens.
///
/// ⚑ **Ein `GET` ohne `Content-Length` ist kein Fehler**, sondern der
/// Normalfall: Ein Abfrageweg hat keinen Rumpf. Bei `POST` bleibt die
/// Längenangabe Pflicht, denn dort ist ihr Fehlen die Frage „wo hört
/// die Nachricht auf".
pub fn kopf_lesen_wege(daten: &[u8], erlaubt: &[(&str, &str)]) -> Result<Kopf, Httpfehler> {
    let ende = match finde_kopfende(daten) {
        Some(e) => e,
        None => {
            return if daten.len() > MAX_KOPF {
                Err(Httpfehler::KopfZuGross { bytes: daten.len() })
            } else {
                Err(Httpfehler::Unvollstaendig)
            }
        }
    };
    if ende > MAX_KOPF {
        return Err(Httpfehler::KopfZuGross { bytes: ende });
    }
    let kopf = core::str::from_utf8(&daten[..ende]).map_err(|_| Httpfehler::Startzeile)?;
    let mut zeilen = kopf.split("\r\n");

    let start = zeilen.next().ok_or(Httpfehler::Startzeile)?;
    let mut teile = start.split(' ');
    let verfahren = teile.next().ok_or(Httpfehler::Startzeile)?;
    let weg = teile.next().ok_or(Httpfehler::Startzeile)?;
    let fassung = teile.next().ok_or(Httpfehler::Startzeile)?;
    if teile.next().is_some() || !fassung.starts_with("HTTP/1.") {
        return Err(Httpfehler::Startzeile);
    }
    // ⚑ **Erst der Weg, dann das Verfahren.** Andersherum bekäme ein
    // `GET` auf einen Weg, den es gar nicht gibt, die Auskunft „falsches
    // Verfahren", und das sagte dem Fragenden, dass der Weg existiert.
    if !erlaubt.iter().any(|(_, w)| *w == weg) {
        return Err(Httpfehler::FalscherWeg);
    }
    if !erlaubt.iter().any(|(v, w)| *w == weg && *v == verfahren) {
        return Err(Httpfehler::FalschesVerfahren);
    }

    let mut laenge: Option<usize> = None;
    let mut vollmacht: Option<String> = None;
    for z in zeilen {
        if z.is_empty() {
            continue;
        }
        let (name, wert) = z.split_once(':').ok_or(Httpfehler::Startzeile)?;
        let name = name.trim().to_ascii_lowercase();
        let wert = wert.trim();
        if name == "transfer-encoding" {
            // ⚑ Nicht raten: Wer stueckweise sendet, bekommt eine
            // Ablehnung und keine halbe Nachricht.
            return Err(Httpfehler::Stueckweise);
        }
        if name == "content-length" {
            if laenge.is_some() {
                // Zwei Laengen heissen zwei Meinungen ueber die
                // Nachrichtengrenze.
                return Err(Httpfehler::Laengenangabe);
            }
            laenge = Some(wert.parse::<usize>().map_err(|_| Httpfehler::Laengenangabe)?);
        }
        if name == "authorization" {
            // ⚑ **Zwei Ausweise heissen zwei Meinungen darueber, wer
            // da ist.** Derselbe Grund wie bei zwei Laengenangaben, und
            // dieselbe Antwort: ablehnen statt einen auswaehlen.
            if vollmacht.is_some() {
                return Err(Httpfehler::Ausweisangabe);
            }
            // Nur `Bearer`, und der Name ist ohne Ruecksicht auf
            // Grossschreibung zu lesen (RFC 7235).
            let Some(rest) = wert.get(..7).filter(|p| p.eq_ignore_ascii_case("bearer ")) else {
                return Err(Httpfehler::Ausweisangabe);
            };
            let _ = rest;
            let token = wert[7..].trim();
            if token.is_empty() {
                return Err(Httpfehler::Ausweisangabe);
            }
            vollmacht = Some(token.to_string());
        }
    }
    let laenge = match laenge {
        Some(l) => l,
        // Ein Abfrageweg hat keinen Rumpf; für alles Schreibende bleibt
        // die Längenangabe Pflicht.
        None if verfahren == "GET" => 0,
        None => return Err(Httpfehler::Laengenangabe),
    };
    if laenge > MAX_RUMPF {
        return Err(Httpfehler::RumpfZuGross {
            bytes: laenge,
            grenze: MAX_RUMPF,
        });
    }
    Ok(Kopf {
        verfahren: verfahren.to_string(),
        weg: weg.to_string(),
        rumpf_ab: ende + 4,
        laenge,
        vollmacht,
    })
}

fn finde_kopfende(daten: &[u8]) -> Option<usize> {
    daten.windows(4).position(|f| f == b"\r\n\r\n")
}

/// Baut eine Antwort.
///
/// **Immer mit `Content-Length` und `Connection: close`.** Stufe 1 hält
/// keine Verbindung offen; wer das ändert, muss auch die Grenze zwischen
/// zwei Nachrichten neu regeln.
pub fn antwort(status: u16, typ: &str, rumpf: &[u8]) -> Vec<u8> {
    let grund = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    };
    let mut aus = format!(
        "HTTP/1.1 {status} {grund}\r\nContent-Type: {typ}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        rumpf.len()
    )
    .into_bytes();
    aus.extend_from_slice(rumpf);
    aus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roh(kopf: &str, rumpf: &str) -> Vec<u8> {
        format!("{kopf}\r\n\r\n{rumpf}").into_bytes()
    }

    #[test]
    fn eine_gewoehnliche_anfrage_wird_gelesen() {
        let d = roh("POST /inferenz HTTP/1.1\r\nContent-Length: 5", "hallo");
        let k = kopf_lesen(&d, "/inferenz").expect("lesbar");
        assert_eq!(k.laenge, 5);
        assert_eq!(&d[k.rumpf_ab..k.rumpf_ab + k.laenge], b"hallo");
    }

    /// ⚑ **Unvollstaendig ist kein Fehler, sondern „lies weiter".**
    #[test]
    fn ein_halber_kopf_heisst_weiterlesen() {
        assert_eq!(
            kopf_lesen(b"POST /inferenz HTTP/1.1\r\nContent-Le", "/inferenz"),
            Err(Httpfehler::Unvollstaendig)
        );
    }

    /// ⚑ **Stueckweise wird abgelehnt, nicht geraten.**
    ///
    /// Es stillschweigend als Rumpf zu lesen waere die klassische
    /// Schmuggelstelle: zwei Leser, zwei Meinungen ueber die
    /// Nachrichtengrenze.
    #[test]
    fn stueckweise_wird_abgelehnt() {
        let d = roh(
            "POST /inferenz HTTP/1.1\r\nTransfer-Encoding: chunked",
            "5\r\nhallo\r\n0\r\n",
        );
        assert_eq!(kopf_lesen(&d, "/inferenz"), Err(Httpfehler::Stueckweise));
    }

    /// ⚑ **Zwei Laengen heissen zwei Meinungen.**
    #[test]
    fn zwei_laengenangaben_werden_abgelehnt() {
        let d = roh(
            "POST /inferenz HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 9",
            "hallo",
        );
        assert_eq!(kopf_lesen(&d, "/inferenz"), Err(Httpfehler::Laengenangabe));
    }

    #[test]
    fn ohne_laengenangabe_wird_abgelehnt() {
        let d = roh("POST /inferenz HTTP/1.1\r\nHost: localhost", "hallo");
        assert_eq!(kopf_lesen(&d, "/inferenz"), Err(Httpfehler::Laengenangabe));
    }

    #[test]
    fn ein_zu_grosser_rumpf_wird_abgelehnt_bevor_er_kommt() {
        let d = roh(
            &format!("POST /inferenz HTTP/1.1\r\nContent-Length: {}", MAX_RUMPF + 1),
            "",
        );
        assert_eq!(
            kopf_lesen(&d, "/inferenz"),
            Err(Httpfehler::RumpfZuGross {
                bytes: MAX_RUMPF + 1,
                grenze: MAX_RUMPF
            })
        );
    }

    /// ⚑ **Ein Kopf ohne Ende waechst nicht mit.**
    #[test]
    fn ein_endloser_kopf_wird_abgebrochen() {
        let d = vec![b'A'; MAX_KOPF + 1];
        assert!(matches!(
            kopf_lesen(&d, "/inferenz"),
            Err(Httpfehler::KopfZuGross { .. })
        ));
    }

    #[test]
    fn falsches_verfahren_und_falscher_weg_sind_verschiedene_befunde() {
        let g = roh("GET /inferenz HTTP/1.1\r\nContent-Length: 0", "");
        assert_eq!(kopf_lesen(&g, "/inferenz"), Err(Httpfehler::FalschesVerfahren));
        let w = roh("POST /anderswo HTTP/1.1\r\nContent-Length: 0", "");
        assert_eq!(kopf_lesen(&w, "/inferenz"), Err(Httpfehler::FalscherWeg));
        assert_ne!(
            Httpfehler::FalschesVerfahren.status(),
            Httpfehler::FalscherWeg.status()
        );
    }

    /// Kopfnamen sind gross- und kleinschreibungsunabhaengig.
    #[test]
    fn die_schreibweise_des_kopfnamens_zaehlt_nicht() {
        let d = roh("POST /inferenz HTTP/1.1\r\ncOnTeNt-LeNgTh: 3", "abc");
        assert_eq!(kopf_lesen(&d, "/inferenz").expect("lesbar").laenge, 3);
    }

    #[test]
    fn eine_antwort_traegt_ihre_laenge() {
        let a = antwort(200, "application/octet-stream", b"xy");
        let s = String::from_utf8_lossy(&a);
        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Length: 2\r\n"));
        assert!(s.contains("Connection: close\r\n"));
        assert!(s.ends_with("\r\n\r\nxy"));
    }
}
