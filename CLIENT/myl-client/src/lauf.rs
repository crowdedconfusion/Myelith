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
//! | den Lauf nachbauen | ⛑ Zwei Wege zu derselben Sache laufen auseinander, und der zweite ist der schlechter geprueete |
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
    /// Ein Werkzeugaufruf, wie das Modell ihn vorschlug.
    Aufruf {
        /// Der Name des Werkzeugs.
        name: String,
        /// Die Argumente, schon lesbar gemacht.
        argumente: String,
    },
    /// Ein Vorschlag, den niemand lesen konnte.
    Unlesbar(String),
    /// Was ein Werkzeug zurueckgab.
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
    /// ⛑ **`None` ist ein Befund und kein Nichts:** Der Lauf ist in
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
/// ⛑ **Panik statt Fehlerwert**, und das ist hier vertretbar: Die Werte
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
    let anfang = std::time::Instant::now();
    let grenzen = myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen::neu(
        kontrakt_fuer(schritte),
        ruestung.kasten.angebote(),
    );
    let zuordnung = ruestung.zuordnung();
    let erg = myl_local_agent::schleife::Lauf {
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
    }
    .fahren(auftrag);

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
                let rufe = myl_local_agent::werkzeug::vorschlaege(&n.content);
                if rufe.is_empty() {
                    let t = n.content.trim();
                    if !t.is_empty() {
                        letzte = Some(t.to_string());
                    }
                    continue;
                }
                let dazwischen = ohne_aufrufe(&n.content);
                if !dazwischen.is_empty() {
                    aus.push(Schritt::Plan(dazwischen));
                }
                for r in &rufe {
                    match r {
                        Ok(v) => aus.push(Schritt::Aufruf {
                            name: v.name.clone(),
                            argumente: kurzform(&v.arguments),
                        }),
                        Err(u) => aus.push(Schritt::Unlesbar(u.roh.clone())),
                    }
                }
            }
            "tool" => aus.push(Schritt::Ergebnis(eine_zeile(&n.content, 200))),
            _ => {}
        }
    }
    if let Some(t) = &letzte {
        aus.push(Schritt::Antwort(t.clone()));
    }
    (aus, letzte)
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
