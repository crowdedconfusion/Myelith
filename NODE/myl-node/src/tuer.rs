//! Das eigene Gateway im Knoten (B6-3, Stufe 4, erster Schnitt).
//!
//! # ⚑ Warum die Tür hier läuft und nicht anderswo
//!
//! **Entschieden am 2026-09-03: nur das eigene Gateway.** Der Betreiber
//! ist der Kontoinhaber, die Tür hört auf der Rückschleife, und damit
//! entfallen Namensfindung, Vertrauensfrage und Vergütung.
//!
//! ⚑ **K0s Einwand gilt hier nicht, und das ist der Grund, warum es
//! geht.** K0 sagt: „Eine öffentliche Tür gehört nicht auf die
//! Konsensmaschine", denn ein Überlastangriff gegen die Tür wäre einer
//! gegen die Lebendigkeit des Konsenses. **Diese Tür ist nicht
//! öffentlich.** Wer sie hinausbindet, verlässt den entschiedenen
//! Zuschnitt, und die Hilfe sagt das.
//!
//! # ⚑ Was der Kontraktquelle zugrunde liegt
//!
//! Der Zugang hängt an einem Sitzungskontrakt **aus der Kette**. Der
//! Kettenzustand gehört der Ereignisschleife des Knotens; ihn mit einem
//! Netzdienst zu teilen hiesse, eine Sperre über ein `await` zu halten.
//!
//! **Deshalb legt der Knoten bei jedem Block eine Abschrift ab**, und
//! die Tür liest nur diese. Dieselbe Bauart wie bei der
//! Betriebsbeobachtung, und aus demselben Grund.
//!
//! ⚑ **Die Abschrift ist so frisch wie der letzte Block.** Ein Widerruf
//! wirkt also mit der Verzögerung eines Blocks, nicht sofort. Das
//! gehört gesagt: Wer widerruft, will meist sofort, und zwei Sekunden
//! sind zwei Sekunden.
//!
//! # ⚑ Die Epoche kommt aus der Abschrift, nicht aus dem Start
//!
//! **Fund 166 (2026-09-03).** Bis dahin las `main.rs` die Epoche
//! **einmal vor dem `spawn`** und benutzte den Wert für immer. Ein
//! Sitzungskontrakt lief damit nie ab: `Zugangsstelle` prüft
//! `gueltig_ab` und `gueltig_bis` gegen diese Zahl, und
//! `Befund::Abgelaufen` konnte auf einem laufenden Knoten nicht
//! eintreten.
//!
//! ⚑ **Deshalb steht die Schleifenrunde jetzt hier und nicht in
//! `main.rs`.** Der Fehler war nicht schwer, er war **unerreichbar**:
//! In `main.rs` sieht ihn kein Test. [`eine_anfrage`] ist dieselbe
//! Runde an einem Ort, an dem eine Zusicherung hinkommt.
//!
//! # Was dieser Schnitt **nicht** ist
//!
//! **Er gibt noch nichts an einen Pod.** ⚑ **Und das ist seit dem
//! 2026-09-03 keine fehlende Kiste mehr, sondern eine fehlende
//! Verdrahtung** (Fund 165): `myl_node::rechenweg::Ortsweg` ist gebaut
//! und im Gesamtlauf bewiesen, nur ruft `main.rs` weiterhin
//! `bedienen_mit_zugang` (Stufe 2, `/inferenz`) statt `bedienen_v1` und
//! baut keinen Weg dahinter.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use myl_types::ids::SitzungId;
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand};

/// Der Vorgabeport der eigenen Tür.
///
/// ⚑ **Neben dem Netzport (4150) und dem Beobachtungsport (4151)**,
/// damit ein Betreiber die drei nicht verwechselt und ein Blick in
/// `netstat` sagt, was wozu gehört.
pub const TUER_PORT: u16 = 4160;

/// Die Abschrift der Sitzungskontrakte, die die Tür liest.
///
/// Der Knoten schreibt, die Tür liest. Eine `Mutex` genügt: Die Sperre
/// wird nur für das Kopieren gehalten, nie über ein `await`.
#[derive(Debug, Clone, Default)]
pub struct Kontraktabschrift {
    stand: Arc<Mutex<BTreeMap<SitzungId, (Sitzungskontrakt, Sitzungszustand)>>>,
    /// Die Epoche des zuletzt abgelegten Standes.
    ///
    /// ⚑ **Sie gehört hierher und nicht in eine zweite Kopie** (Fund
    /// 166). Die Tür braucht Kontrakt **und** Epoche, beide aus
    /// derselben Kette und derselben Auffrischung; zwei getrennte
    /// Quellen liefen irgendwann auseinander, und dann prüfte die Tür
    /// einen frischen Kontrakt gegen eine alte Epoche.
    ///
    /// Ein `AtomicU64` statt einer zweiten Sperre: ein Schreiber, viele
    /// Leser, ein Wort.
    epoche: Arc<AtomicU64>,
}

impl Kontraktabschrift {
    /// Eine leere Abschrift.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Legt den Stand der Kette ab.
    ///
    /// **Eine vergiftete Sperre wird übergangen, nicht weitergereicht.**
    /// Der Knoten soll an der Tür nicht sterben.
    pub fn setzen(&self, zustand: &myl_ledger::LedgerState) {
        let neu: BTreeMap<SitzungId, (Sitzungskontrakt, Sitzungszustand)> = zustand
            .sitzungen
            .iter()
            .map(|(id, s)| (*id, (s.kontrakt.clone(), s.zustand)))
            .collect();
        if let Ok(mut g) = self.stand.lock() {
            *g = neu;
        }
        // ⚑ **Nach den Kontrakten und nicht davor.** Wer die Epoche
        // zuerst setzte, hätte einen Augenblick lang die neue Epoche
        // über dem alten Stand: genau die Verwechslung, gegen die die
        // gemeinsame Quelle steht.
        self.epoche.store(zustand.epoch.0, Ordering::Relaxed);
    }

    /// Die Epoche des zuletzt abgelegten Standes.
    pub fn epoche(&self) -> myl_types::ids::EpochId {
        myl_types::ids::EpochId(self.epoche.load(Ordering::Relaxed))
    }

    /// Wie viele Kontrakte die Abschrift führt.
    pub fn anzahl(&self) -> usize {
        self.stand.lock().map(|g| g.len()).unwrap_or(0)
    }
}

/// Bedient **eine** Anfrage an der eigenen Tür, mit der Epoche aus der
/// Abschrift.
///
/// # ⚑ Warum das eine eigene Funktion ist
///
/// Sie ist genau die Schleifenrunde, die bis zum 2026-09-03 in
/// `main.rs` stand, und dort hat sie Fund 166 getragen: eine Epoche, die
/// einmal gelesen und nie aufgefrischt wurde. **Ein `main.rs` ist der
/// Ort, an dem ein Fehler am längsten lebt**, weil ihn kein Test
/// erreicht.
///
/// Die Epoche wird **je Runde** gelesen und an beide Stellen gereicht,
/// die sie brauchen: an die Annahmestelle für die Anfragebindung und an
/// die Zugangsprüfung für das Gültigkeitsfenster des Kontrakts.
///
/// 📌 **Fund 647 (2026-10-26): Gelesen wird nach dem Annehmen.** Die
/// Verbindung kommt angenommen herein ([`myl_gateway::Tuer::annehmen`]),
/// und erst dann stehen Epoche und Uhr fest. Vorher las diese Funktion
/// beides und wartete danach; die erste Anfrage nach einer langen Pause
/// traf so auf die Epoche vom Beginn des Wartens.
pub async fn eine_anfrage(
    tuer: &myl_gateway::Tuer,
    verbindung: myl_gateway::Verbindung,
    annahme: &mut myl_gateway::annahme::Annahme,
    stelle: &mut myl_gateway::zugang::Zugangsstelle<Kontraktabschrift>,
    abschrift: &Kontraktabschrift,
) -> std::io::Result<()> {
    let epoche = abschrift.epoche();
    annahme.epoche_setzen(epoche);
    tuer.mit_zugang_beantworten(verbindung, annahme, stelle, epoche, jetzt_ms())
        .await
}

fn jetzt_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// ⚑ **Wie viele Anfragen die `/v1`-Tuer zugleich bedient** (Sicherheitsaudit
/// A23): so viele, wie die Ortsleitung des Shards zugleich annimmt. Darueber
/// bekommt die Verbindung sofort 503, statt in einer Schlange zu warten.
pub const TUER_ZUGLEICH: usize = 32;

/// Die Pause nach einem Fehler beim Annehmen (etwa keine freien Griffe mehr):
/// Ohne sie drehte die Schleife leer und nahme dem Knoten die Rechenzeit.
const NACH_ANNAHMEFEHLER: std::time::Duration = std::time::Duration::from_millis(100);

/// Bedient die Tuer ohne Rechenweg, eine Verbindung nach der anderen. Hier
/// wird nur gelesen und festgeschrieben, die Lesefrist begrenzt jede.
pub async fn ohne_rechenweg_bedienen(
    tuer: myl_gateway::Tuer,
    mut annahme: myl_gateway::annahme::Annahme,
    mut stelle: myl_gateway::zugang::Zugangsstelle<Kontraktabschrift>,
    abschrift: Kontraktabschrift,
) {
    loop {
        // 📌 **Erst annehmen, dann lesen** (Fund 647): Die Tuer wartet
        // beliebig lange; Epoche und Uhr gelten fuer den Augenblick, in dem
        // die Anfrage da ist.
        let Ok(verbindung) = tuer.annehmen().await else {
            tokio::time::sleep(NACH_ANNAHMEFEHLER).await;
            continue;
        };
        let _ = eine_anfrage(&tuer, verbindung, &mut annahme, &mut stelle, &abschrift).await;
    }
}

/// ⚑ **Bedient die `/v1`-Tuer, mehrere Anfragen zugleich** (A23, Fund 165 fuer
/// den Rechenweg). Bis 0.117.0 lief eine Verbindung nach der anderen, und ein
/// einziger gueltiger Kontrakt belegte die Tuer bis zur Frist eines Auftrags.
///
/// Der Deckel steht **vor** dem Lesen: Ohne freien Platz antwortet die Tuer
/// sofort, fuer alle gleich. Annahme und Zugang werden nur fuer die Buchung
/// gesperrt, nie ueber die Inferenz. Je Sitzung rechnet hoechstens eine Anfrage
/// ([`myl_gateway::tuer::Besetzung`]): Sonst naehme ein Kontrakt alle Plaetze,
/// und zwei Anfragen bestuenden dieselbe Budgetpruefung, von denen die Kette
/// dann eine abweist.
pub async fn v1_bedienen<R, V>(
    tuer: myl_gateway::Tuer,
    annahme: myl_gateway::annahme::Annahme,
    stelle: myl_gateway::zugang::Zugangsstelle<Kontraktabschrift>,
    abschrift: Kontraktabschrift,
    weg: std::sync::Arc<R>,
    vor_jeder_anfrage: V,
) where
    R: myl_gateway::oai::Rechenweg + 'static,
    V: Fn(&R),
{
    let tuer = std::sync::Arc::new(tuer);
    let annahme = std::sync::Arc::new(std::sync::Mutex::new(annahme));
    let stelle = std::sync::Arc::new(std::sync::Mutex::new(stelle));
    let besetzung = std::sync::Arc::new(myl_gateway::tuer::Besetzung::default());
    let plaetze = std::sync::Arc::new(tokio::sync::Semaphore::new(TUER_ZUGLEICH));
    loop {
        let Ok(verbindung) = tuer.annehmen().await else {
            tokio::time::sleep(NACH_ANNAHMEFEHLER).await;
            continue;
        };
        let Ok(platz) = plaetze.clone().try_acquire_owned() else {
            let _ = myl_gateway::Tuer::voll(verbindung).await;
            continue;
        };
        vor_jeder_anfrage(&weg);
        let epoche = abschrift.epoche();
        if let Ok(mut a) = annahme.lock() {
            a.epoche_setzen(epoche);
        }
        let (tuer, annahme, stelle, besetzung, weg) = (tuer.clone(), annahme.clone(), stelle.clone(), besetzung.clone(), weg.clone());
        tokio::spawn(async move {
            // Ein Verbindungsfehler ist kein Grund, die Tuer zu schliessen:
            // Meist ist es eine Gegenstelle, die aufgelegt hat.
            let _ = tuer.v1_beantworten(verbindung, &annahme, &stelle, &besetzung, weg.as_ref(), epoche, jetzt_ms()).await;
            drop(platz);
        });
    }
}

impl myl_gateway::zugang::Kontraktquelle for Kontraktabschrift {
    fn nachschlagen(&self, sitzung: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
        self.stand.lock().ok()?.get(&sitzung).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_gateway::zugang::Kontraktquelle;
    use myl_types::ids::{Address, EpochId};
    use myl_types::sitzung::Grenzen;

    fn kontrakt(b: u8) -> Sitzungskontrakt {
        Sitzungskontrakt {
            inhaber: Address::new([b; 32]),
            agent: Address::new([b; 32]),
            credits: Grenzen::gesperrt(),
            myl: Grenzen::gesperrt(),
            empfaenger_wurzel: myl_types::hash::Hash([0; 32]),
            gueltig_ab: EpochId(0),
            gueltig_bis: EpochId(10),
            max_schritte: 1,
        }
    }

    /// Was in der Kette steht, findet die Tür.
    #[test]
    fn die_abschrift_gibt_wieder_was_in_der_kette_steht() {
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let k = kontrakt(3);
        let id = k.adresse();
        zustand.sitzungen.insert(
            id,
            myl_ledger::state::Sitzung {
                kontrakt: k.clone(),
                zustand: Sitzungszustand::neu(),
            },
        );

        let a = Kontraktabschrift::neu();
        assert_eq!(a.nachschlagen(id), None, "vor dem Abgleich weiss sie nichts");
        a.setzen(&zustand);
        assert_eq!(a.anzahl(), 1);
        assert_eq!(a.nachschlagen(id).map(|(k, _)| k), Some(k));
    }

    /// ⚑ **Ein Widerruf in der Kette erreicht die Tür**, sobald der
    /// nächste Abgleich läuft. Ohne diesen Test wäre die Abschrift eine
    /// Kopie, die niemand auffrischt.
    #[test]
    fn ein_widerruf_erreicht_die_tuer() {
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let k = kontrakt(4);
        let id = k.adresse();
        zustand.sitzungen.insert(
            id,
            myl_ledger::state::Sitzung {
                kontrakt: k,
                zustand: Sitzungszustand::neu(),
            },
        );
        let a = Kontraktabschrift::neu();
        a.setzen(&zustand);
        assert!(!a.nachschlagen(id).expect("da").1.widerrufen);

        zustand.sitzungen.get_mut(&id).expect("da").zustand.widerrufen = true;
        a.setzen(&zustand);
        assert!(
            a.nachschlagen(id).expect("da").1.widerrufen,
            "der Widerruf kam nicht an; die Abschrift wird nicht aufgefrischt"
        );
    }

    /// ⚑ **Die Abschrift trägt die Epoche mit** und frischt sie auf.
    ///
    /// 📌 Ohne diesen Test wäre `epoche()` ein Feld, das immer null
    /// zurückgibt: Genau das war Fund 166, nur eine Ebene höher.
    #[test]
    fn die_abschrift_traegt_die_epoche_und_frischt_sie_auf() {
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let a = Kontraktabschrift::neu();
        assert_eq!(a.epoche(), EpochId(0), "eine frische Abschrift steht bei null");

        zustand.epoch = EpochId(7);
        a.setzen(&zustand);
        assert_eq!(a.epoche(), EpochId(7), "die Epoche kam nicht an");

        zustand.epoch = EpochId(8);
        a.setzen(&zustand);
        assert_eq!(
            a.epoche(),
            EpochId(8),
            "die Epoche wird nicht aufgefrischt; genau das war Fund 166"
        );
    }

    /// ⚑ **Fund 166, an der Naht geprüft: ein Kontrakt, der erst später
    /// gilt, wird jetzt abgewiesen und später angenommen.**
    ///
    /// **Das ist der Test, an dem der Fund hängt.** Der Kontrakt gilt ab
    /// Epoche 5. Steht die Tür bei Epoche 0, muss sie abweisen; wandert
    /// die Kette auf Epoche 7, muss dieselbe Anfrage durchkommen, **ohne
    /// dass irgendetwas neu gestartet wird**.
    ///
    /// 📌 **Die Gegenprobe:** Wer in [`eine_anfrage`] die Zeile
    /// `let epoche = abschrift.epoche();` durch die Startepoche ersetzt,
    /// bekommt hier zweimal 403. Genau so lief `main.rs` bis zum
    /// 2026-09-03. Und wer die Epoche wieder vor dem Annehmen liest,
    /// ebenso (Fund 647): Beim zweiten Mal wartet die Tür schon, als die
    /// Kette wandert.
    #[tokio::test]
    async fn ein_kontrakt_der_erst_spaeter_gilt_kommt_erst_spaeter_durch() {
        use myl_types::vollmacht::{Vollmacht, Vorbehalt};
        use std::io::{Read, Write};

        // Ein Kontrakt, der erst ab Epoche 5 gilt.
        let agent_sk =
            myl_types::bls::BlsSecretKey::key_gen(&[9u8; 32]).expect("Schluessel");
        let agent = Address::aus_schluessel(&agent_sk.public_key().expect("pk"));
        let mut kontrakt = kontrakt(3);
        kontrakt.agent = agent;
        kontrakt.gueltig_ab = EpochId(5);
        kontrakt.gueltig_bis = EpochId(9);
        let s_id = kontrakt.adresse();

        let mut zustand = myl_ledger::LedgerState::genesis(1);
        zustand.sitzungen.insert(
            s_id,
            myl_ledger::state::Sitzung {
                kontrakt,
                zustand: Sitzungszustand::neu(),
            },
        );

        let abschrift = Kontraktabschrift::neu();
        zustand.epoch = EpochId(0);
        abschrift.setzen(&zustand);

        let token = Vollmacht::ausstellen(
            &agent_sk,
            vec![Vorbehalt::NurSitzung(s_id), Vorbehalt::GueltigBis(EpochId(9))],
            [4u8; 32],
        )
        .expect("ausstellen")
        .als_bearer();

        let lauscher = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("binden");
        let port = lauscher.local_addr().expect("adresse").port();
        let tuer = myl_gateway::Tuer::aus_lauscher(lauscher);
        let mut annahme = myl_gateway::annahme::Annahme::neu(1, EpochId(0));
        let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift.clone());

        // Eine Anfrage stellen und die ganze Antwort holen.
        async fn frage(port: u16, token: String) -> Vec<u8> {
            tokio::task::spawn_blocking(move || {
                let rumpf = b"frage";
                let bytes = format!(
                    "POST /inferenz HTTP/1.1\r\nHost: localhost\r\n\
                     Authorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n",
                    rumpf.len()
                );
                let mut strom =
                    std::net::TcpStream::connect(("127.0.0.1", port)).expect("verbinden");
                strom
                    .set_read_timeout(Some(std::time::Duration::from_secs(20)))
                    .expect("Frist");
                strom.write_all(bytes.as_bytes()).expect("Kopf");
                strom.write_all(rumpf).expect("Rumpf");
                strom.flush().expect("leeren");
                let mut aus = Vec::new();
                let _ = strom.read_to_end(&mut aus);
                aus
            })
            .await
            .expect("Klient")
        }

        fn kopf_von(antwort: &[u8]) -> String {
            String::from_utf8_lossy(&antwort[..antwort.len().min(16)]).to_string()
        }
        fn rumpf_von(antwort: &[u8]) -> &[u8] {
            antwort
                .windows(4)
                .position(|f| f == b"\r\n\r\n")
                .map(|i| &antwort[i + 4..])
                .unwrap_or(&[])
        }

        // --- Epoche 0: der Kontrakt gilt noch nicht -------------------
        let dienst = async {
            let v = tuer.annehmen().await.expect("annehmen");
            let _ = eine_anfrage(&tuer, v, &mut annahme, &mut stelle, &abschrift).await;
        };
        let (antwort, _) = tokio::join!(frage(port, token.clone()), dienst);
        let kopf = kopf_von(&antwort);
        // ⚑ **403 und nicht 401**, und der Unterschied ist die
        // Aussage: Der Ausweis trug, der Kontrakt galt nur noch nicht.
        assert!(
            kopf.starts_with("HTTP/1.1 403"),
            "ein Kontrakt, der erst ab Epoche 5 gilt, kam bei Epoche 0 durch: {kopf}"
        );

        // --- Die Kette wandert auf Epoche 7, während die Tür wartet ---
        //
        // 📌 **Fund 647:** Die Tür wartet schon, wenn die Kette wandert,
        // so wie im Betrieb zwischen zwei Anfragen. Wer die Epoche vor dem
        // Annehmen liest, prüft gegen Epoche 0 und weist ab.
        zustand.epoch = EpochId(7);
        let dienst = async {
            let v = tuer.annehmen().await.expect("annehmen");
            let _ = eine_anfrage(&tuer, v, &mut annahme, &mut stelle, &abschrift).await;
        };
        let klient = async {
            tokio::task::yield_now().await;
            abschrift.setzen(&zustand);
            frage(port, token).await
        };
        let (_, antwort) = tokio::join!(dienst, klient);
        let kopf = kopf_von(&antwort);
        assert!(
            kopf.starts_with("HTTP/1.1 200"),
            "die Tuer haengt an der Startepoche fest (Fund 166, 647): {kopf}"
        );

        // ⚑ **Und der Beleg trägt dieselbe Epoche.** Ohne diese Zeile
        // wäre `annahme.epoche_setzen` ungeprüft: Die Zugangsprüfung
        // benutzt die frische Epoche, die **Anfragebindung** aber die
        // der Annahmestelle, und beide sind verschiedene Felder. Die
        // Gegenprobe hat genau das gezeigt.
        let beleg: myl_gateway::annahme::Beleg =
            borsh::from_slice(rumpf_von(&antwort)).expect("Beleg");
        assert_eq!(
            beleg.bindung.epoche,
            EpochId(7),
            "die Anfragebindung nennt eine andere Epoche als die Kette"
        );
    }

    /// Die drei Ports liegen nebeneinander und stoßen sich nicht.
    #[test]
    fn die_drei_ports_sind_verschieden() {
        assert_eq!(TUER_PORT, 4160);
        assert_ne!(TUER_PORT, 4150, "der Netzport");
        assert_ne!(TUER_PORT, 4151, "der Beobachtungsport");
    }

    /// Ein Rechenweg, der bei „warten“ haengt, bis die Probe ihn weiterlaesst.
    struct Prueweg {
        begonnen: tokio::sync::Notify,
        weiter: tokio::sync::Notify,
    }

    #[async_trait::async_trait]
    impl myl_gateway::oai::Rechenweg for Prueweg {
        async fn rechne(&self, a: myl_gateway::oai::Rechenauftrag<'_>) -> Option<myl_gateway::oai::Rechenergebnis> {
            if a.prompt.contains("warten") {
                self.begonnen.notify_one();
                self.weiter.notified().await;
            }
            Some(myl_gateway::oai::Rechenergebnis { text: "ok".to_string(), prompt_token: 1, neue_token: 1, segment: "ab".to_string() })
        }
        async fn modell(&self) -> Option<myl_gateway::oai::Modellstand> {
            Some(myl_gateway::oai::Modellstand { name: "m".to_string(), pipeline: "beef".to_string() })
        }
    }

    /// Eine Abschrift mit einem bezahlbaren Kontrakt je Agent, und je Agent
    /// eine Vollmacht.
    fn zwei_kontrakte() -> (Kontraktabschrift, Vec<String>) {
        use myl_types::vollmacht::{Vollmacht, Vorbehalt};
        let mut zustand = myl_ledger::LedgerState::genesis(1);
        let mut tokens = Vec::new();
        for n in [21u8, 22] {
            let sk = myl_types::bls::BlsSecretKey::key_gen(&[n; 32]).expect("Schluessel");
            let mut k = kontrakt(n);
            k.agent = Address::aus_schluessel(&sk.public_key().expect("pk"));
            k.credits = Grenzen { budget: 10_000, einzellimit: 1_000, schwelle: u64::MAX, zeugenleiter: Vec::new() };
            let id = k.adresse();
            zustand.sitzungen.insert(id, myl_ledger::state::Sitzung { kontrakt: k, zustand: Sitzungszustand::neu() });
            tokens.push(Vollmacht::ausstellen(&sk, vec![Vorbehalt::NurSitzung(id), Vorbehalt::GueltigBis(EpochId(9))], [4u8; 32]).expect("ausstellen").als_bearer());
        }
        let a = Kontraktabschrift::neu();
        a.setzen(&zustand);
        (a, tokens)
    }

    async fn frage_v1(port: u16, token: &str, wort: &str) -> Vec<u8> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let rumpf = format!(r#"{{"model":"m","messages":[{{"role":"user","content":"{wort}"}}],"max_tokens":4}}"#);
        let kopf = format!(
            "POST {} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n",
            myl_gateway::oai::WEG_CHAT,
            rumpf.len()
        );
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.expect("verbinden");
        s.write_all(kopf.as_bytes()).await.expect("Kopf");
        s.write_all(rumpf.as_bytes()).await.expect("Rumpf");
        let mut aus = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(10), s.read_to_end(&mut aus))
            .await
            .expect("die Tuer antwortet nicht in der Frist")
            .expect("lesen");
        aus
    }

    async fn tuer_starten(a: Kontraktabschrift, weg: std::sync::Arc<Prueweg>) -> u16 {
        let lauscher = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("binden");
        let port = lauscher.local_addr().expect("adresse").port();
        let tuer = myl_gateway::Tuer::aus_lauscher(lauscher);
        let annahme = myl_gateway::annahme::Annahme::neu(1, EpochId(0));
        let stelle = myl_gateway::zugang::Zugangsstelle::neu(a.clone());
        tokio::spawn(v1_bedienen(tuer, annahme, stelle, a, weg, |_| {}));
        port
    }

    /// ⚑ **Eine lange Anfrage haelt die Tuer nicht auf** (A23): Waehrend der
    /// Kontrakt des einen rechnet, bekommt der andere seine Antwort.
    /// Gegenprobe: wer in [`v1_bedienen`] das `spawn` durch ein `await`
    /// ersetzt, laesst die zweite Frage in die Frist laufen.
    #[tokio::test]
    async fn eine_lange_anfrage_haelt_die_tuer_nicht_auf() {
        let (a, tokens) = zwei_kontrakte();
        let weg = std::sync::Arc::new(Prueweg { begonnen: tokio::sync::Notify::new(), weiter: tokio::sync::Notify::new() });
        let port = tuer_starten(a, weg.clone()).await;
        let lang = {
            let t = tokens[0].clone();
            tokio::spawn(async move { frage_v1(port, &t, "warten").await })
        };
        weg.begonnen.notified().await;
        let kurz = frage_v1(port, &tokens[1], "sofort").await;
        assert!(kurz.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&kurz));
        weg.weiter.notify_one();
        let lang = lang.await.expect("Klient");
        assert!(lang.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&lang));
    }

    /// ⚑ **Der Deckel steht vor dem Lesen** (A23): Sind alle Plaetze von
    /// schweigenden Verbindungen besetzt, bekommt die naechste sofort 503,
    /// ohne etwas gesendet zu haben. Gegenprobe: ohne `try_acquire_owned`
    /// wartet sie bis zur Lesefrist und bekommt 408.
    #[tokio::test]
    async fn eine_volle_tuer_sagt_es_sofort() {
        use tokio::io::AsyncReadExt;
        let (a, _) = zwei_kontrakte();
        let weg = std::sync::Arc::new(Prueweg { begonnen: tokio::sync::Notify::new(), weiter: tokio::sync::Notify::new() });
        let port = tuer_starten(a, weg).await;
        let mut schweiger = Vec::new();
        for _ in 0..TUER_ZUGLEICH {
            schweiger.push(tokio::net::TcpStream::connect(("127.0.0.1", port)).await.expect("verbinden"));
        }
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.expect("verbinden");
        let mut aus = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(5), s.read_to_end(&mut aus))
            .await
            .expect("keine Antwort, also kein Deckel vor dem Lesen")
            .expect("lesen");
        assert!(aus.starts_with(b"HTTP/1.1 503"), "{}", String::from_utf8_lossy(&aus));
        drop(schweiger);
    }
}
