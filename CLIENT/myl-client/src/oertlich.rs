//! Ein [`Modellweg`], der das Modell auf **derselben Maschine** rechnet.
//!
//! # ⚑ Damit laeuft der Agent ohne Netz
//!
//! Bis zum 2026-09-08 brauchte die Agentenschleife die Tuer eines
//! Knotens. Sie braucht sie weiterhin, wenn sie bezahlte, geprüfte
//! Arbeit will; fuer den eigenen Rechner genuegt dieses hier.

use std::path::Path;
use std::sync::Arc;

use integer_llm_runtime::generate::{dekodieren_fortgesetzt, Denkgrenze, Erzeugung, Fortsetzung, Ziehparameter};
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::tokenizer::Tokenizer;
use myl_local_agent::{Antwort, Modellweg, Nachricht, Tuerfehler};

/// Welche Form ein Artefakt versteht.
///
/// # 📌 Warum das nicht der Client entscheidet (2026-09-08)
///
/// Der erste Anlauf setzte ChatML fuer alles, mit der Begruendung, es
/// sei „die richtige" Vorlage. **Der Ende-zu-Ende-Test hat das
/// widerlegt:** Qwen2.5-0,5B echote die Frage und lieferte Unsinn.
///
/// ⚑ **Der Grund stand im Modellkatalog.** Die Artefakte stammen aus
/// `Qwen/Qwen2.5-0.5B` und `Qwen/Qwen2.5-7B`, also aus den
/// **Basis**repositorien. Ein Basismodell ist auf Textfortsetzung
/// trainiert und hat Rollenmarken nie gesehen; es setzt sie als
/// gewoehnlichen Text fort. Bei **Qwen3** ist es umgekehrt: Dort ist
/// die Fassung ohne Zusatz die instruktionsgeschliffene, das
/// Basismodell hiesse `-Base`.
///
/// ⚑ **Deshalb haengt die Vorlage am Artefakt und nicht an einer
/// Einstellung.** Eine Einstellung kann jemand falsch setzen; eine
/// Eigenschaft des Modells nicht.
///
/// 📌 **Und eine Luecke, die offen bleibt:** Kaeme eine Instruct-Fassung
/// von Qwen2.5 dazu, traeffe die Ableitung aus der Familie das Falsche.
/// Dann braucht der Katalog ein eigenes Feld. Bis dahin ist die
/// Herleitung richtig und die Grenze benannt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vorlage {
    /// Rollenmarken, wie ein instruktionsgeschliffenes Modell sie kennt.
    ChatMl,
    /// ChatML, aber die Aufforderung **oeffnet den Denkblock**.
    ///
    /// ⚑ **Der Unterschied ist eine Zeile und entscheidet alles**
    /// (Fund 425). Qwen3 schreibt `<think>` selbst, sobald es dran ist;
    /// die Vorlage des Qwen3.6 stellt es dagegen in die Aufforderung,
    /// das Modell setzt also **innerhalb** des Blocks fort und schreibt
    /// die Marke nicht mehr. Wer sie weglaesst, laesst das Modell auf
    /// einer Form antworten, die seine Vorlage nie erzeugt.
    ///
    /// ⚠️ **Eine eigene Variante und kein Schalter an `ChatMl`**, damit
    /// die Form fuer Qwen3 Zeichen fuer Zeichen bleibt, was sie war.
    /// Vier eingesetzte Modelle haengen daran.
    ChatMlDenkblock,
    /// Schlichte Fortsetzung, wie ein Basismodell sie erwartet.
    Fortsetzung,
}

impl Vorlage {
    /// Leitet die Vorlage aus der Modellfamilie ab.
    pub fn fuer_familie(familie: &str) -> Self {
        match familie {
            "qwen3" | "qwen3-moe" => Self::ChatMl,
            // ⛔️ **Fund 425: die Familie des Qwen3.6 stand hier nicht**,
            //   und das Modell bekam deshalb `Fortsetzung`, also rohen
            //   Text statt ChatML. Ein Denkmodell auf einer Vorlage, die
            //   es nie gesehen hat, antwortet mit Kauderwelsch, und der
            //   Fehler sieht wie ein Numerikfehler aus.
            //
            // 📌 **Ein `_`-Zweig, der eine Notform liefert, verbirgt
            //   jedes neue Modell.** Er ist hier richtig (ein Basismodell
            //   soll fortsetzen), aber er meldet nichts, wenn ein
            //   Chatmodell hineinfaellt. Deshalb steht die Familie jetzt
            //   ausdruecklich da, und `die_familien_sind_abgedeckt` haelt
            //   es fest.
            f if f.starts_with("qwen3_5") => Self::ChatMlDenkblock,
            _ => Self::Fortsetzung,
        }
    }

    /// Setzt die Unterhaltung zusammen.
    ///
    /// ⚑ **`denken` entscheidet ueber den Denkmodus (2026-09-08).**
    /// Qwen3 beginnt seine Antwort von sich aus mit einem
    /// `<think>`-Block. Der erste Ende-zu-Ende-Lauf lieferte deshalb
    /// „Okay, the user is asking for the capital of France..." und kam
    /// bei vierundzwanzig Token nie zur Antwort.
    ///
    /// 📌 **Fuer einen Agenten ist das schaedlich und faellt nicht
    /// auf:** Er erwartet einen Werkzeugaufruf und bekommt Ueberlegung,
    /// und niemand sieht dem Fehlschlag an, warum. Ein **leerer**
    /// Denkblock im Voraus sagt dem Modell, dass die Ueberlegung schon
    /// stattgefunden hat, und es geht unmittelbar zur Antwort ueber.
    /// ⚑ **Beginnt der Antwortstrom INNERHALB des Denkblocks?**
    ///
    /// Genau dann, wenn [`Self::bauen`] die Marke `<think>` offen
    /// stehen laesst. Dann schreibt das Modell seine Ueberlegung ohne
    /// sie noch einmal zu oeffnen und sendet nur noch `</think>`.
    ///
    /// ⛔️ **Deshalb steht diese Frage hier und nicht beim Zerleger**
    /// (Fund 431, 2026-09-22). Bis dahin nahm der Zerleger an, jeder
    /// Strom beginne ausserhalb, und wartete auf ein `<think>`, das
    /// nie kam: Die **ganze** Ueberlegung des hybriden 35B ging als
    /// Antworttext ins Fenster, und der Nutzer sah den Entwurfsprozess
    /// statt eines ausklappbaren Blocks. 📌 **Wer die Vorlage aendert,
    /// aendert die Lesart der Antwort mit**, und wenn beides an zwei
    /// Stellen entschieden wird, laeuft es auseinander.
    pub fn oeffnet_denkblock(self, denken: bool) -> bool {
        denken && matches!(self, Self::ChatMlDenkblock)
    }

    pub fn bauen(self, nachrichten: &[Nachricht], denken: bool) -> String {
        match self {
            Self::ChatMl | Self::ChatMlDenkblock => {
                let mut aus = String::new();
                for n in nachrichten {
                    aus.push_str("<|im_start|>");
                    aus.push_str(&n.role);
                    aus.push('\n');
                    aus.push_str(&n.content);
                    aus.push_str("<|im_end|>\n");
                }
                // ⚑ Die offene Marke sagt dem Modell, dass jetzt es
                // dran ist. Ohne sie setzt es die Nutzerseite fort.
                aus.push_str("<|im_start|>assistant\n");
                if !denken {
                    aus.push_str("<think>\n\n</think>\n\n");
                } else if matches!(self, Self::ChatMlDenkblock) {
                    // ⚑ **Offen, nicht geschlossen** (Fund 425): Das
                    //   Modell soll ueberlegen, und seine Vorlage gibt
                    //   ihm dafuer den Block vor.
                    aus.push_str("<think>\n");
                }
                aus
            }
            // ⚑ **Fuer ein Basismodell zaehlt, was wie Text aussieht.**
            // Am Pruefstand vom 2026-09-07 beantwortete Qwen2.5-0,5B
            // vier von vier Fortsetzungsfragen und nur eine von zwei im
            // Frage-Antwort-Schema. Die Fortsetzung ist damit nicht die
            // Notloesung, sondern die passende Form.
            Self::Fortsetzung => {
                let mut aus = String::new();
                for n in nachrichten {
                    match n.role.as_str() {
                        "system" => {
                            aus.push_str(&n.content);
                            aus.push_str("\n\n");
                        }
                        "assistant" => {
                            aus.push_str("Antwort: ");
                            aus.push_str(&n.content);
                            aus.push('\n');
                        }
                        _ => {
                            aus.push_str("Frage: ");
                            aus.push_str(&n.content);
                            aus.push('\n');
                        }
                    }
                }
                aus.push_str("Antwort:");
                aus
            }
        }
    }
}

/// Die Token, bei denen eine Antwort dieses Modells zu Ende ist.
///
/// ⚑ **Aus dem Wortschatz und nicht aus einer Tabelle.** Was eine
/// Endmarke ist, steht im Wortschatz des Modells; eine Tokennummer im
/// Quelltext passte zu genau einem.
///
/// 📌 **Nur, was zu **einem** Token wird, zaehlt.** Kennt ein Wortschatz
/// die Marke nicht als Sonderzeichen, zerlegt er sie in gewoehnliche
/// Stuecke, und dann waere ein Halt darauf ein Halt mitten im Text: Das
/// Stueck `<` beendete jede Antwort, die eine spitze Klammer enthaelt.
fn haltemarken(wortschatz: &Tokenizer, familie: &str) -> Vec<usize> {
    // ⚑ `<|endoftext|>` gilt fuer beide Vorlagen; `<|im_end|>` beendet
    // eine Runde und hat nur dort einen Sinn, wo es Runden gibt.
    let marken: &[&str] = match Vorlage::fuer_familie(familie) {
        // ⚑ Dieselben Haltemarken: der Denkblock aendert die
        //   Aufforderung, nicht das Ende der Antwort (Fund 425).
        Vorlage::ChatMl | Vorlage::ChatMlDenkblock => &["<|im_end|>", "<|endoftext|>"],
        Vorlage::Fortsetzung => &["<|endoftext|>"],
    };
    marken
        .iter()
        .filter_map(|m| {
            let t = wortschatz.encode(m);
            (t.len() == 1).then(|| t[0])
        })
        .collect()
}

/// **Was eingeschoben wird, wenn das Denkbudget erschoepft ist.**
///
/// Ein kurzer Satz, der die Ueberlegung beendet, und die Endmarke. Das
/// Modell antwortet danach auf Grundlage dessen, was es bis dahin
/// ueberlegt hat.
pub const DENKSCHLUSS: &str =
    "\n\nTime is short, so I will answer now based on what I have worked out so far.\n</think>\n\n";

/// **Wie viel der Antwortlaenge das Ueberlegen hoechstens bekommt, in
/// Achteln.**
///
/// ⛔️ **Fund 512 (2026-09-30):** Das 35B ueberlegte im Agenten die ganzen
/// 4 000 Token einer Antwort lang, ob eine Verrundung geht, und die
/// Antwort endete mitten im Denken, ohne Werkzeugaufruf. Ein Denkbudget
/// gab es nur beim Vorlesen. ⚑ **Das Ueberlegen darf die Antwort nie ganz
/// verbrauchen**: Fuenf Achtel, der Rest bleibt fuer den Aufruf oder die
/// Antwort. Bei 4 000 Token sind das 2 500 und 1 500, bei 1 600 im
/// Gespraech 1 000 und 600. Ein CAD-Skript von 1 500 Byte braucht rund 500
/// Token.
pub const DENKANTEIL_ACHTEL: usize = 5;

/// **Das Denkbudget, das wirkt**: das kleinere aus dem ausdruecklich
/// gesetzten und dem Anteil an der Antwortlaenge.
pub fn wirksames_denkbudget(ausdruecklich: Option<usize>, grenze: usize) -> usize {
    let anteil = (grenze * DENKANTEIL_ACHTEL) >> 3;
    ausdruecklich.map_or(anteil, |b| b.min(anteil))
}

/// **Wie ohne Denkmodus gezogen wird**: Temperatur 0,7, Top-k 20, Top-p 0,8.
///
/// ⚑ Die Empfehlung der Qwen-Modellkarten fuer den Betrieb ohne
/// Ueberlegung, auf der alle grossen Modelle hier beruhen. Die Temperatur
/// steht als Kehrwert (`256 / 0,7`), damit multipliziert und nicht geteilt
/// wird.
pub const ZIEHEN: Ziehparameter = Ziehparameter { kehrwert_temperatur_q8: 366, top_k: 20, top_p_q16: 52_429 };

/// **Wie mit Denkmodus gezogen wird**: Temperatur 0,6, Top-k 20, Top-p 0,95,
/// ebenfalls nach den Modellkarten.
pub const ZIEHEN_DENKEND: Ziehparameter = Ziehparameter { kehrwert_temperatur_q8: 427, top_k: 20, top_p_q16: 62_259 };

/// **Eine Saat aus dem Zufall des Systems.** `RandomState` zieht seine
/// Schluessel beim Betriebssystem, auf jedem der drei Systeme, ohne
/// weitere Kiste.
///
/// ⚑ **Unter 2^53** ([`SAAT_BIS`]): So viel stellt JavaScript als ganze
/// Zahl genau dar. Eine groessere Saat kaeme im Fenster gerundet an, und
/// „wiederholen“ liefe still mit einer anderen. Hoechstens 16 Stellen sind
/// ausserdem abtippbar. Gesetzt werden darf jede `u64`.
pub fn zufallssaat() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish() & SAAT_BIS
}

/// Die groesste gezogene Saat, `2^53 - 1`.
pub const SAAT_BIS: u64 = (1 << 53) - 1;

/// Der Stand einer Aktion in diesem Faden.
#[derive(Clone, Copy, Default)]
struct Aktionsstand {
    saat: u64,
    /// Die wievielte Antwort der Aktion zuletzt gezogen wurde.
    zug: u64,
    /// Wie tief Aktionen geschachtelt sind; nur die aeusserste zieht.
    tiefe: usize,
}

thread_local! {
    /// ⚑ **Je Faden, nicht je Modell.** `myl agent` faehrt mehrere Auftraege
    /// gleichzeitig ueber ein geladenes Modell; ein gemeinsamer Zaehler
    /// haette ihre Saaten verschraenkt, und keiner waere wiederholbar. Ein
    /// Lauf samt Pruefung laeuft immer in einem Faden.
    static AKTION: std::cell::Cell<Aktionsstand> = const { std::cell::Cell::new(Aktionsstand { saat: 0, zug: 0, tiefe: 0 }) };
}

/// Das Modell im eigenen Speicher.
pub struct Oertlichesmodell {
    modell: Arc<IntegerModel>,
    wortschatz: Tokenizer,
    /// Die Familie aus `model_config.json`, aus der die Vorlage folgt.
    familie: String,
    /// Wie viele Token eine Antwort hoechstens hat, wenn der Aufrufer
    /// nichts sagt.
    pub grenze: usize,
    /// ⚑ **Gezogen als Vorgabe, mit Saat** (Regel des Projektinhabers,
    /// 2026-09-29). Bis dahin stand hier „gierig als Vorgabe“, mit dem
    /// Grund, Ziehen mache einen Fehlschlag unauffindbar. Die Saat loest
    /// genau das: Gleiche Saat, bitgleicher Lauf; andere Saat, anderer Weg.
    /// 📌 Gierig lief jede Wiederholung wortgleich, und ein Vergleich zweier
    /// Einstellungen sah nur je einen einzigen Weg. `true` bleibt fuer
    /// Messungen, die genau einen festen Weg brauchen.
    pub gierig: bool,
    /// **Eine feste Saat, oder `None`: Zufall je Aktion** (die Vorgabe).
    /// Gesetzt aus `modell.saat`, `--saat` oder „immer“ in der Wahl.
    ///
    /// ⚑ **Die Saat gehoert zur Aktion, nicht zum Laden.** Eine Aktion ist
    /// ein Auftrag, eine Runde, eine Nachricht; alle Aufrufe darin, auch die
    /// Pruefung, ziehen der Reihe nach aus ihr. Mit der Saat einer Aktion
    /// und demselben Verlauf davor wiederholt sich genau diese Aktion; eine
    /// Saat je Laden haette verlangt, die ganze Sitzung nachzuspielen.
    pub saat_fest: Option<u64>,
    /// **Temperatur, Top-p und Top-k aus den Einstellungen**, in
    /// Hundertsteln bzw. als Zahl; `None` heisst die Vorgabe des Modus
    /// ([`ZIEHEN`], [`ZIEHEN_DENKEND`]). Temperatur 0 ist gierig.
    pub temperatur: Option<u32>,
    pub top_p: Option<u32>,
    pub top_k: Option<u32>,
    /// Eine Saat nur fuer die naechste Aktion („nur naechste Antwort“,
    /// `/seed 1234`); danach gilt wieder [`Self::saat_fest`].
    naechste: std::sync::Mutex<Option<u64>>,
    /// Die Saat der zuletzt begonnenen Aktion, fuer die Anzeige. Die Saat
    /// eines einzelnen Laufs traegt sein `lauf::Ausgang`; der Stand, aus dem
    /// gezogen wird, liegt je Faden in [`AKTION`].
    aktion: std::sync::Mutex<Option<u64>>,
    /// ⚑ **Denkmodus, standardmaessig AUS.** Ein Harness will
    /// Werkzeugaufrufe, keine Ueberlegung; und jedes Denktoken kostet
    /// dieselbe Rechenzeit wie ein Antworttoken.
    pub denken: bool,
    /// **Wie viele Token hoechstens ueberlegt wird**, wenn [`Self::denken`]
    /// an ist. `None` heisst ohne Grenze, `Some(0)` heisst gar nicht.
    ///
    /// ⚑ **Vorgabe `None`**, also das Verhalten von vorher. Gesetzt wird
    /// es vom Aufrufer fuer einen einzelnen Weg, heute beim Vorlesen: Dort
    /// ist die Ueberlegung Wartezeit, in der nichts klingt.
    pub denkbudget: Option<usize>,
    /// Die Endmarke der Ueberlegung (`</think>`), aus dem Wortschatz;
    /// `None`, wenn er sie nicht als ein Token kennt, und dann wirkt
    /// kein Budget.
    denkende: Option<usize>,
    /// Was eingeschoben wird, wenn das Budget erschoepft ist, bis
    /// einschliesslich der Endmarke. Siehe [`DENKSCHLUSS`].
    pub denkschluss: String,
    /// Die Token, bei denen eine Antwort zu Ende ist.
    ///
    /// # 📌 Der Fehler, aus dem dieses Feld entstanden ist
    ///
    /// **Ohne Haltemarken rechnet die Erzeugung stur bis zur Grenze.**
    /// Gemessen am 2026-09-10 mit Qwen3-4B und 600 Token: Das Modell
    /// beendete seine Antwort, schrieb `<|im_end|>`, dann
    /// `<|endoftext|>` und **erfand danach ein ganzes Gespraech
    /// weiter**, samt einem zweiten, ausgedachten Nutzer. Der Zuschnitt
    /// der fertigen Antwort schnitt das ab; **die laufende Anzeige
    /// nicht**, und im Agentenlauf ging der erfundene Text als
    /// Modellantwort in die naechste Runde.
    ///
    /// ⚑ **Das ist kein Fehler des Modells, sondern seine Aufgabe.** Es
    /// setzt Text fort, und nach einer beendeten Antwort setzt es die
    /// naechste Runde fort. Wer es aufhalten will, sagt ihm, wo.
    ///
    /// ⚑ **Hergeleitet aus dem Wortschatz und nicht hingeschrieben.**
    /// Eine Tokennummer im Quelltext waere eine Zahl, die zu genau
    /// einem Modell passt.
    pub halt: Vec<usize>,
    /// Wer beim Schreiben zusehen will.
    ///
    /// # ⚑ Warum der Beobachter am Modell haengt und nicht am Aufruf
    ///
    /// **Weil beide Wege ihn brauchen und nur einer davon ihn
    /// durchreichen koennte.** Eine Frage ruft `chat` unmittelbar; ein
    /// Agentenlauf ruft es aus der Schleife heraus, und die liegt in
    /// `myl-local-agent`. Jene Kiste traegt eine Vollmacht und haelt
    /// ihre Flaeche klein; ihr einen Anzeigeweg durchzureichen kehrte
    /// die Entscheidung um, die sie ueberhaupt begruendet.
    ///
    /// ⚑ **So meldet das Modell selbst, was es gerade schreibt**, und
    /// die Schleife merkt davon nichts.
    ///
    /// ⚑ **Er bekommt fertige Stuecke und keinen rohen Zuwachs.**
    /// Ob ein Stueck Ueberlegung oder Antwort ist, entscheidet sich an
    /// Marken im Strom, und die kommen zerrissen an: `</think>` trifft
    /// als `</`, `think`, `>` ein. Das gehoert einmal geloest und nicht
    /// bei jedem, der zusieht; [`myl_local_agent::textstrom::Zerleger`] tut es.
    ///
    /// 📌 **Und das Ende einer Antwort weiss nur diese Stelle.** Ein
    /// Zerleger haelt zurueck, was noch eine Marke werden koennte;
    /// ohne einen Abschluss verschwaenden die letzten Zeichen jeder
    /// Antwort. Wer den Zerleger aussen hielte, muesste raten, wann er
    /// ihn leert, und beim Agenten liegen zwischen zwei Antworten
    /// Werkzeugaufrufe.
    ///
    /// 📌 **`Fn` und nicht `FnMut`:** [`Modellweg::chat`] nimmt `&self`,
    /// und das Modell wird ueber Faeden geteilt (Punkt 0.5). Wer hier
    /// veraenderlichen Zustand braucht, legt ihn hinter ein eigenes
    /// Schloss und nicht in diese Naht.
    pub beobachter: Option<Box<dyn Fn(myl_local_agent::textstrom::Stueck) + Send + Sync>>,
    /// Wie viele Token dieses Modell bisher gelesen und geschrieben
    /// hat.
    ///
    /// ⚑ **Ein Zaehler und kein Rueckgabewert.** Was ein Lauf kostet,
    /// will man **waehrend** des Laufs sehen und nicht danach; die
    /// Zahlen in [`Antwort`] stehen erst fest, wenn der Schritt vorbei
    /// ist. Der Zaehler haengt am Modell, weil dort gezaehlt wird.
    ///
    /// ⚠️ **Er zaehlt ueber Schritte hinweg weiter** und wird von dem
    /// zurueckgesetzt, der einen neuen Auftrag beginnt: Ein Zaehler,
    /// der sich selbst zurueckstellt, zeigt beim Agenten nur den
    /// letzten Schritt.
    pub zaehler: std::sync::Arc<Tokenzaehler>,
    /// **Der KV-Speicher des laufenden Gespraechs** (Fund 372).
    ///
    /// ⚑ Ein Agentenlauf schickt in jedem Schritt alle Nachrichten davor
    /// noch einmal; gerechnet wird nur, was dahinter neu ist. Hinter einem
    /// Schloss, weil [`Modellweg::chat`] `&self` nimmt.
    fortsetzung: std::sync::Mutex<Fortsetzung>,
}

/// Gelesene und geschriebene Token, waehrend es geschieht.
#[derive(Debug, Default)]
pub struct Tokenzaehler {
    /// Token im Prompt, aufsummiert ueber alle Schritte.
    pub hinein: std::sync::atomic::AtomicU32,
    /// Erzeugte Token, aufsummiert ueber alle Schritte.
    pub heraus: std::sync::atomic::AtomicU32,
    /// Token im Prompt, die schon im KV-Speicher standen und nicht noch
    /// einmal gerechnet wurden, aufsummiert (Fund 372).
    pub wiederverwendet: std::sync::atomic::AtomicU32,
    /// Wie viele Positionen der Kontext nach dem letzten Schritt belegt:
    /// Prompt und Antwort.
    pub kontext: std::sync::atomic::AtomicU32,
}

impl Tokenzaehler {
    /// Beide Zaehler auf null, fuer einen neuen Auftrag.
    pub fn zuruecksetzen(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        self.hinein.store(0, Relaxed);
        self.heraus.store(0, Relaxed);
        self.wiederverwendet.store(0, Relaxed);
    }

    /// Der Stand, als Paar.
    pub fn stand(&self) -> (u32, u32) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.hinein.load(Relaxed), self.heraus.load(Relaxed))
    }
}

impl Oertlichesmodell {
    /// Laedt ein Artefaktverzeichnis, soweit die Freigabe es zulaesst.
    ///
    /// # ⚑ Warum die Schranke hier steht und nicht beim Aufrufer
    ///
    /// **Weil es sieben Aufrufer gibt.** Eine Pruefung daneben waere an
    /// sechs Stellen richtig und an der siebten vergessen, und das ist
    /// die haeufigste Fehlerklasse dieses Projekts. Hier kommt niemand
    /// daran vorbei, denn der Uebersetzer verlangt die Freigabe als
    /// Argument.
    ///
    /// 📌 **Und sie ist vorsichtig und nicht genau.** Gemessen wird die
    /// Groesse des Artefakts auf der Platte; die Gewichte werden aber
    /// **speicherabgebildet**, der wirkliche Verbrauch liegt also
    /// darunter und haengt daran, wie viel davon angefasst wird. Die
    /// Schranke lehnt damit gelegentlich etwas ab, das gerade noch
    /// gepasst haette. **Das ist die richtige Richtung zu irren:** Ein
    /// abgelehntes Laden ist ein Satz, ein zu spaet bemerkter
    /// Speichermangel ist ein toter Rechner.
    pub fn laden(
        verzeichnis: &str,
        kapazitaet: &crate::einstellungen::Kapazitaet,
    ) -> Result<Self, String> {
        if let Some(grenze) = kapazitaet.speicher_gib {
            let braucht = crate::reservierung::belegung(Path::new(verzeichnis));
            let erlaubt = grenze as u64 * crate::hardware::GIB;
            if braucht > erlaubt {
                let gib = |b: u64| b as f64 / crate::hardware::GIB as f64;
                return Err(format!(
                    "Das Artefakt belegt {:.1} GiB, freigegeben sind {grenze} GiB \
                     Arbeitsspeicher. Erhöhe die Freigabe oder wähle ein kleineres Modell.",
                    gib(braucht)
                ));
            }
        }
        let modell = load_model(Path::new(verzeichnis)).map_err(|e| format!("Modell: {e}"))?;
        let wortschatz = Tokenizer::from_file(
            &format!("{verzeichnis}/tokenizer.json"),
        )
        .map_err(|e| format!("Wortschatz: {e}"))?;
        // ⚑ Die Familie steht im Artefakt und wird nicht geraten.
        let roh = std::fs::read_to_string(format!("{verzeichnis}/model_config.json"))
            .map_err(|e| format!("model_config.json: {e}"))?;
        let konf: serde_json::Value =
            serde_json::from_str(&roh).map_err(|e| format!("model_config.json: {e}"))?;
        let familie = konf
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let halt = haltemarken(&wortschatz, &familie);
        // ⚑ Wie bei den Haltemarken: nur, was ein einziges Token ist.
        let denkende = {
            let t = wortschatz.encode("</think>");
            (t.len() == 1).then(|| t[0])
        };
        // ⚑ **Die Merkmarke fuer Modelle mit rekurrenten Ebenen**: Vor der
        //   letzten Rollenmarke bleibt ein Verlauf von Schritt zu Schritt
        //   gleich, dahinter steht die neu gerenderte Antwort. Dort hebt
        //   das Laufwerk den Zustand auf. 📌 Ohne sie las das 27B im
        //   Agentenbetrieb jeden Schritt den ganzen Verlauf neu vor, 12 bis
        //   13 Minuten je Schritt.
        let mut fortsetzung = Fortsetzung::neu(&modell);
        if matches!(Vorlage::fuer_familie(&familie), Vorlage::ChatMl | Vorlage::ChatMlDenkblock) {
            let t = wortschatz.encode("<|im_start|>");
            fortsetzung.merkmarke_setzen((t.len() == 1).then(|| t[0]));
        }
        Ok(Self {
            wortschatz,
            familie,
            grenze: 512,
            gierig: false,
            saat_fest: None,
            temperatur: None,
            top_p: None,
            top_k: None,
            naechste: std::sync::Mutex::new(None),
            aktion: std::sync::Mutex::new(None),
            denken: false,
            denkbudget: None,
            denkende,
            denkschluss: DENKSCHLUSS.to_string(),
            halt,
            beobachter: None,
            zaehler: std::sync::Arc::new(Tokenzaehler::default()),
            fortsetzung: std::sync::Mutex::new(fortsetzung),
            modell: Arc::new(modell),
        })
    }

    /// **Ob gierig gewaehlt wird**: ausdruecklich (`--gierig`) oder mit
    /// Temperatur 0.
    pub fn waehlt_gierig(&self) -> bool {
        self.gierig || self.temperatur == Some(0)
    }

    /// **Wie gezogen wird**, oder `None`, wenn gierig: die Vorgabe des
    /// Modus, darueber, was eingestellt ist.
    pub fn ziehparameter(&self) -> Option<Ziehparameter> {
        if self.waehlt_gierig() {
            return None;
        }
        let mut p = if self.denkt() { ZIEHEN_DENKEND } else { ZIEHEN };
        // Umgerechnet beim Einstellen, nicht im Rechenpfad: 256 / (T / 100),
        // gerundet.
        if let Some(t) = self.temperatur.filter(|t| *t > 0) {
            p.kehrwert_temperatur_q8 = (25_600 + t / 2) / t;
        }
        if let Some(q) = self.top_p {
            p.top_p_q16 = (q * 65_536 + 50) / 100;
        }
        if let Some(k) = self.top_k {
            p.top_k = k as usize;
        }
        Some(p)
    }

    /// **Die Modelleinstellungen an einer Stelle uebernehmen**: Laenge,
    /// Denkmodus und, falls gesetzt, die Saat. Ohne gesetzte Saat bleibt
    /// die beim Laden gezogene.
    pub fn uebernehmen(&mut self, e: &crate::einstellungen::Modelleinstellung) {
        self.grenze = e.token;
        self.denken = e.denken;
        self.saat_fest = e.saat;
        self.temperatur = e.temperatur;
        self.top_p = e.top_p;
        self.top_k = e.top_k;
    }

    /// **Eine Saat nur fuer die naechste Aktion**; danach wieder die feste
    /// oder der Zufall.
    pub fn naechste_saat(&self, saat: u64) {
        *self.naechste.lock().unwrap_or_else(|e| e.into_inner()) = Some(saat);
    }

    /// Die vorgemerkte Saat fuer die naechste Aktion, falls eine da ist.
    pub fn naechste(&self) -> Option<u64> {
        *self.naechste.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// **Die Saat der laufenden oder zuletzt gelaufenen Aktion**; `None`,
    /// bevor eine lief.
    pub fn letzte_saat(&self) -> Option<u64> {
        *self.aktion.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Die Parameter in einer Zeile, fuer Protokoll und Anzeige, etwa
    /// `T 0,70 · top-p 0,80 · top-k 20`.
    pub fn parameterzeile(&self) -> String {
        if self.waehlt_gierig() {
            return "gierig".to_string();
        }
        let hundertstel = |z: u32| format!("{},{:02}", z / 100, z % 100);
        let t = self.temperatur.unwrap_or(if self.denkt() { 60 } else { 70 });
        let p = self.top_p.unwrap_or(if self.denkt() { 95 } else { 80 });
        let k = self.top_k.unwrap_or(ZIEHEN.top_k as u32);
        format!("T {} · top-p {} · top-k {k}", hundertstel(t), hundertstel(p))
    }

    /// Die Laufparameter dieses Modells, an einer Stelle.
    ///
    /// ⚑ **Damit beide Zweige dieselben nehmen.** Ein Lauf mit
    /// Zuschauer und einer ohne unterscheiden sich nur im Zuschauer;
    /// zwei getippte Parameterlisten koennten irgendwann mehr
    /// unterscheiden, und dann haetten sie verschiedene Antworten.
    fn erzeugung<'a>(&'a self, grenze: usize, schluss: &'a [usize]) -> Erzeugung<'a> {
        // Die n-te Antwort der Aktion bekommt ihre eigene Saat, fest aus der
        // Saat der Aktion und n.
        let seed = AKTION.with(|a| {
            let mut s = a.get();
            s.zug += 1;
            a.set(s);
            s.saat ^ s.zug.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        });
        Erzeugung {
            max_new_tokens: grenze,
            seed,
            greedy: self.gierig,
            ziehen: self.ziehparameter(),
            halt: &self.halt,
            // ⚑ Immer mit Grenze, wenn ueberlegt wird (Fund 512); siehe
            //   [`wirksames_denkbudget`].
            denkgrenze: match (self.denkt(), self.denkende) {
                (true, Some(ende)) => {
                    Some(Denkgrenze { budget: wirksames_denkbudget(self.denkbudget, grenze), ende, schluss })
                }
                _ => None,
            },
            // ⛔️ Der Notaus haelt die Erzeugung vor dem naechsten Token an.
            abbruch: Some(myl_local_agent::notaus::schalter()),
        }
    }

    /// **Ob dieser Lauf ueberlegt**: Denkmodus an und ein Budget, das
    /// nicht null ist.
    ///
    /// ⚑ **Null heisst: gar nicht ueberlegen**, und das ist die leere
    /// Ueberlegung im Voraus, nicht ein Einschub nach null Token. Der
    /// Einschub saehe fuer das Modell aus wie eine abgebrochene
    /// Ueberlegung; die leere ist die, auf die es trainiert ist.
    pub fn denkt(&self) -> bool {
        self.denken && self.denkbudget != Some(0)
    }

    /// Welche Vorlage dieses Artefakt versteht.
    pub fn vorlage(&self) -> Vorlage {
        Vorlage::fuer_familie(&self.familie)
    }

    /// ⚑ **ChatML, und das weicht bewusst vom Netzweg ab.**
    ///
    /// Der Gateway setzt eine Unterhaltung heute als `role: content`
    /// zusammen. **Fund 181 haelt fest, dass Qwen2.5 und Qwen3 auf
    /// ChatML trainiert sind** (`<|im_start|>rolle\ninhalt<|im_end|>`)
    /// und diese Form damit eine ist, die das Modell **nie gesehen
    /// hat**. Genau die Werkzeugfaehigkeit haengt an der Vorlage, und
    /// ein Harness lebt von ihr.
    ///
    /// 📌 **Dort wurde es nicht geaendert, und das aus gutem Grund:** Die
    /// Vorlage bestimmt die Token, die Token bestimmen die E2E-Vektoren,
    /// und die stehen im Konformitaetswert. Eine Aenderung ist eine
    /// Entscheidung ueber den numerischen Vertrag.
    ///
    /// ⚑ **Hier gilt das nicht.** Der lokale Weg ist keine bezahlte,
    /// geprüfte Arbeit und steht in keinem Vertrag. Er benutzt deshalb
    /// die **richtige** Vorlage und ist damit zugleich der Pruefstand,
    /// an dem sich zeigt, ob ChatML die Werkzeugfaehigkeit hebt, **bevor**
    /// jemand den Konformitaetswert dafuer bewegt.
    pub fn chatml(nachrichten: &[Nachricht]) -> String {
        let mut aus = String::new();
        for n in nachrichten {
            aus.push_str("<|im_start|>");
            aus.push_str(&n.role);
            aus.push('\n');
            aus.push_str(&n.content);
            aus.push_str("<|im_end|>\n");
        }
        // Die offene Marke sagt dem Modell, dass jetzt es dran ist.
        aus.push_str("<|im_start|>assistant\n");
        aus
    }
}

impl Oertlichesmodell {
    /// Eine Antwort, innerhalb der laufenden Aktion.
    fn antworten(
        &self,
        nachrichten: &[Nachricht],
        max_tokens: Option<u32>,
    ) -> Result<Antwort, Tuerfehler> {
        let prompt = self.vorlage().bauen(nachrichten, self.denkt());
        // Einmal zerlegt, fuer den Zaehler und fuer die Rechnung: Bei einem
        // langen Verlauf kostet das Zerlegen selbst merklich.
        let prompt_token = self.wortschatz.encode(&prompt);
        // ⚑ **Ein Prompt ohne Platz fuer eine Antwort ist ein benannter
        // Fehler**, den die Schleife mit Verdichten beantwortet; das
        // Laufwerk selbst bricht dort ab (Fund 368).
        let kontextgrenze = self.modell.kontextgrenze();
        if prompt_token.len() >= kontextgrenze {
            return Err(Tuerfehler::KontextVoll { belegt: prompt_token.len(), grenze: kontextgrenze });
        }
        self.zaehler.hinein.fetch_add(prompt_token.len() as u32, std::sync::atomic::Ordering::Relaxed);
        let grenze = max_tokens.map(|m| m as usize).unwrap_or(self.grenze);
        let schluss = self.wortschatz.encode(&self.denkschluss);
        // ⚑ **Ein Schloss fuer den ganzen Schritt**: Zwei gleichzeitige
        // Aufrufe duerfen nicht in denselben Speicher schreiben.
        let mut speicher = self.fortsetzung.lock().unwrap_or_else(|e| e.into_inner());
        let (token, wiederverwendung) = match &self.beobachter {
            // ⚑ Auch ohne Zuschauer wird gehalten: Die Marken gehoeren
            // zur Antwort und nicht zur Anzeige. Ohne sie rechnete
            // `myl frage` dieselben ueberzaehligen Token wie das
            // Fenster, nur ohne dass jemand zusieht.
            None => dekodieren_fortgesetzt(
                &self.modell,
                &prompt_token,
                &self.erzeugung(grenze, &schluss),
                &mut speicher,
                &mut |_| {
                    self.zaehler.heraus.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                },
            ),
            // ⚑ **Der Zuwachs entsteht aus der ganzen Folge und nicht
            // aus dem einzelnen Token**, und das ist kein Umweg.
            // Ein Token ist ein Wortteil und manchmal nur ein Stueck
            // einer Mehrbytefolge; wer jedes fuer sich dekodiert,
            // schreibt Ersatzzeichen ins Fenster. Die ganze Folge zu
            // dekodieren und den Zuwachs zu nehmen ist die einzige
            // Lesart, die immer stimmt.
            Some(f) => {
                let mut bisher = String::new();
                let mut alle: Vec<usize> = Vec::with_capacity(grenze);
                let mut zerleger = myl_local_agent::textstrom::Zerleger::neu_im_denken(
                    self.vorlage().oeffnet_denkblock(self.denkt()),
                );
                let token = dekodieren_fortgesetzt(
                    &self.modell,
                    &prompt_token,
                    &self.erzeugung(grenze, &schluss),
                    &mut speicher,
                    &mut |t| {
                        self.zaehler.heraus.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        alle.push(t);
                        let jetzt = self.wortschatz.decode(&alle);
                        if let Some(zuwachs) = jetzt.strip_prefix(&bisher) {
                            if !zuwachs.is_empty() {
                                for s in zerleger.schluck(zuwachs) {
                                    f(s);
                                }
                            }
                        }
                        bisher = jetzt;
                    },
                );
                // ⚑ **Der Abschluss gehoert hierher und nirgends
                // sonst.** Genau hier endet eine Antwort, und nur hier
                // ist bekannt, dass sie endet: Beim Agenten folgt
                // danach ein Werkzeugaufruf und dann die naechste.
                for s in zerleger.abschluss() {
                    f(s);
                }
                token
            }
        };
        {
            use std::sync::atomic::Ordering::Relaxed;
            self.zaehler.wiederverwendet.fetch_add(wiederverwendung.wiederverwendet as u32, Relaxed);
            self.zaehler.kontext.store(speicher.laenge() as u32, Relaxed);
        }
        drop(speicher);
        let text = self.wortschatz.decode(&token);
        // ⛔️ **Fund 512: An der Grenze geendet ist nicht „stop“.** Die
        //   Schleife muss es wissen, sonst haelt sie eine abgeschnittene
        //   Antwort ohne Aufruf fuer das Ende der Arbeit.
        let abschlussgrund = if token.len() >= grenze && !text.contains("<|im_end|>") { "length" } else { "stop" };
        // ⚑ Die Endmarke gehoert nicht in die Antwort; sie ist Rahmen
        // und nicht Inhalt.
        // ⚑ Rahmen ist nicht Inhalt: die Endmarke des einen Weges und
        // die naechste Frage des anderen gehoeren nicht in die Antwort.
        let text = text
            .split("<|im_end|>")
            .next()
            .unwrap_or(&text)
            .split("\nFrage:")
            .next()
            .unwrap_or(&text)
            .trim()
            .to_string();
        // ⛔️ **Angehalten ist kein Ergebnis wie jedes andere**: Die Schleife
        //   muss enden, und der Text bis dahin kommt mit.
        if myl_local_agent::notaus::ausgeloest() {
            return Err(Tuerfehler::Abgebrochen { bisher: text });
        }
        Ok(Antwort {
            text,
            abschlussgrund: Some(abschlussgrund.to_string()),
            // 📌 **Keine Kennung und kein Segment, und das mit Absicht.**
            // Beides sind Belege der Kette. Lokal gerechnete Arbeit hat
            // keinen, und einen zu erfinden waere schlimmer als keiner:
            // Er saehe aus wie ein Nachweis.
            kennung: String::new(),
            segment: None,
            prompt_token: prompt_token.len() as u32,
            antwort_token: token.len() as u32,
        })
    }

}

impl Modellweg for Oertlichesmodell {
    /// ⚑ **Ohne offene Aktion ist jede Antwort eine eigene**, etwa eine
    /// Nachricht im Gespraech; innerhalb eines Laufs zieht sie aus dessen
    /// Saat.
    fn chat(
        &self,
        _modell: &str,
        nachrichten: &[Nachricht],
        max_tokens: Option<u32>,
    ) -> Result<Antwort, Tuerfehler> {
        let eigene = AKTION.with(|a| a.get().tiefe == 0);
        if eigene {
            self.aktion_beginnen();
        }
        let aus = self.antworten(nachrichten, max_tokens);
        if eigene {
            self.aktion_beenden();
        }
        aus
    }

    fn aktion_beginnen(&self) -> Option<u64> {
        let stand = AKTION.with(|a| {
            let mut s = a.get();
            if s.tiefe == 0 {
                let einmal = self.naechste.lock().unwrap_or_else(|e| e.into_inner()).take();
                s.saat = einmal.or(self.saat_fest).unwrap_or_else(zufallssaat);
                s.zug = 0;
                *self.aktion.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.saat);
                myl_local_agent::protokoll::saat("aktion", (!self.waehlt_gierig()).then_some(s.saat), &self.parameterzeile());
            }
            s.tiefe += 1;
            a.set(s);
            s
        });
        (!self.waehlt_gierig()).then_some(stand.saat)
    }

    fn aktion_beenden(&self) {
        AKTION.with(|a| {
            let mut s = a.get();
            s.tiefe = s.tiefe.saturating_sub(1);
            a.set(s);
        });
    }

    /// Genau gezaehlt: dieselbe Vorlage und derselbe Wortschatz wie in
    /// [`Modellweg::chat`].
    fn kontext(&self, nachrichten: &[Nachricht]) -> Option<myl_local_agent::Kontextstand> {
        let prompt = self.vorlage().bauen(nachrichten, self.denkt());
        Some(myl_local_agent::Kontextstand {
            belegt: self.wortschatz.encode(&prompt).len(),
            grenze: self.modell.kontextgrenze(),
        })
    }
}

#[cfg(test)]
mod teilbarkeit {

    /// ⛔️ **Fund 512: Das Ueberlegen verbraucht nie die ganze Antwort.**
    /// Fuenf Achtel der Grenze, ein kleineres ausdrueckliches Budget gilt
    /// weiter, ein groesseres nicht.
    #[test]
    fn das_ueberlegen_laesst_platz_fuer_die_antwort() {
        assert_eq!(wirksames_denkbudget(None, 4000), 2500);
        assert_eq!(wirksames_denkbudget(None, 1600), 1000);
        assert_eq!(wirksames_denkbudget(Some(32), 4000), 32);
        assert_eq!(wirksames_denkbudget(Some(100_000), 4000), 2500);
        // Gegenprobe: Es bleibt immer etwas, auch bei kleinen Grenzen.
        for g in [8usize, 64, 1000, 4000, 32_000] {
            assert!(wirksames_denkbudget(None, g) < g, "bei {g} bleibt nichts fuer die Antwort");
        }
    }

    use super::*;

    /// ⚑ **Woran Punkt 0.5 haengt, und zwar ganz.**
    ///
    /// Unteragenten auf freigegebenen Rechenwerken sind nur dann
    /// bezahlbar, wenn sie sich **ein** geladenes Modell teilen. Ein
    /// 4B-Artefakt je Unteragent waere kein Nebenlaeufigkeitsentwurf,
    /// sondern eine Speichersperre: Drei Agenten braechten drei
    /// Kopien der Gewichte.
    ///
    /// Die Voraussetzung dafuer ist, dass [`Oertlichesmodell`] geteilt
    /// werden darf: [`Modellweg::chat`] nimmt `&self`, die Gewichte
    /// liegen hinter einem `Arc`, und der Rechenpfad haelt keinen
    /// veraenderlichen Zustand.
    ///
    /// ⚑ **Deshalb steht das hier als Uebersetzungsfehler und nicht
    /// als Absicht in einem Kommentar.** Wer spaeter ein `RefCell` oder
    /// einen Zwischenspeicher in das Modell legt, faellt hier auf und
    /// nicht erst beim Bau der Unteragenten.
    #[test]
    fn das_modell_laesst_sich_ueber_faeden_teilen() {
        fn nur_geteilt<T: Send + Sync>() {}
        nur_geteilt::<Oertlichesmodell>();
    }

    /// Und dasselbe fuer den Weg, ueber den das Harness es sieht.
    #[test]
    fn auch_als_modellweg_teilbar() {
        fn nur_geteilt<T: Modellweg + Send + Sync>() {}
        nur_geteilt::<Oertlichesmodell>();
    }
}
