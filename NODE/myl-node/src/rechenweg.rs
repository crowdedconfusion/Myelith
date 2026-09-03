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
    /// Die Epoche, für die dieser Weg gerade Schlüssel hält.
    ///
    /// ⚑ **Beweglich, seit es eine Rotation gibt** (Fund 165). Eine
    /// feste Epoche im Weg hiesse: An der ersten Epochengrenze
    /// versiegelt der Knoten für eine Epoche, die der Shard nicht mehr
    /// führt, und niemand sagt es.
    epoche: std::sync::atomic::AtomicU64,
    /// Der eigene Endpunkt. ⚑ **Hier gemerkt und nicht aus `Sitzungen`
    /// gezogen**, damit die Rotation ihn nicht verliert: Ein neuer
    /// `Sitzungen`-Satz braucht denselben Endpunkt, sonst redet der
    /// Knoten nach dem Epochenwechsel als jemand anderes.
    ich: Endpunkt,
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
    /// Die eigene Epochenankündigung als Bytes, oder `None`.
    ///
    /// # ⚑ Warum der Shard sie braucht (Fund 165)
    ///
    /// `Umschlag::oeffnen` bildet den Kanal aus **beiden** Punktpaaren.
    /// Der Knoten erfragt die des Shards mit [`Ortsfrage::Gegenstelle`];
    /// umgekehrt gab es bis zum 2026-09-03 nichts, und deshalb konnte
    /// kein Shard je einen echten Umschlag öffnen.
    ///
    /// ⚑ **Unterschrieben, und zwar mit einem echten Schlüssel.** Der
    /// Ausweis der Leitung sagt „du darfst hereinreden", nicht „du bist
    /// der Knoten"; erst die Unterschrift bindet die Punkte an einen
    /// Endpunkt. **Nicht mit `kette::schluessel_fuer`**: Der ist
    /// `probeschluessel(sha256(name)[0])`, also einer von acht öffentlich
    /// ableitbaren, und eine damit unterschriebene Ankündigung fälschte
    /// jeder, der den Knotennamen kennt.
    ankuendigung: Option<Vec<u8>>,
    /// Für welche Epoche die Ankündigung schon angenommen wurde.
    ///
    /// ⚑ **Träge und wiederholbar.** Startet der Shard neu, weist er die
    /// nächste Anfrage ab, und der Knoten kündigt wieder an; ein einmal
    /// beim Start gesetztes Merkmal bliebe stehen und nichts liefe mehr.
    angekuendigt: Mutex<Option<EpochId>>,
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
            epoche: std::sync::atomic::AtomicU64::new(epoche.0),
            ich,
            // `ich` geht in die Sitzungen und wird sonst nicht
            // gebraucht: ein Feld, das niemand liest, kann jede
            // Bedeutung tragen.
            sitzungen: Mutex::new(Sitzungen::neu(ich, eigener)),
            shard: Mutex::new(None),
            modellname: modellname.to_string(),
            pipeline: Mutex::new(None),
            ankuendigung: None,
            angekuendigt: Mutex::new(None),
            abrechnung: None,
            abrechnungsnummer: std::sync::atomic::AtomicU64::new(1),
            empfaenger,
        }
    }

    /// Hängt die eigene, unterschriebene Epochenankündigung an.
    ///
    /// ⚑ **Getrennt vom Konstruktor**, wie die Abrechnung, und aus
    /// demselben Grund: Ein Weg ohne Ankündigung kann nichts
    /// versiegeln, und das soll sichtbar sein und nicht heimlich.
    ///
    /// `bytes` ist eine borsh-kodierte `myl_siegel::Epochenankuendigung`
    /// für **dieselbe** Epoche wie dieser Weg. Sie hier zu bauen hiesse,
    /// den Identitätsschlüssel bis hierher zu reichen; er bleibt, wo er
    /// gelesen wurde.
    pub fn mit_ankuendigung(mut self, bytes: Vec<u8>) -> Self {
        self.ankuendigung = Some(bytes);
        self
    }

    /// Die Epoche, für die dieser Weg gerade Schlüssel hält.
    pub fn epoche(&self) -> EpochId {
        EpochId(self.epoche.load(std::sync::atomic::Ordering::SeqCst))
    }

    /// Wechselt die Epoche: neue Sitzungsschlüssel, neue Ankündigung,
    /// vergessene Gegenstelle.
    ///
    /// ⚑ **Vier Dinge zusammen oder gar nichts.** Sitzungsschlüssel,
    /// Epochenzahl, die eigene Ankündigung und die **gemerkten Punkte
    /// des Shards** gehören zu derselben Epoche. Wer nur eines
    /// austauschte, bekäme einen Weg, der versiegelt und nie geöffnet
    /// wird, ohne dass irgendwo ein Fehler entsteht.
    ///
    /// Die Reihenfolge im Ablauf ist deshalb: erst ankündigen (der
    /// Shard zieht nach), dann die Gegenstelle neu erfragen. Genau so
    /// steht es in `rechne`.
    pub fn epoche_wechseln(&mut self, neue: EpochId, schluessel: Epochenschluessel, ankuendigung: Vec<u8>) {
        if let Ok(mut s) = self.sitzungen.lock() {
            *s = Sitzungen::neu(self.ich, schluessel);
        }
        if let Ok(mut g) = self.shard.lock() {
            *g = None;
        }
        if let Ok(mut g) = self.angekuendigt.lock() {
            *g = None;
        }
        self.ankuendigung = Some(ankuendigung);
        self.epoche
            .store(neue.0, std::sync::atomic::Ordering::SeqCst);
    }

    /// Ob dieser Weg sich ankündigen kann.
    pub fn kann_ankuendigen(&self) -> bool {
        self.ankuendigung.is_some()
    }

    /// Sagt dem Shard, wer hier redet, falls das noch nicht gilt.
    ///
    /// `true` heisst: Der Shard kennt die Punkte dieses Knotens für
    /// diese Epoche. Ohne Ankündigung `true`, denn dann ist die
    /// Gegenstelle anderweitig bekannt (im echten Pod aus der Kette).
    async fn sicherstellen_angekuendigt(&self) -> bool {
        let Some(bytes) = self.ankuendigung.as_ref() else {
            return true;
        };
        if let Ok(g) = self.angekuendigt.lock() {
            if *g == Some(self.epoche()) {
                return true;
            }
        }
        let angenommen = matches!(
            self.anschluss
                .frage(&Ortsfrage::Ankuendigung(bytes.clone()))
                .await,
            Some(Ortsantwort::Angenommen)
        );
        if angenommen {
            if let Ok(mut g) = self.angekuendigt.lock() {
                *g = Some(self.epoche());
            }
        }
        angenommen
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
        // ⚑ **Zuerst sagen, wer hier redet.** Ohne das kennt der Shard
        // die Punkte dieses Knotens nicht und bekommt einen Umschlag,
        // den er nicht öffnen kann. Träge, also heilt es einen
        // Shard-Neustart von selbst.
        if !self.sicherstellen_angekuendigt().await {
            return None;
        }
        let (gegenstelle, punkte) = self.shard_erfragen().await?;
        let pipeline = self.pipeline_erfragen().await?;

        let versiegelt = {
            let mut sitzungen = self.sitzungen.lock().ok()?;
            let kanal = sitzungen.kanal(self.pod, gegenstelle, &punkte).ok()?;
            Umschlag::schliessen(kanal, auftrag.prompt.as_bytes()).ok()?
        };
        let inferenz = Inferenzauftrag {
            sitzung: auftrag.sitzung,
            bindung: Anfragebindung::neu(auftrag.sitzung, auftrag.prompt.as_bytes(), self.epoche()),
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


/// Setzt den Rechenweg der Tuer zusammen, falls alles da ist (Fund 165).
///
/// ⚑ **Hier und nicht in `main.rs`**, und das ist die Lehre aus Fund
/// 166: Was im Binary steht, sieht kein Test. Diese Funktion ist die
/// einzige Stelle, an der ein Betreiber-Rechenweg entsteht, und sie ist
/// aufrufbar.
///
/// # ⚑ Vier Angaben, und keine davon lässt sich erraten
///
/// - **`--ortsleitung` und `--ortsausweis`**: wo der Shard horcht und
///   womit man hereinkommt.
/// - **`--pod`**: die Kennung, die in die Ableitung des Sitzungskanals
///   eingeht. Beide Seiten müssen dieselbe nennen, sonst geht kein
///   Umschlag auf und niemand kann sagen, warum.
/// - **`--konsensschluessel`**: der Schlüssel, der die
///   Epochenankündigung unterschreibt.
///
/// **Fehlt eines, gibt es keinen Weg**, und die Tür sagt das beim
/// Start. Ein Weg, der halb steht, wäre eine Tür, die annimmt und nie
/// antwortet.
#[allow(clippy::too_many_arguments)]
pub fn fuer_betreiber(
    ortsleitung: Option<std::net::SocketAddr>,
    ortsausweis: Option<&std::path::Path>,
    pod: Option<[u8; 32]>,
    modellname: &str,
    konsens: Option<&crate::schluessel::Konsensschluessel>,
    epoche: myl_types::ids::EpochId,
    empfaenger: myl_types::ids::Address,
    abrechnung: tokio::sync::mpsc::UnboundedSender<myl_consensus::block::Anweisung>,
) -> Option<Ortsweg> {
    let adresse = ortsleitung?;
    let ausweis = ortsausweis?;
    let pod = pod?;
    let konsens = konsens?;

    let anschluss = match crate::ortsklient::Ortsanschluss::neu(adresse, ausweis) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("myl-node: Ortsausweis nicht gelesen ({}): {e}", ausweis.display());
            return None;
        }
    };
    let schluessel = myl_siegel::Epochenschluessel::ziehe(epoche);
    let ankuendigung = match konsens.epochenankuendigung(&schluessel) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("myl-node: Epochenankuendigung nicht gebaut: {e:?}");
            return None;
        }
    };
    let ich = konsens.endpunkt();
    eprintln!(
        "myl-node: eigener Endpunkt {} (der Shard erwartet ihn als --knoten)",
        ich.bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    // ⚑ **Nur `Probelauf` ist gefaehrlich**, nicht `NeuErzeugt`. Ein
    // frisch geschriebener Schluessel ist ein echtes Geheimnis; ein
    // abgeleiteter ist aus seinem Namen nachzubauen, und wer ihn kennt,
    // faelscht die Ankuendigung. Der erste Entwurf pruefte auf
    // `!= Datei` und warnte damit vor dem falschen Fall.
    if konsens.herkunft() == crate::schluessel::Herkunft::Probelauf {
        // ⚑ **Gesagt, nicht verschwiegen.** Ein abgeleiteter Schluessel
        // ist aus seinem Namen nachzubauen; wer ihn kennt, faelscht die
        // Ankuendigung, und die ganze Schicht traegt nicht mehr.
        eprintln!(
            "myl-node: WARNUNG: die Epochenankuendigung wird mit einem aus dem Namen \
             abgeleiteten Schluessel unterschrieben. Wer den Namen kennt, faelscht sie. \
             Fuer einen echten Lauf gehoert dahin eine Schluesseldatei."
        );
    }

    // ⚑ **Der Empfaenger kommt von aussen** (Fund 170). Er muss das
    // Konto sein, dessen Schluessel der Knoten haelt, sonst weist
    // `pruefe` die Abbuchung mit `EmpfaengerNichtGelistet` ab; wer ihn
    // hier aus dem Namen ableitete, band ihn an den Probeschluessel.
    Some(
        Ortsweg::neu(
            anschluss,
            myl_types::ids::PodId::new(pod),
            epoche,
            ich,
            schluessel,
            modellname,
            empfaenger,
        )
        .mit_ankuendigung(ankuendigung)
        .mit_abrechnung(abrechnung),
    )
}
