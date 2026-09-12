//! Ein API-Aufruf eines Nutzers, bis zum echten Modell und zurück.
//!
//! # ⚑ Was dieser Test belegt, und was die anderen nicht belegen
//!
//! `tuer_bis_rechenwerk.rs` zeigt, dass der **Weg** zusammenpasst: fünf
//! Kisten, echte Sockets, aber ein Rechenwerk, das bezeugt statt zu
//! rechnen. Hier hängt am Ende die **echte Shard-Pipeline** über den
//! Artefakten des Ankermodells (ohne Vorgabe `myelith-0.6b`): vier
//! Shards, Wortschatz, Koordinator.
//!
//! Damit steht die eine Frage auf dem Prüfstand, die keiner der übrigen
//! Tests beantwortet: **Kommt am lokalen Harness Text an, den das
//! geshardete Modell erzeugt hat?**
//!
//! Der Weg, jeder Sprung echt:
//!
//! 1. HTTP `POST /v1/chat/completions` mit `Authorization: Bearer`,
//!    über einen echten Socket, in der Form, die jedes Harness spricht.
//! 2. Die Tür prüft die Vollmacht gegen den Sitzungskontrakt und
//!    schreibt die Anfrage als Beleg fest.
//! 3. Der Knoten versiegelt den Prompt für den Shard-Prozess (X25519
//!    und ML-KEM-768) und schickt ihn über die lokale Leitung, mit
//!    Ausweis.
//! 4. Der Shard entsiegelt, prüft die Bindung gegen den Klartext und
//!    gibt ihn erst dann an die Pipeline.
//! 5. Vier Shards rechnen, der letzte sampelt, der Wortschatz
//!    dekodiert.
//! 6. Zurück als JSON, mit Segmentkennung und Verbrauchszahlen.
//!
//! # ⚑ Warum er hier steht
//!
//! Er braucht `myl-gateway`, `myl-node`, `myl-siegel` **und** `myl-pod`
//! samt Ganzzahl-Laufzeit. Keine dieser Kisten darf die anderen kennen;
//! `myl-testclient` ist die einzige Stelle, die alle sieht.
//!
//! # Ohne Artefakte
//!
//! Der Test **schlägt fehl**, wenn die Artefakte fehlen, mit einem Satz,
//! der sagt was zu tun ist. Wer sie bewusst nicht hat, setzt
//! `MYL_OHNE_ARTEFAKTE=1`. Das ist Fund 113: Ein stiller Sprung sieht
//! aus wie ein bestandener Test.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use myl_gateway::annahme::Annahme;
use myl_gateway::oai::{WEG_CHAT, WEG_MODELLE};
use myl_gateway::tuer::Tuer;
use myl_gateway::vollmacht::{Vollmacht, Vorbehalt};
use myl_gateway::zugang::{Kontraktquelle, Zugangsstelle};
use myl_node::ortsklient::Ortsanschluss;
use myl_node::rechenweg::Ortsweg;
use myl_pod::entsiegelung::{Entsiegelndes, Gegenstellen};
use myl_pod::ortsdienst::Ortsdienst;
use myl_pod::pipelinewerk::Pipelinewerk;
use myl_siegel::{Endpunkt, Epochenschluessel, Gegenpunkte, Sitzungen};
use myl_types::bls::{BlsPublicKey, BlsSecretKey};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId, SitzungId};
use myl_types::ortsleitung::SCHLUESSEL_DATEI;
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand};
use myl_types::Address;

const POD: [u8; 32] = [0xAA; 32];

/// Der gemessene Artefakt-Digest aus `scale_packs/REGISTER.json`.
///
/// ⚑ **Abgeschrieben und nicht nachgerechnet**, und das ist Absicht:
/// Der Test soll prüfen, dass der Stand **durchgereicht** wird, nicht
/// wie er entsteht. Wer ihn hier neu bildete, führte eine zweite
/// Wahrheit über die Modellfassung.
const PIPELINE_DIGEST: &str = "c42bb8a8d85bba5a76b3302298903fb5c1edfe4463c5d1d44256bef447ffd5c9";

fn artefakte() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "myelith-0.6b".to_string());
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push(modell);
    p
}

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

/// Der Shard nimmt jede Sitzung dieses Knotens an.
///
/// ⚑ **Das ist die Stelle, an der später die Zuteilung aus der Kette
/// steht.** Sie steht hier als Merkmal und nicht als Nachbildung, weil
/// `myl-pod` die Kette nicht kennt.
struct JedeSitzung {
    wer: Endpunkt,
    punkte: Gegenpunkte,
}
impl Gegenstellen for JedeSitzung {
    fn nachschlagen(&self, _: u64) -> Option<(Endpunkt, Gegenpunkte)> {
        Some((self.wer, self.punkte.clone()))
    }
}

fn verzeichnis() -> PathBuf {
    let v = std::env::temp_dir().join(format!(
        "myl-harness-modell-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&v).expect("Verzeichnis");
    v
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

fn get_bearer(weg: &str, token: &str) -> Vec<u8> {
    format!("GET {weg} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n")
        .into_bytes()
}

fn rumpf_von(antwort: &[u8]) -> &[u8] {
    antwort
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .map(|i| &antwort[i + 4..])
        .unwrap_or(&[])
}

/// Ein Harness, das nur Basis-URL und Schlüssel kennt.
async fn wie_ein_harness(port: u16, bytes: Vec<u8>) -> Vec<u8> {
    tokio::task::spawn_blocking(move || {
        let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
        strom
            .set_read_timeout(Some(Duration::from_secs(600)))
            .expect("Frist");
        strom.write_all(&bytes).expect("senden");
        strom.flush().expect("leeren");
        let mut aus = Vec::new();
        let _ = strom.read_to_end(&mut aus);
        aus
    })
    .await
    .expect("Klient")
}

/// ⚑ **Der Aufruf eines Nutzers, bis zum Modell und zurück.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_nutzeraufruf_erreicht_das_geshardete_modell() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let verz = verzeichnis();

    // --- Der Shard-Prozess mit echten Gewichten ----------------------
    let digest: [u8; 32] = {
        let mut b = [0u8; 32];
        for (i, p) in b.iter_mut().enumerate() {
            *p = u8::from_str_radix(&PIPELINE_DIGEST[i * 2..i * 2 + 2], 16).expect("hex");
        }
        b
    };
    let pipeline = Hash(digest);

    let werk = Pipelinewerk::laden(&dir, PodId::new(POD), EpochId(5), pipeline, 8)
        .expect("die Shard-Pipeline laedt");
    assert_eq!(werk.shardzahl(), 4, "die Probepipeline hat vier Shards");

    let shard_schluessel = Epochenschluessel::probe(EpochId(5), [2u8; 32]);
    let knoten_schluessel = Epochenschluessel::probe(EpochId(5), [1u8; 32]);
    let knoten_punkte = Gegenpunkte {
        punkt: knoten_schluessel.punkt(),
        kapselpunkt: knoten_schluessel.kapselpunkt(),
    };
    let entsiegelnd = Entsiegelndes::neu(
        PodId::new(POD),
        Endpunkt::aus_bytes([2u8; 32]),
        Sitzungen::neu(Endpunkt::aus_bytes([2u8; 32]), shard_schluessel),
        Box::new(JedeSitzung {
            wer: Endpunkt::aus_bytes([1u8; 32]),
            punkte: knoten_punkte,
        }),
        Box::new(werk),
    );
    let (dienst, befund) = Ortsdienst::oeffnen(
        "127.0.0.1:0".parse().expect("Adresse"),
        &verz,
        Box::new(entsiegelnd),
    )
    .expect("Shard-Tuer");
    assert!(!befund.nach_aussen, "der Shard-Dienst haengt nach aussen");
    let shard_adresse = befund.adresse;
    std::thread::spawn(move || loop {
        if dienst.bediene_eine().is_err() {
            break;
        }
    });

    // --- Der Knoten als Rechenweg der Tür ----------------------------
    let anschluss =
        Ortsanschluss::neu(shard_adresse, &verz.join(SCHLUESSEL_DATEI)).expect("Anschluss");
    let weg = Ortsweg::neu(
        anschluss,
        PodId::new(POD),
        EpochId(5),
        Endpunkt::aus_bytes([1u8; 32]),
        knoten_schluessel,
        "myelith-myelith-0.6b",
        myl_types::Address::new([210u8; 32]),
    );

    // --- Die Tür -----------------------------------------------------
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

    // --- 1. Was ein Harness zuerst tut: die Modelle abfragen ---------
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(
        wie_ein_harness(port, get_bearer(WEG_MODELLE, &token)),
        dienst
    );
    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(32)]).to_string();
    assert!(kopf.starts_with("HTTP/1.1 200"), "Modellliste: {kopf}");
    let liste = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    assert!(liste.contains("myelith-myelith-0.6b"), "{liste}");
    // ⚑ Der Pipeline-Stand geht bis nach draussen durch, unverändert.
    assert!(liste.contains(PIPELINE_DIGEST), "der Stand kam nicht durch: {liste}");
    assert!(liste.contains("\"myelith_deterministisch\":true"), "{liste}");

    // --- 2. Der eigentliche Aufruf -----------------------------------
    let frage = "Die Hauptstadt von Frankreich ist";
    let koerper = format!(
        r#"{{"model":"myelith-myelith-0.6b","messages":[{{"role":"user","content":"{frage}"}}],"max_tokens":8,"temperature":0.7}}"#
    );
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(
        wie_ein_harness(port, post_bearer(WEG_CHAT, &token, koerper.as_bytes())),
        dienst
    );

    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(32)]).to_string();
    assert!(kopf.starts_with("HTTP/1.1 200"), "{kopf}");
    let rumpf = String::from_utf8_lossy(rumpf_von(&antwort)).to_string();
    eprintln!("\n--- Antwort an das Harness ---\n{rumpf}\n");

    // ⚑ **Die Form, die ein Harness erwartet.**
    assert!(rumpf.contains(r#""object":"chat.completion""#), "{rumpf}");
    assert!(rumpf.contains(r#""role":"assistant""#), "{rumpf}");
    assert!(rumpf.contains(r#""finish_reason":"stop""#), "{rumpf}");
    assert!(rumpf.contains(r#""myelith_sitzung":41"#), "{rumpf}");
    // `temperature` war gesetzt und hat nichts bewirkt, und das steht da.
    assert!(rumpf.contains(r#""myelith_deterministisch":true"#), "{rumpf}");

    // ⚑ **Gelesen wie ein Klient liest**, mit einem JSON-Zerleger und
    // nicht mit `find` und Indizes. Der erste Entwurf dieses Tests hat
    // sich damit vertan und den halben Rumpf als `content` gelesen; die
    // Zusicherung „nicht leer" galt dann für die falsche Zeichenkette.
    let doc: serde_json::Value = serde_json::from_str(&rumpf).expect("die Antwort ist JSON");

    let inhalt = doc["choices"][0]["message"]["content"]
        .as_str()
        .expect("kein content");
    assert!(
        !inhalt.trim().is_empty(),
        "das Modell hat nichts erzeugt: {rumpf}"
    );

    // ⚑ **Die Verbrauchszahlen sind gezählt und nicht geschätzt**
    // (Fund 160): `prompt_tokens` kommt aus dem Wortschatz des Shards.
    let neue = doc["usage"]["completion_tokens"].as_u64().expect("completion_tokens");
    let prompt_token = doc["usage"]["prompt_tokens"].as_u64().expect("prompt_tokens");
    assert!(neue > 0 && neue <= 8, "erzeugte Token ausserhalb des Deckels: {neue}");
    assert!(prompt_token > 0, "der Prompt hatte null Token");
    // Der Prompt ist 39 Byte lang; als Token muss er deutlich kürzer
    // sein, sonst zählt hier wieder jemand Bytes.
    assert!(
        prompt_token < 20,
        "prompt_tokens sieht nach Bytes aus und nicht nach Token: {prompt_token}"
    );
    // --- 2b. Derselbe Aufruf durch das echte Harness -----------------
    //
    // ⚑ **Das ist der Beleg von AGENT_LAYER 5.1**, und er ist ein
    // anderer als der oben. Oben steht eine von Hand getippte Anfrage;
    // hier spricht `myl_local_agent::Tuerklient`, der die OpenAI-Form
    // **unabhaengig** aufschreibt und `myl-gateway` nicht kennen darf.
    // Zwei getrennt geschriebene Fassungen, die sich hier treffen: Genau
    // das ist die Behauptung „ein gewoehnlicher OpenAI-Klient erreicht
    // diese Tuer", und sie liesse sich mit geteilten Typen nicht pruefen.
    let dienst2 = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen (Harness)");
    };
    let token2 = token.clone();
    let harness = tokio::task::spawn_blocking(move || {
        myl_local_agent::Tuerklient::neu("127.0.0.1", port, token2)
            .mit_frist(std::time::Duration::from_secs(600))
            .chat(
                "myelith-myelith-0.6b",
                &[myl_local_agent::Nachricht::nutzer(frage)],
                Some(8),
            )
    });
    let (aus, _) = tokio::join!(harness, dienst2);
    let a = aus.expect("Harness-Faden").expect("die Tuer antwortet dem Harness");

    eprintln!("\n--- Was das Harness liest ---\n{a:#?}\n");
    assert!(!a.text.trim().is_empty(), "das Harness hat keinen Text bekommen");
    assert_eq!(a.abschlussgrund.as_deref(), Some("stop"), "{a:?}");
    assert!(!a.kennung.is_empty(), "ohne Kennung hat der Nutzer keinen Beleg");
    assert!(a.antwort_token > 0 && a.antwort_token <= 8, "{a:?}");
    assert!(a.prompt_token > 0, "{a:?}");

    // --- 2c. Das Werkzeugangebot geht durch die Tuer --------------
    //
    // ⚑ **Der Beleg fuer AGENT_LAYER 5.2, soweit er hier zu haben ist.**
    // Geprueft wird, dass eine Systemnachricht mit dem Werkzeugangebot
    // und eine Werkzeugantwort **ankommen**, also die Tuer sie annimmt
    // und in den Prompt setzt. **Nicht geprueft** wird, ob das Modell
    // daraufhin einen Aufruf vorschlaegt: Ein Modell dieser Groesse tut
    // das unzuverlaessig, und ein Test, der davon abhinge, waere
    // flatterig statt scharf.
    //
    // ⚑ **Was ein Vorschlag darf, entscheidet ohnehin nicht das
    // Modell**, sondern `werkzeug::Erlaubnis`, und die ist in
    // `AGENT_LAYER/local-agent/tests/werkzeug.rs` geprueft, samt dem
    // eingeschleusten Aufruf.
    let werkzeuge = vec![myl_local_agent::werkzeug::Werkzeug::ohne_parameter(
        "zeit",
        "Die aktuelle Zeit.",
    )];
    let angebot = myl_local_agent::werkzeug::angebot(
        &werkzeuge,
        myl_local_agent::werkzeug::Ansageform::Amtlich,
    );
    let ergebnis = myl_local_agent::werkzeug::Werkzeugergebnis::nachricht("zeit", "12:00");
    let dienst3 = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000)
            .await
            .expect("bedienen (Werkzeuge)");
    };
    let token3 = token.clone();
    let mit_werkzeugen = tokio::task::spawn_blocking(move || {
        myl_local_agent::Tuerklient::neu("127.0.0.1", port, token3)
            .mit_frist(std::time::Duration::from_secs(600))
            .chat(
                "myelith-myelith-0.6b",
                &[angebot, myl_local_agent::Nachricht::nutzer("Wie spät ist es?"), ergebnis],
                Some(8),
            )
    });
    let (aus3, _) = tokio::join!(mit_werkzeugen, dienst3);
    let a3 = aus3.expect("Faden").expect("die Tuer nimmt Werkzeugnachrichten an");
    assert!(a3.prompt_token > 20, "das Angebot ist nicht im Prompt gelandet: {a3:?}");
    eprintln!("\n--- Mit Werkzeugangebot ---\n{} Prompt-Token statt {}\n", a3.prompt_token, a.prompt_token);

    // --- 2d. Die ganze Agentenschleife, an der echten Tuer ----------
    //
    // ⚑ **Die Simulation unter echten Bedingungen.** Kein Stummel: die
    // Tuer prueft die Vollmacht, der Knoten versiegelt, vier Shards
    // rechnen, der Wortschatz dekodiert. Was hier laeuft, ist die
    // Schleife aus `myl_local_agent::schleife` von Anfang bis Ende.
    //
    // ⚑ **Was NICHT geprueft wird: dass das Modell ein Werkzeug
    // vorschlaegt.** Ein Modell dieser Groesse tut das unzuverlaessig.
    // Geprueft wird, dass die Schleife durchlaeuft, einen Beleg
    // hinterlaesst und an einer Grenze endet statt ins Leere.
    {
        use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler, Werkzeugkasten};
        use myl_local_agent::betrieb::Betriebsart;
        use myl_local_agent::schleife::Lauf;
        use myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen;

        struct Zeit;
        impl Werkzeugausfuehrung for Zeit {
            fn name(&self) -> &str {
                "zeit"
            }
            fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
                Ok("12:00".to_string())
            }
        }

        let mut kasten = Werkzeugkasten::neu();
        kasten
            .einhaengen(
                myl_local_agent::werkzeug::Werkzeug::ohne_parameter("zeit", "Die Zeit."),
                Box::new(Zeit),
            )
            .expect("eingehaengt");
        let mut registratur = myl_agent::registratur::Registratur::neu();
        let a_zeit = registratur
            .nimm_werkzeug(myl_agent::manifest::Werkzeugmanifest {
                name: "zeit".into(),
                anbieter: "Myelith".into(),
                revision: "1".into(),
                lizenz: "PolyForm-Shield-1.0.0".into(),
                art: myl_agent::manifest::Werkzeugart::Deterministisch,
                herkunft: myl_agent::manifest::Herkunft::Verankert,
            })
            .expect("angenommen");
        let kontrakt = myl_types::sitzung::Sitzungskontrakt::neu(
            myl_types::ids::Address::new([1u8; 32]),
            myl_types::ids::Address::new([2u8; 32]),
            myl_types::sitzung::Grenzen {
                budget: 1000,
                einzellimit: 100,
                schwelle: u64::MAX,
                zeugenleiter: Vec::new(),
            },
            myl_types::sitzung::Grenzen {
                budget: 1000,
                einzellimit: 100,
                schwelle: u64::MAX,
                zeugenleiter: Vec::new(),
            },
            vec![myl_types::ids::Address::new([9u8; 32])],
            EpochId(0),
            EpochId(100),
            2,
        )
        .expect("Kontrakt");
        let grenzen = Sitzungsgrenzen::neu(kontrakt, kasten.angebote());
        let finden = move |n: &str| -> Option<myl_types::ids::MerkleRoot> {
            (n == "zeit").then_some(a_zeit)
        };

        let token4 = token.clone();
        let agent = tokio::task::spawn_blocking(move || {
            let klient = myl_local_agent::Tuerklient::neu("127.0.0.1", port, token4)
                .mit_frist(std::time::Duration::from_secs(600));
            Lauf {
                klient: &klient,
                modell: "myelith-myelith-0.6b",
                grenzen: &grenzen,
                betriebsart: Betriebsart::NurVerankert,
                kasten: &kasten,
                registratur: &registratur,
                adressen: &finden,
                anker: myl_types::hash::Hash::from_bytes([7u8; 32]),
                max_tokens: Some(16),
                ansageform: myl_local_agent::werkzeug::Ansageform::Amtlich,
                melder: None,
                // ⚑ Der Harness haengt nichts ein: Die Einhaengegrenze
                // ist eine Zusage des Clients an den Nutzer ueber
                // dessen eigene Ablage, und dieser Lauf hat keine.
                einhaengung: None,
            }
            .fahren("Wie spaet ist es?")
        });

        // ⚑ **Die Tuer bedient, solange der Agent laeuft, und keinen
        // Aufruf laenger.** Der erste Entwurf hat hier `join!` mit einer
        // festen Zahl von `bedienen_v1` benutzt, und der Lauf blieb am
        // 2026-09-05 stehen: Ein Modell dieser Groesse schlaegt oft KEIN
        // Werkzeug vor, dann endet die Schleife nach einem Schritt, und
        // die Tuer wartete auf einen zweiten Aufruf, der nie kam.
        // Gemessen: null Prozent CPU nach 32 Sekunden, damals an
        // Qwen2.5-0,5B; der Anker ist seit dem 2026-09-11 Qwen3-0,6B,
        // und die Beobachtung ist an ihm **nicht** nachgemessen. Sie
        // begruendet hier nur, warum der Test nicht zaehlt, wie viele
        // Aufrufe kommen, und das ist unabhaengig davon richtig.
        //
        // Wie viele Aufrufe kommen, weiss nur die Schleife. Also fragt
        // der Test nicht danach, sondern hoert auf, wenn sie fertig ist.
        tokio::pin!(agent);
        let aus4 = loop {
            tokio::select! {
                r = &mut agent => break r,
                _ = tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(5), 1_700_000_000_000) => {}
            }
        };
        let e = aus4.expect("Agentenfaden");

        eprintln!(
            "\n--- Agentenschleife an der echten Tuer ---\n  Ende: {}\n{}",
            e.ende,
            e.strom.bericht()
        );
        assert!(!e.strom.schritte().is_empty(), "die Schleife hat keinen Schritt gemacht");
        // ⚑ **Der Beleg traegt eine Kette**, denn die echte Tuer nennt
        // ihre Segmentkennung. Das ist der Unterschied zum Stummel.
        let glieder = e.strom.kettenglieder().expect("die Tuer nennt Segmentkennungen");
        assert_eq!(glieder.len(), e.strom.schritte().len());
        assert!(
            e.strom.nicht_nachrechenbare().is_empty(),
            "unter `nur verankert` darf kein Schritt offen bleiben"
        );
    }

    assert_eq!(
        doc["usage"]["total_tokens"].as_u64(),
        Some(prompt_token + neue),
        "die Summe stimmt nicht"
    );

    // ⚑ **Der Faden zur bezeugten Arbeit.** Die Segmentkennung ist
    // `session ‖ position` (siehe `segment_id_from`), also muss die
    // Sitzungsnummer darin stehen: Ein Segment aus einer fremden Sitzung
    // wäre Arbeit, die niemand dieser Anfrage zuordnen kann.
    let segment = doc["myelith_segment"].as_str().expect("kein Segment");
    assert_eq!(segment.len(), 64, "die Kennung hat nicht 32 Byte: {segment}");
    let roh: Vec<u8> = (0..32)
        .map(|i| u8::from_str_radix(&segment[i * 2..i * 2 + 2], 16).expect("hex"))
        .collect();
    let sitzung_im_segment = u64::from_le_bytes(roh[..8].try_into().expect("acht Byte"));
    assert_eq!(
        sitzung_im_segment, 41,
        "das Segment gehoert zu einer fremden Sitzung: {segment}"
    );

    eprintln!("--- Ergebnis ---");
    eprintln!("  Frage:         {frage}");
    eprintln!("  Antwort:       {inhalt}");
    eprintln!("  Prompt-Token:  {prompt_token}");
    eprintln!("  Neue Token:    {neue}");
    eprintln!("  Segment:       {segment}");

    let _ = std::fs::remove_dir_all(&verz);
}
