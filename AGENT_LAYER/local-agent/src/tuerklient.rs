//! Der Anfrageweg zur eigenen Tür (AGENT_LAYER 5.1).
//!
//! # ⚑ Ein gewöhnlicher Klient und kein Sonderweg
//!
//! Die Tür des eigenen Knotens spricht die OpenAI-Form. Das war der
//! Zweck der Gateway-Stufe 3, und dieser Klient nimmt ihn beim Wort: Er
//! kennt eine Adresse, eine Vollmacht und einen Weg, sonst nichts. Kein
//! Kettenzustand, kein Schlüssel, keine Signatur.
//!
//! # ⚑ Warum die Typen hier stehen und nicht aus `myl-gateway` kommen
//!
//! Der naheliegende Griff wäre `myl_gateway::oai::Chatanfrage`. **Er
//! wäre falsch herum**, aus zwei Gründen.
//!
//! **Erstens die Grenze.** Dieses Harness soll wenig kennen; wer das
//! Gateway einbindet, hat den Server im Klienten, und die
//! Abhängigkeitsliste ist wieder länger als die Grenze erlaubt
//! (siehe [`crate::VERBOTENE_KISTEN`]).
//!
//! **Zweitens, und das ist der schwerere Grund: Zwei unabhängige
//! Fassungen sind der Beleg.** Die Behauptung lautet, dass ein
//! **gewöhnlicher** OpenAI-Klient diese Tür erreicht. Ein Klient, der
//! die Typen des Servers benutzt, kann sie nicht belegen; er würde auch
//! dann noch passen, wenn beide gemeinsam von der Form abgewichen
//! wären. Zwei getrennt geschriebene Fassungen, die sich im Test
//! treffen, belegen sie.
//!
//! ⚑ **Und der Treffpunkt liegt woanders**: in `myl-testclient`, der
//! einzigen Stelle, die Tür und Harness zugleich sieht. Hier steht der
//! Klient und ein Stummel-Server, der ihn auf die Probe stellt.
//!
//! # ⚑ Die Länge wird gelesen, nicht das Ende der Verbindung
//!
//! Drei Testdateien im Repositorium lasen die Antwort bisher mit
//! `read_to_end`, verlassen sich also darauf, dass die Gegenseite
//! auflegt. Das geht gut, solange sie es tut. **Ein Klient, der eine
//! Vollmacht trägt und eine Abrechnung auslöst, darf nicht daran
//! hängen**, ob der Server `Connection: keep-alive` beherrscht: Er läse
//! bis zur Frist und meldete eine Zeitüberschreitung für eine Antwort,
//! die längst vollständig da war.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Eine Nachricht der Unterhaltung.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Nachricht {
    /// `system`, `user` oder `assistant`.
    pub role: String,
    /// ⚑ **Nur Text.** Die OpenAI-Form erlaubt hier auch Bild- und
    /// Tonteile; Myelith rechnet Text, und die Tür lehnt anderes ab.
    pub content: String,
}

impl Nachricht {
    /// Eine Nutzernachricht.
    pub fn nutzer(text: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: text.into() }
    }
    /// Eine Systemnachricht.
    pub fn system(text: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: text.into() }
    }
}

/// Was der Klient sendet.
#[derive(Debug, Clone, Serialize)]
struct Chatanfrage<'a> {
    model: &'a str,
    messages: &'a [Nachricht],
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct Wahl {
    message: Nachricht,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Verbrauch {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct Rohantwort {
    choices: Vec<Wahl>,
    #[serde(default)]
    usage: Verbrauch,
    #[serde(default)]
    id: String,
    /// Die **Segmentkennung** des Konsens, 32 Bytes als Hex.
    ///
    /// ⚑ **Nicht dasselbe wie `id`**, und das ist der Grund, warum
    /// dieses Feld seit dem 2026-09-05 gelesen wird. `id` ist eine
    /// Anzeigekennung („myl-42"); die Kette nach Kap. 8.4 hängt an der
    /// **SegmentId**, also `h(sitzung ‖ index)`. Der Klient hat sie bis
    /// dahin weggeworfen, und ein Sitzungsstrom, der sich auf sie
    /// beruft, hätte sie nicht gehabt.
    #[serde(default)]
    myelith_segment: String,
}

/// Was zurückkam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antwort {
    /// Der erzeugte Text.
    pub text: String,
    /// Warum die Erzeugung endete, falls die Tür es sagt.
    pub abschlussgrund: Option<String>,
    /// Die Kennung, unter der die Tür den Auftrag festgeschrieben hat.
    ///
    /// ⚑ **Sie gehört ins Protokoll des Harness.** Wer später prüfen
    /// will, was gerechnet wurde, braucht sie; ohne sie hat der Nutzer
    /// eine Antwort und keinen Beleg.
    ///
    /// **Zum Anzeigen**, siehe [`Antwort::segment`] für die Kennung, an
    /// der die Kette hängt.
    pub kennung: String,
    /// Die **Segmentkennung** des Konsens, falls die Tür sie nennt.
    ///
    /// ⚑ **Daran hängt die Kette nach Kap. 8.4**, nicht an
    /// [`Antwort::kennung`]. `None` heisst: Diese Tür nennt sie nicht,
    /// und dann lässt sich für diesen Schritt kein Kettenglied bilden.
    /// **Das ist eine Aussage und keine Auslassung**; wer sie überginge,
    /// bekäme eine Kette mit erfundenen Gliedern.
    pub segment: Option<[u8; 32]>,
    /// Token im Prompt.
    pub prompt_token: u32,
    /// Erzeugte Token.
    pub antwort_token: u32,
}

/// Was schiefgehen kann.
///
/// ⚑ **Der Status wird unterschieden und nicht eingeebnet.** Ein
/// Harness, das jede Nicht-200 gleich behandelt, kann „die Vollmacht
/// ist abgelaufen" nicht von „kein Guthaben" und nicht von „der Knoten
/// rechnet gerade" trennen. Der Mensch davor soll erfahren, was zu tun
/// ist, und das ist je nach Zahl etwas anderes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tuerfehler {
    /// Die Tür ist nicht erreichbar.
    Unerreichbar { grund: String },
    /// Die Tür hat innerhalb der Frist nicht geantwortet.
    Zeitueberschreitung { frist_ms: u64 },
    /// Die Antwort ist kein HTTP, das dieser Klient versteht.
    KeinHttp,
    /// Die Tür hat abgelehnt.
    Abgelehnt { status: u16, rumpf: String },
    /// Der Rumpf ist kein JSON dieser Form.
    Unlesbar { rumpf: String },
    /// Die Tür hat keine Wahl geliefert.
    OhneWahl,
}

impl std::fmt::Display for Tuerfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unerreichbar { grund } => {
                write!(f, "die Tuer ist nicht erreichbar: {grund}. Laeuft der Knoten?")
            }
            Self::Zeitueberschreitung { frist_ms } => write!(
                f,
                "keine Antwort in {frist_ms} ms. Ein geshardeter Lauf braucht Zeit; \
                 eine laengere Frist setzen oder pruefen, ob der Pod rechnet"
            ),
            Self::KeinHttp => f.write_str("die Antwort ist kein HTTP/1.1, das dieser Klient liest"),
            Self::Abgelehnt { status, rumpf } => match status {
                401 => write!(f, "401: die Vollmacht gilt hier nicht ({rumpf})"),
                402 => write!(f, "402: der Sitzungskontrakt traegt das nicht ({rumpf})"),
                429 => write!(f, "429: der Knoten nimmt gerade nichts an ({rumpf})"),
                _ => write!(f, "{status}: {rumpf}"),
            },
            Self::Unlesbar { rumpf } => write!(f, "der Rumpf ist kein lesbares JSON: {rumpf}"),
            Self::OhneWahl => f.write_str("die Antwort enthaelt keine Wahl"),
        }
    }
}

impl std::error::Error for Tuerfehler {}

/// Ein Klient für die Tür eines Knotens.
///
/// ⚑ **Er hält eine Vollmacht und sonst nichts.** Das ist die ganze
/// Berührung dieses Harness mit der Kette, siehe Modulkopf von
/// [`crate`].
#[derive(Debug, Clone)]
pub struct Tuerklient {
    wirt: String,
    port: u16,
    vollmacht: String,
    frist: Duration,
}

/// Die Vorgabefrist.
///
/// ⚑ **Zehn Minuten, und das ist nicht großzügig, sondern gemessen.**
/// Ein geshardeter Lauf über vier Prozesse und ein 0,5B-Modell braucht
/// auf einer bescheidenen Maschine Sekunden bis Minuten; die
/// Testläufe des Repositoriums setzen 600 Sekunden. Eine kurze Frist
/// hier hiesse, eine laufende und bereits bezahlte Rechnung
/// wegzuwerfen.
pub const FRIST_VORGABE: Duration = Duration::from_secs(600);

impl Tuerklient {
    /// Ein Klient gegen `wirt:port` mit dieser Vollmacht.
    pub fn neu(wirt: impl Into<String>, port: u16, vollmacht: impl Into<String>) -> Self {
        Self {
            wirt: wirt.into(),
            port,
            vollmacht: vollmacht.into(),
            frist: FRIST_VORGABE,
        }
    }

    /// Gegen die Tür auf dieser Maschine.
    pub fn oertlich(vollmacht: impl Into<String>) -> Self {
        Self::neu("127.0.0.1", crate::TUER_PORT, vollmacht)
    }

    /// Setzt die Frist.
    pub fn mit_frist(mut self, frist: Duration) -> Self {
        self.frist = frist;
        self
    }

    /// Eine Vervollständigung.
    pub fn chat(
        &self,
        modell: &str,
        nachrichten: &[Nachricht],
        max_tokens: Option<u32>,
    ) -> Result<Antwort, Tuerfehler> {
        let anfrage = Chatanfrage { model: modell, messages: nachrichten, max_tokens };
        // Kann nicht scheitern: drei Felder aus Zeichenketten und Zahlen.
        let rumpf = serde_json::to_vec(&anfrage).unwrap_or_default();
        let (status, antwort) = self.senden(crate::WEG_CHAT, &rumpf)?;
        if status != 200 {
            return Err(Tuerfehler::Abgelehnt {
                status,
                rumpf: String::from_utf8_lossy(&antwort).into_owned(),
            });
        }
        let roh: Rohantwort = serde_json::from_slice(&antwort).map_err(|_| Tuerfehler::Unlesbar {
            rumpf: String::from_utf8_lossy(&antwort).chars().take(200).collect(),
        })?;
        let wahl = roh.choices.into_iter().next().ok_or(Tuerfehler::OhneWahl)?;
        Ok(Antwort {
            text: wahl.message.content,
            abschlussgrund: wahl.finish_reason,
            segment: hex32(&roh.myelith_segment),
            kennung: roh.id,
            prompt_token: roh.usage.prompt_tokens,
            antwort_token: roh.usage.completion_tokens,
        })
    }

    /// Eine POST-Anfrage, und die Antwort als (Status, Rumpf).
    fn senden(&self, weg: &str, rumpf: &[u8]) -> Result<(u16, Vec<u8>), Tuerfehler> {
        let mut strom = TcpStream::connect((self.wirt.as_str(), self.port))
            .map_err(|e| Tuerfehler::Unerreichbar { grund: e.to_string() })?;
        strom
            .set_read_timeout(Some(self.frist))
            .map_err(|e| Tuerfehler::Unerreichbar { grund: e.to_string() })?;

        let mut aus = format!(
            "POST {weg} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.wirt,
            self.vollmacht,
            rumpf.len()
        )
        .into_bytes();
        aus.extend_from_slice(rumpf);
        strom
            .write_all(&aus)
            .and_then(|()| strom.flush())
            .map_err(|e| Tuerfehler::Unerreichbar { grund: e.to_string() })?;

        self.lesen(&mut strom)
    }

    /// Liest eine HTTP/1.1-Antwort: Statuszeile, Köpfe, dann genau so
    /// viele Rumpfbytes, wie `Content-Length` sagt.
    fn lesen(&self, strom: &mut TcpStream) -> Result<(u16, Vec<u8>), Tuerfehler> {
        let mut roh: Vec<u8> = Vec::new();
        let mut puffer = [0u8; 4096];
        // Erst bis zum Ende der Köpfe.
        let kopfende = loop {
            if let Some(i) = finde_leerzeile(&roh) {
                break i;
            }
            match strom.read(&mut puffer) {
                Ok(0) => return Err(Tuerfehler::KeinHttp),
                Ok(n) => roh.extend_from_slice(&puffer[..n]),
                Err(e) if ist_frist(&e) => {
                    return Err(Tuerfehler::Zeitueberschreitung {
                        frist_ms: self.frist.as_millis() as u64,
                    })
                }
                Err(e) => return Err(Tuerfehler::Unerreichbar { grund: e.to_string() }),
            }
        };
        let kopf = String::from_utf8_lossy(&roh[..kopfende]).into_owned();
        let status = statuszeile(&kopf).ok_or(Tuerfehler::KeinHttp)?;
        let laenge = inhaltslaenge(&kopf);

        let mut rumpf = roh[kopfende + 4..].to_vec();
        // ⚑ **Mit Längenangabe genau so viele Bytes, ohne sie bis zum
        // Auflegen.** Die Tür schickt immer eine; die zweite Hälfte ist
        // für jeden anderen Server, gegen den ein Harness zeigen könnte.
        loop {
            if let Some(n) = laenge {
                if rumpf.len() >= n {
                    rumpf.truncate(n);
                    return Ok((status, rumpf));
                }
            }
            match strom.read(&mut puffer) {
                Ok(0) => {
                    if let Some(n) = laenge {
                        if rumpf.len() < n {
                            // ⚑ **Abgeschnitten, und das ist ein Fehler.**
                            // Ein halber Rumpf ergibt kein JSON; ihn
                            // durchzureichen hiesse, „unlesbar" zu melden,
                            // wo „die Verbindung brach" gemeint ist.
                            return Err(Tuerfehler::Unerreichbar {
                                grund: format!(
                                    "die Verbindung brach nach {} von {n} Rumpfbytes",
                                    rumpf.len()
                                ),
                            });
                        }
                    }
                    return Ok((status, rumpf));
                }
                Ok(n) => rumpf.extend_from_slice(&puffer[..n]),
                Err(e) if ist_frist(&e) => {
                    return Err(Tuerfehler::Zeitueberschreitung {
                        frist_ms: self.frist.as_millis() as u64,
                    })
                }
                Err(e) => return Err(Tuerfehler::Unerreichbar { grund: e.to_string() }),
            }
        }
    }
}

/// 64 Hexzeichen zu 32 Bytes, oder nichts.
///
/// ⚑ **Nichts statt Nullen.** Eine Kennung, die aus Nullen besteht,
/// sähe aus wie eine Kennung; `None` sagt, dass keine da war.
fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut aus = [0u8; 32];
    for (i, b) in aus.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(aus)
}

fn ist_frist(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

fn finde_leerzeile(roh: &[u8]) -> Option<usize> {
    roh.windows(4).position(|f| f == b"\r\n\r\n")
}

/// Die Zahl aus `HTTP/1.1 200 OK`.
fn statuszeile(kopf: &str) -> Option<u16> {
    let erste = kopf.lines().next()?;
    let mut teile = erste.split_whitespace();
    let fassung = teile.next()?;
    if !fassung.starts_with("HTTP/") {
        return None;
    }
    teile.next()?.parse().ok()
}

/// `Content-Length`, gross oder klein geschrieben.
fn inhaltslaenge(kopf: &str) -> Option<usize> {
    kopf.lines()
        .find(|z| z.to_ascii_lowercase().starts_with("content-length:"))
        .and_then(|z| z.split(':').nth(1))
        .and_then(|w| w.trim().parse().ok())
}
