//! **Findet ein Modell eine Einzelheit wieder, die in seinem Kontext
//! nicht mehr steht?**
//!
//! # ⚑ Warum diese Probe gebaut wurde
//!
//! Der Mitschnitt und seine beiden Werkzeuge waren gebaut und
//! **stueckweise** geprueft: Das Verzeichnis zeigt auf die richtigen
//! Zeilen, der Deckel greift, die Sitzungen werden genannt. ⛔️ **Keine
//! dieser Pruefungen sagt, ob ein Modell den Weg von allein geht.** Das
//! ist der Zweck der ganzen Einrichtung, und er war unbelegt. **Ein
//! Werkzeug, von dem niemand weiss, ob ein Modell es findet, ist eine
//! Vermutung mit Quelltext.**
//!
//! # ⚑ Warum Verlauf und Kontext beide erfunden sind
//!
//! Der erste Entwurf liess das Modell **wirklich verdichten** und hoffte
//! darauf, dass die Nadel dabei verschwindet. Das war der falsche
//! Aufbau, und zwei Messungen haben es gezeigt: Das 0,6B fasst nicht
//! zusammen, sondern schreibt den Anfang ab, und das 4B **behaelt** eine
//! auffaellig markierte Einzelheit (3 von 3 Laeufen ungueltig). **Die
//! Verdichtung war damit der Versuchsaufbau und der Messgegenstand
//! zugleich.**
//!
//! ⚑ **Jetzt sind beide Seiten gesetzt:** Der Verlauf ist erfunden und
//! wird als echter Mitschnitt geschrieben; die Zusammenfassung ist
//! ebenfalls erfunden und nennt die Themen, **nicht** die Nadel. ⛔️
//! **Und die Abwesenheit wird zugesichert, nicht gehofft:** Vor jedem
//! Lauf wird der ganze Prompt nach der Kennung durchsucht. Findet sich
//! eine, bricht die Probe ab, denn dann misst sie nichts.
//!
//! # Was ein Lauf tut
//!
//! 1. Schreibt den erfundenen Verlauf als Mitschnitt unter `.AGENT/`.
//! 2. Baut den Kontext: erfundene Zusammenfassung **plus den Verweis,
//!    den der Betrieb selbst anhaengt** (`gespraech::verweis`).
//! 3. ⛔️ Prueft, dass die Kennung im ganzen Prompt nicht vorkommt.
//! 4. Fragt nach der Kennung, mit Werkzeugen an der Hand.
//! 5. ⚑ Fragt dieselbe Frage mit **leerem** Mitschnitt (die Gegenprobe).
//!    Trifft sie, misst diese Probe nicht, was sie zu messen vorgibt.
//!
//! # ⚑ Was gezaehlt wird
//!
//! Der **Treffer** (die Kennung steht in der Antwort), der **Weg**
//! (welche Werkzeuge gerufen wurden) und der **Platz** (Kontext am Ende
//! gegen den ganzen ungekuerzten Verlauf). ⚠️ **Nicht gezaehlt wird, ob
//! die Antwort schoen ist.**
//!
//! ```text
//! nadelprobe <artefakt> [--laeufe N] [--schritte N] [--token N] [--zeigen]
//!            [--regel streng|mild] [--kiste Base|Advanced] [--anweisung] [--deutsch]
//! ```

use myl_client::{Einstellungen, Modellweg, Nachricht, Oertlichesmodell};

/// Die Stellen im Gespraech, an denen die Nadel liegen kann, als
/// Anteil in Prozent.
///
/// ⚑ **Drei, und nicht eine.** Eine Zusammenfassung behandelt Anfang,
/// Mitte und Ende nicht gleich: Was zuletzt gesagt wurde, ueberlebt sie
/// eher. Eine Probe mit einer einzigen Stelle misst diese eine Stelle.
const STELLEN: [(&str, usize); 3] = [("frueh", 10), ("mitte", 50), ("spaet", 85)];

/// **Die Hausregel, deren Wirkung gemessen wird.**
///
/// # ⚑ Sie zielt auf die zwei beobachteten Fehlschlaege
///
/// Gemessen am 2026-09-17: Das 0,6B **antwortet aus dem Gedaechtnis**,
/// obwohl es nichts weiss (27 von 27 Laeufen ohne einen einzigen
/// Aufruf), und das 30B **beschreibt**, was man tun koennte, statt es
/// zu tun („Sie koennen mit `search_history` suchen … ich liste
/// zunaechst auf"), und setzt danach keinen Aufruf ab.
///
/// ⚑ **Englisch, weil die Ansage englisch ist.** Die amtliche Vorlage
/// ist zeichengleich die des Modells; ein deutscher Absatz dahinter
/// waere ein Sprachwechsel mitten im Systemprompt.
const HAUSREGEL_STRENG: &str = "When the user asks for a detail that is not in the \
conversation above, call search_history with a word from the question before you answer. \
Do not answer from memory, and do not describe what could be done: emit the tool call \
itself.";

/// **Dieselbe Regel mit der Bedingung vorn.**
///
/// ⚑ **Gebaut, nachdem die strenge gemessen war.** Sie erzeugt beim 4B
/// den Aufruf auch dort, wo die Auskunft schon im Kontext steht: neun
/// von neun Umwegen. **Eine Regel, die immer gilt, gilt auch, wenn sie
/// nicht gebraucht wird**, und das kostet je Frage einen Aufruf,
/// Kontext und Zeit. Diese hier nennt beide Faelle und sagt
/// ausdruecklich, wann **nicht** zu rufen ist.
const HAUSREGEL_MILD: &str = "If the answer to the user's question is in the conversation \
above, answer directly and do not call a tool. If it is not, call search_history with a \
word from the question instead of answering from memory. Never describe a tool call in \
prose: emit it.";

/// **Die Kontrollfrage, deren Antwort im Kontext steht.**
///
/// ⚑ **Eine Regel hat einen Preis, und der gehoert gemessen.** „Ruf
/// erst das Werkzeug" kann ein Modell dazu bringen, auch dann zu
/// suchen, wenn die Auskunft vor ihm steht. Diese Frage ist aus der
/// Zusammenfassung heraus zu beantworten, und **ein Werkzeugaufruf ist
/// hier der Fehler**, nicht die Antwort.
const KONTROLLFRAGE: &str =
    "Ueber wie viele Positionen laeuft die Messung des Umsetzungsverlusts?";
/// Was in der Antwort darauf stehen muss.
const KONTROLLANTWORT: &str = "13797";

/// Woran das Verzeichnis in der Zusammenfassung zu erkennen ist.
const VERWEIS: &str = "Der vollstaendige Verlauf liegt unter";

/// Wie viele Nachrichtenpaare das Gespraech hat.
///
/// ⛔️ **Der erste Entwurf hatte dreizehn, und das war der Fehler.** Ein
/// Gespraech von 682 Token wird nicht wirklich verdichtet: Die
/// Zusammenfassung behielt die Nadel in zwei von drei Laeufen, und ein
/// Lauf, in dem nichts verschwunden ist, belegt nichts. **Verdichtet
/// wird, wenn der Kontext voll laeuft**, und das ist die Lage, die
/// diese Probe herstellen muss. ⚑ **Seit dem zweiten Anlauf 120**, denn
/// bei sechzig behielt das 4B die Nadel noch: Ein Zusammenfasser, der
/// genug Platz hat, wirft nichts weg.
const PAARE: usize = 120;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0].starts_with('-') {
        eprintln!(
            "nadelprobe <artefakt> [--laeufe N] [--schritte N] [--token N] [--zeigen] \
             [--regel streng|mild] [--kiste Base|Advanced] [--anweisung] [--deutsch]"
        );
        std::process::exit(2);
    }
    let artefakt = args[0].clone();
    let laeufe = zahl(&args, "--laeufe").unwrap_or(1);
    let schritte = zahl(&args, "--schritte").unwrap_or(6);
    let token = zahl(&args, "--token").unwrap_or(384);
    let zeigen = args.iter().any(|a| a == "--zeigen");
    // ⚑ **Drei Schalter, die die Frage „war es zu schwer?"
    // auseinandernehmen.** `--anweisung` sagt dem Modell rundheraus, es
    // solle nachschlagen (dann misst der Lauf nur noch das Bedienen,
    // nicht mehr das Erkennen), `--deutsch` gibt die Werkzeuge in der
    // deutschen Paraphrase an (dann misst er, ob die englische Ansage
    // die Huerde war), und `--kiste` waehlt den Satz.
    let anweisung = args.iter().any(|a| a == "--anweisung");
    let deutsch = args.iter().any(|a| a == "--deutsch");
    let kiste = match args
        .windows(2)
        .find(|p| p[0] == "--kiste")
        .map(|p| p[1].to_lowercase())
        .unwrap_or_else(|| "advanced".to_string())
        .as_str()
    {
        "base" => myl_client::werkzeuge::Werkzeugkiste::Base,
        "advanced" => myl_client::werkzeuge::Werkzeugkiste::Advanced,
        anderes => {
            eprintln!("nadelprobe: --kiste kennt `Base` und `Advanced`, nicht `{anderes}`");
            std::process::exit(2);
        }
    };
    // `--regel streng` oder `--regel mild`; ohne den Schalter keine Regel.
    let regelwort = args
        .windows(2)
        .find(|p| p[0] == "--regel")
        .map(|p| p[1].clone())
        .unwrap_or_default();
    let regel: Option<&str> = match regelwort.as_str() {
        "streng" => Some(HAUSREGEL_STRENG),
        "mild" => Some(HAUSREGEL_MILD),
        "" => None,
        anderes => {
            eprintln!("nadelprobe: --regel kennt `streng` und `mild`, nicht `{anderes}`");
            std::process::exit(2);
        }
    };

    let e = Einstellungen::lesen(&Einstellungen::vorgabepfad()).unwrap_or_default();
    let anfang = std::time::Instant::now();
    let mut m = match Oertlichesmodell::laden(&artefakt, &e.kapazitaet) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("nadelprobe: {f}");
            std::process::exit(1);
        }
    };
    m.grenze = token;
    let modellname = std::path::Path::new(&artefakt)
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| artefakt.clone());

    println!("=== Nadel im Heuhaufen: {modellname} ===");
    println!("Artefakt: {artefakt}, in {:.1} s geladen", anfang.elapsed().as_secs_f64());
    println!(
        "Erfundener Verlauf: {} Nachrichten, Nadel an drei Stellen, {laeufe} Lauf/Laeufe je Stelle",
        PAARE * 2
    );
    println!("Kontext: erfundene Zusammenfassung ohne die Kennung, dazu der Verweis aus dem Betrieb");
    println!("Schritte je Auftrag: {schritte}, Token je Antwort: {token}");
    println!(
        "Kiste: {}, Ansage: {}, Frage: {}, Hausregel: {}\n",
        kiste.name(),
        if deutsch { "deutsch" } else { "amtlich" },
        if anweisung { "mit Anweisung nachzuschlagen" } else { "ohne Hinweis" },
        if regelwort.is_empty() { "keine" } else { regelwort.as_str() }
    );

    let mut gueltig = 0usize;
    let mut treffer = 0usize;
    let mut ungueltig = 0usize;
    let mut gegenprobe_traf = 0usize;
    let mut kontrolle_richtig = 0usize;
    let mut kontrolle_umweg = 0usize;
    let mut zeilen: Vec<String> = Vec::new();

    for (wo, stelle) in STELLEN {
        for lauf in 1..=laeufe {
            let z = ein_lauf(&m, &modellname, wo, stelle, lauf, &Aufbau {
                schritte,
                token,
                zeigen,
                regel,
                anweisung,
                deutsch,
                kiste,
            });
            match z {
                Urteil::Ungueltig(grund) => {
                    ungueltig += 1;
                    zeilen.push(format!("  {wo:<6} {lauf}  UNGUELTIG  {grund}"));
                }
                Urteil::Gueltig(g) => {
                    gueltig += 1;
                    if g.gefunden {
                        treffer += 1;
                    }
                    if g.gegenprobe_gefunden {
                        gegenprobe_traf += 1;
                    }
                    if g.kontrolle_richtig {
                        kontrolle_richtig += 1;
                    }
                    if g.kontrolle_mit_werkzeug {
                        kontrolle_umweg += 1;
                    }
                    zeilen.push(format!(
                        "  {wo:<6} {lauf}  {:<9}  {:<34}  Verlauf {:>5}, Kontext {:>4} (davon Verzeichnis {:>4}), Ende {:>5} Token (Ansage {})  {:.1} s{}",
                        if g.gefunden { "GEFUNDEN" } else { "verfehlt" },
                        g.weg,
                        g.kontext_ganz,
                        g.kontext_verdichtet,
                        g.kontext_verzeichnis,
                        g.kontext_ende,
                        g.ansage,
                        g.sekunden,
                        format!(
                            "  Kontrollfrage {}{}",
                            if g.kontrolle_richtig { "richtig" } else { "FALSCH" },
                            if g.kontrolle_mit_werkzeug { " (Umweg ueber ein Werkzeug)" } else { "" }
                        ) + if g.gegenprobe_gefunden { "  ⛔️ Gegenprobe traf auch" } else { "" }
                    ));
                }
            }
            println!("{}", zeilen.last().map(|s| s.as_str()).unwrap_or(""));
        }
    }

    println!("\n--- {modellname} ---");
    println!("Gueltige Laeufe: {gueltig}, davon gefunden: {treffer}");
    println!(
        "Kontrollfrage (Antwort steht im Kontext): {kontrolle_richtig} richtig, \
         davon {kontrolle_umweg} mit unnoetigem Werkzeugaufruf"
    );
    println!("Ungueltig (die Probe hat sich selbst verraten): {ungueltig}");
    if gegenprobe_traf > 0 {
        println!("⛔️ Die Gegenprobe traf {gegenprobe_traf} mal: diese Probe misst nicht, was sie messen soll.");
        std::process::exit(1);
    }
    println!("⚑ Die Gegenprobe (leerer Mitschnitt) fand die Nadel kein einziges Mal.");
}

struct Gueltig {
    gefunden: bool,
    gegenprobe_gefunden: bool,
    weg: String,
    /// Der ganze ungekuerzte Heuhaufen, in Token.
    kontext_ganz: usize,
    /// Die Zusammenfassung samt Verzeichnis, in Token.
    kontext_verdichtet: usize,
    /// Davon das Verzeichnis allein.
    kontext_verzeichnis: usize,
    /// Alles, was am Ende des Laufs im Prompt stand.
    kontext_ende: usize,
    /// Davon die Werkzeugansage.
    ///
    /// ⚑ **Sie gehoert ausgewiesen und nicht verschwiegen.** Sie steht
    /// bei **jedem** Lauf vorn, mit und ohne Mitschnitt; wer sie
    /// mitzaehlt, ohne sie zu nennen, laesst den Leser glauben, das
    /// Nachschlagen habe so viel gekostet.
    ansage: usize,
    sekunden: f64,
    /// Ob die Kontrollfrage richtig beantwortet wurde.
    kontrolle_richtig: bool,
    /// Und ob sie dafuer ein Werkzeug gerufen hat, was ein Umweg waere.
    kontrolle_mit_werkzeug: bool,
}

enum Urteil {
    Gueltig(Gueltig),
    Ungueltig(String),
}

/// Wie ein Lauf aufgebaut ist.
///
/// ⚑ **Eine Struktur und keine acht Parameter**, seit der dritte
/// Schalter dazukam: Acht Argumente in einer Reihe sind acht
/// Gelegenheiten, zwei zu vertauschen.
struct Aufbau<'a> {
    schritte: usize,
    token: usize,
    zeigen: bool,
    regel: Option<&'a str>,
    anweisung: bool,
    deutsch: bool,
    kiste: myl_client::werkzeuge::Werkzeugkiste,
}

fn ein_lauf(
    m: &Oertlichesmodell,
    modellname: &str,
    wo: &str,
    stelle: usize,
    lauf: usize,
    a: &Aufbau<'_>,
) -> Urteil {
    // ⚑ **Ein eigener Ordner je Lauf.** Ein Lauf in den Ueberresten des
    // vorigen kann bestehen, ohne etwas getan zu haben, und genau diese
    // Sorte stiller Erfolg ist das, was hier nicht passieren darf.
    let wurzel = std::env::temp_dir()
        .join(format!("myl-nadelprobe-{}-{wo}-{lauf}", std::process::id()));
    let _ = std::fs::remove_dir_all(&wurzel);
    if let Err(f) = std::fs::create_dir_all(&wurzel) {
        return Urteil::Ungueltig(format!("kein Ordner: {f}"));
    }
    let kennung = nadelkennung(wo, lauf);
    let anfang = std::time::Instant::now();

    // 1. Der erfundene Verlauf, als echter Mitschnitt geschrieben.
    let verlauf = heuhaufen(&kennung, stelle);
    let kontext_ganz = m.kontext(&verlauf).map(|s| s.belegt).unwrap_or(0);
    let abschnitte: Vec<(String, String)> =
        verlauf.iter().map(|n| (n.role.clone(), n.content.clone())).collect();
    let name = match myl_client::verlauf::schreiben(
        &wurzel,
        &format!("nadel-{wo}-{lauf}"),
        modellname,
        &abschnitte,
    ) {
        Ok(n) => n,
        Err(f) => return Urteil::Ungueltig(format!("Mitschnitt scheiterte: {f}")),
    };

    // 2. Der erfundene Kontext: Themen ja, Nadel nein, dazu der Verweis
    //    aus dem Betrieb.
    let zusammenfassung = format!(
        "{}\n{}{}",
        myl_local_agent::verdichtung::KOPF,
        ZUSAMMENFASSUNG,
        myl_client::gespraech::verweis(&wurzel, &name)
    );
    let kontext = vec![Nachricht::nutzer(zusammenfassung)];
    let kontext_verdichtet = m.kontext(&kontext).map(|s| s.belegt).unwrap_or(0);
    let kontext_verzeichnis = kontext
        .iter()
        .find_map(|n| n.content.find(VERWEIS).map(|i| n.content[i..].to_string()))
        .and_then(|t| m.kontext(&[Nachricht::nutzer(t)]))
        .map(|s| s.belegt)
        .unwrap_or(0);
    if a.zeigen {
        println!("\n--- Kontext ({wo}/{lauf}), {kontext_verdichtet} Token ---");
        for n in &kontext {
            println!("[{}] {}", n.role, n.content);
        }
    }

    // 3. ⛔️ **Die Zusicherung, auf der alles ruht.** Steht die Kennung
    //    irgendwo im Prompt, misst dieser Lauf nichts, und das ist ein
    //    Fehler der Probe und keiner des Modells.
    let ganzer_prompt: String = kontext.iter().map(|n| n.content.clone()).collect();
    if enthaelt(&ganzer_prompt, &kennung) {
        return Urteil::Ungueltig("die Kennung steht im Kontext".to_string());
    }

    // ⛔️ **Kein Formatbeispiel in der Frage.** Ein frueherer Entwurf
    // nannte die Form „XX-0000-WORT", und das kleine Modell antwortete
    // woertlich „XX-0000-WORT". **Eine Frage, die die Gestalt der
    // Antwort verraet, misst das Nachplappern und nicht das
    // Nachschlagen.**
    let frage = if a.anweisung {
        // ⚑ **Die Aufgabe ohne den Erkennungsschritt.** Wer dem Modell
        // sagt, dass es nachschlagen soll, misst nur noch, ob es das
        // Werkzeug **bedienen** kann.
        "Schlag im aufgezeichneten Verlauf nach und nenne die Kennung, unter der der \
         Pruefstand in Halle 3 laeuft."
            .to_string()
    } else {
        "Unter welcher Kennung laeuft der Pruefstand in Halle 3? \
         Die Kennung wurde frueher im Gespraech genannt."
            .to_string()
    };

    let (gefunden, weg, kontext_ende, ansage) =
        fragen(m, &wurzel, &kontext, &frage, &kennung, a, a.zeigen);

    // ⚑ **Der Preis der Regel, im selben Lauf gemessen.** Die Antwort
    // auf diese Frage steht in der Zusammenfassung; ein Werkzeugaufruf
    // ist hier ein Umweg und keine Leistung.
    let (kontrolle_richtig, kontrollweg, _, _) =
        fragen(m, &wurzel, &kontext, KONTROLLFRAGE, KONTROLLANTWORT, a, false);

    // ⚑ **Die Gegenprobe im selben Lauf**, mit demselben Kontext und
    // **ohne** Mitschnitt: Was hier trifft, kam nicht aus dem
    // Mitschnitt.
    let _ = std::fs::remove_dir_all(wurzel.join(myl_client::verlauf::ORDNER));
    let (gegenprobe_gefunden, _, _, _) =
        fragen(m, &wurzel, &kontext, &frage, &kennung, a, false);

    let _ = std::fs::remove_dir_all(&wurzel);
    Urteil::Gueltig(Gueltig {
        gefunden,
        gegenprobe_gefunden,
        weg,
        kontext_ganz,
        kontext_verdichtet,
        kontext_verzeichnis,
        kontext_ende,
        ansage,
        sekunden: anfang.elapsed().as_secs_f64(),
        kontrolle_richtig,
        kontrolle_mit_werkzeug: kontrollweg != "ohne Werkzeug",
    })
}

/// **Die erfundene Zusammenfassung.**
///
/// ⚑ **Sie nennt die Themen und nicht die Nadel**, und sie ist so
/// geschrieben, wie eine brauchbare Verdichtung aussaehe: Sie sagt,
/// worum es ging, und laesst die Einzelheiten weg. ⛔️ **Halle 3 kommt
/// darin nicht vor**, denn ein Kontext, der das Thema nennt, aber die
/// Zahl weglaesst, waere ein Hinweis; hier soll die Einzelheit
/// vollstaendig verschwunden sein.
const ZUSAMMENFASSUNG: &str = "Das Gespraech ging ueber den Stand der Arbeit an einer \
ganzzahligen Inferenz und ihrem Umfeld. Besprochen wurden die Konformitaetsvektoren \
(48 Stueck, gegen beide Rechenwege, Abdruck unveraendert), der Durchsatz auf der \
kleinen Maschine, die Kosten des Umbaus auf die neue Modellreihe, die Schranken fuer \
die Netzbetreiber, der Stand der Speicherseite, die Dauer der Trainingslaeufe samt \
Haltemenge und Rauschnullpunkt, die Messung des Umsetzungsverlusts ueber 13797 \
Positionen, der Aufbau des Expertengemisches, der Bauweg des Servers, die \
Freigabemaske und der Konsolenclient. Dazwischen ging es um den Betrieb der Anlage, \
um Pruefstaende und Messreihen und um die Frage, was am Abend noch laufen soll. \
Einzelne Zahlen und Kennungen sind in dieser Zusammenfassung nicht enthalten.";

/// Ein Auftrag mit Werkzeugen, und was dabei herauskam.
fn fragen(
    m: &Oertlichesmodell,
    wurzel: &std::path::Path,
    verlauf: &[Nachricht],
    frage: &str,
    kennung: &str,
    a: &Aufbau<'_>,
    zeigen: bool,
) -> (bool, String, usize, usize) {
    let agent = myl_client::einstellungen::Agenteneinstellung {
        schritte: a.schritte as u32,
        wurzel: Some(wurzel.display().to_string()),
        schreiben: false,
        ..Default::default()
    };
    let r = match myl_client::ruestung::ruesten(
        &agent,
        if a.deutsch { myl_client::Ansageform::Deutsch } else { myl_client::Ansageform::Amtlich },
        a.kiste,
        vec![],
    ) {
        Ok(r) => r,
        Err(f) => return (false, format!("ungeruestet: {f}"), 0, 0),
    };
    let lauf = myl_client::lauf::fahren_mit_hausregel(
        m,
        &r,
        a.schritte,
        true,
        a.token as u32,
        verlauf,
        frage,
        None,
        a.regel,
    );
    let antwort = lauf.antwort.clone().unwrap_or_default();
    let gefunden = enthaelt(&antwort, kennung);
    let mut gerufen: Vec<String> = Vec::new();
    for n in &lauf.nachrichten {
        if n.role == "assistant" {
            for ruf in myl_local_agent::werkzeug::vorschlaege(&n.content).into_iter().flatten() {
                gerufen.push(ruf.name.clone());
            }
        }
    }
    let weg = if gerufen.is_empty() { "ohne Werkzeug".to_string() } else { gerufen.join(">") };
    let kontext_ende = m.kontext(&lauf.nachrichten).map(|s| s.belegt).unwrap_or(0);
    let ansage = lauf
        .nachrichten
        .iter()
        .find(|n| n.role == "system")
        .and_then(|n| m.kontext(std::slice::from_ref(n)))
        .map(|s| s.belegt)
        .unwrap_or(0);
    if zeigen {
        println!("--- Weg und Antwort ---");
        for n in &lauf.nachrichten {
            if n.role != "system" {
                println!("[{}] {}", n.role, myl_client::lauf::bis_zur_grenze(&n.content, 600));
            }
        }
        println!("--- Ende (gesucht: {kennung}) ---\n");
    }
    (gefunden, weg, kontext_ende, ansage)
}

/// ⚑ **Die Nadel ist nicht zu erraten.** Sie steht nirgends sonst, und
/// sie wechselt je Lauf: Ein Modell, das sie nennt, hat sie gelesen.
fn nadelkennung(wo: &str, lauf: usize) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    let woerter = ["MOOS", "KIES", "NEBEL", "FIRN", "TANG", "LEHM"];
    let buchstaben = ['Q', 'X', 'V', 'Z', 'K', 'J'];
    let a = buchstaben[(t + wo.len()) % buchstaben.len()];
    let b = buchstaben[(t / 7 + lauf) % buchstaben.len()];
    let zahl = 1000 + (t / 13 + lauf * 97) % 9000;
    format!("{a}{b}-{zahl}-{}", woerter[(t / 31 + lauf) % woerter.len()])
}

fn enthaelt(text: &str, kennung: &str) -> bool {
    text.to_uppercase().contains(&kennung.to_uppercase())
}

/// **Das Gespraech, in dem die Nadel liegt.**
///
/// ⚑ **Es ist erfunden und bleibt gleich**, bis auf die Nadel: Zwei
/// Laeufe unterscheiden sich in dem, was gemessen wird, und sonst in
/// nichts.
fn heuhaufen(kennung: &str, stelle_prozent: usize) -> Vec<Nachricht> {
    // ⚑ **Dreizehn Themen, sechzig Runden.** Die Zahlen wandern je
    // Runde, damit kein Satz woertlich zweimal dasteht: Ein Gespraech
    // aus Wiederholungen liesse sich mit einem Satz zusammenfassen, und
    // dann waere die Verdichtung leichter als die echte.
    const THEMEN: [(&str, &str); 13] = [
        ("Wie steht es um die Konformitaetsvektoren in Runde {n}?",
         "Alle 48 Vektoren laufen gegen beide Rechenwege durch, und der Abdruck hat sich nicht bewegt. Gemessen wurde auf Ebene {n}, mit derselben Sperrdatei wie zuvor. Der Lauf brauchte {a} Sekunden."),
        ("Und der Durchsatz im Abschnitt {n}?",
         "Vorbereitung {b} Token je Sekunde, Dekodieren knapp {c}. Beides mit den SIMD-Kernen und auf einer ruhigen Maschine, denn eine Messung auf einer beschaeftigten Maschine ist keine."),
        ("Was hat der Umbau in Woche {n} gekostet?",
         "Zwei Tage. Die Vektoren mussten neu gestempelt werden, weil sich die Ebenenzahl von {a} auf {b} aenderte, und die Anleitung ging dieselbe Kette durch."),
        ("Wer prueft die Schranken fuer die Betreiber, Stand {n}?",
         "Die Governance-Seite haelt die Zahlen gegen das Register. Aktuell stehen {b} GiB je Knoten und {c} Byte je Segment, und beide Zahlen sind gerechnet und nicht geschaetzt."),
        ("Gibt es offene Punkte auf der Speicherseite, Liste {n}?",
         "Der Blockverlauf auf die Platte fehlt weiterhin. Der Zwischenspeicher ist vollstaendig und traegt {b} Eintraege; alles andere ist nachrechenbar und wird deshalb nicht abgelegt."),
        ("Wie lange braucht ein Trainingslauf in Stufe {n}?",
         "Auf dem kleinen Artefakt {a} Stunden, mit Haltemenge und Rauschnullpunkt. Ohne diese beiden belegt ein Lauf gar nichts, auch wenn die Zahl am Ende gut aussieht."),
        ("Was ist mit der Haltemenge aus Vorlauf {n}?",
         "Die liegt bereit und ist disjunkt, {b} Zeilen. Von Hand gebaute Mengen zaehlen nicht, weil sie die Regel enthalten, die sie pruefen sollen."),
        ("Und die Messung des Umsetzungsverlusts, Durchgang {n}?",
         "Sie laeuft ueber 13797 Positionen je Modell. Der gebuendelte Weg kostet ein Drittel der Zeit, und die Zahl bleibt bis zur letzten Stelle dieselbe."),
        ("Wieviele Experten hat das Gemisch in Konfiguration {n}?",
         "128, davon acht je Token. Deshalb ist es schneller als das dichte Modell derselben Groesse, obwohl es {b} GB auf der Platte belegt."),
        ("Steht der Bauweg fuer den Server, Fassung {n}?",
         "Ja, mit Sperrdatei und ohne Netz beim Bauen. Die Einheit haelt fuer den Dauerbetrieb, startet hoechstens {a} mal in {c} Sekunden neu und laeuft unter eigenen Rechten."),
        ("Was macht die Freigabemaske in Entwurf {n}?",
         "Jeder Regler steht offen ganz rechts, und wer nichts einstellt, gibt alles frei. Die linke Endstellung nimmt ein Rechenwerk ganz heraus, eine Grenze faellt nur bis eins."),
        ("Und der Konsolenclient, Punkt {n}?",
         "Der nimmt jetzt ebenfalls alles, ausser es wird ausdruecklich reduziert. Die Pfeiltasten schieben den Regler, {a} Prozent je Anschlag."),
        ("Gibt es sonst etwas fuer Tag {n}?",
         "Nur die Laeufe am Abend, damit sie ueber Nacht durchlaufen koennen. Bis dahin steht die Kette, und die Proben sind gruen."),
    ];

    // ⛔️ **Die Nadel darf sich nicht wichtig machen.** Der erste
    // Entwurf schrieb „Noch eine Sache fuers Protokoll", und das 4B
    // behielt sie prompt in der Zusammenfassung: Ein so markierter Satz
    // ist genau das, was ein Zusammenfasser aufhebt. **Dann misst die
    // Probe die Markierung und nicht das Nachschlagen.** Jetzt steht
    // die Nadel da wie alles andere, als eine Auskunft unter 120.
    let nadel = "Und was macht der Pruefstand in Halle 3?".to_string();
    let nadelantwort = format!(
        "Der laeuft unter der Kennung {kennung} und ist seit Dienstag wieder frei. \
         Die Messreihe darauf ist durch, ohne Befund."
    );
    let stelle = (PAARE * 2).saturating_mul(stelle_prozent) / 100;
    let stelle = stelle - stelle % 2;

    let mut aus: Vec<Nachricht> = Vec::new();
    for runde in 0..PAARE {
        if aus.len() == stelle {
            aus.push(Nachricht::nutzer(&nadel));
            aus.push(Nachricht { role: "assistant".to_string(), content: nadelantwort.clone() });
        }
        let (frage, antwort) = THEMEN[runde % THEMEN.len()];
        let n = runde + 1;
        let fuellen = |t: &str| {
            t.replace("{n}", &n.to_string())
                .replace("{a}", &(3 + n % 9).to_string())
                .replace("{b}", &(100 + n * 7).to_string())
                .replace("{c}", &(40 + n * 3).to_string())
        };
        aus.push(Nachricht::nutzer(fuellen(frage)));
        aus.push(Nachricht { role: "assistant".to_string(), content: fuellen(antwort) });
    }
    if !aus.iter().any(|n| n.content.contains(kennung)) {
        aus.push(Nachricht::nutzer(&nadel));
        aus.push(Nachricht { role: "assistant".to_string(), content: nadelantwort.clone() });
    }
    aus
}

fn zahl(args: &[String], name: &str) -> Option<usize> {
    args.windows(2).find(|p| p[0] == name).and_then(|p| p[1].parse().ok())
}
