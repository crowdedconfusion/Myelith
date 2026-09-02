//! `myl-node` — Kommandozeile des Myelith-Knotens.
//!
//! ```text
//! myl-node --name alpha --port 4150
//! myl-node --name beta  --port 4151 --bootstrap /ip4/…/tcp/4150/p2p/12D3Koo…
//! myl-node --name relais --rolle relais --oeffentlich /ip4/203.0.113.5/tcp/4150
//! ```
//!
//! Der Knoten läuft, bis Strg-C kommt, oder für `--laufzeit` Sekunden.
//! Beim Beenden schreibt er eine letzte Zustandsaufnahme, damit das
//! Protokoll mit einem Bild vom Ende schließt und nicht mittendrin
//! abbricht.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{standard_horchadressen, KnotenKonfig, Rolle};
use myl_node::Knoten;

const HILFE: &str = "\
myl-node — ein Myelith-Netzknoten

  --name <text>          Name im Protokoll (Vorgabe: knoten)
  --port <zahl>          Port für TCP und QUIC (Vorgabe: 4150)
  --horche <multiaddr>   Horchadresse, mehrfach möglich (ersetzt --port)
  --bootstrap <addr>     Einstiegsknoten mit /p2p/-Anteil, mehrfach möglich
  --rolle <wort>         teilnehmer (Vorgabe) oder relais
  --oeffentlich <addr>   eigene erreichbare Adresse, für --rolle relais Pflicht
  --relais <addr>        Relais, über das dieser Knoten erreichbar sein will
  --schluessel <datei>   Schlüsseldatei (Vorgabe: knoten.key)
  --protokolle <verz>    Verzeichnis für Betriebsprotokolle (Vorgabe: logs)
  --aufnahme <sek>       Abstand der Zustandsaufnahmen (Vorgabe: 30)
  --testverkehr <sek>    Takt des Testverkehrs (Vorgabe: keiner). Mit
                         --erzeuger wird in diesem Takt ein Block gebaut,
                         sonst eine Burn-Transaktion geschickt. Ohne das
                         belegt ein Lauf nur, dass die Knoten einander
                         finden, nicht dass Nachrichten fließen und der
                         Zustand übereinstimmt. NUR FÜR TESTNETZE.
  --teilnehmer <name>    Name eines Teilnehmers, mehrfach anzugeben. Daraus
                         entsteht der Satz, gegen den Latenz-Atteste geprüft
                         werden (Audit A10). FEHLT EIN NAME, werden dessen
                         Atteste als unbekannter Aussteller verworfen. Ohne
                         Angabe werden alle Atteste verworfen: Ungeprüfte
                         durchzulassen wäre schlechter.
  --kette <datei>        Blockprotokoll der Kette (Vorgabe: kette.dat).
                         Der Knoten spielt die Datei beim Start nach und
                         rechnet dabei jede Zustandswurzel neu; ein
                         abgebrochener letzter Satz wird verworfen und die
                         Datei gekürzt. Gespeichert werden nur die Blöcke:
                         Höhe, Zustand und letzter Hash folgen daraus.
  --ohne-kette           Nichts auf die Platte schreiben, JEDER START
                         BEGINNT DANN BEI NULL. Bis zum 2026-09-02 war das
                         die Vorgabe; die sichere Vorgabe ist die
                         umgekehrte, und wer nichts behalten will, sagt es.
                         Zweite Folge: Dieser Knoten kann Nachzueglern dann
                         nur noch die juengsten 256 Bloecke nachliefern,
                         denn mehr haelt er nicht im Arbeitsspeicher.
  --beobachtung <adr>    Wo der Beobachtungsendpunkt horcht. Vorgabe
                         127.0.0.1:4151. Wege: /metriken (Prometheus),
                         /gesundheit (lebt der Prozess), /bereit (kann er
                         bedienen). ACHTUNG: Der Endpunkt hat KEINE
                         Zugangskontrolle und gibt Peerzahl, Hoehe und
                         Latenzspanne preis. Wer ihn ueber die
                         Rueckschleife hinaus bindet, stellt selbst etwas
                         davor.
  --ohne-beobachtung     Keinen Beobachtungsendpunkt oeffnen.
  --genesis <datei>      Genesis-Datei mit dem Validator-Satz. Nur damit
                         stimmt dieser Knoten bei BFT-Runden mit; ohne sie
                         hört er zu und rechnet nach. Der Knoten muss mit
                         seinem Konsensschlüssel darin stehen.
  --konsensschluessel <datei>
                         geheimer BLS-Schlüssel für die Stimme (Vorgabe:
                         <name>.konsens.key). GETRENNT von --schluessel,
                         der die Netzidentität trägt: Ein Leck darf nicht
                         beide Ebenen zugleich treffen. Fehlt die Datei,
                         wird eine mit Rechten 0600 angelegt.
  --probe-konsensschluessel
                         Konsensschlüssel aus --name ableiten statt aus
                         einer Datei. WER DEN NAMEN KENNT, KENNT DEN
                         SCHLUESSEL. Nur für Probeläufe; die Herkunft steht
                         danach in jeder Protokollzeile.
  --bft-frist <ms>       Basis-Frist einer BFT-Phase (Vorgabe: 1000 für den
                         Vorschlag, 500 für Stimmen und Commits). Läuft sie
                         ab, wechselt der Knoten die Runde. Der Vorgabewert
                         ist aus der 2-Sekunden-Blockzeit hergeleitet und
                         gegen KEIN echtes Netz geprüft.
  --bft-zuwachs <ms>     Zuwachs der Frist je Runde (Vorgabe: 500). NULL
                         heißt: sicher, aber möglicherweise dauerhaft
                         blockiert, denn erst der Zuwachs überschreitet
                         irgendwann jede reale Nachrichtenlaufzeit.
  --konformitaet <pfad>  Verzeichnis der Op-Vektoren (Vorgabe:
                         INTEGER_LLM/conformance/vectors/op). Der Knoten
                         fährt sie VOR dem Netz und startet nicht, wenn
                         seine Maschine anders rechnet.
  --ohne-konformitaet    ohne diese Prüfung starten. AUSDRÜCKLICH und
                         nicht stillschweigend: Ein Knoten, der anders
                         rechnet, liefert abweichende Segmente, wird
                         geschlachtet und verschmutzt bis dahin den
                         Auftragsstrom. Wer den Schalter setzt, sieht ihn
                         in der Kommandozeile und im Protokoll.
  --genesiszeile <stake> die eigene Zeile für die Genesis-Datei ausgeben
                         und beenden. Erzeugt den Konsensschlüssel, falls
                         er noch nicht existiert. Damit niemand 288 Zeichen
                         Hex von Hand abschreibt: Jeder Betreiber ruft das
                         einmal auf, schickt die Zeile, und jemand setzt
                         die Datei daraus zusammen.
  --erzeuger             dieser Knoten baut die Blöcke. GENAU EINER im Netz:
                         zwei Erzeuger gabeln die Kette sofort, weil niemand
                         entscheidet, welcher Block gilt (das täte BFT).
                         Die übrigen schicken Transaktionen und rechnen nach.
  --laufzeit <sek>       nach so vielen Sekunden beenden (Vorgabe: bis Strg-C)
  --still                nicht auf den Bildschirm protokollieren
  --hilfe                diese Übersicht

Die Schlüsseldatei bestimmt die Identität. Bleibt sie erhalten, behält
der Knoten seine Peer-Id über Neustarts, und nur dann lassen sich die
Protokolle mehrerer Läufe zusammenführen.
";

struct Argumente {
    konfig: KnotenKonfig,
    laufzeit: Option<u64>,
    auf_bildschirm: bool,
    /// Konsensschlüssel aus dem Namen ableiten statt aus einer Datei.
    probeschluessel: bool,
    /// Nur die eigene Genesis-Zeile ausgeben, mit diesem Stake.
    genesiszeile: Option<u64>,
    /// Fristen der BFT-Runden.
    timeouts: myl_consensus::round_change::TimeoutConfig,
    /// Wo die Op-Vektoren liegen.
    konformitaetspfad: std::path::PathBuf,
    /// Ausdrücklich ohne Konformitätsprüfung starten.
    ohne_konformitaet: bool,
}

fn lies_argumente() -> Result<Option<Argumente>, String> {
    let roh: Vec<String> = std::env::args().skip(1).collect();
    if roh.iter().any(|a| a == "--hilfe" || a == "-h" || a == "--help") {
        print!("{HILFE}");
        return Ok(None);
    }

    let mut konfig = KnotenKonfig::default();
    let mut port: u16 = 4150;
    let mut horche: Vec<String> = Vec::new();
    let mut laufzeit = None;
    let mut auf_bildschirm = true;
    let mut konformitaetspfad = myl_node::konformitaetstor::vorgabepfad();
    let mut ohne_konformitaet = false;
    let mut probeschluessel = false;
    let mut genesiszeile: Option<u64> = None;
    let mut bft_frist: Option<u64> = None;
    let mut bft_zuwachs: Option<u64> = None;

    let mut i = 0;
    while i < roh.len() {
        let wert = |i: usize| -> Result<String, String> {
            roh.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} erwartet einen Wert", roh[i]))
        };
        match roh[i].as_str() {
            "--name" => { konfig.name = wert(i)?; i += 2; }
            "--port" => {
                port = wert(i)?.parse().map_err(|_| "--port erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--horche" => { horche.push(wert(i)?); i += 2; }
            "--bootstrap" => { konfig.bootstrap.push(wert(i)?); i += 2; }
            "--rolle" => {
                let t = wert(i)?;
                konfig.rolle = Rolle::aus_text(&t)
                    .ok_or_else(|| format!("unbekannte Rolle: {t} (teilnehmer oder relais)"))?;
                i += 2;
            }
            "--oeffentlich" => { konfig.nat.oeffentliche_adressen.push(wert(i)?); i += 2; }
            "--relais" => { konfig.nat.relais.push(wert(i)?); i += 2; }
            "--schluessel" => { konfig.schluesseldatei = PathBuf::from(wert(i)?); i += 2; }
            "--kette" => { konfig.kettendatei = Some(PathBuf::from(wert(i)?)); i += 2; }
            // ⚑ Der Gegenweg zur neuen Vorgabe (Fund 122). Wer nichts
            // behalten will, sagt es; wer nichts sagt, behaelt.
            "--ohne-kette" => { konfig.kettendatei = None; i += 1; }
            "--beobachtung" => {
                konfig.beobachtung = Some(
                    wert(i)?
                        .parse()
                        .map_err(|_| "--beobachtung erwartet adresse:port".to_string())?,
                );
                i += 2;
            }
            "--ohne-beobachtung" => { konfig.beobachtung = None; i += 1; }
            "--protokolle" => { konfig.protokollverzeichnis = PathBuf::from(wert(i)?); i += 2; }
            "--aufnahme" => {
                konfig.aufnahme_sekunden = wert(i)?
                    .parse()
                    .map_err(|_| "--aufnahme erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--testverkehr" => {
                konfig.testverkehr_sekunden = Some(
                    wert(i)?.parse().map_err(|_| "--testverkehr erwartet eine Zahl".to_string())?,
                );
                i += 2;
            }
            "--teilnehmer" => { konfig.teilnehmer.push(wert(i)?); i += 2; }
            "--genesis" => { konfig.genesisdatei = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--konsensschluessel" => {
                konfig.konsensschluesseldatei = Some(PathBuf::from(wert(i)?));
                i += 2;
            }
            "--probe-konsensschluessel" => { probeschluessel = true; i += 1; }
            "--bft-frist" => {
                bft_frist = Some(
                    wert(i)?.parse().map_err(|_| "--bft-frist erwartet eine Zahl".to_string())?,
                );
                i += 2;
            }
            "--bft-zuwachs" => {
                bft_zuwachs = Some(
                    wert(i)?.parse().map_err(|_| "--bft-zuwachs erwartet eine Zahl".to_string())?,
                );
                i += 2;
            }
            "--genesiszeile" => {
                genesiszeile = Some(
                    wert(i)?
                        .parse()
                        .map_err(|_| "--genesiszeile erwartet einen Stake als Zahl".to_string())?,
                );
                i += 2;
            }
            "--erzeuger" => { konfig.erzeugt_bloecke = true; i += 1; }
            "--laufzeit" => {
                laufzeit = Some(
                    wert(i)?.parse().map_err(|_| "--laufzeit erwartet eine Zahl".to_string())?,
                );
                i += 2;
            }
            "--still" => { auf_bildschirm = false; i += 1; }
            "--konformitaet" => { konformitaetspfad = PathBuf::from(wert(i)?); i += 2; }
            "--ohne-konformitaet" => { ohne_konformitaet = true; i += 1; }
            unbekannt => return Err(format!("unbekannte Angabe: {unbekannt} (--hilfe)")),
        }
    }

    konfig.horchadressen = if horche.is_empty() { standard_horchadressen(port) } else { horche };
    // Der eigene Name gehört immer dazu: Ein Knoten, der seine eigenen
    // Atteste nicht anerkennt, wäre schwer zu erklären.
    if !konfig.teilnehmer.is_empty() && !konfig.teilnehmer.contains(&konfig.name) {
        konfig.teilnehmer.push(konfig.name.clone());
    }
    // Die Schlüsseldatei bekommt den Knotennamen, sonst teilen sich
    // zwei Knoten im selben Verzeichnis eine Identität und damit eine
    // Peer-Id. Das ist beim lokalen Mehrknotenlauf der Normalfall.
    if konfig.schluesseldatei.as_os_str() == "knoten.key" {
        konfig.schluesseldatei = PathBuf::from(format!("{}.key", konfig.name));
    }
    // Aus demselben Grund bekommt auch der Konsensschlüssel den Namen.
    if konfig.genesisdatei.is_some()
        && konfig.konsensschluesseldatei.is_none()
        && !probeschluessel
    {
        konfig.konsensschluesseldatei =
            Some(PathBuf::from(format!("{}.konsens.key", konfig.name)));
    }
    // Für --genesiszeile gilt dieselbe Vorgabe wie fürs Mitstimmen.
    if genesiszeile.is_some() && konfig.konsensschluesseldatei.is_none() && !probeschluessel {
        konfig.konsensschluesseldatei =
            Some(PathBuf::from(format!("{}.konsens.key", konfig.name)));
    }
    if probeschluessel && konfig.genesisdatei.is_none() && genesiszeile.is_none() {
        return Err(
            "--probe-konsensschluessel ohne --genesis: ohne Validator-Satz \
             gibt es nichts zu stimmen"
                .to_string(),
        );
    }
    let mut timeouts = myl_consensus::round_change::TimeoutConfig::default();
    if let Some(ms) = bft_frist {
        // Eine Basis von null hieße: jede Runde verfällt sofort, das Netz
        // wechselt endlos und kommt nie zu einem Block.
        if ms == 0 {
            return Err("--bft-frist 0: jede Runde verfiele sofort".to_string());
        }
        timeouts.propose_ms = ms;
        timeouts.vote_ms = ms;
        timeouts.commit_ms = ms;
    }
    if let Some(ms) = bft_zuwachs {
        timeouts.delta_ms = ms;
    }
    Ok(Some(Argumente {
        konfig,
        laufzeit,
        auf_bildschirm,
        konformitaetspfad,
        ohne_konformitaet,
        probeschluessel,
        genesiszeile,
        timeouts,
    }))
}

#[tokio::main]
async fn main() {
    let args = match lies_argumente() {
        Ok(Some(a)) => a,
        Ok(None) => return,
        Err(e) => {
            eprintln!("myl-node: {e}");
            std::process::exit(2);
        }
    };

    // ⚑ **Vor dem Start des Netzes.** Wer nur seine Genesis-Zeile
    // braucht, soll dafür kein Netz aufmachen und keine Ports belegen.
    if let Some(stake) = args.genesiszeile {
        match eigene_genesiszeile(
            &args.konfig.name,
            args.konfig.konsensschluesseldatei.clone(),
            args.probeschluessel,
            stake,
        ) {
            Ok(zeile) => {
                println!("{zeile}");
                return;
            }
            Err(e) => {
                eprintln!("myl-node: {e}");
                std::process::exit(1);
            }
        }
    }

    // ⚑ **Das Tor, und es steht vor dem Netz.**
    //
    // Die ganze Zusage des Protokolls ist, dass zwei beliebige Rechner
    // dasselbe Ergebnis bekommen. Ein Knoten, dessen Maschine anders
    // rechnet, ist kein langsamer, sondern ein **schädlicher**: Er
    // liefert abweichende Segmente, wird dafür geschlachtet, und bis
    // dahin verschmutzt er den Auftragsstrom.
    //
    // **Nach `--genesiszeile`**, weil das keinen Betrieb aufnimmt und
    // niemandem schadet, aber **vor allem, was ans Netz geht.**
    let torbefund = if args.ohne_konformitaet {
        myl_node::konformitaetstor::Torbefund::Uebersprungen
    } else {
        myl_node::konformitaetstor::pruefe(&args.konformitaetspfad)
    };
    if args.auf_bildschirm {
        eprintln!("myl-node: {}", torbefund.zeile());
    }
    if !torbefund.darf_starten() {
        use myl_node::konformitaetstor::Torbefund;
        match &torbefund {
            Torbefund::Abweichung { fehler, vektoren } => {
                eprintln!(
                    "myl-node: {} von {vektoren} Konformitaetsvektoren weichen ab. \
                     Diese Maschine rechnet anders als das Netz.",
                    fehler.len()
                );
                for f in fehler {
                    eprintln!("myl-node:   {f}");
                }
            }
            Torbefund::Keine { pfad } => {
                eprintln!(
                    "myl-node: keine Konformitaetsvektoren unter {}. Ohne sie ist \
                     nicht feststellbar, ob diese Maschine wie das Netz rechnet.",
                    pfad.display()
                );
                eprintln!(
                    "myl-node: mit --konformitaet <pfad> den Ort angeben oder mit \
                     --ohne-konformitaet ausdruecklich darauf verzichten."
                );
            }
            _ => {}
        }
        std::process::exit(3);
    }

    // Vor dem Start festhalten, was danach gebraucht wird: `starten`
    // nimmt die Konfiguration mit.
    let genesisdatei = args.konfig.genesisdatei.clone();
    let konsensschluessel = args.konfig.konsensschluesseldatei.clone();
    let name = args.konfig.name.clone();
    let kettendatei = args.konfig.kettendatei.clone();
    let probe = args.probeschluessel;
    let timeouts = args.timeouts;
    let beobachtungsadresse = args.konfig.beobachtung;

    let mut knoten = match Knoten::starten(args.konfig, args.auf_bildschirm).await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("myl-node: Start fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("myl-node: Peer-Id {}", knoten.peer_id());
    if kettendatei.is_some() {
        eprintln!(
            "myl-node: Kette bei Höhe {} ({} Blöcke in der Datei)",
            knoten.kette().hoehe(),
            knoten.kette().gespeicherte_bloecke().unwrap_or(0)
        );
    } else {
        eprintln!("myl-node: keine Kettendatei, dieser Start beginnt bei null");
    }
    eprintln!("myl-node: Protokoll {}", knoten.protokollpfad().display());

    // ⚑ **Der Beobachtungsendpunkt, bevor der Vorlauf beginnt**
    // (Fund 129). Gerade waehrend des Aufholens will jemand wissen, wie
    // weit der Knoten ist; ein Endpunkt, der erst danach aufmacht,
    // antwortet genau dann nicht, wenn man ihn braucht.
    //
    // Ein Fehler beim Binden beendet den Knoten **nicht**: Ein besetzter
    // Port ist ein Betriebsfehler, kein Grund, das Netz zu verlassen.
    // Er steht aber sichtbar da, sonst suchte jemand die Metriken an
    // einer Stelle, an der nie etwas horchte.
    if let Some(adresse) = beobachtungsadresse {
        match tokio::net::TcpListener::bind(adresse).await {
            Ok(lauscher) => {
                let wo = lauscher.local_addr().unwrap_or(adresse);
                eprintln!("myl-node: Beobachtung auf http://{wo}/metriken");
                if !wo.ip().is_loopback() {
                    eprintln!(
                        "myl-node: WARNUNG: die Beobachtung horcht auf {wo}, also nicht nur \
                         auf der Rueckschleife. Sie hat keine Zugangskontrolle."
                    );
                }
                tokio::spawn(myl_node::beobachtung::laufen(
                    lauscher,
                    knoten.beobachtungsstelle(),
                ));
            }
            Err(e) => eprintln!("myl-node: Beobachtung nicht geoeffnet ({adresse}): {e}"),
        }
    }

    // ⚑ **Der Startvorlauf laeuft gegen die Wache** (Fund 140).
    //
    // Was jetzt folgt, wartet: bis zu acht Sekunden auf eine
    // QUIC-Adresse, bis zu fuenf auf irgendeine Horchadresse, danach
    // auf ein Mesh fuer den Konsens. Bis zum 2026-09-02 haengte der
    // Signalhandler erst **danach** ein, und ein SIGTERM in diesem
    // Fenster riss den Knoten hart heraus, ohne Abschlusseintrag: also
    // genau das, wogegen Fund 123 gebaut wurde, nur eine Stelle
    // frueher. Wer einen Knoten in einem Container startet und schnell
    // wieder stoppt, traf immer dieses Fenster.
    let wache = knoten.beendigungswache();
    let vorlauf_beendet = tokio::select! {
        grund = wache.warten() => Some(grund),
        _ = startvorlauf(
            &mut knoten,
            genesisdatei.clone(),
            &name,
            konsensschluessel,
            probe,
            timeouts,
        ) => None,
    };
    if let Some(grund) = vorlauf_beendet {
        eprintln!("myl-node: {grund} im Startvorlauf, beende");
        knoten.abschluss(grund).await;
        eprintln!(
            "myl-node: beendet, {} Protokollzeilen in {}",
            knoten.protokollzeilen(),
            knoten.protokollpfad().display()
        );
        return;
    }

    // Ein Weg für beide Fälle: Auch mit --laufzeit muss Strg-C einen
    // Abschlusseintrag schreiben, sonst sieht ein früh beendeter Lauf
    // aus wie ein Absturz.
    knoten
        .laufen_bis(args.laufzeit.map(Duration::from_secs))
        .await;
    eprintln!(
        "myl-node: beendet, {} Protokollzeilen in {}",
        knoten.protokollzeilen(),
        knoten.protokollpfad().display()
    );
}

/// Alles zwischen „der Knoten steht" und „die Schleife laeuft".
///
/// Als eigene Funktion, damit `main` das Ganze in ein `select!` gegen
/// die Beendigungswache stellen kann (Fund 140). Inline ginge das
/// nicht: Ein `select!`-Zweig braucht **ein** Future, und der Vorlauf
/// leiht sich den Knoten ueber mehrere `await` hinweg aus.
async fn startvorlauf(
    knoten: &mut Knoten,
    genesisdatei: Option<PathBuf>,
    name: &str,
    konsensschluessel: Option<PathBuf>,
    probe: bool,
    timeouts: myl_consensus::round_change::TimeoutConfig,
) {
    // Die eigenen Adressen nennen, sobald sie feststehen. Sie sind das,
    // was die anderen Maschinen als --bootstrap brauchen.
    // Erst auf QUIC warten, dann ausgeben: Der Betreiber soll die
    // quic-v1-Adresse weitergeben, also muss sie dastehen.
    let hat_quic = knoten.warte_auf_quic(Duration::from_secs(8)).await;
    if knoten.warte_auf_adresse(Duration::from_secs(5)).await.is_some() {
        // QUIC zuerst: Der Transport folgt der Adresse, die weitergegeben
        // wird. Wer eine /tcp/-Adresse verteilt, bekommt ein reines
        // TCP-Netz, und über UDP gelingt das Lochstanzen durch NAT
        // deutlich zuverlässiger. Die Reihenfolge ist die Empfehlung.
        let mut adressen = knoten.adressen();
        adressen.sort_by_key(|a| !myl_net::ist_quic(a));
        for a in adressen {
            eprintln!("myl-node: erreichbar unter {a}");
        }
        if hat_quic {
            eprintln!("myl-node: für Läufe über das Internet die quic-v1-Adresse weitergeben");
        } else {
            eprintln!(
                "myl-node: WARNUNG: keine quic-v1-Adresse gemeldet. Über TCP allein \
                 gelingt der Durchstich durch Heimrouter oft nicht."
            );
        }
    } else {
        eprintln!("myl-node: noch keine Horchadresse gemeldet");
    }

    // Wenn eine Genesis-Datei da ist: mitstimmen.
    //
    // ⚑ **Erst hier, nicht beim Start.** Der Propose des Leaders muss
    // durch ein Mesh, und das steht beim Start noch nicht. Ein Knoten,
    // der sofort proposet, redet ins Leere und die Runde hängt, ohne
    // dass jemand etwas falsch gemacht hätte.
    if let Some(pfad) = genesisdatei {
        if let Err(e) =
            starte_konsens(knoten, &pfad, name, konsensschluessel, probe, timeouts).await
        {
            eprintln!("myl-node: Konsens nicht gestartet: {e}");
            std::process::exit(1);
        }
    }
}

/// Lädt Genesis und Konsensschlüssel und beginnt Runde 0.
///
/// **Der Vorschlag ist aus dem Netz abgeleitet**, nämlich
/// `sha256(genesis_hash ‖ runde)`. Das ist ein Platzhalter für einen
/// echten Block, aber kein beliebiger: Er hängt am Netz, sodass zwei
/// Netze nie denselben Vorschlag haben, und er ist von jedem Knoten
/// nachrechenbar.
async fn starte_konsens(
    knoten: &mut Knoten,
    genesisdatei: &std::path::Path,
    name: &str,
    schluesseldatei: Option<PathBuf>,
    probe: bool,
    timeouts: myl_consensus::round_change::TimeoutConfig,
) -> Result<(), String> {
    let text = std::fs::read_to_string(genesisdatei)
        .map_err(|e| format!("{}: {e}", genesisdatei.display()))?;
    let g = myl_node::genesis::Genesis::aus_text(&text)
        .map_err(|e| format!("{}: {e}", genesisdatei.display()))?;

    let schluessel = if probe {
        eprintln!(
            "myl-node: WARNUNG: Konsensschlüssel aus dem Namen {name:?} abgeleitet. \
             Wer den Namen kennt, kann in diesem Namen stimmen. Nur für Probeläufe."
        );
        myl_node::schluessel::Konsensschluessel::probe(name).map_err(|e| e.to_string())?
    } else {
        let pfad = schluesseldatei
            .ok_or_else(|| "kein Konsensschlüssel angegeben".to_string())?;
        myl_node::schluessel::Konsensschluessel::aus_datei(&pfad)
            .map_err(|e| e.to_string())?
    };

    eprintln!(
        "myl-node: Genesis {} ({}), {} Validatoren, Gesamtstake {}",
        myl_node::knoten::kurz(&g.hash()),
        g.netz,
        g.validatoren.len(),
        g.gesamtstake()
    );

    // Auf das Mesh warten, sonst geht der Propose ins Leere.
    let mesh = knoten
        .warte_auf_mesh(myl_net::GossipTopic::Consensus, 1, Duration::from_secs(60))
        .await;
    if mesh == 0 {
        return Err(
            "kein Mesh auf /myelith/consensus/1 nach 60 s. Ohne Mesh nimmt Gossipsub \
             keine Nachricht an, und die Runde begänne im Leeren"
                .to_string(),
        );
    }

    // Der Vorschlag hängt am Netz, nicht an der Runde: Ein Leader
    // schlägt in jeder Runde denselben Block vor, solange keine Sperre
    // etwas anderes vorschreibt. Zwei Netze haben nie denselben.
    let mut roh = Vec::with_capacity(40);
    roh.extend_from_slice(g.hash().as_bytes());
    roh.extend_from_slice(b"runde-0");
    let vorschlag = myl_types::hash::Hash::sha256(&roh);

    if !timeouts.is_live() {
        eprintln!(
            "myl-node: WARNUNG: --bft-zuwachs 0. Das Verfahren bleibt sicher, kann \
             aber dauerhaft blockieren: Erst der Zuwachs überschreitet irgendwann \
             jede reale Nachrichtenlaufzeit."
        );
    }
    knoten
        .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts)
        .await
        .map_err(|e| e.to_string())?;
    eprintln!(
        "myl-node: BFT-Runde 0 begonnen, Mesh {mesh}, Frist {} ms mit {} ms Zuwachs je Runde",
        timeouts.propose_ms, timeouts.delta_ms
    );
    Ok(())
}

/// Baut die eigene Zeile für die Genesis-Datei.
///
/// Legt die Schlüsseldatei an, falls sie fehlt: Wer seine Zeile
/// erzeugt, legt damit seine Stimme fest, und die muss von da an
/// dieselbe bleiben.
fn eigene_genesiszeile(
    name: &str,
    schluesseldatei: Option<PathBuf>,
    probe: bool,
    stake: u64,
) -> Result<String, String> {
    let k = if probe {
        eprintln!(
            "myl-node: WARNUNG: Zeile aus einem Probeschlüssel. Wer den Namen \
             {name:?} kennt, kann in diesem Namen stimmen."
        );
        myl_node::schluessel::Konsensschluessel::probe(name).map_err(|e| e.to_string())?
    } else {
        let pfad = schluesseldatei.ok_or_else(|| "kein Konsensschlüssel angegeben".to_string())?;
        let k = myl_node::schluessel::Konsensschluessel::aus_datei(&pfad)
            .map_err(|e| e.to_string())?;
        eprintln!(
            "myl-node: Konsensschlüssel {} ({})",
            pfad.display(),
            k.herkunft().als_text()
        );
        k
    };
    k.genesiszeile(stake).map_err(|e| e.to_string())
}
