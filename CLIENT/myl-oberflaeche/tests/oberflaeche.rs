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

/// **Nichts im Skript heisst `t`, ausser der Uebersetzung.**
///
/// ⛑ **Fund 324, und er kostete den Projektinhaber einen Abend.** Zwei
/// Funktionen hielten ein Element in `const t`, und eine davon rief
/// oberhalb dieser Zeile `t("lauf.arbeitet")`. **Eine lokale Bindung
/// verdeckt den aeusseren Namen im ganzen Block, auch vor ihrer eigenen
/// Zeile**, und dort ist er noch nicht da: „Cannot access 't' before
/// initialization", und zwar genau dann, wenn ein Lauf laeuft.
///
/// ⚑ **Die Pruefung sucht die Verdeckung und nicht den Fehler.** Ein
/// Test, der den Aufruf prueft, faende die naechste Stelle nicht; ein
/// Name, den es nur einmal gibt, kann gar nicht erst verdeckt werden.
#[test]
fn nichts_im_skript_verdeckt_die_uebersetzung() {
    let js = lies("app.js");
    for (nr, zeile) in js.lines().enumerate() {
        let z = zeile.trim_start();
        // Die Uebersetzung selbst darf so heissen.
        if z.starts_with("const t = (") || z.starts_with("function t(") {
            continue;
        }
        for anfang in ["const t ", "let t ", "var t ", "const t=", "let t="] {
            assert!(
                !z.starts_with(anfang),
                "Zeile {}: `{}` verdeckt die Uebersetzung `t`",
                nr + 1,
                z
            );
        }
        assert!(
            !z.contains("(const t of") && !z.contains("(let t of"),
            "Zeile {}: `{}` verdeckt die Uebersetzung `t`",
            nr + 1,
            z
        );
    }
}

/// **Die Ueberschrift der Seite ist die groesste, und alles darunter
/// ist gleich gross.**
///
/// ⛑ **Vorher waren es drei Groessen, und die falsche war die
/// groesste:** `h2` hatte keine Angabe und nahm die Vorgabe des
/// Browsers, also mehr als das `h1`. **Eine Seite, deren
/// Unterueberschriften groesser sind als ihre Ueberschrift, liest sich
/// wie vier Seiten.** Gemeldet vom Projektinhaber am 2026-09-11.
#[test]
fn die_ueberschriften_haben_genau_zwei_stufen() {
    let css = lies("stil.css");
    let groesse = |wahl: &str| -> f32 {
        let i = css.find(wahl).unwrap_or_else(|| panic!("{wahl} fehlt im Stilblatt"));
        let rest = &css[i..];
        let j = rest.find("font-size:").unwrap_or_else(|| panic!("{wahl} ohne Groesse"));
        let z = &rest[j + "font-size:".len()..];
        let ende = z.find("rem").unwrap_or_else(|| panic!("{wahl}: keine rem-Angabe"));
        z[..ende].trim().parse::<f32>().unwrap_or_else(|e| panic!("{wahl}: {e}"))
    };
    let seite = groesse(".seitenkopf h1");
    let abschnitt = groesse("#einstellungsseite h2,\n.bereichszeile th {");
    assert!(
        seite > abschnitt,
        "die Seitenueberschrift ({seite}rem) ist nicht groesser als die Abschnitte ({abschnitt}rem)"
    );
    // ⚑ **Beide aus derselben Regel**, also kann es gar keine dritte
    // Groesse geben: Die Pruefung haelt fest, dass sie zusammenstehen.
    assert!(
        css.contains("#einstellungsseite h2,\n.bereichszeile th {"),
        "Abschnitte und Bereiche haben nicht mehr dieselbe Regel"
    );
    // Und die Linie darunter trennt die Bereiche sichtbar.
    let i = css.find("#einstellungsseite h2,\n.bereichszeile th {").expect("Regel");
    let block = &css[i..i + 260];
    assert!(block.contains("border-bottom"), "den Ueberschriften fehlt die Trennlinie");
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

/// Alle Klassen, die das Skript vergibt, aus dem Skript gelesen.
///
/// ⚑ **`className = "..."`, die festen Teile einer Vorlage und
/// `classList.add/remove/toggle`.** Der veraenderliche Teil einer
/// Vorlage laesst sich nicht ablesen, der feste davor schon: Aus
/// ``` `beitrag von-${b.von}` ``` kommt `beitrag`, und `von-` faellt
/// weg, weil dort ein Wort angehaengt wird.
fn klassen_aus_skript(js: &str) -> BTreeSet<String> {
    let mut aus = BTreeSet::new();
    for (marke, ende) in [("className = \"", '"'), ("className = `", '`')] {
        let mut rest = js;
        while let Some(a) = rest.find(marke) {
            let nach = &rest[a + marke.len()..];
            let Some(e) = nach.find(ende) else { break };
            for k in nach[..e].split("${").next().unwrap_or("").split_whitespace() {
                if !k.ends_with('-') {
                    aus.insert(k.to_string());
                }
            }
            rest = &nach[e..];
        }
    }
    for marke in ["classList.add(\"", "classList.remove(\"", "classList.toggle(\""] {
        let mut rest = js;
        while let Some(a) = rest.find(marke) {
            let nach = &rest[a + marke.len()..];
            let Some(e) = nach.find('"') else { break };
            aus.insert(nach[..e].to_string());
            rest = &nach[e..];
        }
    }
    aus
}

/// Gibt es zu dieser Klasse eine Regel?
///
/// ⛑ **Auf ganze Namen und nicht auf Teilzeichenketten.** Ein blosses
/// `css.contains(".zu")` faende auch `.zusatz`, und `js.contains("grenze")`
/// fand seinerzeit das Wort „Obergrenze" in einem Kommentar.
fn hat_regel(css: &str, klasse: &str) -> bool {
    let marke = format!(".{klasse}");
    let mut rest = css;
    while let Some(a) = rest.find(&marke) {
        let nach = &rest[a + marke.len()..];
        let ende_ist_wortende = nach
            .as_bytes()
            .first()
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_'));
        if ende_ist_wortende {
            return true;
        }
        rest = nach;
    }
    false
}

/// Und die Klassen, die das Skript vergibt, ebenso.
///
/// ⛑ **Diese Pruefung hielt eine von Hand gepflegte Liste von elf
/// Namen.** Das ist derselbe Fehler wie Fund 261, nur an einer anderen
/// Stelle: Die Liste rottet. Als am 2026-09-09 die Marke `grenze`
/// entfiel, weil ihr Hinweis in den Satz unter der Beschriftung
/// gewandert ist, blieb sie in der Liste stehen, und die Pruefung fiel
/// wegen einer Klasse, die es nicht mehr gibt. Sie liest die Namen
/// jetzt aus dem Skript.
#[test]
fn jede_klasse_aus_dem_skript_hat_eine_regel() {
    let js = lies("app.js");
    let css = lies("stil.css");

    let vergeben = klassen_aus_skript(&js);
    assert!(vergeben.len() >= 15, "nur {} Klassen gefunden; liest die Pruefung noch richtig?", vergeben.len());
    for k in &vergeben {
        assert!(hat_regel(&css, k), "`{k}` wird im Skript vergeben, hat aber keine Regel");
    }

    // ⚑ **Die Schrittarten kommen aus einer Vorlage und lassen sich
    // deshalb nicht als vergebene Klasse ablesen.**
    //
    // ⛑ **Hier stand bis zum 2026-09-10 eine Liste von vier Namen von
    // Hand**, also Fund 271 zum vierten Mal in dieser Datei, und sie war
    // gerottet: `hinweis` stand darin, obwohl der Ruecken diese Art
    // laengst nicht mehr erzeugt. Die alte Marketabelle im Skript trug
    // den Namen mit, und deshalb blieb die Pruefung gruen.
    //
    // ⚑ **Gelesen wird jetzt der Ruecken selbst**, also die Arten, die
    // `zeile_aus` vergibt. Eine Art, die dazukommt, muss danach eine
    // Marke und eine Regel haben; eine, die entfaellt, faellt hier auf.
    let rs = lies_quelle("main.rs");
    let mut arten = BTreeSet::new();
    let mut rest = rs.as_str();
    while let Some(a) = rest.find("art: \"") {
        let nach = &rest[a + "art: \"".len()..];
        let Some(e) = nach.find('"') else { break };
        arten.insert(nach[..e].to_string());
        rest = &nach[e..];
    }
    assert!(
        arten.len() >= 4,
        "nur {} Schrittarten im Ruecken gefunden; liest die Pruefung ihn noch richtig?",
        arten.len()
    );
    for k in &arten {
        // ⚑ **Gefragt wird, ob das Fenster die Art kennt**, und nicht,
        // wie es sie zeigt. Die meisten werden zu einer Zeile mit
        // Marke; `denken` wird seit dem 2026-09-10 zu einem eigenen
        // Block, und eine Marke dafuer waere ein Eintrag, den niemand
        // liest. Eine Art, die gar nicht vorkommt, faellt weiterhin auf.
        assert!(
            js.contains(&format!("\"{k}\"")) || js.contains(&format!("{k}:")),
            "Der Ruecken erzeugt die Schrittart `{k}`, das Fenster kennt sie nicht."
        );
        assert!(hat_regel(&css, k), "die Schrittart `{k}` hat keine Regel");
    }
}

/// ⛑ **Keine Fremdquelle.** Die Sicherheitsregel erlaubt nur `self`;
/// eine Oberflaeche, die Schriften oder Skripte aus dem Netz nachlaedt,
/// hat eine Verbindung, die niemand angemeldet hat, und sie faellt
/// nicht auf, weil sie einfach nicht laedt.
#[test]
fn nichts_wird_aus_dem_netz_geladen() {
    // ⛑ **Der Namensraum eines SVG ist eine Kennung und keine
    // Adresse.** `createElementNS("http://www.w3.org/2000/svg", …)`
    // ruft nichts ab, der Text steht in jedem SVG der Welt, und ohne
    // ihn erzeugt der Browser ein HTML-Element namens „svg", das
    // nichts zeichnet. Er wird deshalb vor der Suche weggeschnitten
    // und nicht von der Suche ausgenommen: Wer eine **zweite**
    // `http://`-Stelle einbaut, faellt weiter auf.
    const SVG_NS: &str = "http://www.w3.org/2000/svg";
    for datei in ["index.html", "stil.css", "app.js", "netz.js"] {
        let inhalt = lies(datei).replace(SVG_NS, "");
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
    let stil = ohne_kommentare(&lies("stil.css"));
    let i = stil.find("prefers-reduced-motion").expect("die Regel gibt es");

    // ⛑ **Der Block hat ein Ende, und alles danach zaehlt wieder mit.**
    //   Ein erster Entwurf durchsuchte nur `stil[..i]`, also den Teil
    //   VOR der Regel. Eine Bewegung, die danach steht, sah er nicht,
    //   und die Gegenprobe schlug prompt nicht an: Genau die Sorte
    //   Wache, die gruen ist, weil sie nicht hinsieht.
    let ab = stil[i..].find('{').map(|k| i + k + 1).expect("Blockanfang");
    let mut tiefe = 1usize;
    let mut bis = ab;
    for (k, c) in stil[ab..].char_indices() {
        match c {
            '{' => tiefe += 1,
            '}' => {
                tiefe -= 1;
                if tiefe == 0 {
                    bis = ab + k;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = stil[ab..bis].to_string();
    let ausserhalb = format!("{}{}", &stil[..i], &stil[bis..]);

    // ⛑ **Die Liste wird hergeleitet und nicht gepflegt.** Sie stand
    //   hier fest, und als `button::after` aus dem Stilblatt
    //   verschwand, schlug die Pruefung auf etwas an, das es nicht mehr
    //   gibt. Eine Wache, die eine Abschrift fuehrt, verrottet mit der
    //   Vorlage; diese liest die Vorlage.
    //
    // ⚑ Gesucht wird jeder Selektor, der ausserhalb des Blocks eine
    //   `animation` setzt. Wer sich bewegt, muss sich abbestellen
    //   lassen.
    let mut beweglich: BTreeSet<String> = BTreeSet::new();
    for block_text in ausserhalb.split('}') {
        let Some((selektor, regeln)) = block_text.split_once('{') else { continue };
        if regeln.contains("animation:") && !regeln.contains("animation: none") {
            for teil in selektor.split(',') {
                let s = teil.trim();
                if !s.is_empty() && !s.starts_with('@') && !s.starts_with('%') {
                    beweglich.insert(s.to_string());
                }
            }
        }
    }

    // ⚑ Das Netz bewegt sich vom Skript aus, nicht vom Stilblatt; es
    //   steht deshalb zusaetzlich hier.
    beweglich.insert("#netz".to_string());

    assert!(!beweglich.is_empty(), "nichts bewegt sich? dann stimmt die Suche nicht");
    for was in &beweglich {
        assert!(
            block.contains(was.as_str()),
            "{was} bewegt sich, wird im Block fuer reduzierte Bewegung aber\n\
             nicht abbestellt:\n{block}"
        );
    }
}

/// ⛑ **Die eingebettete Schrift braucht `font-src … data:` in der
/// CSP.** Ohne das greift `default-src 'self'`, und ein `data:`-URI
/// ist nicht `self`: Die Wortmarke faellt in der echten App auf die
/// Systemschrift zurueck. **Die Vorschau kann das nicht zeigen**, denn
/// Quick Look kennt keine CSP; gefunden wurde es beim Lesen der
/// Konfiguration, nicht beim Ansehen.
#[test]
fn die_eingebettete_schrift_ist_von_der_csp_erlaubt() {
    let stil = lies("stil.css");
    if !stil.contains("data:font") {
        return; // keine eingebettete Schrift, nichts zu erlauben
    }
    let konf = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"),
    )
    .expect("tauri.conf.json");
    let csp = konf
        .lines()
        .find(|z| z.contains("\"csp\""))
        .unwrap_or_else(|| panic!("keine CSP in der Konfiguration"));
    assert!(
        csp.contains("font-src") && csp[csp.find("font-src").unwrap()..].contains("data:"),
        "der Stil bettet eine Schrift als `data:` ein, die CSP erlaubt das nicht:\n{csp}"
    );
}

/// ⛑ **`listen` ist ein Kernbefehl und braucht eine Erlaubnis.**
/// Eigene Befehle laufen in Tauri 2 ohne, `core:event` nicht: Ohne
/// Faehigkeitsdatei bekaeme das Fenster die Baufortschritte nie zu
/// sehen, und zwar **stumm**, ohne Fehler in der Oberflaeche.
#[test]
fn wer_horcht_braucht_die_erlaubnis_dazu() {
    let js = lies("app.js");
    if !js.contains("listen(") {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let dateien: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|x| std::fs::read_to_string(x.path()).unwrap_or_default())
        .collect();
    assert!(
        dateien.iter().any(|t| t.contains("core:event")),
        "`app.js` horcht auf Ereignisse, aber keine Faehigkeit erlaubt `core:event`"
    );
    // ⚑ Und die Faehigkeit muss auf ein Fenster zeigen, das es gibt.
    let konf = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"),
    )
    .expect("tauri.conf.json");
    for t in &dateien {
        for fenster in ["main"] {
            if t.contains(&format!("\"{fenster}\"")) {
                assert!(
                    konf.contains(&format!("\"label\": \"{fenster}\"")),
                    "die Faehigkeit nennt das Fenster `{fenster}`, die Konfiguration kennt es nicht"
                );
            }
        }
    }
}

/// ⛑ **Nichts, was gelesen werden soll, darf auf eine Animation
/// warten.** `animation: … both` setzt den Anfangszustand schon vor
/// dem Lauf; steht dort `opacity: 0`, ist das Element unsichtbar,
/// solange die Animation nicht laeuft. Bei einer Meldung heisst das:
/// Sie erscheint nie. Gefunden im Standbild der Vorschau, wo
/// Animationen nicht laufen.
///
/// ⚑ **Der Vorhang darf es**, denn er traegt eine Wartezeit und sein
/// Text soll sich aus dem Rauschen zusammensetzen; er ist die einzige
/// Ausnahme und steht hier namentlich.
#[test]
fn nichts_wartet_unsichtbar_auf_eine_animation() {
    let stil = ohne_kommentare(&lies("stil.css"));
    for sel in selektoren_mit(&stil, "animation:") {
        if sel.contains("vorhangtext") {
            continue;
        }
        let mut rest = stil.as_str();
        while let Some(auf) = rest.find('{') {
            let Some(zu) = rest[auf..].find('}') else { break };
            let kopf = rest[..auf].rsplit(['}', ';']).next().unwrap_or("");
            let rumpf = &rest[auf + 1..auf + zu];
            if kopf.contains(&sel) && rumpf.contains("animation:") {
                assert!(
                    !rumpf.contains(" both") && !rumpf.contains(" backwards"),
                    "`{sel}` steht vor seiner Animation auf dem Anfangszustand \
                     (`both`/`backwards`). Laeuft sie nicht, bleibt es unsichtbar:\n{rumpf}"
                );
            }
            rest = &rest[auf + zu + 1..];
        }
    }
}

/// **Die Buendelversion ist die Kistenversion.**
///
/// ⛑ Sie war es nicht: `tauri.conf.json` stand am 2026-09-09 auf
/// `0.1.0`, waehrend die Kiste bei `0.13.0` war. Sichtbar wird das
/// erst am fertigen Buendel, denn der Dateiname traegt sie:
/// `Myelith_0.1.0_amd64.deb` neben einer Freigabe, die anders heisst.
/// Wer die Datei spaeter zuordnen will, hat dann zwei Versionen und
/// keine Auskunft, welche gilt.
#[test]
fn die_buendelversion_ist_die_kistenversion() {
    let konf = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"),
    )
    .expect("tauri.conf.json");
    let gesucht = format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION"));
    assert!(
        konf.contains(&gesucht),
        "tauri.conf.json traegt nicht die Version der Kiste ({}).\n\
         Der Dateiname jedes Buendels kommt aus dieser Zahl.",
        env!("CARGO_PKG_VERSION")
    );
}

/// **Und das Buendelskript liest die Fassung, statt sie zu tragen.**
///
/// ⛑ **Der zweite Ort derselben Zahl, und er lief davon.**
/// `buendeln-macos.sh` schrieb `0.4.0` als festen Text in die
/// `Info.plist`, waehrend die Kiste bei `0.16.0` stand. Die Pruefung
/// darueber band `tauri.conf.json` an die Kiste und dieses Skript an
/// nichts; gemessen am 2026-09-10 trug jedes doppelgeklickte Buendel
/// zwoelf Anhebungen zu wenig, und das oertliche Freigabeskript gab sie so
/// weiter.
///
/// ⚑ **Geprueft wird die Ableitung, nicht die Zahl.** Eine Pruefung,
/// die den Wert `0.16.0` im Skript sucht, waere beim naechsten Sprung
/// rot, ohne dass etwas kaputt ist. Hier faellt sie genau dann, wenn
/// jemand die Ableitung wieder durch eine Zahl ersetzt.
#[test]
fn das_buendelskript_liest_die_fassung() {
    let skript = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/buendeln-macos.sh"),
    )
    .expect("buendeln-macos.sh");

    assert!(
        skript.contains("FASSUNG=$(grep -m1 '^version' CLIENT/myl-oberflaeche/Cargo.toml"),
        "Das Buendelskript leitet die Fassung nicht mehr aus der Cargo.toml ab."
    );

    // Beide Schluessel, und beide ueber die Variable. `CFBundleVersion`
    // ist ein echtes Praefix von `CFBundleShortVersionString`, deshalb
    // wird auf die ganze Zeile geprueft und nicht auf den Schluessel.
    for schluessel in ["CFBundleVersion", "CFBundleShortVersionString"] {
        let zeile = skript
            .lines()
            .find(|z| z.trim_start().starts_with(&format!("<key>{schluessel}</key>")))
            .unwrap_or_else(|| panic!("{schluessel} steht nicht in der Info.plist"));
        assert!(
            zeile.contains("<string>${FASSUNG}</string>"),
            "{schluessel} traegt eine feste Zahl statt der Fassung aus der Cargo.toml:\n  {zeile}\n\
             Ein Buendel meldet dann eine Fassung, die es nicht ist."
        );
    }

    // ⚑ Und das Dokument muss unquotiert sein, sonst steht die Variable
    // wortwoertlich in der Datei. Beides zusammen ist die Zusage; eines
    // allein ist ein Buendel mit `${FASSUNG}` als Versionsangabe.
    assert!(
        skript.contains("<<PLIST") && !skript.contains("<<'PLIST'"),
        "Das Plist-Dokument ist quotiert; ${{FASSUNG}} bliebe dann als Text stehen."
    );
}

/// **Jedes angemeldete Symbol liegt auch da, und die zwei
/// Systemformate sind dabei.**
///
/// ⛑ Bis zum 2026-09-09 lagen nur PNG-Dateien im Verzeichnis. Der
/// Buendler braucht fuer das `.msi` ein `.ico` und fuer das `.dmg` ein
/// `.icns`; was er ohne sie liefert, ist im guenstigen Fall ein
/// Abbruch und im unguenstigen ein Buendel mit dem Platzhalter des
/// Werkzeugs, und das sieht erst der, der es anklickt.
///
/// Beide sind aus `icon.png` abgeleitet und liegen fertig da.
#[test]
fn die_angemeldeten_symbole_liegen_da() {
    let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let konf = std::fs::read_to_string(wurzel.join("tauri.conf.json")).expect("tauri.conf.json");

    let liste = konf
        .split_once("\"icon\": [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(inhalt, _)| inhalt)
        .expect("Symbolliste");

    let mut gezaehlt = 0;
    for stueck in liste.split(',') {
        let pfad = stueck.trim().trim_matches('"');
        if pfad.is_empty() {
            continue;
        }
        gezaehlt += 1;
        assert!(
            wurzel.join(pfad).exists(),
            "tauri.conf.json meldet `{pfad}` an, die Datei fehlt aber."
        );
    }
    assert!(gezaehlt >= 4, "nur {gezaehlt} Symbole angemeldet");

    for pflicht in ["icons/icon.ico", "icons/icon.icns"] {
        assert!(
            liste.contains(pflicht),
            "`{pflicht}` fehlt in der Symbolliste. Ohne es buendelt Tauri \
             fuer dieses System kein brauchbares Symbol."
        );
    }
}

/// **Und die Gegenrichtung: Jede Klasse mit einer Regel wird auch
/// vergeben.**
///
/// ⛑ Die beiden Pruefungen oben gehen von HTML und Skript zum CSS. Die
/// Richtung faengt eine Klasse ohne Gestaltung, aber keine Gestaltung
/// ohne Klasse. Genau die blieb am 2026-09-09 zurueck: Der Ortsschalter
/// „Lokal | Netz" war entfallen, `.schalter` stand aber weiter in fuenf
/// Regeln, darunter in der Linsenliste des Skripts. Tote Regeln sind
/// nicht nur Ballast: Wer eine davon anfasst und nichts passieren
/// sieht, sucht den Fehler an der falschen Stelle.
#[test]
fn jede_regel_hat_ein_element() {
    let html = lies("index.html");
    let js = lies("app.js");
    let css = ohne_kommentare(&lies("stil.css"));

    // ⚑ Nur Klassen aus Selektoren, also vor `{`. Ein `.` in einer
    //   Zahl oder in einem Kommentar zaehlt nicht.
    let mut aus_regeln: BTreeSet<String> = BTreeSet::new();
    for block in css.split('{') {
        let selektor = block.rsplit('}').next().unwrap_or("");
        let mut rest = selektor;
        while let Some(a) = rest.find('.') {
            let nach = &rest[a + 1..];
            let ende = nach
                .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(nach.len());
            if ende > 0 && nach.as_bytes()[0].is_ascii_alphabetic() {
                aus_regeln.insert(nach[..ende].to_string());
            }
            rest = &nach[ende..];
        }
    }

    // ⛑ **Zusammengesetzte Klassennamen stehen nirgends als Literal.**
    //   `wurzel.className = `beitrag von-${b.von}`` vergibt
    //   `von-nutzer` und `von-modell`, im Skript steht aber nur
    //   `von-`. Der erste Entwurf dieser Pruefung hielt
    //   `.beitrag.von-nutzer` deshalb fuer tot; ein Loeschen haette
    //   Nutzerbeitraege stillschweigend linksbuendig gemacht.
    //
    //   Gesammelt wird darum der feste Anfang jeder Vorlage: der Text
    //   bis zum ersten `${`, davon das letzte Wort. Endet er auf einem
    //   Leerzeichen, ist die ganze Klasse aus Daten gebaut, und dann
    //   traegt die Pruefung oben sie namentlich.
    let mut vorsilben: Vec<String> = Vec::new();
    let mut rest = js.as_str();
    while let Some(a) = rest.find("className = `") {
        let nach = &rest[a + "className = `".len()..];
        let ende = nach.find("${").unwrap_or(0);
        if let Some(letztes) = nach[..ende].split_whitespace().last() {
            if !letztes.is_empty() && nach[..ende].ends_with(letztes) {
                vorsilben.push(letztes.to_string());
            }
        }
        rest = &nach[ende..];
    }

    let im_html = klassen(&html);
    let mut tot: Vec<String> = Vec::new();
    for k in &aus_regeln {
        // Das Skript vergibt Klassen als Zeichenkette, deshalb genuegt
        // das Vorkommen des Namens darin.
        let benutzt = im_html.contains(k)
            || js.contains(k.as_str())
            || vorsilben.iter().any(|v| k.starts_with(v.as_str()));
        if !benutzt {
            tot.push(k.clone());
        }
    }
    assert!(
        tot.is_empty(),
        "Regeln ohne Element: {tot:?}\n\
         Entweder fehlt das Element oder die Regel ist ein Rest."
    );
}

/// **Jedes Bauskript laeuft mit der Kalibrier-Umgebung im Pfad.**
///
/// ⛑ Bis zum 2026-09-09 bekam nur der Kalibrierschritt sie. Der
/// Download davor lief mit dem blossen System-`PATH`, und
/// `fetch_model.sh` bricht ohne `hf` ab. Der Befehl liegt aber genau in
/// dieser Umgebung, denn `huggingface_hub` wird dorthin installiert.
/// Auf einem **richtig** eingerichteten Klon waere „Download" damit
/// fehlgeschlagen, mit der Aufforderung zu installieren, was schon da
/// ist. Das faellt in keinem Bau auf: Es ist eine fehlende Zeile und
/// kein Fehler.
#[test]
fn jedes_bauskript_bekommt_die_umgebung_in_den_pfad() {
    let quelle = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("main.rs");

    let mut gefunden = 0;
    let mut rest = quelle.as_str();
    while let Some(a) = rest.find("Command::new(\"bash\")") {
        // Der Aufbau eines Befehls endet an seinem Komma vor dem
        // naechsten Argument von `lauf_mit_ausgabe`; ein grosszuegiges
        // Fenster genuegt, um die Kette der `.env` zu sehen.
        let fenster = &rest[a..(a + 700).min(rest.len())];
        assert!(
            fenster.contains("pfad_mit_venv"),
            "ein Bauskript wird ohne die Kalibrier-Umgebung im Pfad gefahren:\n{}",
            fenster.lines().take(6).collect::<Vec<_>>().join("\n")
        );
        gefunden += 1;
        rest = &rest[a + 20..];
    }
    assert!(gefunden >= 2, "nur {gefunden} Bauskriptaufrufe gefunden, erwartet zwei");

    // ⚑ Und die Stelle, an der der Pfad zusammengesetzt wird, gibt es
    //   nur einmal. Zwei Fassungen liefen irgendwann auseinander, und
    //   genau das war der Fehler.
    assert_eq!(
        quelle.matches("calibrate/.venv/bin\")").count(),
        1,
        "der Pfad zur Kalibrier-Umgebung steht mehr als einmal im Quelltext"
    );
}

/// **Wer `window.__TAURI__` benutzt, braucht `withGlobalTauri`.**
///
/// ⛑ Ohne die Zeile in `tauri.conf.json` gibt es das Objekt nicht. Das
/// Skript holt sich `invoke` daraus in seiner **zweiten** Zeile; ein
/// Zugriff auf `undefined` wirft dort, und dann laeuft vom Modul
/// ueberhaupt nichts. Das Fenster bleibt am Vorschaltbild stehen und
/// meldet nichts, denn der Fehler passiert, bevor irgendein `catch`
/// existiert.
///
/// ⚑ **Keine der uebrigen Pruefungen faengt das**, und das ist der
/// Grund, warum es diese gibt: Sie lesen HTML, CSS und Skript als
/// Text. Ob die Bruecke ins Fenster ueberhaupt da ist, entscheidet die
/// Konfiguration, und die stand nie daneben.
#[test]
fn wer_die_globale_bruecke_benutzt_muss_sie_anmelden() {
    let js = lies("app.js");
    let konf = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"),
    )
    .expect("tauri.conf.json");

    if js.contains("window.__TAURI__") {
        assert!(
            konf.contains("\"withGlobalTauri\": true"),
            "app.js greift auf `window.__TAURI__` zu, aber `withGlobalTauri`\n\
             steht nicht in tauri.conf.json. Das Objekt gibt es dann nicht,\n\
             und das Fenster bleibt am Vorschaltbild stehen."
        );
    }
}

/// **Die Pseudoelemente rechnen im selben Kastenmodell wie alles
/// andere.**
///
/// ⛑ `* { box-sizing: border-box }` trifft `::before` und `::after`
/// **nicht**. Beide tragen die Ringe der Glasoptik, mit
/// `position: absolute; inset: 0; padding: 1px`; im `content-box`-Modell
/// kommt das Padding aussen dazu, der Ring wird zwei Pixel groesser als
/// sein Traeger und sitzt sichtbar daneben. Gemeldet wurde es als
/// „Highlight nach links verschoben" an jedem runden Knopf.
///
/// ⚑ Die Pruefung haengt an der Technik und nicht am Wortlaut: Sie
/// greift nur, wenn ueberhaupt ein Ring ueber `inset` und `padding`
/// gebaut wird.
#[test]
fn die_ringe_rechnen_im_randkasten() {
    let css = ohne_kommentare(&lies("stil.css"));
    let baut_ringe = css.contains("::before") && css.contains("inset: 0");
    if !baut_ringe {
        return;
    }
    let hat = css.contains("*::before") && css.contains("*::after");
    assert!(
        hat,
        "Die Ringe liegen auf ::before und ::after, aber `box-sizing`\n\
         gilt nur fuer `*`. Pseudoelemente sind davon nicht erfasst und\n\
         werden um ihr Padding groesser als ihr Traeger."
    );
}

/// Eine Datei aus `src/`, nicht aus `ui/`.
fn lies_quelle(name: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// **Jeder Befehl ist angemeldet, und jeder gerufene Name ist ein
/// Befehl.**
///
/// ⛑ **Das ist die Luecke der Sorte 251:** `#[tauri::command]` allein
/// tut nichts. Fehlt der Name in `generate_handler!`, uebersetzt alles
/// sauber, die Kiste ist gruen, und der Aufruf scheitert erst beim
/// Klicken mit „not allowed by ACL" oder „command not found". Beim
/// Ordnerauswaehler waere das ein Knopf gewesen, der aussieht, als
/// tue er etwas.
///
/// ⚑ **Drei Richtungen, denn jede kann einzeln kaputtgehen:** ein
/// Befehl ohne Anmeldung, eine Anmeldung ohne Befehl (nach einer
/// Umbenennung), und ein Aufruf im Fenster, den es im Ruecken nicht
/// gibt.
#[test]
fn jeder_befehl_ist_angemeldet() {
    let rs = lies_quelle("main.rs");
    let js = lies("app.js");

    /// Der Funktionsname nach einem `#[tauri::command]`.
    fn befehle(rs: &str) -> BTreeSet<String> {
        let mut aus = BTreeSet::new();
        for teil in rs.split("#[tauri::command]").skip(1) {
            let Some(a) = teil.find("fn ") else { continue };
            let nach = &teil[a + 3..];
            let ende = nach
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(nach.len());
            aus.insert(nach[..ende].to_string());
        }
        aus
    }

    let vorhanden = befehle(&rs);
    assert!(vorhanden.len() >= 10, "nur {} Befehle gefunden; sucht die Pruefung noch richtig?", vorhanden.len());

    let liste = rs
        .split("tauri::generate_handler![")
        .nth(1)
        .expect("keine Anmeldeliste im Ruecken")
        .split(']')
        .next()
        .unwrap_or("");
    let angemeldet: BTreeSet<String> = liste
        .split(',')
        .map(|z| z.trim().to_string())
        .filter(|z| !z.is_empty())
        .collect();

    assert_eq!(
        vorhanden, angemeldet,
        "Befehle und Anmeldeliste stimmen nicht ueberein.\n\
         Nur mit `#[tauri::command]`: {:?}\n\
         Nur in `generate_handler!`: {:?}",
        vorhanden.difference(&angemeldet).collect::<Vec<_>>(),
        angemeldet.difference(&vorhanden).collect::<Vec<_>>()
    );

    // Und die dritte Richtung: Was das Fenster ruft, muss es geben.
    let mut rest = js.as_str();
    let mut gerufen = BTreeSet::new();
    while let Some(a) = rest.find("invoke(\"") {
        let nach = &rest[a + "invoke(\"".len()..];
        let Some(e) = nach.find('"') else { break };
        gerufen.insert(nach[..e].to_string());
        rest = &nach[e..];
    }
    assert!(!gerufen.is_empty(), "das Fenster ruft gar keinen Befehl; sucht die Pruefung noch richtig?");
    for name in &gerufen {
        assert!(
            angemeldet.contains(name),
            "das Fenster ruft `{name}`, aber der Ruecken meldet ihn nicht an"
        );
    }

    // ⚑ **Und die vierte Richtung: Was niemand ruft, gehoert weg.**
    // Diese Haelfte hat beim ersten Lauf `modell` gefunden, einen
    // Befehl, der ein ganzes Artefakt laedt, dessen Vorlage druckt und
    // es wegwirft; gerufen hat ihn nichts. Ein angemeldeter Befehl ist
    // eine Zusage an das Fenster, und eine Zusage, die niemand
    // einloest, ist eine Behauptung.
    assert_eq!(
        angemeldet, gerufen,
        "angemeldet, aber vom Fenster nicht gerufen: {:?}",
        angemeldet.difference(&gerufen).collect::<Vec<_>>()
    );

    // ⚑ **Und die fuenfte Richtung: die Zahl im Komponenten-README.**
    //
    // ⛑ Dieselbe Zahl stand am 2026-09-10 an drei Orten und war dreimal
    // verschieden: „sechzehn Stellen" im README, „zweiundzwanzig" zwei
    // Papiere weiter, neunundzwanzig gezaehlt. Die fluechtige Zahl ist
    // gestrichen; die tragende ist die der Befehle, und sie steht ab
    // hier nicht mehr unverbunden da.
    let readme = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("README")
            .join("README.md"),
    )
    .expect("CLIENT/README/README.md");
    let gesucht = format!("über {} Befehle", zahlwort(angemeldet.len()));
    assert!(
        readme.contains(&gesucht),
        "Das Komponenten-README nennt nicht die heutige Zahl der Befehle.\n\
         Gezaehlt sind {} ({}), gesucht war der Satzteil `{}`.",
        angemeldet.len(),
        zahlwort(angemeldet.len()),
        gesucht
    );
}

/// **Jedes setzbare Feld zeigt auch seinen Wert.**
///
/// ⛑ **Gemessen am 2026-09-10: Es waren neun von zwoelf.** Die Seite
/// zeichnet ihre Zeilen aus der Feldliste der Kiste, holte die
/// anzuzeigenden Werte aber aus einer **zweiten**, von Hand gepflegten
/// Zuordnung im Skript, und die kannte `kap.beschleuniger`,
/// `kap.speicher` und `kap.platte` nicht. In JavaScript ist ein
/// fehlender Schluessel kein Fehler, sondern `undefined`: Der Schalter
/// stand danach **immer aus**, die beiden Textfelder **immer leer**,
/// gleichgueltig was in der Ablage stand.
///
/// ⚑ **Behoben wurde es nicht durch drei nachgetragene Zeilen, sondern
/// durch das Abschaffen der zweiten Zuordnung.** Die Werte kommen
/// heute aus `Einstellungen::wert`, unter demselben Namen, unter dem
/// auch gesetzt wird, und `wert_und_setzer_kennen_dieselben_felder`
/// faehrt in der Kiste jedes Feld einmal hin und zurueck.
///
/// ⚑ **Diese Pruefung haelt fest, dass es dabei bleibt.** Sie prueft
/// nicht mehr den Inhalt einer Liste, sondern dass es die Liste nicht
/// wieder gibt: Das Fenster nimmt die Werte, wie sie kommen, und legt
/// keine eigene Zuordnung von Feldnamen auf Werte an.
#[test]
fn jedes_feld_zeigt_seinen_wert() {
    let js = lies("app.js");

    assert!(
        js.contains("const wert = e.werte;"),
        "Das Fenster nimmt die Werte nicht mehr geschlossen aus der Kiste.\n\
         Sobald es sie einzeln zusammensucht, kann eines fehlen, und ein\n\
         fehlender Schluessel sieht im Fenster aus wie „nicht gesetzt\"."
    );

    // ⚑ **Und keine zweite Zuordnung daneben.** Ein Feldname als
    // Objektschluessel, also `"modell.artefakt":`, ist genau die Form,
    // die den Fund gemacht hat. Aufrufe wie
    // `setzen({feld: "modell.artefakt"})` sind davon nicht betroffen:
    // Dort steht der Name hinter einem Doppelpunkt und nicht davor.
    //
    // ⛑ **Sie sucht die WIRKLICHEN Feldnamen und nicht ihre Anfaenge**
    // (2026-09-10). Vorher genuegte ein Schluessel, der mit `"modell.`
    // begann, und damit fiel sie ueber die Sprachtabelle: `"modell.laedt"`
    // ist kein Feld, sondern ein Satz. **Eine Pruefung, die Aehnlichkeit
    // fuer Gleichheit haelt, bestraft jeden, der in derselben Gegend
    // etwas Neues anlegt** (dieselbe Klasse wie die Wache, die
    // `innerHTML` in einem Kommentar fand).
    let namen: BTreeSet<String> = myl_client::einstellungen::FELDER
        .iter()
        .map(|f| f.name.to_string())
        .collect();
    for zeile in js.lines() {
        let z = zeile.trim();
        for name in &namen {
            let marke = format!("\"{name}\"");
            if let Some(a) = z.find(&marke) {
                assert!(
                    !z[a + marke.len()..].trim_start().starts_with(':'),
                    "Im Fenster steht wieder eine Zuordnung von Feldnamen auf Werte:\n  {z}\n\
                     Genau die ist am 2026-09-10 mit Fund 280 entfallen."
                );
            }
        }
    }
}

/// **Jeder Regler kennt sein Ende, und ein gesperrter sagt, was fehlt.**
///
/// ⚑ **Die Zusage, um die es geht:** Ein Regler, der nichts bewirkt,
/// darf nicht aussehen wie einer, der wirkt, und er muss sagen, was
/// dafuer geschrieben werden muss. Ein blosses „noch nicht verfuegbar"
/// liesse den Leser genauso klug zurueck wie zuvor.
#[test]
fn ein_gesperrter_regler_sagt_was_fehlt() {
    let js = lies("app.js");
    let css = ohne_kommentare(&lies("stil.css"));

    // Gesperrt heisst: nicht bedienbar, und zwar im Element selbst und
    // nicht nur in der Farbe.
    assert!(
        js.contains("schieber.disabled = Boolean(r.sperrgrund)"),
        "ein gesperrter Regler laesst sich noch bedienen"
    );
    // Und der Grund haengt am Ding, nicht in einer Anleitung.
    assert!(
        js.contains("zeile.title = r.sperrgrund") && js.contains("schieber.title = r.sperrgrund"),
        "der Sperrgrund erscheint beim Zeigen nicht"
    );
    // Und er steht auch sichtbar da, nicht nur im Titel: Ein Titel
    // erscheint erst beim Zeigen, und auf einem Zeigegeraet ohne Maus
    // gar nicht.
    assert!(
        js.contains("satz.textContent = r.sperrgrund ? r.sperrgrund : r.hinweis"),
        "der Sperrgrund steht nur im Titel und nirgends sichtbar"
    );

    // ⚑ Und ausgegraut ist eine Regel und keine Absicht.
    assert!(
        hat_regel(&css, "gesperrt"),
        "`gesperrt` wird im Skript vergeben, sieht aber aus wie jeder andere Regler"
    );
}

/// **Jede Live-Meldung des Rueckens wird im Fenster auch behandelt.**
///
/// ⚑ **Dieselbe Luecke wie bei den Befehlen, eine Ebene weiter.** Ein
/// Ereignis, das der Ruecken schickt und das Fenster nicht kennt,
/// verschwindet: kein Fehler, keine Meldung, nur eine Anzeige, in der
/// etwas fehlt. Beim Denken waere das der ganze Ueberlegungsblock, und
/// niemand merkte es, weil er ohnehin zugeklappt gehoert.
///
/// ⚑ **Und der Kanalname gehoert dazu.** Ein Tippfehler dort macht die
/// ganze Live-Anzeige still: Der Ruecken meldet, das Fenster horcht
/// woanders, und die Antwort erscheint wie zuvor erst am Ende.
#[test]
fn jede_lebende_meldung_wird_behandelt() {
    let rs = lies_quelle("main.rs");
    let js = lies("app.js");

    let kanal = rs
        .split_once("const LEBEND: &str = \"")
        .and_then(|(_, r)| r.split_once('"'))
        .map(|(n, _)| n)
        .expect("kein Kanalname im Ruecken");
    assert!(
        js.contains(&format!("horchen(\"{kanal}\"")),
        "Das Fenster horcht nicht auf `{kanal}`; die Live-Anzeige bliebe stumm."
    );

    let rumpf = rs
        .split_once("enum Lebend {")
        .and_then(|(_, r)| r.split_once("\n}"))
        .map(|(k, _)| k)
        .expect("kein `enum Lebend` im Ruecken");
    let arten: BTreeSet<String> = rumpf
        .lines()
        .map(str::trim)
        .filter(|z| !z.starts_with("//"))
        .filter_map(|z| z.split([' ', '{', ',']).next())
        .filter(|n| !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
        .map(str::to_string)
        .collect();
    assert!(
        arten.len() >= 5,
        "nur {} Meldungsarten gefunden ({arten:?}); liest die Pruefung sie noch richtig?",
        arten.len()
    );

    // ⚑ Der Ruecken serialisiert mit `tag = "art"`, also steht der
    // Variantenname genau so im Ereignis.
    for a in &arten {
        assert!(
            js.contains(&format!("\"{a}\"")),
            "Der Ruecken kann `{a}` melden, das Fenster kennt die Art nicht.\n\
             Sie verschwindet dann spurlos: kein Fehler, nur eine Anzeige ohne sie."
        );
    }
}

/// **Der laufende Beitrag wird fertiggeschrieben und nicht ersetzt.**
///
/// ⚑ **Sonst verschwaende, was live ankam.** Die Rueckgabe traegt den
/// Text und die Schrittliste, aber **nicht die Ueberlegung**; die kam
/// nur ueber den Kanal. Wer den Beitrag am Ende durch einen neuen
/// ersetzte, loeschte sie vor den Augen des Nutzers.
#[test]
fn der_laufende_beitrag_wird_fertiggeschrieben() {
    let js = lies("app.js");
    assert!(
        js.contains("laufender.text = a.text") && js.contains("laufender.schritte = a.verlauf"),
        "Die Rueckgabe schreibt den laufenden Beitrag nicht fertig."
    );
    let rumpf = js
        .split_once("async function senden(")
        .and_then(|(_, r)| r.split_once("\n}"))
        .map(|(k, _)| k)
        .expect("kein `senden` im Skript");
    // ⚑ Genau zwei Anlagen: der Beitrag des Nutzers und, im
    // Fehlerfall, die Fehlermeldung. Die Antwort selbst entsteht ueber
    // `live_anfangen`; ein dritter `push` waere sie ein zweites Mal.
    let anlagen = rumpf.matches("beitraege.push(").count();
    assert_eq!(
        anlagen, 2,
        "in `senden` werden {anlagen} Beitraege angelegt statt zwei \
         (Nutzerbeitrag und Fehlerfall)."
    );
}

/// **Das Fenster setzt niemals Markup.**
///
/// # ⛑ Die Zusage, an der hier alles haengt
///
/// Seit dem 2026-09-10 wird eine Modellantwort als Markdown gezeigt.
/// **Der naheliegende Weg dorthin waere `innerHTML`, und er waere eine
/// Luecke:** Eine Antwort mit `<img src=x onerror=…>` bekaeme damit
/// Code in dieser Seite ausgefuehrt, und diese Seite traegt wegen
/// `withGlobalTauri` die Bruecke zu **allen** Befehlen des Rueckens.
/// Ein eingeschleuster Satz koennte Einstellungen setzen oder Dateien
/// schreiben.
///
/// ⚑ **Deshalb zerlegt die Kiste und das Fenster zeichnet nur.** Was
/// ankommt, ist ein Baum aus Text; daraus werden Elemente mit
/// `createElement` und `textContent`. Ein `<` bleibt ein `<`.
///
/// ⚑ **Diese Pruefung ist die Gegenprobe dazu**, denn ein Kommentar
/// ueber eine Regel belegt nicht, dass sie gilt.
#[test]
fn das_fenster_setzt_niemals_markup() {
    /// Nur der Quelltext, ohne Kommentarzeilen.
    ///
    /// ⛑ **Beim ersten Lauf fiel die Pruefung ueber den Kommentar, der
    /// die Regel erklaert:** „Kein `innerHTML`, nirgends" enthaelt das
    /// Wort. **Eine Pruefung, die Erwaehnung fuer Gebrauch haelt,
    /// bestraft das Aufschreiben der Regel**, und dann schreibt sie
    /// niemand mehr auf.
    fn ohne_kommentare(js: &str) -> String {
        js.lines()
            .filter(|z| {
                let z = z.trim_start();
                !(z.starts_with("//") || z.starts_with('*') || z.starts_with("/*"))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    for datei in ["app.js", "netz.js"] {
        let js = ohne_kommentare(&lies(datei));
        for weg in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "createContextualFragment",
        ] {
            assert!(
                !js.contains(weg),
                "`{datei}` benutzt `{weg}`.\n\
                 Eine Modellantwort ist Daten; daraus Markup zu machen gibt einem\n\
                 eingeschleusten Satz einen Weg zu `invoke`."
            );
        }
        // ⚑ Und kein Skriptelement von Hand. Es waere der zweite Weg
        // zum selben Ziel und faellt der obigen Liste nicht auf.
        for form in ["createElement(\"script\"", "createElement('script'"] {
            assert!(!js.contains(form), "`{datei}` erzeugt ein Skriptelement");
        }
    }
}

/// **Jede Blockart und jedes Stueck der Kiste wird gezeichnet.**
///
/// ⚑ **Dieselbe Luecke wie bei den Befehlen und den Live-Meldungen.**
/// Eine Art, die der Zerleger erzeugt und das Fenster nicht kennt,
/// faellt in den Absatzzweig: kein Fehler, keine Meldung, nur eine
/// Tabelle, die als Textzeile dasteht.
#[test]
fn jede_blockart_wird_gezeichnet() {
    let js = lies("app.js");
    let rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../myl-client/src/markdown.rs"),
    )
    .expect("markdown.rs");

    let varianten = |name: &str| -> BTreeSet<String> {
        let rumpf = rs
            .split_once(&format!("pub enum {name} {{"))
            .and_then(|(_, r)| r.split_once("\n}"))
            .map(|(k, _)| k)
            .unwrap_or_else(|| panic!("kein `enum {name}`"));
        rumpf
            .lines()
            .map(str::trim)
            .filter(|z| !z.starts_with("//"))
            .filter_map(|z| z.split([' ', '{', ',']).next())
            .filter(|n| !n.is_empty() && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
            .map(str::to_string)
            .collect()
    };

    let bloecke = varianten("Block");
    let teile = varianten("Teil");
    assert!(bloecke.len() >= 6, "nur {bloecke:?} gefunden; liest die Pruefung noch richtig?");
    assert!(teile.len() >= 5, "nur {teile:?} gefunden; liest die Pruefung noch richtig?");

    // ⚑ Der Ruecken serialisiert mit `tag = "art"`, also steht der
    // Variantenname genau so im Baum, den das Fenster bekommt.
    for a in bloecke.iter().chain(teile.iter()) {
        // `Absatz` und `Text` sind die Rueckfaelle und stehen deshalb
        // nicht als Vergleich im Skript; alles andere muss vorkommen.
        if a == "Absatz" || a == "Text" {
            continue;
        }
        assert!(
            js.contains(&format!("\"{a}\"")),
            "Die Kiste erzeugt `{a}`, das Fenster kennt die Art nicht.\n\
             Sie faellt dann in den Absatzzweig und sieht aus wie eine Textzeile."
        );
    }
}

/// **Die Zeile unter der Eingabe sagt genau eine Sache.**
///
/// ⛑ **Vorher sagte sie alles Mögliche:** „der Agent faehrt", „Modell
/// gewechselt", „Fehler: …", und dazwischen den Ladesatz. **Eine
/// Zeile, die je nach Augenblick etwas anderes bedeutet, liest man
/// irgendwann gar nicht mehr**: Wer dort „das Modell antwortet" gewohnt
/// ist, sieht „nicht geladen" nicht mehr.
///
/// ⚑ **Geprüft wird, dass sie nur von zwei Stellen beschrieben wird**,
/// und beide sagen dasselbe Thema: welches Modell im Speicher liegt.
#[test]
fn die_zeile_unter_der_eingabe_sagt_nur_den_modellstand() {
    let js = lies("app.js");

    // ⚑ Kein Aufrufer schreibt mehr beliebigen Text dorthin.
    assert!(
        !js.contains("hinweis("),
        "es gibt wieder eine allgemeine Hinweisfunktion; die Zeile bekommt dann \
         wieder alles Moegliche"
    );
    // ⚑ Und die beiden Stellen, die es duerfen, gibt es.
    assert!(
        js.contains("async function modellzeile_schreiben("),
        "es gibt keine Stelle, die den Modellstand schreibt"
    );
    // ⚑ **Gezaehlt wird, wer die Zeile ueberhaupt anfasst.** Zwei
    // Stellen duerfen es: die, die den Ladezustand schreibt, und die,
    // die nach der Ruhefrist „wieder entladen" hinsetzt. Eine dritte
    // waere der Anfang des alten Zustands.
    let schreibende = js.matches("hinweiszeile").count();
    assert_eq!(
        schreibende, 2,
        "{schreibende} Stellen fassen die Zeile an; es gehoeren zwei dorthin \
         (geladen und wieder entladen)"
    );
    // ⛑ Und das Netzmodell hat keinen Ladezustand: Es wird hier nicht
    // geladen, und eine Zeile darueber waere eine Unwahrheit.
    assert!(
        js.contains("if (artefakt.startsWith(NETZMODELL))"),
        "das Netzmodell bekommt eine Ladezeile, obwohl es nicht geladen wird"
    );
}

/// **Das Modell geht nach einer Weile wieder, und der Lauf haelt es.**
///
/// ⚑ **Ein 4B-Artefakt sind viereinhalb Gigabyte**, und sie liegen im
/// Speicher, solange das Fenster offen ist. Das steht in derselben
/// Reihe wie die Kapazitätsfreigabe: Was Myelith nimmt, soll es auch
/// wieder hergeben.
///
/// ⛑ **Die zweite Hälfte ist die wichtigere:** Ein Entladen mitten in
/// einem Lauf wäre entweder wirkungslos oder schlimmer.
#[test]
fn das_modell_geht_nach_einer_weile_wieder() {
    let js = lies("app.js");
    let rs = lies_quelle("main.rs");

    assert!(
        js.contains("const RUHEFRIST_MS = 15 * 60 * 1000;"),
        "es gibt keine Ruhefrist von fuenfzehn Minuten"
    );
    assert!(
        js.contains("invoke(\"modell_entladen\")"),
        "die Frist laeuft ab und niemand entlaedt"
    );
    assert!(
        rs.contains("fn modell_entladen("),
        "der Ruecken kennt das Entladen nicht"
    );
    // ⚑ Nicht mitten im Lauf.
    assert!(
        js.contains("if (laufender) {\n      ruhe_neu_stellen();"),
        "die Frist entlaedt auch waehrend eines Laufs"
    );
    // ⚑ Und ein Modellwechsel gibt das alte frei: Es antwortet ohnehin
    // nicht mehr, sein Speicher waere von da an geschenkt.
    assert!(
        js.matches("modell_entladen").count() >= 2,
        "nur die Frist entlaedt; ein Modellwechsel laesst das alte liegen"
    );
}

/// **Das Ladezeichen steht dort, wo gleich die Antwort steht.**
///
/// ⛑ Vorher stand „der Agent faehrt" unter der Eingabe, also am anderen
/// Ende des Fensters. Wer auf eine Antwort wartet, sieht auf den Fleck,
/// an dem sie erscheinen wird.
#[test]
fn das_ladezeichen_steht_beim_beitrag() {
    let js = lies("app.js");
    let css = ohne_kommentare(&lies("stil.css"));

    assert!(js.contains("l.className = \"laeuft\""), "es gibt kein Ladezeichen");
    assert!(hat_regel(&css, "laeuft"), "das Ladezeichen hat keine Regel");
    // ⚑ Es haengt am Beitrag und nicht an der Zeile unter der Eingabe.
    assert!(
        js.contains("wurzel.append(laufzeichen);"),
        "das Ladezeichen haengt nicht am Beitrag"
    );

    // ⛑ **Und es haengt am Lauf und nicht am Inhalt.**
    //
    // Die erste Fassung zeigte es nur, solange noch gar nichts da war
    // (`b.laufend && !b.text && !schritte.length`). Gemeldet vom
    // Projektinhaber am 2026-09-10: **Nach einem Werkzeugaufruf rechnet
    // das Modell weiter**, oft eine halbe Minute, und in dieser Zeit
    // stand nichts. Ein Ladezeichen, das nur den ersten Wartezeitraum
    // abdeckt, deckt genau den ab, in dem ohnehin gleich etwas kommt.
    assert!(
        js.contains("let laufzeichen = null;\n  if (b.laufend) {"),
        "das Ladezeichen haengt an einer Bedingung ueber den Inhalt statt am Lauf"
    );
}

/// Das deutsche Zahlwort, so wie die READMEs dieses Projekts schreiben.
///
/// ⚑ **Eine Tabelle und kein Rechenwerk.** Ein Zahlwortbildner waere
/// mehr Code als Nutzen; die Tabelle deckt den Bereich, in dem sich
/// diese Zahlen bewegen, und wer darueber hinauskommt, bekommt es
/// gesagt statt eines stillen Durchlaufs.
fn zahlwort(n: usize) -> String {
    const WORTE: [&str; 31] = [
        "null", "ein", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
        "zehn", "elf", "zwölf", "dreizehn", "vierzehn", "fünfzehn", "sechzehn", "siebzehn",
        "achtzehn", "neunzehn", "zwanzig", "einundzwanzig", "zweiundzwanzig",
        "dreiundzwanzig", "vierundzwanzig", "fünfundzwanzig", "sechsundzwanzig",
        "siebenundzwanzig", "achtundzwanzig", "neunundzwanzig", "dreissig",
    ];
    WORTE
        .get(n)
        .map(|w| (*w).to_string())
        .unwrap_or_else(|| panic!("{n} steht nicht in der Zahlworttabelle; ergaenze sie"))
}

/// **Glas tragen Bedienelemente, keine Behaelter.**
///
/// ⛑ Am 2026-09-09 stand `section` in der Linsenliste. Das klang
/// harmlos und war es nicht: `#einstellungsseite` liegt auf `inset: 0`
/// ueber dem ganzen Fenster, `#modellbau` fuellt zwei Drittel davon.
/// Beim Ueberfahren leuchtete also die halbe Einstellungsseite auf,
/// gemeldet vom Projektinhaber als „ein erleuchtender Kasten ueber
/// etwa zwei Dritteln".
///
/// ⚑ **Die Pruefung haengt an der Technik und nicht am Wortlaut.** Sie
/// nimmt die Selektoren, in deren Rumpf `var(--mx` steht, denn das ist
/// der Glanz, und verlangt zweierlei. Erstens darf kein Behaelter
/// darunter sein. Zweitens muss das Skript **genau dieselben** Traeger
/// verfolgen: Rechnet es fuer mehr, ist das Arbeit fuer nichts;
/// rechnet es fuer weniger, sitzt der Glanz bei einem Traeger fest in
/// der Mitte, weil `--mx` nie gesetzt wird.
#[test]
fn glas_traegt_nur_bedienelemente() {
    let css = ohne_kommentare(&lies("stil.css"));
    let js = lies("app.js");

    /// Der Traeger ohne Pseudoelement und ohne `:not(...)`.
    fn stamm(sel: &str) -> String {
        sel.split(':').next().unwrap_or(sel).trim().to_string()
    }

    let glanz: BTreeSet<String> = selektoren_mit(&css, "var(--mx").iter().map(|s| stamm(s)).collect();
    assert!(!glanz.is_empty(), "kein Selektor zeichnet den Glanz; sucht die Pruefung noch richtig?");

    const BEHAELTER: [&str; 8] =
        ["section", "main", "article", "aside", "div", "body", "html", "form"];
    for b in BEHAELTER {
        assert!(
            !glanz.contains(b),
            "`{b}` traegt Glas. Das ist ein Behaelter und kein Bedienelement:\n\
             Er nimmt die ganze Flaeche ein, also leuchtet beim Ueberfahren\n\
             die ganze Flaeche. Glas tragen Knopf, Eingabefeld und Karte."
        );
    }

    let zeile = js
        .lines()
        .find(|z| z.contains("const LINSEN"))
        .expect("`const LINSEN` steht nicht mehr im Skript; wer verfolgt jetzt den Zeiger?");
    let linsen: BTreeSet<String> = zeile
        .split('"')
        .nth(1)
        .expect("die Linsenliste ist keine Zeichenkette mehr")
        .split(',')
        .map(stamm)
        .collect();

    assert_eq!(
        glanz, linsen,
        "Stil und Skript verfolgen verschiedene Traeger.\n\
         Glanz im Stil: {glanz:?}\n\
         Linsen im Skript: {linsen:?}"
    );
}

/// **Was `hidden` traegt, bleibt versteckt.**
///
/// ⛑ Das Vorgabestilblatt setzt `[hidden] { display: none }` mit der
/// schwaechsten Spezifitaet. Eine eigene Regel mit `display: grid` oder
/// `display: block` gewinnt dagegen, und das Element steht sichtbar da,
/// obwohl das Skript es versteckt hat. Dieses Stilblatt hatte den Fall
/// dreimal; zweimal war er einzeln geflickt, beim dritten Mal stand das
/// Kontextmenue nach jedem Start links oben und liess sich nicht
/// schliessen.
#[test]
fn verstecktes_bleibt_versteckt() {
    let html = lies("index.html");
    let css = ohne_kommentare(&lies("stil.css"));
    if !html.contains(" hidden") {
        return;
    }
    assert!(
        css.contains("[hidden]") && css.contains("display: none !important"),
        "Im HTML tragen Elemente `hidden`, aber das Stilblatt hat keine\n\
         Regel `[hidden] {{ display: none !important }}`. Jede eigene\n\
         `display`-Angabe schlaegt sonst das Verstecken."
    );
}

/// **Jede Lizenzangabe steht bei der Sache, fuer die sie gilt.**
///
/// # ⛑ Fund 295, gemeldet vom Projektinhaber am 2026-09-10
///
/// Die Artefaktliste der Einstellungsseite zeigte je Eintrag eine
/// einzelne Lizenz an, und sie stand hinter dem Namen „Myelith 4B":
///
/// ```text
/// Myelith 4B
/// 4 Mrd.  ·  rund 7,5 GB  ·  Artefakt 4,5 GB  ·  Apache-2.0  ·  verifiziert
/// ```
///
/// **Unter Apache-2.0 stehen die Grundgewichte.** Das daraus gebaute
/// Artefakt steht unter der Lizenz dieses Repositoriums: Es ist eine
/// Bearbeitung nach dem Verfahren dieses Projekts, mit eigenen Skalen
/// und Nachschlagetabellen, und es rechnet ganzzahlig, wo das
/// Grundmodell in Gleitkomma rechnet.
///
/// ⚑ **Eine Angabe ist nicht dadurch richtig, dass sie stimmt, sondern
/// dadurch, dass sie sich auf das bezieht, wonebendran sie steht.**
/// „Apache-2.0" war fuer sich genommen wahr und an dieser Stelle
/// falsch.
///
/// # ⚑ Warum die Pruefung bis zur Lizenzdatei geht
///
/// Der Wert im Katalog ist von Hand geschrieben. Eine Pruefung, die nur
/// nachsieht, **dass** dort etwas steht, faengt den Tippfehler nicht
/// und den Lizenzwechsel schon gar nicht. Sie haelt ihn deshalb gegen
/// `LICENSE.md`, also gegen die Quelle. **Dieselbe Klasse wie Fund 271:
/// ein Wert, den jemand abgeschrieben hat, ist so lange keiner, wie ihn
/// niemand gegen sein Original haelt.**
#[test]
fn jede_lizenz_steht_bei_ihrer_sache() {
    let wurzel = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Wurzel des Repositoriums");

    // 1. Das Fenster zeigt keine Lizenz ohne ihre Sache.
    let js = lies("app.js");
    assert!(
        !js.contains("m.lizenz,") && !js.contains("m.lizenz "),
        "das Fenster zeigt eine Lizenz, ohne zu sagen, wofuer sie gilt"
    );
    for feld in ["m.lizenz_gewichte", "m.lizenz_artefakt"] {
        assert!(js.contains(feld), "{feld} fehlt in der Artefaktliste");
    }

    // 2. Der Katalog fuehrt beide, und zwar bei jedem Eintrag.
    let katalog = std::fs::read_to_string(wurzel.join("INTEGER_LLM/models/KATALOG.json"))
        .expect("KATALOG.json");
    let eintraege = katalog.matches("\"anzeigename\":").count();
    assert!(eintraege >= 4, "zu wenige Katalogeintraege: {eintraege}");
    for feld in ["lizenz_gewichte", "lizenz_artefakt"] {
        assert_eq!(
            katalog.matches(&format!("\"{feld}\":")).count(),
            eintraege,
            "{feld} fehlt bei mindestens einem Eintrag"
        );
    }

    // 3. Und die Artefaktlizenz ist die dieses Repositoriums, gehalten
    //    gegen die Lizenzdatei selbst.
    let lizenzdatei = std::fs::read_to_string(wurzel.join("LICENSE.md")).expect("LICENSE.md");
    let genannt = katalog
        .split("\"lizenz_artefakt\": \"")
        .skip(1)
        .filter_map(|s| s.split_once('"').map(|(w, _)| w.to_string()))
        .collect::<BTreeSet<_>>();
    assert_eq!(genannt.len(), 1, "der Katalog nennt mehrere Artefaktlizenzen: {genannt:?}");
    let name = genannt.iter().next().expect("eine Artefaktlizenz");
    assert!(
        lizenzdatei.contains(name),
        "{name} steht so nicht in LICENSE.md"
    );
}

/// **Ein Schalter ist ein Schieber und kein Kaestchen.**
///
/// ⚑ Festlegung des Projektinhabers am 2026-09-10. Geprueft wird nicht
/// das Aussehen, sondern dass es **weiter ein `input[type=checkbox]`
/// ist**: Ein nachgebauter Schieber aus zwei `div` verliert den
/// Tastaturfokus, die Leertaste, die Ansage der Vorlesehilfe und den
/// Zustand, und alles davon muesste einzeln wiederhergestellt werden.
#[test]
fn ein_schalter_ist_ein_schieber() {
    let js = lies("app.js");
    assert!(
        js.contains("element.type = \"checkbox\";"),
        "der Schalter ist kein Ankreuzfeld mehr, damit ist er auch keins fuer die Tastatur"
    );
    let stil = ohne_kommentare(&lies("stil.css"));
    for stueck in [
        "input[type=\"checkbox\"] {",
        "appearance: none;",
        "input[type=\"checkbox\"]::after {",
        "input[type=\"checkbox\"]:checked::after {",
    ] {
        assert!(stil.contains(stueck), "dem Schieber fehlt `{stueck}`");
    }
}

/// **Die Leiste faehrt senkrecht und nicht waagerecht.**
///
/// ⛑ Gemeldet vom Projektinhaber am 2026-09-10. Ein Titel, der breiter
/// ist als die Leiste, machte sie breiter, statt gekuerzt zu werden.
/// **Beides gehoert zusammen:** Ohne die Kuerzung waere die Sperre nur
/// ein Abschneiden, ohne die Sperre die Kuerzung wirkungslos.
#[test]
fn die_leiste_faehrt_nur_senkrecht() {
    let stil = ohne_kommentare(&lies("stil.css"));
    let mitte = stil
        .split_once(".leistenmitte {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect(".leistenmitte");
    assert!(mitte.contains("overflow-y: auto"), "die Leiste faehrt gar nicht");
    assert!(
        mitte.contains("overflow-x: hidden"),
        "die Leiste faehrt auch waagerecht"
    );
    let titel = stil
        .split_once(".chat .titel {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect(".chat .titel");
    for stueck in ["text-overflow: ellipsis", "white-space: nowrap", "min-width: 0"] {
        assert!(titel.contains(stueck), "dem Titel fehlt `{stueck}`");
    }
}

/// **Die Liste zeigt einen Modus, und der Zeigetext nennt ihn nicht.**
///
/// ⛑ Beides am 2026-09-10 vom Projektinhaber gemeldet, und beides
/// haengt zusammen: Die Klammer `(Agent)` im Zeigetext war die einzige
/// Auskunft darueber, zu welchem Modus eine Zeile gehoert. Wird nach
/// Modus gefiltert, ist sie ueberfluessig; **wird sie entfernt, ohne zu
/// filtern, ist die Liste nicht mehr zu lesen.**
#[test]
fn die_liste_zeigt_nur_den_gewaehlten_modus() {
    let js = lies("app.js");
    assert!(
        js.contains("gespraeche.filter((g) => g.modus === modus_jetzt())"),
        "die Liste zeigt weiter alle Modi"
    );
    assert!(
        !js.contains("auf.title = `${g.titel} ("),
        "der Zeigetext nennt weiter den Modus"
    );
    assert!(js.contains("auf.title = g.titel;"), "der Zeigetext fehlt");
}

/// **Im Agentenmodus heissen sie Prozesse.**
///
/// ⚑ Festlegung des Projektinhabers am 2026-09-10, und sie hat einen
/// sachlichen Grund: Im Chat traegt eine Zeile einen Verlauf, beim
/// Agenten steht jeder Auftrag fuer sich, mit eigenem Schrittbudget und
/// eigener Belegkette.
///
/// ⚑ **Geprueft wird die Tabelle und nicht die einzelne Zeile.** Ein
/// Wort, das an vier Stellen von Hand steht, ist Fund 271; es steht an
/// genau einer, und alle vier Stellen holen es dort.
#[test]
fn im_agentenmodus_heissen_sie_prozesse() {
    let js = lies("app.js");
    for stueck in [
        "\"wort.agent.eines\": \"Prozess\"",
        "\"wort.agent.viele\": \"Prozesse\"",
        "\"wort.agent.eines\": \"Process\"",
        "kopf.textContent = wort().viele;",
        "neu.textContent = wort().neu;",
        "titel: wort(art).frisch,",
    ] {
        assert!(js.contains(stueck), "der Wortwahl fehlt `{stueck}`");
    }
}

/// **Klappt die Leiste zu, geht die Marke nach oben.**
///
/// ⚑ Festlegung des Projektinhabers am 2026-09-10. Die Marke stand nur
/// in der Leiste; klappte sie weg, trug das Fenster nirgends mehr
/// seinen Namen.
///
/// ⛑ **Und sie wird geklont, nicht abgeschrieben.** Die Spirale ist
/// gerechnet, 72 Pfade; eine zweite Abschrift im HTML waere ein zweiter
/// Ort, an dem die naechste Aenderung ankommen muesste.
#[test]
fn die_marke_wechselt_mit_der_leiste_den_ort() {
    let html = lies("index.html");
    let js = lies("app.js");
    assert!(
        html.contains("id=\"kopfmarke\""),
        "im Kopf ist kein Platz fuer die Marke"
    );
    assert_eq!(
        html.matches("class=\"spirale\"").count(),
        1,
        "die Spirale steht mehr als einmal im HTML; sie gehoert geklont"
    );
    assert!(
        js.contains("kopfmarke_zeigen(zu);"),
        "die Marke haengt nicht am Zustand der Leiste"
    );
    for klasse in ["kommt", "geht"] {
        assert!(
            js.contains(&format!("classList.add(\"{klasse}\")")),
            "das Stoerbild `{klasse}` wird nie gesetzt"
        );
    }
    let stil = ohne_kommentare(&lies("stil.css"));
    assert!(
        stil.contains("@keyframes marke-kommt") && stil.contains("@keyframes marke-geht"),
        "kommen und gehen tragen nicht zwei verschiedene Stoerbilder"
    );
}

/// **Das Fenster laesst sich nicht kleiner ziehen als seine Bedienung.**
///
/// ⚑ Festlegung des Projektinhabers am 2026-09-10: Gespraechsfenster
/// und Zahnrad bleiben sichtbar. Ohne Untergrenze liess sich das
/// Fenster auf wenige Zentimeter ziehen, und dann stand dort eine
/// Kopfleiste und sonst nichts.
#[test]
fn das_fenster_hat_eine_untergrenze() {
    let konf = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tauri.conf.json"
    ))
    .expect("tauri.conf.json");
    for feld in ["minWidth", "minHeight"] {
        assert!(konf.contains(feld), "dem Fenster fehlt `{feld}`");
    }
    let zahl = |feld: &str| -> u32 {
        konf.split_once(&format!("\"{feld}\": "))
            .and_then(|(_, r)| r.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|z| z.parse().ok())
            .unwrap_or(0)
    };
    // ⛑ **Dreimal wurde diese Zahl zu klein geraten** (2026-09-10,
    // jedes Mal vom Projektinhaber gemeldet), und beim dritten Mal war
    // klar, warum: **Der Aufbau ist in `rem` bemessen, die Zahl in
    // Pixeln.** Leiste 16 rem, Knoepfe 2 rem, Polster 1 rem; wer die
    // Systemschrift groesser stellt, bekommt all das groesser und die
    // Mindestbreite nicht. Eine Rechnung ueber zwei Einheiten stimmt
    // fuer genau eine Schriftgroesse.
    //
    // ⚑ **Die Zusage haengt deshalb nicht mehr an der Zahl.** Leiste
    // und Polster wachsen mit dem Fenster, und beide Kopfpolster
    // kommen aus **einer** Formel: Was links vor dem Leistensymbol
    // steht, steht rechts hinter dem Zahnrad, bei jeder Breite und
    // jeder Schriftgroesse. Die Zahl unten ist seither eine
    // Bequemlichkeitsgrenze und keine Zusage.
    assert!(zahl("minWidth") >= 800, "unter dieser Breite wird die Bedienung eng");
    assert!(zahl("minHeight") >= 500, "zu niedrig: die Eingabe fiele weg");

    let stil = ohne_kommentare(&lies("stil.css"));
    let kopf = stil
        .split_once("\nheader {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect("header");

    // ⚑ **Beide Kopfknoepfe sind an ihre Ecke geheftet, mit
    // demselben Wert.**
    //
    // ⛑ **Dreimal gemeldet, dreimal anders repariert** (2026-09-10):
    // erst ein groesseres `minWidth`, dann `minmax(0, 1fr)` auf der
    // mittleren Spalte, dann ein konstantes Polster. Keines hat es
    // geheilt, denn **in einem Raster haengt die Lage jeder Spalte an
    // allen anderen**, und die Marke in der Mitte erscheint genau
    // dann, wenn die Leiste zugeht.
    //
    // ⚑ **Geheftet statt eingeordnet**, und deshalb prueft diese
    // Stelle jetzt eine Zusage statt einer Rasterangabe: Beide liegen
    // absolut, und der Abstand zur Kante ist derselbe Wert.
    // ⚑ **Vier Abstaende aus einem Wert, und eine feste Hoehe.**
    //
    // ⛑ **Dreimal am Polster geschraubt, dreimal daneben**
    // (2026-09-10). Zuletzt: `min-height` plus Polsterung, und weil
    // `box-sizing: border-box` gilt, war die Hoehe **das Groessere von
    // beidem**: mit der Marke in der Mitte ihr Inhalt plus Polster,
    // ohne sie der Mindestwert. Die Marke erscheint aber genau dann,
    // wenn die Leiste zugeht, und deshalb stauchte sich die Kopfleiste
    // beim Bedienen.
    //
    // ⚑ **Eine Hoehe, die am Inhalt haengt, aendert sich mit dem
    // Inhalt.** Sie haengt jetzt an zwei Variablen, und die vier
    // Abstaende der Knoepfe kommen aus einer davon: **Was aus
    // derselben Zahl kommt, kann nicht auseinanderlaufen.**
    assert!(
        kopf.contains("height: calc(var(--kopf-knopf) + 2 * var(--kopf-polster))"),
        "die Kopfhoehe haengt wieder am Inhalt"
    );
    assert!(
        !kopf.contains("min-height"),
        "der Kopf hat wieder eine Mindesthoehe, die der Inhalt ueberbieten kann"
    );
    let polster = kopf
        .split_once("padding:")
        .and_then(|(_, r)| r.split_once(';'))
        .map(|(w, _)| w.trim().to_string())
        .expect("der Kopf hat kein Polster");
    assert_eq!(
        polster, "0",
        "der Kopf traegt wieder ein eigenes Polster neben dem der Knoepfe"
    );

    // ⚑ **Senkrecht und waagerecht aus derselben Variablen.** Steht
    // die Hoehe auf Knopf plus zweimal Polster und sitzt der Knopf
    // mittig, ist der Abstand nach oben und unten **gerechnet**
    // dasselbe Polster wie links und rechts. Zwei Zahlen koennten
    // auseinanderlaufen, eine nicht.
    for zeile in ["header > #leiste-schalten { ", "header > .kopfrechts { "] {
        let rumpf = stil
            .split_once(zeile)
            .and_then(|(_, r)| r.split_once('}'))
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| panic!("{zeile} fehlt"));
        assert!(
            rumpf.contains("var(--kopf-polster)"),
            "{zeile} nennt eine eigene Zahl statt der gemeinsamen: {rumpf}"
        );
    }
    let gemeinsam = stil
        .split_once("header > #leiste-schalten,\nheader > .kopfrechts {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect("die gemeinsame Regel der Kopfknoepfe");
    for regel in ["position: absolute", "top: 50%", "translateY(-50%)"] {
        assert!(gemeinsam.contains(regel), "den Kopfknoepfen fehlt `{regel}`");
    }

    // ⚠️ Und das Polster ist gross genug, dass die Leiste nicht
    // gedrueckt wirkt; ebenfalls gemeldet.
    let wert = stil
        .split_once("--kopf-polster:")
        .and_then(|(_, r)| r.split_once(';'))
        .map(|(w, _)| w.trim().trim_end_matches("rem").to_string())
        .expect("--kopf-polster");
    let rem: f32 = wert.parse().unwrap_or_else(|_| panic!("`{wert}` ist kein rem-Wert"));
    assert!(rem >= 0.8, "der Kopf ist mit {rem}rem Polster zu schmal");

    // ⚑ **Und die Marke im Kopf traegt keinen Rahmen.** Bei 1,15 rem
    // sind Rahmen und Kreis zwei Striche dicht nebeneinander, und der
    // Rahmen gewinnt, weil er gerade ist. In der Seitenleiste bleibt
    // er, dort hat die Marke Platz.
    assert!(
        stil.contains(".kopfmarke .geruest { display: none; }"),
        "die kleine Marke traegt noch ihren Rahmen"
    );

    // ⚑ **Ein festes Polster, links wie rechts.** `padding: .6rem 1rem`
    // setzt denselben Abstand auf beide Seiten, und zwar einen
    // konstanten.
    //
    // ⛑ **Symmetrisch genuegt nicht, es muss stabil sein.** Ein kurz
    // eingesetztes `clamp(.5rem, 1.6vw, 1rem)` war auf beiden Seiten
    // gleich und wanderte trotzdem mit der Fensterbreite; gemeldet vom
    // Projektinhaber, weil es beim Auf- und Zuklappen der Leiste wie
    // ein Sprung aussah. **Ein Abstand, der sich beim Bedienen
    // aendert, ist keiner, auf den man sich verlaesst.**
    // ⚑ Und die Leiste waechst mit, statt eine feste Breite gegen ein
    // schmales Fenster zu behaupten.
    let leiste = stil
        .split_once("\n#seitenleiste {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect("#seitenleiste");
    assert!(
        leiste.contains("width: clamp("),
        "die Seitenleiste hat wieder eine feste Breite"
    );

    // ⛑ **Und die Huelle laesst sie wirklich schrumpfen.** Der
    // selbsttaetige Mindestwert einer Rasterspalte ist der
    // Mindestinhalt ihres Kindes, nicht null.
    let huelle = stil
        .split_once("\n#huelle {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(rumpf, _)| rumpf.to_string())
        .expect("#huelle");
    // ⛑ **Die Leiste gibt nach, nicht der Inhalt.** Gemeldet vom
    // Projektinhaber: Bei zugeklappter Leiste stimmt alles, bei
    // offener wird rechts abgeschnitten. Wird der Platz knapp,
    // verliert im Raster zuerst die **flexible** Spalte, und das war
    // das Hauptfenster. **Was abgeschnitten wird, soll das sein, was
    // sich zuklappen laesst.**
    assert!(
        huelle.contains("grid-template-columns: minmax(0, auto) minmax(22rem, 1fr)"),
        "die Leiste nimmt dem Hauptfenster wieder Platz weg"
    );
    // ⚠️ **Und die Schranke steht in `rem`**, wie alles darin. Eine
    // Schranke in Pixeln ueber einem Aufbau in `rem` war der Fehler,
    // den diese Stelle dreimal wiederholt hat.
    assert!(
        !huelle.contains("minmax(24px") && !huelle.contains("minmax(352px"),
        "die Schranke steht wieder in Pixeln"
    );

    // ⚑ **Und die beiden Fussnoten sind kleiner als das, was sie
    // erklaeren** (Festlegung des Projektinhabers, 2026-09-10): der
    // Pfad unter der Modellwahl und die Zeile unter der Eingabe. Beide
    // beantworten eine Frage fuer den Zweifelsfall und keine, die sich
    // staendig stellt.
    for (was, wo) in [("der Pfad unter der Modellwahl", ".leistenfuss .pfad {"),
                      ("die Zeile unter der Eingabe", "#hinweiszeile {")] {
        let rumpf = stil
            .split_once(wo)
            .and_then(|(_, r)| r.split_once('}'))
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| panic!("{wo} fehlt"));
        let groesse = rumpf
            .split_once("font-size: .")
            .and_then(|(_, r)| r.split_once("rem"))
            .and_then(|(z, _)| z.parse::<u32>().ok())
            .unwrap_or_else(|| panic!("{was} nennt keine Schriftgroesse"));
        assert!(
            groesse <= 62,
            "{was} steht mit 0,{groesse} rem noch zu gross da"
        );
    }
}

/// Alle Schluessel eines Sprachabschnitts der Tabelle in `app.js`.
fn sprachschluessel(js: &str, sprache: &str) -> BTreeSet<String> {
    let rumpf = js
        .split_once(&format!("\n  {sprache}: {{\n"))
        .and_then(|(_, r)| r.split_once("\n  },\n"))
        .map(|(k, _)| k)
        .unwrap_or_else(|| panic!("kein Abschnitt `{sprache}` in TEXTE"));
    let mut aus = BTreeSet::new();
    for zeile in rumpf.lines() {
        let z = zeile.trim();
        let Some(rest) = z.strip_prefix('"') else { continue };
        let Some((name, danach)) = rest.split_once('"') else { continue };
        if danach.trim_start().starts_with(':') {
            aus.insert(name.to_string());
        }
    }
    aus
}

/// **Beide Sprachen kennen dieselben Saetze.**
///
/// # ⛑ Warum das eine Pruefung braucht
///
/// Ein fehlender Schluessel ist in JavaScript kein Fehler, sondern
/// `undefined`. `t()` faellt deshalb auf Deutsch zurueck, und **das ist
/// die richtige Entscheidung und zugleich der Grund, warum niemand es
/// merkt**: Ein englisches Fenster mit drei deutschen Saetzen darin
/// sieht aus wie ein Fenster mit drei Saetzen, die jemand vergessen
/// hat, und genau das ist es auch.
///
/// ⚑ **Dieselbe Klasse wie Fund 280**, nur eine Ebene hoeher: Dort
/// fehlten Felder in einer Zuordnung, hier Saetze in einer Sprache.
#[test]
fn beide_sprachen_kennen_dieselben_saetze() {
    let js = lies("app.js");
    let de = sprachschluessel(&js, "de");
    let en = sprachschluessel(&js, "en");
    assert!(de.len() > 40, "nur {} Saetze; liest die Pruefung die Tabelle noch?", de.len());

    let fehlt_en: Vec<_> = de.difference(&en).collect();
    let fehlt_de: Vec<_> = en.difference(&de).collect();
    assert!(fehlt_en.is_empty(), "auf Englisch fehlen: {fehlt_en:?}");
    assert!(fehlt_de.is_empty(), "auf Deutsch fehlen: {fehlt_de:?}");
}

/// **Jede Beschriftung im HTML hat einen Satz in beiden Sprachen.**
///
/// ⚑ **Und umgekehrt braucht es die Pruefung nicht:** Ein Satz, den
/// niemand benutzt, kostet nichts. Eine Beschriftung ohne Satz kostet
/// ein leeres Element, denn `t()` gibt fuer einen unbekannten
/// Schluessel den leeren Text zurueck und nicht den Schluessel: Ein
/// Fenster, in dem „leiste.modus" steht, ist kaputt.
#[test]
fn jede_beschriftung_hat_ihren_satz() {
    let js = lies("app.js");
    let html = lies("index.html");
    let de = sprachschluessel(&js, "de");

    let mut gefunden = 0;
    for marke in ["data-t=\"", "data-t-marke=\"", "data-t-platz=\""] {
        let mut rest = html.as_str();
        while let Some(a) = rest.find(marke) {
            rest = &rest[a + marke.len()..];
            let Some(e) = rest.find('"') else { break };
            let schluessel = &rest[..e];
            assert!(
                de.contains(schluessel),
                "`{schluessel}` steht im HTML und in keiner Sprachtabelle"
            );
            gefunden += 1;
        }
    }
    assert!(gefunden >= 15, "nur {gefunden} Beschriftungen gefunden");
}

/// **Was ein Programm vergleicht, wird nie uebersetzt.**
///
/// ⚑ Feldnamen, Pfade und die Kennung des Netzmodells stehen in jeder
/// Sprache gleich. **Ein uebersetzter Schluessel ist kein Schluessel
/// mehr**, und der Fehler faellt erst auf, wenn jemand die Sprache
/// umstellt und danach nichts mehr findet.
#[test]
fn kennungen_bleiben_in_jeder_sprache_gleich() {
    let js = lies("app.js");
    for sprache in ["de", "en"] {
        let schluessel = sprachschluessel(&js, sprache);
        for f in myl_client::einstellungen::FELDER {
            assert!(
                !schluessel.contains(f.name),
                "`{}` ist ein Feldname und steht als Satz in der Sprachtabelle `{sprache}`",
                f.name
            );
        }
    }
    // Und die Kennung des Netzmodells steht genau einmal, als Konstante.
    //
    // ⛑ **Seit dem 2026-09-11 ein Praefix**, weil jedes Modell auch im
    // Netz waehlbar ist und der Wert deshalb `netz:<Artefaktname>`
    // lautet. Die geprueften Eigenschaft ist dieselbe geblieben: eine
    // Stelle, nicht zwei.
    assert_eq!(
        js.matches("const NETZMODELL = \"netz:\";").count(),
        1,
        "die Kennung des Netzmodells steht nicht mehr an genau einer Stelle"
    );
}

/// **Die Sprache ist ein Feld wie jedes andere, mit einer Auswahl.**
///
/// ⚑ **Die Sprachnamen stehen in ihrer eigenen Sprache.** Wer die
/// Oberflaeche gerade nicht versteht, findet seine Sprache nur so
/// wieder; „German/English" auf Englisch hilft dem nicht, der Deutsch
/// sucht.
#[test]
fn die_sprache_steht_in_den_einstellungen() {
    let feld = myl_client::einstellungen::FELDER
        .iter()
        .find(|f| f.name == "oberflaeche.sprache")
        .expect("kein Feld fuer die Sprache");
    assert_eq!(feld.art, myl_client::einstellungen::Feldart::Auswahl);
    let werte: Vec<_> = feld.wahl.iter().map(|w| w.wert).collect();
    assert_eq!(werte, vec!["de", "en"]);
    assert_eq!(feld.wahl[0].titel, "Deutsch");
    assert_eq!(feld.wahl[1].titel, "English");

    // Und das Fenster kann sie zeichnen und uebernimmt sie sofort.
    let js = lies("app.js");
    assert!(js.contains("if (f.art === \"Auswahl\")"), "das Fenster kennt keine Auswahl");
    assert!(
        js.contains("await sprache_setzen(neu);"),
        "die Sprache wirkt erst beim naechsten Start"
    );
}

/// **Ein Eintrag entsteht auf zwei Wege und sonst nie.**
///
/// # ⛑ Gemeldet vom Projektinhaber am 2026-09-10
///
/// Der Modus hing am geoeffneten Gespraech. Daraus folgte, dass ein
/// **Moduswechsel etwas anlegen musste**, um den Modus ueberhaupt
/// festhalten zu koennen: Wer zwischen Chat und Agent hin und her
/// klickte, hinterliess bei jedem Klick ein leeres Gespraech, und die
/// Liste des anderen Modus zeigte es beim naechsten Wechsel mit an.
///
/// ⚑ **Ein Modus ist eine Ansicht, und eine Ansicht legt nichts an.**
/// Angelegt wird auf Knopfdruck und beim Abschicken in einem leeren
/// Feld, an genau zwei Stellen.
#[test]
fn ein_eintrag_entsteht_nur_auf_zwei_wege() {
    let js = lies("app.js");

    // ⚑ Der Modus ist ein eigener Zustand und nicht abgeleitet.
    assert!(
        js.contains("const modus_jetzt = () => modus;"),
        "der Modus haengt wieder am geoeffneten Eintrag"
    );

    // ⚑ **Gezaehlt wird der Aufruf und nicht das Wort.** Drei Stellen
    // duerfen anlegen: der Knopf, das Abschicken, und die Zeile im
    // Start, die gar keine mehr ist. Kommt eine vierte dazu, faellt
    // diese Pruefung, und das ist ihr Zweck.
    let anlagen: Vec<&str> = js
        .lines()
        .map(str::trim)
        .filter(|z| z.contains("neues_gespraech(") && !z.starts_with("//") && !z.starts_with("function"))
        .collect();
    assert_eq!(
        anlagen.len(),
        2,
        "es wird an {} Stellen angelegt, erlaubt sind Knopf und Abschicken:\n{anlagen:#?}",
        anlagen.len()
    );

    // Und der Start gehoert nicht dazu.
    assert!(
        js.contains("offen = gespraeche[0] || null;"),
        "der Start legt wieder etwas an"
    );
    // Der Moduswechsel auch nicht.
    assert!(
        js.contains("if (offen && offen.modus !== modus) offen = null;"),
        "der Moduswechsel legt an oder zieht einen fremden Eintrag mit"
    );
}

/// **Jeder Beschreibungssatz der Freigabemaske spricht die eingestellte
/// Sprache.**
///
/// # ⛑ Gemeldet vom Projektinhaber am 2026-09-10
///
/// Die Sprache wirkte auf die Feldbeschriftungen und nicht auf die
/// Regler: `hardware::regler` nahm `FELDER` roh, also immer auf
/// Deutsch, und die beiden laengsten Saetze der ganzen Seite,
/// `RECHENWERK_GESPERRT` und die Beschreibung eines Rechenwerks,
/// standen ueberhaupt nur auf Deutsch da.
///
/// ⚑ **Uebersetzt wird, was ein Mensch liest, und das gilt besonders
/// fuer den Satz, der erklaert, warum etwas nicht geht.**
#[test]
fn auch_die_regler_sprechen_die_eingestellte_sprache() {
    let e = myl_client::Einstellungen::default();
    let hw = myl_client::hardware::Hardware::erheben(std::path::Path::new("."));

    let mut deutsch = e.clone();
    deutsch.oberflaeche.sprache = myl_client::einstellungen::Sprache::De;
    let mut englisch = e;
    englisch.oberflaeche.sprache = myl_client::einstellungen::Sprache::En;

    let de = hw.regler(&deutsch);
    let en = hw.regler(&englisch);
    assert_eq!(de.len(), en.len(), "verschieden viele Regler je Sprache");
    assert!(!de.is_empty(), "kein einziger Regler; misst die Pruefung noch etwas?");

    for (d, e) in de.iter().zip(en.iter()) {
        assert_eq!(d.name, e.name, "der Feldname wurde uebersetzt");
        assert_ne!(
            d.hinweis, e.hinweis,
            "`{}` traegt in beiden Sprachen denselben Satz",
            d.name
        );
        match (&d.sperrgrund, &e.sperrgrund) {
            (Some(a), Some(b)) => assert_ne!(a, b, "`{}` sperrt in beiden Sprachen gleich", d.name),
            (None, None) => {}
            _ => panic!("`{}` ist nur in einer Sprache gesperrt", d.name),
        }
    }
}

/// **Die Sprache steht zuoberst, die Updates gleich darunter.**
///
/// ⚑ Festlegung des Projektinhabers am 2026-09-10, und sie hat einen
/// Grund: Die Sprache beschriftet alles, was darunter kommt. **Wer die
/// Seite in einer Sprache oeffnet, die er nicht liest, soll den
/// Schalter finden, ohne bis ans Ende zu suchen.**
#[test]
fn die_sprache_steht_zuoberst() {
    assert_eq!(
        myl_client::einstellungen::FELDER[0].name,
        "oberflaeche.sprache",
        "die Sprache ist nicht das erste Feld"
    );

    let html = lies("index.html");
    let wo = |k: &str| html.find(k).unwrap_or_else(|| panic!("{k} steht nicht im HTML"));

    // ⚑ **Die Reihenfolge der ganzen Seite, an einer Stelle geprueft**
    // (Festlegung des Projektinhabers, 2026-09-11): Sprache, dann die
    // Aktualisierung, dann die Modelle, dann was dieser Rechner
    // hergibt, und zuletzt die Feinheiten von Modell und Agent.
    let folge = [
        ("id=\"felder-oberflaeche\"", "die Sprache"),
        ("id=\"aktualisierung\"", "die Aktualisierung"),
        ("id=\"modellbau\"", "die Modelle"),
        ("id=\"felder-grenzen\"", "die Grenzen dieses Rechners"),
        ("id=\"freigabe\"", "die Freigaben"),
        ("id=\"felder-rest\"", "Modell und Agent"),
    ];
    for paar in folge.windows(2) {
        assert!(
            wo(paar[0].0) < wo(paar[1].0),
            "{} steht nicht vor {}",
            paar[0].1,
            paar[1].1
        );
    }
}

/// **Der Einspielknopf erscheint erst, wenn es etwas einzuspielen
/// gibt.**
///
/// ⛑ Vorher stand er gesperrt da. **Ein gesperrter Knopf beantwortet
/// die Frage „gibt es Updates" mit einem Bedienelement**, und der Grund
/// steckte in seinem Zeigetext, wo ihn nur findet, wer mit der Maus
/// darauf wartet. Die Zeile darueber beantwortet dieselbe Frage mit
/// einem Satz.
#[test]
fn der_einspielknopf_erscheint_erst_bei_bedarf() {
    let html = lies("index.html");
    let js = lies("app.js");
    assert!(
        html.contains("id=\"akt-einspielen\" data-t=\"akt.einspielen\" hidden"),
        "der Knopf steht beim Oeffnen der Seite schon da"
    );
    assert!(js.contains("knopf.hidden = !lohnt;"), "der Knopf wird nur gesperrt statt verborgen");
    // Und die deutsche Beschriftung ist die vom Projektinhaber gewaehlte.
    for satz in ["\"akt.pruefen\": \"Nach Updates suchen\"", "\"akt.einspielen\": \"Updates installieren\""] {
        assert!(js.contains(satz), "der Wortlaut fehlt: {satz}");
    }
}

/// **Gekuerzt wird die Anzeige, nicht die Sache.**
///
/// # ⛑ Gemeldet vom Projektinhaber am 2026-09-10
///
/// `titel_aus` kuerzte auf vierzig Zeichen und haengte `...` an, und
/// **das Ergebnis war der gespeicherte Titel**. Die Zeile in der Leiste
/// kuerzte danach ein zweites Mal, und der Zeigetext beim Ueberfahren
/// zeigte genau dieselbe gekuerzte Zeichenkette: **Das Lange war
/// nirgends mehr zu holen.**
///
/// ⚑ **Kuerzen ist Anzeige und gehoert ins Stilblatt.** Die Zeile
/// bekommt eine Ellipse aus dem CSS, der Zeigetext den ganzen Titel.
/// Eine Schranke bleibt, aber weit oben: Wer einen Absatz einwirft,
/// soll keinen Absatz in der Ablage haben.
#[test]
fn gekuerzt_wird_die_anzeige_und_nicht_die_sache() {
    let js = lies("app.js");
    let stil = ohne_kommentare(&lies("stil.css"));

    // 1. Der gespeicherte Titel ist nicht auf Leistenbreite gestutzt.
    let rumpf = js
        .split_once("function titel_aus(text) {")
        .and_then(|(_, r)| r.split_once("\n}"))
        .map(|(k, _)| k.to_string())
        .expect("titel_aus");
    assert!(
        !rumpf.contains("> 40"),
        "der Titel wird beim Speichern auf Leistenbreite gekuerzt"
    );
    assert!(rumpf.contains("> 200"), "der Titel hat gar keine Schranke mehr");

    // 2. Die Zeile kuerzt, und zwar im Stilblatt.
    let titel = stil
        .split_once(".chat .titel {")
        .and_then(|(_, r)| r.split_once('}'))
        .map(|(k, _)| k.to_string())
        .expect(".chat .titel");
    for regel in ["text-overflow: ellipsis", "white-space: nowrap", "overflow: hidden"] {
        assert!(titel.contains(regel), "der Zeile fehlt `{regel}`");
    }

    // 3. Und der Zeigetext gibt den ganzen Titel her.
    assert!(js.contains("auf.title = g.titel;"), "der Zeigetext zeigt nicht den ganzen Titel");

    // ⚑ **Meldungen kuerzen weiter, und das ist richtig:** Sie haben
    // keine Leiste, die fuer sie kuerzt, und keinen Zeigetext, der das
    // Lange nachreichte.
    assert!(js.contains("const kurz_titel = (text) =>"), "die kurze Form fehlt");
}

/// **Kein Stilblatt mit Bruchstuecken darin.**
///
/// # ⛑ Der Fund, der mehrere Meldungen eines Abends erklaert
///
/// Im Stilblatt stand ein verwaister Block: eine schliessende Klammer,
/// ein Backtick, und danach das Ende eines Kommentars samt zwei Dutzend
/// Angaben ohne Regel darum. Er war **eingecheckt** und stammte aus
/// einem halb zurueckgenommenen Umbau.
///
/// ⚠️ **Ein Browser wirft das nicht weg, er verschluckt das Naechste.**
/// Nach dem Backtick sucht der Aufloeser einen Selektor und liest alles
/// bis zur naechsten `{` als solchen. Die naechste `{` war die von
/// `.chat`, und damit war **die ganze Regel fuer eine Gespraechszeile
/// weg**: kein `display: flex`, keine Breite, und deshalb auch keine
/// Ellipse am Titel, denn `text-overflow` braucht eine Schranke.
///
/// ⚑ **Drei Meldungen desselben Abends hingen daran**: Titel ohne
/// Kuerzung, eine Leiste, die dem Hauptfenster Platz nahm, und ein
/// Zahnrad, das dabei aus der Ecke rutschte.
///
/// **Diese Pruefung misst die Form und nicht den Inhalt.** Sie sagt
/// nicht, welche Regel richtig ist; sie sagt, dass die Datei eine ist.
#[test]
fn das_stilblatt_ist_ganz() {
    let roh = lies("stil.css");

    // 1. Jeder Kommentar wird geschlossen, und keiner schliesst zweimal.
    assert_eq!(
        roh.matches("/*").count(),
        roh.matches("*/").count(),
        "ein Kommentar wird nicht geschlossen, oder einer schliesst zweimal"
    );

    // 2. Die Klammern gehen auf und wieder zu, in dieser Reihenfolge.
    let ohne = ohne_kommentare(&roh);
    let mut tiefe: i32 = 0;
    for (n, zeile) in ohne.lines().enumerate() {
        for c in zeile.chars() {
            match c {
                '{' => tiefe += 1,
                '}' => {
                    tiefe -= 1;
                    assert!(tiefe >= 0, "Zeile {}: eine Klammer zu viel", n + 1);
                }
                _ => {}
            }
        }
    }
    assert_eq!(tiefe, 0, "am Ende bleibt eine Klammer offen");

    // 3. ⚑ **Und kein Backtick ausserhalb eines Kommentars.** Er ist
    //    in CSS kein gueltiges Zeichen; genau einer hat den Fund
    //    verursacht, und er stand seit einem Commit da.
    assert!(
        !ohne.contains('`'),
        "im Stilblatt steht ein Backtick ausserhalb eines Kommentars"
    );
}

/// **Das Skript setzt keine Stilangaben am Element.**
///
/// # ⛑ Fund 305, gemeldet vom Projektinhaber am 2026-09-10
///
/// Hinter jedem Gespraechstitel stand ein rundes Feld. Der Titel ist
/// ein Knopf, und `.blank` schaltet nur die beiden Zierpseudoelemente
/// ab; Glasverlauf, Rundung, Polsterung, Schatten und
/// `backdrop-filter` blieben. Das Skript nahm davon **fuenf Dinge von
/// Hand wieder weg**, mit `element.style.…`, und vergass Rundung und
/// Hintergrundfilter.
///
/// ⚑ **Aussehen gehoert ins Stilblatt.** Ein Skript, das Stilangaben
/// setzt, ist eine zweite Stelle, an der etwas fehlen kann, und sie
/// ist die schlechter geprueefte: Im Stilblatt steht der Rueckbau
/// beieinander und faellt als Luecke auf, im Skript steht er zwischen
/// zwei Ereignisbehandlungen.
///
/// ⚠️ **Die Ausnahme ist Bewegung und Lage**, die aus gemessenen Werten
/// entsteht: wo ein Kontextmenue aufgeht, wie breit ein Balken ist.
/// Das kann kein Stilblatt wissen.
#[test]
fn das_skript_setzt_kein_aussehen() {
    // ⚑ Kommentare heraus, aus demselben Grund wie bei der Wache
    // ueber `innerHTML`: **Eine Pruefung, die Erwaehnung fuer Gebrauch
    // haelt, bestraft das Aufschreiben der Regel.**
    let js: String = lies("app.js")
        .lines()
        .filter(|z| {
            let z = z.trim_start();
            !(z.starts_with("//") || z.starts_with('*') || z.starts_with("/*"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    const AUSSEHEN: [&str; 8] = [
        "style.background",
        "style.border",
        "style.borderRadius",
        "style.boxShadow",
        "style.color",
        "style.padding",
        "style.font",
        "style.textAlign",
    ];
    for eigenschaft in AUSSEHEN {
        assert!(
            !js.contains(eigenschaft),
            "das Skript setzt `{eigenschaft}` am Element; das gehoert ins Stilblatt"
        );
    }
}

/// **Ein berührtes Gespräch wandert in der Leiste nach oben.**
///
/// ⚑ Auftrag des Projektinhabers vom 2026-09-12: Womit man gerade
/// arbeitet, steht oben. Bis dahin stand die Leiste in der Reihenfolge
/// der Anlage, und ein Gespräch, das man seit Wochen führt, rutschte
/// mit jedem neuen weiter nach unten.
///
/// ⚠️ **Geprüft wird der Weg, nicht das Bild.** Ein Test ohne Browser
/// kann die Leiste nicht sehen; er kann aber festhalten, **dass** das
/// Umordnen beim Senden geschieht und **nicht** beim blossen Öffnen.
/// Genau diese beiden Aussagen sind die Entscheidung.
#[test]
fn ein_beruehrtes_gespraech_wandert_nach_oben() {
    let js = lies("app.js");
    assert!(
        js.contains("function nach_oben("),
        "die Funktion zum Umordnen fehlt"
    );
    // Sie versetzt wirklich, statt nur zu sortieren.
    assert!(
        js.contains("gespraeche.splice(i, 1)") && js.contains("gespraeche.unshift(g)"),
        "`nach_oben` ordnet nicht um"
    );

    // **Beim Senden**, und dort steht der Aufruf.
    let senden = js
        .split("async function senden(")
        .nth(1)
        .expect("`senden` fehlt");
    let rumpf = &senden[..senden.len().min(900)];
    assert!(
        rumpf.contains("nach_oben(offen)"),
        "beim Senden wird nicht umgeordnet"
    );
}

/// **Das Umordnen fasst `wann` nicht an.**
///
/// ⛑ `g.wann` steht im Markdown-Export als „Begonnen", also als
/// Aussage über den **Anfang**. Wer es beim Umordnen fortschriebe,
/// machte daraus stillschweigend „zuletzt benutzt", und der Export
/// sagte etwas anderes, als dort steht.
#[test]
fn das_umordnen_faelscht_den_anfangszeitpunkt_nicht() {
    let js = lies("app.js");
    let f = js
        .split("function nach_oben(")
        .nth(1)
        .expect("`nach_oben` fehlt");
    let rumpf = &f[..f.find("\n}").unwrap_or(f.len())];
    assert!(
        !rumpf.contains("wann"),
        "`nach_oben` schreibt `wann` fort: {rumpf}"
    );
    // Und der Export nennt es weiterhin als Anfang.
    assert!(
        js.contains("Begonnen: ${g.wann}"),
        "der Export nennt `wann` nicht mehr als Anfang"
    );
}

