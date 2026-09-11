//! Was ein lokaler Agentenlauf an Werkzeugen mitbekommt.
//!
//! # ⚑ Warum das hier steht und nicht im Bedieninstrument
//!
//! Solange die Verdrahtung in `myl.rs` lag, war sie **nur mit einem
//! geladenen Modell erreichbar**. Damit war der eine Weg, der die
//! Kette Einhaengung, Werkzeugkasten, Manifest, Adresse, Betriebsart
//! wirklich durchlaeuft, auch der teuerste. Geprueft wurde er deshalb
//! nie.
//!
//! ⚑ **Hier ist er ohne Modell zu haben**, und genau dafuer gibt es
//! `Modellweg`: Ein Pruefstand steckt eine Antwort hinein und sieht zu,
//! was der Harness damit macht.

use std::collections::BTreeMap;

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugkasten};

use crate::einstellungen::Agenteneinstellung;
use crate::werkzeuge::Einhaengung;

/// Alles, was ein Lauf ueber seine Werkzeuge wissen muss.
pub struct Ruestung {
    /// Die Werkzeuge selbst.
    pub kasten: Werkzeugkasten,
    /// Ihre Manifeste.
    pub registratur: myl_agent::registratur::Registratur,
    /// Name zu Adresse, hergeleitet aus dem jeweiligen Manifest.
    pub adressen: BTreeMap<String, myl_types::ids::MerkleRoot>,
    /// Die Einhaengung, falls eine gesetzt war.
    pub einhaengung: Option<Einhaengung>,
    /// In welcher Form das Modell angesprochen wird.
    ///
    /// ⚑ **Sie steht hier und nicht als Parameter an `lauf::fahren`**,
    /// denn die Ruestung ist mit ihr gebaut: Die Werkzeugnamen im Kasten
    /// haengen daran. Eine zweite Angabe an der Schleife koennte davon
    /// abweichen, und dann kuendigte die Ansage Namen an, die der Kasten
    /// nicht kennt.
    pub form: myl_local_agent::werkzeug::Ansageform,
    /// Welcher Werkzeugkiste angeboten wurde.
    pub satz: crate::werkzeuge::Werkzeugkiste,
}

impl Ruestung {
    /// Die Zuordnung, wie der Harness sie braucht.
    pub fn zuordnung(&self) -> impl Fn(&str) -> Option<myl_types::ids::MerkleRoot> + '_ {
        move |n: &str| self.adressen.get(n).copied()
    }
}

/// Baut die Ruestung aus den Einstellungen.
///
/// ⚑ **Ohne gesetzte Wurzel gibt es keine Dateiwerkzeuge.** Das
/// Arbeitsverzeichnis stillschweigend zu nehmen waere bequem und gaebe
/// dem Agenten eine Grenze, die davon abhaengt, wo der Nutzer gerade
/// steht.
/// **Wer gefragt wird, bevor geschrieben wird.**
///
/// ⚑ **Ein Rueckruf und keine Einstellung.** Ob gefragt wird, sagt
/// `agent.modus`; **wie** gefragt wird, weiss nur die Oberflaeche: Die
/// Konsole legt eine Zeile vor, das Fenster einen Kasten. Diese Kiste
/// weiss von beidem nichts und soll es auch nicht.
///
/// `None` heisst: nicht fragen. Ein `Some`, das immer `true` gibt, ist
/// etwas anderes und **fuehlt sich fuer den Nutzer auch anders an**,
/// deshalb der Unterschied.
pub type Nachfrage = std::sync::Arc<dyn Fn(&str, &serde_json::Value) -> bool + Send + Sync>;

/// Ein Werkzeug, das vor dem Ausfuehren fragt.
///
/// ⚑ **Es umhuellt, statt einzugreifen.** Das Werkzeug selbst weiss
/// nichts davon, und die Erlaubnisse des Harness gelten unveraendert:
/// **Eine Nachfrage nimmt etwas weg und gibt nie etwas dazu.**
struct Nachfragend {
    inner: Box<dyn Werkzeugausfuehrung>,
    fragen: Nachfrage,
}

impl Werkzeugausfuehrung for Nachfragend {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn ausfuehren(
        &self,
        argumente: &serde_json::Value,
    ) -> Result<String, myl_local_agent::ausfuehrung::Werkzeugfehler> {
        if !(self.fragen)(self.inner.name(), argumente) {
            // ⚑ **Die Absage geht an das Modell zurueck**, als
            // Ergebnis dieses Werkzeugs. Ein stiller Fehlschlag liesse
            // es dieselbe Handlung gleich noch einmal vorschlagen.
            // ⛑ **Der Satz ist fuer ein kleines Modell geschrieben.**
            // „Vom Nutzer abgelehnt" allein liess das 4B-Modell im
            // Probelauf raten, die Datei sei nicht da oder der Zugriff
            // gestoert. **Eine Absage, die wie ein Fehler klingt, wird
            // wie ein Fehler behandelt**, und dann versucht es dieselbe
            // Handlung gleich noch einmal.
            return Err(myl_local_agent::ausfuehrung::Werkzeugfehler {
                grund: "Der Nutzer hat diese Handlung abgelehnt. Sie wurde nicht \
                        ausgefuehrt, und die Datei ist unveraendert. Wiederhole sie \
                        nicht; sage stattdessen, was du sonst tun kannst."
                    .to_string(),
            });
        }
        self.inner.ausfuehren(argumente)
    }
}

pub fn ruesten(
    agent: &Agenteneinstellung,
    form: myl_local_agent::werkzeug::Ansageform,
    satz: crate::werkzeuge::Werkzeugkiste,
    zusaetzlich: Vec<(myl_local_agent::werkzeug::Werkzeug, Box<dyn Werkzeugausfuehrung>)>,
) -> Result<Ruestung, String> {
    ruesten_mit(agent, form, satz, zusaetzlich, None)
}

/// Dasselbe, aber mit einer Nachfrage vor jeder schreibenden Handlung.
pub fn ruesten_mit(
    agent: &Agenteneinstellung,
    form: myl_local_agent::werkzeug::Ansageform,
    satz: crate::werkzeuge::Werkzeugkiste,
    zusaetzlich: Vec<(myl_local_agent::werkzeug::Werkzeug, Box<dyn Werkzeugausfuehrung>)>,
    nachfrage: Option<Nachfrage>,
) -> Result<Ruestung, String> {
    let mut kasten = Werkzeugkasten::neu();
    for (angebot, ausfuehrung) in zusaetzlich {
        let name = angebot.name.clone();
        kasten
            .einhaengen(angebot, ausfuehrung)
            .map_err(|f| format!("Werkzeug {name} haengt nicht: {f:?}"))?;
    }

    let einhaengung = match agent.wurzel.as_deref() {
        None => None,
        Some(pfad) => {
            let ein = Einhaengung::neu(pfad, agent.schreiben)?;
            // ⛑ **Hier stand ein Zeichenkettenvergleich auf den drei
            // Namen**, und das ging genau so lange gut, wie die Namen
            // fest waren. Seit sie an der Ansageform haengen, kaeme eine
            // Umbenennung an einer Stelle hier als „unbekanntes
            // Dateiwerkzeug" heraus, und zwar erst zur Laufzeit. Jetzt
            // liefert `Dateiwerkzeug` beides, den Namen und die
            // Ausfuehrung, aus derselben Aufzaehlung.
            for w in satz.werkzeuge() {
                if w.schreibt() && !ein.darf_schreiben() {
                    continue;
                }
                let name = w.name(form).to_string();
                let angebot = crate::werkzeuge::angebote(&ein, form, satz)
                    .into_iter()
                    .find(|a| a.name == name)
                    .ok_or_else(|| format!("Dateiwerkzeug {name} wird nicht angeboten"))?;
                let mut ausfuehrung: Box<dyn Werkzeugausfuehrung> =
                    w.ausfuehrung(ein.clone(), form);
                // ⚑ **Nur die schreibenden.** Ein Lesewerkzeug, das
                // nachfragt, waere nach drei Fragen abgeschaltet, und
                // dann bestaetigt niemand mehr etwas.
                if w.schreibt() {
                    if let Some(f) = nachfrage.clone() {
                        ausfuehrung = Box::new(Nachfragend { inner: ausfuehrung, fragen: f });
                    }
                }
                kasten
                    .einhaengen(angebot, ausfuehrung)
                    .map_err(|f| format!("Dateiwerkzeug {name} haengt nicht: {f:?}"))?;
            }
            Some(ein)
        }
    };

    // ⚑ **Jedes Werkzeug bekommt ein Manifest, und daraus seine
    // Adresse.** Der Harness sperrt `Unbekannt` in JEDER Betriebsart,
    // weil sich in einen Zustand ohne Gegenstand nicht einwilligen
    // laesst. Der Ausweg ist nicht, die Schranke zu umgehen, sondern
    // das Werkzeug ordentlich anzumelden.
    //
    // ⛑ **Hier stand bis zum 2026-09-08 `Deterministisch` fuer die
    // Uhr**, mit der Begruendung, sie rechne zwar nicht, tue es aber
    // ohne Netz und ohne Dritte. Die Begruendung verwechselt zwei
    // Dinge. `Herkunft::Lokal` sagt „ohne Dritte";
    // `Werkzeugart::Deterministisch` sagt laut eigener Doku
    // „verifizierbar wie ein regulaeres Segment". Zwei Aufrufe der Uhr
    // geben zwei Antworten, also ist sie das nicht.
    //
    // ⚑ **Alle Werkzeuge hier sind `Extern` und `Lokal`**, und daraus
    // folgt sichtbar: In der Vorgabebetriebsart `NurVerankert` sperrt
    // der Harness sie. Genau so soll es sein.
    let mut registratur = myl_agent::registratur::Registratur::neu();
    let mut adressen = BTreeMap::new();
    for angebot in kasten.angebote() {
        let manifest = myl_agent::manifest::Werkzeugmanifest {
            name: angebot.name.clone(),
            anbieter: "myl-client".to_string(),
            revision: "1".to_string(),
            lizenz: "Apache-2.0".to_string(),
            art: myl_agent::manifest::Werkzeugart::Extern,
            herkunft: myl_agent::manifest::Herkunft::Lokal,
        };
        // ⚑ Die Adresse ist der Abdruck des Manifests, also hergeleitet
        // und nicht erfunden.
        let a = registratur
            .nimm_werkzeug(manifest)
            .map_err(|f| format!("Manifest {}: {f:?}", angebot.name))?;
        adressen.insert(angebot.name.clone(), a);
    }

    Ok(Ruestung { kasten, registratur, adressen, einhaengung, form, satz })
}
