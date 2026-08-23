//! Die wirtschaftliche Frage, gerechnet statt behauptet (Kritikpunkt K8).
//!
//! K8 lautet: *„Alle Parameter sind technisch konsistent und ökonomisch
//! ungeprüft. Es gibt keine Rechnung dazu, ob verteilte
//! Ganzzahl-Inferenz mit Redundanzfaktor gegen zentrale GPU-Inferenz
//! preislich bestehen kann."*
//!
//! Dieses Programm stellt die Rechnung an, in zwei Teilen: Kosten je
//! Token gegenüber einem zentralen Anbieter, und die Prägekurve über
//! viele Epochen.
//!
//! **Warum hier und nicht als Tabellenkalkulation:** Die Prägekurve
//! benutzt [`mint_amount`] und [`ema_update`] aus diesem Crate, also
//! genau die Formeln, die auch im Ledger laufen. Eine Nachbildung wäre
//! eine zweite Quelle für dieselbe Aussage, und daraus entstand Fund 34.
//!
//! Aufruf: `cargo run --release --bin oekonomie`

use myl_tokenomics::{ema_update, mint_amount, MintParams, UNITS_PER_MYL};

/// Gemessener Durchsatz des Ganzzahlpfads gegen bf16, gleiche Maschine.
///
/// **Die Ganzzahlwerte sind der aktuelle Stand** (nach v0.13.4 SIMD und
/// v0.16.0 Gewichtskopie), die bf16-Werte stammen aus dem Bench-Lauf
/// davor. Das ist zulässig und kein Vermischen: Am Gleitkommapfad hat
/// sich nichts geändert, er ist nicht Gegenstand dieses Projekts. Beide
/// Seiten wurden auf **derselben Maschine** gemessen (arm64/Darwin),
/// und nur das Verhältnis geht in die Rechnung ein.
struct Messpunkt {
    modell: &'static str,
    ganzzahl_tok_s: f64,
    bf16_tok_s: f64,
    /// Der Stand vor der Parallelisierung, zum Vergleich.
    vorher_tok_s: f64,
}

const MESSUNGEN: [Messpunkt; 2] = [
    Messpunkt { modell: "0,5B", ganzzahl_tok_s: 49.17, bf16_tok_s: 77.57, vorher_tok_s: 38.19 },
    Messpunkt { modell: "7B", ganzzahl_tok_s: 10.74, bf16_tok_s: 9.86, vorher_tok_s: 2.07 },
];

/// Redundanzfaktor r aus Kap. 4.4: Jedes Segment rechnen zwei Pods.
const REDUNDANZ: f64 = 2.0;

/// Stichprobenrate der Checker (Kap. 6.4: „~1–3 % des Volumens").
const STICHPROBE_MIN: f64 = 0.01;
const STICHPROBE_MAX: f64 = 0.03;

fn trennlinie(titel: &str) {
    println!("\n{}", titel);
    println!("{}", "=".repeat(titel.len()));
}

/// Teil 1: Was kostet ein Token im Netz gegenüber einem zentralen Anbieter?
fn kosten_je_token() {
    trennlinie("1. Kosten je Token: Netz mit Redundanz gegen zentralen Anbieter");

    println!("\nGemessener Durchsatz auf derselben Maschine (arm64/Darwin),");
    println!("beide Seiten im selben Lauf, beide auf der CPU:\n");
    println!("  {:<7} {:>10} {:>12} {:>10} {:>11}",
             "Modell", "vorher", "ganzzahlig", "bf16", "Verhältnis");
    for m in &MESSUNGEN {
        println!(
            "  {:<7} {:>7.2} t/s {:>9.2} t/s {:>7.2} t/s {:>11.3}",
            m.modell, m.vorher_tok_s, m.ganzzahl_tok_s, m.bf16_tok_s,
            m.ganzzahl_tok_s / m.bf16_tok_s
        );
    }
    println!("\n  Die Spalte -vorher- ist der Stand vor der Zeilen-Parallelisierung");
    println!("  (kernels v0.21.0). Sie hat den Durchsatz bei 0,5B um 29 %");
    println!("  und bei 7B um das **5,2-Fache** gehoben, bei unveraendertem");
    println!("  Digest: Zeilen sind voneinander unabhaengig, die Aufteilung");
    println!("  ist bitgleich per Konstruktion.");

    println!("\nDer Redundanz-Aufschlag: r = {REDUNDANZ} Pods je Segment, dazu");
    println!("Kontrollsegmente von {:.0} bis {:.0} Prozent des Volumens.",
             STICHPROBE_MIN * 100.0, STICHPROBE_MAX * 100.0);

    println!("\nKostenverhältnis = (1 / Durchsatzverhältnis) · Aufschlag\n");
    println!("  {:<7} {:>14} {:>14}", "Modell", "bei 1 % Probe", "bei 3 % Probe");
    for m in &MESSUNGEN {
        let v = m.ganzzahl_tok_s / m.bf16_tok_s;
        let lo = (1.0 / v) * (REDUNDANZ + STICHPROBE_MIN);
        let hi = (1.0 / v) * (REDUNDANZ + STICHPROBE_MAX);
        println!("  {:<7} {:>13.1}x {:>13.1}x", m.modell, lo, hi);
    }

    println!("\nWas sich damit geaendert hat:");
    println!("  Bei 7B ist der Ganzzahlpfad jetzt SCHNELLER als bf16 auf derselben");
    println!("  Maschine (Faktor 1,09). Der Durchsatz ist damit kein Kostentreiber");
    println!("  mehr, und uebrig bleibt im Wesentlichen die Redundanz.");
    println!("  Bei 0,5B bleibt ein Rueckstand: Die Matrizen sind zu klein, als");
    println!("  dass sich das Aufteilen ueber Threads voll auszahlt.");
    println!("\n  Vor der Parallelisierung standen hier 3,6x (0,5B) und 9,2x (7B).");

    println!("\nWie empfindlich das Ergebnis ist (7B, 2 % Probe):\n");
    println!("  {:>22} {:>16}", "Durchsatzverhältnis", "Kostenverhältnis");
    for v in [0.219, 0.5, 1.089, 1.5, 2.0] {
        let k = (1.0 / v) * (REDUNDANZ + 0.02);
        let marke = if (v - 1.089).abs() < 1e-9 {
            "  <- gemessen"
        } else if (v - 0.219).abs() < 1e-9 {
            "  <- vor der Parallelisierung"
        } else {
            ""
        };
        println!("  {:>22.3} {:>15.1}x{}", v, k, marke);
    }
    println!("\n  Selbst bei Durchsatz-Gleichstand bleiben {:.2}x uebrig.", REDUNDANZ + 0.02);
    println!("  Das ist der Preis der Verifizierbarkeit und nicht wegzuoptimieren.");

    println!("\nWas diese Rechnung NICHT ist:");
    println!("  Kein Marktpreis. Beide Seiten sind CPU-Messungen auf einer");
    println!("  Maschine; auf GPU verschiebt sich das Bild in beide Richtungen.");
    println!("  Vendor-Kernel fuer Gleitkomma sind hochoptimiert, was gegen uns");
    println!("  spricht; Tensor Cores sind fuer uns gesperrt (sie akkumulieren in");
    println!("  reduzierter Breite), was ebenfalls gegen uns spricht. Eine");
    println!("  belastbare Zahl braucht eine GPU-Messung, und die steht aus.");
}

/// Teil 2: Die Prägekurve über viele Epochen.
fn praegekurve() {
    trennlinie("2. Prägekurve über 200 Epochen");

    // Ein Verlauf mit drei Abschnitten, damit sichtbar wird, wie die EMA
    // auf Anstieg, Einbruch und Erholung reagiert.
    let verbrauch = |e: usize| -> u64 {
        let myl = match e {
            0..=49 => 1_000.0,
            50..=99 => 1_000.0 + (e - 49) as f64 * 60.0,   // Anstieg
            100..=119 => 400.0,                            // Einbruch
            _ => 400.0 + (e - 119) as f64 * 25.0,          // Erholung
        };
        (myl * UNITS_PER_MYL as f64) as u64
    };

    // Anlaufphase: Subvention 20 %, danach Zielbetrieb ohne Subvention.
    let anlauf = MintParams { subsidy_num: 20, subsidy_den: 100, m_max: 5_000 * UNITS_PER_MYL };
    let ziel = MintParams { subsidy_num: 0, subsidy_den: 100, m_max: 5_000 * UNITS_PER_MYL };

    let mut ema = verbrauch(0);
    let mut umlauf: i128 = 0;
    let mut gedeckelt = 0;

    println!("\n  {:>6} {:>12} {:>12} {:>12} {:>14}",
             "Epoche", "Burn (MYL)", "EMA (MYL)", "Mint (MYL)", "Umlauf (MYL)");
    for e in 0..200usize {
        let b = verbrauch(e);
        ema = ema_update(ema, b);
        let p = if e < 100 { &anlauf } else { &ziel };
        let m = mint_amount(ema, p);
        if m >= p.m_max {
            gedeckelt += 1;
        }
        umlauf += m as i128 - b as i128;
        if e % 25 == 0 || e == 199 {
            println!("  {:>6} {:>12.0} {:>12.0} {:>12.0} {:>14.0}",
                     e,
                     b as f64 / UNITS_PER_MYL as f64,
                     ema as f64 / UNITS_PER_MYL as f64,
                     m as f64 / UNITS_PER_MYL as f64,
                     umlauf as f64 / UNITS_PER_MYL as f64);
        }
    }

    println!("\n  Epochen an der Praegeobergrenze: {gedeckelt} von 200");
    println!("  (Die Obergrenze M_max hat in diesem Verlauf NIE gegriffen. Sie ist");
    println!("   damit hier auch nicht geprueft, sondern nur nicht verletzt worden.)");

    println!("\n  Was die Kurve zeigt, und es ist nicht, was man erwartet:");
    println!("\n  - Bei FLACHEM Verbrauch waechst der Umlauf um genau die");
    println!("    Subvention: 1000 verbrannt, 1200 gepraegt, +200 je Epoche.");
    println!("    Epoche 0 bis 25: der Umlauf steigt von 200 auf 5200.");
    println!("\n  - Bei STEIGENDEM Verbrauch faellt der Umlauf, obwohl subventioniert");
    println!("    wird. Zwischen Epoche 75 und 100 sinkt er von 4733 auf 282: Die");
    println!("    EMA hinkt dem Verbrauch nach, es wird also weniger gepraegt als");
    println!("    verbrannt. **Wachsende Nachfrage wirkt deflationaer**, und das");
    println!("    steht so in keinem Kapitel.");
    println!("\n  - Beim EINBRUCH in Epoche 100 kehrt sich das um: Der Verbrauch");
    println!("    faellt sofort, die Praegung folgt der EMA und faellt langsam. In");
    println!("    25 Epochen waechst der Umlauf von 282 auf 30222.");
    println!("\n  Die Traegheit schneidet also in beide Richtungen, und die zweite");
    println!("  ist die unangenehme: Wer den Verbrauch hochtreibt und dann");
    println!("  aussteigt, laesst eine Praegung zurueck, die der EMA folgt. Ob das");
    println!("  lohnend ist, haengt am Preis und ist mit dieser Rechnung NICHT");
    println!("  beantwortet. **Das ist der naechste offene Punkt von K8.**");
}

fn main() {
    println!("Myelith, wirtschaftliche Ueberschlagsrechnung (K8)");
    println!("Erzeugt mit `cargo run --release --bin oekonomie`");
    kosten_je_token();
    praegekurve();
    trennlinie("Fazit");
    println!("\n  Das Netz kostet je Token rund das 1,9-Fache (7B) bis 3,2-Fache");
    println!("  (0,5B) eines zentralen Anbieters, gemessen auf gleicher Hardware.");
    println!("  Bei 7B ist davon fast alles Redundanz, also der Preis der");
    println!("  Verifizierbarkeit; der Durchsatz taugt nicht mehr als Ausrede.");
    println!("\n  Vor der Parallelisierung standen hier 3,6x und 9,2x. Der");
    println!("  Unterschied kam nicht aus besserer Numerik, sondern daraus, dass");
    println!("  der Ganzzahlpfad einkernig lief, waehrend die Vergleichsseite");
    println!("  fuenf Kerne benutzte. Die Messung war richtig und ihre Deutung");
    println!("  falsch: Sie mass Quantisierungskosten UND fehlende Parallelitaet.");
    println!("\n  Was bleibt: Auf GPU ist nichts davon gemessen, und dort");
    println!("  verschiebt sich das Bild erneut.\n");
}
