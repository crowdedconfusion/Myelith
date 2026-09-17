//! Das Gespraech einer Sitzung: was ueber Auftraege hinweg mitgeht, wie
//! viel Kontext es belegt, und wie es verdichtet wird.
//!
//! # ⚑ Warum es das gibt (2026-09-14)
//!
//! Bis hierher stand im Agenten jeder Auftrag fuer sich (Entscheidung C2).
//! Eine Nachfrage wie „und jetzt dasselbe fuer die andere Datei" lief ins
//! Leere, und eine Anzeige des Kontexts haette nur den einen laufenden
//! Auftrag zeigen koennen. **Konsole und Fenster halten jetzt ein
//! Gespraech**, und beide benutzen dafuer diese eine Stelle: Wie gezaehlt
//! und wie verdichtet wird, soll nicht an zwei Orten verschieden werden.
//!
//! ⚑ **Schrittbudget und Belegkette bleiben je Auftrag.** Das Gespraech ist
//! Eingabe eines Laufs, keine Fortsetzung seines Vertrags; die Begruendung
//! steht an `Lauf::fahren_mit_verlauf`.

use myl_local_agent::verdichtung;
use myl_local_agent::{Modellweg, Nachricht, Tuerfehler};

/// Die Nachrichten, die ueber Auftraege hinweg mitgehen.
///
/// ⚑ **Ohne Werkzeugansage.** Die gehoert dem jeweiligen Lauf und steht
/// dort vorn; im Gespraech veraltete sie, sobald sich die Werkzeuge
/// aendern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gespraech {
    nachrichten: Vec<Nachricht>,
}

impl Gespraech {
    /// Ein leeres Gespraech.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Aus vorhandenen Nachrichten, etwa aus dem Speicher des Fensters.
    pub fn aus(nachrichten: Vec<Nachricht>) -> Self {
        Self { nachrichten: ohne_system(&nachrichten) }
    }

    pub fn nachrichten(&self) -> &[Nachricht] {
        &self.nachrichten
    }

    pub fn ist_leer(&self) -> bool {
        self.nachrichten.is_empty()
    }

    /// **Uebernimmt die Nachrichten eines beendeten Laufs.**
    ///
    /// ⚑ **Ersetzt und haengt nicht an**: Der Lauf hat das bisherige
    /// Gespraech schon vor seinem Auftrag stehen, und hat er verdichtet,
    /// steht dort die Zusammenfassung statt der alten Nachrichten.
    pub fn nach_dem_lauf(&mut self, nachrichten: &[Nachricht]) {
        self.nachrichten = ohne_system(nachrichten);
    }

    /// Vergisst alles.
    pub fn leeren(&mut self) {
        self.nachrichten.clear();
    }
}

fn ohne_system(nachrichten: &[Nachricht]) -> Vec<Nachricht> {
    nachrichten.iter().filter(|n| n.role != "system").cloned().collect()
}

/// Was die Kontextanzeige zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Kontextanzeige {
    /// Token von Ansage und Gespraech zusammen.
    pub belegt: usize,
    /// Positionen des Modells.
    pub grenze: usize,
    /// Davon die Werkzeugansage.
    pub ansage: usize,
    /// Wie viele Nachrichten das Gespraech hat.
    pub nachrichten: usize,
    /// Belegter Anteil in ganzen Prozent.
    pub prozent: usize,
}

/// **Wie viel Kontext Ansage und Gespraech belegen**, so gezaehlt, wie das
/// Modell sie als Prompt saehe. `None`, wenn das Modell es nicht weiss.
pub fn anzeige(modell: &dyn Modellweg, ansage: Option<&Nachricht>, gespraech: &Gespraech) -> Option<Kontextanzeige> {
    let mut alle: Vec<Nachricht> = ansage.cloned().into_iter().collect();
    alle.extend(gespraech.nachrichten.iter().cloned());
    let stand = modell.kontext(&alle)?;
    let ansage = match ansage {
        Some(a) => modell.kontext(std::slice::from_ref(a)).map(|s| s.belegt).unwrap_or(0),
        None => 0,
    };
    Some(Kontextanzeige {
        belegt: stand.belegt,
        grenze: stand.grenze,
        ansage,
        nachrichten: gespraech.nachrichten.len(),
        prozent: stand.prozent(),
    })
}

/// **Die Zusammenfassung, mit der das Gespraech beginnt**, falls es
/// verdichtet wurde.
///
/// ⚑ Das Fenster legt sie ab, damit ein verdichtetes Gespraech nach einem
/// Neustart nicht wieder mit dem ganzen alten Verlauf beginnt; die
/// uebrigen Nachrichten leitet es aus den Beitraegen her.
pub fn zusammenfassung(gespraech: &Gespraech) -> Option<String> {
    gespraech
        .nachrichten
        .first()
        .filter(|n| n.role == "user" && n.content.starts_with(verdichtung::KOPF))
        .map(|n| n.content.clone())
}

/// Die Werkzeugansage, wie ein Lauf mit dieser Ruestung sie vorn stehen hat.
pub fn ansage(ruestung: &crate::ruestung::Ruestung) -> Nachricht {
    myl_local_agent::werkzeug::angebot(ruestung.kasten.angebote(), ruestung.form)
}

/// **Verdichtet das ganze Gespraech zu einer Zusammenfassung.** Zurueck
/// kommen die belegten Token davor und danach, ohne Ansage.
///
/// ⚑ **Das Gespraech bleibt unberuehrt, wenn es schiefgeht.** Eine halbe
/// Verdichtung waere ein Verlust ohne Gegenwert.
pub fn verdichten(modell: &dyn Modellweg, gespraech: &mut Gespraech) -> Result<(usize, usize), Tuerfehler> {
    verdichten_mit_mitschnitt(modell, gespraech, None, "", "")
}

/// **Wie [`verdichten`], und legt den Verlauf vorher als Mitschnitt ab.**
///
/// # ⛔️ Warum das zusammengehoert
///
/// `verdichten` **ersetzt** die Nachrichten durch die Zusammenfassung,
/// und die Urfassung ist danach weg. Genau in dem Augenblick, in dem
/// sie verschwindet, ist sie noch da: **Das ist die einzige Stelle, an
/// der ein Mitschnitt vollstaendig sein kann.** Wer ihn spaeter
/// schreiben wollte, schriebe die Zusammenfassung ab.
///
/// `wurzel` ist der eingehaengte Arbeitsordner; ohne ihn wird nichts
/// abgelegt, und das ist kein Fehler: Ein Klient ohne Arbeitsordner hat
/// keinen Ort, an dem der Agent nachlesen koennte.
///
/// ⚠️ **Ein Mitschnitt, der nicht geschrieben werden kann, haelt die
/// Verdichtung nicht auf.** Der Kontext ist voll, und das ist das
/// dringendere Problem; der Fehler geht als Text zurueck.
pub fn verdichten_mit_mitschnitt(
    modell: &dyn Modellweg,
    gespraech: &mut Gespraech,
    wurzel: Option<&std::path::Path>,
    sitzung: &str,
    modellname: &str,
) -> Result<(usize, usize), Tuerfehler> {
    let mitschnitt = wurzel.and_then(|w| {
        if gespraech.ist_leer() {
            return None;
        }
        let abschnitte: Vec<(String, String)> = gespraech
            .nachrichten()
            .iter()
            .map(|n| (n.role.clone(), n.content.clone()))
            .collect();
        match crate::verlauf::schreiben(w, sitzung, modellname, &abschnitte) {
            Ok(name) => {
                // ⚑ **Das Verzeichnis kommt in die Zusammenfassung und
                // nicht hinter einen Werkzeugaufruf** (2026-09-16).
                //
                // Der erste Entwurf gab es auf Anfrage heraus, und dafuer
                // brauchte das Werkzeug einen **optionalen** Parameter:
                // ohne Argumente das Verzeichnis, mit ihnen die Zeilen.
                // Das bricht die Regel, dass kein Werkzeug einen
                // optionalen Parameter hat, und die Regel hat recht.
                //
                // ⚑ **Hier steht es besser als dort.** Die
                // Zusammenfassung ist das Einzige, was der naechste
                // Schritt sieht; ein Verzeichnis darin kostet keinen
                // Aufruf, und das Werkzeug behaelt genau eine Aufgabe.
                // **Es ist ausserdem klein:** eine Zeile je Nachricht,
                // gegen den ganzen Verlauf, den es ersetzt.
                let t = verweis(w, &name);
                Some(Ok((name, t)))
            }
            Err(f) => Some(Err(f.to_string())),
        }
    });
    let ergebnis = verdichten_roh(modell, gespraech);

    // ⚑ **Der Hinweis steht in der Zusammenfassung selbst**, nicht
    // daneben: Sie ist das Einzige, was der naechste Schritt des Modells
    // zu sehen bekommt. Ein Verweis, der nicht im Kontext steht, ist
    // keiner.
    if let (Ok(_), Some(Ok((_name, verzeichnis)))) = (&ergebnis, &mitschnitt) {
        if let Some(erste) = gespraech.nachrichten.first_mut() {
            erste.content.push_str(verzeichnis);
        }
    }
    ergebnis
}

/// **Der Satz, mit dem die Zusammenfassung auf den Mitschnitt zeigt**,
/// samt Verzeichnis.
///
/// ⚑ **Oeffentlich, damit die Messung denselben Text benutzt wie der
/// Betrieb.** Eine Probe, die sich den Verweis selbst nachbaut, misst
/// ihren eigenen Nachbau: Aendert sich hier ein Wort, liefe sie
/// weiterhin gruen und sagte nichts mehr ueber das Programm aus. **Das
/// ist dieselbe Angabe an zwei Orten, und dieses Projekt hat damit
/// genug Erfahrung.**
pub fn verweis(wurzel: &std::path::Path, name: &str) -> String {
    let mut t = format!(
        "\n\nDer vollstaendige Verlauf liegt unter `{name}`. \
         Wer eine Einzelheit braucht, die oben fehlt, liest sie mit \
         `read_history` an den genannten Zeilen nach; `list_history` \
         nennt das vollstaendige Verzeichnis:\n"
    );
    for ab in &abschnitte_verzeichnis(wurzel, name) {
        t.push_str(ab);
        t.push('\n');
    }
    // ⚑ **Und was an Wissen bereitliegt** (2026-09-17). Die Verdichtung
    // ist der Augenblick, in dem der Kontext geleert wird; genau dann
    // gehoert der Hinweis dorthin, wo das Modell als Naechstes hinsieht.
    // **Ein Verweis, der nicht im Kontext steht, ist keiner.**
    if let Some(v) = crate::skills::verweis(Some(wurzel)) {
        t.push_str(&v);
        t.push('\n');
    }
    t
}

fn verdichten_roh(modell: &dyn Modellweg, gespraech: &mut Gespraech) -> Result<(usize, usize), Tuerfehler> {
    let vorher = modell.kontext(&gespraech.nachrichten);
    let Some(grenze) = vorher.map(|s| s.grenze) else {
        return Err(Tuerfehler::KontextVoll { belegt: 0, grenze: 0 });
    };
    if gespraech.ist_leer() {
        return Ok((0, 0));
    }
    let text = verdichtung::zusammenfassen(modell, "lokal", &gespraech.nachrichten, verdichtung::antwortlaenge(grenze))?;
    let neu = vec![verdichtung::als_nachricht(&text)];
    let nachher = modell.kontext(&neu).map(|s| s.belegt).unwrap_or(0);
    gespraech.nachrichten = neu;
    Ok((vorher.map(|s| s.belegt).unwrap_or(0), nachher))
}

/// **Wie viele Zeilen das Verzeichnis in der Zusammenfassung hoechstens
/// hat.**
///
/// ⚑ **Eine feste Zahl, und genau das ist der Punkt.** Sie haengt nicht
/// an der Laenge des Gespraechs, also kostet das Verzeichnis immer
/// gleich viel. Sechzehn Zeilen sind ueberschaubar und kosten rund 300
/// Token; wer genauer sucht, ruft `list_history`.
const VERZEICHNISZEILEN: usize = 16;

/// Die Zeilen des Verzeichnisses, so wie sie in der Zusammenfassung
/// stehen.
///
/// ⚑ **Gelesen und nicht aus den Abschnitten noch einmal gerechnet.**
/// Zwei Rechnungen derselben Zeilennummern liefen auseinander, und die
/// zweite waere die, auf die sich das Modell verlaesst.
///
/// ⛔️ **Und hoechstens [`VERZEICHNISZEILEN`] Zeilen** (Fund 387): Eine
/// Zeile je Nachricht waechst genauso schnell wie das, was sie ersetzen
/// soll. Gemessen wog das Verzeichnis eines Verlaufs aus 120
/// Nachrichten 3 597 Token, und die verdichtete Fassung war groesser
/// als das Original.
fn abschnitte_verzeichnis(wurzel: &std::path::Path, name: &str) -> Vec<String> {
    let pfad = wurzel.join(name);
    // ⚠️ Ohne Verzeichnis bleibt der Verweis auf die Datei stehen: Ein
    // Hinweis ohne Zeilennummern ist weniger als einer mit, aber mehr
    // als keiner.
    crate::verlauf::grobverzeichnis(&pfad, VERZEICHNISZEILEN).unwrap_or_default()
}

/// **Ein Balken aus Blockzeichen**, `breite` Zeichen, mit Achteln am Rand.
pub fn balken(belegt: usize, grenze: usize, breite: usize) -> String {
    const ACHTEL: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let achtel = (belegt.min(grenze).saturating_mul(breite * 8)).checked_div(grenze).unwrap_or(0);
    let voll = achtel / 8;
    let mut s: String = std::iter::repeat_n('█', voll).collect();
    if voll < breite {
        s.push(if achtel % 8 == 0 { '░' } else { ACHTEL[achtel % 8] });
        s.extend(std::iter::repeat_n('░', breite - voll - 1));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Modellweg, der in Zeichen zaehlt und jede Verdichtung mit `KURZ`
    /// beantwortet.
    struct Zeichen(usize);

    impl Modellweg for Zeichen {
        fn chat(&self, _m: &str, _n: &[Nachricht], _t: Option<u32>) -> Result<myl_local_agent::Antwort, Tuerfehler> {
            Ok(myl_local_agent::Antwort {
                text: " KURZ ".to_string(),
                abschlussgrund: None,
                kennung: String::new(),
                segment: None,
                prompt_token: 0,
                antwort_token: 0,
            })
        }
        fn kontext(&self, n: &[Nachricht]) -> Option<myl_local_agent::Kontextstand> {
            Some(myl_local_agent::Kontextstand { belegt: n.iter().map(|m| m.content.len()).sum(), grenze: self.0 })
        }
    }

    #[test]
    fn nach_dem_lauf_steht_das_gespraech_ohne_ansage() {
        let mut g = Gespraech::neu();
        g.nach_dem_lauf(&[Nachricht::system("Ansage"), Nachricht::nutzer("a"), Nachricht::modell("b")]);
        assert_eq!(g.nachrichten(), &[Nachricht::nutzer("a"), Nachricht::modell("b")]);
        g.nach_dem_lauf(&[Nachricht::system("Ansage"), Nachricht::nutzer("z")]);
        assert_eq!(g.nachrichten(), &[Nachricht::nutzer("z")], "ersetzt und haengt nicht an");
    }

    #[test]
    fn die_anzeige_zaehlt_ansage_und_gespraech() {
        let g = Gespraech::aus(vec![Nachricht::nutzer("12345"), Nachricht::modell("123")]);
        let a = anzeige(&Zeichen(40), Some(&Nachricht::system("12")), &g).expect("bekannt");
        assert_eq!((a.belegt, a.ansage, a.nachrichten, a.grenze, a.prozent), (10, 2, 2, 40, 25));
    }

    /// ⛔️ **Die Naht zwischen Verdichten und Mitschnitt, und sie ist der
    /// Grund, warum es beides gibt.**
    ///
    /// Ein Modul, das niemand ruft, ist das haeufigste Fehlerbild dieses
    /// Projekts, und hier waere es besonders still: Der Mitschnitt
    /// entstuende nicht, die Verdichtung gelaenge trotzdem, und niemand
    /// merkte es, bis jemand nachlesen will und nichts findet.
    ///
    /// ⚑ **Geprueft wird dreierlei:** dass die Datei entsteht, dass sie
    /// den **vollstaendigen** Verlauf traegt (nicht die Zusammenfassung,
    /// die ihn ersetzt), und dass die Zusammenfassung selbst auf sie
    /// zeigt. **Ein Verweis, der nicht im Kontext steht, ist keiner**:
    /// Die Zusammenfassung ist das Einzige, was der naechste Schritt des
    /// Modells sieht.
    #[test]
    fn das_verdichten_legt_den_verlauf_ab_und_sagt_wo() {
        let d = std::env::temp_dir()
            .join(format!("myl-verdichten-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");

        // ⚑ **Die zweite Nachricht hat zwei Zeilen**, und das ist
        // Absicht: Nur so laesst sich zeigen, dass in die
        // Zusammenfassung das **Verzeichnis** kommt und nicht der
        // Verlauf. Die erste Zeile jedes Abschnitts ist seine Marke im
        // Verzeichnis; die zweite darf dort nicht stehen.
        let mut g = Gespraech::aus(vec![
            Nachricht::nutzer("Die Kennzahl lautet 4711 und steht in der Tabelle."),
            Nachricht::modell("Verstanden.\nDie Nebenzahl ist 0815 und gehoert nicht ins Verzeichnis."),
        ]);
        verdichten_mit_mitschnitt(&Zeichen(10_000), &mut g, Some(&d), "probe-1", "myelith-0.6b")
            .expect("verdichtet");

        let mitschnitte = crate::verlauf::vorhandene(&d);
        assert_eq!(mitschnitte.len(), 1, "der Mitschnitt entsteht nicht");

        // ⚑ **Die Zusammenfassung zeigt darauf**, mit dem Ordnernamen.
        let text = &g.nachrichten()[0].content;
        assert!(text.contains(crate::verlauf::ORDNER), "die Zusammenfassung nennt den Ordner nicht: {text}");
        assert!(text.contains("KURZ"), "die Zusammenfassung selbst fehlt: {text}");

        // ⛔️ **Und sie traegt das Verzeichnis**, sonst wuesste das
        // Modell nicht, welche Zeilen es verlangen kann, und das
        // Werkzeug braeuchte doch einen optionalen Parameter.
        assert!(
            text.contains("Die Kennzahl lautet") && text.contains("Verstanden."),
            "das Verzeichnis fehlt in der Zusammenfassung: {text}"
        );
        // ⚑ **Eine Zeile je Nachricht, nicht der ganze Verlauf.** Die
        // zweite Zeile der zweiten Nachricht ist keine Marke und gehoert
        // deshalb nicht hinein.
        assert!(
            !text.contains("Die Nebenzahl ist 0815"),
            "der Verlauf selbst steht in der Zusammenfassung statt nur sein Verzeichnis: {text}"
        );

        // ⛔️ **Und im Mitschnitt steht der ganze Verlauf**, nicht die
        // Zusammenfassung: Sonst waere die Ablage eine zweite Kopie
        // dessen, was ohnehin im Kontext steht.
        let v = crate::verlauf::verzeichnis(&mitschnitte[0]).expect("Verzeichnis");
        assert_eq!(v.nachrichten, 2, "der Mitschnitt traegt nicht den ganzen Verlauf");
        assert_eq!(v.modell, "myelith-0.6b");
        let alles = crate::verlauf::zeilen(&mitschnitte[0], 1, v.zeilen).expect("Zeilen");
        assert!(alles.contains("4711"), "die Kennzahl fehlt im Mitschnitt: {alles}");
        // ⛔️ **Und die Zeile, die nicht im Verzeichnis steht, steht im
        // Mitschnitt**: Genau dafuer gibt es ihn.
        assert!(alles.contains("0815"), "die Nebenzahl fehlt im Mitschnitt: {alles}");

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn verdichten_ersetzt_das_gespraech_durch_die_zusammenfassung() {
        let mut g = Gespraech::aus(vec![Nachricht::nutzer("x".repeat(50)), Nachricht::modell("y".repeat(50))]);
        let (vorher, nachher) = verdichten(&Zeichen(10_000), &mut g).expect("verdichtet");
        assert_eq!(g.nachrichten(), &[verdichtung::als_nachricht("KURZ")]);
        assert_eq!(vorher, 100);
        assert_eq!(nachher, verdichtung::als_nachricht("KURZ").content.len());
        let mut leer = Gespraech::neu();
        assert_eq!(verdichten(&Zeichen(10), &mut leer), Ok((0, 0)));
    }

    #[test]
    fn eine_zusammenfassung_wird_erkannt() {
        let mut g = Gespraech::aus(vec![Nachricht::nutzer("x"), Nachricht::modell("y")]);
        assert_eq!(zusammenfassung(&g), None);
        verdichten(&Zeichen(10_000), &mut g).expect("verdichtet");
        assert_eq!(zusammenfassung(&g), Some(verdichtung::als_nachricht("KURZ").content));
        let falsch = Gespraech::aus(vec![Nachricht::modell(verdichtung::als_nachricht("KURZ").content)]);
        assert_eq!(zusammenfassung(&falsch), None, "nur als Nutzernachricht am Anfang");
    }

    #[test]
    fn der_balken_ist_immer_gleich_breit() {
        for (belegt, grenze) in [(0, 100), (1, 100), (50, 100), (99, 100), (100, 100), (300, 100), (5, 0)] {
            assert_eq!(balken(belegt, grenze, 20).chars().count(), 20, "{belegt}/{grenze}");
        }
        assert_eq!(balken(50, 100, 4), "██░░");
        assert_eq!(balken(100, 100, 4), "████");
        assert_eq!(balken(0, 100, 4), "░░░░");
    }
}
