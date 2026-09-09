//! Pruefungen der Oberflaeche, ohne sie zu oeffnen.
//!
//! # ⚑ Warum es sie gibt
//!
//! Diese Oberflaeche hat **keinen Buendler**, und das ist eine bewusste
//! Entscheidung: Sie ruft dieselben Unterbefehle wie das
//! Kommandozeilenwerkzeug und braucht dafuer keine Node-Werkzeugkette.
//!
//! ⛑ **Der Preis ist, dass niemand einen Tippfehler findet.** Eine
//! Klasse, die im HTML steht und in keiner Regel, faellt nicht auf: Das
//! Element ist da, es sieht nur falsch aus. Eine Kennung, die das
//! Skript sucht und die es nicht gibt, ergibt `null` und einen Fehler
//! erst beim Klicken.
//!
//! **Diese Datei ersetzt den Buendler an genau der Stelle, an der er
//! etwas geleistet haette**, und nirgends sonst.

use std::collections::BTreeSet;

fn lies(name: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Alle `id="..."` aus dem HTML.
fn kennungen(html: &str) -> BTreeSet<String> {
    werte(html, "id=\"")
}

/// Alle `class="..."` aus dem HTML, in einzelne Klassen zerlegt.
fn klassen(html: &str) -> BTreeSet<String> {
    werte(html, "class=\"")
        .iter()
        .flat_map(|z| z.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

fn werte(html: &str, marke: &str) -> BTreeSet<String> {
    let mut aus = BTreeSet::new();
    let mut rest = html;
    while let Some(a) = rest.find(marke) {
        let nach = &rest[a + marke.len()..];
        let Some(e) = nach.find('"') else { break };
        aus.insert(nach[..e].to_string());
        rest = &nach[e..];
    }
    aus
}

/// ⚑ **Jede Kennung, die das Skript sucht, muss es geben.**
/// `getElementById` gibt sonst `null`, und der Fehler faellt erst beim
/// Klicken auf, wenn ueberhaupt.
#[test]
fn jede_gesuchte_kennung_steht_im_html() {
    let html = lies("index.html");
    let js = lies("app.js");
    let da = kennungen(&html);
    // ⛑ **Zwei Schreibweisen, und die zweite hat diese Pruefung einmal
    // blind gemacht.** Am 2026-09-09 fuehrte `app.js` die Kurzform
    // `$("...")` ein, und die Pruefung fand daraufhin **null** Kennungen
    // und haette alles durchgelassen. Gemerkt hat es nur die Zusicherung
    // eine Zeile weiter unten, dass ueberhaupt etwas gefunden wird.
    // **Genau dafuer steht sie da**, und deshalb steht sie in jeder
    // Pruefung dieser Datei, die aus einer Datei etwas herausliest.
    let mut gesucht = BTreeSet::new();
    for marke in ["getElementById(\"", "$(\""] {
        let mut rest = js.as_str();
        while let Some(a) = rest.find(marke) {
            let nach = &rest[a + marke.len()..];
            let Some(e) = nach.find('"') else { break };
            gesucht.insert(nach[..e].to_string());
            rest = &nach[e..];
        }
    }
    assert!(
        gesucht.len() >= 8,
        "nur {} Kennungen gefunden; sucht die Pruefung noch richtig?",
        gesucht.len()
    );
    for k in &gesucht {
        assert!(da.contains(k), "app.js sucht #{k}, das HTML hat es nicht");
    }
}

/// ⚑ **Jede Klasse im HTML muss eine Regel haben.** Eine ohne ist ein
/// Element, das anders aussieht, als jemand gedacht hat.
#[test]
fn jede_klasse_im_html_hat_eine_regel() {
    let html = lies("index.html");
    let css = lies("stil.css");
    for k in klassen(&html) {
        assert!(
            css.contains(&format!(".{k}")),
            "die Klasse `{k}` steht im HTML, aber in keiner Regel"
        );
    }
}

/// Und die Klassen, die das Skript vergibt, ebenso.
#[test]
fn jede_klasse_aus_dem_skript_hat_eine_regel() {
    let js = lies("app.js");
    let css = lies("stil.css");
    // `className = "schritt ..."` und die Vorlagen darin.
    for k in ["schritt", "aufruf", "ergebnis", "unlesbar", "antwort", "hinweis",
              "marke", "eng", "offen", "grenze", "weg"] {
        assert!(
            js.contains(k) || css.contains(k),
            "`{k}` wird weder vergeben noch gestaltet"
        );
        assert!(css.contains(&format!(".{k}")), "`{k}` wird vergeben, hat aber keine Regel");
    }
}

/// ⛑ **Keine Fremdquelle.** Die Sicherheitsregel erlaubt nur `self`;
/// eine Oberflaeche, die Schriften oder Skripte aus dem Netz nachlaedt,
/// hat eine Verbindung, die niemand angemeldet hat, und sie faellt
/// nicht auf, weil sie einfach nicht laedt.
#[test]
fn nichts_wird_aus_dem_netz_geladen() {
    for datei in ["index.html", "stil.css", "app.js", "netz.js"] {
        let inhalt = lies(datei);
        for marke in ["http://", "https://", "//fonts.", "cdn."] {
            assert!(
                !inhalt.contains(marke),
                "{datei} nennt `{marke}`, das waere eine Verbindung nach draussen"
            );
        }
    }
}

/// ⚑ **Wer Bewegung abbestellt hat, bekommt keine.**
#[test]
fn bewegung_laesst_sich_abbestellen() {
    let css = lies("stil.css");
    assert!(
        css.contains("prefers-reduced-motion"),
        "die Gestaltung fragt nicht nach `prefers-reduced-motion`"
    );
}

/// ⛑ Und der Vorhang muss auch wieder weggehen koennen: Ein
/// Vorschaltbild ohne Abgang ist ein Fenster, das nie aufmacht.
#[test]
fn der_vorhang_geht_wieder_weg() {
    let js = lies("app.js");
    let css = lies("stil.css");
    assert!(js.contains("classList.add(\"weg\")"), "niemand nimmt den Vorhang weg");
    assert!(css.contains("#vorhang.weg"), "es gibt keine Regel fuer den weggenommenen Vorhang");
    assert!(js.contains("netzAnhalten()"), "die Animation wird nie angehalten");
}

/// ⛑ **Graustufen, und zwar nachpruefbar.** Am 2026-09-09 hat der
/// Projektinhaber die Gestaltung auf mattes Schwarz und Graustufen
/// festgelegt. Ein einzelner bunter Wert faellt niemandem auf, der die
/// Datei liest, aber jedem, der die Oberflaeche ansieht. Diese Pruefung
/// nimmt jeden Sechsstellerwert aus dem Stil und verlangt, dass Rot,
/// Gruen und Blau dicht beieinanderliegen.
///
/// ⚑ **Die Toleranz ist nicht null, und sie ist nicht geraten.** Ein
/// exakt neutrales Grau wirkt auf mattem Schwarz steril; die Toene hier
/// tragen einen Hauch ins Kuehle, hoechstens drei Stufen je Kanal, also
/// eine Spanne von hoechstens acht. Der erste Entwurf setzte sechs, und
/// prompt fiel `#8e9195` mit sieben durch: Die Zahl war geraten und
/// nicht an der Palette gemessen.
///
/// ⛑ **Damit die Toleranz nicht bedeutungslos wird, prueft sie sich
/// selbst mit.** Das alte Stahlblau `#7d9ab8` hat eine Spanne von 59,
/// also mehr als das Siebenfache; die Pruefung verlangt ausdruecklich,
/// dass es durchfiele. Ohne diesen Teil koennte jemand die Toleranz
/// hochsetzen, bis alles besteht, und niemand saehe es.
#[test]
fn die_farben_sind_graustufen() {
    const TOLERANZ: i32 = 8;

    let spanne = |n: u32| {
        let (r, g, b) = ((n >> 16) as i32, ((n >> 8) & 0xff) as i32, (n & 0xff) as i32);
        (r.max(g).max(b) - r.min(g).min(b), r, g, b)
    };
    // Der frueher benutzte Akzent, als Massstab dafuer, was die
    // Toleranz noch fangen MUSS.
    let (alt, ..) = spanne(0x7d9ab8);
    assert!(
        alt > TOLERANZ * 4,
        "die Toleranz {TOLERANZ} liesse das alte Stahlblau (Spanne {alt}) fast durch"
    );

    let stil = lies("stil.css");
    let mut geprueft = 0;
    let zeichen: Vec<char> = stil.chars().collect();
    for (i, c) in zeichen.iter().enumerate() {
        if *c != '#' || i + 6 >= zeichen.len() {
            continue;
        }
        let hex: String = zeichen[i + 1..i + 7].iter().collect();
        if hex.len() != 6 || !hex.chars().all(|z| z.is_ascii_hexdigit()) {
            continue;
        }
        let n = u32::from_str_radix(&hex, 16).expect("sechs Hexstellen");
        let (s, r, g, b) = spanne(n);
        assert!(
            s <= TOLERANZ,
            "#{hex} ist kein Grau: R{r} G{g} B{b}, Spanne {s} ueber {TOLERANZ}"
        );
        geprueft += 1;
    }
    assert!(geprueft >= 6, "nur {geprueft} Farbwerte gefunden; sucht die Pruefung noch richtig?");
}

/// ⛑ **Glas braucht einen Rueckfall, und der muss geprueft sein.**
/// `backdrop-filter` traegt auf macOS; auf WebKitGTK ist es je nach
/// Fassung da, und unter NixOS mit `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
/// das dieses Projekt ausdruecklich empfiehlt, faellt es weg, weil
/// genau die Komposition fehlt, die es braucht.
///
/// Ohne Rueckfall waeren die Flaechen dort **fast durchsichtig**: Text
/// auf Text, und das sieht nach kaputt aus. Mit Rueckfall sind sie
/// matte Platten, und das sieht nach Entscheidung aus.
#[test]
fn glas_hat_einen_rueckfall() {
    let stil = ohne_kommentare(&lies("stil.css"));
    assert!(
        stil.contains("-webkit-backdrop-filter"),
        "ohne das Praefix traegt es auf aelteren WebKit-Fassungen nicht"
    );

    // ⛑ **Diese Pruefung zaehlte einmal `@supports`-Bloecke und wollte
    // mindestens zwei.** Das war das falsche Mass: Ein Block, der alle
    // Flaechen deckt, ist besser als zwei, die je eine decken, und die
    // Pruefung fiel, als genau das gebaut wurde. Sie prueft jetzt, was
    // sie meint, naemlich dass **jeder Selektor mit `backdrop-filter`
    // in einem Rueckfall vorkommt**.
    let (rueckfaelle, uebriges) = supports_trennen(&stil);
    assert!(!rueckfaelle.is_empty(), "es gibt gar keinen Rueckfall");

    // ⛑ **Wer `backdrop-filter: none` setzt, benutzt es nicht, sondern
    // schaltet es ab**, und braucht folglich keinen Rueckfall. Der
    // erste Entwurf hat das nicht unterschieden und `#auftrag`
    // angemahnt, das genau deshalb dasteht: Es liegt IN einem Glas und
    // soll keines sein.
    let braucht: BTreeSet<String> = selektoren_mit(&uebriges, "backdrop-filter")
        .difference(&selektoren_mit(&uebriges, "backdrop-filter: none"))
        .cloned()
        .collect();
    assert!(
        braucht.len() >= 4,
        "nur {} Flaechen mit `backdrop-filter` gefunden; sucht die Pruefung noch richtig?",
        braucht.len()
    );
    let gedeckt: BTreeSet<String> = rueckfaelle
        .iter()
        .flat_map(|b| selektoren_mit(b, "background"))
        .collect();

    for sel in &braucht {
        assert!(
            gedeckt.contains(sel),
            "`{sel}` benutzt `backdrop-filter` und hat keinen Rueckfall.\n\
             Unter NixOS mit WEBKIT_DISABLE_COMPOSITING_MODE=1 waere die Flaeche \
             fast durchsichtig, also Text auf Text.\n\
             Gedeckt sind: {gedeckt:?}"
        );
    }
}

/// Nimmt `/* ... */` heraus, damit ein Kommentar, der `backdrop-filter`
/// erwaehnt, nicht als Regel zaehlt.
fn ohne_kommentare(css: &str) -> String {
    let mut aus = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(a) = rest.find("/*") {
        aus.push_str(&rest[..a]);
        match rest[a..].find("*/") {
            Some(e) => rest = &rest[a + e + 2..],
            None => return aus,
        }
    }
    aus.push_str(rest);
    aus
}

/// Trennt die Rueempfe der `@supports not`-Bloecke vom Rest.
fn supports_trennen(css: &str) -> (Vec<String>, String) {
    let mut bloecke = Vec::new();
    let mut uebrig = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(a) = rest.find("@supports not") {
        uebrig.push_str(&rest[..a]);
        let nach = &rest[a..];
        let Some(auf) = nach.find('{') else { break };
        // Klammern zaehlen, denn im Rumpf stehen weitere Regeln.
        let mut tiefe = 0usize;
        let mut ende = None;
        for (i, c) in nach[auf..].char_indices() {
            if c == '{' {
                tiefe += 1;
            } else if c == '}' {
                tiefe -= 1;
                if tiefe == 0 {
                    ende = Some(auf + i);
                    break;
                }
            }
        }
        let Some(ende) = ende else { break };
        bloecke.push(nach[auf + 1..ende].to_string());
        rest = &nach[ende + 1..];
    }
    uebrig.push_str(rest);
    (bloecke, uebrig)
}

/// Alle Selektoren, deren Regel `eigenschaft` setzt, einzeln.
fn selektoren_mit(css: &str, eigenschaft: &str) -> BTreeSet<String> {
    let mut aus = BTreeSet::new();
    let mut rest = css;
    while let Some(auf) = rest.find('{') {
        let Some(zu) = rest[auf..].find('}') else { break };
        let rumpf = &rest[auf + 1..auf + zu];
        if rumpf.contains(eigenschaft) {
            let kopf = rest[..auf].rsplit(['}', ';']).next().unwrap_or("");
            for sel in kopf.split(',') {
                let sel = sel.trim();
                if !sel.is_empty() && !sel.starts_with('@') {
                    aus.insert(sel.to_string());
                }
            }
        }
        rest = &rest[auf + zu + 1..];
    }
    aus
}
/// ⚑ **Die Startanimation laesst sich abbestellen, und der Vorhang
/// bleibt trotzdem.** Er traegt eine echte Wartezeit; wer Bewegung
/// abbestellt, soll sie sehen koennen, nur eben still.
#[test]
fn das_rauschen_gehoert_zur_abbestellbaren_bewegung() {
    let stil = lies("stil.css");
    let i = stil.find("prefers-reduced-motion").expect("die Regel gibt es");
    let block: String = stil[i..].chars().take(400).collect();
    for was in ["#netz", ".vorhangtext", "button::after"] {
        assert!(block.contains(was), "{was} bewegt sich weiter, obwohl abbestellt:\n{block}");
    }
}
