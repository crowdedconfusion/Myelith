//! Ein Agentenlauf, als **Daten** statt als Ausgabe.
//!
//! # ⚑ Warum das hier steht und nicht im Bedieninstrument
//!
//! Die Oberflaeche soll dieselben Unterbefehle rufen wie die
//! Kommandozeile, **ohne eigene Logik**. Solange der Lauf im
//! Binaerprogramm stand und dabei druckte, blieben der Oberflaeche
//! genau zwei Wege, und beide sind falsch:
//!
//! | Weg | Warum er nicht taugt |
//! |---|---|
//! | `myl` als Unterprozess starten | Dann muss sie eine Ausgabe **fuer Menschen** wieder zerlegen |
//! | den Lauf nachbauen | 📌 Zwei Wege zu derselben Sache laufen auseinander, und der zweite ist der schlechter geprueete |
//!
//! ⚑ **Deshalb gibt dieses Modul den Verlauf als Struktur heraus.** Wer
//! ihn druckt, druckt; wer ihn in ein Fenster schreibt, schreibt.
//! Gerechnet wird einmal.

use crate::ruestung::Ruestung;

/// Ein Schritt des Verlaufs, wie ihn beide Anzeigen brauchen.
///
/// ⚑ **Der Systemtext mit den Werkzeugschemata ist NICHT dabei.** Er
/// ist laenger als jede Antwort und gehoert in keine Anzeige; wer ihn
/// sehen will, nimmt [`Ausgang::nachrichten`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schritt {
    /// Was das Modell vor einem Werkzeugaufruf geschrieben hat.
    ///
    /// ⚑ **Gehoert dazu.** Ein Modell schreibt oft hin, was es vorhat,
    /// und genau daran erkennt man einen falschen Plan, **bevor** das
    /// Werkzeug ihn ausfuehrt.
    Plan(String),
    /// Was das Modell dabei ueberlegt hat.
    ///
    /// 📌 **Bis zum 2026-09-10 steckte das im Plan.** `ohne_aufrufe`
    /// schnitt die Werkzeugaufrufe heraus und den Denkblock nicht, und
    /// so stand die ganze Ueberlegung als Zeile in der Befehlsliste,
    /// gemeldet vom Projektinhaber. **Sie ist kein Befehl**, und sie
    /// gehoert in ihre eigene Klappe: Nach einem Werkzeugaufruf faengt
    /// das Modell neu an zu ueberlegen, und das ist ein neuer Block und
    /// keine Fortsetzung des alten.
    Denken(String),
    /// Ein Werkzeugaufruf, wie das Modell ihn vorschlug.
    Aufruf {
        /// Der Name des Werkzeugs.
        name: String,
        /// Die Argumente, schon lesbar gemacht.
        argumente: String,
        /// **Die Argumente genau so, wie sie ankamen**, als eingerucktes
        /// JSON, bis [`VOLLTEXT_GRENZE`] Zeichen. Die Anzeige klappt sie auf
        /// Wunsch aus (Auftrag des Projektinhabers, 2026-09-14).
        voll: String,
    },
    /// Ein Vorschlag, den niemand lesen konnte.
    Unlesbar(String),
    /// Was ein Werkzeug zurueckgab, **vollstaendig** bis
    /// [`VOLLTEXT_GRENZE`] Zeichen.
    ///
    /// 📌 **Bis zum 2026-09-14 eine Zeile von 200 Zeichen.** Die Anzeige
    /// zeigte damit, dass etwas zurueckkam, aber nicht was; wer wissen
    /// wollte, warum der Agent danach etwas Bestimmtes tat, konnte es
    /// nicht nachlesen.
    Ergebnis(String),
    /// Die Schlussantwort ohne Werkzeugaufruf.
    Antwort(String),
}

/// Was ein Lauf zurueckbringt.
pub struct Ausgang {
    /// Der Verlauf, wie ihn eine Anzeige braucht.
    pub verlauf: Vec<Schritt>,
    /// Die Schlussantwort, falls es eine gab.
    ///
    /// 📌 **`None` ist ein Befund und kein Nichts:** Der Lauf ist in
    /// Werkzeugaufrufen steckengeblieben oder an der Schrittgrenze
    /// gelandet.
    pub antwort: Option<String>,
    /// Die vollstaendigen Nachrichten, fuer den Fehlerfall.
    pub nachrichten: Vec<myl_local_agent::tuerklient::Nachricht>,
    /// Warum es aufhoerte.
    pub ende: myl_local_agent::schleife::Ende,
    /// Wie lange es dauerte.
    pub sekunden: f64,
}

impl Ausgang {
    /// Ob der Lauf regulaer endete.
    pub fn fertig(&self) -> bool {
        matches!(self.ende, myl_local_agent::schleife::Ende::Fertig)
    }
}

/// Der Sitzungskontrakt eines Auftrags.
///
/// ⚑ **Er ist kein Vertrag mit jemandem**, sondern die Schranke, an der
/// die Schleife haelt: Schrittzahl und Budget. Ohne ihn liefe ein
/// Agent, dessen Modell sich verrennt, unbegrenzt weiter.
///
/// 📌 **Panik statt Fehlerwert**, und das ist hier vertretbar: Die Werte
/// sind fest verdrahtet, also kann `neu` nur scheitern, wenn dieser
/// Quelltext falsch ist, und nicht wegen einer Eingabe.
pub fn kontrakt_fuer(schritte: usize) -> myl_types::sitzung::Sitzungskontrakt {
    let grenze = || myl_types::sitzung::Grenzen {
        budget: 1000,
        einzellimit: 100,
        schwelle: u64::MAX,
        zeugenleiter: Vec::new(),
    };
    myl_types::sitzung::Sitzungskontrakt::neu(
        myl_types::ids::Address::new([1u8; 32]),
        myl_types::ids::Address::new([2u8; 32]),
        grenze(),
        grenze(),
        vec![myl_types::ids::Address::new([9u8; 32])],
        myl_types::ids::EpochId(0),
        myl_types::ids::EpochId(u64::MAX),
        schritte as u32,
    )
    .expect("der Sitzungskontrakt ist fest verdrahtet und kann nicht an einer Eingabe scheitern")
}

/// Faehrt einen Auftrag und gibt den Verlauf heraus.
pub fn fahren(
    modell: &dyn myl_local_agent::tuerklient::Modellweg,
    ruestung: &Ruestung,
    schritte: usize,
    bezeugtes: bool,
    max_tokens: u32,
    auftrag: &str,
) -> Ausgang {
    fahren_beobachtet(modell, ruestung, schritte, bezeugtes, max_tokens, auftrag, None)
}

/// Wie [`fahren`], meldet aber **waehrend** des Laufs, was geschieht.
///
/// # ⚑ Wofuer das da ist
///
/// Ein Werkzeug, das ein Verzeichnis durchsucht, laeuft merklich lange.
/// Wer erst am Ende anzeigt, zeigt in dieser Zeit ein Fenster, das
/// stillsteht, und ein stehendes Fenster sieht aus wie ein
/// abgestuerztes.
///
/// ⚑ **Der Melder sieht zu und entscheidet nichts.** Er wird an
/// Stellen gerufen, an denen die Entscheidung schon gefallen ist, und
/// gibt nichts zurueck. Ein Haken, der den Lauf beeinflussen koennte,
/// waere eine zweite Quelle fuer Erlaubnisse neben Erlaubnis und
/// Betriebsart.
///
/// 📌 **Und der laufende Text kommt nicht von hier**, sondern vom
/// Modell selbst: `Oertlichesmodell::beobachter` meldet jedes Token,
/// sobald es dasteht. Die Schleife weiss davon nichts, und sie soll es
/// auch nicht wissen.
pub fn fahren_beobachtet(
    modell: &dyn myl_local_agent::tuerklient::Modellweg,
    ruestung: &Ruestung,
    schritte: usize,
    bezeugtes: bool,
    max_tokens: u32,
    auftrag: &str,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
) -> Ausgang {
    fahren_im_gespraech(modell, ruestung, schritte, bezeugtes, max_tokens, &[], auftrag, melder)
}

/// **Wie [`fahren_beobachtet`], mit dem bisherigen Gespraech vor dem
/// Auftrag** (Entscheidung C2, siehe
/// `myl_local_agent::schleife::Lauf::fahren_mit_verlauf`).
///
/// Die Nachrichten des Ausgangs tragen das Gespraech danach, samt einer
/// Verdichtung, falls eine noetig war; [`crate::gespraech::Gespraech`]
/// uebernimmt sie.
#[allow(clippy::too_many_arguments)]
pub fn fahren_im_gespraech(
    modell: &dyn myl_local_agent::tuerklient::Modellweg,
    ruestung: &Ruestung,
    schritte: usize,
    bezeugtes: bool,
    max_tokens: u32,
    verlauf: &[myl_local_agent::Nachricht],
    auftrag: &str,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
) -> Ausgang {
    fahren_mit_hausregel(
        modell, ruestung, schritte, bezeugtes, max_tokens, verlauf, auftrag, melder, None,
    )
}

/// **Wie [`fahren_im_gespraech`], mit einer Hausregel hinter der
/// Werkzeugansage.**
///
/// ⚑ **Gebaut fuer die Messung**, weil „hilft ein strengerer
/// Systemprompt?" eine Frage ist, die man beantwortet und nicht
/// bespricht. ⛔️ **Im Netz muss die Regel fuer alle Knoten dieselbe
/// sein**, sonst ruesten zwei Knoten am selben Auftrag verschieden;
/// oertlich ist sie eine Einstellung wie die Kiste.
#[allow(clippy::too_many_arguments)]
pub fn fahren_mit_hausregel(
    modell: &dyn myl_local_agent::tuerklient::Modellweg,
    ruestung: &Ruestung,
    schritte: usize,
    bezeugtes: bool,
    max_tokens: u32,
    verlauf: &[myl_local_agent::Nachricht],
    auftrag: &str,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
    hausregel: Option<&str>,
) -> Ausgang {
    let anfang = std::time::Instant::now();
    let grenzen = myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen::neu(
        kontrakt_fuer(schritte),
        ruestung.kasten.angebote(),
    );
    let zuordnung = ruestung.zuordnung();
    let erg = myl_local_agent::schleife::Lauf {
        hausregel,
        // ⚡ Was die Werkzeuge erreichen durften, gehoert ins
        // Protokoll: Sonst steht dort nur, DASS ein Schreibwerkzeug
        // erlaubt war, und nicht, worauf es zeigte.
        einhaengung: ruestung.einhaengung.as_ref().map(|e| e.marke()),
        ansageform: ruestung.form,
        klient: modell,
        modell: "lokal",
        grenzen: &grenzen,
        // ⚑ Die enge Vorgabe gilt, bis der Nutzer sie aufhebt.
        betriebsart: if bezeugtes {
            myl_local_agent::betrieb::Betriebsart::Alles
        } else {
            myl_local_agent::betrieb::Betriebsart::NurVerankert
        },
        kasten: &ruestung.kasten,
        registratur: &ruestung.registratur,
        adressen: &zuordnung,
        anker: myl_types::hash::Hash::from_bytes([0u8; 32]),
        max_tokens: Some(max_tokens),
        melder,
    }
    .fahren_mit_verlauf(auftrag, verlauf);

    let (verlauf, antwort) = verlauf_aus(&erg.nachrichten);
    Ausgang {
        verlauf,
        antwort,
        nachrichten: erg.nachrichten,
        ende: erg.ende,
        sekunden: anfang.elapsed().as_secs_f64(),
    }
}

/// Zerlegt die Nachrichten in den Verlauf und die Schlussantwort.
pub fn verlauf_aus(
    nachrichten: &[myl_local_agent::tuerklient::Nachricht],
) -> (Vec<Schritt>, Option<String>) {
    let mut aus = Vec::new();
    let mut letzte: Option<String> = None;
    for n in nachrichten {
        match n.role.as_str() {
            "assistant" => {
                // ⚑ **Zerlegt wird mit demselben Werkzeug wie im
                // laufenden Strom**, und das ist der ganze Grund, warum
                // es hier steht: Zwei Lesarten derselben Antwort liefen
                // auseinander, und die Anzeige zeigte je nach Weg
                // etwas anderes.
                let (denken, prosa) = denken_und_prosa(&n.content);

                let rufe = myl_local_agent::werkzeug::vorschlaege(&n.content);
                if rufe.is_empty() {
                    // ⚑ Auch eine Schlussantwort kann eine Ueberlegung
                    // vor sich haben; sie gehoert in ihre Klappe und
                    // nicht in die Antwort.
                    if !denken.is_empty() {
                        aus.push(Schritt::Denken(denken));
                    }
                    if !prosa.is_empty() {
                        letzte = Some(prosa);
                    }
                    continue;
                }
                if !denken.is_empty() {
                    aus.push(Schritt::Denken(denken));
                }
                let dazwischen = eine_zeile(&prosa, 200);
                if !dazwischen.is_empty() {
                    aus.push(Schritt::Plan(dazwischen));
                }
                for r in &rufe {
                    match r {
                        Ok(v) => aus.push(Schritt::Aufruf {
                            name: v.name.clone(),
                            argumente: kurzform(&v.arguments),
                            voll: volltext_der_argumente(&v.arguments),
                        }),
                        Err(u) => aus.push(Schritt::Unlesbar(u.roh.clone())),
                    }
                }
            }
            "tool" => aus.push(Schritt::Ergebnis(bis_zur_grenze(&n.content, VOLLTEXT_GRENZE))),
            _ => {}
        }
    }
    if let Some(t) = &letzte {
        aus.push(Schritt::Antwort(t.clone()));
    }
    (aus, letzte)
}

/// Ueberlegung und Prosa einer Modellantwort, getrennt.
///
/// ⚑ **Ueber denselben Zerleger wie der laufende Strom.** Eine zweite
/// Lesart derselben Antwort liefe auseinander, und die Anzeige zeigte
/// je nach Weg etwas anderes.
pub fn denken_und_prosa(inhalt: &str) -> (String, String) {
    let mut z = crate::strom::Zerleger::neu();
    let mut stuecke = z.schluck(inhalt);
    stuecke.extend(z.abschluss());
    let sammeln = |waehle: fn(&crate::strom::Stueck) -> Option<&str>| -> String {
        stuecke.iter().filter_map(waehle).collect::<String>().trim().to_string()
    };
    (
        sammeln(|s| match s {
            crate::strom::Stueck::Denken(t) => Some(t.as_str()),
            _ => None,
        }),
        sammeln(|s| match s {
            crate::strom::Stueck::Text(t) => Some(t.as_str()),
            _ => None,
        }),
    )
}

/// Der Text einer Antwort **ohne** die Werkzeugaufrufe darin.
///
/// ⚑ Geschnitten wird an denselben Marken, an denen
/// `werkzeug::vorschlaege` liest; alles andere waere eine zweite,
/// abweichende Lesart derselben Antwort.
pub fn ohne_aufrufe(t: &str) -> String {
    let mut aus = String::new();
    let mut rest = t;
    while let Some(a) = rest.find("<tool_call>") {
        aus.push_str(&rest[..a]);
        rest = match rest[a..].find("</tool_call>") {
            Some(e) => &rest[a + e + "</tool_call>".len()..],
            None => "",
        };
    }
    aus.push_str(rest);
    eine_zeile(&aus, 200)
}

/// Argumente in einer Zeile, ohne die JSON-Klammern.
pub fn kurzform(a: &serde_json::Value) -> String {
    let Some(o) = a.as_object() else { return eine_zeile(&a.to_string(), 80) };
    o.iter()
        .map(|(k, v)| {
            let w = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
            format!("{k}={}", eine_zeile(&w, 40))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **Wie viele Zeichen eines Befehls oder einer Werkzeugantwort die Anzeige
/// bekommt.**
///
/// ⚑ **Eine Grenze und kein Volltext um jeden Preis.** Das Fenster legt
/// seine Gespraeche im Browserspeicher ab, und der fasst wenige Megabyte;
/// ein Werkzeug, das eine grosse Datei liest, fuellte ihn mit einer
/// einzigen Antwort. Was darueber hinausgeht, steht mit Vermerk da.
pub const VOLLTEXT_GRENZE: usize = 4000;

/// Die Argumente eines Aufrufs als eingerucktes JSON, bis zur Grenze.
pub fn volltext_der_argumente(a: &serde_json::Value) -> String {
    bis_zur_grenze(&serde_json::to_string_pretty(a).unwrap_or_else(|_| a.to_string()), VOLLTEXT_GRENZE)
}

/// Text mit allen Zeilen, aber hoechstens `n` Zeichen, mit Vermerk.
pub fn bis_zur_grenze(t: &str, n: usize) -> String {
    let zeichen = t.chars().count();
    if zeichen <= n {
        return t.to_string();
    }
    let vorn: String = t.chars().take(n).collect();
    format!("{vorn}\n… ({} Zeichen mehr)", zeichen - n)
}

/// Text auf eine Zeile und auf `n` Zeichen, mit sichtbarer Kuerzung.
///
/// ⚑ **Gezaehlt wird in Zeichen und nicht in Bytes.** Ein Schnitt
/// mitten durch eine Mehrbytefolge erzeugte ein Ersatzzeichen, das im
/// Text gar nicht steht.
pub fn eine_zeile(t: &str, n: usize) -> String {
    let flach: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if flach.chars().count() <= n {
        return flach;
    }
    let gekuerzt: String = flach.chars().take(n).collect();
    format!("{gekuerzt}…")
}

#[cfg(test)]
mod anzeige {
    use super::*;

    /// ⚑ **Gezaehlt wird in Zeichen, nicht in Bytes.** Ein Umlaut ist
    /// zwei Bytes; wer nach Bytes schneidet, trifft irgendwann die
    /// Mitte einer Folge und schreibt ein Ersatzzeichen hin, das im
    /// Text nicht steht.
    #[test]
    fn gekuerzt_wird_nach_zeichen() {
        let t = "ä".repeat(50);
        let aus = eine_zeile(&t, 10);
        assert_eq!(aus.chars().count(), 11, "zehn Zeichen und das Kuerzungszeichen");
        assert!(!aus.contains('\u{FFFD}'), "eine Mehrbytefolge wurde zerschnitten");
    }

    #[test]
    fn der_text_neben_dem_aufruf_bleibt() {
        let t = "Ich sehe zuerst nach. <tool_call>{\"name\":\"x\"}</tool_call> Danach mehr.";
        assert_eq!(ohne_aufrufe(t), "Ich sehe zuerst nach. Danach mehr.");
    }

    #[test]
    fn ohne_aufruf_bleibt_alles() {
        assert_eq!(ohne_aufrufe("nur Text"), "nur Text");
    }

    /// ⚑ Eine abgeschnittene Marke darf nicht den Rest verschlucken
    /// **und** nicht in eine Endlosschleife laufen.
    #[test]
    fn eine_offene_marke_endet() {
        assert_eq!(ohne_aufrufe("davor <tool_call>{\"name\""), "davor");
    }

    #[test]
    fn kurzes_bleibt_ganz() {
        assert_eq!(eine_zeile("kurz", 40), "kurz");
    }

    /// Zeilenumbrueche einer Werkzeugantwort duerfen die Uebersicht
    /// nicht sprengen.
    #[test]
    fn mehrere_zeilen_werden_eine() {
        assert_eq!(eine_zeile("a\n  b\n\nc", 40), "a b c");
    }

    #[test]
    fn argumente_werden_lesbar() {
        let a = serde_json::json!({"pfad": "unter/x.txt", "inhalt": "kurz"});
        let k = kurzform(&a);
        assert!(k.contains("pfad=unter/x.txt"), "{k}");
        assert!(k.contains("inhalt=kurz"), "{k}");
    }

    /// ⚑ Ein langes Argument wird gekuerzt und nicht weggelassen: Wer
    /// eine Datei schreibt, will sehen **welche**, nicht den Inhalt.
    #[test]
    fn ein_langes_argument_wird_gekuerzt() {
        let a = serde_json::json!({"inhalt": "x".repeat(200)});
        assert!(kurzform(&a).chars().count() < 60);
    }
}

#[cfg(test)]
mod denkschritte {
    use super::*;

    fn nachricht(rolle: &str, inhalt: &str) -> myl_local_agent::tuerklient::Nachricht {
        myl_local_agent::tuerklient::Nachricht {
            role: rolle.to_string(),
            content: inhalt.to_string(),
        }
    }

    /// 📌 **Der gemeldete Fehler vom 2026-09-10.**
    ///
    /// Die Ueberlegung stand als Zeile in der Befehlsliste, weil
    /// `ohne_aufrufe` nur die Werkzeugaufrufe herausschnitt und den
    /// Denkblock stehen liess. **Sie ist kein Befehl.**
    #[test]
    fn die_ueberlegung_ist_kein_befehl() {
        let inhalt = "<think>Ich muss das Verzeichnis lesen.</think>Ich sehe nach. \
                      <tool_call>{\"name\":\"verzeichnis\",\"arguments\":{}}</tool_call>";
        let (verlauf, _) = verlauf_aus(&[nachricht("assistant", inhalt)]);

        let denken: Vec<_> = verlauf
            .iter()
            .filter_map(|s| match s {
                Schritt::Denken(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(denken, ["Ich muss das Verzeichnis lesen."], "das Denken fehlt als eigener Schritt");

        for s in &verlauf {
            if let Schritt::Plan(t) = s {
                assert!(
                    !t.contains("Ich muss das Verzeichnis lesen"),
                    "die Ueberlegung steht immer noch im Plan: {t}"
                );
                assert_eq!(t, "Ich sehe nach.");
            }
        }
    }

    /// ⚑ **Nach einem Werkzeugaufruf ist es ein NEUER Block.**
    ///
    /// Das Modell faengt dort neu an zu ueberlegen; zwei Ueberlegungen
    /// in einer Klappe waeren die Behauptung, es sei ein Gedankengang
    /// gewesen.
    #[test]
    fn nach_dem_werkzeug_beginnt_eine_neue_ueberlegung() {
        let verlauf = vec![
            nachricht("assistant", "<think>Erst nachsehen.</think><tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>"),
            nachricht("tool", "12:00"),
            nachricht("assistant", "<think>Jetzt kann ich antworten.</think>Es ist zwölf."),
        ];
        let (schritte, antwort) = verlauf_aus(&verlauf);

        let denken: Vec<_> = schritte
            .iter()
            .filter_map(|s| match s {
                Schritt::Denken(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            denken,
            ["Erst nachsehen.", "Jetzt kann ich antworten."],
            "die beiden Ueberlegungen sind nicht getrennt"
        );

        // ⚑ Und die zweite steht **nach** dem Ergebnis, nicht davor.
        let i = schritte.iter().position(|s| matches!(s, Schritt::Ergebnis(_))).expect("Ergebnis");
        let j = schritte
            .iter()
            .rposition(|s| matches!(s, Schritt::Denken(_)))
            .expect("zweite Ueberlegung");
        assert!(i < j, "die zweite Ueberlegung steht vor dem Werkzeugergebnis");

        assert_eq!(antwort.as_deref(), Some("Es ist zwölf."), "die Antwort traegt das Denken mit");
    }

    /// ⚑ **Befehl und Werkzeugantwort kommen vollstaendig an**, bis zur
    /// Anzeigegrenze, mit allen Zeilen (Auftrag des Projektinhabers,
    /// 2026-09-14: aufklappen, was genau gerufen wurde und was zurueckkam).
    #[test]
    fn befehl_und_antwort_kommen_vollstaendig_an() {
        let lang = format!("Zeile 1\nZeile 2\n{}", "x".repeat(VOLLTEXT_GRENZE));
        let verlauf = vec![
            nachricht("assistant", "<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"src/main.rs\",\"bis\":40}}</tool_call>"),
            nachricht("tool", &lang),
        ];
        let (schritte, _) = verlauf_aus(&verlauf);
        let Some(Schritt::Aufruf { voll, .. }) = schritte.iter().find(|s| matches!(s, Schritt::Aufruf { .. })) else {
            panic!("kein Aufruf: {schritte:?}");
        };
        assert!(voll.contains("\"path\": \"src/main.rs\"") && voll.contains('\n'), "eingeruecktes JSON: {voll}");
        let Some(Schritt::Ergebnis(text)) = schritte.iter().find(|s| matches!(s, Schritt::Ergebnis(_))) else {
            panic!("kein Ergebnis");
        };
        assert!(text.starts_with("Zeile 1\nZeile 2\n"), "die Zeilen bleiben");
        assert!(text.ends_with("… (16 Zeichen mehr)"), "{}", &text[text.len() - 40..]);
        assert_eq!(bis_zur_grenze("kurz", 10), "kurz");
        assert_eq!(bis_zur_grenze("äöüäöü", 3), "äöü\n… (3 Zeichen mehr)", "in Zeichen, nicht in Bytes");
    }

    /// ⚑ **Und eine Antwort ohne Denkblock bleibt, was sie war.**
    #[test]
    fn ohne_denkblock_aendert_sich_nichts() {
        let (schritte, antwort) = verlauf_aus(&[nachricht("assistant", "Schlicht geantwortet.")]);
        assert_eq!(antwort.as_deref(), Some("Schlicht geantwortet."));
        assert!(
            !schritte.iter().any(|s| matches!(s, Schritt::Denken(_))),
            "es wurde eine Ueberlegung erfunden"
        );
    }
}
