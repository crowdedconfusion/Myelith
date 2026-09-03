//! Die Tür, über einen echten Socket.
//!
//! ⚑ **Ohne diese Tests wäre der Endpunkt eine Vermutung.** Die
//! Zerlegung ist einzeln geprüft; was hier geprüft wird, ist der Weg
//! **durch** einen Socket: dass eine Anfrage ankommt, dass ein Beleg
//! zurückkommt, und dass die Ablehnungen ablehnen statt zu schweigen.

use myl_gateway::annahme::{Annahme, Beleg};
use myl_gateway::{Tuer, WEG};
use myl_types::ids::EpochId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Schickt rohe Bytes und liest die ganze Antwort.
///
/// ⚑ **Die Schreibseite wird geschlossen, und das ist nicht Kosmetik.**
/// Ohne `shutdown` wartet der Server auf den angekündigten Rest, während
/// der Klient auf die Antwort wartet: **ein Deadlock, und der erste Lauf
/// ist darin hängengeblieben.** Er hat damit gezeigt, dass der Server
/// einen abgebrochenen Rumpf am Dateiende erkennt und nicht an einer
/// Frist; das ist die richtige Reihenfolge.
///
/// Die Frist darüber ist der zweite Riegel: Ein hängender Test sagt
/// nichts, ein fehlgeschlagener sagt etwas.
async fn frage(port: u16, roh: &[u8]) -> Vec<u8> {
    let arbeit = async {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.expect("verbinden");
        s.write_all(roh).await.expect("senden");
        s.flush().await.expect("leeren");
        s.shutdown().await.expect("Schreibseite schliessen");
        let mut aus = Vec::new();
        s.read_to_end(&mut aus).await.expect("lesen");
        aus
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), arbeit)
        .await
        .expect("die Tuer hat innerhalb von fuenf Sekunden nicht geantwortet")
}

fn post(weg: &str, rumpf: &str) -> Vec<u8> {
    format!(
        "POST {weg} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{rumpf}",
        rumpf.len()
    )
    .into_bytes()
}

fn rumpf_von(antwort: &[u8]) -> &[u8] {
    let i = antwort
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .expect("Kopfende");
    &antwort[i + 4..]
}

#[tokio::test]
async fn eine_anfrage_bekommt_einen_beleg() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "was ist ein pod")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");

    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 200 OK"),
        "keine 200er-Antwort: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(60)])
    );
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert_eq!(beleg.sitzung, 1);
    // ⚑ **Der Beleg passt zu der Frage, die gestellt wurde**, und zu
    // keiner anderen. Genau das ist sein Zweck.
    assert!(beleg.bindung.passt(b"was ist ein pod"));
    assert!(!beleg.bindung.passt(b"was ist ein Pod"));
}

/// ⚑ **Ein falscher Weg wird abgelehnt, nicht stillschweigend bedient.**
#[tokio::test]
async fn ein_falscher_weg_bekommt_404() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post("/anderswo", "x")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 404"));
}

/// ⚑ **Stückweise wird abgelehnt, und der Klient erfährt es.**
///
/// Schweigen ließe ihn auf eine Zeitüberschreitung warten, die ihm
/// nichts sagt; stillschweigend als Rumpf zu lesen wäre die
/// Schmuggelstelle.
#[tokio::test]
async fn stueckweise_bekommt_eine_ablehnung() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let roh = format!("POST {WEG} HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
        .into_bytes();
    let auftrag = tokio::spawn(async move { frage(port, &roh).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 411"));
}

/// ⚑ **Eine leere Anfrage verbraucht keine Sitzungsnummer.**
#[tokio::test]
async fn eine_leere_anfrage_wird_abgelehnt() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 400"));
    // Die naechste echte Anfrage bekommt trotzdem die Eins.
    let auftrag = tokio::spawn(async move { frage(port, &post(WEG, "echt")).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert_eq!(beleg.sitzung, 1);
}

/// ⚑ **Ein Rumpf, der kürzer ankommt als angekündigt, wird abgelehnt.**
///
/// Ihn als kurze Anfrage zu nehmen hieße, eine andere Frage
/// festzuschreiben als die gestellte.
#[tokio::test]
async fn ein_abgebrochener_rumpf_wird_abgelehnt() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(9));

    let roh = format!("POST {WEG} HTTP/1.1\r\nContent-Length: 99\r\n\r\nnurdrei").into_bytes();
    let auftrag = tokio::spawn(async move { frage(port, &roh).await });
    tuer.bedienen(&mut annahme).await.expect("bedienen");
    let antwort = auftrag.await.expect("Auftrag");
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 400"));
}

/// Und die Tür hört nur auf der Rückschleife.
#[tokio::test]
async fn die_tuer_hoert_nur_auf_localhost() {
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    // Über die Rückschleife erreichbar.
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_ok());
}

// --- Stufe 2: der Kontrakt als Zugangsschlüssel ----------------------

use myl_gateway::zugang::{
    zugangsbotschaft, Anfragehuelle, Kontraktquelle, Zugangsanfrage, ZugangsanfrageRoh,
    Zugangsstelle,
};
use myl_types::bls::{BlsPublicKey, BlsSecretKey};
use myl_types::ids::{Address, SitzungId};
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand};

fn geheim(b: u8) -> BlsSecretKey {
    BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schluessel")
}

fn oeffentlich(b: u8) -> BlsPublicKey {
    geheim(b).public_key().expect("gueltiger Punkt")
}

fn kontrakt(agent: u8) -> Sitzungskontrakt {
    Sitzungskontrakt {
        inhaber: Address::aus_schluessel(&oeffentlich(200)),
        agent: Address::aus_schluessel(&oeffentlich(agent)),
        credits: Grenzen {
            budget: 10_000,
            einzellimit: 1_000,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        },
        myl: Grenzen::gesperrt(),
        empfaenger: vec![Address::aus_schluessel(&oeffentlich(210))],
        gueltig_ab: EpochId(0),
        gueltig_bis: EpochId(100),
        max_schritte: 1_000,
    }
}

struct EineQuelle(Sitzungskontrakt);

impl Kontraktquelle for EineQuelle {
    fn nachschlagen(&self, s: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        (s == self.0.adresse()).then(|| (self.0.clone(), Sitzungszustand::neu()))
    }
}

/// Baut den Rumpf einer Stufe-2-Anfrage.
fn huelle(agent: u8, sitzung: SitzungId, nummer: u64, epoche: EpochId, prompt: &[u8]) -> Vec<u8> {
    let msg = zugangsbotschaft(sitzung, nummer, epoche, prompt);
    let a = Zugangsanfrage {
        sitzung,
        schluessel: oeffentlich(agent),
        nummer,
        unterschrift: geheim(agent).sign(&msg).expect("unterschreiben"),
    };
    borsh::to_vec(&Anfragehuelle {
        zugang: ZugangsanfrageRoh::from(&a),
        rumpf: prompt.to_vec(),
    })
    .expect("kodieren")
}

fn post_roh(weg: &str, rumpf: &[u8]) -> Vec<u8> {
    let mut aus = format!(
        "POST {weg} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        rumpf.len()
    )
    .into_bytes();
    aus.extend_from_slice(rumpf);
    aus
}

/// ⚑ **Stufe 2 über einen echten Socket: mit Kontrakt kommt ein Beleg.**
#[tokio::test]
async fn mit_gueltigem_kontrakt_kommt_ein_beleg() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let prompt = b"was ist die hauptstadt von frankreich";
    let bytes = post_roh(WEG, &huelle(7, s_id, 0, EpochId(5), prompt));

    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, antwort) = tokio::join!(dienst, frage(port, &bytes));

    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 200 "),
        "abgelehnt: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(80)])
    );
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert!(
        beleg.bindung.passt(prompt),
        "der Beleg bindet einen anderen Prompt"
    );
    assert_eq!(
        beleg.weg,
        Some(myl_gateway::zugang::Ausweisweg::Unterschrift),
        "der Beleg nennt den staerkeren Weg nicht"
    );
}

/// ⚑ **Ohne gültigen Kontrakt: 403, ohne Grund und ohne Rumpf.**
///
/// Der Test fährt vier verschiedene Ablehnungen und verlangt, dass sie
/// **byteweise dieselbe Antwort** ergeben. Unterschieden sie sich, wäre
/// die Tür ein Auskunftsdienst über fremde Kontrakte.
#[tokio::test]
async fn jede_ablehnung_sieht_gleich_aus() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let prompt = b"frage";

    let faelle: Vec<(&str, Vec<u8>)> = vec![
        (
            "Sitzung gibt es nicht",
            huelle(7, SitzungId::new([0xAB; 32]), 0, EpochId(5), prompt),
        ),
        ("falscher Agent", huelle(9, s_id, 0, EpochId(5), prompt)),
        (
            "Unterschrift fuer eine andere Epoche",
            huelle(7, s_id, 0, EpochId(4), prompt),
        ),
        ("unlesbare Huelle", b"das ist kein borsh".to_vec()),
    ];

    let mut antworten = Vec::new();
    for (was, rumpf) in &faelle {
        let tuer = Tuer::binden(0).await.expect("binden");
        let port = tuer.port().expect("port");
        let mut annahme = Annahme::neu(1, EpochId(5));
        let mut stelle = Zugangsstelle::neu(EineQuelle(k.clone()));
        let bytes = post_roh(WEG, rumpf);
        let dienst = async {
            tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
                .await
                .expect("bedienen")
        };
        let (_, antwort) = tokio::join!(dienst, frage(port, &bytes));
        assert!(
            String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 403 "),
            "der Fall `{was}` wurde nicht mit 403 abgewiesen: {}",
            String::from_utf8_lossy(&antwort[..antwort.len().min(60)])
        );
        antworten.push((*was, antwort));
    }

    let (erster_name, erste) = &antworten[0];
    for (was, a) in &antworten[1..] {
        assert_eq!(
            a, erste,
            "`{was}` antwortet anders als `{erster_name}`; die Tuer verraet den Grund"
        );
    }
}

/// ⚑ **Eine abgelehnte Anfrage bekommt keine Sitzungsnummer.**
///
/// Sonst verriete die Nummernfolge im nächsten Beleg, wie oft geklopft
/// wurde, und wäre damit ein Zähler über fremde Fehlversuche.
#[tokio::test]
async fn eine_abgelehnte_anfrage_verbraucht_keine_nummer() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let prompt = b"frage";
    let mut annahme = Annahme::neu(42, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    // Erst eine Ablehnung.
    {
        let tuer = Tuer::binden(0).await.expect("binden");
        let port = tuer.port().expect("port");
        let bytes = post_roh(WEG, &huelle(9, s_id, 0, EpochId(5), prompt));
        let dienst = async {
            tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
                .await
                .expect("bedienen")
        };
        let (_, a) = tokio::join!(dienst, frage(port, &bytes));
        assert!(String::from_utf8_lossy(&a).starts_with("HTTP/1.1 403 "));
    }

    // Dann eine gute: sie muss die **erste** Nummer bekommen.
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let bytes = post_roh(WEG, &huelle(7, s_id, 0, EpochId(5), prompt));
    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, a) = tokio::join!(dienst, frage(port, &bytes));
    let beleg: Beleg = borsh::from_slice(rumpf_von(&a)).expect("Beleg");
    assert_eq!(
        beleg.sitzung, 42,
        "die abgelehnte Anfrage hat eine Nummer verbraucht"
    );
}

// --- Stufe 2, zweiter Weg: die Vollmacht als Bearer-Token ------------

use myl_gateway::vollmacht::{Vollmacht, Vorbehalt};

/// Eine HTTP-Anfrage, wie ein OpenAI-verträgliches Harness sie stellt:
/// Bearer im Kopf, Prompt im Rumpf.
fn post_bearer(weg: &str, token: &str, prompt: &[u8]) -> Vec<u8> {
    let mut aus = format!(
        "POST {weg} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        prompt.len()
    )
    .into_bytes();
    aus.extend_from_slice(prompt);
    aus
}

/// ⚑ **Der Weg, den ein Harness gehen kann.**
///
/// Kein Signieren, keine Epochenkenntnis, kein Myelith-Klient: eine
/// Kopfzeile und ein Rumpf. Genau das war der Einwand gegen den ersten
/// Entwurf der Stufe 2.
#[tokio::test]
async fn ein_bearer_token_reicht_fuer_einen_beleg() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let token = Vollmacht::ausstellen(
        &geheim(7),
        vec![Vorbehalt::NurSitzung(s_id), Vorbehalt::GueltigBis(EpochId(100))],
        [11u8; 32],
    )
    .expect("ausstellen")
    .als_bearer();

    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let prompt = b"was ist die hauptstadt von frankreich";
    let bytes = post_bearer(WEG, &token, prompt);
    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, antwort) = tokio::join!(dienst, frage(port, &bytes));

    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 200 "),
        "abgelehnt: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(80)])
    );
    let beleg: Beleg = borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
    assert!(
        beleg.bindung.passt(prompt),
        "der Beleg bindet einen anderen Prompt"
    );
    // ⚑ **Und der Beleg sagt, welche Zusicherung der Nutzer hat.**
    // Ohne das wäre die schwächere von der stärkeren nicht zu
    // unterscheiden, und der Unterschied wäre versteckt statt vermerkt.
    assert_eq!(
        beleg.weg,
        Some(myl_gateway::zugang::Ausweisweg::Vollmacht),
        "der Beleg nennt den Ausweisweg nicht"
    );
}

/// ⚑ **Eine abgeschwächte Vollmacht gilt enger, und die Tür sieht das.**
///
/// Der Halter engt auf eine andere Sitzung ein; damit passt sie nicht
/// mehr zu diesem Kontrakt.
#[tokio::test]
async fn eine_abgeschwaechte_vollmacht_gilt_enger() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let eng = Vollmacht::ausstellen(&geheim(7), vec![Vorbehalt::NurSitzung(s_id)], [11u8; 32])
        .expect("ausstellen")
        .abschwaechen(vec![Vorbehalt::GueltigBis(EpochId(4))], [12u8; 32])
        .expect("abschwaechen")
        .als_bearer();

    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    // Epoche 5 liegt hinter dem angehaengten Vorbehalt.
    let bytes = post_bearer(WEG, &eng, b"frage");
    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, antwort) = tokio::join!(dienst, frage(port, &bytes));
    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 403 "),
        "die abgeschwaechte Vollmacht galt trotzdem: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(60)])
    );
}

/// ⚑ **Ein fremdes Token sieht aus wie jede andere Ablehnung.**
#[tokio::test]
async fn ein_fremdes_token_wird_wie_alles_andere_abgewiesen() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    // Agent 9 stellt sich selbst eine Vollmacht aus: formal gueltig,
    // aber nicht der Agent dieses Kontrakts.
    let fremd = Vollmacht::ausstellen(&geheim(9), vec![Vorbehalt::NurSitzung(s_id)], [11u8; 32])
        .expect("ausstellen")
        .als_bearer();

    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));
    let bytes = post_bearer(WEG, &fremd, b"frage");
    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, antwort) = tokio::join!(dienst, frage(port, &bytes));
    assert!(String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 403 "));
    assert!(
        rumpf_von(&antwort).is_empty(),
        "die Ablehnung nennt einen Grund"
    );
}

/// ⚑ **Zwei Ausweise heissen zwei Meinungen darueber, wer da ist.**
#[tokio::test]
async fn zwei_authorization_koepfe_werden_abgewiesen() {
    let k = kontrakt(7);
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let roh = b"POST /inferenz HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer aaa\r\n\
                Authorization: Bearer bbb\r\nContent-Length: 5\r\n\r\nfrage"
        .to_vec();
    let dienst = async {
        tuer.bedienen_mit_zugang(&mut annahme, &mut stelle, EpochId(5), 1_000)
            .await
            .expect("bedienen")
    };
    let (_, antwort) = tokio::join!(dienst, frage(port, &roh));
    assert!(
        String::from_utf8_lossy(&antwort).starts_with("HTTP/1.1 401 "),
        "zwei Ausweise gingen durch: {}",
        String::from_utf8_lossy(&antwort[..antwort.len().min(60)])
    );
}

// --- Stufe 3: die Flaeche nach aussen in der OpenAI-Form -------------

use myl_gateway::oai::{
    Modellstand, Rechenauftrag, Rechenergebnis, Rechenweg, WEG_CHAT, WEG_MODELLE,
};

/// Ein Rechenweg, der bezeugt, was er bekommen hat.
struct Spiegelwerk {
    gesehen: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl Rechenweg for Spiegelwerk {
    async fn rechne(&self, auftrag: Rechenauftrag<'_>) -> Option<Rechenergebnis> {
        if let Ok(mut g) = self.gesehen.lock() {
            g.push(auftrag.prompt.to_string());
        }
        Some(Rechenergebnis {
            text: "Paris".to_string(),
            prompt_token: 12,
            neue_token: 1,
            segment: "ab12cd".to_string(),
        })
    }
    async fn modell(&self) -> Option<Modellstand> {
        Some(Modellstand {
            name: "myelith-qwen".to_string(),
            pipeline: "beef".to_string(),
        })
    }
}

/// Ein Rechenweg, der nie rechnet.
struct Totwerk;

#[async_trait::async_trait]
impl Rechenweg for Totwerk {
    async fn rechne(&self, _: Rechenauftrag<'_>) -> Option<Rechenergebnis> {
        None
    }
    async fn modell(&self) -> Option<Modellstand> {
        Some(Modellstand {
            name: "myelith-qwen".to_string(),
            pipeline: "beef".to_string(),
        })
    }
}

fn get_bearer(weg: &str, token: &str) -> Vec<u8> {
    format!("GET {weg} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n")
        .into_bytes()
}

fn bearer_fuer(agent: u8, sitzung: SitzungId) -> String {
    Vollmacht::ausstellen(
        &geheim(agent),
        vec![Vorbehalt::NurSitzung(sitzung), Vorbehalt::GueltigBis(EpochId(100))],
        [11u8; 32],
    )
    .expect("ausstellen")
    .als_bearer()
}

fn status_von(antwort: &[u8]) -> u16 {
    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(64)]).to_string();
    kopf.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// ⚑ **Der ganze Punkt von Stufe 3: ein Harness spricht die Tuer an,
/// ohne etwas ueber Myelith zu wissen.**
///
/// Basis-URL und Schluessel, sonst nichts. Ueber einen echten Socket.
#[tokio::test]
async fn ein_harness_bekommt_eine_openai_antwort() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let token = bearer_fuer(7, s_id);
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(41, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));
    let werk = Spiegelwerk {
        gesehen: std::sync::Mutex::new(Vec::new()),
    };

    let koerper = r#"{"model":"myelith-qwen","messages":[{"role":"user","content":"hauptstadt von frankreich?"}],"max_tokens":16,"temperature":0.7}"#;
    let bytes = post_bearer(WEG_CHAT, &token, koerper.as_bytes());

    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &werk, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(frage(port, &bytes), dienst);

    assert_eq!(status_von(&antwort), 200, "{}", String::from_utf8_lossy(&antwort));
    let rumpf = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    assert!(rumpf.contains(r#""content":"Paris""#), "{rumpf}");
    assert!(rumpf.contains(r#""object":"chat.completion""#), "{rumpf}");
    assert!(rumpf.contains(r#""myelith_segment":"ab12cd""#), "{rumpf}");
    // ⚑ Der Hinweis, der die Falschbedienung durch `temperature` verhindert.
    assert!(rumpf.contains(r#""myelith_deterministisch":true"#), "{rumpf}");
    // Die Sitzungsnummer kommt aus der Annahme, nicht vom Klienten.
    assert!(rumpf.contains(r#""myelith_sitzung":41"#), "{rumpf}");
    // Und der Prompt kam mit Rollen beim Rechenwerk an.
    let gesehen = werk.gesehen.lock().expect("gesehen").clone();
    assert_eq!(gesehen, vec!["user: hauptstadt von frankreich?\n".to_string()]);
}

/// ⚑ **Auch die Modellliste verlangt einen Ausweis.** Wer sie frei
/// herausgaebe, sagte einem Fremden, welcher Stand hier laeuft.
#[tokio::test]
async fn die_modellliste_verlangt_einen_ausweis() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));
    let werk = Totwerk;

    let ohne = format!("GET {WEG_MODELLE} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes();
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &werk, EpochId(5), 1_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(frage(port, &ohne), dienst);
    assert_eq!(status_von(&antwort), 401, "{}", String::from_utf8_lossy(&antwort));

    // Gegenprobe: mit Ausweis kommt die Liste.
    let token = bearer_fuer(7, s_id);
    let mit = get_bearer(WEG_MODELLE, &token);
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &werk, EpochId(5), 1_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(frage(port, &mit), dienst);
    assert_eq!(status_von(&antwort), 200, "{}", String::from_utf8_lossy(&antwort));
    let rumpf = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    assert!(rumpf.contains(r#""id":"myelith-qwen""#), "{rumpf}");
    assert!(rumpf.contains(r#""myelith_pipeline":"beef""#), "{rumpf}");
}

/// ⚑ **Kein Pod, kein 500.** „Niemand hat gerechnet" ist eine Aussage
/// ueber die Gegenseite; bei 502 wiederholt ein Klient sinnvoll.
#[tokio::test]
async fn ohne_pod_kommt_502_in_der_erwarteten_huelle() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let token = bearer_fuer(7, s_id);
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let koerper = r#"{"messages":[{"role":"user","content":"x"}]}"#;
    let bytes = post_bearer(WEG_CHAT, &token, koerper.as_bytes());
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &Totwerk, EpochId(5), 1_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(frage(port, &bytes), dienst);
    assert_eq!(status_von(&antwort), 502, "{}", String::from_utf8_lossy(&antwort));
    let rumpf = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    assert!(rumpf.contains(r#""type":"api_error""#), "{rumpf}");
    assert!(rumpf.contains("\"message\""), "{rumpf}");
}

/// ⚑ **`stream: true` bekommt einen Grund und keine stille
/// Falschbedienung.** Ein Klient, der einen Strom erwartet und eine
/// ganze Antwort bekommt, haengt in seiner Leseschleife.
#[tokio::test]
async fn ein_strom_wird_an_der_tuer_abgelehnt() {
    let k = kontrakt(7);
    let s_id = k.adresse();
    let token = bearer_fuer(7, s_id);
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(1, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));
    let werk = Spiegelwerk {
        gesehen: std::sync::Mutex::new(Vec::new()),
    };

    let koerper = r#"{"messages":[{"role":"user","content":"x"}],"stream":true}"#;
    let bytes = post_bearer(WEG_CHAT, &token, koerper.as_bytes());
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &werk, EpochId(5), 1_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(frage(port, &bytes), dienst);
    assert_eq!(status_von(&antwort), 400, "{}", String::from_utf8_lossy(&antwort));
    assert!(
        werk.gesehen.lock().expect("gesehen").is_empty(),
        "ein abgelehnter Strom hat trotzdem rechnen lassen"
    );
}
