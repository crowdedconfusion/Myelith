//! Der Knoten erfüllt den Rechenweg der Tür (GATEWAY Stufe 3).
//!
//! # ⚑ Die Arbeitsteilung, und warum sie so verläuft
//!
//! Die Tür macht HTTP, Ausweis, Deckel und Beleg. Sie kann **nicht**
//! rechnen lassen: Dazu gehören die Zuteilung, der Sitzungskanal und
//! der Transport, und `myl-gateway` kennt keinen davon. Hier stehen sie
//! zusammen, denn der Knoten ist die Stelle, die alle drei sieht.
//!
//! # ⚑ Der Knoten versiegelt, obwohl er den Klartext ohnehin sieht
//!
//! Das klingt sinnlos und ist es nicht. **Der Weg endet nicht hier.**
//! Heute liegt der Shard auf derselben Maschine; morgen liegt er in
//! einem fremden Pod, und dann trägt genau dieselbe Versiegelung. Wer
//! den Klartext auf die lokale Leitung legt, weil sie kurz ist, hat
//! beim ersten fremden Pod eine zweite Codeform und einen zweiten
//! Fehlerpfad.
//!
//! ⚑ **Und die Bindung braucht es ohnehin.** Sie bindet den Klartext,
//! den der Rechnende erst nach dem Entsiegeln sieht; ohne Siegel gäbe
//! es nichts, wogegen er sie prüfen könnte, und dann rechnete er etwas,
//! das niemand später einer Anfrage zuordnen kann.
//!
//! # Was hier bewusst nicht steht
//!
//! **Die Zuteilung aus der Kette.** Dieser Weg fragt den **lokalen**
//! Shard und keinen fremden Pod. Wer einen fremden fragt, braucht die
//! Zuteilung, den Weg über `myl-net` und die angekündigten Punkte aus
//! dem Block; das ist der nächste Schnitt und keine Zeile, die man hier
//! nebenbei unterbringt.

use std::sync::Mutex;

use myl_siegel::{Endpunkt, Epochenpunkt, Epochenschluessel, Gegenpunkte, Kapselpunkt, Sitzungen, Umschlag};
use myl_types::ids::{EpochId, PodId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag, MAX_NEUE_TOKEN};
use myl_types::ortsleitung::{Ortsantwort, Ortsfrage};
use myl_types::sitzung::Anfragebindung;

use crate::ortsklient::Ortsanschluss;

/// Der Rechenweg über den lokalen Shard-Prozess.
pub struct Ortsweg {
    anschluss: Ortsanschluss,
    pod: PodId,
    epoche: EpochId,
    sitzungen: Mutex<Sitzungen>,
    /// Wer der Shard ist, sobald er es gesagt hat.
    ///
    /// ⚑ **Einmal gefragt und dann behalten.** Die Punkte gelten für
    /// eine Epoche; sie bei jeder Anfrage zu erfragen wäre ein
    /// zusätzlicher Umlauf vor jeder Inferenz.
    shard: Mutex<Option<(Endpunkt, Gegenpunkte)>>,
    /// Wie das Modell heisst, das dieser Knoten anbietet.
    modellname: String,
    /// Der Pipeline-Stand, sobald der Shard ihn genannt hat.
    pipeline: Mutex<Option<myl_types::hash::Hash>>,
    /// Wohin die Abrechnung geht, oder `None`.
    ///
    /// # ⚑ Ohne diesen Kanal ist das Budget eine Absichtserklärung
    ///
    /// Bis zum 2026-09-03 prüfte die Tür den Kontrakt gegen eine
    /// Abschrift und liess durch; **abgebucht wurde nie**, und ein
    /// Nutzer konnte unbegrenzt fragen. `sitzung_ausgeben` hatte
    /// ausserhalb der Ledger-Tests keinen Aufrufer.
    ///
    /// ⚑ **Gebucht wird, was gerechnet wurde, nicht was verlangt war.**
    /// Der Deckel der Anfrage ist eine Zusage, keine Rechnung.
    abrechnung: Option<tokio::sync::mpsc::UnboundedSender<myl_consensus::block::Anweisung>>,
    /// Die laufende Abrechnungsnummer dieses Knotens.
    ///
    /// ⚑ **Sie muss steigen** (siehe `Vorhaben::nummer`), sonst weist
    /// die Kette die zweite Abbuchung als Wiedereinreichung ab. Der
    /// Zähler beginnt bei eins, weil null der Anfangswert im Zustand
    /// ist.
    abrechnungsnummer: std::sync::atomic::AtomicU64,
    /// Wohin die Credits gehen: das Konto dieses Betreibers.
    ///
    /// ⚑ **Er muss in der Positivliste des Kontrakts stehen**, sonst
    /// weist `pruefe` die Abbuchung mit `EmpfaengerNichtGelistet` ab.
    /// Das ist Absicht: Der Inhaber entscheidet, wem seine Sitzung
    /// überhaupt begegnen darf.
    empfaenger: myl_types::ids::Address,
}

impl Ortsweg {
    /// Setzt den Weg zusammen.
    ///
    /// `epochensaat` liefert das eigene Schlüsselmaterial der Epoche.
    pub fn neu(
        anschluss: Ortsanschluss,
        pod: PodId,
        epoche: EpochId,
        ich: Endpunkt,
        eigener: Epochenschluessel,
        modellname: &str,
        empfaenger: myl_types::ids::Address,
    ) -> Self {
        Self {
            anschluss,
            pod,
            epoche,
            // `ich` geht in die Sitzungen und wird sonst nicht
            // gebraucht: ein Feld, das niemand liest, kann jede
            // Bedeutung tragen.
            sitzungen: Mutex::new(Sitzungen::neu(ich, eigener)),
            shard: Mutex::new(None),
            modellname: modellname.to_string(),
            pipeline: Mutex::new(None),
            abrechnung: None,
            abrechnungsnummer: std::sync::atomic::AtomicU64::new(1),
            empfaenger,
        }
    }

    /// Hängt den Abrechnungskanal an.
    ///
    /// ⚑ **Getrennt vom Konstruktor**, damit ein Weg ohne Abrechnung
    /// nicht heimlich entsteht: Wer ihn weglässt, tut es sichtbar, und
    /// der Knoten sagt es im Protokoll.
    pub fn mit_abrechnung(
        mut self,
        an: tokio::sync::mpsc::UnboundedSender<myl_consensus::block::Anweisung>,
    ) -> Self {
        self.abrechnung = Some(an);
        self
    }

    /// Ob dieser Weg abbucht.
    pub fn bucht_ab(&self) -> bool {
        self.abrechnung.is_some()
    }

    /// Fragt den Shard, wer er ist, und merkt es sich.
    async fn shard_erfragen(&self) -> Option<(Endpunkt, Gegenpunkte)> {
        if let Ok(g) = self.shard.lock() {
            if let Some(bekannt) = g.as_ref() {
                return Some(bekannt.clone());
            }
        }
        let Some(Ortsantwort::Gegenstelle {
            endpunkt,
            punkt,
            kapselpunkt,
        }) = self.anschluss.frage(&Ortsfrage::Gegenstelle).await
        else {
            return None;
        };
        let roh: [u8; myl_siegel::KAPSELPUNKT_LEN] = kapselpunkt.try_into().ok()?;
        let paar = (
            Endpunkt::aus_bytes(endpunkt),
            Gegenpunkte {
                punkt: Epochenpunkt::aus_bytes(punkt),
                kapselpunkt: Kapselpunkt::aus_bytes(roh),
            },
        );
        if let Ok(mut g) = self.shard.lock() {
            *g = Some(paar.clone());
        }
        Some(paar)
    }

    /// Den Pipeline-Stand erfragen und merken.
    async fn pipeline_erfragen(&self) -> Option<myl_types::hash::Hash> {
        if let Ok(g) = self.pipeline.lock() {
            if let Some(p) = *g {
                return Some(p);
            }
        }
        let Some(Ortsantwort::Lebenszeichen { pipeline, .. }) =
            self.anschluss.frage(&Ortsfrage::Lebenszeichen).await
        else {
            return None;
        };
        if let Ok(mut g) = self.pipeline.lock() {
            *g = Some(pipeline);
        }
        Some(pipeline)
    }
}

#[async_trait::async_trait]
impl myl_gateway::oai::Rechenweg for Ortsweg {
    async fn rechne(
        &self,
        auftrag: myl_gateway::oai::Rechenauftrag<'_>,
    ) -> Option<myl_gateway::oai::Rechenergebnis> {
        let (gegenstelle, punkte) = self.shard_erfragen().await?;
        let pipeline = self.pipeline_erfragen().await?;

        let versiegelt = {
            let mut sitzungen = self.sitzungen.lock().ok()?;
            let kanal = sitzungen.kanal(self.pod, gegenstelle, &punkte).ok()?;
            Umschlag::schliessen(kanal, auftrag.prompt.as_bytes()).ok()?
        };
        let inferenz = Inferenzauftrag {
            sitzung: auftrag.sitzung,
            bindung: Anfragebindung::neu(auftrag.sitzung, auftrag.prompt.as_bytes(), self.epoche),
            prompt_versiegelt: versiegelt,
            // ⚑ **Der Deckel des Protokolls schlägt den Wunsch des
            // Klienten.** Ein Harness darf mehr verlangen, als das
            // Protokoll erlaubt; es bekommt dann das Erlaubte und keine
            // Ablehnung, denn die Zahl ist eine Obergrenze und kein
            // Versprechen.
            max_token: auftrag.max_token.clamp(1, MAX_NEUE_TOKEN),
            pipeline,
        };
        if inferenz.pruefe_form().is_err() {
            return None;
        }
        let ergebnis = match self.anschluss.frage(&Ortsfrage::Inferenz(inferenz)).await? {
            Ortsantwort::Inferenz(Inferenzantwort::Ergebnis {
                token,
                segment,
                prompt_token,
                text,
                ..
            }) => Some(myl_gateway::oai::Rechenergebnis {
                text,
                // ⚑ **Gezählt vom Shard und nicht hier geschätzt**
                // (Fund 160). Hier stand die Byte-Länge des Prompts, und
                // `usage.prompt_tokens` ist ein Feld mit festgelegter
                // Bedeutung: Ein Klient rechnet damit Kosten. Der
                // Wortschatz liegt beim Shard, also zählt der Shard.
                prompt_token,
                neue_token: token.len() as u64,
                segment: hex::encode(segment.as_bytes()),
            }),
            _ => None,
        };
        // ⚑ **Erst wenn gerechnet wurde, wird abgebucht.** Wer vorher
        // bucht, kassiert für eine Anfrage, die scheitern kann; wer gar
        // nicht bucht, verschenkt jede.
        if let (Some(e), Some(kanal)) = (&ergebnis, &self.abrechnung) {
            let nummer = self
                .abrechnungsnummer
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let vorhaben = myl_types::sitzung::Vorhaben {
                sitzung: auftrag.sitzung_id,
                handelnder: myl_types::ids::Address::aus_schluessel(&auftrag.vollmacht.agent),
                waehrung: myl_types::sitzung::Waehrung::Credits,
                // ⚑ **Ein Token, ein Credit, und mindestens einer.**
                // Eine Anfrage, die nichts erzeugt hat, hat trotzdem
                // einen vollen Durchlauf gekostet; sie umsonst zu
                // rechnen wäre eine Einladung, genau das zu tun.
                betrag: e.neue_token.max(1),
                empfaenger: self.empfaenger,
                bestaetigt_ausgeliefert: false,
                nummer,
            };
            let _ = kanal.send(myl_consensus::block::Anweisung::SitzungAusgeben {
                vorhaben,
                vollmacht: Some(auftrag.vollmacht.clone()),
            });
        }
        ergebnis
    }

    async fn modell(&self) -> Option<myl_gateway::oai::Modellstand> {
        // ⚑ **Gefragt und nicht geraten** (Fund 160). Die erste Fassung
        // las nur den gemerkten Stand und gab „unbekannt" zurück,
        // solange keiner da war; da ein Harness die Modelle **vor** der
        // ersten Anfrage abruft, war das der Normalfall.
        let pipeline = self.pipeline_erfragen().await?;
        Some(myl_gateway::oai::Modellstand {
            name: self.modellname.clone(),
            pipeline: hex::encode(pipeline.as_bytes()),
        })
    }
}
