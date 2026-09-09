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

    // ⚑ **Die vier Schrittarten kommen aus einer Vorlage und lassen
    // sich deshalb nicht ablesen.** Damit die Aufzaehlung trotzdem
    // nicht rottet, muss jeder Name **als Schluessel** im Skript
    // vorkommen; genau dort steht die Tabelle, die sie erzeugt.
    for k in ["aufruf", "ergebnis", "unlesbar", "hinweis"] {
        assert!(js.contains(&format!("{k}:")), "`{k}` ist keine Schrittart mehr");
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

/// **Jedes angemeldete Symbol liegt auch da, und die zwei
/// Systemformate sind dabei.**
///
/// ⛑ Bis zum 2026-09-09 lagen nur PNG-Dateien im Verzeichnis. Der
/// Buendler braucht fuer das `.msi` ein `.ico` und fuer das `.dmg` ein
/// `.icns`; was er ohne sie liefert, ist im guenstigen Fall ein
/// Abbruch und im unguenstigen ein Buendel mit dem Platzhalter des
/// Werkzeugs, und das sieht erst der, der es anklickt.
///
/// Erzeugt werden beide von `werkzeuge/symbole.py` aus `icon.png`.
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
