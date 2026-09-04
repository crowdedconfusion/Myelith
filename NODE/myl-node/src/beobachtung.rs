//! Was von außen über diesen Knoten zu sehen ist: Metriken, Leben,
//! Bereitschaft.
//!
//! # ⚑ Warum es das gibt (Fund 129, 2026-09-02)
//!
//! Bis dahin hatte dieses Repositorium **keinen einzigen** Metrik-,
//! Zustands- oder Bereitschaftsendpunkt. Für einen Probelauf, den
//! jemand von Hand startet und dessen Protokolldatei er danach liest,
//! ist das tragbar. Für ein Netz, das jemand **betreibt**, nicht: Wer
//! zwanzig Knoten fährt, liest keine zwanzig Protokolldateien, und wer
//! einen Knoten in ein Kubernetes stellt, wird nach `livenessProbe`
//! und `readinessProbe` gefragt und hat nichts anzubieten.
//!
//! Die Zahlen selbst gab es längst: Die Zustandsaufnahme im Protokoll
//! trägt sie seit Langem. Was fehlte, war ein **Abholweg**. Genau das
//! ist dieses Modul, und mehr ist es nicht.
//!
//! # Die drei Wege, und warum es drei sind
//!
//! | Weg | Frage | Antwort |
//! |---|---|---|
//! | `/metriken` | Wie steht es? | Zahlen im Prometheus-Textformat |
//! | `/gesundheit` | Lebt der Prozess? | 200, solange er antwortet |
//! | `/bereit` | Kann er bedienen? | 200 nur, wenn er auf Stand ist |
//!
//! ⚑ **Leben und Bereitschaft sind nicht dieselbe Frage**, und sie zu
//! einer zu machen ist der häufigste Fehler an dieser Stelle.
//! Kubernetes trennt sie aus gutem Grund: Auf ein totes Leben folgt ein
//! **Neustart**, auf eine fehlende Bereitschaft nur, dass kein Verkehr
//! geschickt wird. Ein Knoten, der aufholt, ist am Leben und nicht
//! bereit. Wer beides zusammenwirft, startet ihn mitten im Aufholen
//! neu, und dann holt er wieder von vorn auf: eine Schleife, die von
//! selbst nicht endet.
//!
//! **Bereit heißt hier: Es ist niemand da, von dem wir wissen, dass er
//! weiter ist.** Also mindestens ein Peer, und keine Höhe gehört, die
//! über der eigenen liegt. Es ist die Frage, für die jede Kette einen
//! Endpunkt hat, meist unter einem Namen wie „hole gerade auf".
//!
//! # ⚑ Die Bindeadresse ist eine Sicherheitsentscheidung
//!
//! **Vorgabe ist `127.0.0.1`, nicht `0.0.0.0`.** Was hier heraussieht,
//! ist eine Landkarte: Peerzahl, Höhe, Latenzspanne, Mesh-Größen je
//! Topic. Für einen Betreiber ist das Diagnose, für einen Angreifer die
//! Aufklärung vor dem Angriff, und zwar ohne einen einzigen
//! Verbindungsversuch ins Protokoll zu schreiben.
//!
//! **Diesen Fehler hat die Messwelt schon durchgemacht:** Ausführer,
//! die ab Werk auf allen Schnittstellen horchten, stehen bis heute
//! zehntausendfach offen im Netz. Wer daraus gelernt hat, bindet seinen
//! Messendpunkt ab Werk auf die Rückschleife, und das ist die richtige
//! Wahl. Wer weiter hinaus will, sagt es ausdrücklich und stellt eine
//! Zugangskontrolle davor; **dieser Endpunkt hat keine**, und das steht
//! auch in der Hilfe.
//!
//! # Warum kein Rahmenwerk
//!
//! Dieselbe Entscheidung wie beim Gateway (Punkt 39): Ein `GET` ohne
//! Rumpf ist der einfachste HTTP-Fall, den es gibt. **Ohne Rumpf gibt
//! es die Schmuggelklasse nicht**, an der handgeschriebenes HTTP sonst
//! scheitert: Es gibt keine zweite Meinung über eine Nachrichtengrenze,
//! wenn nach dem Kopf nichts mehr gelesen wird.
//!
//! Das Zerlegen steht trotzdem als **reine Funktion** ohne Netz und
//! wird einzeln geprüft; der Teil, der Sockets anfasst, bleibt so dünn,
//! dass an ihm nichts schiefgehen kann.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Höchstlänge eines Anfragekopfes, in Bytes.
///
/// Ein `GET /metriken HTTP/1.1` mit ein paar Kopfzeilen ist ein paar
/// hundert Bytes groß. Wer mehr schickt, will nicht abfragen, sondern
/// den Puffer wachsen lassen.
pub const MAX_KOPF: usize = 4 * 1024;

/// Der Inhaltstyp des Prometheus-Textformats.
///
/// ⚑ **Das Semikolon vor `charset` ist keine Kleinigkeit.** Mit einem
/// Komma weist Prometheus die Antwort ab; das ist ein gemeldeter und
/// bestätigter Fehlerfall der Bibliotheken, nicht Theorie. Fehlt die
/// Fassung ganz, fällt der Sammler auf die neueste zurück, und das ist
/// ein Raten, das man ihm ersparen kann.
pub const TYP_METRIKEN: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Was der Knoten von sich preisgibt.
///
/// **Ein einfacher Datensatz, kein Verweis auf den Knoten.** Der Knoten
/// gehört seiner Ereignisschleife; ihn mit einem Netzdienst zu teilen
/// hieße, eine Sperre über ein `await` zu halten. Stattdessen legt er
/// hier bei jeder Zustandsaufnahme eine Kopie ab, und der Dienst liest
/// nur diese.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Beobachtungsstand {
    /// Wann dieser Stand entstand, in Millisekunden seit der Epoche.
    pub stand_ms: u64,
    pub hoehe: u64,
    /// Die höchste Höhe, von der dieser Knoten gehört hat.
    pub hoechste_gehoerte: u64,
    pub peers: u64,
    pub wartend: u64,
    pub schlecht_bewertet: u64,
    pub protokollzeilen: u64,
    pub konsens_vorlauf: u64,
    pub konsens_vorlauf_verworfen: u64,
    pub kette_schreibfehler: u64,
    pub kette_lesefehler: u64,
    pub kette_gespeichert: u64,
    pub latenz_messungen: u64,
    pub latenz_min_us: u64,
    pub latenz_max_us: u64,
    /// Ob dieser Knoten gerade Bloecke nachfordert.
    ///
    /// ⚑ **Die Frage eines Betreibers lautet nicht „ist er wach“,
    /// sondern „holt er noch auf“.** Ein Knoten mit niedriger Hoehe
    /// und laufender Nachforderung arbeitet; einer mit niedriger Hoehe
    /// und ohne Nachforderung haengt.
    pub nachforderung_laeuft: bool,
}

impl Beobachtungsstand {
    /// Ob dieser Knoten bedienen kann.
    ///
    /// **Zwei Bedingungen, beide notwendig.** Ohne Peer weiß er nichts
    /// über die Welt und kann seine eigene Höhe nicht einordnen; kennt
    /// er eine höhere Höhe, fehlt ihm etwas, und was er ausliefert,
    /// wäre veraltet.
    pub fn bereit(&self) -> bool {
        self.peers > 0 && self.hoechste_gehoerte <= self.hoehe
    }
}

/// Der geteilte Ablageort für den Stand.
///
/// Der Knoten schreibt, der Dienst liest. Eine `Mutex` genügt: Die
/// Sperre wird nur für das Kopieren eines kleinen Datensatzes gehalten,
/// nie über ein `await`.
#[derive(Debug, Clone, Default)]
pub struct Beobachtungsstelle {
    stand: Arc<Mutex<Beobachtungsstand>>,
}

impl Beobachtungsstelle {
    /// Eine leere Stelle.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Legt einen neuen Stand ab.
    ///
    /// **Eine vergiftete Sperre wird übergangen, nicht weitergereicht.**
    /// Der Knoten soll an einer Metrik nicht sterben; siehe Fund 128 zur
    /// selben Frage im Pod.
    pub fn setzen(&self, neu: Beobachtungsstand) {
        if let Ok(mut g) = self.stand.lock() {
            *g = neu;
        }
    }

    /// Holt den letzten Stand.
    pub fn holen(&self) -> Beobachtungsstand {
        self.stand
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

/// Der Weg aus einer HTTP-Anfrage, oder `None`.
///
/// **Rein, ohne Netz, einzeln geprüft.** Alles, was schiefgehen kann,
/// geht hier schief und nicht in einer Socket-Schleife.
///
/// `None` heißt: keine erste Zeile, kein `GET`, oder ein Kopf über
/// [`MAX_KOPF`]. **Nur `GET`**, denn ein Abfragedienst hat nichts
/// entgegenzunehmen.
pub fn weg_aus_anfrage(daten: &[u8]) -> Option<String> {
    if daten.len() > MAX_KOPF {
        return None;
    }
    let ende = daten
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .unwrap_or(daten.len());
    let kopf = std::str::from_utf8(&daten[..ende]).ok()?;
    let erste = kopf.lines().next()?;
    let mut teile = erste.split(' ');
    if teile.next()? != "GET" {
        return None;
    }
    let weg = teile.next()?;
    // Eine Abfragezeichenkette abschneiden: `/metriken?x=1` ist
    // derselbe Weg. Sonst führte ein angehängtes Fragezeichen zu 404,
    // und mancher Sammler hängt eines an.
    let weg = weg.split('?').next().unwrap_or(weg);
    Some(weg.to_string())
}

/// Baut eine Antwort.
///
/// Immer mit Länge und `Connection: close`: Dieser Dienst hält keine
/// Verbindung offen, und wer das ändert, muss die Grenze zwischen zwei
/// Nachrichten neu regeln.
pub fn antwort(status: u16, typ: &str, rumpf: &str) -> Vec<u8> {
    let grund = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let mut aus = format!(
        "HTTP/1.1 {status} {grund}\r\nContent-Type: {typ}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        rumpf.len()
    )
    .into_bytes();
    aus.extend_from_slice(rumpf.as_bytes());
    aus
}

/// Der Stand als Prometheus-Text.
///
/// **Namen nach der Übereinkunft:** ein gemeinsames Präfix, Einheiten
/// im Namen, Zähler auf `_total`. Wer sich nicht daran hält, bekommt
/// Warnungen von jedem Linter der Prometheus-Welt und Verwirrung bei
/// jedem, der die Zahlen später aggregiert.
pub fn als_prometheus(stand: &Beobachtungsstand, jetzt_ms: u64) -> String {
    let mut aus = String::with_capacity(2048);
    let mut zeile = |name: &str, hilfe: &str, art: &str, wert: u64| {
        aus.push_str(&format!("# HELP {name} {hilfe}\n# TYPE {name} {art}\n{name} {wert}\n"));
    };

    zeile("myelith_kette_hoehe", "Hoehe der eigenen Kette.", "gauge", stand.hoehe);
    zeile(
        "myelith_kette_hoechste_gehoerte_hoehe",
        "Hoechste Hoehe, von der dieser Knoten gehoert hat.",
        "gauge",
        stand.hoechste_gehoerte,
    );
    zeile(
        "myelith_kette_rueckstand",
        "Wie viele Bloecke fehlen, soweit bekannt.",
        "gauge",
        stand.hoechste_gehoerte.saturating_sub(stand.hoehe),
    );
    zeile("myelith_peers", "Verbundene Gegenstellen.", "gauge", stand.peers);
    zeile(
        "myelith_mempool_wartend",
        "Transaktionen, die auf einen Block warten.",
        "gauge",
        stand.wartend,
    );
    zeile(
        "myelith_peers_schlecht_bewertet",
        "Gegenstellen mit schlechter Bewertung.",
        "gauge",
        stand.schlecht_bewertet,
    );
    zeile(
        "myelith_konsens_vorlauf",
        "Konsensnachrichten, die auf ihre Runde warten.",
        "gauge",
        stand.konsens_vorlauf,
    );
    zeile(
        "myelith_nachforderung_laeuft",
        "Ob dieser Knoten gerade Bloecke nachfordert (1) oder nicht (0).",
        "gauge",
        u64::from(stand.nachforderung_laeuft),
    );
    zeile(
        "myelith_kette_gespeicherte_bloecke",
        "Bloecke in der Kettendatei.",
        "gauge",
        stand.kette_gespeichert,
    );
    zeile(
        "myelith_protokollzeilen_total",
        "Geschriebene Protokollzeilen seit dem Start.",
        "counter",
        stand.protokollzeilen,
    );
    zeile(
        "myelith_konsens_vorlauf_verworfen_total",
        "Verworfene Konsensnachrichten seit dem Start.",
        "counter",
        stand.konsens_vorlauf_verworfen,
    );
    zeile(
        "myelith_kette_schreibfehler_total",
        "Fehlgeschlagene Schreibversuche auf die Kettendatei.",
        "counter",
        stand.kette_schreibfehler,
    );
    zeile(
        "myelith_kette_lesefehler_total",
        "Fehlgeschlagene Leseversuche aus der Kettendatei.",
        "counter",
        stand.kette_lesefehler,
    );
    // Die Latenzspanne beschreibt das Fenster seit der vorigen Aufnahme
    // und wird dort zurueckgesetzt. Ohne Messungen gibt es keine Spanne,
    // und dann steht hier auch keine: Eine Null waere eine Aussage, die
    // niemand gemacht hat.
    zeile(
        "myelith_latenz_messungen",
        "Latenzmessungen im letzten Aufnahmefenster.",
        "gauge",
        stand.latenz_messungen,
    );
    if stand.latenz_messungen > 0 {
        zeile(
            "myelith_latenz_min_mikrosekunden",
            "Kleinste gemessene Latenz im letzten Aufnahmefenster.",
            "gauge",
            stand.latenz_min_us,
        );
        zeile(
            "myelith_latenz_max_mikrosekunden",
            "Groesste gemessene Latenz im letzten Aufnahmefenster.",
            "gauge",
            stand.latenz_max_us,
        );
    }
    zeile(
        "myelith_bereit",
        "1, wenn dieser Knoten bedienen kann, sonst 0.",
        "gauge",
        u64::from(stand.bereit()),
    );
    // ⚑ **Wie alt die Zahlen sind, gehoert dazu.** Der Stand entsteht
    // im Takt der Zustandsaufnahme; ein Sammler, der oefter fragt,
    // bekommt denselben Wert mehrfach. Ohne diese Zahl sieht ein
    // stehengebliebener Knoten aus wie ein ruhiger.
    zeile(
        "myelith_stand_alter_millisekunden",
        "Wie alt der abgelegte Stand ist.",
        "gauge",
        jetzt_ms.saturating_sub(stand.stand_ms),
    );
    aus
}

/// Beantwortet eine Anfrage. **Rein, ohne Netz.**
pub fn bedienen(weg: Option<&str>, stand: &Beobachtungsstand, jetzt_ms: u64) -> Vec<u8> {
    match weg {
        Some("/metriken") => antwort(200, TYP_METRIKEN, &als_prometheus(stand, jetzt_ms)),
        // Leben heisst: Dieser Prozess antwortet. Mehr kann eine
        // Antwort ueber sich selbst nicht aussagen.
        Some("/gesundheit") => antwort(200, "text/plain; charset=utf-8", "lebt\n"),
        Some("/bereit") => {
            if stand.bereit() {
                antwort(200, "text/plain; charset=utf-8", "bereit\n")
            } else {
                // 503 und nicht 200 mit Text: Ein Lastverteiler liest
                // den Status, nicht den Rumpf.
                antwort(
                    503,
                    "text/plain; charset=utf-8",
                    "nicht bereit: ohne Peers oder im Rueckstand\n",
                )
            }
        }
        Some(_) => antwort(404, "text/plain; charset=utf-8", "unbekannter Weg\n"),
        // Kein GET, unlesbar oder zu gross.
        None => antwort(405, "text/plain; charset=utf-8", "nur GET\n"),
    }
}

/// Horcht und beantwortet, bis die Aufgabe abgeräumt wird.
///
/// Der dünne Teil: Er liest bis zum Kopfende oder bis [`MAX_KOPF`],
/// ruft [`bedienen`] und schließt. **Kein Zustand, keine Entscheidung**;
/// beides steht in den reinen Funktionen darüber.
pub async fn laufen(lauscher: TcpListener, stelle: Beobachtungsstelle) {
    loop {
        let Ok((mut strom, _)) = lauscher.accept().await else {
            // Ein fehlgeschlagenes `accept` ist kein Grund, den Dienst
            // aufzugeben: Oft ist es nur eine Gegenstelle, die vor dem
            // Handschlag ging.
            continue;
        };
        let stelle = stelle.clone();
        tokio::spawn(async move {
            let mut puffer = Vec::with_capacity(512);
            let mut stueck = [0u8; 512];
            // Lesen, bis der Kopf steht. Der Deckel ist eine Ablehnung
            // und kein Abschneiden: Wer mehr schickt, bekommt 405.
            loop {
                if puffer.windows(4).any(|f| f == b"\r\n\r\n") || puffer.len() > MAX_KOPF {
                    break;
                }
                match strom.read(&mut stueck).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => puffer.extend_from_slice(&stueck[..n]),
                }
            }
            let weg = weg_aus_anfrage(&puffer);
            let stand = stelle.holen();
            let aus = bedienen(weg.as_deref(), &stand, crate::protokoll::jetzt_ms().max(0) as u64);
            let _ = strom.write_all(&aus).await;
            let _ = strom.shutdown().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stand() -> Beobachtungsstand {
        Beobachtungsstand {
            stand_ms: 1_000,
            hoehe: 42,
            hoechste_gehoerte: 42,
            peers: 3,
            wartend: 1,
            protokollzeilen: 900,
            kette_gespeichert: 42,
            latenz_messungen: 5,
            nachforderung_laeuft: true,
            latenz_min_us: 300,
            latenz_max_us: 9_000,
            ..Default::default()
        }
    }

    #[test]
    fn ein_get_wird_zerlegt() {
        assert_eq!(
            weg_aus_anfrage(b"GET /metriken HTTP/1.1\r\nHost: x\r\n\r\n").as_deref(),
            Some("/metriken")
        );
    }

    /// ⚑ **Nur GET.** Ein Abfragedienst hat nichts entgegenzunehmen,
    /// und was er nicht entgegennimmt, kann ihm auch niemand
    /// unterschieben.
    #[test]
    fn andere_verfahren_werden_abgewiesen() {
        for roh in [
            &b"POST /metriken HTTP/1.1\r\n\r\n"[..],
            &b"PUT /metriken HTTP/1.1\r\n\r\n"[..],
            &b"DELETE /bereit HTTP/1.1\r\n\r\n"[..],
        ] {
            assert_eq!(weg_aus_anfrage(roh), None, "durchgelassen: {roh:?}");
        }
    }

    /// Ein Kopf ueber der Grenze ist ein Versuch, den Puffer wachsen zu
    /// lassen, und keine Anfrage.
    #[test]
    fn ein_zu_grosser_kopf_wird_abgewiesen() {
        let mut roh = b"GET /metriken HTTP/1.1\r\n".to_vec();
        roh.extend(std::iter::repeat_n(b'x', MAX_KOPF));
        assert_eq!(weg_aus_anfrage(&roh), None);
    }

    #[test]
    fn eine_abfragezeichenkette_aendert_den_weg_nicht() {
        assert_eq!(
            weg_aus_anfrage(b"GET /metriken?ts=1 HTTP/1.1\r\n\r\n").as_deref(),
            Some("/metriken")
        );
    }

    /// ⚑ **Ein Ja/Nein wird zu 1 und 0**, nicht zu `true` und `false`.
    /// Prometheus kennt nur Zahlen; ein Wort in der Wertspalte macht
    /// die Zeile unlesbar, und zwar still.
    #[test]
    fn ein_zustand_wird_als_eins_oder_null_ausgegeben() {
        let mut s = stand();
        s.nachforderung_laeuft = true;
        let t = als_prometheus(&s, 1_500);
        assert!(t.contains("\nmyelith_nachforderung_laeuft 1\n"), "{t}");
        s.nachforderung_laeuft = false;
        let t = als_prometheus(&s, 1_500);
        assert!(t.contains("\nmyelith_nachforderung_laeuft 0\n"), "{t}");
    }

    #[test]
    fn der_text_traegt_hilfe_typ_und_wert() {
        let t = als_prometheus(&stand(), 1_500);
        assert!(t.contains("# HELP myelith_kette_hoehe"));
        assert!(t.contains("# TYPE myelith_kette_hoehe gauge"));
        assert!(t.contains("\nmyelith_kette_hoehe 42\n"));
        assert!(t.contains("\nmyelith_stand_alter_millisekunden 500\n"));
        // Die letzte Zeile endet auf einen Zeilenvorschub: Das verlangt
        // das Format ausdruecklich.
        assert!(t.ends_with('\n'));
    }

    /// ⚑ **Zaehler heissen `_total`, Messwerte nicht.** Die
    /// Uebereinkunft ist keine Kosmetik: Wer sie bricht, bekommt bei
    /// jeder Aggregation eine falsche Rate.
    #[test]
    fn die_namen_folgen_der_uebereinkunft() {
        let t = als_prometheus(&stand(), 1_500);
        for zeile in t.lines() {
            let Some(rest) = zeile.strip_prefix("# TYPE ") else {
                continue;
            };
            let (name, art) = rest.split_once(' ').expect("Name und Art");
            assert!(name.starts_with("myelith_"), "{name} traegt kein Praefix");
            match art {
                "counter" => assert!(name.ends_with("_total"), "{name} ist ein Zaehler ohne _total"),
                "gauge" => assert!(!name.ends_with("_total"), "{name} ist ein Messwert auf _total"),
                anderes => panic!("unerwartete Art {anderes}"),
            }
        }
    }

    /// ⚑ **Leben und Bereitschaft sind zwei Fragen** (siehe Modulkopf).
    #[test]
    fn ein_knoten_im_rueckstand_lebt_und_ist_nicht_bereit() {
        let mut s = stand();
        s.hoechste_gehoerte = 100;
        assert!(!s.bereit());

        let leben = String::from_utf8(bedienen(Some("/gesundheit"), &s, 1_500)).unwrap();
        assert!(leben.starts_with("HTTP/1.1 200 "), "ein aufholender Knoten lebt");

        let bereit = String::from_utf8(bedienen(Some("/bereit"), &s, 1_500)).unwrap();
        assert!(
            bereit.starts_with("HTTP/1.1 503 "),
            "ein aufholender Knoten darf nicht bereit melden"
        );
    }

    /// Ohne Peer weiss der Knoten nichts ueber die Welt, also auch
    /// nicht, ob seine Hoehe die aktuelle ist.
    #[test]
    fn ohne_peers_ist_niemand_bereit() {
        let mut s = stand();
        s.peers = 0;
        assert!(!s.bereit());
    }

    #[test]
    fn ein_unbekannter_weg_ist_404_und_kein_verfahren_ist_405() {
        let s = stand();
        let vier = String::from_utf8(bedienen(Some("/anderswo"), &s, 1)).unwrap();
        assert!(vier.starts_with("HTTP/1.1 404 "));
        let fuenf = String::from_utf8(bedienen(None, &s, 1)).unwrap();
        assert!(fuenf.starts_with("HTTP/1.1 405 "));
    }

    /// ⚑ **Das Semikolon vor `charset`, nicht ein Komma.** Mit einem
    /// Komma weist Prometheus die Antwort ab; das ist ein bestaetigter
    /// Fehlerfall und keine Theorie.
    #[test]
    fn der_inhaltstyp_ist_der_vorgeschriebene() {
        let a = String::from_utf8(bedienen(Some("/metriken"), &stand(), 1)).unwrap();
        assert!(
            a.contains("Content-Type: text/plain; version=0.0.4; charset=utf-8\r\n"),
            "der Inhaltstyp stimmt nicht: {}",
            a.lines().take(4).collect::<Vec<_>>().join(" | ")
        );
    }

    #[test]
    fn die_stelle_gibt_zurueck_was_hineinging() {
        let stelle = Beobachtungsstelle::neu();
        assert_eq!(stelle.holen(), Beobachtungsstand::default());
        stelle.setzen(stand());
        assert_eq!(stelle.holen(), stand());
    }
}
