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
  --protokoll-mb <n>     höchstens so viele MiB Protokoll für diesen Knoten,
                         über alle Dateien und Starts; die älteste fällt
                         zuerst weg (Vorgabe: 128)
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
  --kette-mb <n>         Platzbudget der Kettendatei in MiB (Vorgabe: 2048).
                         Darüber wird an einer Langzeitgrenze verdichtet:
                         Ein Langzeitabzug wird Basis, die Blöcke davor
                         fallen weg. Behalten wird immer alles ab dem
                         vorletzten Langzeitabzug; 0 heißt ohne Grenze.
  --archiv-ausfuhr <ordner>
                         Jede fertige Periode der Geschichte kalt in diesen
                         Ordner schreiben, solange der Knoten sie ganz hat
                         (Datei je Periode, Verzeichnis mit Summen).
  --ohne-kette           Nichts auf die Platte schreiben, JEDER START
                         BEGINNT DANN BEI NULL. Bis zum 2026-09-02 war das
                         die Vorgabe; die sichere Vorgabe ist die
                         umgekehrte, und wer nichts behalten will, sagt es.
                         Zweite Folge: Dieser Knoten kann Nachzueglern dann
                         nur noch die juengsten 256 Bloecke nachliefern,
                         denn mehr haelt er nicht im Arbeitsspeicher.
  --tuer <adr>           Wo das eigene Gateway horcht. Vorgabe
                         127.0.0.1:4160, Weg /inferenz. Der Zugang ist ein
                         Sitzungskontrakt aus der Kette; der API-Schluessel
                         ist eine Vollmacht als Bearer-Token. ACHTUNG: Der
                         entschiedene Zuschnitt ist das EIGENE Gateway auf
                         der Rueckschleife. Wer hinausbindet, macht aus
                         einem Ueberlastangriff gegen die Tuer einen gegen
                         den Konsens.
  --ohne-tuer            Kein eigenes Gateway oeffnen.
  --ortsleitung <adr>    Wo der lokale Shard-Prozess horcht. Vorgabe: aus.
                         Ein Knoten rechnet NICHT selbst; ein Shard laeuft
                         in einem eigenen Prozess, damit ein Absturz beim
                         Rechnen den Konsens nicht anhaelt. Ohne diese
                         Angabe lehnt der Knoten Inferenzauftraege ab,
                         und das ist die ehrliche Antwort.
  --pod <64 hex>         Die Pod-Kennung. Knoten und Shard muessen
                         dieselbe nennen, sonst geht kein Umschlag auf.
                         Noetig, damit die /v1-Tuer rechnen laesst.
  --modell <name>        Wie das Modell nach aussen heisst. Vorgabe
                         sparrow-0.6b-int8.
  --kontoschluessel <datei>
                         Der Schluessel fuer Kettentransaktionen. OHNE IHN
                         nimmt der Knoten einen aus seinem Namen
                         abgeleiteten Schluessel, den jeder nachrechnen
                         kann. Eine ANDERE Datei als --konsensschluessel.
  --speicherorte <datei> Die Speicherorte (speicherorte.toml). Der Knoten
                         zaehlt bei jeder Zustandsaufnahme, was in den
                         Orten mit dem Zweck \"netz\" liegt, und meldet es
                         im Protokoll und unter /metriken.
  --zusage <datei>       Eine Kapazitaetszusage (TOML). Der Knoten
                         unterschreibt sie beim Start mit dem
                         Kontoschluessel und legt <datei>.unterschrift
                         daneben. Braucht --kontoschluessel.
  --netz <name>          Das Netz: probe (Vorgabe, die Probekette mit dem
                         Probemodell) oder test-int4 (das Testnetz mit dem
                         Raven 35B A3B int4). Bestimmt Startwert und
                         Podmodell der Kette.
  --modellzusage <ebenen>:<us>[:<von>-<bis>]
                         Was dieser Knoten fuer das Podmodell zusagt:
                         Ebenen samt Kopf, Mikrosekunden je Ebene und
                         Token, wahlweise den gehaltenen Bereich
                         (Teilablage). Der Knoten meldet sich damit als
                         Miner an und reicht die Zusage ein, beides mit
                         dem Konsensschluessel. OHNE ZUSAGE KEIN POD.
  --zone <name>          Die Zone der Anmeldung (Vorgabe Europe).
  --shard-programm <pfad>
                         myl-shard: Damit startet der Knoten seinen Shard
                         aus der Kette, mit dem Bereich, den die Kette
                         seiner Position zuteilt, und fuehrt ihn ueber die
                         Epochen. Braucht --artefakte.
  --artefakte <ordner>   Das Artefakt des Netzmodells, fuer den Shard und
                         den Dateidienst.
  --shard-training       Den Shard mit Trainingshaelfte starten.
  --konto <64 hex>       Wohin die Ertraege gehen. Vorgabe: die Adresse
                         des unterschreibenden Schluessels selbst.
  --ortsausweis <pfad>   Wo der Ausweis des Shard-Prozesses liegt, Datei
                         oder Verzeichnis. Der Shard legt ihn beim Start
                         ab, unter Unix mit 0600. Gehoert zusammen mit
                         --ortsleitung; eines allein ist ein Fehler.
  --beobachtung <adr>    Wo der Beobachtungsendpunkt horcht. Vorgabe
                         127.0.0.1:4151. Wege: /metriken (Prometheus),
                         /gesundheit (lebt der Prozess), /bereit (kann er
                         bedienen). ACHTUNG: Der Endpunkt hat KEINE
                         Zugangskontrolle und gibt Peerzahl, Hoehe und
                         Latenzspanne preis. Wer ihn ueber die
                         Rueckschleife hinaus bindet, stellt selbst etwas
                         davor.
  --ohne-beobachtung     Keinen Beobachtungsendpunkt oeffnen.
  --stimmsatz <datei>    Stimmsatzdatei mit dem Validator-Satz. Nur damit
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
  --stimmsatzzeile <stake>
                         die eigene Zeile für die Stimmsatzdatei ausgeben
                         und beenden. Erzeugt den Konsensschlüssel, falls
                         er noch nicht existiert. Damit niemand 288 Zeichen
                         Hex von Hand abschreibt: Jeder Betreiber ruft das
                         einmal auf, schickt die Zeile, und jemand setzt
                         die Datei daraus zusammen.
  --erzeuger             dieser Knoten baut die Blöcke. GENAU EINER im Netz:
                         zwei Erzeuger gabeln die Kette sofort, weil niemand
                         entscheidet, welcher Block gilt (das täte BFT).
                         Die übrigen schicken Transaktionen und rechnen nach.
  --blocktakt <ms>       mit --erzeuger: alle so viele Millisekunden ein
                         Block, unabhängig vom Testverkehr (Vorgabe: im
                         Takt des Testverkehrs). Für Läufe, die Epochen
                         in Minuten brauchen. NUR FÜR TESTNETZE.
  --lokale-suche         Knoten im lokalen Netz suchen (mDNS), auch wenn
                         das Netz es nicht vorgibt. Braucht eine
                         Horchadresse auf den Schnittstellen, nicht nur
                         auf der Rückschleife.
  --abzug <hoehe>:<hash> Ohne eigene Kette beim Zustandsabzug dieser
                         Epochengrenze beginnen statt bei Block 0. Der
                         Blockhash ist der Anker: Geglaubt wird nur ein
                         Abzug, dessen Block genau diesen Hash hat und
                         dessen Zustand die Wurzel in diesem Block trifft.
                         Vorgabe: der jüngste Anker des Netzes, falls es
                         einen nennt. Ein Knoten mit Ablage meldet jeden
                         eigenen Abzug im Protokoll als `abzug_angelegt`.
  --ohne-hintergrundpruefung
                         Nach einem Abzug die Blöcke bis dorthin NICHT
                         nachholen und nachrechnen. Vorgabe ist, es zu tun
                         und anzuhalten, wenn die Wurzel nicht trifft.
  --laufzeit <sek>       nach so vielen Sekunden beenden (Vorgabe: bis Strg-C)
  --still                nicht auf den Bildschirm protokollieren
  --hilfe                diese Übersicht

Bei angehaltenem Knoten:
  myl-node archiv-ausfuhr <kettendatei> <ordner> [--netz <name>]
                         Die ganze Kette kalt in den Ordner schreiben.
  myl-node archiv-einfuhr <ordner> <kettendatei> [--netz <name>]
                         Eine Ausfuhr einlesen: in eine leere Kette (der
                         Start rechnet sie ab Genesis nach) oder vor eine
                         verdichtete (geprüft gegen die Historie der Basis).

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
    /// Nur die eigene Stimmsatzdatei-Zeile ausgeben, mit diesem Stake.
    genesiszeile: Option<u64>,
    /// Fristen der BFT-Runden.
    timeouts: myl_consensus::round_change::TimeoutConfig,
    /// Wo die Op-Vektoren liegen.
    konformitaetspfad: std::path::PathBuf,
    /// Ausdrücklich ohne Konformitätsprüfung starten.
    ohne_konformitaet: bool,
    /// Eine Zusage, die der Knoten beim Start unterschreibt.
    zusage: Option<PathBuf>,
}

/// `archiv-ausfuhr <kette> <ordner>` und `archiv-einfuhr <ordner> <kette>`,
/// jeweils mit `--netz <name>` (Vorgabe wie beim Knoten).
fn archivbefehl(roh: &[String]) -> Result<String, String> {
    let mut frei = Vec::new();
    let mut netz = Some(KnotenKonfig::default().netz);
    let mut i = 1;
    while i < roh.len() {
        if roh[i] == "--netz" {
            let name = roh.get(i + 1).ok_or("--netz erwartet einen Namen")?;
            netz = Some(myl_node::netz::Netz::aus_name(name).ok_or_else(|| format!("unbekanntes Netz {name}"))?);
            i += 2;
        } else {
            frei.push(PathBuf::from(&roh[i]));
            i += 1;
        }
    }
    let netz = netz.ok_or("kein Netz")?;
    let [a, b] = &frei[..] else {
        return Err(format!("{} erwartet zwei Pfade", roh[0]));
    };
    if roh[0] == "archiv-ausfuhr" {
        let (mut speicher, w) = myl_node::speicher::Kettenspeicher::oeffnen(a, netz.startwert_hash())
            .map_err(|e| e.to_string())?;
        if speicher.kleinste_hoehe().is_some_and(|h| h > 1) {
            return Err("die Kette ist verdichtet; ausführen kann nur, wer sie ab Genesis hat".into());
        }
        let n = myl_node::archivausfuhr::ausfuhr(b, &netz.startwert_hash(), w.anzahl, false, |ab, bis| {
            speicher.bloecke_ab(ab, bis.min(ab + 511)).unwrap_or_default()
        })?;
        return Ok(format!("{} Blöcke, {n} Dateien neu geschrieben nach {}", w.anzahl, b.display()));
    }
    let leer = std::fs::metadata(b).map(|m| m.len() == 0).unwrap_or(true);
    let n = if leer {
        myl_node::archivausfuhr::einfuhr_in_leere_kette(a, b, netz)?
    } else {
        myl_node::archivausfuhr::einfuhr_vor_basis(a, b, netz)?
    };
    Ok(format!("{n} Blöcke eingeführt nach {}", b.display()))
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

    let mut zusage: Option<PathBuf> = None;
    let mut shard_programm: Option<PathBuf> = None;
    let mut artefakte: Option<PathBuf> = None;
    let mut shard_training = false;
    let mut lokale_suche = false;
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
            "--kette-mb" => {
                konfig.kette_mb = wert(i)?.parse().map_err(|_| "--kette-mb erwartet eine ganze Zahl".to_string())?;
                i += 2;
            }
            // ⚑ Der Gegenweg zur neuen Vorgabe (Fund 122). Wer nichts
            // behalten will, sagt es; wer nichts sagt, behaelt.
            "--archiv-ausfuhr" => { konfig.archiv_ausfuhr = Some(PathBuf::from(wert(i)?)); i += 2; }
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
            "--tuer" => {
                konfig.tuer = Some(
                    wert(i)?
                        .parse()
                        .map_err(|_| "--tuer erwartet adresse:port".to_string())?,
                );
                i += 2;
            }
            "--ohne-tuer" => { konfig.tuer = None; i += 1; }
            "--ortsleitung" => {
                konfig.ortsleitung = Some(
                    wert(i)?
                        .parse()
                        .map_err(|_| "--ortsleitung erwartet adresse:port".to_string())?,
                );
                i += 2;
            }
            "--ortsausweis" => { konfig.ortsausweis = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--pod" => {
                let t = wert(i)?;
                if t.len() != 64 {
                    return Err("--pod erwartet 64 Hexzeichen".to_string());
                }
                let mut b = [0u8; 32];
                for (n, p) in b.iter_mut().enumerate() {
                    *p = u8::from_str_radix(&t[n * 2..n * 2 + 2], 16)
                        .map_err(|_| "--pod ist kein Hex".to_string())?;
                }
                konfig.pod = Some(b);
                i += 2;
            }
            "--modell" => { konfig.modellname = wert(i)?; i += 2; }
            "--speicherorte" => { konfig.speicherorte = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--zusage" => { zusage = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--netz" => {
                let t = wert(i)?;
                konfig.netz = myl_node::netz::Netz::aus_name(&t).ok_or_else(|| {
                    format!("unbekanntes Netz: {t} (bekannt: {})", myl_node::netz::NETZE.map(|n| n.name).join(", "))
                })?;
                i += 2;
            }
            "--modellzusage" => {
                let t = wert(i)?;
                konfig.modellzusage = Some(modellzusage_lesen(&t).ok_or_else(|| {
                    format!("--modellzusage erwartet <ebenen>:<us>[:<von>-<bis>], bekam {t}")
                })?);
                i += 2;
            }
            "--zone" => {
                let t = wert(i)?;
                konfig.zone = myl_types::node_metadata::GeoRegion::all()
                    .into_iter()
                    .find(|z| format!("{z:?}").eq_ignore_ascii_case(&t))
                    .ok_or_else(|| format!("unbekannte Zone: {t}"))?;
                i += 2;
            }
            "--shard-programm" => { shard_programm = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--artefakte" => { artefakte = Some(PathBuf::from(wert(i)?)); i += 2; }
            "--shard-training" => { shard_training = true; i += 1; }
            "--kontoschluessel" => {
                konfig.kontoschluesseldatei = Some(PathBuf::from(wert(i)?));
                i += 2;
            }
            "--konto" => {
                let t = wert(i)?;
                if t.len() != 64 {
                    return Err("--konto erwartet 64 Hexzeichen".to_string());
                }
                let mut b = [0u8; 32];
                for (n, p) in b.iter_mut().enumerate() {
                    *p = u8::from_str_radix(&t[n * 2..n * 2 + 2], 16)
                        .map_err(|_| "--konto ist kein Hex".to_string())?;
                }
                konfig.konto = Some(myl_types::ids::Address::new(b));
                i += 2;
            }
            "--protokolle" => { konfig.protokollverzeichnis = PathBuf::from(wert(i)?); i += 2; }
            "--protokoll-mb" => {
                konfig.protokoll_mb = wert(i)?
                    .parse::<u64>()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or_else(|| "--protokoll-mb erwartet eine Zahl größer null".to_string())?;
                i += 2;
            }
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
            // ⚑ **`--genesis` hiess dieser Schalter bis zum 2026-09-03**,
            // und der Name war die Quelle einer gefaehrlichen
            // Verwechslung: Er liest den **Stimmsatz**, eine
            // Betriebsdatei fuer jeden Netzlauf einschliesslich
            // Probelauf, und hat mit dem **produktiven Genesis** nichts
            // zu tun. Der alte Name wird noch angenommen, aber er warnt.
            "--stimmsatz" => {
                konfig.stimmsatzdatei_pfad = Some(PathBuf::from(wert(i)?));
                i += 2;
            }
            "--genesis" => {
                eprintln!(
                    "myl-node: WARNUNG: --genesis heisst jetzt --stimmsatz. Der alte Name \
                     verwechselt den Stimmsatz mit dem produktiven Genesis und faellt weg."
                );
                konfig.stimmsatzdatei_pfad = Some(PathBuf::from(wert(i)?));
                i += 2;
            }
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
            "--stimmsatzzeile" | "--genesiszeile" => {
                if roh[i] == "--genesiszeile" {
                    eprintln!(
                        "myl-node: WARNUNG: --genesiszeile heisst jetzt --stimmsatzzeile."
                    );
                }
                genesiszeile = Some(
                    wert(i)?
                        .parse()
                        .map_err(|_| "--stimmsatzzeile erwartet einen Stake als Zahl".to_string())?,
                );
                i += 2;
            }
            "--erzeuger" => { konfig.erzeugt_bloecke = true; i += 1; }
            "--blocktakt" => {
                konfig.blocktakt_ms = Some(
                    wert(i)?.parse().map_err(|_| "--blocktakt erwartet Millisekunden".to_string())?,
                );
                i += 2;
            }
            "--lokale-suche" => { lokale_suche = true; i += 1; }
            "--abzug" => {
                konfig.abzug = Some(myl_node::abzug::Anker::aus_text(&wert(i)?).map_err(|e| format!("--abzug: {e}"))?);
                i += 2;
            }
            "--ohne-hintergrundpruefung" => { konfig.hintergrundpruefung = false; i += 1; }
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
    // Das Netz bleibt dasselbe (Startwert, Podmodell); nur die Suche kommt
    // dazu. Einmal je Prozess, daher darf die Abschrift bleiben.
    if lokale_suche && !konfig.netz.lokale_suche {
        konfig.netz = Box::leak(Box::new(myl_node::netz::Netz { lokale_suche: true, ..*konfig.netz }));
    }
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
    if konfig.stimmsatzdatei_pfad.is_some()
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
    if probeschluessel && konfig.stimmsatzdatei_pfad.is_none() && genesiszeile.is_none() {
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
    konfig.shard = match (shard_programm, artefakte) {
        (Some(programm), Some(artefakte)) => {
            Some(myl_node::konfig::Shardkonfig { programm, artefakte, training: shard_training })
        }
        (Some(_), None) => return Err("--shard-programm braucht --artefakte".into()),
        (None, Some(artefakte)) => {
            // ⚑ Ohne Programm kein Shard, aber der Dateidienst liefert aus.
            Some(myl_node::konfig::Shardkonfig { programm: PathBuf::new(), artefakte, training: false })
        }
        (None, None) => None,
    };
    Ok(Some(Argumente {
        konfig,
        laufzeit,
        auf_bildschirm,
        konformitaetspfad,
        ohne_konformitaet,
        probeschluessel,
        genesiszeile,
        timeouts,
        zusage,
    }))
}

/// `<ebenen>:<us>[:<von>-<bis>]` als Zusage für das Podmodell des Netzes;
/// den Abdruck setzt der Knoten.
fn modellzusage_lesen(text: &str) -> Option<myl_types::zusage::Modellzusage> {
    let mut teile = text.split(':');
    let ebenen_hoechstens = teile.next()?.parse().ok()?;
    let ebenenzeit_us = teile.next()?.parse().ok()?;
    let gehalten = match teile.next() {
        None => None,
        Some(b) => {
            let (v, b) = b.split_once('-')?;
            Some((v.parse().ok()?, b.parse().ok()?))
        }
    };
    if teile.next().is_some() {
        return None;
    }
    Some(myl_types::zusage::Modellzusage { abdruck: [0; 32], ebenen_hoechstens, ebenenzeit_us, gehalten })
}

#[tokio::main]
async fn main() {
    // ⚑ **Ausfuhr und Einfuhr der Geschichte** (Archivknoten, Stufe A8):
    //   eigene Befehle für den angehaltenen Knoten, ohne Netz.
    let roh: Vec<String> = std::env::args().skip(1).collect();
    if matches!(roh.first().map(String::as_str), Some("archiv-ausfuhr" | "archiv-einfuhr")) {
        match archivbefehl(&roh) {
            Ok(meldung) => println!("{meldung}"),
            Err(e) => {
                eprintln!("myl-node: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let args = match lies_argumente() {
        Ok(Some(a)) => a,
        Ok(None) => return,
        Err(e) => {
            eprintln!("myl-node: {e}");
            std::process::exit(2);
        }
    };

    // ⚑ **Vor dem Start des Netzes.** Wer nur seine Stimmsatzdatei-Zeile
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
    let stimmsatzdatei_pfad = args.konfig.stimmsatzdatei_pfad.clone();
    let konsensschluessel = args.konfig.konsensschluesseldatei.clone();
    let name = args.konfig.name.clone();
    let kettendatei = args.konfig.kettendatei.clone();
    let probe = args.probeschluessel;
    let timeouts = args.timeouts;
    let beobachtungsadresse = args.konfig.beobachtung;
    let tueradresse = args.konfig.tuer;
    // ⚑ **Der Rechenweg der Tuer** (Fund 165). `starten` nimmt die
    // Konfiguration mit, also wird hier festgehalten, was danach noch
    // gebraucht wird.
    let ortsleitung = args.konfig.ortsleitung;
    let ortsausweis = args.konfig.ortsausweis.clone();
    let podkennung = args.konfig.pod;
    let modellname = args.konfig.modellname.clone();
    let konsenspfad = args.konfig.konsensschluesseldatei.clone();
    let zusage = args.zusage.clone();
    // ⚑ **Der Identitaetsschluessel der Tuer.** Er unterschreibt die
    // Epochenankuendigung, mit der sich der Knoten beim Shard ausweist.
    // Ohne ihn gibt es keinen Shard-Weg, und das ist richtig so: Der
    // Ausweis der Leitung sagt nur „du darfst hereinreden".
    let konsens: Option<myl_node::schluessel::Konsensschluessel> = match &konsenspfad {
        Some(pfad) => match myl_node::schluessel::Konsensschluessel::aus_datei(pfad) {
            Ok(k) => Some(k),
            Err(e) => {
                eprintln!("myl-node: Konsensschluessel nicht gelesen ({}): {e:?}", pfad.display());
                None
            }
        },
        None => None,
    };

    let mut knoten = match Knoten::starten(args.konfig, args.auf_bildschirm).await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("myl-node: Start fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("myl-node: Peer-Id {}", knoten.peer_id());
    // ⚑ **Die Zusage vor allem anderen**: Wer mit einer Zusage startet,
    // deren Unterschrift scheitert, soll das sofort erfahren und nicht
    // einen Knoten haben, der ohne sie laeuft.
    if let Some(pfad) = &zusage {
        match knoten.zusage_unterschreiben(pfad) {
            Ok(ziel) => eprintln!("myl-node: Zusage unterschrieben: {}", ziel.display()),
            Err(e) => {
                eprintln!("myl-node: Zusage nicht unterschrieben: {e}");
                std::process::exit(1);
            }
        }
    }
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
    if !knoten.kontoschluessel_aus_datei() {
        // ⚑ **Fund 170, laut gesagt.** `kette::schluessel_fuer(name)`
        // ist `probeschluessel(sha256(name)[0])`: einer von acht, aus
        // dem Namen nachrechenbar. Wer den Namen kennt, unterschreibt
        // im Namen dieses Knotens.
        eprintln!(
            "myl-node: WARNUNG: kein --kontoschluessel. Kettentransaktionen werden mit \
             einem aus dem Namen abgeleiteten Schluessel unterschrieben, den jeder \
             nachrechnen kann, der den Namen kennt. Nur fuer Probelaeufe."
        );
    }

    // ⚑ **Die eigene Tür** (B6-3, Stufe 4, erster Schnitt). Sie hört
    // auf der Rückschleife; der Verkehr verlässt die Maschine nie, und
    // deshalb braucht sie weder TLS noch Rahmenwerk.
    //
    // ⚑ **Sie gibt noch nichts an einen Pod.** Es gibt keinen Weg
    // dorthin, auf keiner der beiden Seiten; das ist der Rest von
    // Stufe 4. Was hier läuft, nimmt an, prüft den Kontrakt aus der
    // Kette und schreibt fest.
    if let Some(adresse) = tueradresse {
        match tokio::net::TcpListener::bind(adresse).await {
            Ok(lauscher) => {
                let wo = lauscher.local_addr().unwrap_or(adresse);
                if !wo.ip().is_loopback() {
                    eprintln!(
                        "myl-node: WARNUNG: die Tuer horcht auf {wo}, also nicht nur auf der \
                         Rueckschleife. Das verlaesst den entschiedenen Zuschnitt: Ein \
                         Ueberlastangriff gegen die Tuer trifft dann den Konsens mit."
                    );
                }
                let abschrift = knoten.kontraktabschrift();
                // ⚑ **Die Startepoche nur als Startwert** (Fund 166).
                // Ab hier holt sie sich jede Runde aus der Abschrift;
                // bis zum 2026-09-03 stand dieser Wert für immer, und
                // ein Sitzungskontrakt lief nie ab.
                let start = myl_types::ids::EpochId(knoten.kette().zustand().epoch.0);

                // ⚑ **Und jetzt bekommt die Tuer einen Rechenweg**
                // (Fund 165). Bis zum 2026-09-03 bediente sie
                // `/inferenz` und gab nichts an einen Shard: `Ortsweg`,
                // `bedienen_v1` und `mit_abrechnung` hatten null
                // Produktionsaufrufer.
                let empfaenger = knoten.eigenes_konto();
                let weg = myl_node::rechenweg::fuer_betreiber(
                    ortsleitung,
                    ortsausweis.as_deref(),
                    podkennung,
                    &modellname,
                    konsens.as_ref(),
                    start,
                    empfaenger,
                    knoten.abrechnungskanal(),
                );
                match &weg {
                    Some(_) => eprintln!(
                        "myl-node: eigenes Gateway auf http://{wo}/v1/chat/completions (Shard-Weg steht, Abrechnung an)"
                    ),
                    None => eprintln!(
                        "myl-node: kein Shard-Weg, die Tuer bedient nur /inferenz. Fuer /v1: --ortsleitung, --ortsausweis, --pod und --konsensschluessel."
                    ),
                }
                let konsens_fuer_tuer = konsens;
                tokio::spawn(async move {
                    let tuer = myl_gateway::Tuer::aus_lauscher(lauscher);
                    let mut annahme = myl_gateway::annahme::Annahme::neu(1, start);
                    let mut stelle =
                        myl_gateway::zugang::Zugangsstelle::neu(abschrift.clone());
                    let mut weg = weg;
                    loop {
                        // ⚑ **Der Epochenwechsel, bevor die naechste
                        // Anfrage kommt.** `Sitzungen` gilt fuer genau
                        // eine Epoche; ohne diesen Schritt versiegelte
                        // der Knoten nach der ersten Epochengrenze fuer
                        // eine Epoche, die der Shard nicht mehr fuehrt,
                        // und keine Anfrage kaeme mehr durch.
                        if let (Some(w), Some(k)) =
                            (weg.as_mut(), konsens_fuer_tuer.as_ref())
                        {
                            let jetzt = abschrift.epoche();
                            if jetzt != w.epoche() {
                                let s = myl_siegel::Epochenschluessel::ziehe(jetzt);
                                match k.epochenankuendigung(&s) {
                                    Ok(bytes) => {
                                        w.epoche_wechseln(jetzt, s, bytes);
                                        eprintln!(
                                            "myl-node: Sitzungsschluessel auf Epoche {} gewechselt",
                                            jetzt.0
                                        );
                                    }
                                    Err(e) => eprintln!(
                                        "myl-node: Epochenankuendigung gescheitert ({e:?}); der Shard-Weg bleibt auf Epoche {}",
                                        w.epoche().0
                                    ),
                                }
                            }
                        }
                        // Ein Verbindungsfehler ist kein Grund, die
                        // Tuer zu schliessen: Meist ist es eine
                        // Gegenstelle, die aufgelegt hat.
                        match weg.as_ref() {
                            Some(w) => {
                                let _ = myl_node::tuer::eine_v1_anfrage(
                                    &tuer,
                                    &mut annahme,
                                    &mut stelle,
                                    &abschrift,
                                    w,
                                )
                                .await;
                            }
                            None => {
                                let _ = myl_node::tuer::eine_anfrage(
                                    &tuer,
                                    &mut annahme,
                                    &mut stelle,
                                    &abschrift,
                                )
                                .await;
                            }
                        }
                    }
                });
            }
            Err(e) => eprintln!("myl-node: Tuer nicht geoeffnet ({adresse}): {e}"),
        }
    }

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
            stimmsatzdatei_pfad.clone(),
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
    // Ein Halt von innen ist kein gewöhnliches Ende; wer den Knoten startet,
    // soll es am Rückgabewert sehen.
    if let Some(grund) = knoten.angehalten() {
        eprintln!("myl-node: angehalten: {grund}");
        std::process::exit(3);
    }
}

/// Alles zwischen „der Knoten steht" und „die Schleife laeuft".
///
/// Als eigene Funktion, damit `main` das Ganze in ein `select!` gegen
/// die Beendigungswache stellen kann (Fund 140). Inline ginge das
/// nicht: Ein `select!`-Zweig braucht **ein** Future, und der Vorlauf
/// leiht sich den Knoten ueber mehrere `await` hinweg aus.
async fn startvorlauf(
    knoten: &mut Knoten,
    stimmsatzdatei_pfad: Option<PathBuf>,
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

    // Wenn eine Stimmsatzdatei da ist: mitstimmen.
    //
    // ⚑ **Erst hier, nicht beim Start.** Der Propose des Leaders muss
    // durch ein Mesh, und das steht beim Start noch nicht. Ein Knoten,
    // der sofort proposet, redet ins Leere und die Runde hängt, ohne
    // dass jemand etwas falsch gemacht hätte.
    if let Some(pfad) = stimmsatzdatei_pfad {
        if let Err(e) =
            starte_konsens(knoten, &pfad, name, konsensschluessel, probe, timeouts).await
        {
            eprintln!("myl-node: Konsens nicht gestartet: {e}");
            std::process::exit(1);
        }
    }
}

/// Lädt Stimmsatzdatei und Konsensschlüssel und beginnt Runde 0.
///
/// **Der Vorschlag ist aus dem Netz abgeleitet**, nämlich
/// `sha256(genesis_hash ‖ runde)`. Das ist ein Platzhalter für einen
/// echten Block, aber kein beliebiger: Er hängt am Netz, sodass zwei
/// Netze nie denselben Vorschlag haben, und er ist von jedem Knoten
/// nachrechenbar.
async fn starte_konsens(
    knoten: &mut Knoten,
    stimmsatzdatei_pfad: &std::path::Path,
    name: &str,
    schluesseldatei: Option<PathBuf>,
    probe: bool,
    timeouts: myl_consensus::round_change::TimeoutConfig,
) -> Result<(), String> {
    let text = std::fs::read_to_string(stimmsatzdatei_pfad)
        .map_err(|e| format!("{}: {e}", stimmsatzdatei_pfad.display()))?;
    let g = myl_node::stimmsatzdatei::Stimmsatzdatei::aus_text(&text)
        .map_err(|e| format!("{}: {e}", stimmsatzdatei_pfad.display()))?;

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
        "myl-node: Stimmsatzdatei {} ({}), {} Validatoren, Gesamtstake {}",
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

/// Baut die eigene Zeile für die Stimmsatzdatei.
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

