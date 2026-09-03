//! Der Weg des Knotens zum Shard-Prozess (GATEWAY Stufe 4).
//!
//! # ⚑ Warum der Knoten nicht selbst rechnet
//!
//! Entscheidung vom 2026-09-03: **Ein Shard läuft in einem eigenen
//! Prozess.** Ein Absturz beim Rechnen soll den Konsens nicht anhalten,
//! und ein Modell im Adressraum der Konsensmaschine hiesse, beide teilen
//! sich jeden Fehler. **Dieselbe Trennung, die K0 für die Tür
//! verlangt.**
//!
//! Die Gegenseite steht in `myl_pod::ortsdienst`, das Protokoll in
//! [`myl_types::ortsleitung`], damit es nur eine Quelle dafür gibt.
//!
//! # ⚑ Die Frist ist die Zusage an den Fragenden
//!
//! Ein Auftrag, der beim Shard hängt, darf den Fragenden im Netz nicht
//! auf eine Zeitüberschreitung laufen lassen: Die sagt ihm nichts, und
//! er weiss danach nicht, ob er neu fragen soll. **Nach [`FRIST`]
//! antwortet der Knoten selbst mit `Abgelehnt`**, und das ist eine
//! Auskunft.
//!
//! # ⚑ Und nichts davon läuft in der Ereignisschleife
//!
//! Der Aufruf ist `async` und wird von `Knoten` in eine eigene Aufgabe
//! gegeben. Ein blockierender Aufruf an dieser Stelle hielte die
//! Blockverarbeitung an, solange der Shard rechnet, und das sind
//! Sekunden bis Minuten.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use myl_types::ortsleitung::{
    antwort_entrahmen, rahmen, schluessel_lesen, Ortsantwort, Ortsfrage, Rahmenfehler,
    SCHLUESSEL_DATEI, SCHLUESSEL_LEN,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Wie lange auf die Antwort des Shard-Prozesses gewartet wird.
///
/// ⚑ **Kürzer als die Leerlauffrist der Netzverbindung (60 s).** Sonst
/// wäre die Verbindung zum Fragenden weg, bevor die Antwort da ist, und
/// der Knoten rechnete für einen Papierkorb.
pub const FRIST: Duration = Duration::from_secs(45);

/// Der Anschluss an einen lokalen Shard-Prozess.
#[derive(Debug, Clone)]
pub struct Ortsanschluss {
    adresse: SocketAddr,
    schluessel: [u8; SCHLUESSEL_LEN],
}

impl Ortsanschluss {
    /// Liest den Ausweis und merkt sich die Adresse.
    ///
    /// `ausweis` ist entweder die Datei selbst oder das Verzeichnis, in
    /// dem der Shard-Prozess sie abgelegt hat.
    ///
    /// ⚑ **Der Ausweis wird beim Start gelesen und nicht bei jeder
    /// Frage.** Eine Datei, die im heissen Pfad liegt, ist eine
    /// Fehlerquelle im heissen Pfad. Startet der Shard neu, hat er
    /// einen neuen Ausweis, und der Knoten muss ebenfalls neu starten;
    /// das gehört gesagt statt angenommen.
    pub fn neu(adresse: SocketAddr, ausweis: &Path) -> std::io::Result<Self> {
        let pfad = if ausweis.is_dir() {
            ausweis.join(SCHLUESSEL_DATEI)
        } else {
            ausweis.to_path_buf()
        };
        Ok(Self {
            adresse,
            schluessel: schluessel_lesen(&pfad)?,
        })
    }

    /// Die Adresse des Shard-Prozesses.
    pub fn adresse(&self) -> SocketAddr {
        self.adresse
    }

    /// Stellt eine Frage und wartet höchstens [`FRIST`] auf die Antwort.
    ///
    /// `None` heisst: nicht erreicht, nicht lesbar oder zu spät. **Der
    /// Grund bleibt hier**, denn was der Fragende im Netz bekommt, ist
    /// in jedem Fall dieselbe Ablehnung; eine feinere Auskunft nach
    /// aussen wäre ein Auskunftsdienst über den Zustand dieses Knotens.
    pub async fn frage(&self, frage: &Ortsfrage) -> Option<Ortsantwort> {
        self.frage_mit_frist(frage, FRIST).await
    }

    /// Wie [`Self::frage`], mit einer anderen Frist.
    ///
    /// ⚑ **Nur für Tests.** Eine Frist von 45 Sekunden lässt sich nicht
    /// in einem Test abwarten, und eine Frist, die kein Test je
    /// erreicht, ist eine Zusicherung ohne Deckung.
    #[doc(hidden)]
    pub async fn frage_mit_frist(
        &self,
        frage: &Ortsfrage,
        frist: Duration,
    ) -> Option<Ortsantwort> {
        tokio::time::timeout(frist, self.frage_ohne_frist(frage))
            .await
            .ok()
            .flatten()
    }

    async fn frage_ohne_frist(&self, frage: &Ortsfrage) -> Option<Ortsantwort> {
        let nutzlast = borsh::to_vec(frage).ok()?;
        let roh = rahmen(&self.schluessel, &nutzlast)?;
        let mut strom = tokio::net::TcpStream::connect(self.adresse).await.ok()?;
        strom.write_all(&roh).await.ok()?;
        strom.flush().await.ok()?;
        let mut puffer = Vec::new();
        let mut stueck = [0u8; 8192];
        loop {
            // ⚑ **Die Fehlerart wird gelesen, nicht nur das Gelingen.**
            // Eine Fassung dieser Schleife prüfte nur auf `Ok` und las
            // sonst weiter: Ein Shard-Prozess, der eine masslose Länge
            // ankündigt, brachte den Knoten damit dazu, **ein Megabyte
            // Müll zu sammeln**, bevor ein Notdeckel griff. „Lokal"
            // heisst nicht „vertrauenswürdig ohne Grenze".
            match antwort_entrahmen(&puffer) {
                Ok((_, n)) => return borsh::from_slice::<Ortsantwort>(&n).ok(),
                Err(Rahmenfehler::FremdeMagie) | Err(Rahmenfehler::ZuLang { .. }) => return None,
                Err(Rahmenfehler::FalscherSchluessel) => return None,
                Err(Rahmenfehler::Unvollstaendig) | Err(Rahmenfehler::NutzlastFehlt { .. }) => {}
            }
            // **Und damit ist der Puffer gebunden**, ohne zweite
            // Grenze: `NutzlastFehlt` kommt nur, solange weniger als
            // `KOPF_LEN + laenge` Bytes da sind, und `laenge` liegt
            // unter [`MAX_NUTZLAST_BYTES`], sonst wäre es `ZuLang`.
            match strom.read(&mut stueck).await {
                Ok(0) | Err(_) => return None,
                Ok(n) => puffer.extend_from_slice(&stueck[..n]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::ortsleitung::schluessel_ablegen;

    fn verzeichnis(marke: &str) -> std::path::PathBuf {
        let v = std::env::temp_dir().join(format!("myl-ortsklient-{marke}-{}", std::process::id()));
        std::fs::create_dir_all(&v).expect("Verzeichnis");
        v
    }

    /// ⚑ **Verzeichnis oder Datei, beides muss gehen.** Ein Betreiber
    /// gibt mal das eine an, mal das andere; wer nur eines annimmt,
    /// bekommt eine Fehlermeldung über eine Datei, die es gibt.
    #[test]
    fn der_ausweis_wird_als_datei_und_als_verzeichnis_gefunden() {
        let v = verzeichnis("ausweis");
        let k = [7u8; SCHLUESSEL_LEN];
        schluessel_ablegen(&v.join(SCHLUESSEL_DATEI), &k).expect("ablegen");
        let adr: SocketAddr = "127.0.0.1:4170".parse().expect("Adresse");
        assert_eq!(
            Ortsanschluss::neu(adr, &v).expect("ueber das Verzeichnis").schluessel,
            k
        );
        assert_eq!(
            Ortsanschluss::neu(adr, &v.join(SCHLUESSEL_DATEI))
                .expect("ueber die Datei")
                .schluessel,
            k
        );
        let _ = std::fs::remove_dir_all(&v);
    }

    /// Ohne Ausweis kein Anschluss, und zwar beim Start und nicht bei
    /// der ersten Frage.
    #[test]
    fn ohne_ausweis_gibt_es_keinen_anschluss() {
        let v = verzeichnis("ohne");
        let adr: SocketAddr = "127.0.0.1:4170".parse().expect("Adresse");
        assert!(Ortsanschluss::neu(adr, &v).is_err(), "ein Anschluss ohne Ausweis");
        let _ = std::fs::remove_dir_all(&v);
    }

    /// ⚑ **Ein Shard, der nicht da ist, kostet keine Ewigkeit.**
    #[tokio::test]
    async fn ein_toter_shard_liefert_nichts() {
        let v = verzeichnis("tot");
        schluessel_ablegen(&v.join(SCHLUESSEL_DATEI), &[1u8; SCHLUESSEL_LEN]).expect("ablegen");
        // Port 1 ist auf keiner Maschine ein Shard-Prozess.
        let a = Ortsanschluss::neu("127.0.0.1:1".parse().expect("Adresse"), &v).expect("Anschluss");
        assert_eq!(a.frage(&Ortsfrage::Lebenszeichen).await, None);
        let _ = std::fs::remove_dir_all(&v);
    }

    /// ⚑ **Ein Shard, der eine masslose Länge ankündigt, wird sofort
    /// fallengelassen** und nicht bis zu einem Notdeckel weitergelesen.
    ///
    /// Der Unterschied ist messbar: Wer die Fehlerart nicht liest,
    /// wartet hier bis zur Frist, weil nach dem Vorspann nichts mehr
    /// kommt. Wer sie liest, ist sofort fertig.
    #[tokio::test]
    async fn eine_masslose_laenge_wird_sofort_fallengelassen() {
        use myl_types::ortsleitung::MAGIE;
        let v = verzeichnis("masslos");
        let k = [3u8; SCHLUESSEL_LEN];
        schluessel_ablegen(&v.join(SCHLUESSEL_DATEI), &k).expect("ablegen");
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adr = horcher.local_addr().expect("Adresse");
        std::thread::spawn(move || {
            let mut offen = Vec::new();
            while let Ok((mut s, _)) = horcher.accept() {
                use std::io::Write;
                let mut kopf = Vec::new();
                kopf.extend_from_slice(&MAGIE);
                kopf.extend_from_slice(&u32::MAX.to_le_bytes());
                let _ = s.write_all(&kopf);
                let _ = s.flush();
                // Danach nichts mehr, die Verbindung bleibt offen.
                offen.push(s);
            }
        });
        let a = Ortsanschluss::neu(adr, &v).expect("Anschluss");
        let begonnen = std::time::Instant::now();
        let antwort = a
            .frage_mit_frist(&Ortsfrage::Lebenszeichen, Duration::from_secs(3))
            .await;
        let gedauert = begonnen.elapsed();
        assert_eq!(antwort, None, "eine masslose Laenge lieferte eine Antwort");
        assert!(
            gedauert < Duration::from_secs(1),
            "der Klient hat weitergelesen statt abzubrechen: {gedauert:?}"
        );
        let _ = std::fs::remove_dir_all(&v);
    }

    /// ⚑ **Ein Shard, der annimmt und schweigt, hält den Knoten nicht
    /// fest.** Das ist der gefährlichere Fall als ein toter Port: Die
    /// Verbindung steht, es kommt nur nie etwas zurück.
    #[tokio::test]
    async fn ein_schweigender_shard_laeuft_in_die_frist() {
        let v = verzeichnis("stumm");
        let k = [2u8; SCHLUESSEL_LEN];
        schluessel_ablegen(&v.join(SCHLUESSEL_DATEI), &k).expect("ablegen");
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adr = horcher.local_addr().expect("Adresse");
        // Nimmt an und schweigt. Die Verbindung wird gehalten, sonst
        // pruefte der Test einen geschlossenen Socket.
        std::thread::spawn(move || {
            let mut offen = Vec::new();
            while let Ok((s, _)) = horcher.accept() {
                offen.push(s);
            }
        });
        let a = Ortsanschluss::neu(adr, &v).expect("Anschluss");
        let begonnen = std::time::Instant::now();
        let antwort = a
            .frage_mit_frist(&Ortsfrage::Lebenszeichen, Duration::from_millis(300))
            .await;
        let gedauert = begonnen.elapsed();
        assert_eq!(antwort, None, "ein schweigender Shard lieferte eine Antwort");
        assert!(
            gedauert < Duration::from_secs(5),
            "die Frist hat nicht gegriffen: {gedauert:?}"
        );
        let _ = std::fs::remove_dir_all(&v);
    }
}
