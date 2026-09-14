//! Interaktives Menü: „Durchklicken" statt Befehle tippen.
//!
//! Wird gestartet, wenn `myl-test` **ohne Unterbefehl** aufgerufen wird.
//! Der Grund: Die Hardwaretests laufen auf fremden Maschinen, oft von
//! Leuten, die das Projekt nicht täglich sehen. Wer erst eine Hilfeseite
//! lesen muss, um einen Testlauf zu starten, führt ihn seltener aus.
//!
//! Ausgewählt wird mit den Pfeiltasten und Enter; die Ziffer daneben
//! bleibt gültig. Wo kein Terminal vorhanden ist: in einer Pipe, in
//! einem Skript, in einer seriellen Konsole ohne Rohmodus , fällt die
//! Auswahl auf zeilenweise Eingabe zurück. Das Verfahren steht in
//! [`crate::auswahl`]; hier stehen nur die Punkte.


use std::path::PathBuf;

use crate::auswahl::{self, Punkt};
use crate::logging::RunLog;
use crate::{banner, runs, stack};

/// Laufeinstellungen, die im Menü verändert werden können.
///
/// **Artefakt und Testdatei sind Auswahlzustände, keine Vorgaben**
/// (2026-08-22). Beim Start ist beides `None`, und die Übersicht sagt
/// „nicht ausgewählt". Vorher zeigte der Client auf ein Artefakt, das
/// jemand vielleicht nie gewählt hatte, und auf die eingebauten Prompts:
/// Das sah aus wie eine Entscheidung, war aber eine Annahme. Wer den
/// Testlauf startete, maß dann möglicherweise etwas anderes als der
/// Vergleichspartner, ohne dass ihm eine Frage gestellt worden wäre.
///
/// Der Testlauf fragt genau das ab, was fehlt, und nichts weiter: Ist
/// beides gewählt, läuft er sofort los.
pub struct Einstellungen {
    pub prompts: Vec<String>,
    pub steps: usize,
    pub shards: usize,
    /// Das gewählte Artefakt. `None`, solange keines gewählt wurde.
    pub artifacts: Option<PathBuf>,
    /// Die gewählte Testdatei (ihre Kennung). `None`, solange keine
    /// gewählt wurde; dann laufen die eingebauten Vorgabewerte, und der
    /// Lauf trägt die Einstellungs-Kennung `ohne-plan`.
    pub testdatei: Option<String>,
    pub logs: PathBuf,
    /// Kurzkennung der Einstellungen: benennt das Protokollverzeichnis.
    /// `ohne-plan`, solange kein Testplan geladen wurde.
    pub einstellungen_id: String,
    /// Name des Teilnehmers. Steht im Protokoll und im Dateinamen.
    pub teilnehmer: String,
    /// Wie oft jeder Prompt im Determinismuslauf gerechnet wird.
    ///
    /// Zwei ist die Vorgabe und das Minimum: Ein einzelner Lauf hat
    /// nichts, womit er sich vergleichen ließe. Höhere Werte sind für
    /// Langläufe gedacht und suchen sporadische Abweichungen, die bei
    /// zwei Läufen durchrutschen (Speicherfehler, thermisches Drosseln).
    pub wiederholungen: usize,
}


/// Das Nutzermenü: nur, was jeder Teilnehmer braucht.
///
/// Wer eine Maschine beisteuert, soll messen, vergleichen und das
/// Ergebnis schicken; er soll keine Testpläne erzeugen und keine Pfade
/// umstellen. Alles Weitere liegt eine Ebene tiefer unter [9].
///
/// **In der Reihenfolge des Ablaufs (2026-08-22).** Modell, Testdatei,
/// Lauf: die drei Schritte, die jeder Teilnehmer in dieser Folge geht.
/// Vorher stand das Gespräch mit dem Modell an erster und das Artefakt an
/// vierter Stelle, also das Ergebnis vor seiner Voraussetzung: Wer [1]
/// wählte, ohne ein Artefakt zu haben, bekam als Erstes eine
/// Modellauswahl, die er nicht erwartet hatte.
///
/// **Das Gespräch steht hinter dem Lauf**, obwohl man es davor führen
/// mag. Es ist der einzige Punkt, der **nicht misst**, und ein Menü
/// ordnet nach Aufgabe, nicht nach Neugier. Wer es sucht, findet es; wer
/// den Test fahren soll, stolpert nicht darüber.
///
/// Die vier Schritte sind durch eine Leerzeile von den drei
/// Nebenfunktionen abgesetzt. Sieben gleichrangige Zeilen lesen sich wie
/// sieben Möglichkeiten; vier plus drei lesen sich wie ein Weg mit
/// Beiwerk, und das ist es auch.
/// Die Stufen des Sammellaufs, in der Reihenfolge, in der sie laufen.
///
/// Je Eintrag ein **kurzer** Name für den Bildschirm und ein
/// **vollständiger** für die Protokollzeile. Der kurze steht im Menü, wo
/// die Zeile schmal ist; der vollständige im Protokoll, wo später jemand
/// lesen muss, was gemeint war.
///
/// ⚑ **Eine Quelle, weil drei getippte Fassungen auseinandergelaufen
/// sind.** Als die Konformität am 2026-08-27 fünfte Stufe wurde, zog nur
/// die Protokollzeile nach. Der Menüpunkt [3] versprach dem Teilnehmer
/// weiter vier Stufen, und die Kurzanleitung ebenfalls: Wer dem Menü
/// folgte, hielt die fünfte für einen Fehler. Der Zusammenhang ist
/// seither **hergestellt statt geprüft** — die Beschreibung des
/// Menüpunkts entsteht aus dieser Liste, und [`stufe`] greift auf sie zu,
/// sodass eine sechste Stufe ohne Eintrag beim ersten Lauf auffällt.
pub(crate) const STUFEN: [(&str, &str); 7] = [
    ("Hardware", "Hardware"),
    ("Determinismus", "Determinismus (Einzelknoten)"),
    ("Shards", "Geshardete Inferenz"),
    ("Protokoll-Durchlauf", "Protokoll-Durchlauf"),
    ("Konformität", "Konformität"),
    // ⚑ **Seit dem 2026-09-04, und sie ist die zweite Hälfte der
    // Kernthese.** Die fünf davor belegen, dass zwei Maschinen dieselbe
    // Inferenz rechnen; bezahlte Trainingsarbeit ist ohne diese Stufe
    // unprüfbar.
    ("Trainingsschritt", "Trainingsschritt (letzte Ebene)"),
    // ⚑ **Seit dem 2026-09-14.** Die Stufen davor rechnen auf dem
    // schnellsten Weg dieser Maschine; diese rechnet dieselbe Arbeit auf
    // jedem Weg, den sie bietet, und verlangt überall denselben Abdruck.
    ("Rechenwege", "Rechenwege dieser Maschine"),
];

/// Die Protokollzeile einer Stufe: `Stufe 3 von 5: Geshardete Inferenz`.
///
/// **Zählt ab eins**, weil die Zeile für Menschen ist. Ein Aufruf
/// jenseits der Liste bricht ab, und das ist Absicht: Eine Stufe, die
/// niemand in [`STUFEN`] eingetragen hat, fehlt auch im Menü und in der
/// Kurzanleitung. Lieber beim ersten Lauf laut als still im Text.
fn stufe(nr: usize) -> String {
    let (_, lang) = STUFEN[nr - 1];
    format!("Stufe {} von {}: {}", nr, STUFEN.len(), lang)
}




/// Das deutsche Zahlwort für die Stufenzahl.
///
/// Nur für die Kurzanleitung, die als fester Text auf den Bildschirm
/// gerechnet ist und deshalb nicht erzeugt wird. Ein Test verbindet
/// beide: Er verlangt, dass dort dieses Wort steht. Deshalb `cfg(test)`
/// — im Programm selbst gibt es keinen Aufrufer, und ein toter wäre
/// unter `-D warnings` ein Baufehler.
#[cfg(test)]
fn stufen_zahlwort() -> &'static str {
    match STUFEN.len() {
        1 => "eine",
        2 => "zwei",
        3 => "drei",
        4 => "vier",
        5 => "fünf",
        6 => "sechs",
        7 => "sieben",
        8 => "acht",
        _ => "mehrere",
    }
}


/// Das Entwickler-Menü: **nur die Auswertung.**
///
/// # ⚑ Warum hier nur noch ein Punkt steht (2026-09-11)
///
/// Dieses Menü führte acht Punkte: vergleichen, Testplan erzeugen,
/// Artefakte prüfen, Einstellungen ändern, umbenennen, Plattenplatz
/// freigeben, einen Knoten betreiben, einen Netzlauf auswerten.
///
/// **Sieben davon waren Werkzeuge, keine Menüpunkte.** Wer sie braucht,
/// weiß, was er sucht, und findet sie auf der Befehlszeile, wo der
/// Aufruf in einem Skript stehen und in einem Ticket zitiert werden
/// kann. Im Menü kostete jeder von ihnen eine Zeile Aufmerksamkeit bei
/// jemandem, der etwas anderes wollte.
///
/// **Der eine Punkt, der bleibt, ist der, für den es dieses Menü
/// gibt:** die zugesandten Ergebnisse gegenüberstellen und beurteilen,
/// ob sie den Nachweis über verschiedene Hardware tragen. Das ist die
/// Tätigkeit des Koordinators, und sie ist die einzige, die eine
/// Auswahl braucht (eigene Läufe oder zugesandte).
///
/// ⚠️ **Nichts ist dabei verlorengegangen.** Was hier stand, ist
/// erreichbar als `myl-test plan`, `artefakte`, `netz`, `teilnehmen`,
/// `anlaufstelle` und `aufraeumen`.
fn menue_entwickler() -> Vec<Punkt> {
    vec![
        Punkt::neu(
            '1',
            "Ergebnisse auswerten und Bericht schreiben",
            "Die zugesandten Läufe gegenüberstellen und urteilen, ob sie\n\
             den Nachweis über verschiedene Hardware tragen. Der Punkt,\n\
             für den es dieses Menü gibt.",
        ),
        Punkt::neu('0', "Beenden", ""),
    ]
}

/// Kürzt lange Pfade auf die letzten drei Bestandteile.
///
/// **Das Trennzeichen der Plattform, auch im Auslassungszeichen.**
/// Vorher stand hier ein festes `…/`. Der Rest entsteht aus
/// `PathBuf::collect`, und das setzt das Trennzeichen des Systems; unter
/// Windows kam deshalb `…/d\e\f` heraus, also beide Zeichen in einer
/// Zeile. Gefunden hat es der Windows-Job der CI, beim ersten Lauf, den
/// es ihn je gab.
///
/// Entschieden für die Plattform und gegen einen festen Schrägstrich,
/// weil dieser Pfad **nur angezeigt** wird und nie in ein Protokoll
/// wandert. Ginge er ins Protokoll, wäre die Antwort umgekehrt.
fn kurz(p: &std::path::Path) -> String {
    let teile: Vec<_> = p.components().collect();
    if teile.len() <= 3 {
        return p.display().to_string();
    }
    let rest: PathBuf = teile[teile.len() - 3..].iter().collect();
    format!("…{}{}", std::path::MAIN_SEPARATOR, rest.display())
}

/// Startet den Client: Animation, Name, Prüfstand.
///
/// # ⚑ Ein Weg, keine Wahl (2026-09-11, Festlegung des Projektinhabers)
///
/// Hier stand bis zu diesem Tag ein Menü mit sechs Punkten. Zwei davon
/// waren Entscheidungen, die ein Teilnehmer nicht treffen kann, weil
/// sie auf **allen** Maschinen gleich ausfallen müssen: welches
/// Artefakt und welcher Testplan. Wer sie anders traf, lieferte ein
/// Protokoll, das der Vergleich in eine eigene Gruppe legte, und die
/// Maschine hatte umsonst gerechnet.
///
/// **Der Grund, warum das Menü überhaupt bestand, ist weggefallen.**
/// Es trug auch das Ausprobieren: mit dem Modell sprechen, ein anderes
/// holen, Plattenplatz freigeben. Dafür gibt es seit CLIENT v0.17.0
/// einen eigenen Client samt Konsolenfassung. **Der Testclient ist ein
/// Messgerät, und ein Messgerät hat einen Knopf.**
///
/// Der Ablauf, in dieser Reihenfolge und ohne Zwischenfrage:
///
/// 1. Animation, Logo, Namenseingabe.
/// 2. Der Prüfstand läuft durch ([`crate::ablauf::fahren`]).
/// 3. Das Ergebnis liegt in `TESTCLIENT/Ergebnisse/`, bereit zum
///    Verschicken.
/// 4. Enter schliesst.
///
/// Die **einzige** Rückfrage dazwischen: Fehlt das Artefakt, wird
/// gefragt, bevor 2,5 GB geholt werden.
///
/// ⚑ **`Admin` als Name führt ins Entwicklermenü**, und das enthält
/// seit heute nur noch die Auswertung. Siehe [`ist_entwickler`].
pub fn run(mut e: Einstellungen) -> bool {
    // Erstes Bild: Animation, dann Logo und Namenseingabe, sonst nichts.
    //
    // Der Name steht vor allem anderen, weil er die Ergebnisdatei
    // benennt, die in dieser Sitzung entsteht: nachträglich umbenennen
    // müsste sie sonst der Koordinator.
    // Das Farbschema der Sitzung wird hier zum ersten Mal abgerufen und
    // damit gewürfelt: während das Logo aus der Spirale entsteht.
    let farbe = crate::farben::logo();
    banner::start_if_mit(true, farbe);
    if e.teilnehmer == crate::logging::OHNE_NAME {
        e.teilnehmer = namen_erfragen();
    }
    // Aufräumen, Logo stehen lassen, dann die Begrüßung darunter. Die
    // Eingabezeile mit dem getippten Namen hat ihren Zweck erfüllt und
    // stünde sonst über der Antwort darauf.
    banner::bildschirm_mit(farbe);
    crate::animation::begruessung(&e.teilnehmer, farbe);

    if ist_entwickler(&e.teilnehmer) {
        return entwickler_menue(&mut e, true);
    }

    pruefstand_fahren(&e.teilnehmer)
}

/// Der Prüfstandslauf mit seinem Rahmen: Ansage, Lauf, Abschluss.
///
/// Getrennt von [`run`], damit die Reihenfolge der Bilder prüfbar
/// bleibt, ohne eine Animation zu starten.
fn pruefstand_fahren(name: &str) -> bool {
    println!();
    for zeile in ANSAGE.lines() {
        println!("{zeile}");
    }
    println!();

    let ausgang = crate::ablauf::fahren(name, &mut |t| println!("{t}"));

    println!();
    if ausgang.bestanden {
        println!("  Alle Stufen bestanden.");
    } else {
        // ⚑ **Kein Ausrufezeichen und keine Entschuldigung.** Eine
        // Abweichung ist der interessante Fall, nicht der Unfall: Genau
        // dafür läuft dieser Test. Das Ergebnis wird trotzdem
        // verschickt, und zwar gerade dann.
        println!("  Mindestens eine Stufe ist abgewichen.");
        println!("  Das ist ein Befund, kein Fehler deinerseits: Bitte trotzdem schicken.");
    }
    if let Some(pfad) = &ausgang.ergebnis {
        println!("
  Ergebnis: {}", kurz(pfad));
        println!("  Diese Datei geht an den Koordinator.");
    }

    println!("
  Enter schliesst das Fenster.");
    let mut _w = String::new();
    let _ = std::io::stdin().read_line(&mut _w);
    ausgang.bestanden
}

/// Was vor dem Lauf auf dem Schirm steht.
///
/// **Kurz, und es sagt, was gleich geschieht.** Wer einen Messlauf
/// startet, will wissen, wie lange er wartet und ob er etwas tun muss.
/// Beides steht hier, sonst nichts.
const ANSAGE: &str = "  Es läuft jetzt alles durch, ohne weitere Fragen.

  Gemessen wird, ob diese Maschine dieselben Bits rechnet wie jede
  andere. Das dauert je nach Rechner einige Minuten; Strg-C bricht ab,
  dann ist das Ergebnis unvollständig und muss wiederholt werden.";

/// Fragt den Namen, unter dem die Protokolle dieser Sitzung laufen.
///
/// **Ein Name, keine Kennung.** Er beschreibt den Teilnehmer, nicht die
/// Maschine, die steht ohnehin gemessen im Protokoll. Wer den Namen
/// leer lässt, bekommt `ohne-name`: sichtbar fehlend statt stillschweigend
/// geraten. Ein aus der Umgebung übernommener Benutzername wäre bequemer
/// und zugleich eine Personenangabe, die niemand angeordnet hat, und
/// Protokolle wandern per Copy-Paste in Tickets.
/// Ob dieser Name das Entwickler-Menü freischaltet.
///
/// # Warum das Menü überhaupt verborgen ist
///
/// Das Entwickler-Menü enthält Punkte, die ein Teilnehmer nicht braucht
/// und mit denen er sich schaden kann: Artefakte löschen, Testpläne
/// erzeugen, einen Knoten als Anlaufstelle betreiben. Ein Menü, das
/// alles zeigt, lädt zum Ausprobieren ein, und ein Teilnehmer, der aus
/// Versehen 25 GB löscht, hat einen schlechten ersten Eindruck.
///
/// **Es ist kein Schutz, sondern eine Aufräumhilfe.** Wer den Namen
/// kennt, kommt hinein, und der Name steht hier im Quelltext. Gegen
/// jemanden, der etwas anrichten will, hilft das nicht; gegen einen
/// vollen Bildschirm mit Punkten, die niemanden angehen, schon.
///
/// Groß- und Kleinschreibung ist gleichgültig: `Admin`, `admin` und
/// `ADMIN` öffnen dasselbe, und `AdMiN` ebenfalls, weil eine
/// Unterscheidung dort nur Verdruss stiftete.
pub fn ist_entwickler(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("admin")
}

fn namen_erfragen() -> String {
    // Der Text steht als Block mittig unter dem Schriftzug, die
    // Eingabezeile am linken Rand dieses Blocks. Nur so bleibt sie dort,
    // wo das Auge sie nach dem Lesen erwartet; zentriert stünde der Cursor
    // je nach getipptem Namen an einer anderen Stelle.
    let text = "  Unter welchem Nutzernamen sollen die Protokolle dieser Sitzung laufen?\n  \
                Er steht im Dateinamen und im Protokoll, damit der Koordinator sie\n  \
                ohne Rückfrage zuordnen kann. Leer lassen ist erlaubt.";
    println!("{}\n", banner::zentriert(text));

    let einzug = banner::blockeinzug(
        text.lines().map(|z| z.chars().count()).max().unwrap_or(0),
    );
    let eingabe = auswahl::frage(&format!("{}  Nutzername: ", einzug)).unwrap_or_default();
    let name = eingabe.trim();
    if name.is_empty() {
        println!(
            "\n{}\n",
            banner::zentriert(&format!(
                "  Kein Name, die Protokolle laufen unter {:?}.",
                crate::logging::OHNE_NAME
            ))
        );
        return crate::logging::OHNE_NAME.to_string();
    }
    name.to_string()
}

fn ja_nein(b: bool) -> &'static str {
    if b {
        "OK"
    } else {
        "FEHLGESCHLAGEN"
    }
}












/// Die Entwickler-Ebene. Kehrt mit dem letzten Ergebnis zurück.
fn entwickler_menue(e: &mut Einstellungen, mut letztes_ergebnis: bool) -> bool {
    banner::bildschirm();
    loop {
        // ⚑ **Ohne Einstellungs-Fuss.** Er zeigte Prompt, Token, Shards
        // und Pfade, also lauter Werte, die seit dem festen Prüfstand
        // niemand mehr ändern kann. Ein Zustand, den man nur ansehen und
        // nicht beeinflussen kann, gehört nicht unter eine Auswahl.
        let Some(wahl) = auswahl::waehlen("Entwickler", &menue_entwickler()) else {
            banner::bildschirm();
            return letztes_ergebnis;
        };
        println!();
        match wahl {
            '1' => letztes_ergebnis = vergleichen(e),
            '0' => {
                banner::bildschirm();
                return letztes_ergebnis;
            }
            _ => {}
        }
        weiter();
    }
}



/// Die Stufen selbst, mit bereits geklärten Einstellungen.
///
/// # ⚑ Warum das vom Menü getrennt ist (2026-09-04)
///
/// Der Sammellauf war nur über das Menü erreichbar, und auf einer
/// Mietmaschine sitzt niemand davor: Man verbindet sich über SSH, tippt
/// einen Befehl und liest das Protokoll. **Ein Messverfahren, das eine
/// Tastatur voraussetzt, ist auf einer stundenweise gemieteten Maschine
/// eine Fehlerquelle**, denn jeder Handgriff mehr ist einer, der beim
/// zweiten Rechner anders ausfällt.
///
/// Das Menü fragt nach, was fehlt, und ruft dann hierher; `myl-test
/// testlauf` reicht die Einstellungen aus den Aufrufparametern durch.
/// **Eine Umsetzung, zwei Eingänge**, siehe Fund 178.
/// ⚑ **Das Protokoll kommt von aussen, und zwar genau eins.** Wer es
/// hier anlegte, schriebe für den Aufruf über die Befehlszeile ein
/// zweites: eine leere Datei mit demselben Namensmuster, die der
/// Vergleich als zweiten Lauf derselben Maschine liest. Ein Testlauf
/// mit zwei Protokollen ist eins zuviel.
pub fn stufen_fahren(e: &Einstellungen, artefakt: PathBuf, log: RunLog) -> bool {
    let mut log = log;

    // Nur auf den Bildschirm, nicht ins Protokoll: Der Hinweis richtet
    // sich an den Menschen davor, und er kommt VOR der ersten Stufe, weil
    // er danach nicht mehr hilft. Ein Abbruch ist erlaubt, er kostet nur
    // den ganzen Lauf: Ein Protokoll ohne Abschlusseintrag wird vom
    // Vergleich als unvollständig geführt und trägt keinen Nachweis.
    log.nur_anzeigen(
        "  Der Lauf läuft jetzt durch. Strg-C bricht ihn ab; das Protokoll ist dann\n           unvollständig und muss wiederholt werden.\n",
    );

    log.note(stufe(1));
    let hardware = runs::run_hardware(&mut log);

    log.note(stufe(2));
    let determinismus =
        runs::run_determinism(&mut log, &artefakt, &e.prompts, e.steps, e.wiederholungen);

    log.note(stufe(3));
    let shard = runs::run_shard(&mut log, &artefakt, &e.prompts, e.steps, e.shards);

    log.note(stufe(4));
    let stapel = stack::run_stack(&mut log);

    // Die Konformität läuft gegen dasselbe Artefakt wie der Rest des
    // Laufs: Passt es nicht zu den Layer-/E2E-Vektoren, überspringt sie
    // diese Stufe mit Begründung, statt blind zu laden.
    log.note(stufe(5));
    let konformitaet = crate::konformitaet::laufen(&mut log, Some(&artefakt));

    // ⚑ **Nach der Konformität und nicht davor.** Weicht schon ein
    // Rückwärtskern gegen sein festes Soll ab, ist die Ursache benannt,
    // und der Trainingsabdruck sagt darüber nichts Neues. Umgekehrt ist
    // ein abweichender Abdruck bei stimmender Konformität die
    // interessante Lage: die Kerne rechnen gleich, der Weg als Ganzes
    // nicht.
    log.note(stufe(6));
    let training = crate::training::laufen(&mut log, &artefakt);

    log.note(stufe(7));
    let rechenwege = crate::rechenwege::laufen(&mut log, &artefakt);

    println!(
        "\n  Gesamt: Hardware {}, Determinismus {}, Shards {}, Stack {}, Konformität {}, Training {}, Rechenwege {}",
        ja_nein(hardware),
        ja_nein(determinismus),
        ja_nein(shard),
        ja_nein(stapel),
        ja_nein(konformitaet),
        ja_nein(training),
        ja_nein(rechenwege)
    );
    log.finish(hardware && determinismus && shard && stapel && konformitaet && training && rechenwege)
}

/// Wartet auf einen Tastendruck und räumt danach den Bildschirm auf.
///
/// **Der Gegenpart zum Aufräumen.** Ohne das Warten verschwände die
/// Ausgabe eines Laufs in dem Augenblick, in dem sie fertig ist, der
/// Nutzer sähe das Ergebnis nie. Mit ihm bleibt sie stehen, solange er
/// sie liest, und er entscheidet, wann weitergegangen wird.
///
/// **Das Aufräumen gehört hierher, nicht an den Anfang der Menüschleife.**
/// So folgt auf **jeden** Tastendruck ein sauberer Bildschirm, gleich von
/// welcher Stelle aus gewartet wurde: aus dem Nutzermenü, aus dem
/// Entwicklermenü, nach einem Untermenü. Lag es am Schleifenanfang, blieb
/// jeder Pfad ungedeckt, der nicht dorthin zurückkehrt.
///
/// Ohne Terminal wird nicht gewartet: Ein Skript hat niemanden, der eine
/// Taste drückt, und würde stillstehen.
fn weiter() {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }
    let hinweis = "  ── Weiter mit einer beliebigen Taste ──";
    println!("\n{}", banner::zentriert(hinweis));
    let _ = std::io::Write::flush(&mut std::io::stdout());
    if let Ok(roh) = auswahl::Rohmodus::an() {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(k))
                    if k.kind == crossterm::event::KeyEventKind::Press =>
                {
                    break
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        drop(roh);
    }
    banner::bildschirm();
}








fn vergleichen(e: &mut Einstellungen) -> bool {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let zugesandt = crate::vergleich::vergleichsordner(&repo);
    let berichte = crate::vergleich::berichtsordner(&repo);

    let punkte = vec![
        Punkt::neu(
            '1',
            "Zugesandte Protokolle",
            &format!(
                "Was im Ordner {} liegt, der Weg des Koordinators.",
                crate::vergleich::ORDNER
            ),
        ),
        Punkt::neu(
            '2',
            "Eigene Läufe",
            "Die Protokolle dieser Maschine. Ergibt für sich keinen\n\
             Nachweis, zeigt aber, ob wiederholte Läufe übereinstimmen.",
        ),
        Punkt::neu('0', "Zurück", ""),
    ];

    match auswahl::waehlen("Welche Protokolle vergleichen?", &punkte) {
        Some('1') => crate::vergleich::run(&zugesandt, Some(&berichte)),
        Some('2') => crate::vergleich::run(&e.logs, Some(&berichte)),
        _ => true,
    }
}

/// Kürzel für den n-ten Eintrag einer erzeugten Liste.
///
/// Ziffern zuerst, danach Buchstaben. Ohne dieses Kürzel hätte ein Eintrag
/// jenseits des neunten nur den Weg über die Pfeiltasten, und der Weg
/// über die Tastenkürzel soll nicht ab dem zehnten Eintrag verschwinden.
fn kuerzel(i: usize) -> char {
    match i {
        0..=8 => char::from(b'1' + i as u8),
        9..=34 => char::from(b'a' + (i - 9) as u8),
        _ => ' ',
    }
}

/// Gibt Plattenplatz frei: Artefakte und heruntergeladene Gewichte.
///
/// ## Warum getrennt gefragt wird
///
/// Artefakte und Gewichte sind verschieden teuer wiederzubeschaffen.
/// Artefakte entstehen aus dem versionierten Skalenpaket in Sekunden; die
/// Gewichte kosten einen Download über mehrere Gigabyte. Wer Platz
/// braucht und den Test später wiederholen will, gibt deshalb die
/// Artefakte frei und behält die Gewichte. Ein einziger Punkt „alles
/// löschen" nähme ihm diese Wahl.
///
/// ## Warum eine getippte Bestätigung
///
/// Löschen ist der einzige Vorgang in diesem Client, der etwas zerstört.
/// Eine Auswahlliste, in der ein Pfeiltastendruck zuviel ein 15-GB-Modell
/// löscht, wäre die falsche Bedienung dafür. Verlangt wird deshalb ein
/// getipptes „ja", die eine Stelle, an der Enter allein nicht genügt.
/// ⚑ **Seit dem 2026-09-11 von der Befehlszeile aus** (`myl-test
/// aufraeumen`). Der Punkt stand im Entwicklermenü, und das führt jetzt
/// nur noch die Auswertung. **Verloren gehen sollte er nicht:** Er ist
/// der einzige Weg, die bis zu 46 GB wieder freizugeben, die ein
/// Testlauf an Gewichten und Artefakten hinterlässt, und die Anleitung
/// verweist in Abschnitt A8 darauf.
pub fn freigeben() {
    let repo = crate::artefakte::repo_wurzel(std::env::current_dir().unwrap_or_default());
    let belegung: Vec<crate::artefakte::Belegung> = crate::artefakte::belegung(&repo)
        .into_iter()
        .filter(|b| b.belegt())
        .collect();

    if belegung.is_empty() {
        println!("  Auf dieser Maschine liegen weder Artefakte noch Gewichte.");
        return;
    }

    let gesamt: u64 = belegung.iter().map(|b| b.bytes()).sum();
    println!("  Belegt auf dieser Maschine: {}\n", crate::artefakte::groesse(gesamt));

    let mut punkte = Vec::new();
    let mut ziele: Vec<(String, std::path::PathBuf)> = Vec::new();
    for b in &belegung {
        if let Some((pfad, bytes)) = &b.artefakte {
            punkte.push(Punkt::neu(
                kuerzel(ziele.len()),
                &format!(
                    "{} · Artefakte · {}",
                    b.modell,
                    crate::artefakte::groesse(*bytes)
                ),
                "Aus dem Skalenpaket in Sekunden wiederherstellbar.",
            ));
            ziele.push((format!("{} (Artefakte)", b.modell), pfad.clone()));
        }
        if let Some((pfad, bytes)) = &b.gewichte {
            punkte.push(Punkt::neu(
                kuerzel(ziele.len()),
                &format!(
                    "{} · Gewichte · {}",
                    b.modell,
                    crate::artefakte::groesse(*bytes)
                ),
                "Erneut zu holen kostet einen Download über Hugging Face.",
            ));
            ziele.push((format!("{} (Gewichte)", b.modell), pfad.clone()));
        }
    }
    // Alles auf einmal, als eigener Punkt am Ende der Liste.
    //
    // **Warum überhaupt.** Wer eine Maschine für einen Test zur Verfügung
    // gestellt hat und danach seine 25 GB zurückhaben will, soll nicht
    // sechsmal dasselbe Menü durchlaufen. Der Punkt steht bewusst **unten**
    // und nicht oben: Er ist der folgenreichste der Liste, und die erste
    // Zeile ist die, auf der die Markierung beim Öffnen steht.
    //
    // Das Kürzel kommt aus derselben Folge wie die Einträge darüber, statt
    // ein sprechendes 'a' zu setzen: `kuerzel(9)` **ist** 'a', und ab zehn
    // Einträgen träfen zwei Punkte auf dieselbe Taste. Ein Menü, in dem
    // eine Taste zwei Bedeutungen hat, ist ein Fehler, der erst auf einer
    // fremden Maschine mit vielen Modellen auffiele.
    let alles = kuerzel(ziele.len());
    punkte.push(Punkt::neu(
        alles,
        &format!("ALLES löschen · {}", crate::artefakte::groesse(gesamt)),
        "Artefakte und Gewichte aller Modelle. Fragt zweimal nach.",
    ));
    punkte.push(Punkt::neu('0', "Nichts löschen", ""));

    let Some(wahl) = auswahl::waehlen("Was freigeben?", &punkte) else {
        return;
    };

    if wahl == alles {
        alles_freigeben(&repo, &ziele, gesamt);
        return;
    }

    let Some(index) = punkte.iter().position(|p| p.taste == wahl) else {
        return;
    };
    if index >= ziele.len() {
        println!("  Nichts gelöscht.");
        return;
    }

    let (was, pfad) = &ziele[index];
    println!("\n  Löschen: {}", was);
    println!("  Pfad:    {}", pfad.display());
    println!("  Das lässt sich nicht rückgängig machen.\n");

    if !bestaetigt("  Zum Bestätigen \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    match crate::artefakte::freigeben(&repo, pfad) {
        Ok(bytes) => println!("\n  {} freigegeben.", crate::artefakte::groesse(bytes)),
        Err(e) => println!("\n  {}", e),
    }
}

/// Eine getippte Bestätigung. Nur ein ausgeschriebenes „ja" zählt.
///
/// Kein Menüpunkt und keine einzelne Taste: Eine Auswahl lässt sich mit
/// einem versehentlichen Enter bestätigen, ein Wort nicht.
fn bestaetigt(frage: &str) -> bool {
    auswahl::frage(frage)
        .unwrap_or_default()
        .trim()
        .eq_ignore_ascii_case("ja")
}

/// Löscht Artefakte und Gewichte aller Modelle.
///
/// **Zwei Bestätigungen, und die zweite ist die eigentliche.** Die erste
/// fragt, ob gelöscht werden soll; die zweite nennt jeden betroffenen Pfad
/// einzeln und verlangt danach noch einmal ein „ja". Der Grund ist die
/// Reichweite: Ein Fehlgriff kostet hier einen Download von bis zu 25 GB,
/// und das ist für jemanden mit langsamer Leitung ein verlorener Abend.
///
/// Die Liste zwischen den beiden Fragen ist kein Zierat. Ohne sie
/// bestätigte man zweimal dieselbe Zahl; mit ihr sieht man, was tatsächlich
/// verschwindet, und kann beim zweiten Mal begründet abbrechen.
fn alles_freigeben(repo: &std::path::Path, ziele: &[(String, std::path::PathBuf)], gesamt: u64) {
    println!(
        "\n  ALLES löschen: {} Einträge, zusammen {}.",
        ziele.len(),
        crate::artefakte::groesse(gesamt)
    );
    println!("  Das lässt sich nicht rückgängig machen.\n");

    if !bestaetigt("  Erste Bestätigung, \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    println!("\n  Betroffen sind:");
    for (was, pfad) in ziele {
        println!("    {} · {}", was, pfad.display());
    }
    println!(
        "\n  Gewichte kosten danach einen erneuten Download über Hugging Face;\n  \
         Artefakte sind aus dem Skalenpaket in Sekunden wiederhergestellt.\n"
    );

    if !bestaetigt("  Zweite Bestätigung, nochmals \"ja\" eintippen: ") {
        println!("\n  Abgebrochen, nichts gelöscht.");
        return;
    }

    let mut frei = 0u64;
    let mut fehler = 0usize;
    for (was, pfad) in ziele {
        match crate::artefakte::freigeben(repo, pfad) {
            Ok(bytes) => {
                frei += bytes;
                println!("  gelöscht: {}", was);
            }
            // Weitermachen statt abbrechen: Ein Eintrag, der sich nicht
            // löschen lässt, soll die übrigen nicht am Freiwerden hindern.
            Err(e) => {
                fehler += 1;
                println!("  FEHLER bei {}: {}", was, e);
            }
        }
    }

    println!("\n  {} freigegeben.", crate::artefakte::groesse(frei));
    if fehler > 0 {
        println!("  {} Eintrag/Einträge blieben liegen, siehe oben.", fehler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Das Entwicklermenü führt genau einen Punkt und den Ausgang.**
    ///
    /// 📌 Die Probe, an der die Festlegung des Projektinhabers vom
    /// 2026-09-11 hängt. Vorher standen hier acht Punkte, sieben davon
    /// Werkzeuge, die ein Koordinator selten und ein Teilnehmer nie
    /// braucht. Wer einen davon wieder einträgt, bricht diese Probe.
    #[test]
    fn das_entwicklermenue_fuehrt_nur_die_auswertung() {
        let punkte = menue_entwickler();
        assert_eq!(punkte.len(), 2, "das Menü führt {} Punkte", punkte.len());
        assert!(
            punkte[0].titel.to_lowercase().contains("auswerten"),
            "der erste Punkt ist nicht die Auswertung: {:?}",
            punkte[0].titel
        );
        assert_eq!(punkte[1].taste, '0', "der letzte Punkt ist nicht der Ausgang");
    }

    /// **Kein Menüpunkt führt mehr zu einem Messlauf.**
    ///
    /// ⚑ Der Prüfstand läuft nach der Namenseingabe von selbst. Ein
    /// zweiter Weg dorthin wäre ein zweiter Ablauf, und zwei Abläufe
    /// laufen auseinander.
    #[test]
    fn kein_menuepunkt_startet_einen_messlauf() {
        for p in menue_entwickler() {
            let t = p.titel.to_lowercase();
            for verboten in ["testlauf", "determinismus", "prüfstand", "messen"] {
                assert!(
                    !t.contains(verboten),
                    "{:?} führt zu einem Messlauf",
                    p.titel
                );
            }
        }
    }

    /// Tasten müssen eindeutig sein, sonst greift die zweite nie.
    #[test]
    fn tasten_sind_eindeutig() {
        let punkte = menue_entwickler();
        let mut tasten: Vec<char> = punkte.iter().map(|p| p.taste).collect();
        tasten.sort_unstable();
        let vorher = tasten.len();
        tasten.dedup();
        assert_eq!(vorher, tasten.len(), "eine Taste ist doppelt vergeben");
    }

    /// **Der Zugang hängt am Namen, und nur an ihm.**
    #[test]
    fn das_entwicklermenue_haengt_am_namen() {
        for ja in ["admin", "Admin", "ADMIN", "AdMiN", "  admin  "] {
            assert!(ist_entwickler(ja), "{ja:?} sollte hineinführen");
        }
        for nein in ["anna", "administrator", "admin2", "", "ohne-name"] {
            assert!(!ist_entwickler(nein), "{nein:?} sollte nicht hineinführen");
        }
    }

    /// **Die Ansage passt in achtzig Spalten.**
    ///
    /// Sie steht unter dem Logo auf einem aufgeräumten Bildschirm; eine
    /// umbrechende Zeile schöbe das Logo nach oben weg.
    #[test]
    fn die_ansage_passt_in_achtzig_spalten() {
        for zeile in ANSAGE.lines() {
            assert!(
                zeile.chars().count() <= 80,
                "{} Zeichen: {zeile:?}",
                zeile.chars().count()
            );
        }
    }

    /// **Die Ansage sagt, was gleich geschieht und wie man abbricht.**
    ///
    /// ⚑ Ein Lauf ohne Rückfrage muss vorher sagen, dass er ohne
    /// Rückfrage läuft. Sonst wartet jemand auf eine Frage, die nicht
    /// kommt, und hält den Lauf für hängengeblieben.
    #[test]
    fn die_ansage_nennt_ablauf_und_abbruch() {
        assert!(ANSAGE.contains("ohne weitere Fragen"), "die Ansage verspricht keinen Durchlauf");
        assert!(ANSAGE.contains("Strg-C"), "die Ansage nennt den Abbruch nicht");
        assert!(ANSAGE.contains("Bits"), "die Ansage sagt nicht, was gemessen wird");
    }

    /// Die Menüpunkte passen in achtzig Spalten, Titel wie Hinweis.
    #[test]
    fn menuepunkte_passen_in_achtzig_spalten() {
        for p in menue_entwickler() {
            for zeile in p.hinweis.lines().chain(std::iter::once(p.titel.as_str())) {
                assert!(
                    zeile.chars().count() <= 74,
                    "{} Zeichen: {zeile:?}",
                    zeile.chars().count()
                );
            }
        }
    }

    /// **Jede Stufe hat eine Protokollzeile, und die Zahl darin stimmt.**
    #[test]
    fn jede_stufe_nennt_sich_und_ihre_zahl() {
        for nr in 1..=STUFEN.len() {
            let z = stufe(nr);
            assert!(z.starts_with(&format!("Stufe {nr} von {}", STUFEN.len())), "{z}");
            assert!(z.contains(STUFEN[nr - 1].1), "{z}");
        }
    }

    /// Eine Stufe jenseits der Liste bricht ab, statt still zu zählen.
    #[test]
    #[should_panic]
    fn stufe_ausserhalb_der_liste_bricht_ab() {
        let _ = stufe(STUFEN.len() + 1);
    }

    /// Das Zahlwort muss zur Stufenzahl passen.
    #[test]
    fn das_zahlwort_passt_zur_stufenzahl() {
        assert_eq!(stufen_zahlwort(), "sieben", "{} Stufen", STUFEN.len());
    }

    /// Lange Pfade werden gekürzt, kurze nicht.
    #[test]
    fn pfade_werden_gekuerzt() {
        let lang = PathBuf::from("/a/b/c/d/e/f");
        let gekuerzt = kurz(&lang);
        assert!(gekuerzt.starts_with('…'), "{gekuerzt}");
        assert!(gekuerzt.ends_with("f"), "{gekuerzt}");
        let kurzer = PathBuf::from("a/b");
        assert_eq!(kurz(&kurzer), "a/b");
    }

    /// **Ein gekürzter Pfad mischt keine Trennzeichen.**
    ///
    /// 📌 Gefunden vom Windows-Job der CI: `…/d\e\f` in einer Zeile.
    #[test]
    fn gekuerzte_pfade_mischen_keine_trennzeichen() {
        let gekuerzt = kurz(&PathBuf::from("/a/b/c/d/e/f"));
        let fremd = if std::path::MAIN_SEPARATOR == '/' { '\\' } else { '/' };
        assert!(!gekuerzt.contains(fremd), "fremdes Trennzeichen in {gekuerzt}");
    }

    #[test]
    fn ja_nein_ist_eindeutig() {
        assert_ne!(ja_nein(true), ja_nein(false));
    }

    /// Kürzel bleiben eindeutig, auch jenseits der neunten Stelle.
    #[test]
    fn kuerzel_bleiben_eindeutig() {
        let mut gesehen = Vec::new();
        for i in 0..35 {
            let k = kuerzel(i);
            assert!(!gesehen.contains(&k), "Kürzel {k:?} doppelt bei {i}");
            gesehen.push(k);
        }
    }
}
