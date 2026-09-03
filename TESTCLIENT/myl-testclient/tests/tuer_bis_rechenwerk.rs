//! Von der Türklinke bis zum Rechenwerk (GATEWAY Stufen 3 und 4).
//!
//! # ⚑ Wieder die Naht, und wieder ist dies der einzige Ort dafür
//!
//! Der Weg berührt fünf Kisten: `myl-gateway` (HTTP, Ausweis, Beleg),
//! `myl-node` (Rechenweg, lokale Leitung), `myl-siegel` (der
//! Sitzungskanal), `myl-pod` (Entsiegelung, Bindung, Rechenwerk) und
//! `myl-types` (die Vokabeln). **Keine von ihnen sieht mehr als ihren
//! eigenen Nachbarn.**
//!
//! Was hier geprüft wird, ist nicht die Inferenz, sondern **dass das
//! Ganze zusammenpasst**: Ein Harness, das nur Basis-URL und Schlüssel
//! kennt, bekommt eine Antwort, und der Prompt ist auf keinem Stück des
//! Weges im Klartext zu sehen ausser dort, wo er hingehört.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use myl_gateway::annahme::Annahme;
use myl_gateway::oai::WEG_CHAT;
use myl_gateway::tuer::Tuer;
use myl_gateway::vollmacht::{Vollmacht, Vorbehalt};
use myl_gateway::zugang::{Kontraktquelle, Zugangsstelle};
use myl_node::ortsklient::Ortsanschluss;
use myl_node::rechenweg::Ortsweg;
use myl_pod::entsiegelung::{Entsiegelndes, Gegenstellen, Klartextwerk};
use myl_pod::ortsdienst::Ortsdienst;
use myl_siegel::{Endpunkt, Epochenschluessel, Gegenpunkte, Sitzungen};
use myl_types::bls::{BlsPublicKey, BlsSecretKey};
use myl_types::Address;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId, SegmentId, SitzungId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::ortsleitung::SCHLUESSEL_DATEI;
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand};

const POD: [u8; 32] = [9u8; 32];

fn geheim(b: u8) -> BlsSecretKey {
    BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schluessel")
}
fn oeffentlich(b: u8) -> BlsPublicKey {
    geheim(b).public_key().expect("Punkt")
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

/// Der Shard nimmt jede Sitzung an, die der Knoten eröffnet.
struct JedeSitzung {
    wer: Endpunkt,
    punkte: Gegenpunkte,
}
impl Gegenstellen for JedeSitzung {
    fn nachschlagen(&self, _: u64) -> Option<(Endpunkt, Gegenpunkte)> {
        Some((self.wer, self.punkte.clone()))
    }
}

/// Das Rechenwerk bezeugt, was es im Klartext bekommen hat.
struct Spiegelwerk {
    gesehen: Arc<Mutex<Vec<String>>>,
    laeufe: Arc<AtomicUsize>,
}
impl Klartextwerk for Spiegelwerk {
    fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort {
        self.laeufe.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut g) = self.gesehen.lock() {
            g.push(String::from_utf8_lossy(prompt).to_string());
        }
        Inferenzantwort::Ergebnis {
            sitzung: auftrag.sitzung,
            token: vec![1, 2, 3, 4],
            segment: SegmentId::new([0xab; 32]),
            prompt_token: 5,
            text: "Paris".to_string(),
        }
    }
    fn pipeline(&self) -> Hash {
        Hash::sha256(b"probe-pipeline")
    }
    fn shards(&self) -> u32 {
        4
    }
}

fn verzeichnis(marke: &str) -> std::path::PathBuf {
    let v = std::env::temp_dir().join(format!(
        "myl-stufe3-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&v).expect("Verzeichnis");
    v
}

/// Eine HTTP-Anfrage, wie ein OpenAI-verträgliches Harness sie stellt.
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

/// ⚑ **Der ganze Weg: Basis-URL und Schlüssel, sonst nichts.**
#[tokio::test]
async fn ein_harness_erreicht_durch_alle_schichten_ein_rechenwerk() {
    let verz = verzeichnis("ganz");

    // --- Der Shard-Prozess -------------------------------------------
    let shard_schluessel = Epochenschluessel::probe(EpochId(5), [2u8; 32]);
    let knoten_schluessel = Epochenschluessel::probe(EpochId(5), [1u8; 32]);
    let knoten_punkte = Gegenpunkte {
        punkt: knoten_schluessel.punkt(),
        kapselpunkt: knoten_schluessel.kapselpunkt(),
    };
    let gesehen = Arc::new(Mutex::new(Vec::new()));
    let laeufe = Arc::new(AtomicUsize::new(0));
    let entsiegelnd = Entsiegelndes::neu(
        PodId::new(POD),
        Endpunkt::aus_bytes([2u8; 32]),
        Sitzungen::neu(Endpunkt::aus_bytes([2u8; 32]), shard_schluessel),
        Box::new(JedeSitzung {
            wer: Endpunkt::aus_bytes([1u8; 32]),
            punkte: knoten_punkte,
        }),
        Box::new(Spiegelwerk {
            gesehen: Arc::clone(&gesehen),
            laeufe: Arc::clone(&laeufe),
        }),
    );
    let (dienst, befund) = Ortsdienst::oeffnen(
        "127.0.0.1:0".parse().expect("Adresse"),
        &verz,
        Box::new(entsiegelnd),
    )
    .expect("Shard-Tuer");
    let shard_adresse = befund.adresse;
    std::thread::spawn(move || loop {
        if dienst.bediene_eine().is_err() {
            break;
        }
    });

    // --- Der Knoten als Rechenweg ------------------------------------
    let anschluss =
        Ortsanschluss::neu(shard_adresse, &verz.join(SCHLUESSEL_DATEI)).expect("Anschluss");
    let weg = Ortsweg::neu(
        anschluss,
        PodId::new(POD),
        EpochId(5),
        Endpunkt::aus_bytes([1u8; 32]),
        knoten_schluessel,
        "myelith-qwen",
        myl_types::Address::new([210u8; 32]),
    );

    // --- Die Tuer ----------------------------------------------------
    let k = kontrakt(7);
    let s_id = k.adresse();
    let token = Vollmacht::ausstellen(
        &geheim(7),
        vec![
            Vorbehalt::NurSitzung(s_id),
            Vorbehalt::GueltigBis(EpochId(100)),
        ],
        [11u8; 32],
    )
    .expect("ausstellen")
    .als_bearer();
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(41, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let koerper = r#"{"model":"myelith-qwen","messages":[{"role":"user","content":"hauptstadt von frankreich?"}],"max_tokens":16}"#;
    let bytes = post_bearer(WEG_CHAT, &token, koerper.as_bytes());

    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let klient = tokio::task::spawn_blocking(move || {
        let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
        strom
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("Frist");
        strom.write_all(&bytes).expect("senden");
        strom.flush().expect("leeren");
        let mut aus = Vec::new();
        let _ = strom.read_to_end(&mut aus);
        aus
    });
    let (antwort, _) = tokio::join!(klient, dienst);
    let antwort = antwort.expect("Klient");

    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(32)]).to_string();
    assert!(kopf.starts_with("HTTP/1.1 200"), "{kopf}");
    let rumpf = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    assert!(rumpf.contains(r#""content":"Paris""#), "{rumpf}");
    assert!(rumpf.contains(r#""myelith_deterministisch":true"#), "{rumpf}");
    // ⚑ Der Faden zur bezeugten Arbeit, hexadezimal.
    assert!(rumpf.contains(&"ab".repeat(32)), "{rumpf}");

    // ⚑ **Und das Rechenwerk hat den Klartext gesehen**, obwohl er
    // versiegelt über die lokale Leitung ging.
    assert_eq!(laeufe.load(Ordering::SeqCst), 1, "das Rechenwerk lief nicht genau einmal");
    let gesehen = gesehen.lock().expect("gesehen").clone();
    assert_eq!(gesehen, vec!["user: hauptstadt von frankreich?\n".to_string()]);

    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Was gerechnet wurde, wird abgebucht.**
///
/// Bis zum 2026-09-03 prüfte die Tür den Kontrakt und liess durch;
/// `sitzung_ausgeben` hatte ausserhalb der Ledger-Tests **keinen
/// Aufrufer**, und das Budget eines Nutzers sank nie. Ein Nutzer konnte
/// unbegrenzt fragen.
///
/// Dieser Test geht den ganzen Weg: Anfrage über die Tür, Rechnen,
/// Abrechnung in den Kanal, Signatur durch den Knoten, und am Ende steht
/// im **Kettenzustand**, dass verbraucht wurde.
#[tokio::test]
async fn eine_gerechnete_anfrage_bucht_credits_ab() {
    let verz = verzeichnis("abrechnung");

    // --- Shard ------------------------------------------------------
    let shard_schluessel = Epochenschluessel::probe(EpochId(5), [2u8; 32]);
    let knoten_schluessel = Epochenschluessel::probe(EpochId(5), [1u8; 32]);
    let knoten_punkte = Gegenpunkte {
        punkt: knoten_schluessel.punkt(),
        kapselpunkt: knoten_schluessel.kapselpunkt(),
    };
    let gesehen = Arc::new(Mutex::new(Vec::new()));
    let laeufe = Arc::new(AtomicUsize::new(0));
    let (dienst, befund) = Ortsdienst::oeffnen(
        "127.0.0.1:0".parse().expect("Adresse"),
        &verz,
        Box::new(Entsiegelndes::neu(
            PodId::new(POD),
            Endpunkt::aus_bytes([2u8; 32]),
            Sitzungen::neu(Endpunkt::aus_bytes([2u8; 32]), shard_schluessel),
            Box::new(JedeSitzung {
                wer: Endpunkt::aus_bytes([1u8; 32]),
                punkte: knoten_punkte,
            }),
            Box::new(Spiegelwerk {
                gesehen: Arc::clone(&gesehen),
                laeufe: Arc::clone(&laeufe),
            }),
        )),
    )
    .expect("Shard-Tuer");
    let shard_adresse = befund.adresse;
    std::thread::spawn(move || loop {
        if dienst.bediene_eine().is_err() {
            break;
        }
    });

    // --- Der Kontrakt, mit einem Agenten aus einem echten Schluessel -
    let agent_sk = geheim(7);
    let agent = Address::aus_schluessel(&oeffentlich(7));
    let empfaenger = Address::aus_schluessel(&oeffentlich(210));
    let k = kontrakt(7);
    let s_id = k.adresse();

    // --- Die Kette, mit Guthaben und offener Sitzung -----------------
    let mut kette = myl_node::kette::Kette::probestand();
    let inhaber = Address::aus_schluessel(&oeffentlich(200));
    kette.zustand_mut().account_mut(&inhaber).credits.push(
        myl_types::core_types::InferenceCredit {
            owner: inhaber,
            vtfe: 10_000,
            expiry: EpochId(1_000),
        },
    );
    myl_ledger::transitions::sitzung_eroeffnen(kette.zustand_mut(), &inhaber, k.clone())
        .expect("Sitzung eroeffnen");
    assert_eq!(
        kette.zustand().sitzung(&s_id).expect("da").zustand.verbraucht_credits,
        0
    );

    // --- Der Rechenweg mit Abrechnungskanal --------------------------
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let anschluss =
        Ortsanschluss::neu(shard_adresse, &verz.join(SCHLUESSEL_DATEI)).expect("Anschluss");
    let weg = Ortsweg::neu(
        anschluss,
        PodId::new(POD),
        EpochId(5),
        Endpunkt::aus_bytes([1u8; 32]),
        knoten_schluessel,
        "myelith-qwen",
        empfaenger,
    )
    .mit_abrechnung(tx);
    assert!(weg.bucht_ab(), "der Weg bucht nicht ab");

    // --- Die Tuer ----------------------------------------------------
    let token = Vollmacht::ausstellen(
        &agent_sk,
        vec![
            Vorbehalt::NurSitzung(s_id),
            Vorbehalt::HoechstensCredits(64),
            Vorbehalt::GueltigBis(EpochId(100)),
        ],
        [11u8; 32],
    )
    .expect("ausstellen");
    let bearer = token.als_bearer();
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(41, EpochId(5));
    let mut stelle = Zugangsstelle::neu(EineQuelle(k));

    let koerper = r#"{"messages":[{"role":"user","content":"x"}],"max_tokens":16}"#;
    let bytes = post_bearer(WEG_CHAT, &bearer, koerper.as_bytes());
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_000)
            .await
            .expect("bedienen");
    };
    let klient = tokio::task::spawn_blocking(move || {
        let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
        strom.set_read_timeout(Some(Duration::from_secs(20))).expect("Frist");
        strom.write_all(&bytes).expect("senden");
        strom.flush().expect("leeren");
        let mut aus = Vec::new();
        let _ = strom.read_to_end(&mut aus);
        aus
    });
    let (antwort, _) = tokio::join!(klient, dienst);
    let antwort = antwort.expect("Klient");
    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(20)]).to_string();
    assert!(kopf.starts_with("HTTP/1.1 200"), "{kopf}");

    // --- Die Abrechnung ist da und geht in die Kette -----------------
    let anweisung = rx.try_recv().expect("keine Abrechnung abgelegt");
    let myl_consensus::block::Anweisung::SitzungAusgeben { vorhaben, vollmacht } = &anweisung
    else {
        panic!("die Abrechnung ist keine Ausgabe: {anweisung:?}");
    };
    assert_eq!(vorhaben.sitzung, s_id);
    assert_eq!(vorhaben.handelnder, agent, "es wurde ein fremder Agent genannt");
    // Vier Token erzeugt das Spiegelwerk, also vier Credits.
    assert_eq!(vorhaben.betrag, 4, "es wurde nicht die geleistete Arbeit gebucht");
    assert!(vollmacht.is_some(), "ohne Vollmacht kann die Kette nicht autorisieren");

    // ⚑ **Und jetzt der Punkt: Der Betreiber reicht ein, nicht der
    // Agent.** Genau das kann ein Harness nicht selbst.
    let betreiber = Address::aus_schluessel(&oeffentlich(99));
    assert_ne!(betreiber, agent);
    myl_ledger::transitions::sitzung_ausgeben(
        kette.zustand_mut(),
        &betreiber,
        vorhaben,
        vollmacht.as_ref(),
    )
    .expect("die Vollmacht autorisiert die Abbuchung");

    assert_eq!(
        kette.zustand().sitzung(&s_id).expect("da").zustand.verbraucht_credits,
        4,
        "im Kettenzustand steht kein Verbrauch"
    );
    assert_eq!(
        kette.zustand().account(&inhaber).credits[0].vtfe,
        9_996,
        "die Credits des Inhabers sind nicht gesunken"
    );

    // Gegenprobe: dieselbe Abrechnung ein zweites Mal kommt nicht durch.
    assert!(
        myl_ledger::transitions::sitzung_ausgeben(
            kette.zustand_mut(),
            &betreiber,
            vorhaben,
            vollmacht.as_ref(),
        )
        .is_err(),
        "dieselbe Abrechnung wurde zweimal gebucht"
    );

    let _ = std::fs::remove_dir_all(&verz);
}
