//! Die Speicherlast der Streitfrist, gerechnet statt geschätzt.
//!
//! # Die offene Frage
//!
//! Die Streitfrist steht seit dem 2026-08-13 auf **sieben Tagen**, und
//! die Begründung dafür ist gut: Sieben Stunden sind knapp, wenn ein
//! Checker nicht rund um die Uhr läuft, und ein Angreifer legte seine
//! Segmente in die Nacht des Zielmarktes. Was fehlte, war die andere
//! Seite der Abwägung: **Was kostet das an Speicher?** Solange die Zahl
//! niemand kennt, ist „sieben Tage sind es wert" keine Abwägung, sondern
//! eine Behauptung.
//!
//! Dasselbe gilt für den naheliegenden Gegenvorschlag, eine zweistufige
//! Frist (kurz für die erste Stufe, lang für die zweite). Ob der
//! Mehraufwand sich lohnt, lässt sich erst beurteilen, wenn die
//! einstufige Last dasteht.
//!
//! # Was archiviert wird, und warum genau das
//!
//! ⚑ **Seit E10 archiviert der Shard gar nichts mehr.** Bis dahin legte
//! er je (Segment, Layer) die Ausgabe-Aktivierungen ab, danach kurz nur
//! den Eingang. Beides ist fort: Die strittige Aktivierung bringt im
//! Streitfall der **Ankläger** mit. Was bleibt, ist die Spur, und die
//! ist der Arbeitsnachweis selbst.
//!
//! Ein Segment ist genau ein Vorwärtspass, also eine Token-Position
//! (Festlegung vom 2026-08-23).
//!
//! Daraus folgt die Größe je Segment unmittelbar:
//!
//! ```text
//! Rohbytes  = Layer · hidden_size · 2
//! Abgelegt  = Rohbytes · (k + m) / k
//! ```
//!
//! Weder Gewichte noch KV-Cache gehen ein: Die Gewichte hat jeder Shard
//! ohnehin, und der KV-Cache ist Betriebszustand und kein Beweismittel.
//!
//! # Woher die Zahlen stammen
//!
//! `k` und `m` aus `myl_types::erasure`, die Frist aus
//! `myl_consensus::DEFAULT_DISPUTE_EPOCHS`, beide **benutzt statt
//! wiederholt**: Als sich die Frist mit Fund 50 von 7 auf 168 Epochen
//! korrigierte, hätte eine getippte Zahl hier still weitergerechnet.
//!
//! Die Modellmaße stehen als Tabelle unten, mit ihrer Herkunft dabei;
//! `INTEGER_LLM/tests/audit/test_streitlast.py` hält sie gegen die
//! Artefakt-Konfigurationen, damit sie nicht auseinanderlaufen.
//!
//! Aufruf: `cargo run --release --bin streitlast`

use myl_consensus::DEFAULT_DISPUTE_EPOCHS;
use myl_types::erasure::{DEFAULT_K, DEFAULT_M};

/// Ein Modell, wie es das Projekt heute ausliefert.
///
/// `hidden` und `layer` stammen aus
/// `INTEGER_LLM/artifacts/<modell>/model_config.json`
/// (`hidden_size`, `num_layers`).
///
/// `tok_s` ist der **gemessene** Durchsatz des Ganzzahlpfads auf einer
/// Maschine (arm64/Darwin), dieselbe Quelle wie in
/// `myl_tokenomics::bin::oekonomie`. Wo keine Messung vorliegt, steht
/// `None`, und dann wird für dieses Modell keine Rate hochgerechnet:
/// Eine geschätzte Rate sähe wie eine gemessene aus.
struct Modell {
    name: &'static str,
    hidden: u64,
    layer: u64,
    tok_s: Option<f64>,
}

const MODELLE: [Modell; 4] = [
    Modell { name: "Qwen2.5-0,5B", hidden: 896, layer: 24, tok_s: Some(49.17) },
    Modell { name: "Qwen3-4B", hidden: 2560, layer: 36, tok_s: None },
    Modell { name: "Qwen2.5-7B", hidden: 3584, layer: 28, tok_s: Some(10.74) },
    Modell { name: "Qwen3-30B-A3B", hidden: 2048, layer: 48, tok_s: None },
];

/// Eine Epoche dauert eine Stunde (bestätigt 2026-08-24).
const SEKUNDEN_JE_EPOCHE: u64 = 3600;

/// Shards je Pod im ausgelieferten Manifest
/// (`INTEGER_LLM/configs/pipeline_4node.json`).
const SHARDS_JE_POD: u64 = 4;

/// Shards je Pod im ausgelieferten Manifest.
const LAYER_JE_SHARD_TEILER: u64 = 4;

/// Abgelegte Bytes je Segment, **wie es heute laeuft**: alle
/// Aktivierungen jeder Layer, erasure-codiert.
fn bytes_je_segment(m: &Modell) -> u64 {
    erasure(m.layer * m.hidden * 2)
}

/// Abgelegte Bytes je Segment und Shard, **seit E10**: gar keine
/// Aktivierung mehr, nur die **Spur**.
///
/// ⚑ Hier stand am 2026-08-29 zuerst „ein Hash je Layer", also Faktor
/// 224. **Das war falsch, und der Fehler steckte in einer Annahme, die
/// niemand nachgeprüft hatte:** Ein Shard könne aus den Token
/// nachrechnen. Er kann es nicht. Er hält `layer_start..layer_end` und
/// sonst nichts; die vorderen Layer liegen bei anderen. Was er **allein**
/// nachrechnen kann, beginnt bei seiner eingehenden Aktivierung, und die
/// muss deshalb bleiben.
///
/// E9 zog daraus den Schluss, die eingehende Aktivierung müsse bleiben:
/// 65 bis 260 GiB je Knoten. **Auch das war noch zu viel**, und die
/// Lösung lag nicht im Sparen, sondern in der Beweislast.
///
/// ⚑ **E10, 2026-08-30: Der Ankläger bringt den Wert mit.** Die
/// Bisektion endet an der **ersten** Abweichung, also sind sich beide
/// Seiten bei `j-1` einig, Bit für Bit; und der Ankläger hat den Wert
/// ohnehin, weil er das Segment gerade nachgerechnet hat. Der
/// Angeklagte bewahrt deshalb **keine Aktivierung** mehr auf.
///
/// Was bleibt, ist die **Spur**: 32 Byte je Layer. Sie ist die
/// Zusicherung, gegen die geurteilt wird, und sie ist kein zusätzlicher
/// Speicher, denn sie ist der Arbeitsnachweis, den es ohnehin gibt.
fn eingang_je_segment(m: &Modell) -> u64 {
    erasure(m.hidden * 2)
}

/// Was ein Shard seit E10 wirklich vorhält: die Spur seines Bereichs.
fn spur_je_segment(m: &Modell) -> u64 {
    erasure(layer_je_shard(m) * 32)
}

/// Wie viele Layer ein Shard rechnet, und damit der Faktor.
fn layer_je_shard(m: &Modell) -> u64 {
    m.layer / LAYER_JE_SHARD_TEILER
}

fn erasure(roh: u64) -> u64 {
    roh * (DEFAULT_K + DEFAULT_M) as u64 / DEFAULT_K as u64
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn main() {
    println!("Speicherlast der Streitfrist\n");
    println!(
        "Frist: {} Epochen à {} s = {} Tage",
        DEFAULT_DISPUTE_EPOCHS,
        SEKUNDEN_JE_EPOCHE,
        DEFAULT_DISPUTE_EPOCHS * SEKUNDEN_JE_EPOCHE / 86_400
    );
    println!(
        "Erasure: k={}, m={}, also Faktor {:.2}\n",
        DEFAULT_K,
        DEFAULT_M,
        (DEFAULT_K + DEFAULT_M) as f64 / DEFAULT_K as f64
    );

    // ── 1. Je Segment ──────────────────────────────────────────────
    println!("Je Segment (ein Vorwaertspass, eine Token-Position):\n");
    println!("  {:<16} {:>7} {:>7} {:>12} {:>12}", "Modell", "hidden", "Layer", "roh", "abgelegt");
    for m in &MODELLE {
        let roh = m.layer * m.hidden * 2;
        println!(
            "  {:<16} {:>7} {:>7} {:>9} KiB {:>9} KiB",
            m.name,
            m.hidden,
            m.layer,
            roh / 1024,
            bytes_je_segment(m) / 1024
        );
    }

    // ── 2. Je Pod über die ganze Frist ─────────────────────────────
    println!("\nJe Pod ueber die volle Frist, bei gemessenem Durchsatz:\n");
    println!(
        "  {:<16} {:>9} {:>14} {:>12} {:>12}",
        "Modell", "tok/s", "Segm./Epoche", "je Epoche", "168 Epochen"
    );
    for m in &MODELLE {
        let Some(tok_s) = m.tok_s else {
            println!("  {:<16} {:>9}   (nicht gemessen, keine Hochrechnung)", m.name, "-");
            continue;
        };
        let segmente = (tok_s * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let je_epoche = segmente * bytes_je_segment(m);
        let gesamt = je_epoche * DEFAULT_DISPUTE_EPOCHS;
        println!(
            "  {:<16} {:>9.2} {:>14} {:>9.1} GiB {:>8.1} TiB",
            m.name,
            tok_s,
            segmente,
            gib(je_epoche),
            gib(gesamt) / 1024.0
        );
    }

    // ── 3. Was ein einzelner Knoten davon traegt ───────────────────
    println!(
        "\nJe Knoten, bei {} Shards je Pod (jeder archiviert seine Layer):\n",
        SHARDS_JE_POD
    );
    for m in &MODELLE {
        let Some(tok_s) = m.tok_s else { continue };
        let segmente = (tok_s * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let gesamt = segmente * bytes_je_segment(m) * DEFAULT_DISPUTE_EPOCHS / SHARDS_JE_POD;
        println!("  {:<16} {:>8.1} TiB", m.name, gib(gesamt) / 1024.0);
    }

    // ── 3b. Die Annahme, die die Rechnung guenstig macht ───────────
    //
    // ⚑ Der gemessene Durchsatz stammt von **einem Knoten mit dem ganzen
    // Modell**. In einem Pod haelt jeder Shard nur ein Viertel der Layer,
    // die Stufen laufen ueberlappend, und der Durchsatz des Pods liegt im
    // besten Fall beim Vierfachen. Dann vervierfacht sich auch die Zahl
    // der Segmente, und die Ersparnis aus der Aufteilung ist wieder
    // aufgezehrt: Je Knoten steht dann dieselbe Last wie beim Pod ohne
    // Aufteilung.
    //
    // Diese Schranke gehoert dazu. Ohne sie liest sich die Tabelle
    // darueber wie eine Zusage, und sie ist eine Untergrenze.
    println!("\nJe Knoten, wenn der Pod das Vierfache schafft (obere Schranke):\n");
    for m in &MODELLE {
        let Some(tok_s) = m.tok_s else { continue };
        let segmente = (tok_s * SHARDS_JE_POD as f64 * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let gesamt = segmente * bytes_je_segment(m) * DEFAULT_DISPUTE_EPOCHS / SHARDS_JE_POD;
        println!("  {:<16} {:>8.1} TiB", m.name, gib(gesamt) / 1024.0);
    }

    // ── 4. Die Abwaegung, um die es geht ───────────────────────────
    println!("\nWas eine kuerzere Frist spart (linear, je Pod, Qwen2.5-7B):\n");
    let m = &MODELLE[2];
    let segmente = (m.tok_s.unwrap() * SEKUNDEN_JE_EPOCHE as f64) as u64;
    let je_epoche = segmente * bytes_je_segment(m);
    for (bezeichnung, epochen) in [
        ("7 Tage (heute)", DEFAULT_DISPUTE_EPOCHS),
        ("24 Stunden", 24),
        ("7 Stunden (Stand vor Fund 50)", 7),
        ("1 Stunde", 1),
    ] {
        println!(
            "  {:<32} {:>8.1} TiB",
            bezeichnung,
            gib(je_epoche * epochen) / 1024.0
        );
    }

    println!(
        "\nDie Last ist in der Frist linear: Eine zweistufige Frist spart\n\
         genau den Anteil, den die zweite Stufe kuerzer ist, und sie spart\n\
         ihn nur fuer die Segmente, die die erste Stufe ueberstehen.\n\
         Bei einer Pruefrate p bleibt der Anteil (1 - p) lange liegen,\n\
         also traegt die Verkuerzung erst bei hohem p.\n"
    );

    // ── 5. Der Hebel, den die Bitgenauigkeit hergibt ───────────────
    //
    // ⚑ Das Archiv haelt die **Aktivierungen** vor, damit ein
    // Angeklagter sie im Streitfall offenlegen kann. Sie sind aber
    // nicht die einzige Quelle: Bei bitgenauer Ganzzahl-Inferenz ist
    // jeder Vorwaertspass **exakt nachrechenbar**. Wer Eingabe-Token und
    // Seed hat, erzeugt dieselben Aktivierungen noch einmal, Bit fuer
    // Bit. Die Spur-Hashes stehen ohnehin schon im `Segment`.
    println!("\nDrei Stufen, je Shard und Segment:\n");
    println!(
        "  {:<16} {:>12} {:>12} {:>12} {:>20}",
        "Modell", "jede Layer", "nur Eingang", "nur Spur", "je Knoten (bis)"
    );
    for m in &MODELLE {
        let Some(tok_s) = m.tok_s else { continue };
        let jede = layer_je_shard(m) * eingang_je_segment(m);
        let segmente = (tok_s * SHARDS_JE_POD as f64 * SEKUNDEN_JE_EPOCHE as f64) as u64;
        println!(
            "  {:<16} {:>9} KiB {:>9} KiB {:>10} B {:>7.0} -> {:>5.1} GiB",
            m.name,
            jede / 1024,
            eingang_je_segment(m) / 1024,
            spur_je_segment(m),
            gib(segmente * jede * DEFAULT_DISPUTE_EPOCHS),
            gib(segmente * spur_je_segment(m) * DEFAULT_DISPUTE_EPOCHS)
        );
    }
    println!(
        "\n  ⚑ Der Sprung von der zweiten zur dritten Stufe kommt nicht\n\
           vom Sparen, sondern von der Beweislast: Die Bisektion endet an\n\
           der ERSTEN Abweichung, also sind sich beide Seiten bei j-1\n\
           einig, und der Anklaeger hat den strittigen Wert ohnehin. Er\n\
           bringt ihn mit; der Angeklagte wird gar nicht mehr gefragt.\n\
           Dieselbe Bauart tragen die optimistischen Rollups.\n\n\
           Und die Spur ist kein zusaetzlicher Speicher: Sie IST der\n\
           Arbeitsnachweis, den es ohnehin gibt.\n"
    );

    println!(
        "  Was es kostet: Im Streitfall rechnet der Angeklagte die Folge\n\
           neu, denn eine Position haengt ueber den KV-Cache an allen\n\
           vorherigen. Bei 2048 Token und {:.1} tok/s sind das rund\n\
           {:.0} Sekunden. Die Streitfrist betraegt 7 Tage, die\n\
           Schiedsrunde hat keine fest verdrahtete Antwortfrist.\n",
        MODELLE[2].tok_s.unwrap(),
        2048.0 / MODELLE[2].tok_s.unwrap()
    );

    println!(
        "  Der naechste Hebel, und sein Preis: Rechnet der Pod GEMEINSAM\n\
           nach, muss nur Shard 0 die Token halten, und das sind ueber\n\
           die ganze Frist rund 99 MiB statt zweistelliger Gigabyte.\n\
           Dafuer braucht es einen Wiederholungsweg zwischen den Shards,\n\
           und die Antwort haengt dann an den Nachbarn. Eigener Punkt.\n"
    );

    println!(
        "Befund: Ein Knoten traegt zwischen 0,4 und 1,8 TiB, je nachdem,\n\
         ob der Pod so schnell laeuft wie ein Einzelknoten oder viermal so\n\
         schnell. Das ist eine Platte, keine Anlage. Die 24-fache\n\
         Vorhaltedauer aus Fund 50 klingt nach viel und landet in einer\n\
         Groessenordnung, die ein Rechenzentrum ohnehin hat.\n\n\
         ⚑ Aber das ist der falsche Massstab. Dieses Netz will\n\
         niedrigschwellige Teilhabe, und wer 455 GiB je Knoten allein\n\
         fuer das Beweisarchiv verlangt, schliesst genau die aus, die\n\
         mit einer gewoehnlichen Maschine mitmachen wollen. Der Speicher\n\
         kommt zur Modellgroesse hinzu, nicht statt ihrer.\n\n\
         Die Frist zu kuerzen ist dabei der schwaechere Hebel: Sie wirkt\n\
         linear, und sieben Tage sind aus gutem Grund gewaehlt. Der\n\
         starke Hebel ist, nur den Eingang aufzubewahren und die\n\
         Ausgaben nachzurechnen (Faktor 7, umgesetzt am 2026-08-29), und\n\
         den gibt es nur, weil dieses Netz bitgenau rechnet. Eine\n\
         zweistufige Frist braeuchte mehr Mechanik und braechte weniger.\n\n\
         Was die Rechnung NICHT sagt: was die Bandbreite kostet, wenn ein\n\
         Angeklagter sein Archiv ausliefern muss, und was passiert, wenn\n\
         ein Knoten mehrere Pods gleichzeitig bedient. Beides ist eine\n\
         eigene Rechnung."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Rechnung selbst, an einem von Hand nachvollziehbaren Fall.
    ///
    /// 28 Layer · 3584 · 2 Byte = 200 704 roh, mal 12/8 = 301 056.
    /// Ausgeschrieben und nicht ueber `bytes_je_segment` gerechnet:
    /// Ein Test, der die gepruefte Funktion zum Pruefen benutzt, prueft
    /// sich selbst.
    #[test]
    fn bytes_je_segment_stimmt_mit_der_handrechnung() {
        let m = &MODELLE[2];
        assert_eq!(m.name, "Qwen2.5-7B");
        assert_eq!(bytes_je_segment(m), 28 * 3584 * 2 * 12 / 8);
        assert_eq!(bytes_je_segment(m), 301_056);
    }

    /// ⚑ Frist und Erasure-Faktor kommen aus den Konstanten der
    /// jeweiligen Crates, nicht aus getippten Zahlen. Als sich die Frist
    /// mit Fund 50 von 7 auf 168 Epochen korrigierte, haette eine
    /// getippte Zahl hier still weitergerechnet.
    #[test]
    fn die_frist_und_der_faktor_kommen_aus_den_konstanten() {
        assert_eq!(DEFAULT_DISPUTE_EPOCHS * SEKUNDEN_JE_EPOCHE / 86_400, 7);
        assert_eq!((DEFAULT_K + DEFAULT_M) * 2, DEFAULT_K * 3, "Faktor 1,5");
    }

    /// Der Befund in einer Zahl: Ein Knoten traegt unter zwei Terabyte.
    ///
    /// Schlaegt dieser Test fehl, hat sich eine der Eingangsgroessen
    /// geaendert, und dann gehoert die Abwaegung neu getroffen statt der
    /// Erwartung nachgezogen.
    #[test]
    fn ein_knoten_traegt_unter_zwei_terabyte() {
        let m = &MODELLE[2];
        let tok_s = m.tok_s.expect("fuer 7B liegt eine Messung vor");
        // Obere Schranke: Der Pod schafft das Vierfache eines
        // Einzelknotens, jeder Knoten haelt ein Viertel der Layer.
        let segmente = (tok_s * SHARDS_JE_POD as f64 * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let je_knoten = segmente * bytes_je_segment(m) * DEFAULT_DISPUTE_EPOCHS / SHARDS_JE_POD;
        let tib = gib(je_knoten) / 1024.0;
        assert!(tib < 2.0, "{tib:.2} TiB je Knoten, erwartet unter 2");
        assert!(tib > 1.0, "{tib:.2} TiB je Knoten, erwartet ueber 1");
    }

    /// ⚑ Der Hebel, um den es geht, und **er ist Faktor sieben, nicht
    /// 224**.
    ///
    /// Hier stand zuerst „ein Hash je Layer", also 224. Das war falsch:
    /// Es setzte voraus, dass ein Shard aus den Token nachrechnen kann.
    /// Er kann es nicht, ihm fehlen die vorderen Layer. Bleiben muss
    /// seine **eingehende** Aktivierung, und der Faktor ist damit die
    /// Zahl der Layer je Shard.
    ///
    /// 3584 · 2 = 7168 roh, mal 12/8 = 10 752. Ausgeschrieben, damit
    /// der Test nicht dieselbe Funktion benutzt, die er prueft.
    #[test]
    fn der_faktor_ist_die_layerzahl_je_shard() {
        let m = &MODELLE[2];
        assert_eq!(eingang_je_segment(m), 3584 * 2 * 12 / 8);
        assert_eq!(eingang_je_segment(m), 10_752);
        assert_eq!(layer_je_shard(m), 7);
        // Der ganze Shard-Anteil war siebenmal so viel wie sein Eingang.
        assert_eq!(bytes_je_segment(m) / SHARDS_JE_POD / eingang_je_segment(m), 7);
    }

    /// Und daraus folgt der Befund: aus dreistelligen Gigabyte werden
    /// zweistellige.
    #[test]
    fn mit_dem_eingang_traegt_ein_knoten_unter_dreihundert_gigabyte() {
        let m = &MODELLE[2];
        let tok_s = m.tok_s.expect("fuer 7B liegt eine Messung vor");
        let segmente = (tok_s * SHARDS_JE_POD as f64 * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let je_knoten = segmente * eingang_je_segment(m) * DEFAULT_DISPUTE_EPOCHS;
        let g = gib(je_knoten);
        assert!(g < 300.0, "{g:.0} GiB je Knoten, erwartet unter 300");
        assert!(g > 200.0, "{g:.0} GiB je Knoten, erwartet ueber 200");
    }

    /// ⚑ **Der Befund von E10 in einer Zahl: unter zehn Gigabyte.**
    ///
    /// 7 Layer je Shard, 32 Byte je Spur-Eintrag, mal 12/8. Beide Seiten
    /// ausgeschrieben, damit der Test nicht dieselbe Funktion benutzt,
    /// die er prueft.
    #[test]
    fn mit_der_spur_traegt_ein_knoten_unter_zehn_gigabyte() {
        let m = &MODELLE[2];
        assert_eq!(spur_je_segment(m), 7 * 32 * 12 / 8);
        assert_eq!(spur_je_segment(m), 336);
        // Faktor gegenueber der Stufe davor.
        assert_eq!(eingang_je_segment(m) / spur_je_segment(m), 32);

        let tok_s = m.tok_s.expect("fuer 7B liegt eine Messung vor");
        let segmente = (tok_s * SHARDS_JE_POD as f64 * SEKUNDEN_JE_EPOCHE as f64) as u64;
        let g = gib(segmente * spur_je_segment(m) * DEFAULT_DISPUTE_EPOCHS);
        assert!(g < 10.0, "{g:.1} GiB je Knoten, erwartet unter 10");
        assert!(g > 5.0, "{g:.1} GiB je Knoten, erwartet ueber 5");
    }

    /// Jedes Modell mit gemessenem Durchsatz muss auch Masse haben.
    #[test]
    fn jedes_modell_ist_vollstaendig() {
        for m in &MODELLE {
            assert!(m.hidden > 0 && m.layer > 0, "{}", m.name);
            if let Some(t) = m.tok_s {
                assert!(t > 0.0, "{}", m.name);
            }
        }
    }
}
