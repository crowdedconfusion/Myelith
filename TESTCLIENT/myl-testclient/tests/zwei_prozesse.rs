//! Zwei echte Prozesse, ein Auftrag: der Beweis zu den Funden 165 und 169.
//!
//! # ⚑ Was dieser Test kann, was der Gesamtlauf nicht kann
//!
//! `gesamtlauf.rs` baut alles in **einem** Prozess zusammen, von Hand.
//! Er beweist, dass die Teile zusammenpassen, und er hat genau deshalb
//! die Funde 165 und 169 nicht gefunden: Er spielte selbst die Rolle,
//! die in der Produktion niemand spielte.
//!
//! Hier läuft der Shard als **eigenes Programm**, gestartet über seine
//! Kommandozeile, und der Rechenweg entsteht durch
//! [`myl_node::rechenweg::fuer_betreiber`], also durch **dieselbe
//! Funktion, die `myl-node` aufruft**. Was hier grün ist, ist beim
//! Betreiber grün.
//!
//! # Der Weg
//!
//! 1. Ein Konsensschlüssel entsteht in einer Datei. Sein Hash ist der
//!    Endpunkt, den der Shard als `--knoten` erwartet.
//! 2. `myl-pod-node` startet als **Dienst**: lokale Tür, frischer
//!    Ausweis, echtes Modell.
//! 3. `fuer_betreiber` liest den Ausweis, baut die Ankündigung und
//!    hängt den Abrechnungskanal an.
//! 4. Ein Harness ruft `/v1/chat/completions` mit Bearer.
//! 5. Der Knoten kündigt sich an, der Shard prüft die Unterschrift und
//!    kennt ab jetzt die Punkte des Knotens; erst dadurch geht der
//!    Umschlag auf.
//! 6. Vier Shards rechnen, die Antwort kommt zurück, die Abrechnung
//!    liegt im Kanal.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use myl_consensus::block::Anweisung;
use myl_gateway::annahme::Annahme;
use myl_gateway::oai::WEG_CHAT;
use myl_gateway::tuer::Tuer;
use myl_gateway::zugang::{Kontraktquelle, Zugangsstelle};
use myl_node::schluessel::Konsensschluessel;
use myl_types::ids::{EpochId, SitzungId};
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand};
use myl_types::vollmacht::{Vollmacht, Vorbehalt};
use myl_types::Address;

const PIPELINE_DIGEST: &str = "c42bb8a8d85bba5a76b3302298903fb5c1edfe4463c5d1d44256bef447ffd5c9";
const POD_HEX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn wurzel() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn artefakte() -> PathBuf {
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    wurzel().join("INTEGER_LLM").join("artifacts").join(modell)
}

/// Das gebaute Shard-Binary.
///
/// ⚑ **Fehlt es, schlägt der Test fehl und sagt, was zu tun ist.** Ein
/// stiller Sprung sähe aus wie ein bestandener Test, und dieser hier
/// belegt, dass es den Dienst überhaupt gibt (Fund 169).
fn shard_binary() -> PathBuf {
    for profil in ["debug", "release"] {
        let p = wurzel().join("target-shared").join(profil).join("myl-pod-node");
        if p.exists() {
            return p;
        }
    }
    panic!(
        "myl-pod-node ist nicht gebaut.\n\
         Dieser Test belegt, dass der Shard ein Dienst ist, und braucht dafuer das Programm:\n\
         cd COMPUTE_PIPELINE/myl-pod && cargo build --bin myl-pod-node"
    )
}

fn verzeichnis(zweck: &str) -> PathBuf {
    let v = std::env::temp_dir().join(format!(
        "myl-zweiprozesse-{zweck}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&v).expect("Verzeichnis");
    v
}

/// Beendet den Kindprozess auch dann, wenn eine Zusicherung reisst.
struct Kind(Child);
impl Drop for Kind {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct EineQuelle(Sitzungskontrakt);
impl Kontraktquelle for EineQuelle {
    fn nachschlagen(&self, s: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        (s == self.0.adresse()).then(|| (self.0.clone(), Sitzungszustand::neu()))
    }
}

fn hex32(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn post_bearer(weg: &str, token: &str, rumpf: &[u8]) -> Vec<u8> {
    let mut aus = format!(
        "POST {weg} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        rumpf.len()
    )
    .into_bytes();
    aus.extend_from_slice(rumpf);
    aus
}

fn rumpf_von(antwort: &[u8]) -> &[u8] {
    antwort
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .map(|i| &antwort[i + 4..])
        .unwrap_or(&[])
}

/// ⚑ **Ein Harness, ein Knoten, ein eigener Shard-Prozess.**
///
/// ⛑ **Die Gegenprobe steckt im Aufbau.** Nimmt man `--knoten` einen
/// falschen Endpunkt, weist der Shard die Ankündigung ab, `rechne`
/// bricht vor dem Versiegeln ab, und die Tür antwortet mit 502. Genau
/// dieser Fall steht als zweiter Test darunter.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_aufruf_geht_durch_zwei_prozesse() {
    let Some(aufbau) = Aufbau::starten(true).await else {
        return;
    };
    let (kopf, doc) = aufbau.frage("Die Hauptstadt von Frankreich ist").await;
    assert!(kopf.starts_with("HTTP/1.1 200"), "{kopf}");

    let inhalt = doc["choices"][0]["message"]["content"].as_str().expect("content");
    let neue = doc["usage"]["completion_tokens"].as_u64().expect("completion_tokens");
    let prompt_token = doc["usage"]["prompt_tokens"].as_u64().expect("prompt_tokens");
    assert!(!inhalt.trim().is_empty(), "das Modell hat nichts erzeugt");
    assert!(neue > 0 && neue <= 8, "Token ausserhalb des Deckels: {neue}");
    assert!(prompt_token > 0 && prompt_token < 20, "prompt_tokens sieht nach Bytes aus");
    assert_eq!(doc["myelith_deterministisch"].as_bool(), Some(true));

    // ⚑ **Und die Abrechnung liegt im Kanal.** `mit_abrechnung` hatte
    // bis zum 2026-09-03 keinen Produktionsaufrufer;
    // `fuer_betreiber` hängt ihn jetzt an, und das ist hier belegt.
    let anweisung = aufbau
        .abrechnung()
        .expect("es wurde keine Abrechnung abgelegt");
    let Anweisung::SitzungAusgeben { vorhaben, vollmacht } = &anweisung else {
        panic!("die abgelegte Anweisung ist keine Ausgabe");
    };
    assert_eq!(vorhaben.betrag, neue.max(1), "es wurde nicht die geleistete Arbeit gebucht");
    assert!(vollmacht.is_some(), "ohne Vollmacht kann die Kette nicht autorisieren");

    eprintln!("\n  Frage:   Die Hauptstadt von Frankreich ist");
    eprintln!("  Antwort: {}", inhalt.trim());
    eprintln!("  Gebucht: {} Credits\n", vorhaben.betrag);
}

/// ⚑ **Ein Shard, der einen anderen Knoten erwartet, rechnet nicht.**
///
/// **Das ist die Zeile, an der die ganze Ausweisschicht hängt.** Der
/// Ausweis der Leitung sagt „du darfst hereinreden"; er sagt nicht „du
/// bist der Knoten". Ohne die Prüfung der Ankündigung wäre jeder, der
/// die Ausweisdatei lesen kann, die Gegenstelle, und das Siegel wäre
/// Theater.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_fremder_knoten_bekommt_nichts_gerechnet() {
    let Some(aufbau) = Aufbau::starten(false).await else {
        return;
    };
    let (kopf, _) = aufbau.frage("Die Hauptstadt von Frankreich ist").await;
    assert!(
        !kopf.starts_with("HTTP/1.1 200"),
        "der Shard hat fuer einen fremden Endpunkt gerechnet: {kopf}"
    );
    assert!(
        aufbau.abrechnung().is_none(),
        "es wurde abgebucht, obwohl nichts gerechnet wurde"
    );
}

/// Beide Prozesse, der Rechenweg und die Tür, startbereit.
struct Aufbau {
    _kind: Kind,
    _ausweis: PathBuf,
    _schluessel: PathBuf,
    tuer: Tuer,
    port: u16,
    kontrakt: Sitzungskontrakt,
    bearer: String,
    weg: myl_node::rechenweg::Ortsweg,
    empfang: std::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<Anweisung>>,
}

impl Aufbau {
    /// `echt` heisst: Der Shard erwartet genau diesen Knoten.
    async fn starten(echt: bool) -> Option<Self> {
        let dir = artefakte();
        if !myl_pod::artefakte::vorhanden(&dir) {
            return None;
        }
        let bin = shard_binary();
        let ausweis = verzeichnis("ausweis");
        let schluessel = verzeichnis("schluessel");

        // --- 1. Die Identitaet des Knotens --------------------------
        let konsens =
            Konsensschluessel::neu_erzeugen(&schluessel.join("knoten.konsens.key"))
                .expect("Konsensschluessel");
        // ⚑ Im Fremdfall bekommt der Shard den Endpunkt eines **anderen**
        // Schluessels genannt. Alles andere bleibt gleich, auch der
        // Ausweis: Es geht genau um die eine Frage.
        let erwartet = if echt {
            hex32(konsens.endpunkt().bytes())
        } else {
            let fremd = Konsensschluessel::neu_erzeugen(&schluessel.join("fremd.konsens.key"))
                .expect("fremder Schluessel");
            hex32(fremd.endpunkt().bytes())
        };

        // --- 2. Der Shard als eigener Prozess -----------------------
        let mut prozess = Command::new(&bin)
            .arg("--artefakte")
            .arg(&dir)
            .arg("--ausweis")
            .arg(&ausweis)
            .arg("--pod")
            .arg(POD_HEX)
            .arg("--knoten")
            .arg(&erwartet)
            .arg("--pipeline")
            .arg(PIPELINE_DIGEST)
            // Port null: Das Betriebssystem sucht einen freien, und der
            // Dienst sagt, welchen. Ein fester Port liesse zwei Läufe
            // nebeneinander scheitern.
            .arg("--ortsleitung")
            .arg("127.0.0.1:0")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("myl-pod-node startet");

        // ⚑ **Den Port aus der Ausgabe lesen.** Damit ist zugleich
        // geprüft, dass der Dienst sagt, wo er horcht; ein Betreiber
        // braucht die Zeile genauso.
        let aus = prozess.stdout.take().expect("stdout");
        let mut leser = std::io::BufReader::new(aus);
        let mut zeile = String::new();
        let mut adresse = None;
        for _ in 0..40 {
            zeile.clear();
            if leser.read_line(&mut zeile).unwrap_or(0) == 0 {
                break;
            }
            if let Some(rest) = zeile.trim().strip_prefix("[myl-pod] Shard-Dienst auf ") {
                adresse = rest.parse::<std::net::SocketAddr>().ok();
                break;
            }
        }
        let kind = Kind(prozess);
        let adresse = adresse.expect("der Shard-Dienst hat seine Adresse nicht genannt");

        // --- 3. Der Rechenweg, gebaut wie in `myl-node` -------------
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let betreiber = myl_node::kette::konto_fuer("probe-betreiber");
        let weg = myl_node::rechenweg::fuer_betreiber(
            Some(adresse),
            Some(ausweis.as_path()),
            Some(pod_bytes()),
            "myelith-qwen2.5-0.5b",
            Some(&konsens),
            EpochId(0),
            betreiber,
            tx,
        )
        .expect("der Rechenweg entsteht");

        // --- 4. Kontrakt und Vollmacht ------------------------------
        let agent_sk = myl_types::bls::BlsSecretKey::key_gen(&[5u8; 32]).expect("Schluessel");
        let agent = Address::aus_schluessel(&agent_sk.public_key().expect("pk"));
        let kontrakt = Sitzungskontrakt::neu(
            Address::new([1u8; 32]),
            agent,
            Grenzen {
                budget: 10_000,
                einzellimit: 1_000,
                schwelle: u64::MAX,
                zeugenleiter: Vec::new(),
            },
            Grenzen::gesperrt(),
            vec![betreiber],
            EpochId(0),
            EpochId(100),
            1_000,
        )
        .expect("gueltiger Kontrakt");
        let bearer = Vollmacht::ausstellen(
            &agent_sk,
            vec![
                Vorbehalt::NurSitzung(kontrakt.adresse()),
                Vorbehalt::GueltigBis(EpochId(100)),
            ],
            [3u8; 32],
        )
        .expect("ausstellen")
        .als_bearer();

        let tuer = Tuer::binden(0).await.expect("binden");
        let port = tuer.port().expect("port");
        Some(Self {
            _kind: kind,
            _ausweis: ausweis,
            _schluessel: schluessel,
            tuer,
            port,
            kontrakt,
            bearer,
            weg,
            empfang: std::sync::Mutex::new(rx),
        })
    }

    async fn frage(&self, was: &str) -> (String, serde_json::Value) {
        let koerper = format!(
            r#"{{"model":"myelith-qwen2.5-0.5b","messages":[{{"role":"user","content":"{was}"}}],"max_tokens":8}}"#
        );
        let bytes = post_bearer(WEG_CHAT, &self.bearer, koerper.as_bytes());
        let port = self.port;
        let mut annahme = Annahme::neu(41, EpochId(0));
        let mut stelle = Zugangsstelle::neu(EineQuelle(self.kontrakt.clone()));
        let dienst = async {
            let _ = self
                .tuer
                .bedienen_v1(&mut annahme, &mut stelle, &self.weg, EpochId(0), 1_700_000_000_000)
                .await;
        };
        let klient = tokio::task::spawn_blocking(move || {
            let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
            strom
                .set_read_timeout(Some(Duration::from_secs(900)))
                .expect("Frist");
            strom.write_all(&bytes).expect("senden");
            strom.flush().expect("leeren");
            let mut aus = Vec::new();
            let _ = strom.read_to_end(&mut aus);
            aus
        });
        let (antwort, _) = tokio::join!(klient, dienst);
        let antwort = antwort.expect("Klient");
        let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(24)]).to_string();
        let doc = serde_json::from_slice(rumpf_von(&antwort)).unwrap_or(serde_json::Value::Null);
        (kopf, doc)
    }

    fn abrechnung(&self) -> Option<Anweisung> {
        self.empfang.lock().ok()?.try_recv().ok()
    }
}

fn pod_bytes() -> [u8; 32] {
    let mut b = [0u8; 32];
    for (i, p) in b.iter_mut().enumerate() {
        *p = u8::from_str_radix(&POD_HEX[i * 2..i * 2 + 2], 16).expect("hex");
    }
    b
}
