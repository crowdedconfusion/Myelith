//! Vier Prozesse, ein Pod: der Beweis zu Fund 186.
//!
//! # ⚑ Was dieser Test kann, was `shards_ueber_den_draht.rs` nicht kann
//!
//! Dort laufen vier Türen in **einem** Adressraum. Das beweist das
//! Protokoll, nicht die **Isolation**: Vier Fäden können einander
//! sehen, und ein Fehler, der auf geteiltem Zustand beruht, fiele nicht
//! auf.
//!
//! Hier startet jeder Shard als **eigenes Programm** über seine
//! Kommandozeile. Jeder Prozess hält einen Layerbereich und einen
//! Schlüssel; keiner kennt die Adresse eines anderen. **Was hier grün
//! ist, ist auf vier Maschinen grün.**
//!
//! ⚑ **Bis zum 2026-09-06 gab es diesen Test nicht**, und er ist der
//! einzige, der die Kernaussage des Shardings prüft: dass vier
//! verschiedene Miner einen Pod bilden. `zwei_prozesse.rs` prüft den
//! Weg vom Klienten zum Pod; der Pod war darin **eine** Maschine.

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use myl_pod::coordinator::Coordinator;
use myl_pod::shardweg::UeberDenDraht;
use myl_types::ids::{EpochId, PodId};

const SHARDS: usize = 4;
const PROMPT: [u32; 6] = [9707, 374, 264, 1273, 315, 279];

fn wurzel() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("..").join("..")
}

fn artefakte() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

fn binary() -> PathBuf {
    // ⚑ Für die Messung ein Freigabebau von aussen (2026-11-15): Der Testbau
    //   rechnet mit Überlaufprüfungen und ist durch das Rechenwerk begrenzt,
    //   nicht durch den Speicher; Bündeln zeigte dort nichts.
    if let Ok(p) = std::env::var("MYL_SHARD_PROGRAMM") {
        return PathBuf::from(p);
    }
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("SYSTEM/full-build").join(profil).join("myl-shard")
}

/// Startet einen Shard und liest seine Adresse von der Standardausgabe.
fn starten(nummer: usize, bereich: Option<(usize, usize)>, kerne: Option<usize>) -> (Child, (std::net::SocketAddr, myl_pod::shardweg::Ausweis)) {
    let (ausweisdatei, ausweis) = ausweis_anlegen(nummer);
    let mut befehl = Command::new(binary());
    befehl
        .arg("--ausweis")
        .arg(&ausweisdatei)
        .arg("--artefakte")
        .arg(artefakte())
        .arg("--shard")
        .arg(nummer.to_string())
        .arg("--shards")
        .arg(SHARDS.to_string())
        .arg("--probeschluessel");
    if let Some(n) = kerne {
        befehl.arg("--kerne").arg(n.to_string());
    }
    if let Some((von, bis)) = bereich {
        befehl.arg("--von").arg(von.to_string()).arg("--bis").arg(bis.to_string());
    }
    let mut kind = befehl
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("myl-shard startet");
    let aus = kind.stdout.take().expect("Standardausgabe");
    let mut leser = std::io::BufReader::new(aus);
    let mut zeile = String::new();
    // ⚑ **Die Fehlerausgabe wandert in die Panik**, falls die Adresse
    // ausbleibt. Ohne das sähe ein abgelehntes Artefakt aus wie „der
    // Shard hat seine Adresse nicht genannt"; genau diese Verwechslung
    // hat `zwei_prozesse.rs` einmal Stunden gekostet.
    if leser.read_line(&mut zeile).unwrap_or(0) == 0 {
        let mut fehler = String::new();
        if let Some(mut e) = kind.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut fehler);
        }
        let _ = kind.kill();
        panic!("Shard {nummer} nannte keine Adresse.\nFehlerausgabe:\n{fehler}");
    }
    let adresse = zeile
        .trim()
        .strip_prefix(&format!("SHARD {nummer} "))
        .unwrap_or_else(|| panic!("unerwartete Zeile von Shard {nummer}: {zeile:?}"))
        .parse()
        .expect("Adresse lesbar");
    // Fund 638: Der Shard hat seinen Ausweis gelesen, bevor er seine
    // Adresse nennt; danach braucht den Ordner niemand mehr.
    if let Some(ordner) = ausweisdatei.parent() {
        let _ = std::fs::remove_dir_all(ordner);
    }
    (kind, (adresse, ausweis))
}

/// Der Ausweis der Tür (Fund 611): je Shard eigene 32 Bytes, mit `0600`
/// abgelegt, wie der Knoten es tut.
fn ausweis_anlegen(nummer: usize) -> (PathBuf, myl_pod::shardweg::Ausweis) {
    // Ein Ordner je Aufruf: Die Tests dieser Datei laufen nebeneinander.
    static LAUF: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let lauf = LAUF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ordner = std::env::temp_dir().join(format!("myl-shardausweis-{}-{lauf}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ausweis = [nummer as u8 + 0x41; 32];
    let datei = ordner.join(format!("ausweis-{nummer}"));
    myl_types::ortsleitung::schluessel_ablegen(&datei, &ausweis).expect("Ausweis");
    (datei, ausweis)
}

/// ⚑ **Vier Prozesse rechnen denselben Pod wie einer.**
#[test]
fn vier_prozesse_rechnen_wie_ein_prozess() {
    vier_prozesse(None);
}

/// ⚑ **Und mit ungleichen Bereichen ueber `--von` und `--bis` dasselbe**
/// (Stufe 4 des Dispatchers, 2026-10-21): 3, 7, 13 und 5 Ebenen statt
/// je 7. Token, Abdruck, Arbeit und Segmente muessen gleich sein, denn der
/// Zuschnitt aendert keine Zahl.
#[test]
fn ungleiche_bereiche_rechnen_dasselbe() {
    vier_prozesse(Some([(0, 3), (3, 10), (10, 23), (23, 28)]));
}

fn vier_prozesse(bereiche: Option<[(usize, usize); 4]>) {
    if !myl_pod::artefakte::vorhanden(&artefakte()) {
        return;
    }
    assert!(
        binary().exists(),
        "myl-shard ist nicht gebaut.\nBauen mit:\n  \
         cd COMPUTE_PIPELINE/myl-pod && cargo build --bin myl-shard"
    );

    let mut kinder: Vec<Child> = Vec::new();
    let mut adressen = Vec::new();
    for n in 0..SHARDS {
        let (kind, adresse) = starten(n, bereiche.map(|b| b[n]), None);
        kinder.push(kind);
        adressen.push(adresse);
    }
    // ⚑ **Aufräumen, was auch geschieht.** Ein zurückgelassener Shard
    // hält seinen Port und liesse den nächsten Lauf scheitern, ohne dass
    // die Meldung sagte, warum.
    struct Aufraeumer(Vec<Child>);
    impl Drop for Aufraeumer {
        fn drop(&mut self) {
            for k in &mut self.0 {
                let _ = k.kill();
                let _ = k.wait();
            }
        }
    }
    let _wache = Aufraeumer(kinder);

    let pod = PodId::new([0xAA; 32]);
    let weg = Arc::new(
        UeberDenDraht::neu(adressen).mit_frist(std::time::Duration::from_secs(180)),
    );
    let koordinator =
        Coordinator::ueber_weg(pod, EpochId(0), weg).expect("Besetzung erhoben");
    let token = koordinator.run_prompt(1, &PROMPT, 8);
    let abdruck = koordinator.dekodier_digest(1).expect("Abdruck");
    let buendel = koordinator.build_signed_poi_bundle().expect("Buendel");

    // ⚑ **Der Vergleichswert steht fest und nicht als zweiter Lauf:**
    // Ein Test, der sich selbst vergleicht, prüft nur, dass er zweimal
    // dasselbe tut.
    //
    // 📌 **Neu erhoben am 2026-09-11** mit dem Ankermodell Qwen3-0,6B,
    // und zwar **unabhängig** vom Pod:
    //
    // ```text
    // cargo run --release --bin integer-llm-runtime -- \
    //     artifacts/sparrow-0.6b-int8 "Hello is a test of the" 8
    // ```
    //
    // Dass die vier Prozesse dasselbe liefern, ist damit eine Aussage
    // über den Shardweg und nicht über die Erwartung im Test.
    assert_eq!(
        token,
        vec![1849, 13, 5209, 3410, 264, 2033, 429, 374],
        "vier Prozesse rechnen andere Token als ein Prozess"
    );
    // ⛔️ **Fund 498 (2026-09-28): Dieser Wert war seit dem Neubau des
    // Ankerartefakts am 2026-09-21 veraltet**, und der Test damit rot, ohne
    // dass es auffiel. Der Abdruck umfasst die Logits und haengt deshalb am
    // Artefakt, nicht nur am Code. Neu erhoben auf zwei Wegen, die beide
    // `2a13b710…` liefern: vier Shards **in einem Prozess** ueber
    // `Coordinator::new` (derselbe Aufbau wie `shards_ueber_den_draht.rs`)
    // und dieser Test auf dem Stand des Commits `1445da2`, also vor den
    // Aenderungen desselben Tages. Die Token sind unveraendert.
    //
    // 📌 **Ein fest eingetragener Wert, der am Artefakt haengt, muss mit
    // dem Artefakt neu erhoben werden.** Die CI hat kein Artefakt und
    // ueberspringt den Test; rot war er nur auf der einen Maschine, die ihn
    // haette zeigen koennen, und dort lief er eine Woche lang nicht.
    assert_eq!(
        abdruck.as_ref().map(|(h, s)| (h.as_str(), *s)),
        Some(("2a13b7105cf433ad8a2adc32b0f19914fc3347c0802d73c055ef024f00e87f71", 8)),
        "der Dekodier-Abdruck weicht ab"
    );
    // 📌 **Neu erhoben am 2026-09-11** mit dem Ankermodell Qwen3-0,6B:
    // Der Wert haengt am Modellprofil (28 Ebenen, hidden 1024 statt 24
    // und 896) und aendert sich mit ihm. Die **Segmentzahl** daneben
    // aendert sich nicht, und genau die ist die Aussage.
    assert_eq!(buendel.vtfe_claimed, 12_999_998, "die beanspruchte Arbeit weicht ab");
    // ⚑ **Dreizehn und nicht acht** (Fund 187): Ein Segment ist ein
    // Vorwaertspass, nicht ein gesampeltes Token. Fuenf Prefill-
    // Positionen und acht gesampelte ergeben dreizehn.
    //
    // ⚑ **Hier stand bis zum 2026-09-06 eine Eins im Buendel**, obwohl
    // dreizehn Segmente darunter hingen. Aus dieser Zahl zieht die
    // Stichprobe der Stufe 2; zwoelf Segmente waren damit unziehbar.
    assert_eq!(buendel.segmente, 13, "die Segmentzahl weicht ab");

    eprintln!(
        "\n--- Vier Prozesse, ein Pod ---\n  Token: {token:?}\n  \
         Abdruck: {abdruck:?}\n  vTFE: {}, Segmente: {}",
        buendel.vtfe_claimed, buendel.segmente
    );
}

/// ⚑ **Durchsatz mit Sitzungen nebeneinander** (2026-11-15, Whitepaper 4.2:
/// „Während Shard 3 Token t berechnet, verarbeitet Shard 1 bereits Token
/// t+2“). Vier Shardprozesse, je Durchgang 1, 2, 4, 8 oder 16 Sitzungen zugleich
/// über denselben Koordinator; gemessen wird die Wandzeit je Durchgang, und
/// jede Sitzung muss bitgleich die Token des Einzellaufs liefern.
///
/// Eine Messung, kein Prüfstein: Zeiten auf einer geteilten Maschine
/// schwanken; von Hand mit
/// `cargo test --test vier_prozesse_ein_pod -- --ignored --nocapture`.
///
/// ⚠️ **Mit einem Freigabebau der Shards messen**
/// (`MYL_SHARD_PROGRAMM=<pfad>`, gebaut mit `--release --features
/// integer-llm-runtime/cpu-simd,integer-llm-runtime/pruefnetz` in einen
/// eigenen Zielordner): Der Testbau rechnet mit Überlaufprüfungen, ist
/// durch das Rechenwerk begrenzt und zeigte vom Bündeln nichts.
///
/// 📌 **Gemessen am 2026-11-15** (M5 Pro, `sparrow-0.6b-int8`, Freigabebau,
/// je Sitzung 6 Prompt- und 16 neue Positionen, eine Sitzung 239 ms), als
/// Durchsatz gegen eine Sitzung, ohne und mit Stapeln:
/// 4 Sitzungen 1,79 und 1,82; 8 Sitzungen 1,87 und 1,96; 16 Sitzungen 1,81
/// und 2,25. Mit drei Kernen je Shard bei 16 Sitzungen 1,81 und 2,14.
/// **Die Decke ist die langsamste Stufe** (Whitepaper 4.2), bei gleich
/// vielen Ebenen je Shard der mit dem LM-Kopf (eine Sitzung, ms je Shard:
/// 52, 50, 50, 89). Gegenproben im Testbau: Ein Kern je Shard ändert nichts
/// (kein Streit um Kerne); Bereiche nach Last (9, 9, 8, 2 Ebenen) hoben 4
/// Sitzungen von 2,16 auf 2,76; so schneidet im Netz der Scheduler
/// (`myl_scheduler::ebenenverteilung`, der Kopf mit `kopf_zeit`).
///
/// ⚑ **Viermal gemessen:** ohne Grenze und mit `--kerne 3` je Shard, je ohne
/// und mit Stapeln. Ohne Grenze nimmt jeder Shard alle Kerne der Maschine,
/// mit drei je Shard (zwölf von fünfzehn) steht jeder für sich wie auf
/// einer eigenen Maschine. „Ohne Stapel“ zerlegt der Messweg jeden
/// gemischten Stapel in einen Stapel je Sitzung; derselbe Koordinator, nur
/// das Bündeln über Sitzungen aus.
#[test]
#[ignore = "Messung, von Hand"]
fn durchsatz_mit_sitzungen_nebeneinander() {
    if !myl_pod::artefakte::vorhanden(&artefakte()) {
        return;
    }
    assert!(binary().exists(), "myl-shard ist nicht gebaut");
    for (kerne, einzeln) in [(None, true), (None, false), (Some(3), true), (Some(3), false)] {
        let messung = durchsatz_messen(kerne, einzeln);
        let (_, eins) = messung[0];
        let grenze = format!(
            "{}, {}",
            kerne.map_or("ohne Grenze".to_string(), |k| format!("je Shard {k} Kerne")),
            if einzeln { "ohne Stapel" } else { "mit Stapeln" }
        );
        eprintln!("\n--- Durchsatz, vier Prozesse, {grenze}, {} + {NEUE} Positionen je Sitzung ---", PROMPT.len());
        for (n, ms) in &messung {
            // Gewinn in Promille gegen eine Sitzung: n Sitzungen in der Zeit,
            // die eine allein brauchte, wären 1000 * n.
            let gewinn = (eins * (*n as u128) * 1000).checked_div(*ms).unwrap_or(0);
            eprintln!("  {n} zugleich: {ms} ms, Durchsatz {gewinn} Promille einer Sitzung");
        }
    }
}

const NEUE: u64 = 16;

/// Je Zahl gleichzeitiger Sitzungen (1, 2, 4) die Wandzeit in ms.
fn durchsatz_messen(kerne: Option<usize>, einzeln: bool) -> Vec<(usize, u128)> {
    let mut kinder: Vec<Child> = Vec::new();
    let mut adressen = Vec::new();
    for n in 0..SHARDS {
        let (kind, adresse) = starten(n, None, kerne);
        kinder.push(kind);
        adressen.push(adresse);
    }
    struct Aufraeumer(Vec<Child>);
    impl Drop for Aufraeumer {
        fn drop(&mut self) {
            for k in &mut self.0 {
                let _ = k.kill();
                let _ = k.wait();
            }
        }
    }
    let _wache = Aufraeumer(kinder);
    let weg = Arc::new(Gestoppt {
        innen: UeberDenDraht::neu(adressen).mit_frist(std::time::Duration::from_secs(600)),
        mikros: (0..SHARDS).map(|_| std::sync::atomic::AtomicU64::new(0)).collect(),
        stapel: (0..SHARDS).map(|_| Default::default()).collect(),
        einzeln,
    });
    let koordinator =
        Coordinator::ueber_weg(PodId::new([0xAA; 32]), EpochId(0), Arc::clone(&weg) as Arc<dyn myl_pod::shardweg::Shardweg>)
            .expect("Besetzung erhoben");
    // Aufwärmen und Vergleichswert in einem; dabei die Zeit je Shard (mit
    // dem Weg dorthin), denn den Durchsatz begrenzt die langsamste Stufe.
    for m in &weg.mikros {
        m.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    let referenz = koordinator.rechne(1, &PROMPT, NEUE).token;
    let je_shard: Vec<u64> = weg.mikros.iter().map(|m| m.load(std::sync::atomic::Ordering::Relaxed) / 1000).collect();
    eprintln!("  eine Sitzung, ms je Shard (mit Weg): {je_shard:?}");
    koordinator.sitzung_abschliessen(1);
    assert_eq!(referenz.len() as u64, NEUE);
    let mut aus = Vec::new();
    for n in [1usize, 2, 4, 8, 16] {
        let beginn = std::time::Instant::now();
        let laeufe: Vec<myl_pod::coordinator::Lauf> = std::thread::scope(|faeden| {
            let h: Vec<_> = (0..n)
                .map(|i| {
                    let k = &koordinator;
                    faeden.spawn(move || k.rechne((100 * n + i) as u64, &PROMPT, NEUE))
                })
                .collect();
            h.into_iter().map(|l| l.join().expect("Lauf")).collect()
        });
        let ms = beginn.elapsed().as_millis();
        for (i, l) in laeufe.iter().enumerate() {
            assert_eq!(l.token, referenz, "Sitzung {i} von {n} rechnet andere Token");
            koordinator.sitzung_abschliessen((100 * n + i) as u64);
        }
        let stapel: Vec<(u64, u64)> = weg
            .stapel
            .iter()
            .map(|(a, b)| (a.swap(0, std::sync::atomic::Ordering::Relaxed), b.swap(0, std::sync::atomic::Ordering::Relaxed)))
            .collect();
        eprintln!("  {n} zugleich: gemischte Stapel je Shard (Zahl, Nachrichten) {stapel:?}");
        aus.push((n, ms));
    }
    aus
}

/// Ein Weg zu den Shards mit Stoppuhr: summiert die Zeit je Shard.
struct Gestoppt {
    innen: UeberDenDraht,
    mikros: Vec<std::sync::atomic::AtomicU64>,
    /// Je Shard: Stapel mit mehr als einer Sitzung, Nachrichten darin.
    stapel: Vec<(std::sync::atomic::AtomicU64, std::sync::atomic::AtomicU64)>,
    /// Zum Vergleich: einen gemischten Stapel Nachricht für Nachricht
    /// rechnen lassen, also Pipelining ohne Bündeln.
    einzeln: bool,
}

impl myl_pod::shardweg::Shardweg for Gestoppt {
    fn shardzahl(&self) -> usize {
        self.innen.shardzahl()
    }

    fn frage(
        &self,
        nummer: usize,
        anfrage: &myl_pod::shardweg::Shardanfrage,
    ) -> Result<myl_pod::shardweg::Shardantwort, String> {
        if let (myl_pod::shardweg::Shardanfrage::RechneStapel(ms), Some((n, m))) = (anfrage, self.stapel.get(nummer)) {
            if ms.iter().any(|x| x.session_id != ms[0].session_id) {
                n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                m.fetch_add(ms.len() as u64, std::sync::atomic::Ordering::Relaxed);
                if self.einzeln {
                    // Je Sitzung ein eigener Stapel (der Prefill bleibt also
                    // gestapelt wie vor dem Bündeln über Sitzungen), in der
                    // Folge des Stapels zurückgegeben.
                    let mut aus: Vec<Option<myl_pod::shard::ShardOut>> = (0..ms.len()).map(|_| None).collect();
                    let mut sitzungen: Vec<u64> = Vec::new();
                    for x in ms {
                        if !sitzungen.contains(&x.session_id) {
                            sitzungen.push(x.session_id);
                        }
                    }
                    for s in sitzungen {
                        let stellen: Vec<usize> = (0..ms.len()).filter(|&k| ms[k].session_id == s).collect();
                        let teil: Vec<myl_pod::wire::PodMessage> = stellen.iter().map(|&k| ms[k].clone()).collect();
                        let outs = if teil.len() == 1 {
                            vec![self.rechne(nummer, &teil[0])?]
                        } else {
                            match self.frage(nummer, &myl_pod::shardweg::Shardanfrage::RechneStapel(teil))? {
                                myl_pod::shardweg::Shardantwort::StapelGerechnet(o) => o,
                                andere => return Err(format!("unerwartet: {andere:?}")),
                            }
                        };
                        for (k, o) in stellen.into_iter().zip(outs) {
                            aus[k] = Some(o);
                        }
                    }
                    return Ok(myl_pod::shardweg::Shardantwort::StapelGerechnet(aus.into_iter().flatten().collect()));
                }
            }
        }
        let beginn = std::time::Instant::now();
        let aus = self.innen.frage(nummer, anfrage);
        if let Some(m) = self.mikros.get(nummer) {
            m.fetch_add(beginn.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
        }
        aus
    }
}
