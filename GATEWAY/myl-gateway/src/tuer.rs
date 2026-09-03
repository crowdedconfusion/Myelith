//! Der Endpunkt: ein Zuhörer auf `localhost`.
//!
//! # ⚑ Warum dieser Teil so dünn ist
//!
//! Alles, was entscheidet, steht in [`crate::http`] und
//! [`crate::annahme`] als **reine Funktion** und ist einzeln geprüft.
//! Hier bleibt Lesen, Rufen, Schreiben. **Handgeschriebenes HTTP ist
//! gefährlich, wo Zerlegung auf fremde Eingaben trifft**, und genau die
//! Zerlegung fasst dieser Teil nicht an.
//!
//! # ⚑ Nur die Rückschleife, und das ist keine Voreinstellung
//!
//! [`Tuer::binden`] nimmt keine Adresse entgegen, sondern bindet fest an
//! `127.0.0.1`. Stufe 1 hat **keinen Zugangsschutz, keine
//! Ratenbegrenzung und kein TLS**; eine Adresse als Parameter wäre eine
//! Einladung, sie auf `0.0.0.0` zu setzen, und dann stünde eine Tür ohne
//! Schloss im Netz. **Wer öffentlich hören will, braucht Stufe 2 und
//! nicht ein anderes Argument.**

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use myl_types::ids::{EpochId, SitzungId};
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand};

use crate::annahme::Annahme;
use crate::http::{antwort, kopf_lesen, Httpfehler, MAX_KOPF, MAX_RUMPF};
use crate::zugang::{Anfragehuelle, Kontraktquelle, Zugangsstelle};

/// Eine Quelle, die nichts kennt: nur damit Stufe 1 einen Typ hat.
///
/// ⚑ **Sie wird nie gefragt.** [`Tuer::bedienen`] übergibt `None`; der
/// Typ steht nur da, weil Rust einen braucht.
pub struct LeereQuelle;

impl Kontraktquelle for LeereQuelle {
    fn nachschlagen(&self, _: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        None
    }
}

/// Der Weg, unter dem Anfragen angenommen werden.
pub const WEG: &str = "/inferenz";

/// Der Zuhörer.
pub struct Tuer {
    lauscher: TcpListener,
}

impl Tuer {
    /// Bindet an `127.0.0.1:port`. Port `0` wählt einen freien.
    pub async fn binden(port: u16) -> io::Result<Self> {
        let lauscher = TcpListener::bind(("127.0.0.1", port)).await?;
        Ok(Self { lauscher })
    }

    /// Übernimmt einen schon gebundenen Lauscher.
    ///
    /// ⚑ **Für einen Wirt, der selbst bindet.** Der Knoten beherbergt
    /// die eigene Tür (B6-3) und will beim Binden **melden können, ob
    /// es geklappt hat**, bevor er eine Aufgabe abzweigt. Ein
    /// fehlgeschlagenes Binden in einer abgezweigten Aufgabe wäre eine
    /// Meldung, die niemand liest.
    ///
    /// **Der Wirt trägt damit die Verantwortung für die Adresse.**
    /// [`Tuer::binden`] bindet fest auf die Rückschleife; wer diesen
    /// Weg nimmt, muss selbst wissen, wohin er bindet, und der Knoten
    /// warnt, wenn es nicht die Rückschleife ist.
    pub fn aus_lauscher(lauscher: TcpListener) -> Self {
        Self { lauscher }
    }

    /// Der tatsächlich belegte Port.
    pub fn port(&self) -> io::Result<u16> {
        Ok(self.lauscher.local_addr()?.port())
    }

    /// Nimmt **eine** Verbindung an und beantwortet sie, ohne
    /// Zugangsprüfung (Stufe 1).
    ///
    /// Eine je Aufruf, damit der Aufrufer die Schleife führt: Ein
    /// Gateway, das seine eigene Endlosschleife mitbringt, lässt sich
    /// nicht anhalten und nicht prüfen.
    pub async fn bedienen(&self, annahme: &mut Annahme) -> io::Result<()> {
        let (strom, _) = self.lauscher.accept().await?;
        Self::eine_verbindung(strom, annahme, None::<&mut Zugangsstelle<LeereQuelle>>, EpochId(0), 0)
            .await
    }

    /// Nimmt eine Verbindung an und verlangt einen Sitzungskontrakt
    /// (Stufe 2).
    ///
    /// ⚑ **Der Rumpf ist dann eine [`Anfragehuelle`]** und nicht mehr
    /// der nackte Prompt: Ohne Zugangsdaten im Rumpf gäbe es nichts zu
    /// prüfen. Das ist ein **anderes Drahtformat**, und deshalb ist es
    /// ein anderer Aufruf und kein Schalter: Ein Gateway, das beide
    /// Formen annimmt, müsste raten, welche vorliegt, und Raten ist
    /// genau das, was eine Zerlegung nicht tun darf.
    pub async fn bedienen_mit_zugang<Q: Kontraktquelle>(
        &self,
        annahme: &mut Annahme,
        zugang: &mut Zugangsstelle<Q>,
        jetzt: EpochId,
        jetzt_ms: u64,
    ) -> io::Result<()> {
        let (strom, _) = self.lauscher.accept().await?;
        Self::eine_verbindung(strom, annahme, Some(zugang), jetzt, jetzt_ms).await
    }

    async fn eine_verbindung<Q: Kontraktquelle>(
        mut strom: TcpStream,
        annahme: &mut Annahme,
        zugang: Option<&mut Zugangsstelle<Q>>,
        jetzt: EpochId,
        jetzt_ms: u64,
    ) -> io::Result<()> {
        let mut puffer = Vec::with_capacity(1024);
        let kopf = loop {
            match kopf_lesen(&puffer, WEG) {
                Ok(k) => break k,
                Err(Httpfehler::Unvollstaendig) => {}
                Err(e) => return Self::fehler(&mut strom, &e).await,
            }
            let mut hafen = [0u8; 1024];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                // ⚑ Die Gegenstelle hat aufgelegt, bevor der Kopf stand.
                // Das ist kein Fehler des Klienten, sondern ein Abbruch.
                return Ok(());
            }
            puffer.extend_from_slice(&hafen[..n]);
            if puffer.len() > MAX_KOPF + MAX_RUMPF {
                return Self::fehler(
                    &mut strom,
                    &Httpfehler::RumpfZuGross {
                        bytes: puffer.len(),
                        grenze: MAX_RUMPF,
                    },
                )
                .await;
            }
        };

        // Den Rumpf vollständig lesen. `kopf_lesen` hat die Länge schon
        // gegen `MAX_RUMPF` geprüft.
        while puffer.len() < kopf.rumpf_ab + kopf.laenge {
            let mut hafen = [0u8; 4096];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                // ⚑ **Weniger Rumpf als angekündigt.** Ihn als kurze
                // Anfrage zu nehmen hiesse, eine andere Frage
                // festzuschreiben als die gestellte.
                return Self::fehler(&mut strom, &Httpfehler::Laengenangabe).await;
            }
            puffer.extend_from_slice(&hafen[..n]);
        }
        let rumpf = &puffer[kopf.rumpf_ab..kopf.rumpf_ab + kopf.laenge];

        // ⚑ **Mit Zugang: erst prüfen, dann annehmen.** Andersherum
        // bekäme eine abgelehnte Anfrage eine Sitzungsnummer, und die
        // Nummernfolge verriete, wie oft geklopft wurde.
        //
        // ⚑ **Der Kopf entscheidet, welcher Weg gilt, nicht der
        // Rumpf.** Mit `Authorization: Bearer` ist der Rumpf der nackte
        // Prompt, und genau das macht die Tür harnessfähig; ohne ihn
        // ist er eine [`Anfragehuelle`] mit Unterschrift. **Zu raten,
        // welche Form vorliegt, wäre die Schmuggelstelle**, und geraten
        // wird hier nichts.
        let besitz;
        let mut ausweisweg = None;
        let rumpf: &[u8] = match zugang {
            Some(stelle) => match &kopf.vollmacht {
                Some(roh) => {
                    let Some(vollmacht) = crate::vollmacht::Vollmacht::aus_bearer(roh) else {
                        return Self::abgewiesen(&mut strom).await;
                    };
                    let rahmen = crate::vollmacht::Anfragerahmen {
                        jetzt,
                        sitzung: vollmacht.sitzung().unwrap_or(SitzungId::new([0u8; 32])),
                        credits: 1,
                        modell: myl_types::hash::Hash::sha256(rumpf),
                    };
                    if !stelle
                        .durchlassen_mit_vollmacht(&vollmacht, &rahmen, jetzt_ms)
                        .erlaubt()
                    {
                        return Self::abgewiesen(&mut strom).await;
                    }
                    ausweisweg = Some(crate::zugang::Ausweisweg::Vollmacht);
                    rumpf
                }
                None => {
                    let Ok(huelle) = borsh::from_slice::<Anfragehuelle>(rumpf) else {
                        // ⚑ **Dieselbe Antwort wie bei jeder
                        // Ablehnung.** Ein eigener Fehler für
                        // „unlesbare Hülle" sagte, dass die Hülle
                        // gelesen wurde, und das ist schon eine
                        // Auskunft.
                        return Self::abgewiesen(&mut strom).await;
                    };
                    let anfrage: crate::zugang::Zugangsanfrage = huelle.zugang.clone().into();
                    if !stelle
                        .durchlassen(&anfrage, &huelle.rumpf, jetzt, jetzt_ms)
                        .erlaubt()
                    {
                        return Self::abgewiesen(&mut strom).await;
                    }
                    ausweisweg = Some(crate::zugang::Ausweisweg::Unterschrift);
                    besitz = huelle.rumpf;
                    &besitz
                }
            },
            None => rumpf,
        };

        match annahme.annehmen_mit(rumpf, ausweisweg) {
            Ok(beleg) => {
                let bytes = borsh::to_vec(&beleg).unwrap_or_default();
                let a = antwort(200, "application/octet-stream", &bytes);
                strom.write_all(&a).await?;
            }
            Err(e) => {
                let text = format!("{e:?}");
                let a = antwort(400, "text/plain; charset=utf-8", text.as_bytes());
                strom.write_all(&a).await?;
            }
        }
        strom.flush().await
    }

    /// Bedient die Fläche nach aussen (Stufe 3): `/v1/chat/completions`
    /// und `/v1/models`.
    ///
    /// ⚑ **Zwei Wege und kein Muster.** Die Tür kennt genau die Wege,
    /// die sie bedient; wer mit Mustern arbeitet, bedient irgendwann
    /// einen, den er nicht gemeint hat.
    ///
    /// ⚑ **Der Ausweis ist Pflicht, auch für die Modellliste.** Wer
    /// sie ohne Ausweis herausgäbe, sagte einem Fremden, welcher
    /// Pipeline-Stand hier läuft, und das ist eine Auskunft über den
    /// Betreiber.
    pub async fn bedienen_v1<Q: Kontraktquelle, R: crate::oai::Rechenweg>(
        &self,
        annahme: &mut Annahme,
        zugang: &mut Zugangsstelle<Q>,
        weg: &R,
        jetzt: EpochId,
        jetzt_ms: u64,
    ) -> io::Result<()> {
        let (strom, _) = self.lauscher.accept().await?;
        Self::eine_v1_verbindung(strom, annahme, zugang, weg, jetzt, jetzt_ms).await
    }

    async fn eine_v1_verbindung<Q: Kontraktquelle, R: crate::oai::Rechenweg>(
        mut strom: TcpStream,
        annahme: &mut Annahme,
        zugang: &mut Zugangsstelle<Q>,
        weg: &R,
        jetzt: EpochId,
        jetzt_ms: u64,
    ) -> io::Result<()> {
        use crate::oai::{Chatanfrage, Chatantwort, Modelliste, WEG_CHAT, WEG_MODELLE};
        const ERLAUBT: [(&str, &str); 2] = [("POST", WEG_CHAT), ("GET", WEG_MODELLE)];

        let mut puffer = Vec::with_capacity(1024);
        let kopf = loop {
            match crate::http::kopf_lesen_wege(&puffer, &ERLAUBT) {
                Ok(k) => break k,
                Err(Httpfehler::Unvollstaendig) => {}
                // ⚑ **Der Fehlertext ist die Debug-Form und keine
                // eigene Auskunft.** Wer hier ausformuliert, schreibt
                // eine zweite Fassung derselben Aussage, und die beiden
                // laufen auseinander.
                Err(e) => {
                    return Self::v1_fehler(&mut strom, e.status(), &format!("{e:?}")).await
                }
            }
            let mut hafen = [0u8; 1024];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                return Ok(());
            }
            puffer.extend_from_slice(&hafen[..n]);
            if puffer.len() > MAX_KOPF + MAX_RUMPF {
                return Self::v1_fehler(&mut strom, 413, "der Rumpf ist zu gross").await;
            }
        };
        while puffer.len() < kopf.rumpf_ab + kopf.laenge {
            let mut hafen = [0u8; 4096];
            let n = strom.read(&mut hafen).await?;
            if n == 0 {
                return Self::v1_fehler(&mut strom, 400, "weniger Rumpf als angekuendigt").await;
            }
            puffer.extend_from_slice(&hafen[..n]);
        }
        let rumpf = puffer[kopf.rumpf_ab..kopf.rumpf_ab + kopf.laenge].to_vec();

        // ⚑ **Der Ausweis zuerst, vor jedem Zerlegen.** Wer erst
        // zerlegt, lässt einen Fremden die Arbeit des Zerlegens
        // auslösen und verrät ihm über die Fehlermeldung, wie gut sein
        // JSON war.
        let Some(roh) = kopf.vollmacht.as_deref() else {
            return Self::v1_abgewiesen(&mut strom).await;
        };
        let Some(vollmacht) = crate::vollmacht::Vollmacht::aus_bearer(roh) else {
            return Self::v1_abgewiesen(&mut strom).await;
        };
        let sitzung_id = vollmacht.sitzung().unwrap_or(SitzungId::new([0u8; 32]));
        let rahmen = crate::vollmacht::Anfragerahmen {
            jetzt,
            sitzung: sitzung_id,
            // ⚑ **Geprüft wird gegen den Höchstwert, abgebucht der
            // wirkliche.** Das ist die Reihenfolge jeder Vorauszahlung:
            // Wer nicht zusagen kann, was die Anfrage höchstens kostet,
            // darf sie nicht auslösen; was sie dann wirklich kostet,
            // steht erst danach fest und ist nie mehr.
            //
            // **Vorher stand hier `1`**, also eine Zahl ohne Bezug zur
            // Arbeit. Sie fiel nicht auf, weil nie abgebucht wurde.
            credits: 0,
            modell: myl_types::hash::Hash::sha256(&rumpf),
        };
        if !zugang
            .durchlassen_mit_vollmacht(&vollmacht, &rahmen, jetzt_ms)
            .erlaubt()
        {
            return Self::v1_abgewiesen(&mut strom).await;
        }

        let Some(stand) = weg.modell().await else {
            // ⚑ **Auch die Modellliste braucht die Gegenseite.** Wer
            // hier einen Platzhalter ausgäbe, sagte einem Klienten
            // etwas über einen Stand, den niemand bestätigt hat.
            return Self::v1_fehler(&mut strom, 502, "kein Pod nennt einen Stand").await;
        };
        if kopf.weg == WEG_MODELLE {
            let liste = Modelliste::eine(&stand.name, &stand.pipeline, jetzt_ms / 1000);
            let a = antwort(200, "application/json", &liste.als_json());
            strom.write_all(&a).await?;
            return strom.flush().await;
        }

        let anfrage = match Chatanfrage::lesen(&rumpf) {
            Ok(a) => a,
            Err(e) => {
                return Self::v1_fehler(&mut strom, 400, &e.to_string()).await;
            }
        };

        // ⚑ **Erst annehmen, dann rechnen.** Der Beleg entsteht aus dem
        // Prompt und bindet ihn; wer erst rechnet, hat Arbeit ohne
        // Bindung und kann sie hinterher niemandem zuordnen.
        let prompt = anfrage.prompt();
        let beleg = match annahme
            .annehmen_mit(prompt.as_bytes(), Some(crate::zugang::Ausweisweg::Vollmacht))
        {
            Ok(b) => b,
            Err(e) => return Self::v1_fehler(&mut strom, 400, &format!("{e:?}")).await,
        };

        // ⚑ **Und jetzt, wo der Höchstbetrag feststeht, ein zweites Mal
        // gegen die Vorbehalte.** Beim Eintreffen war er unbekannt; wer
        // hier nicht nachprüft, lässt eine Anfrage rechnen, die er
        // hinterher nicht abbuchen darf, und verschenkt die Arbeit.
        //
        // **Nur die Vorbehalte, nicht die Kette:** Die Signaturen sind
        // oben geprüft, und sie ein zweites Mal zu prüfen verdoppelte
        // genau den Aufwand, den die Ratengrenze beschränkt.
        let deckel = anfrage.token_deckel(256);
        let hoechstens = crate::vollmacht::Anfragerahmen {
            credits: u64::from(deckel),
            ..rahmen
        };
        if !vollmacht.deckt(&hoechstens) {
            return Self::v1_fehler(
                &mut strom,
                402,
                "die Vollmacht deckt diesen Auftrag nicht",
            )
            .await;
        }

        let auftrag = crate::oai::Rechenauftrag {
            sitzung: beleg.sitzung,
            prompt: &prompt,
            max_token: deckel,
            sitzung_id,
            vollmacht: vollmacht.clone(),
        };
        let Some(ergebnis) = weg.rechne(auftrag).await else {
            // ⚑ **502 und kein 500.** „Ich habe niemanden gefunden, der
            // rechnet" ist eine Aussage über die Gegenseite, nicht über
            // diese Tür; ein Klient wiederholt bei 502 sinnvoll.
            return Self::v1_fehler(&mut strom, 502, "kein Pod hat gerechnet").await;
        };

        let modell = if anfrage.model.is_empty() {
            stand.name.clone()
        } else {
            anfrage.model.clone()
        };
        let a = Chatantwort::neu(
            beleg.sitzung,
            jetzt_ms / 1000,
            &modell,
            ergebnis.text,
            ergebnis.prompt_token,
            ergebnis.neue_token,
            &ergebnis.segment,
        );
        let aus = antwort(200, "application/json", &a.als_json());
        strom.write_all(&aus).await?;
        strom.flush().await
    }

    /// Die eine Antwort auf jede Zugangsablehnung von `/v1`.
    ///
    /// ⚑ **Dieselbe Haltung wie `abgewiesen`, nur in der Hülle, die
    /// ein Klient liest.** Der Text nennt keinen Grund.
    async fn v1_abgewiesen(strom: &mut TcpStream) -> io::Result<()> {
        let a = antwort(
            401,
            "application/json",
            &crate::oai::fehler_json("abgelehnt", "invalid_request_error"),
        );
        strom.write_all(&a).await?;
        strom.flush().await
    }

    async fn v1_fehler(strom: &mut TcpStream, status: u16, text: &str) -> io::Result<()> {
        let art = if status >= 500 {
            "api_error"
        } else {
            "invalid_request_error"
        };
        let a = antwort(status, "application/json", &crate::oai::fehler_json(text, art));
        strom.write_all(&a).await?;
        strom.flush().await
    }

    /// Die eine Antwort auf jede Zugangsablehnung.
    ///
    /// ⚑ **Immer dieselbe, ohne Grund und ohne Zahlen.** Wer
    /// unterscheidet, ob ein Kontrakt fehlt, widerrufen ist oder seine
    /// Rate erschöpft hat, betreibt einen Auskunftsdienst über fremde
    /// Kontrakte. `403` und ein leerer Rumpf sagen: nein, und sonst
    /// nichts.
    async fn abgewiesen(strom: &mut TcpStream) -> io::Result<()> {
        let a = antwort(403, "text/plain; charset=utf-8", b"");
        strom.write_all(&a).await?;
        strom.flush().await
    }

    async fn fehler(strom: &mut TcpStream, e: &Httpfehler) -> io::Result<()> {
        let text = format!("{e:?}");
        let a = antwort(e.status(), "text/plain; charset=utf-8", text.as_bytes());
        strom.write_all(&a).await?;
        strom.flush().await
    }
}
