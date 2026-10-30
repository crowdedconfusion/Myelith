//! **Der Lastgeber für Netzläufe** (2026-10-27): viele Absender schicken
//! unterschriebene Burns über die Relaisleitung eines Knotens, in Stufen
//! steigender Rate. Gemessen wird je Stufe, wie viel davon die Kette
//! aufnimmt und wie der Grundpreis der Gebühr darauf antwortet; danach,
//! wie schnell er wieder auf den Boden fällt.
//!
//! ```text
//! lastgeber <relais> <ausweis> <absender> <raten je Sekunde, mit Komma> <sekunden je Stufe>
//! ```
//!
//! Je Stufe eine Zeile auf stdout, beginnend mit `[last]`; Rückgabe 0, auch
//! wenn die Kette nicht alles aufnimmt (das ist die Messung), 1 bei einem
//! Fehler der Umgebung.
//!
//! ⚑ **Nur für Probeläufe und deshalb kein ausgeliefertes Programm**: Die
//! Absender leiten ihre Schlüssel aus einem festen Text ab, und das Geld
//! kommt aus den Probekonten, deren Schlüssel jeder kennt.

use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use myl_consensus::block::{Anweisung, Transaktion};
use myl_types::bls::BlsSecretKey;
use myl_types::hash::Hash;
use myl_types::ids::Address;
use myl_types::relais::{Kontostand, Relaisantwort, Relaisfrage};

/// Wie lange eine einzelne Frage an die Leitung warten darf.
const FRIST: Duration = Duration::from_secs(10);
/// So viel bekommt jeder Absender mindestens, in Kleinstbeträgen: 0,1 MYL,
/// rund achtzig Burns am Boden der Probekette; nachgefüllt vor jeder Stufe
/// nach deren Bedarf ([`kosten_je_burn`]).
/// 📌 Mit 0,05 MYL und ohne Nachfüllen waren die hundert Absender nach rund
/// 29 000 Burns leer (Nachtlauf vom 2026-10-27 nach der Stufe 400/s, kurzer
/// Lauf vom 2026-10-28 nach 800/s); was danach kam, wartete ungedeckt und
/// verfiel, und die Stufe sah aus wie ein Einbruch des Netzes.
const AUSSTATTUNG: u64 = 100_000;
/// Die Probekonten, aus denen ausgestattet wird: alle, die reichsten zuerst.
///
/// 📌 **Fund 671 (2026-10-28, kurzer Podlauf): Die Ausstattung vor der
/// zweiten Stufe sandte 600 s lang nichts.** Hier standen die Konten 6 bis
/// 11 mit der Annahme, der Probehahn der Knoten nehme nur 0 bis 5; er nimmt
/// aus allen zwölf, und der Testverkehr der Knoten läuft über einige davon.
/// Zugleich kostet ein Burn seit den Gebühreneinheiten am Boden rund 1 200
/// statt rund 200 Kleinstbeträge, und die Ausstattung rechnete noch mit 250
/// je Burn. Jetzt werden die Quellen je Runde nach ihrem Guthaben gewählt,
/// und jede Runde meldet ihren Stand auf der Fehlerausgabe.
const QUELLEN: std::ops::Range<u8> = 0..myl_node::kette::PROBEKONTEN;

/// So viel bleibt auf jedem Probekonto liegen: 6 MYL, zwei Zapfungen des
/// Probehahns der Knoten (je 2 MYL Betrag, dazu 1 MYL Polster). Der
/// Testverkehr der Knoten leert seine Konten ohnehin; die übrigen müssen den
/// Probehahn eine Nacht lang tragen (im Nachtlauf vom 2026-10-27 vierzehn
/// Zapfungen, im kurzen Lauf vom 2026-10-28 zwanzig).
///
/// 📌 **Fund 672 (2026-10-28, kurzer Podlauf):** Die Deckelung auf drei
/// Viertel des jeweiligen Rests räumte die Probekonten Stufe um Stufe ab
/// (vor 400/s noch 20,8 MYL, vor 1 600/s noch 1,3), zuerst die drei, die der
/// Testverkehr nicht leert; danach fand der Probehahn kein Konto mehr, und
/// der späte Einsteiger konnte sich nicht anmelden.
///
/// 📌 **Fund 675 (2026-10-28, Nachtlauf):** Mit 30 Knoten leerte schon der
/// Testverkehr die Konten bis auf diese Reserve, und vor der ersten Stufe
/// lagen über ihr zusammen nur 2,9 MYL; die Stufen ab 400/s hatten nichts
/// mehr. Seitdem füllt jeder Epochenwechsel ein Konto unter der Hälfte
/// wieder auf, und die Reserve muss nur noch bis dahin tragen: eine
/// Zapfung des Hahns (höchstens 2 MYL und sein Polster von 1 MYL) und
/// etwas Luft.
const RESERVE_JE_KONTO: u64 = 4 * myl_tokenomics::UNITS_PER_MYL;

/// Rund so viele Bytes hat eine Überweisung oder ein Burn.
const BYTES_JE_TX: u64 = 200;

/// **Was ein Burn von 1 kostet** beim Grundpreis `grundpreis`: der Betrag
/// und die Gebühr nach Einheiten (Bytes plus eine Prüfeinheit).
fn kosten_je_burn(grundpreis: u64) -> u64 {
    1 + gebuehr(grundpreis)
}

/// Die Gebühr einer Transaktion beim Grundpreis `grundpreis`, wie die Kette
/// sie rechnet (`Transaktion::gebuehreneinheiten`), mit runden Bytes.
fn gebuehr(grundpreis: u64) -> u64 {
    grundpreis.max(1) * (BYTES_JE_TX + myl_consensus::block::PRUEFEINHEIT)
}
/// So viele Überweisungen je Quelle und Runde: unter der Grenze des
/// Mempools je Absender (64).
const JE_RUNDE: usize = 48;
/// So weit darf ein Absender vor der Nummer im Zustand liegen, bevor der
/// Lastgeber ihn zurücksetzt: unter dem Deckel des Mempools je Absender (64).
const VORAUS_HOECHSTENS: u64 = 48;
/// So viele Fäden senden zugleich. 📌 Mit vier kamen höchstens rund 180
/// Sendungen je Sekunde hinaus (je eine Verbindung je Sendung); die Stufen
/// darüber hätten den Lastgeber gemessen statt des Knotens.
const FAEDEN: u64 = 16;
/// So oft schickt ein Faden seinen Stapel, auch wenn er nicht voll ist.
const STAPEL_TAKT: Duration = Duration::from_millis(50);

#[derive(Clone)]
struct Leitung {
    adresse: std::net::SocketAddr,
    ausweis: [u8; myl_types::zeitprobe::AUSWEIS_LEN],
}

impl Leitung {
    fn stand(&self, a: &Address) -> Result<Kontostand, String> {
        match myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Konto { adresse: *a }, FRIST)? {
            Relaisantwort::Konto(k) => Ok(k),
            andere => Err(format!("unerwartete Antwort: {andere:?}")),
        }
    }

    /// Schickt eine Transaktion; `Err` mit der Antwort der Leitung, wenn
    /// sie nicht hinausging.
    /// Schickt einen Stapel; Ergebnis: wie viele der Mempool nahm.
    fn senden_gebuendelt(&self, txs: &[Transaktion]) -> Result<usize, String> {
        let txs: Vec<Vec<u8>> = txs.iter().filter_map(|t| borsh::to_vec(t).ok()).collect();
        match myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Transaktionen { txs }, FRIST)? {
            Relaisantwort::Erledigt(z) => z
                .split_whitespace()
                .nth(1)
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| format!("unerwartete Antwort: {z}")),
            andere => Err(format!("{andere:?}")),
        }
    }

    #[allow(dead_code)]
    fn senden(&self, tx: &Transaktion) -> Result<(), String> {
        let bytes = borsh::to_vec(tx).map_err(|e| e.to_string())?;
        match myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Transaktion { tx: bytes }, FRIST)? {
            Relaisantwort::Erledigt(_) => Ok(()),
            andere => Err(format!("{andere:?}")),
        }
    }
}

fn schluessel(text: &str) -> Result<BlsSecretKey, String> {
    BlsSecretKey::key_gen(Hash::sha256(text.as_bytes()).as_bytes()).map_err(|e| format!("{e:?}"))
}

fn adresse(sk: &BlsSecretKey) -> Result<Address, String> {
    Ok(Address::aus_schluessel(&sk.public_key().map_err(|e| format!("{e:?}"))?))
}

/// Stattet jeden Absender unter `ziel` mit `ziel` aus, in Runden je Quelle,
/// bis alle gedeckt sind.
fn ausstatten(l: &Leitung, absender: &[(BlsSecretKey, Address)], mut ziel: u64) -> Result<(), String> {
    let beginn = Instant::now();
    let mut gewartet = false;
    loop {
        let mut offen = Vec::new();
        for (_, a) in absender {
            if l.stand(a)?.guthaben < ziel {
                offen.push(*a);
            }
        }
        if offen.is_empty() {
            println!("[last] {} Absender ausgestattet in {} s", absender.len(), beginn.elapsed().as_secs());
            return Ok(());
        }
        if beginn.elapsed() > Duration::from_secs(600) {
            return Err(format!("{} Absender nach 600 s noch ohne Ausstattung", offen.len()));
        }
        let mut rest = offen.as_slice();
        let mut quellen = Vec::new();
        for q in QUELLEN {
            let sk = myl_node::kette::probeschluessel(q);
            let stand = l.stand(&adresse(&sk)?)?;
            quellen.push((q, sk, stand));
        }
        quellen.sort_by_key(|(_, _, s)| std::cmp::Reverse(s.guthaben));
        // Ausgegeben wird nur, was über der Reserve je Konto liegt; die
        // braucht der Probehahn der Knoten (Fund 672). Reicht es nicht, wird
        // die Ausstattung gedeckelt und gemeldet: Die Stufe misst dann, bis
        // die Absender leer sind.
        let gesamt: u64 = quellen.iter().map(|(_, _, s)| s.guthaben.saturating_sub(RESERVE_JE_KONTO)).sum();
        // 📌 Gedeckelt wird einmal und dann gemessen: Ein Deckel, der je
        //    Runde neu aus dem schrumpfenden Rest käme, senkte das Ziel immer
        //    weiter und käme nie an (kurzer Podlauf vom 2026-10-28, Abbruch
        //    nach 600 s).
        let hoechstens = gesamt / offen.len() as u64;
        // ⚑ **Erst auf den Wechsel warten** (2026-10-29): Er füllt die
        //   Probekonten unter der Hälfte wieder auf (Fund 675). 📌 Im ersten
        //   Durchsatzlauf waren die Stufen ab 2000/s gedeckelt und maßen das
        //   Guthaben statt des Netzes.
        if ziel > hoechstens && !gewartet {
            gewartet = true;
            let epoche = l.stand(&absender[0].1)?.epoche;
            println!("[last] Guthaben reicht nicht ({hoechstens} statt {ziel} je Absender), warte auf den Wechsel");
            let warten = Instant::now();
            while l.stand(&absender[0].1)?.epoche == epoche && warten.elapsed() < Duration::from_secs(300) {
                std::thread::sleep(Duration::from_secs(2));
            }
            // Die Auffüllung steht im ersten Block der neuen Epoche.
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        let gedeckelt = ziel > hoechstens;
        if gedeckelt {
            println!("[last] Ausstattung gedeckelt: {hoechstens} statt {ziel} je Absender (ueber der Reserve zusammen {gesamt})");
            ziel = hoechstens.max(1);
        }
        for (q, sk, stand) in quellen {
            if rest.is_empty() {
                break;
            }
            let bedarf = ziel + stand.aufschlag_konto + gebuehr(stand.grundpreis_je_byte);
            let zahl = rest.len().min(JE_RUNDE).min((stand.guthaben.saturating_sub(RESERVE_JE_KONTO) / bedarf) as usize);
            let mut fehler = Vec::new();
            for (i, nach) in rest[..zahl].iter().enumerate() {
                let anweisung = Anweisung::Ueberweisung { nach: *nach, betrag: ziel };
                let tx = Transaktion::signiere(&stand.netzwert, &sk, stand.nonce + i as u64, anweisung)
                    .map_err(|e| format!("{e:?}"))?;
                if let Err(e) = l.senden(&tx) {
                    fehler.push(e);
                }
            }
            // Auf die Fehlerausgabe, damit ein Abbruch sagt, woran es hing.
            eprintln!(
                "[last] ausstatten nach {} s: Quelle {q} Guthaben {} Nonce {} sendet {zahl}, {} abgewiesen{}",
                beginn.elapsed().as_secs(),
                stand.guthaben,
                stand.nonce,
                fehler.len(),
                fehler.first().map(|e| format!(" (zuerst: {e})")).unwrap_or_default()
            );
            rest = &rest[zahl..];
        }
        std::thread::sleep(Duration::from_secs(10));
        if gedeckelt {
            return Ok(());
        }
    }
}

/// Eine Stufe: `rate` Burns je Sekunde für `dauer`, verteilt auf die Fäden.
fn stufe(l: &Leitung, absender: &[(BlsSecretKey, Address)], rate: u64, dauer: Duration) -> Result<(), String> {
    let mut vorher = Vec::with_capacity(absender.len());
    for (_, a) in absender {
        vorher.push(l.stand(a)?.nonce);
    }
    let netzwert = l.stand(&absender[0].1)?.netzwert;
    let gesendet = Arc::new(AtomicU64::new(0));
    let abgewiesen = Arc::new(AtomicU64::new(0));
    let preis_max = Arc::new(AtomicU64::new(0));
    let ende = Instant::now() + dauer;
    let mut faeden = Vec::new();
    for t in 0..FAEDEN {
        let (l, gesendet, abgewiesen) = (l.clone(), gesendet.clone(), abgewiesen.clone());
        let meine: Vec<(BlsSecretKey, Address, u64)> = absender
            .iter()
            .zip(&vorher)
            .enumerate()
            .filter(|(i, _)| *i as u64 % FAEDEN == t)
            .map(|(_, ((sk, a), n))| (sk.clone(), *a, *n))
            .collect();
        let takt = Duration::from_micros((1_000_000 * FAEDEN / rate.max(1)).max(1));
        // ⚑ **Gebündelt** (2026-10-29): Je Faden werden die unterschriebenen
        //   Transaktionen gesammelt und alle [`STAPEL_TAKT`] als ein Stapel
        //   geschickt. 📌 Einzeln (eine Verbindung und eine Einzelprüfung je
        //   Transaktion) kam der Lastgeber im Nachtlauf vom 2026-10-29 nicht
        //   über rund 350 je Sekunde, gleich welche Stufe; gemessen wurde die
        //   Leitung, nicht das Netz.
        faeden.push(std::thread::spawn(move || {
            let mut naechste: Vec<u64> = meine.iter().map(|(_, _, n)| *n).collect();
            let mut k = 0usize;
            let mut soll = Instant::now();
            let mut stapel: Vec<(usize, Transaktion)> = Vec::new();
            let mut zuletzt = Instant::now();
            let abschicken = |stapel: &mut Vec<(usize, Transaktion)>, naechste: &mut Vec<u64>| {
                if stapel.is_empty() {
                    return;
                }
                let txs: Vec<Transaktion> = stapel.iter().map(|(_, t)| t.clone()).collect();
                match l.senden_gebuendelt(&txs) {
                    Ok(a) => {
                        gesendet.fetch_add(a as u64, Ordering::Relaxed);
                        abgewiesen.fetch_add((txs.len() - a.min(txs.len())) as u64, Ordering::Relaxed);
                        if a < txs.len() {
                            // 📌 Die Nummern neu aus dem Zustand, wie eine
                            //    Wallet, für jeden Absender im Stapel.
                            for (i, _) in stapel.iter() {
                                if let Ok(s) = l.stand(&meine[*i].1) {
                                    naechste[*i] = s.nonce;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        abgewiesen.fetch_add(txs.len() as u64, Ordering::Relaxed);
                    }
                }
                stapel.clear();
            };
            while Instant::now() < ende && !meine.is_empty() {
                let i = k % meine.len();
                // Je Runde ein Absender gegen den Zustand: Verfällt eine
                // angenommene Transaktion im Mempool, liegt er sonst für
                // immer vor dem Zustand.
                if i == 0 {
                    let j = (k / meine.len()) % meine.len();
                    if let Ok(s) = l.stand(&meine[j].1) {
                        if naechste[j] > s.nonce + VORAUS_HOECHSTENS {
                            naechste[j] = s.nonce;
                        }
                    }
                }
                k += 1;
                let anweisung = Anweisung::Burn { betrag: 1 };
                if let Ok(tx) = Transaktion::signiere(&netzwert, &meine[i].0, naechste[i], anweisung) {
                    naechste[i] += 1;
                    stapel.push((i, tx));
                }
                if stapel.len() >= myl_types::relais::TRANSAKTIONEN_JE_FRAGE || zuletzt.elapsed() >= STAPEL_TAKT {
                    abschicken(&mut stapel, &mut naechste);
                    zuletzt = Instant::now();
                }
                soll += takt;
                if let Some(warten) = soll.checked_duration_since(Instant::now()) {
                    std::thread::sleep(warten);
                }
            }
            abschicken(&mut stapel, &mut naechste);
        }));
    }
    // Der Grundpreis während der Stufe, einmal je halbe Sekunde.
    while Instant::now() < ende {
        if let Ok(s) = l.stand(&absender[0].1) {
            preis_max.fetch_max(s.grundpreis_je_byte, Ordering::Relaxed);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    for f in faeden {
        let _ = f.join();
    }
    // ⚑ **Nachlauf bis alles in Blöcken steht** (2026-10-29): gemessen,
    //   wie lange das dauert, höchstens 60 s; was danach fehlt, ging
    //   verloren (angenommen, aber nie in einem Block).
    let angenommen = gesendet.load(Ordering::Relaxed);
    let nachlauf = Instant::now();
    let mut aufgenommen;
    loop {
        aufgenommen = 0u64;
        for ((_, a), n) in absender.iter().zip(&vorher) {
            aufgenommen += l.stand(a)?.nonce.saturating_sub(*n);
        }
        if aufgenommen >= angenommen || nachlauf.elapsed() > Duration::from_secs(60) {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let sekunden = dauer.as_secs().max(1);
    println!(
        "[last] stufe={rate}/s gesendet={} abgewiesen={} aufgenommen={} je_sekunde={} grundpreis_max={} nachlauf_ms={} verloren={}",
        angenommen,
        abgewiesen.load(Ordering::Relaxed),
        aufgenommen,
        aufgenommen / sekunden,
        preis_max.load(Ordering::Relaxed),
        nachlauf.elapsed().as_millis(),
        angenommen.saturating_sub(aufgenommen)
    );
    Ok(())
}

/// Wie lange der Grundpreis braucht, um wieder auf `boden` zu fallen.
fn abklingen(l: &Leitung, a: &Address, boden: u64) -> Result<(), String> {
    let beginn = Instant::now();
    loop {
        let preis = l.stand(a)?.grundpreis_je_byte;
        if preis <= boden {
            println!("[last] grundpreis wieder am boden ({boden}) nach {} s", beginn.elapsed().as_secs());
            return Ok(());
        }
        if beginn.elapsed() > Duration::from_secs(300) {
            println!("[last] grundpreis nach 300 s noch bei {preis} (boden {boden})");
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn lauf(a: &[String]) -> Result<(), String> {
    let [relais, ausweis, zahl, raten, sekunden] = a else {
        return Err("lastgeber <relais> <ausweis> <absender> <raten,...> <sekunden je Stufe>".into());
    };
    let l = Leitung {
        adresse: relais.parse().map_err(|_| format!("keine Adresse: {relais}"))?,
        ausweis: myl_types::relais::ausweis_lesen(std::path::Path::new(ausweis)).map_err(|e| e.to_string())?,
    };
    let zahl: u64 = zahl.parse().map_err(|_| "Absender als Zahl".to_string())?;
    let raten: Vec<u64> = raten.split(',').map(|r| r.trim().parse().map_err(|_| format!("Rate: {r}"))).collect::<Result<_, _>>()?;
    let dauer = Duration::from_secs(sekunden.parse().map_err(|_| "Sekunden als Zahl".to_string())?);
    let mut absender = Vec::new();
    for i in 0..zahl.max(1) {
        let sk = schluessel(&format!("myelith-lastgeber-{i}"))?;
        let a = adresse(&sk)?;
        absender.push((sk, a));
    }
    let boden = l.stand(&absender[0].1)?.grundpreis_je_byte;
    for rate in raten {
        // Vor jeder Stufe nachfüllen, nach dem Bedarf dieser Stufe: Burns je
        // Absender mal ihre Kosten am Boden, mit einem Viertel Reserve
        // (Fund 671).
        let je_absender = rate * dauer.as_secs() / absender.len() as u64;
        ausstatten(&l, &absender, AUSSTATTUNG.max(je_absender * kosten_je_burn(boden) * 5 / 4))?;
        stufe(&l, &absender, rate, dauer)?;
    }
    abklingen(&l, &absender[0].1, boden)
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match lauf(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[last] Fehler: {e}");
            ExitCode::FAILURE
        }
    }
}
