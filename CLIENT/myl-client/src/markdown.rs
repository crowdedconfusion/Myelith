//! Markdown, zerlegt in einen Baum, den ein Fenster zeichnen kann.
//!
//! # 📌 Warum das hier steht und nicht im Fenster
//!
//! **Die Antwort eines Modells ist Text und keine Auszeichnung.** Wer
//! sie mit `innerHTML` in eine Seite schreibt, macht aus Daten
//! Steuerung: Ein Modell, das `<img src=x onerror=…>` erzeugt, bekommt
//! damit Code in der Seite ausgefuehrt, und diese Seite traegt die
//! Bruecke zu den Befehlen des Rueckens. **Ein eingeschleuster Text
//! koennte dann Einstellungen setzen oder Dateien schreiben.**
//!
//! ⚑ **Das ist Kapitel 8.3 eine Ebene hoeher.** Dort verhindert die
//! architektonische Trennung, dass abgerufene Inhalte den Kontrollfluss
//! beeinflussen; hier verhindert sie, dass eine Modellantwort zu Markup
//! wird. In beiden Faellen ist die Antwort nicht ein Filter, sondern
//! eine Bauart: **Was hier herauskommt, ist ein Baum aus Text**, und
//! das Fenster setzt ihn mit `createElement` und `textContent`
//! zusammen. Ein `<` bleibt dabei ein `<`, gleich was davor steht.
//!
//! ⚑ **Und es folgt derselben Regel wie alles andere in dieser Kiste:**
//! Die Kiste weiss, das Fenster zeichnet. Ein Zerleger im Skript waere
//! eigene Logik im Fenster, und die ist hier nicht vorgesehen.
//!
//! # Was zerlegt wird
//!
//! Der Ausschnitt, den ein Sprachmodell wirklich erzeugt:
//! Ueberschriften, Absaetze, Aufzaehlungen mit und ohne Nummern,
//! Codeblöcke und Code im Text, Fettes und Kursives, Zitate, Linien
//! und einfache Tabellen. **Nicht mehr**, denn was nicht zerlegt wird,
//! bleibt als Text stehen und ist damit lesbar statt falsch.

use serde::Serialize;

/// Ein Block, also eine Zeile oder ein Absatz fuer sich.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "art")]
pub enum Block {
    Absatz { inhalt: Vec<Teil> },
    Ueberschrift { stufe: u8, inhalt: Vec<Teil> },
    Liste { geordnet: bool, punkte: Vec<Vec<Teil>> },
    /// Ein Codeblock. ⚑ **Sein Inhalt wird nicht weiter zerlegt**, denn
    /// darin ist ein Stern ein Stern.
    Code { sprache: String, text: String },
    Zitat { inhalt: Vec<Teil> },
    Linie,
    Tabelle { kopf: Vec<Vec<Teil>>, zeilen: Vec<Vec<Vec<Teil>>> },
}

/// Ein Stueck innerhalb einer Zeile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "art")]
pub enum Teil {
    Text { text: String },
    Fett { text: String },
    Kursiv { text: String },
    Code { text: String },
    /// Ein Verweis.
    ///
    /// 📌 **Er wird als Text gezeigt und nicht als Knopf**, und das ist
    /// eine Entscheidung und kein Versaeumnis. Ein angeklickter Verweis
    /// in diesem Fenster fuehrte die Webansicht **aus der Anwendung
    /// heraus**; sie im System zu oeffnen braeuchte eine Erlaubnis, die
    /// die Erlaubnisliste bewusst nicht hat. Das Ziel steht deshalb
    /// sichtbar daneben: Wer hin will, sieht wohin.
    Verweis { text: String, ziel: String },
}

/// Zerlegt Markdown in Bloecke.
pub fn zerlegen(text: &str) -> Vec<Block> {
    let zeilen: Vec<&str> = text.lines().collect();
    let mut aus = Vec::new();
    let mut i = 0;

    while i < zeilen.len() {
        let z = zeilen[i];
        let ohne = z.trim_start();

        // Leerzeile: trennt und sonst nichts.
        if ohne.trim().is_empty() {
            i += 1;
            continue;
        }

        // ── Codeblock: alles bis zum schliessenden Zaun, unzerlegt ──
        if let Some(sprache) = ohne.strip_prefix("```") {
            let sprache = sprache.trim().to_string();
            let mut inhalt = Vec::new();
            i += 1;
            while i < zeilen.len() && !zeilen[i].trim_start().starts_with("```") {
                inhalt.push(zeilen[i]);
                i += 1;
            }
            // ⚑ Ein fehlender Schlusszaun beendet den Block am Textende
            // und ist kein Fehler: Ein abgebrochener Lauf soll lesbar
            // bleiben.
            if i < zeilen.len() {
                i += 1;
            }
            aus.push(Block::Code { sprache, text: inhalt.join("\n") });
            continue;
        }

        // ── Linie ───────────────────────────────────────────────────
        if ist_linie(ohne) {
            aus.push(Block::Linie);
            i += 1;
            continue;
        }

        // ── Ueberschrift ────────────────────────────────────────────
        if let Some((stufe, rest)) = ueberschrift(ohne) {
            aus.push(Block::Ueberschrift { stufe, inhalt: zeile_zerlegen(rest) });
            i += 1;
            continue;
        }

        // ── Tabelle: Kopfzeile, Trennzeile, dann Zeilen ─────────────
        if ohne.starts_with('|') && i + 1 < zeilen.len() && ist_trennzeile(zeilen[i + 1]) {
            let kopf = spalten(ohne);
            i += 2;
            let mut zeilen_aus = Vec::new();
            while i < zeilen.len() && zeilen[i].trim_start().starts_with('|') {
                zeilen_aus.push(spalten(zeilen[i].trim_start()));
                i += 1;
            }
            aus.push(Block::Tabelle { kopf, zeilen: zeilen_aus });
            continue;
        }

        // ── Zitat ───────────────────────────────────────────────────
        if let Some(rest) = ohne.strip_prefix("> ").or_else(|| ohne.strip_prefix(">")) {
            let mut text = vec![rest.trim().to_string()];
            i += 1;
            while i < zeilen.len() {
                let n = zeilen[i].trim_start();
                let Some(r) = n.strip_prefix("> ").or_else(|| n.strip_prefix(">")) else { break };
                text.push(r.trim().to_string());
                i += 1;
            }
            aus.push(Block::Zitat { inhalt: zeile_zerlegen(&text.join(" ")) });
            continue;
        }

        // ── Aufzaehlung ─────────────────────────────────────────────
        if let Some((geordnet, _)) = punkt(ohne) {
            let mut punkte = Vec::new();
            while i < zeilen.len() {
                let n = zeilen[i].trim_start();
                let Some((g, rest)) = punkt(n) else { break };
                if g != geordnet {
                    break;
                }
                punkte.push(zeile_zerlegen(rest));
                i += 1;
            }
            aus.push(Block::Liste { geordnet, punkte });
            continue;
        }

        // ── Absatz: bis zur naechsten Leerzeile oder zum naechsten
        //    Block, der fuer sich steht ──────────────────────────────
        let mut text = Vec::new();
        while i < zeilen.len() {
            let n = zeilen[i].trim_start();
            if n.trim().is_empty()
                || n.starts_with("```")
                || ist_linie(n)
                || ueberschrift(n).is_some()
                || punkt(n).is_some()
                || n.starts_with('>')
                || n.starts_with('|')
            {
                break;
            }
            text.push(n);
            i += 1;
        }
        // 📌 **Diese drei Zeilen verhindern eine Endlosschleife**, und
        // sie sind nicht theoretisch: `| kaputt |` faengt an wie eine
        // Tabelle, ist keine (es fehlt die Trennzeile), und der
        // Absatzzweig bricht dann an seiner eigenen ersten Zeile ab.
        // Ohne den Fortschritt hier stuende `i` still, und der Zerleger
        // liefe fuer immer. **Gefunden von `nichts_geht_verloren`, und
        // zwar als Haenger und nicht als Fehlschlag.**
        //
        // ⚑ Die Behebung ist zugleich die richtige Bedeutung: Eine
        // Zeile, die kein Block ist, ist Text.
        if text.is_empty() {
            text.push(zeilen[i].trim_start());
            i += 1;
        }
        aus.push(Block::Absatz { inhalt: zeile_zerlegen(&text.join(" ")) });
    }
    aus
}

/// `# ` bis `###### `, mit Leerzeichen dahinter.
fn ueberschrift(z: &str) -> Option<(u8, &str)> {
    let rauten = z.len() - z.trim_start_matches('#').len();
    if rauten == 0 || rauten > 6 {
        return None;
    }
    let rest = &z[rauten..];
    // ⚑ Ohne Leerzeichen ist es keine Ueberschrift, sondern eine
    // Raute im Text, etwa `#1`.
    let rest = rest.strip_prefix(' ')?;
    Some((rauten as u8, rest.trim()))
}

/// `---`, `***` oder `___`, mindestens drei.
fn ist_linie(z: &str) -> bool {
    let z = z.trim();
    ['-', '*', '_'].iter().any(|c| z.len() >= 3 && z.chars().all(|x| x == *c))
}

/// `- `, `* ` oder `1. `; gibt zurueck, ob geordnet, und den Rest.
fn punkt(z: &str) -> Option<(bool, &str)> {
    for m in ["- ", "* ", "+ "] {
        if let Some(r) = z.strip_prefix(m) {
            return Some((false, r.trim()));
        }
    }
    let ziffern = z.len() - z.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if ziffern == 0 {
        return None;
    }
    let rest = &z[ziffern..];
    for m in [". ", ") "] {
        if let Some(r) = rest.strip_prefix(m) {
            return Some((true, r.trim()));
        }
    }
    None
}

/// Die Trennzeile einer Tabelle: nur `|`, `-`, `:` und Leerzeichen.
fn ist_trennzeile(z: &str) -> bool {
    let z = z.trim();
    z.starts_with('|')
        && z.contains('-')
        && z.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Die Zellen einer Tabellenzeile.
fn spalten(z: &str) -> Vec<Vec<Teil>> {
    z.trim().trim_matches('|').split('|').map(|s| zeile_zerlegen(s.trim())).collect()
}

/// Zerlegt eine Zeile in ihre Stuecke.
///
/// ⚑ **Der Reihe nach und ohne Rueckgriff.** Ein Markdown im vollen
/// Umfang braucht einen Baum mit Verschachtelung; was ein Modell
/// erzeugt, ist flach. Was nicht erkannt wird, bleibt Text, und das ist
/// die richtige Richtung zu irren: Ein Sternchen zu viel im Text ist
/// lesbar, ein verschluckter Satz nicht.
pub fn zeile_zerlegen(z: &str) -> Vec<Teil> {
    let mut aus: Vec<Teil> = Vec::new();
    let zeichen: Vec<char> = z.chars().collect();
    let mut puffer = String::new();
    let mut i = 0;

    let schieben = |aus: &mut Vec<Teil>, puffer: &mut String| {
        if !puffer.is_empty() {
            aus.push(Teil::Text { text: std::mem::take(puffer) });
        }
    };

    while i < zeichen.len() {
        // Code im Text: alles bis zum naechsten Akzent, unzerlegt.
        if zeichen[i] == '`' {
            if let Some(e) = finden(&zeichen, i + 1, "`") {
                schieben(&mut aus, &mut puffer);
                aus.push(Teil::Code { text: zeichen[i + 1..e].iter().collect() });
                i = e + 1;
                continue;
            }
        }
        // Fett vor kursiv, sonst frisst der eine Stern den zweiten.
        if zeichen[i] == '*' && zeichen.get(i + 1) == Some(&'*') {
            if let Some(e) = finden(&zeichen, i + 2, "**") {
                schieben(&mut aus, &mut puffer);
                aus.push(Teil::Fett { text: zeichen[i + 2..e].iter().collect() });
                i = e + 2;
                continue;
            }
        }
        if zeichen[i] == '*' || zeichen[i] == '_' {
            let marke = zeichen[i].to_string();
            if let Some(e) = finden(&zeichen, i + 1, &marke) {
                // ⚑ Ein Unterstrich mitten im Wort ist keiner: `a_b_c`
                // ist ein Bezeichner und keine Auszeichnung.
                let leer_davor = i == 0 || zeichen[i - 1].is_whitespace();
                if leer_davor && e > i + 1 {
                    schieben(&mut aus, &mut puffer);
                    aus.push(Teil::Kursiv { text: zeichen[i + 1..e].iter().collect() });
                    i = e + 1;
                    continue;
                }
            }
        }
        // Verweis: `[Text](Ziel)`
        if zeichen[i] == '[' {
            if let Some(e) = finden(&zeichen, i + 1, "]") {
                if zeichen.get(e + 1) == Some(&'(') {
                    if let Some(z2) = finden(&zeichen, e + 2, ")") {
                        schieben(&mut aus, &mut puffer);
                        aus.push(Teil::Verweis {
                            text: zeichen[i + 1..e].iter().collect(),
                            ziel: zeichen[e + 2..z2].iter().collect(),
                        });
                        i = z2 + 1;
                        continue;
                    }
                }
            }
        }
        puffer.push(zeichen[i]);
        i += 1;
    }
    schieben(&mut aus, &mut puffer);
    aus
}

/// Sucht `marke` ab `von`; gibt den Anfang zurueck.
fn finden(zeichen: &[char], von: usize, marke: &str) -> Option<usize> {
    let m: Vec<char> = marke.chars().collect();
    (von..zeichen.len().saturating_sub(m.len() - 1))
        .find(|&i| zeichen[i..i + m.len()] == m[..])
}

#[cfg(test)]
mod proben {
    use super::*;

    fn text(t: &str) -> Teil {
        Teil::Text { text: t.into() }
    }

    /// ⚑ **Die tragende Zusage: Es entsteht nie Auszeichnung, immer
    /// Text.**
    ///
    /// 📌 Das ist der Grund, warum dieser Zerleger ueberhaupt hier steht
    /// und nicht im Fenster. Eine Modellantwort mit spitzen Klammern
    /// darf **nichts** ausloesen; sie ist Text und bleibt Text. Wer
    /// sie mit `innerHTML` einsetzte, gaebe einem eingeschleusten Satz
    /// Zugriff auf die Bruecke zum Ruecken.
    #[test]
    fn eine_marke_bleibt_text() {
        let boese = "<img src=x onerror=alert(1)> und <script>alert(2)</script>";
        let b = zerlegen(boese);
        let Block::Absatz { inhalt } = &b[0] else { panic!("kein Absatz: {b:?}") };
        // Alles landet im Textstueck, unveraendert und ungedeutet.
        let ganz: String = inhalt
            .iter()
            .map(|t| match t {
                Teil::Text { text } | Teil::Code { text } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(ganz.contains("<img src=x onerror=alert(1)>"), "{inhalt:?}");
        assert!(ganz.contains("<script>"), "{inhalt:?}");
        // ⚑ Und es entsteht **keine** Art, die Markup bedeuten koennte.
        // Die Aufzaehlung `Teil` hat gar keine solche Variante, und
        // genau das ist die Bauart: Was es nicht gibt, kann nicht
        // erzeugt werden.
    }

    #[test]
    fn ueberschriften_haben_stufen() {
        let b = zerlegen("# Eins\n\n### Drei");
        assert_eq!(b[0], Block::Ueberschrift { stufe: 1, inhalt: vec![text("Eins")] });
        assert_eq!(b[1], Block::Ueberschrift { stufe: 3, inhalt: vec![text("Drei")] });
    }

    /// 📌 **Eine Raute ohne Leerzeichen ist keine Ueberschrift.**
    /// `#1 Punkt` ist Text, und ein Modell schreibt so etwas.
    #[test]
    fn eine_raute_ohne_leerzeichen_ist_text() {
        let b = zerlegen("#1 Punkt");
        assert!(matches!(b[0], Block::Absatz { .. }), "{b:?}");
    }

    #[test]
    fn aufzaehlungen_mit_und_ohne_nummern() {
        let b = zerlegen("- eins\n- zwei");
        assert_eq!(
            b[0],
            Block::Liste { geordnet: false, punkte: vec![vec![text("eins")], vec![text("zwei")]] }
        );
        let b = zerlegen("1. eins\n2. zwei");
        let Block::Liste { geordnet, punkte } = &b[0] else { panic!("{b:?}") };
        assert!(*geordnet);
        assert_eq!(punkte.len(), 2);
    }

    /// ⚑ **Genau die Form, die der gemeldete Lauf erzeugt hat.**
    #[test]
    fn die_antwort_aus_dem_agentenlauf() {
        let roh = "Im aktuellen Verzeichnis liegen folgende Elemente:\n\n\
                   1. `liste.md` (5 Bytes) – Eine Markdown-Datei\n\
                   2. `notiz.txt` (5 Bytes) – Ein Textdatei\n\
                   3. `unter` (Verzeichnis) – Ein Unterverzeichnis";
        let b = zerlegen(roh);
        assert!(matches!(b[0], Block::Absatz { .. }), "{b:?}");
        let Block::Liste { geordnet, punkte } = &b[1] else { panic!("keine Liste: {b:?}") };
        assert!(*geordnet, "die Nummerierung ist verlorengegangen");
        assert_eq!(punkte.len(), 3);
        // ⚑ Und der Dateiname steht als Code da, nicht mit Akzenten.
        assert_eq!(punkte[0][0], Teil::Code { text: "liste.md".into() });
    }

    #[test]
    fn ein_codeblock_bleibt_unzerlegt() {
        let b = zerlegen("```rust\nlet a = *b;\n**nicht fett**\n```");
        assert_eq!(
            b[0],
            Block::Code { sprache: "rust".into(), text: "let a = *b;\n**nicht fett**".into() }
        );
    }

    /// 📌 **Ein Zaun ohne Schluss beendet am Textende.** Ein
    /// abgebrochener Lauf soll lesbar bleiben und nicht verschwinden.
    #[test]
    fn ein_offener_codeblock_verschluckt_nichts() {
        let b = zerlegen("```\nzeile eins\nzeile zwei");
        assert_eq!(b.len(), 1);
        let Block::Code { text, .. } = &b[0] else { panic!("{b:?}") };
        assert_eq!(text, "zeile eins\nzeile zwei");
    }

    #[test]
    fn fett_kursiv_und_code_im_text() {
        let t = zeile_zerlegen("Ein **fettes** und *schraeges* Wort mit `code`.");
        assert_eq!(t[1], Teil::Fett { text: "fettes".into() });
        assert_eq!(t[3], Teil::Kursiv { text: "schraeges".into() });
        assert_eq!(t[5], Teil::Code { text: "code".into() });
    }

    /// 📌 **Ein Unterstrich mitten im Wort ist keiner.** `max_tokens`
    /// und `snake_case` stehen in jeder zweiten Modellantwort.
    #[test]
    fn ein_bezeichner_wird_nicht_kursiv() {
        let t = zeile_zerlegen("Das Feld max_tokens_hier bleibt.");
        assert_eq!(t.len(), 1, "der Bezeichner wurde zerschnitten: {t:?}");
        assert!(matches!(&t[0], Teil::Text { text } if text.contains("max_tokens_hier")));
    }

    /// 📌 **Eine offene Marke ist Text.** Ein Modell, das mitten im
    /// Satz `**` schreibt und nicht schliesst, soll lesbar bleiben.
    #[test]
    fn eine_offene_marke_bleibt_stehen() {
        let t = zeile_zerlegen("Ein **offener Satz");
        assert_eq!(t, vec![text("Ein **offener Satz")]);
    }

    #[test]
    fn ein_verweis_traegt_sein_ziel() {
        let t = zeile_zerlegen("Siehe [die Seite](https://example.org/x) dort.");
        assert_eq!(
            t[1],
            Teil::Verweis { text: "die Seite".into(), ziel: "https://example.org/x".into() }
        );
    }

    #[test]
    fn zitat_und_linie() {
        let b = zerlegen("> Ein Zitat\n> geht weiter\n\n---");
        assert_eq!(b[0], Block::Zitat { inhalt: vec![text("Ein Zitat geht weiter")] });
        assert_eq!(b[1], Block::Linie);
    }

    #[test]
    fn eine_tabelle_bekommt_kopf_und_zeilen() {
        let b = zerlegen("| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |");
        let Block::Tabelle { kopf, zeilen } = &b[0] else { panic!("keine Tabelle: {b:?}") };
        assert_eq!(kopf.len(), 2);
        assert_eq!(zeilen.len(), 2);
        assert_eq!(zeilen[1][1], vec![text("4")]);
    }

    /// ⚑ **Was nicht erkannt wird, bleibt Text und geht nicht
    /// verloren.** Das ist die Zusage, die einen kleinen Zerleger
    /// vertretbar macht.
    #[test]
    fn nichts_geht_verloren() {
        let roh = "Ein Absatz.\n\n- Punkt\n\n~~durchgestrichen~~ kennt er nicht.\n\n| kaputt |";
        let b = zerlegen(roh);
        let alles: String = b
            .iter()
            .flat_map(|x| match x {
                Block::Absatz { inhalt } | Block::Zitat { inhalt } => inhalt.clone(),
                Block::Liste { punkte, .. } => punkte.concat(),
                _ => Vec::new(),
            })
            .map(|t| match t {
                Teil::Text { text } | Teil::Code { text } | Teil::Fett { text } => text,
                Teil::Kursiv { text } => text,
                Teil::Verweis { text, .. } => text,
            })
            .collect();
        for stueck in ["Ein Absatz.", "Punkt", "durchgestrichen", "kaputt"] {
            assert!(alles.contains(stueck), "`{stueck}` ist verschwunden: {alles}");
        }
    }

    /// ⚑ **Leerer Text gibt nichts, und das ist kein Fehler.**
    #[test]
    fn leeres_gibt_nichts() {
        assert!(zerlegen("").is_empty());
        assert!(zerlegen("\n\n  \n").is_empty());
    }
}
