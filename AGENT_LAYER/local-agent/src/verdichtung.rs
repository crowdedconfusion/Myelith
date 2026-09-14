//! Verdichten: einen Verlauf, der nicht mehr in den Kontext passt, vom
//! Modell selbst zusammenfassen lassen.
//!
//! # ⚑ Warum das Modell zusammenfasst und nicht ein Abschneiden
//!
//! Die einfachste Antwort auf einen vollen Kontext waere, die aeltesten
//! Nachrichten wegzulassen. **Dann vergisst der Agent genau das, was am
//! Anfang festgelegt wurde**: welche Datei gemeint war, was schon versucht
//! wurde, was der Nutzer ausgeschlossen hat. Eine Zusammenfassung behaelt
//! das in wenigen hundert Token.
//!
//! # ⚑ Warum erst am Rand und nicht nach jedem Schritt
//!
//! Der Vorschlag lag nahe, den Verlauf nach **jedem** Schritt zu verdichten
//! und nur noch die Zusammenfassung weiterzugeben. Er kostet mehr, als er
//! spart: Der KV-Speicher setzt ein Gespraech fort, dessen Anfang gleich
//! bleibt (Fund 372), und ein Schritt rechnet dann nur, was neu ist. Eine
//! Zusammenfassung je Schritt aendert den Anfang jedes Mal, erzwingt jedes
//! Mal eine neue Vorbereitung des ganzen Prompts und dazu einen eigenen
//! Modellaufruf, und sie verliert bei jedem Durchgang etwas.
//! **Verdichtet wird deshalb erst, wenn der naechste Schritt nicht mehr
//! hineinpasst.**
//!
//! # ⚑ Und was es fuer die Belegkette heisst
//!
//! Die Zusammenfassung ist kein Schritt: Sie schlaegt kein Werkzeug vor und
//! zaehlt nicht gegen das Schrittbudget. **Gebunden ist sie trotzdem**:
//! Der naechste Schritt haelt ein Commitment ueber die Nachrichten fest,
//! die er dem Modell gegeben hat, und die Zusammenfassung ist eine davon.
//! Wer den Verlauf vorlegt, kann damit nicht behaupten, das Modell habe
//! eine andere gesehen.

use crate::tuerklient::{Modellweg, Nachricht, Tuerfehler};

/// Die Anweisung an das Modell.
///
/// ⚑ **Englisch**, wie die amtliche Werkzeugansage: Die Modelle folgen
/// Anweisungen in ihrer Vorlagensprache am zuverlaessigsten. Die
/// Zusammenfassung selbst steht in der Sprache des Gespraechs.
pub const AUFTRAG: &str = "You condense part of the transcript of an ongoing conversation between \
a user and an assistant that uses tools, so that the conversation can continue without it. Keep \
every concrete fact the assistant may still need: the user's goals, instructions and \
constraints, exact file names, paths, identifiers, numbers, commands and error messages, \
decisions made, what was tried and what it showed, and the next steps. Leave out pleasantries \
and material that no longer matters. Write in the language of the user's messages, as short \
plain-text notes without a preamble.";

/// Der Kopf, unter dem eine Zusammenfassung im Verlauf steht.
pub const KOPF: &str = "[Zusammenfassung des bisherigen Verlaufs]";

/// **Fasst `alt` zu einem Text zusammen.**
///
/// ⚑ **Passt der Verlauf selbst nicht in den Kontext, wird er in Stuecken
/// gelesen**: so viele Nachrichten, wie mit der bisherigen Zusammenfassung
/// hineinpassen, dann die naechsten dazu. Jede Runde nimmt mindestens eine
/// Nachricht, also endet es.
///
/// # 📌 Und eine Nachricht, die allein nicht passt, wird gekuerzt
///
/// Gemessen am 2026-09-14 am 4B mit 2 048 Positionen: Eine eingefuegte
/// Notiz von 7 800 Zeichen passte mit der Anweisung nicht in eine Runde.
/// Der erste Entwurf meldete dann einen vollen Kontext, und **das Gespraech
/// blieb fuer immer voll**, denn gerade die Verdichtung, die es haette
/// leeren sollen, scheiterte an ihr. Jetzt wird sie **in der Mitte**
/// gekuerzt, mit Vermerk, bis sie passt: Anfang und Ende einer Nachricht
/// tragen meist, worum es geht. [`Tuerfehler::KontextVoll`] bleibt nur,
/// wenn schon die Anweisung allein nicht hineinpasst.
///
/// `antwort_token` ist die Laenge, die der Zusammenfassung zusteht, und
/// wird beim Zuschnitt der Stuecke freigehalten.
pub fn zusammenfassen(
    klient: &dyn Modellweg,
    modell: &str,
    alt: &[Nachricht],
    antwort_token: u32,
) -> Result<String, Tuerfehler> {
    // 📌 **Die Teile werden aneinandergereiht und nicht noch einmal
    // zusammengefasst.** Der erste Entwurf gab jeder Runde die bisherige
    // Zusammenfassung mit und liess sie neu schreiben; gemessen am 4B fiel
    // dabei nach drei Runden genau das heraus, was am Anfang festgelegt
    // war (Datei und gesperrter Pfad), und die Nachfrage danach nannte eine
    // erfundene Datei. Jetzt sieht eine Runde den vorigen Teil nur als
    // Zusammenhang.
    let mut teile: Vec<String> = Vec::new();
    let mut i = 0;
    while i < alt.len() {
        let bisher = teile.last().cloned();
        let anfrage = |bis: usize| anfrage(bisher.as_deref(), &alt[i..bis]);
        let passt = |bis: usize| {
            klient.kontext(&anfrage(bis)).is_none_or(|s| s.passt(antwort_token as usize))
        };
        if !passt(i + 1) {
            let einzeln = kuerzen_bis_es_passt(klient, &alt[i], antwort_token as usize, |n| {
                self::anfrage(bisher.as_deref(), std::slice::from_ref(n))
            })?;
            let antwort = klient.chat(modell, &self::anfrage(bisher.as_deref(), &[einzeln]), Some(antwort_token))?;
            teile.push(antwort.text.trim().to_string());
            i += 1;
            continue;
        }
        let mut bis = i + 1;
        while bis < alt.len() && passt(bis + 1) {
            bis += 1;
        }
        let antwort = klient.chat(modell, &anfrage(bis), Some(antwort_token))?;
        teile.push(antwort.text.trim().to_string());
        i = bis;
    }
    Ok(teile.join("\n\n"))
}

/// **Kuerzt eine Nachricht in der Mitte, bis die daraus gebaute Anfrage
/// mit `frei` Token Luft in den Kontext passt.**
///
/// Die Laenge wird aus dem Verhaeltnis geschaetzt und danach halbiert,
/// falls die Schaetzung nicht reicht; unter 64 Zeichen gibt es auf.
pub fn kuerzen_bis_es_passt(
    klient: &dyn Modellweg,
    nachricht: &Nachricht,
    frei: usize,
    bauen: impl Fn(&Nachricht) -> Vec<Nachricht>,
) -> Result<Nachricht, Tuerfehler> {
    let zeichen = nachricht.content.chars().count();
    let mut behalten = zeichen;
    let mut kandidat = nachricht.clone();
    loop {
        let Some(stand) = klient.kontext(&bauen(&kandidat)) else {
            return Ok(kandidat);
        };
        if stand.passt(frei) {
            return Ok(kandidat);
        }
        if behalten <= 64 {
            return Err(Tuerfehler::KontextVoll { belegt: stand.belegt, grenze: stand.grenze });
        }
        let geschaetzt = behalten.saturating_mul(stand.grenze.saturating_sub(frei)) / stand.belegt.max(1);
        behalten = geschaetzt.min(behalten / 2 + behalten / 4).max(64.min(behalten / 2));
        kandidat = in_der_mitte_gekuerzt(nachricht, behalten);
    }
}

/// Anfang und Ende einer Nachricht, zusammen `behalten` Zeichen, mit einem
/// Vermerk dazwischen.
pub fn in_der_mitte_gekuerzt(nachricht: &Nachricht, behalten: usize) -> Nachricht {
    let zeichen: Vec<char> = nachricht.content.chars().collect();
    if zeichen.len() <= behalten {
        return nachricht.clone();
    }
    let vorn = behalten / 2;
    let hinten = behalten - vorn;
    let ausgelassen = zeichen.len() - behalten;
    let mut text: String = zeichen[..vorn].iter().collect();
    text.push_str(&format!("\n[… {ausgelassen} Zeichen ausgelassen …]\n"));
    text.extend(&zeichen[zeichen.len() - hinten..]);
    Nachricht { role: nachricht.role.clone(), content: text }
}

/// Die Nachrichten einer Verdichtungsrunde.
fn anfrage(bisher: Option<&str>, stueck: &[Nachricht]) -> Vec<Nachricht> {
    let mut text = String::new();
    if let Some(b) = bisher {
        text.push_str("Notes on the part just before (already kept, do not repeat them):\n");
        text.push_str(b);
        text.push_str("\n\nThe transcript continues:\n\n");
    }
    for n in stueck {
        text.push_str("### ");
        text.push_str(&n.role);
        text.push('\n');
        text.push_str(&n.content);
        text.push_str("\n\n");
    }
    text.push_str("Condense this part of the transcript.");
    vec![Nachricht::system(AUFTRAG), Nachricht::nutzer(text)]
}

/// Die Nachricht, die an die Stelle des verdichteten Verlaufs tritt.
pub fn als_nachricht(zusammenfassung: &str) -> Nachricht {
    Nachricht::nutzer(format!("{KOPF}\n{zusammenfassung}"))
}

/// Wie viele Token einer Zusammenfassung zustehen: ein Achtel des Kontexts,
/// hoechstens 2 048.
///
/// ⚑ **Ein Achtel**, damit nach dem Verdichten mindestens drei Viertel
/// frei sind; **hoechstens 2 048**, weil eine laengere Zusammenfassung
/// kein Verdichten mehr ist.
pub fn antwortlaenge(grenze: usize) -> u32 {
    (grenze / 8).clamp(64, 2048) as u32
}
