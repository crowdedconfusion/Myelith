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
//!
//! # ⚑ Und davor eine billigere Stufe: alte Werkzeugausgaben ausblenden
//!
//! (2026-10-13) Wird der Kontext voll, ersetzt der Lauf zuerst die
//! Werkzeugausgaben aelterer Schritte durch einen kurzen Vermerk
//! ([`ausblenden`]); erst wenn das nicht genug Luft schafft, fasst das
//! Modell zusammen. **Das Ausblenden kostet keinen Modellaufruf**, die
//! Ueberlegungen und Aufrufe des Modells bleiben woertlich, und der
//! Auftrag ohnehin. Vorbild ist die Messung von Lindenbauer und anderen
//! („The Complexity Trap", NeurIPS 2025): Bei Agenten auf
//! Programmieraufgaben loeste das blosse Ausblenden alter Beobachtungen
//! genauso viele Aufgaben wie eine Zusammenfassung durch das Modell, zur
//! halben Rechnung, und beides zusammen war am billigsten.
//!
//! ⚑ **Warum das hier besonders zaehlt:** Lokal kostet jedes Token im
//! Kontext Einlesezeit, und die faellt mit der Laenge (am 35B gemessen 125
//! Token je Sekunde bei 2 k, 47 bei 64 k). Ein kleiner Kontext ist hier
//! nicht nur billiger, sondern schneller.

use crate::tuerklient::{Kontextstand, Modellweg, Nachricht, Tuerfehler};
use crate::werkzeug::Werkzeugergebnis;

/// **Die Werkzeugausgaben so vieler letzter Schritte bleiben beim
/// Ausblenden woertlich.**
///
/// ⚑ **Drei, und das ist eine Abwaegung.** Die Vorlage liess zehn Runden
/// stehen, bei Kontexten von 128 k und mehr. Hier ist der Arbeitskontext
/// 64 k, und eine einzige gelesene Datei belegt leicht 5 000 Token; zehn
/// solche fuellten ihn fast allein. Drei decken, was das Modell gerade
/// vergleicht oder weiterbearbeitet.
pub const WOERTLICHE_SCHRITTE: usize = 3;

/// **Kuerzere Werkzeugausgaben bleiben stehen.** Ein Vermerk kostet selbst
/// rund 30 Token; eine Ausgabe von wenigen Zeilen (ein Exit-Code, „Datei
/// geschrieben") auszublenden, spart nichts und nimmt dem Modell ein
/// Ergebnis.
pub const AUSBLENDEN_AB_ZEICHEN: usize = 600;

/// Der Anfang des Vermerks, der an die Stelle einer ausgeblendeten Ausgabe
/// tritt; an ihm erkennt [`ausblenden`] eine schon ausgeblendete.
///
/// ⚑ **Englisch**, wie die Werkzeugansage und [`AUFTRAG`]: Er ist eine
/// Anweisung an das Modell.
pub const AUSGEBLENDET: &str = "[Output hidden to save context";

/// **Ersetzt die langen Werkzeugausgaben vor den letzten
/// [`WOERTLICHE_SCHRITTE`] Schritten durch einen Vermerk.** Zurueck kommen
/// die neuen Nachrichten und wie viele Ausgaben ausgeblendet wurden.
///
/// Angefasst werden nur Nachrichten ab `ab` mit der Rolle `tool`, deren
/// Form [`Werkzeugergebnis::nachricht`] ist; der Vermerk nennt das Werkzeug
/// und die Laenge der Ausgabe. Alles andere bleibt Zeichen fuer Zeichen,
/// und die Zahl der Nachrichten bleibt gleich, damit jeder Index in den
/// Verlauf (etwa der des Auftrags) weiter stimmt.
pub fn ausblenden(nachrichten: &[Nachricht], ab: usize) -> (Vec<Nachricht>, usize) {
    let schritte: Vec<usize> = (ab..nachrichten.len()).filter(|&i| nachrichten[i].role == "assistant").collect();
    let mut neu = nachrichten.to_vec();
    let Some(&grenze) = schritte.len().checked_sub(WOERTLICHE_SCHRITTE).and_then(|k| schritte.get(k)) else {
        return (neu, 0);
    };
    let mut zahl = 0;
    for n in neu.iter_mut().take(grenze).skip(ab).filter(|n| n.role == "tool") {
        let Some((name, inhalt)) = werkzeugausgabe(&n.content) else {
            continue;
        };
        let zeichen = inhalt.chars().count();
        if zeichen < AUSBLENDEN_AB_ZEICHEN || inhalt.starts_with(AUSGEBLENDET) {
            continue;
        }
        let vermerk = format!(
            "{AUSGEBLENDET}: {zeichen} characters from an earlier step. If you still need them, call the tool again.]"
        );
        // ⛔️ **Die Herkunft bleibt stehen** (2026-10-14): Ohne die Marke
        //   vorn waere ein ausgeblendetes fremdes Ergebnis in der naechsten
        //   Runde ein vertrautes (`secure_flow::label_aus_verlauf`).
        let vermerk = if crate::secure_flow::nennt_sich_fremd(&inhalt) {
            format!("{}\n{vermerk}", crate::secure_flow::fremdmarke(&name))
        } else {
            vermerk
        };
        *n = Werkzeugergebnis::nachricht(&name, &vermerk);
        zahl += 1;
    }
    (neu, zahl)
}

/// Name und Inhalt einer Werkzeugausgabe in der Form von
/// [`Werkzeugergebnis::nachricht`]; `None` fuer jede andere Form.
pub(crate) fn werkzeugausgabe(text: &str) -> Option<(String, String)> {
    let innen = text.strip_prefix("<tool_response>\n")?.strip_suffix("\n</tool_response>")?;
    let wert: serde_json::Value = serde_json::from_str(innen).ok()?;
    Some((wert.get("name")?.as_str()?.to_string(), wert.get("content")?.as_str()?.to_string()))
}

/// **Reicht das Ausblenden?** Nur wenn danach ausser der Antwortlaenge
/// noch ein Viertel des Kontexts frei ist.
///
/// ⚑ **Ein Viertel und nicht „passt gerade"**: Sonst stuende der Lauf
/// einen Schritt spaeter wieder an der Grenze, und jedes Ausblenden
/// aendert den Anfang des Prompts, den der KV-Speicher sonst weiterfuehrt
/// (Fund 372). Selten und gruendlich ist billiger als oft und knapp.
pub fn luft_genug(stand: Kontextstand, reserve: usize) -> bool {
    stand.passt(reserve.saturating_add(stand.grenze / 4))
}

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

/// **Kuerzt die Werkzeugantworten ab `ab` in der Mitte, die laengste
/// zuerst, bis der Verlauf mit `frei` Token Luft passt.** Gibt den Stand
/// danach zurueck, `None` ohne Auskunft ueber den Kontext.
///
/// ⚑ **Gekuerzt wird immer aus dem Original**, nicht aus einer schon
/// gekuerzten Fassung: Sonst stuenden Vermerke in Vermerken, und Anfang und
/// Ende schrumpften ungleich. Die Antworten des Modells bleiben woertlich.
///
/// ⛔️ **Passt es nicht, wenn jede Antwort bei 64 Zeichen steht**, ist es ein
/// benannter Fehler: Dann ist nicht die Ausgabe zu lang, sondern der Rest.
/// Fund 564.
pub fn werkzeugantworten_kuerzen(
    klient: &dyn Modellweg,
    nachrichten: &mut [Nachricht],
    ab: usize,
    frei: usize,
) -> Result<Option<Kontextstand>, Tuerfehler> {
    let stellen: Vec<usize> = (ab..nachrichten.len()).filter(|&i| nachrichten[i].role != "assistant").collect();
    let original: Vec<Nachricht> = stellen.iter().map(|&i| nachrichten[i].clone()).collect();
    let mut behalten: Vec<usize> = original.iter().map(|n| n.content.chars().count()).collect();
    loop {
        let Some(stand) = klient.kontext(nachrichten) else {
            return Ok(None);
        };
        if stand.passt(frei) {
            return Ok(Some(stand));
        }
        let Some(k) = (0..stellen.len()).filter(|&k| behalten[k] > 64).max_by_key(|&k| behalten[k]) else {
            return Err(Tuerfehler::KontextVoll { belegt: stand.belegt, grenze: stand.grenze });
        };
        behalten[k] = (behalten[k] / 2 + behalten[k] / 4).max(64);
        nachrichten[stellen[k]] = in_der_mitte_gekuerzt(&original[k], behalten[k]);
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
    // 📌 **Eingerahmt, und am Ende das Ganze verlangt** (2026-10-14, am
    //   echten Beispiel in der Konsole mit dem 4B): Ohne Rahmen endete der
    //   Text mit der letzten Antwort des Assistenten, und das 4B setzte
    //   genau diese fort. Aus vier Themen samt vier Teilenummern wurden drei
    //   Punkte ueber die letzte Frage (1 603 → 86 Token).
    text.push_str("<transcript>\n");
    for n in stueck {
        text.push_str("### ");
        text.push_str(&n.role);
        text.push('\n');
        text.push_str(&n.content);
        text.push_str("\n\n");
    }
    text.push_str("</transcript>\n\n");
    // 📌 **Und nicht abschreiben** (am selben Tag, Log von 120 Zeilen): Mit
    //   „keep every name, identifier and number exactly as written" schrieb
    //   das 4B den Verlauf samt Logdatei woertlich ab, bis die Tokengrenze
    //   ihn abschnitt; die Antworten des Assistenten fehlten danach ganz.
    text.push_str(
        "Condense the whole transcript above, from its first message to its last, not only the last one, \
         into short notes. Do not copy it; keep the names, identifiers and numbers that still matter.",
    );
    vec![Nachricht::system(AUFTRAG), Nachricht::nutzer(text)]
}

/// Die Nachricht, die an die Stelle des verdichteten Verlaufs tritt.
///
/// ⛔️ **`ersetzt` sind die Nachrichten, an deren Stelle sie tritt** (seit
/// dem 2026-10-14 Pflicht): Stand darin Fremdes, bekommt die
/// Zusammenfassung die Marke in die Zeile nach dem Kopf. Sonst wuesche das
/// Verdichten die Herkunft aus, denn die Belege verschwinden mit den
/// Nachrichten (`secure_flow::label_aus_verlauf`).
pub fn als_nachricht(zusammenfassung: &str, ersetzt: &[Nachricht]) -> Nachricht {
    let (label, _) = crate::secure_flow::label_aus_verlauf(ersetzt);
    if label.vertrauen == crate::secure_flow::Vertrauen::Fremd {
        let marke = crate::secure_flow::fremdmarke("summary of earlier steps");
        Nachricht::nutzer(format!("{KOPF}\n{marke}\n{zusammenfassung}"))
    } else {
        Nachricht::nutzer(format!("{KOPF}\n{zusammenfassung}"))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn schritt(i: usize, laenge: usize) -> [Nachricht; 2] {
        [
            Nachricht::modell(format!("Schritt {i}")),
            Werkzeugergebnis::nachricht("read_file", &format!("{i}{}", "d".repeat(laenge))),
        ]
    }

    /// ⚑ **Nur alte, lange Werkzeugausgaben verschwinden**: Die letzten
    /// [`WOERTLICHE_SCHRITTE`] Schritte, kurze Ausgaben, alles vor `ab` und
    /// alles, was keine Werkzeugausgabe ist, bleiben Zeichen fuer Zeichen,
    /// und die Zahl der Nachrichten bleibt gleich.
    #[test]
    fn alte_lange_ausgaben_werden_ausgeblendet_und_sonst_nichts() {
        let mut n = vec![Nachricht::system("Ansage"), Nachricht::nutzer("Auftrag")];
        n.extend(schritt(0, 2000));
        n.extend(schritt(1, 10));
        n.push(Nachricht::nutzer("x".repeat(2000)));
        for i in 2..6 {
            n.extend(schritt(i, 2000));
        }
        let (neu, zahl) = ausblenden(&n, 1);
        assert_eq!(neu.len(), n.len(), "die Indizes bleiben");
        assert_eq!(zahl, 2, "Schritt 0 und 2; 1 ist kurz, 3 bis 5 sind die letzten drei");
        for i in [3, 8] {
            let (name, inhalt) = werkzeugausgabe(&neu[i].content).expect("Form");
            assert_eq!(name, "read_file");
            assert!(inhalt.starts_with(AUSGEBLENDET) && inhalt.contains("2001 characters"), "{inhalt}");
        }
        for i in (0..n.len()).filter(|i| ![3, 8].contains(i)) {
            assert_eq!(neu[i], n[i], "Nachricht {i} wurde veraendert");
        }
        assert_eq!(ausblenden(&neu, 1), (neu.clone(), 0), "ein zweiter Durchgang findet nichts mehr");
        assert_eq!(ausblenden(&n[..8], 1).1, 0, "mit drei Schritten bleibt alles");
        assert_eq!(ausblenden(&n, 4).1, 1, "vor `ab` bleibt alles");
    }

    #[test]
    fn luft_genug_heisst_ein_viertel_frei() {
        assert!(luft_genug(Kontextstand { belegt: 700, grenze: 1000 }, 50));
        assert!(!luft_genug(Kontextstand { belegt: 701, grenze: 1000 }, 50));
    }
}
