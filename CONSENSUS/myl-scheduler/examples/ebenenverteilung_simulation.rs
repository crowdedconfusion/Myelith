//! **Simulation der Ebenenverteilung** (Stufe 4 des Dispatchers, P9a,
//! 2026-10-21): Bringt die gemessene Ebenenzeit auf dem Draht genug, um das
//! Feld vor dem Audit festzuschreiben?
//!
//! Drei Schnitte je Pod, alle mit den **wahren** Zeiten bewertet:
//!
//! 1. der gleiche Schnitt von heute (`ebenen · s / k`, Kopf beim letzten),
//! 2. die Verteilung nur nach Kapazität (alle Zeiten gleich angesagt), also
//!    ohne `ebenenzeit_us` auf dem Draht,
//! 3. die volle Verteilung mit Zeit.
//!
//! ⚑ **Die Weiche** (Festlegung im Plan): Bringt 3 gegen 2 im Median unter
//! 10 % kürzere Engpasszeit, kommt nur `ebenen_hoechstens` auf den Draht.
//!
//! Annahmen aus Messungen am 2026-10-21: Netzmodell Raven 35B A3B int4,
//! 40 Ebenen zu 0,44 GB, Kopf Platz 4 und Zeit 1, Ebenenzeit auf dem Mac
//! 900 µs; Pods zu k = 4; Arbeitsspeicher der Knoten 8 bis 64 GiB, 4 GiB
//! Rücklage. Aufruf: `cargo run --release --example ebenenverteilung_simulation`.

use myl_scheduler::ebenenverteilung::{engpass_us, verteilen, Bereich, Mitglied};
use myl_types::ids::MinerId;

const EBENEN: u32 = 40;
const K: usize = 4;
const KOPF_PLATZ: u32 = 4;
const KOPF_ZEIT: u32 = 1;
const EBENE_MB: u64 = 440;
const GRUND_US: u64 = 900;
const PODS: usize = 5000;

struct Zufall(u64);
impl Zufall {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Wie viele Ebenen ein Knoten mit `ram_gib` hält (4 GiB Rücklage, ganze
/// Ebenen, höchstens alle samt Kopf).
fn kapazitaet(ram_gib: u64) -> u32 {
    let frei_mb = ram_gib.saturating_sub(4) * 1024;
    ((frei_mb / EBENE_MB) as u32).min(EBENEN + KOPF_PLATZ)
}

/// Der gleiche Schnitt von heute, oder `None`, wenn ein Bereich nicht passt.
fn gleich(ms: &[Mitglied]) -> Option<Vec<Bereich>> {
    let b: Vec<Bereich> = (0..ms.len())
        .map(|s| Bereich {
            kennung: ms[s].kennung,
            von: EBENEN * s as u32 / ms.len() as u32,
            bis: EBENEN * (s as u32 + 1) / ms.len() as u32,
            mit_kopf: s + 1 == ms.len(),
        })
        .collect();
    b.iter()
        .zip(ms)
        .all(|(x, m)| x.ebenen() + if x.mit_kopf { KOPF_PLATZ } else { 0 } <= m.kapazitaet)
        .then_some(b)
}

/// Eine Lage: Name und wie sie eine Ebenenzeit zieht.
type Lage = (&'static str, fn(&mut Zufall) -> u64);

fn median(mut w: Vec<u64>) -> u64 {
    w.sort_unstable();
    w.get(w.len().saturating_sub(1) / 2).copied().unwrap_or(0)
}

fn main() {
    let lagen: [Lage; 3] = [
        ("fast gleich (±10 %)", |z| GRUND_US * (90 + z.bis(21)) / 100),
        ("zwei Klassen (1× und 2×)", |z| if z.bis(2) == 0 { GRUND_US } else { 2 * GRUND_US }),
        ("weit gestreut (0,5× bis 4×)", |z| GRUND_US * [50, 71, 100, 141, 200, 283, 400][z.bis(7) as usize] / 100),
    ];
    let speicher = [8u64, 12, 16, 24, 32, 48, 64];
    println!("Simulation der Ebenenverteilung: {PODS} Pods je Lage, k = {K}, {EBENEN} Ebenen, Kopf Platz {KOPF_PLATZ} Zeit {KOPF_ZEIT}\n");
    println!("| Lage | gleicher Schnitt machbar | Verteilung machbar | Engpass nur Kapazität gegen gleich (Median) | **mit Zeit gegen nur Kapazität (Median)** | Anteil Pods mit ≥ 10 % durch die Zeit |");
    println!("|---|---|---|---|---|---|");
    for (n, (name, zeit)) in lagen.iter().enumerate() {
        let mut z = Zufall(0x5DEE_CE66_D1CE_4E5B ^ n as u64);
        let (mut gleich_ok, mut ok) = (0usize, 0usize);
        let (mut g_kap, mut g_zeit) = (Vec::new(), Vec::new());
        let mut zehn = 0usize;
        for p in 0..PODS {
            let ms: Vec<Mitglied> = (0..K)
                .map(|i| Mitglied {
                    kennung: MinerId::new([(p * K + i) as u8 ^ (p >> 8) as u8, i as u8, (p & 0xff) as u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
                    kapazitaet: kapazitaet(speicher[z.bis(speicher.len() as u64) as usize]),
                    ebenenzeit_us: zeit(&mut z),
                    gehalten: None,
                })
                .collect();
            let saat = [(p & 0xff) as u8; 32];
            let nur_kap: Vec<Mitglied> = ms.iter().map(|m| Mitglied { ebenenzeit_us: GRUND_US, ..*m }).collect();
            let (Ok(mit_zeit), Ok(kap)) = (verteilen(&ms, EBENEN, KOPF_PLATZ, KOPF_ZEIT, &saat), verteilen(&nur_kap, EBENEN, KOPF_PLATZ, KOPF_ZEIT, &saat)) else {
                continue;
            };
            ok += 1;
            let e_zeit = engpass_us(&mit_zeit, &ms, KOPF_ZEIT).unwrap();
            let e_kap = engpass_us(&kap, &ms, KOPF_ZEIT).unwrap();
            // Gewinn in Promille, gegen den Schnitt ohne Zeit.
            let gewinn = 1000 - (e_zeit * 1000 / e_kap.max(1)).min(1000);
            g_zeit.push(gewinn);
            if gewinn >= 100 {
                zehn += 1;
            }
            if let Some(gl) = gleich(&ms) {
                gleich_ok += 1;
                let e_gl = engpass_us(&gl, &ms, KOPF_ZEIT).unwrap();
                g_kap.push(1000 - (e_kap * 1000 / e_gl.max(1)).min(1000));
            }
        }
        let pz = |x: u64| format!("{},{} %", x / 10, x % 10);
        println!(
            "| {name} | {} % | {} % | {} kürzer | **{} kürzer** | {} % |",
            gleich_ok * 100 / PODS,
            ok * 100 / PODS,
            pz(median(g_kap)),
            pz(median(g_zeit)),
            zehn * 100 / ok.max(1)
        );
    }

    println!("Fertig");
}
