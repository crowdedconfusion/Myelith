//! Die Speicherlast eines Knotens, gerechnet statt geschätzt.
//!
//! # Die offene Frage
//!
//! Ob die Wissensdatenbank vervielfältigt oder erasure-kodiert wird, ist
//! in STORAGE ausdrücklich **nicht** entschieden, und die Begründung
//! lautet: Volle Kopien sind gut für sie, weil ein Agent
//! sie in kleinen Stücken liest und acht Abrufe je Textstelle die Latenz
//! genau dort vervielfachen, wo sie weh tut. **Aber:** „Umso mehr
//! Knoten, umso mehr Wissen" heißt, dass die Wissensdatenbank wächst,
//! und siebenfacher Platz auf einem wachsenden Korpus ist teuer.
//! **Latenz gegen Platz, und beide Zahlen fehlen.**
//!
//! Die Latenz braucht Verkehr und wartet auf Phase 4. **Der Platz nicht.**
//! Dieses Programm rechnet ihn.
//!
//! # ⚑ Was daran der eigentliche Punkt ist
//!
//! Nicht die Größe von heute, sondern die **Kopplung**. Der Satz „umso
//! mehr Knoten, umso mehr Wissen" liest sich wie eine Zusicherung, dass
//! die Last je Knoten gleich bleibt. Das täte sie nur, wenn die
//! Wissensdatenbank **mit der Knotenzahl** wüchse. Sie wächst aber mit
//! der Nutzung: Wissen legt hinein, wer welches hat, nicht wer beitritt.
//!
//! Zwei Größen, die nichts aneinander bindet:
//!
//! ```text
//! Last je Knoten = W · f / N
//!
//!   W = Umfang der Wissensdatenbank   (wächst mit der Nutzung)
//!   f = Redundanzfaktor               (Governance)
//!   N = Zahl der speichernden Knoten  (wächst mit dem Ertrag)
//! ```
//!
//! Wächst W ohne N, steigt die Last je Knoten unbegrenzt, und ab einer
//! Grenze gehen Knoten. Dann fällt N, und die Last der Verbliebenen
//! steigt weiter: **eine Rückkopplung in die falsche Richtung.**
//!
//! # Woraus die Kopplung entstehen muss
//!
//! Aus zwei Dingen, und beide fehlen heute:
//!
//! 1. **Ein Speicherentgelt** (Punkt 25). Mehr zu haltendes Wissen heißt
//!    mehr Entgelt heißt mehr Knoten. Das ist die Marktseite, sie wirkt
//!    verzögert und nur, wenn der Satz stimmt.
//! 2. **Eine Kapazitätsschranke.** Die Wissensdatenbank darf nur
//!    wachsen, soweit **nachgewiesene** Kapazität dafür da ist. Das ist
//!    die harte Seite, und sie setzt die Zusage voraus: Erst wenn ein
//!    Miner erklärt und beweist, was er hält, ist die Summe der
//!    Erklärungen das Speicherbudget des Netzes.
//!
//! **Der Schalter, mit dem ein Miner Hardware zu- und abschaltet, ist
//! damit nicht nur Bequemlichkeit.** Er ist die Eingabe, aus der sich
//! ergibt, wie groß die Wissensdatenbank überhaupt werden darf.
//!
//! # Was hier nicht gerechnet wird
//!
//! Latenz, Bandbreite und die Frage, wie oft ein Stück Wissen wirklich
//! abgerufen wird. Alles drei braucht Verkehr. Dieses Programm sagt, was
//! **Platz** kostet, und sonst nichts.

/// Artefaktgrößen in KiB, gemessen am 2026-08-30 auf der
/// Entwicklungsmaschine mit `du -sk`.
///
/// ⚑ **Gemessen, nicht gerechnet.** Die Größe eines Artefakts hängt an
/// Layoutentscheidungen des Exports, nicht nur an der Parameterzahl; eine
/// Formel daraus wäre eine Schätzung, die sich als Messung ausgibt.
/// Die Artefakte sind nicht versioniert, deshalb stehen die Zahlen hier
/// und nicht in einer Prüfung gegen die Dateien.
struct Modell {
    name: &'static str,
    kib: u64,
    layer: u64,
}

const MODELLE: [Modell; 4] = [
    Modell { name: "Qwen2.5-0,5B", kib: 757_960, layer: 24 },
    Modell { name: "Qwen3-4B", kib: 4_703_300, layer: 36 },
    Modell { name: "Qwen2.5-7B", kib: 8_512_912, layer: 28 },
    Modell { name: "Qwen3-30B-A3B", kib: 30_521_756, layer: 48 },
];

/// Shards je Pod im ausgelieferten Manifest
/// (`INTEGER_LLM/configs/pipeline_4node.json`).
const SHARDS_JE_POD: u64 = 4;

/// Die beiden Redundanzformen, zwischen denen D3 nicht entscheidet.
///
/// `Kopien { anzahl: 7 }` ist der in STORAGE genannte Vergleichswert,
/// `Erasure { k: 8, m: 6 }` ergibt den dort genannten Faktor 1,75.
struct Form {
    name: &'static str,
    /// Platz im Netz je Byte Nutzdaten, mal 100.
    faktor_x100: u64,
    /// Wie viele Gegenstellen ein vollständiger Abruf braucht.
    halter_je_abruf: u64,
}

const FORMEN: [Form; 2] = [
    Form { name: "Kopien 7", faktor_x100: 700, halter_je_abruf: 1 },
    Form { name: "Erasure k=8 m=6", faktor_x100: 175, halter_je_abruf: 8 },
];

/// Umfänge der Wissensdatenbank, für die gerechnet wird (in GiB).
const WISSEN_GIB: [u64; 4] = [1_024, 10_240, 102_400, 1_048_576];

/// Knotenzahlen, für die gerechnet wird.
const KNOTEN: [u64; 3] = [100, 1_000, 10_000];

/// Was ein Knoten für die Wissensdatenbank allein trägt, in MiB.
///
/// `W · f / N`, in Ganzzahlarithmetik und ohne Zwischenüberlauf: Der
/// Faktor steht als Hundertstel, deshalb wird vor der Division
/// multipliziert und am Ende durch 100 geteilt.
fn wissenslast_mib(wissen_gib: u64, faktor_x100: u64, knoten: u64) -> u64 {
    debug_assert!(knoten > 0, "ohne Knoten gibt es keine Last je Knoten");
    wissen_gib
        .saturating_mul(1024)
        .saturating_mul(faktor_x100)
        / (knoten * 100)
}

/// Wie groß die Wissensdatenbank sein darf, wenn ein Knoten höchstens
/// `budget_gib` dafür tragen soll.
fn tragbares_wissen_gib(budget_gib: u64, faktor_x100: u64, knoten: u64) -> u64 {
    budget_gib.saturating_mul(knoten).saturating_mul(100) / faktor_x100
}

/// Die Arbeitskopie eines Shards, in MiB.
///
/// Das ist **nicht** die Speicherpflicht, sondern das, was ein Miner
/// ohnehin lokal braucht, um zu rechnen: sein Layer-Bereich des Modells.
fn arbeitskopie_mib(m: &Modell) -> u64 {
    m.kib / SHARDS_JE_POD / 1024
}

/// MiB als GiB mit einer Nachkommastelle.
///
/// ⚑ **Ohne Nachkommastelle stand in der Tabelle `0 GiB`** für ein
/// Artefakt von 740 MiB und für jede Last unter einem Gibibyte. Eine
/// Null, die keine ist, ist in einer Tabelle über Speicherbedarf die
/// teuerste Art von Rundung.
fn gib(mib: u64) -> String {
    format!("{}.{} GiB", mib / 1024, (mib % 1024) * 10 / 1024)
}

/// GiB als lesbarer Umfang, TiB oder PiB.
fn umfang(gib: u64) -> String {
    let tib = gib / 1024;
    if tib >= 1024 {
        // Auch hier eine Nachkommastelle: `1 PiB` für 1 428 TiB
        // unterschlägt vierzig Prozent.
        format!("{}.{} PiB", tib / 1024, (tib % 1024) * 10 / 1024)
    } else {
        format!("{tib} TiB")
    }
}

fn main() {
    println!("Speicherlast eines Knotens\n");
    println!("Shards je Pod: {SHARDS_JE_POD}\n");

    // ── 1. Die Arbeitskopie ────────────────────────────────────────
    println!("Arbeitskopie je Miner (sein Shard, zum Rechnen):\n");
    println!("  {:<16} {:>7} {:>12} {:>14}", "Modell", "Layer", "Artefakt", "je Shard");
    for m in &MODELLE {
        println!(
            "  {:<16} {:>7} {:>12} {:>14}",
            m.name,
            m.layer,
            gib(m.kib / 1024),
            gib(arbeitskopie_mib(m))
        );
    }

    // ── 2. Die Wissensdatenbank, je Knoten ─────────────────────────
    println!("\nWissensdatenbank je speicherndem Knoten:\n");
    for f in &FORMEN {
        println!(
            "  {} (Faktor {:.2}, {} Halter je Abruf):",
            f.name,
            f.faktor_x100 as f64 / 100.0,
            f.halter_je_abruf
        );
        print!("    {:<12}", "W \\ N");
        for n in &KNOTEN {
            print!("{:>14}", n);
        }
        println!();
        for w in &WISSEN_GIB {
            print!("    {:<12}", umfang(*w));
            for n in &KNOTEN {
                let mib = wissenslast_mib(*w, f.faktor_x100, *n);
                print!("{:>14}", gib(mib));
            }
            println!();
        }
        println!();
    }

    // ── 3. Die Umkehrung ───────────────────────────────────────────
    println!("Tragbarer Umfang bei 1 TiB Budget je Knoten:\n");
    print!("  {:<18}", "Form \\ N");
    for n in &KNOTEN {
        print!("{:>14}", n);
    }
    println!();
    for f in &FORMEN {
        print!("  {:<18}", f.name);
        for n in &KNOTEN {
            print!("{:>14}", umfang(tragbares_wissen_gib(1024, f.faktor_x100, *n)));
        }
        println!();
    }

    println!(
        "\nDer Faktor ist der Hebel, nicht die Knotenzahl: {:.1}-fach zwischen den Formen.",
        FORMEN[0].faktor_x100 as f64 / FORMEN[1].faktor_x100 as f64
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Kernformel gegen eine Handrechnung.
    ///
    /// 10 TiB Wissen, siebenfach, 1 000 Knoten:
    /// 10 240 GiB · 7 / 1 000 = 71,68 GiB, also 73 400 MiB (abgerundet).
    #[test]
    fn die_wissenslast_stimmt_mit_der_handrechnung() {
        let mib = wissenslast_mib(10_240, 700, 1_000);
        assert_eq!(mib, 10_240 * 1024 * 7 / 1_000);
        assert_eq!(mib / 1024, 71);
    }

    /// ⚑ **Die Aussage, um die es geht.** Ohne wachsende Knotenzahl
    /// steigt die Last je Knoten linear mit dem Wissen. Zehnfaches
    /// Wissen bei gleicher Knotenzahl ist zehnfache Last.
    #[test]
    fn ohne_mehr_knoten_waechst_die_last_linear_mit_dem_wissen() {
        let klein = wissenslast_mib(1_024, 700, 1_000);
        let gross = wissenslast_mib(10_240, 700, 1_000);
        assert_eq!(gross, klein * 10);
    }

    /// Und die Gegenprobe dazu: Wächst die Knotenzahl mit, bleibt die
    /// Last gleich. Genau das behauptet „umso mehr Knoten, umso mehr
    /// Wissen", und genau das ist heute nicht zugesichert.
    #[test]
    fn mit_der_knotenzahl_mitwachsend_bleibt_die_last_gleich() {
        let a = wissenslast_mib(1_024, 700, 1_000);
        let b = wissenslast_mib(10_240, 700, 10_000);
        assert_eq!(a, b);
    }

    /// ⚑ **Der Faktor ist der Hebel.** Von sieben Kopien auf Erasure
    /// k=8/m=6 ist der Vierfache Umfang bei gleicher Last je Knoten.
    #[test]
    fn der_redundanzfaktor_ist_der_groessere_hebel() {
        let kopien = tragbares_wissen_gib(1_024, 700, 1_000);
        let erasure = tragbares_wissen_gib(1_024, 175, 1_000);
        assert_eq!(erasure / kopien, 4);
    }

    /// Umkehrung und Vorwärtsrechnung müssen zueinander passen.
    #[test]
    fn umkehrung_und_vorwaertsrechnung_passen_zusammen() {
        for f in &FORMEN {
            for n in &KNOTEN {
                let tragbar = tragbares_wissen_gib(1_024, f.faktor_x100, *n);
                let last_mib = wissenslast_mib(tragbar, f.faktor_x100, *n);
                // Abrundung erlaubt eine Abweichung von unter einem GiB.
                assert!(
                    last_mib <= 1_024 * 1024 && last_mib + 1024 >= 1_024 * 1024,
                    "{} bei {} Knoten: {} MiB statt 1 TiB",
                    f.name,
                    n,
                    last_mib
                );
            }
        }
    }

    /// Die Arbeitskopie des größten Modells passt in den Hauptspeicher
    /// einer Mietkiste mit 16 GB, sonst liest jede Ebene erneut von der
    /// Platte.
    #[test]
    fn die_arbeitskopie_des_groessten_modells_passt_in_sechzehn_gigabyte() {
        let groesstes = MODELLE.iter().max_by_key(|m| m.kib).expect("Modelle");
        assert_eq!(groesstes.name, "Qwen3-30B-A3B");
        let gib = arbeitskopie_mib(groesstes) / 1024;
        assert!(gib <= 8, "{} GiB je Shard", gib);
    }

    /// Jedes Modell trägt seine Zahlen, damit die Tabelle nicht still
    /// unvollständig wird.
    #[test]
    fn jedes_modell_ist_vollstaendig() {
        for m in &MODELLE {
            assert!(!m.name.is_empty());
            assert!(m.kib > 0, "{}", m.name);
            assert!(m.layer > 0, "{}", m.name);
        }
    }

    /// Die Formen sind so gewählt, dass sie die beiden Enden von D3
    /// abbilden: viele Halter mit wenig Platz gegen einen Halter mit
    /// viel Platz.
    #[test]
    fn die_formen_bilden_beide_enden_von_d3_ab() {
        assert_eq!(FORMEN[0].halter_je_abruf, 1);
        assert!(FORMEN[0].faktor_x100 > FORMEN[1].faktor_x100);
        assert!(FORMEN[1].halter_je_abruf > FORMEN[0].halter_je_abruf);
    }
}
