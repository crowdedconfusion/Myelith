//! Der Gesamtlauf: alle Kompartimente verzahnt, an einem Auftrag.
//!
//! # ⚑ Was dieser Lauf ist, und was die Einzeltests nicht sind
//!
//! Jede Kiste hat ihre eigenen Tests, und die sind grün. **Fast jeder
//! schwere Fund dieses Projekts saß trotzdem nicht *in* einer
//! Komponente, sondern zwischen zweien:** Fund 114 zwischen Ziehung und
//! Aufrufer, Fund 160 zwischen Tür und Zählung, Fund 161 zwischen
//! Bündel und Gewichtung. Ein Test je Kiste findet solche Nähte nicht,
//! weil auf beiden Seiten der Naht alles stimmt.
//!
//! Dieser Lauf geht deshalb **einen Weg von Anfang bis Ende**, an einer
//! Kette, mit einem Nutzer, einem Auftrag und einem Modell. Er ist kein
//! Ersatz für die Einzeltests, sondern die Naht über allen.
//!
//! # Der Weg, in der Reihenfolge, in der er wirklich läuft
//!
//! 1. **GOVERNANCE** stellt die Parameter, gegen die alles gerechnet wird.
//! 2. **CONSENSUS/NODE**: Sechs Miner melden sich an, ein Block wird gebaut.
//! 3. **TOKENOMICS**: Der Nutzer verbrennt MYL und bekommt Credits.
//! 4. **AGENT_LAYER**: Sitzungskontrakt in der Kette, Vollmacht beim Agenten.
//! 5. **INTEGER_LLM**: Der echte Qwen2.5-0,5B wird geladen.
//! 6. **COMPUTE_PIPELINE**: Der Shard-Prozess öffnet seine lokale Tür.
//! 7. **NETWORKING**: Der Knoten baut den versiegelten Kanal dorthin.
//! 8. **GATEWAY**: Ein Harness ruft `/v1/chat/completions` mit Bearer.
//! 9. **CONSENSUS**: Die gerechnete Anfrage wird gegen die Vollmacht abgebucht.
//! 10. **VERIFICATION**: Dieselbe Frage ein zweites Mal, Wort für Wort verglichen.
//! 11. **STORAGE**: Das Speicherentgelt eines Skalenpakets wird abgerechnet.
//! 12. **TOKENOMICS/NODE**: PoI-Bündel in den Block, Epoche schliesst, Miner bezahlt.
//!
//! # ⚑ Was er ausdrücklich **nicht** abdeckt
//!
//! **TRAINING und SIMULATION nehmen an einem Anfrageweg nicht teil.**
//! Die eine trainiert, die andere simuliert Netzverhalten über Stunden;
//! sie in diesen Lauf zu zwingen hiesse, eine Verzahnung zu behaupten,
//! die es nicht gibt. Beide haben eigene Tests und eigene Läufe.
//!
//! ⚑ **Der Pod baut sein Bündel nicht selbst.** Die vier Shards laufen
//! in *einem* Prozess; das ist der Phase-1-Probelauf und kein Pod aus
//! unabhängigen Minern. Das Bündel entsteht hier wie in den Kettentests,
//! aus der Zuteilung und mit echten Unterschriften der sechs Miner.
//! **Diese Grenze steht hier, damit niemand den grünen Lauf für mehr
//! hält, als er sagt**, und sie fällt mit dem nächsten Bauschritt: ein
//! Knoten, der genau einen Shard hält.
//!
//! # ⚑ Was der erste Lauf gefunden hat (Fund 164)
//!
//! Die Gegenprobe „beide Anfragen bekommen dieselbe Sitzungsnummer"
//! wurde nicht von der Determinismuszeile gefangen, sondern von der
//! Segmentzeile: Bei gleicher Nummer kam dieselbe Antwort. Der Grund ist
//! ein Befund und keine Beruhigung: `run_prompt` füllt immer ab Position
//! 0 vor, **der KV-Cache je Sitzung wird also geschrieben und nie wieder
//! gelesen**, und weder er noch `dekodier_digest` noch
//! `Coordinator::completed` werden je geräumt. Gemessen: 14 Segmente je
//! Anfrage, streng linear. Steht im Fahrplan; behoben wird es mit dem
//! Bauschritt, der ohnehin klären muss, wer im Pod das Bündel zieht.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use myl_consensus::block::{Anweisung, Transaktion, BLOECKE_JE_EPOCHE};
use myl_gateway::annahme::Annahme;
use myl_gateway::oai::WEG_CHAT;
use myl_gateway::tuer::Tuer;
use myl_gateway::zugang::{Kontraktquelle, Zugangsstelle};
use myl_node::kette::{probekonto, probeschluessel, Kette};
use myl_node::ortsklient::Ortsanschluss;
use myl_node::rechenweg::Ortsweg;
use myl_pod::entsiegelung::{Entsiegelndes, Gegenstellen};
use myl_pod::ortsdienst::Ortsdienst;
use myl_pod::pipelinewerk::Pipelinewerk;
use myl_siegel::{Endpunkt, Epochenschluessel, Gegenpunkte, Sitzungen};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MinerId, PodId, SitzungId};
use myl_types::ortsleitung::SCHLUESSEL_DATEI;
use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand, Waehrung, Zeugenstufe};
use myl_types::vollmacht::{Vollmacht, Vorbehalt};

const POD: [u8; 32] = [0xAA; 32];

/// Der gemessene Artefakt-Digest aus `scale_packs/REGISTER.json`.
/// Abgeschrieben und nicht nachgerechnet, siehe `harness_bis_modell.rs`.
const PIPELINE_DIGEST: &str = "c42bb8a8d85bba5a76b3302298903fb5c1edfe4463c5d1d44256bef447ffd5c9";

/// Die Frage, die zweimal gestellt wird.
const FRAGE: &str = "Die Hauptstadt von Frankreich ist";

/// Zählt die Stationen mit, damit am Ende belegbar ist, dass keine
/// übersprungen wurde.
///
/// ⚑ **Ohne diese Liste wäre der Lauf wertlos.** Ein `return` mitten im
/// Test, ein Zweig, der still nichts tut: Der Test bliebe grün und
/// hätte die halbe Kette nie berührt. Das ist Fund 113 in klein.
struct Bericht {
    stationen: Vec<&'static str>,
}

impl Bericht {
    fn neu() -> Self {
        Self { stationen: Vec::new() }
    }
    fn schritt(&mut self, kompartiment: &'static str, befund: impl AsRef<str>) {
        eprintln!("  [{kompartiment:<18}] {}", befund.as_ref());
        self.stationen.push(kompartiment);
    }
}

fn artefakte() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    let mut p = PathBuf::from(manifest);
    p.push("..");
    p.push("..");
    p.push("INTEGER_LLM");
    p.push("artifacts");
    p.push(modell);
    p
}

fn verzeichnis() -> PathBuf {
    let v = std::env::temp_dir().join(format!(
        "myl-gesamtlauf-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&v).expect("Verzeichnis");
    v
}

/// Das kalte Konto eines Miners: getrennt vom Schlüssel, der arbeitet.
fn kaltes_konto(w: u8) -> Address {
    Address::new([200 + w; 32])
}

struct EineQuelle(Sitzungskontrakt);
impl Kontraktquelle for EineQuelle {
    fn nachschlagen(&self, s: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        (s == self.0.adresse()).then(|| (self.0.clone(), Sitzungszustand::neu()))
    }
}

struct JedeSitzung {
    wer: Endpunkt,
    punkte: Gegenpunkte,
}
impl Gegenstellen for JedeSitzung {
    fn nachschlagen(&self, _: u64) -> Option<(Endpunkt, Gegenpunkte)> {
        Some((self.wer, self.punkte.clone()))
    }
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

async fn wie_ein_harness(port: u16, bytes: Vec<u8>) -> Vec<u8> {
    tokio::task::spawn_blocking(move || {
        let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
        strom
            .set_read_timeout(Some(Duration::from_secs(900)))
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

/// Was von einer Antwort für den Vergleich zählt.
struct Ergebnis {
    inhalt: String,
    prompt_token: u64,
    neue_token: u64,
    segment: String,
}

fn auswerten(antwort: &[u8]) -> Ergebnis {
    let kopf = String::from_utf8_lossy(&antwort[..antwort.len().min(32)]).to_string();
    assert!(kopf.starts_with("HTTP/1.1 200"), "{kopf}");
    let doc: serde_json::Value =
        serde_json::from_slice(rumpf_von(antwort)).expect("die Antwort ist JSON");
    assert_eq!(
        doc["myelith_deterministisch"].as_bool(),
        Some(true),
        "die Tür verspricht keinen Determinismus mehr"
    );
    Ergebnis {
        inhalt: doc["choices"][0]["message"]["content"]
            .as_str()
            .expect("content")
            .to_string(),
        prompt_token: doc["usage"]["prompt_tokens"].as_u64().expect("prompt_tokens"),
        neue_token: doc["usage"]["completion_tokens"]
            .as_u64()
            .expect("completion_tokens"),
        segment: doc["myelith_segment"].as_str().expect("Segment").to_string(),
    }
}

/// ⚑ **Ein Auftrag durch alle Kompartimente, mit echtem Modell.**
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alle_kompartimente_verzahnt() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let verz = verzeichnis();
    let mut b = Bericht::neu();
    eprintln!("\n=== MYELITH GESAMTLAUF: alle Kompartimente an einem Auftrag ===\n");

    // ---- 1. GOVERNANCE: die Parameter, gegen die gerechnet wird ------
    let registry = myl_governance::ParameterRegistry::vorgabe();
    let anzahl = myl_governance::Parameter::alle().len();
    assert!(
        myl_governance::pruefe_invarianten(&registry).is_ok(),
        "die Vorgabeparameter verletzen eine Invariante"
    );
    b.schritt(
        "GOVERNANCE",
        format!("{anzahl} Parameter in der Vorgabe, alle Invarianten halten"),
    );

    // ---- 2. CONSENSUS/NODE: sechs Miner an einer echten Kette --------
    let mut kette = Kette::probestand();
    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        kette.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: myl_types::miner::HardwareClass::MediumGpu,
                    zone: myl_types::node_metadata::GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    kette.baue_block();
    assert_eq!(kette.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");
    b.schritt(
        "CONSENSUS/NODE",
        format!("6 Miner angemeldet, Kette auf Hoehe {}", kette.hoehe()),
    );

    // ---- 3. TOKENOMICS: MYL verbrennen, Credits bekommen -------------
    let nutzer = probekonto(0);
    let vor_myl = kette.zustand().account(&nutzer).balance;
    kette.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            nonce[0],
            Anweisung::Burn { betrag: 5_000_000 },
        )
        .expect("signieren"),
    );
    nonce[0] += 1;
    kette.baue_block();
    let nach_myl = kette.zustand().account(&nutzer).balance;
    let credits: u64 = kette
        .zustand()
        .account(&nutzer)
        .credits
        .iter()
        .map(|c| c.vtfe)
        .sum();
    assert_eq!(vor_myl - nach_myl, 5_000_000, "es wurde nicht genau der Betrag verbrannt");
    assert!(credits > 0, "es wurden keine Credits gepraegt");
    assert!(kette.zustand().burn_epoche > 0, "der Burn wurde fuer die Praegung nicht gezaehlt");
    b.schritt(
        "TOKENOMICS",
        format!(
            "5 000 000 MYL verbrannt -> {credits} Credits bei Preis {}",
            kette.zustand().credit_price
        ),
    );

    // ---- 4. AGENT_LAYER: Kontrakt in der Kette, Vollmacht beim Agenten
    let agent_sk = probeschluessel(1);
    let agent = probekonto(1);
    let betreiber = probekonto(2);
    let kontrakt = Sitzungskontrakt::neu(
        nutzer,
        agent,
        Grenzen {
            budget: 10_000,
            einzellimit: 1_000,
            schwelle: u64::MAX,
            // Unter 5 000 Credits verlangt der Kontrakt keine Zeugen.
            zeugenleiter: vec![Zeugenstufe { ab_betrag: 5_000, zeugen: 3 }],
        },
        Grenzen::gesperrt(),
        vec![betreiber],
        EpochId(0),
        EpochId(100),
        1_000,
    )
    .expect("gueltiger Kontrakt");
    let s_id = kontrakt.adresse();
    kette.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            nonce[0],
            Anweisung::SitzungEroeffnen { kontrakt: kontrakt.clone() },
        )
        .expect("signieren"),
    );
    nonce[0] += 1;
    kette.baue_block();
    assert!(kette.zustand().sitzung(&s_id).is_some(), "die Sitzung steht nicht im Zustand");

    // ⚑ **Die Zeugenregel des Agent Layers, an diesem Kontrakt.** Ein
    // kleiner Betrag geht ohne Bezeugung durch, ein grosser nicht.
    let anfrage = Hash::sha256(FRAGE.as_bytes());
    let klein = myl_agent::darf_verwendet_werden(&kontrakt, Waehrung::Credits, 8, &anfrage, &[])
        .expect("Regel");
    let gross = myl_agent::darf_verwendet_werden(&kontrakt, Waehrung::Credits, 9_000, &anfrage, &[])
        .expect("Regel");
    assert!(klein.ja(), "ein Betrag unter der Schwelle wurde ohne Not abgewiesen");
    assert!(!gross.ja(), "ein Betrag ueber der Schwelle kam ohne Zeugen durch");
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
    b.schritt(
        "AGENT_LAYER",
        format!(
            "Kontrakt offen, Vollmacht mit 3 Vorbehalten; Zeugenregel greift ab {} Credits",
            kontrakt.grenzen(Waehrung::Credits).zeugenleiter[0].ab_betrag
        ),
    );

    // ---- 5. INTEGER_LLM: das echte Modell ----------------------------
    let digest: [u8; 32] = {
        let mut x = [0u8; 32];
        for (i, p) in x.iter_mut().enumerate() {
            *p = u8::from_str_radix(&PIPELINE_DIGEST[i * 2..i * 2 + 2], 16).expect("hex");
        }
        x
    };
    let werk = Pipelinewerk::laden(&dir, PodId::new(POD), EpochId(0), Hash(digest), 16)
        .expect("die Shard-Pipeline laedt");
    assert_eq!(werk.shardzahl(), 4, "die Probepipeline hat vier Shards");
    b.schritt(
        "INTEGER_LLM",
        format!("Qwen2.5-0,5B geladen, auf {} Shards verteilt", werk.shardzahl()),
    );

    // ---- 6. COMPUTE_PIPELINE: die lokale Tür des Shard-Prozesses -----
    let shard_schluessel = Epochenschluessel::probe(EpochId(0), [2u8; 32]);
    let knoten_schluessel = Epochenschluessel::probe(EpochId(0), [1u8; 32]);
    let knoten_punkte = Gegenpunkte {
        punkt: knoten_schluessel.punkt(),
        kapselpunkt: knoten_schluessel.kapselpunkt(),
    };
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
            Box::new(werk),
        )),
    )
    .expect("Shard-Tuer");
    assert!(!befund.nach_aussen, "der Shard-Dienst haengt nach aussen");
    let shard_adresse = befund.adresse;
    std::thread::spawn(move || loop {
        if dienst.bediene_eine().is_err() {
            break;
        }
    });
    b.schritt(
        "COMPUTE_PIPELINE",
        format!("Shard-Prozess auf {shard_adresse}, nur lokal, Ausweis noetig"),
    );

    // ---- 7. NETWORKING: der versiegelte Kanal dorthin -----------------
    let (abr_tx, mut abr_rx) = tokio::sync::mpsc::unbounded_channel();
    let anschluss =
        Ortsanschluss::neu(shard_adresse, &verz.join(SCHLUESSEL_DATEI)).expect("Anschluss");
    let weg = Ortsweg::neu(
        anschluss,
        PodId::new(POD),
        EpochId(0),
        Endpunkt::aus_bytes([1u8; 32]),
        knoten_schluessel,
        "myelith-qwen2.5-0.5b",
        betreiber,
    )
    .mit_abrechnung(abr_tx);
    b.schritt("NETWORKING", "Sitzungskanal X25519 + ML-KEM-768 steht");

    // ---- 8. GATEWAY: der Aufruf eines Harness -------------------------
    let tuer = Tuer::binden(0).await.expect("binden");
    let port = tuer.port().expect("port");
    let mut annahme = Annahme::neu(41, EpochId(0));
    let mut stelle = Zugangsstelle::neu(EineQuelle(kontrakt.clone()));
    let koerper = format!(
        r#"{{"model":"myelith-qwen2.5-0.5b","messages":[{{"role":"user","content":"{FRAGE}"}}],"max_tokens":8,"temperature":0.7}}"#
    );

    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(0), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(
        wie_ein_harness(port, post_bearer(WEG_CHAT, &bearer, koerper.as_bytes())),
        dienst
    );
    let erst = auswerten(&antwort);
    assert!(!erst.inhalt.trim().is_empty(), "das Modell hat nichts erzeugt");
    assert!(erst.neue_token > 0 && erst.neue_token <= 8, "Token ausserhalb des Deckels");
    assert!(erst.prompt_token > 0 && erst.prompt_token < 20, "prompt_tokens sieht nach Bytes aus");
    b.schritt(
        "GATEWAY",
        format!(
            "HTTP 200, {} Prompt-Token, {} erzeugt, Segment {}",
            erst.prompt_token,
            erst.neue_token,
            &erst.segment[..16]
        ),
    );
    eprintln!("\n      Frage:   {FRAGE}");
    eprintln!("      Antwort: {}\n", erst.inhalt.trim());

    // ---- 9. CONSENSUS: die gerechnete Anfrage wird abgebucht ----------
    let anweisung = abr_rx.try_recv().expect("es wurde keine Abrechnung abgelegt");
    let Anweisung::SitzungAusgeben { vorhaben, vollmacht } = &anweisung else {
        panic!("die abgelegte Anweisung ist keine Ausgabe");
    };
    assert_eq!(
        vorhaben.betrag,
        erst.neue_token.max(1),
        "es wurde nicht die geleistete Arbeit gebucht"
    );
    assert_eq!(vorhaben.empfaenger, betreiber, "das Geld geht an den Falschen");
    myl_ledger::transitions::sitzung_ausgeben(
        kette.zustand_mut(),
        &betreiber,
        vorhaben,
        vollmacht.as_ref(),
    )
    .expect("die Vollmacht autorisiert den Betreiber");
    // ⚑ **Und ein zweites Mal geht nicht** (der Riegel aus `Vorhaben::nummer`).
    assert!(
        myl_ledger::transitions::sitzung_ausgeben(
            kette.zustand_mut(),
            &betreiber,
            vorhaben,
            vollmacht.as_ref(),
        )
        .is_err(),
        "dieselbe Abrechnung ging ein zweites Mal durch"
    );
    let verbraucht = kette.zustand().sitzung(&s_id).expect("Sitzung").zustand.verbraucht_credits;
    assert_eq!(verbraucht, erst.neue_token.max(1), "im Kettenzustand steht kein Verbrauch");
    b.schritt(
        "CONSENSUS",
        format!("{verbraucht} Credits abgebucht, Wiederholung abgewiesen"),
    );

    // ---- 10. VERIFICATION: dieselbe Frage, dieselbe Antwort -----------
    // ⚑ **Das ist die Aussage, auf der die Redundanzprüfung ruht.** Zwei
    // Pods rechnen dieselbe Anfrage und ihre Spuren müssen gleich sein;
    // wären sie es nicht, wäre `compare_commitments` ein Zufallsgenerator.
    let dienst = async {
        tuer.bedienen_v1(&mut annahme, &mut stelle, &weg, EpochId(0), 1_700_000_000_000)
            .await
            .expect("bedienen");
    };
    let (antwort, _) = tokio::join!(
        wie_ein_harness(port, post_bearer(WEG_CHAT, &bearer, koerper.as_bytes())),
        dienst
    );
    let zweit = auswerten(&antwort);
    assert_eq!(zweit.inhalt, erst.inhalt, "dieselbe Frage gab eine andere Antwort");
    assert_eq!(zweit.neue_token, erst.neue_token, "die Tokenzahl schwankt");
    assert_ne!(zweit.segment, erst.segment, "zwei Anfragen teilten sich ein Segment");

    let spur_a = [Hash::sha256(erst.inhalt.as_bytes())];
    let spur_b = [Hash::sha256(zweit.inhalt.as_bytes())];
    let spur_c = [Hash::sha256(b"etwas anderes")];
    assert!(
        matches!(
            myl_verifier::compare_commitments(&spur_a, &spur_b).expect("Vergleich"),
            myl_verifier::CompareResult::Match
        ),
        "zwei gleiche Laeufe galten als verschieden"
    );
    assert!(
        matches!(
            myl_verifier::compare_commitments(&spur_a, &spur_c).expect("Vergleich"),
            myl_verifier::CompareResult::Mismatch { first_divergence: 0 }
        ),
        "zwei verschiedene Laeufe galten als gleich"
    );
    b.schritt(
        "VERIFICATION",
        "zweiter Lauf Wort fuer Wort gleich, Redundanzvergleich erkennt beides",
    );

    // ---- 11. STORAGE: das Entgelt eines Skalenpakets ------------------
    let manifest = myl_types::gegenstand::Manifest {
        art: myl_types::gegenstand::Gegenstandsart::Skalenpaket,
        fassung: 1,
        teilzahl: 12,
        wurzel: myl_types::ids::MerkleRoot::new([3; 32]),
        redundanz: myl_types::gegenstand::Redundanzform::Erasure { k: 8, m: 4 },
        laenge: 1_048_576,
    };
    let je_epoche = myl_store::verbrauch_je_epoche(&manifest);
    let mut guthaben = myl_store::Speicherguthaben { byte_epochen: je_epoche * 10 };
    let abrechnung = myl_store::abrechnen(&manifest, &mut guthaben, 12);
    assert_eq!(abrechnung.verbraucht, je_epoche, "es wurde nicht eine Epoche abgerechnet");
    assert!(
        je_epoche > u128::from(manifest.laenge),
        "Erasure kostet mehr Platz als die Nutzdaten, das muss sich im Entgelt zeigen"
    );
    b.schritt(
        "STORAGE",
        format!(
            "1 MiB bei k=8/m=4 kostet {je_epoche} Byte-Epochen je Epoche, {} an 12 Halter",
            abrechnung.je_halter
        ),
    );

    // ---- 12. TOKENOMICS/NODE: Bündel, Epochenschluss, Auszahlung -----
    for w in 0..6u8 {
        myl_ledger::transitions::auszahlungskonto_eintragen(
            kette.zustand_mut(),
            &probekonto(w),
            &MinerId::new(*probekonto(w).as_bytes()),
            kaltes_konto(w),
        )
        .expect("Eintragung");
    }
    let register = myl_ledger::transitions::angemeldete_miner(kette.zustand());
    let zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche(
        &register,
        kette.zustand().epoch.0,
        &kette.zustand().epochensaat,
        4,
    );
    assert_eq!(zuteilung.pods.len(), 1, "aus sechs Minern entstand kein Pod");
    let pod = &zuteilung.pods[0];
    let mut buendel = myl_types::PoIBundle {
        epoch: kette.zustand().epoch,
        pod: myl_types::pod_kennung(kette.zustand().epoch.0, pod.pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente: 1_000,
    };
    // Echte Unterschriften aller Mitglieder, zu einem Aggregat.
    let botschaft = myl_consensus::poi::bundle_message(&buendel);
    let mut teile = Vec::new();
    for m in pod.mitglieder() {
        let w = (0..6u8)
            .find(|w| MinerId::new(*probekonto(*w).as_bytes()) == m.miner_id)
            .expect("Mitglied ist ein Probekonto");
        teile.push(probeschluessel(w).sign(&botschaft).expect("Unterschrift"));
    }
    buendel.aggregate_sig =
        myl_types::bls::BlsSignature(myl_types::bls::aggregate_signatures(&teile).expect("Aggregat").0);
    let koordinator = (0..6u8)
        .find(|w| MinerId::new(*probekonto(*w).as_bytes()) == pod.shards[0].miner.miner_id)
        .expect("der Koordinator ist ein Probekonto");
    kette.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(koordinator),
            nonce[koordinator as usize],
            Anweisung::BuendelEinreichen { buendel },
        )
        .expect("signieren"),
    );
    kette.baue_block();
    assert_eq!(kette.zustand().buendel.len(), 1, "das Buendel kam nicht in den Zustand");

    let vorher: Vec<u64> = (0..6u8)
        .map(|w| kette.zustand().account(&kaltes_konto(w)).balance)
        .collect();
    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        if kette.zustand().epoch.0 == 1 {
            break;
        }
        kette.baue_block();
    }
    assert_eq!(kette.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    let nachher: Vec<u64> = (0..6u8)
        .map(|w| kette.zustand().account(&kaltes_konto(w)).balance)
        .collect();
    let gewachsen = (0..6).filter(|i| nachher[*i] > vorher[*i]).count();
    assert!(gewachsen > 0, "bezeugte Arbeit erreichte kein einziges Konto");
    let summe: u64 = nachher.iter().sum::<u64>() - vorher.iter().sum::<u64>();
    b.schritt(
        "TOKENOMICS/NODE",
        format!(
            "Buendel in Block {}, Epoche 0 geschlossen, {summe} MYL an {gewachsen} von 6 Konten",
            kette.hoehe()
        ),
    );

    // ---- Abschluss: ist jede Station gelaufen? ------------------------
    const ERWARTET: [&str; 12] = [
        "GOVERNANCE",
        "CONSENSUS/NODE",
        "TOKENOMICS",
        "AGENT_LAYER",
        "INTEGER_LLM",
        "COMPUTE_PIPELINE",
        "NETWORKING",
        "GATEWAY",
        "CONSENSUS",
        "VERIFICATION",
        "STORAGE",
        "TOKENOMICS/NODE",
    ];
    assert_eq!(
        b.stationen, ERWARTET,
        "eine Station fehlt oder lief in falscher Reihenfolge"
    );
    eprintln!("\n=== {} Stationen, alle verzahnt gelaufen ===\n", ERWARTET.len());

    let _ = std::fs::remove_dir_all(&verz);
}
