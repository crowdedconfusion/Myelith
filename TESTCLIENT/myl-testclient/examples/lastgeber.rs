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
//! kommt aus den Probekonten 6 bis 11, deren Schlüssel jeder kennt.

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
/// So viel bekommt jeder Absender, in Kleinstbeträgen: 0,05 MYL, rund
/// zweihundertfünfzig Burns am Boden der Probekette.
const AUSSTATTUNG: u64 = 50_000;
/// Die Probekonten, aus denen ausgestattet wird; 0 bis 5 bleiben dem
/// Probehahn der Knoten.
const QUELLEN: std::ops::Range<u8> = 6..12;
/// So viele Überweisungen je Quelle und Runde: unter der Grenze des
/// Mempools je Absender (64).
const JE_RUNDE: usize = 48;
/// So viele Fäden senden zugleich. 📌 Mit vier kamen höchstens rund 180
/// Sendungen je Sekunde hinaus (je eine Verbindung je Sendung); die Stufen
/// darüber hätten den Lastgeber gemessen statt des Knotens.
const FAEDEN: u64 = 16;

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

    fn senden(&self, tx: &Transaktion) -> bool {
        let Ok(bytes) = borsh::to_vec(tx) else {
            return false;
        };
        matches!(
            myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Transaktion { tx: bytes }, FRIST),
            Ok(Relaisantwort::Erledigt(_))
        )
    }
}

fn schluessel(text: &str) -> Result<BlsSecretKey, String> {
    BlsSecretKey::key_gen(Hash::sha256(text.as_bytes()).as_bytes()).map_err(|e| format!("{e:?}"))
}

fn adresse(sk: &BlsSecretKey) -> Result<Address, String> {
    Ok(Address::aus_schluessel(&sk.public_key().map_err(|e| format!("{e:?}"))?))
}

/// Stattet jeden Absender unter der halben Ausstattung aus, in Runden je
/// Quelle, bis alle gedeckt sind.
fn ausstatten(l: &Leitung, absender: &[(BlsSecretKey, Address)]) -> Result<(), String> {
    let beginn = Instant::now();
    loop {
        let mut offen = Vec::new();
        for (_, a) in absender {
            if l.stand(a)?.guthaben < AUSSTATTUNG / 2 {
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
        for q in QUELLEN {
            if rest.is_empty() {
                break;
            }
            let sk = myl_node::kette::probeschluessel(q);
            let stand = l.stand(&adresse(&sk)?)?;
            let bedarf = AUSSTATTUNG + stand.aufschlag_konto + 1_000;
            let zahl = rest.len().min(JE_RUNDE).min((stand.guthaben / bedarf) as usize);
            for (i, nach) in rest[..zahl].iter().enumerate() {
                let anweisung = Anweisung::Ueberweisung { nach: *nach, betrag: AUSSTATTUNG };
                let tx = Transaktion::signiere(&stand.netzwert, &sk, stand.nonce + i as u64, anweisung)
                    .map_err(|e| format!("{e:?}"))?;
                l.senden(&tx);
            }
            rest = &rest[zahl..];
        }
        std::thread::sleep(Duration::from_secs(10));
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
        faeden.push(std::thread::spawn(move || {
            let mut naechste: Vec<u64> = meine.iter().map(|(_, _, n)| *n).collect();
            let mut k = 0usize;
            let mut soll = Instant::now();
            while Instant::now() < ende && !meine.is_empty() {
                let i = k % meine.len();
                k += 1;
                let anweisung = Anweisung::Burn { betrag: 1 };
                if let Ok(tx) = Transaktion::signiere(&netzwert, &meine[i].0, naechste[i], anweisung) {
                    if l.senden(&tx) {
                        naechste[i] += 1;
                        gesendet.fetch_add(1, Ordering::Relaxed);
                    } else {
                        abgewiesen.fetch_add(1, Ordering::Relaxed);
                    }
                }
                soll += takt;
                if let Some(warten) = soll.checked_duration_since(Instant::now()) {
                    std::thread::sleep(warten);
                }
            }
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
    // Nachlauf: Was gesendet ist, darf noch in die nächsten Blöcke.
    std::thread::sleep(Duration::from_secs(5));
    let mut aufgenommen = 0u64;
    for ((_, a), n) in absender.iter().zip(&vorher) {
        aufgenommen += l.stand(a)?.nonce.saturating_sub(*n);
    }
    let sekunden = dauer.as_secs().max(1);
    println!(
        "[last] stufe={rate}/s gesendet={} abgewiesen={} aufgenommen={} je_sekunde={} grundpreis_max={}",
        gesendet.load(Ordering::Relaxed),
        abgewiesen.load(Ordering::Relaxed),
        aufgenommen,
        aufgenommen / sekunden,
        preis_max.load(Ordering::Relaxed)
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
    ausstatten(&l, &absender)?;
    for rate in raten {
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
