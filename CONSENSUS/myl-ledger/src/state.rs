//! Kontenmodell und Ledger-Zustand (Punkt 1.1).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::gegenstand::Manifest;
use myl_types::miner::MinerRegistration;
use myl_types::ids::{Address, EpochId, MerkleRoot, MinerId, PodId, SitzungId};
use myl_types::PoIBundle;
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand};
use myl_types::Hash;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Wie viele Epochen weit die Verstoßhistorie eines Kontos zurückreicht.
///
/// **Eine Aufbewahrungsfrist, keine Politik.** Sie begrenzt, wie viel
/// Zustand ein Konto tragen kann: Nach jedem Vermerk stehen höchstens so
/// viele Einträge in [`AccountState::verstoesse`], wie hier Epochen
/// genannt sind. Ohne diese Grenze wüchse der Konsenszustand mit jedem
/// Urteil weiter, und zwar dauerhaft.
///
/// ⚑ **Wer daran dreht, dreht zugleich an der Slashing-Staffelung.**
/// `myl_tokenomics::WIEDERHOLUNGSFENSTER` **ist** diese Konstante und
/// keine zweite daneben; eine Staffelung über ein längeres Fenster als
/// die Aufbewahrung würde eine Vorgeschichte lesen, die es nicht mehr
/// gibt, und das fiele niemandem auf — der Zähler stünde einfach
/// niedriger. Dieselbe Klasse wie γ und der Kontrollsegment-Vorrat: Ein
/// Wert, der eine Schutzwirkung verstärkt, kann eine andere aufzehren.
pub const VERSTOSS_FENSTER: u64 = 10;

/// Wie viele Epochen nach ihrem Ende eine Session noch im Zustand
/// steht.
///
/// ⚑ **Eine Aufbewahrungsfrist aus demselben Grund wie
/// [`VERSTOSS_FENSTER`], und zwar ein dringenderer.** Session-Kontrakte
/// legt jeder Nutzer selbst an; ohne Frist wüchse der Konsenszustand
/// mit jedem jemals eröffneten Kontrakt weiter, und die Größe hinge an
/// einer Eingabe, die ein Angreifer bestimmt.
///
/// **Aufgeräumt wird in einem Übergang, nicht nebenbei**
/// ([`crate::transitions::sitzung_aufraeumen`]): Ein Aufräumen beim
/// Lesen machte den Zustand davon abhängig, **wer wann gelesen hat**,
/// und zwei Knoten mit verschiedener Lesereihenfolge kämen zu
/// verschiedenen Verpflichtungen. Dieselbe Lehre wie bei
/// [`LedgerState::verstoesse_im_fenster`].
pub const SITZUNG_NACHFRIST: u64 = 100;

/// Eine Agenten-Session im Zustand: der unveränderliche Kontrakt und
/// was unter ihm schon verbraucht ist (Whitepaper Kap. 8.2).
///
/// **Der Kontrakt liegt hier, obwohl seine Adresse der Schlüssel ist**,
/// und nicht etwa nur der Verbrauch. Ein Knoten muss die Grenzen prüfen
/// können, ohne sie von irgendwem gezeigt zu bekommen; genau darin
/// besteht der Unterschied zwischen „vom Konsens durchgesetzt" und „vom
/// Client behauptet".
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Sitzung {
    /// Die Grenzen, wie der Inhaber sie gesetzt hat.
    pub kontrakt: Sitzungskontrakt,
    /// Was davon verbraucht ist.
    pub zustand: Sitzungszustand,
}

/// Ein Eintrag der Verstoßhistorie: wie oft ein Konto in **einer**
/// Epoche geschlachtet wurde.
///
/// **Je Epoche ein Eintrag, nicht je Verstoß.** Ein Konto, das in
/// derselben Epoche zehnmal auffällt, trägt einen Eintrag mit `anzahl =
/// 10` statt zehn Einträgen. Sonst hinge die Größe des Konsenszustands
/// daran, wie oft jemand auffällt, und das ist genau die Größe, die ein
/// Angreifer selbst bestimmt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verstoss {
    /// Die Epoche, in der die Verstöße gebucht wurden.
    pub epoche: EpochId,
    /// Wie viele es in dieser Epoche waren.
    pub anzahl: u32,
}

/// Zustand eines Kontos (Adresse).
///
/// - `balance`: verfügbare MYL-Kleinstbeträge
/// - `staked`: als Validator-/Miner-Stake gebundene MYL-Kleinstbeträge
/// - `credits`: noch nicht verbrauchte Inferenz-Credits (vTFE),
///   aufsteigend nach Verfalls-Epoche geordnet (Ausgabereihenfolge:
///   zuerst verfallende Credits, siehe `credit_spend`).
/// - `verstoesse`: Verstoßhistorie, aufsteigend nach Epoche, gekürzt auf
///   [`VERSTOSS_FENSTER`] (siehe [`LedgerState::verstoesse_im_fenster`]).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AccountState {
    pub balance: u64,
    /// Zähler gegen Wiedereinspielung: die nächste gültige
    /// Transaktionsnummer dieses Kontos.
    ///
    /// ⚑ **Ohne ihn ist jede unterschriebene Transaktion beliebig oft
    /// gültig.** Wer eine Überweisung einmal im Netz sieht, kann
    /// dieselben Bytes erneut einreichen, und sie sind weiterhin korrekt
    /// unterschrieben. Der Zähler macht aus „diese Anweisung stammt von
    /// mir" ein „diese Anweisung stammt von mir und gilt genau einmal".
    ///
    /// **Strenge Gleichheit, keine Lücken.** `tx.nonce == konto.nonce`
    /// oder die Transaktion gilt nicht. Eine Fensterlogik erlaubte
    /// Umordnung, und zwei Knoten mit verschiedener Reihenfolge kämen zu
    /// verschiedenen Zuständen.
    pub nonce: u64,
    pub staked: u64,
    /// Gekündigter Einsatz, nach der Epoche seiner Freigabe.
    ///
    /// # ⚑ Warum er nicht einfach zurück ins Guthaben geht (Punkt B11)
    ///
    /// **Ein Einsatz, den man sofort abziehen kann, ist keiner.** Wer
    /// falsch rechnet, zöge ab, bevor das Urteil da ist, und die
    /// Schlachtung fände ein leeres Konto. Die Sperrfrist ist deshalb
    /// so lang wie die Streitfrist.
    ///
    /// ⚑ **Und er haftet hier weiter.** Was gekündigt ist, zählt zur
    /// Schlachtmasse, bis es abgeholt wurde
    /// ([`crate::einsatz::schlachtbar`]). Zählte man es nicht
    /// mit, wäre die Kündigung genau der Fluchtweg, den die Frist
    /// schliessen soll.
    ///
    /// **Nach Freigabe-Epoche geschlüsselt und damit begrenzt.**
    /// Kündigungen derselben Epoche werden zusammengelegt, es gibt also
    /// höchstens einen Eintrag je Freigabe-Epoche und nie mehr als
    /// [`crate::einsatz::MAX_OFFENE_KUENDIGUNGEN`]. Eine Liste statt einer Karte wäre
    /// unbegrenzt gewachsen, und das ist die Klasse von Fund 144.
    ///
    /// **Angehängt und nicht eingefügt:** Die Feldreihenfolge ist
    /// Konsensvertrag.
    pub gekuendigt: BTreeMap<u64, u64>,
    pub credits: Vec<myl_types::InferenceCredit>,
    /// Wann dieses Konto geschlachtet wurde, je Epoche gezählt.
    ///
    /// **Ein Konsensfeld.** Es geht in [`LedgerState::commitment`] ein,
    /// weil die Slashing-Staffelung daraus folgt: Zwei Knoten, die
    /// verschiedene Vorgeschichten führen, schlachten verschieden hoch
    /// und kommen zu verschiedenen Zuständen.
    pub verstoesse: Vec<Verstoss>,
}

impl AccountState {
    /// Leeres Konto.
    pub fn empty() -> Self {
        Self {
            balance: 0,
            nonce: 0,
            staked: 0,
            gekuendigt: BTreeMap::new(),
            credits: Vec::new(),
            verstoesse: Vec::new(),
        }
    }

    /// ⚑ **Ein Konto, das in `epoche` entsteht** (Skalierung, Festlegung des
    /// Projektinhabers vom 2026-10-29): leer, mit dem **Nonce-Boden**
    /// `epoche · 2³²`. Leere Konten fallen weg
    /// ([`LedgerState::leere_konten_entfernen`]); entsteht an derselben
    /// Adresse später wieder eines, gilt keine Transaktion des früheren
    /// mehr, denn deren Nummern liegen alle darunter. So braucht der Schutz
    /// vor Wiederholung weder ein neues Feld in der Transaktion noch einen
    /// Grabstein im Zustand.
    pub fn neu(epoche: EpochId) -> Self {
        Self { nonce: epoche.0.saturating_mul(1 << 32), ..Self::empty() }
    }

    /// ⚑ **Leer**: kein Guthaben, kein Einsatz, nichts gekündigt, keine
    /// Credits, keine Verstösse (die Verstossgeschichte ist das Gedächtnis
    /// der Staffel beim Schlachten und bleibt).
    pub fn ist_leer(&self) -> bool {
        self.balance == 0
            && self.staked == 0
            && self.gekuendigt.is_empty()
            && self.credits.is_empty()
            && self.verstoesse.is_empty()
    }
}

/// Der vollständige Ledger-Zustand.
///
/// `accounts` ist eine `BTreeMap` — die deterministische Ordnung ist
/// Konsens-Eigenschaft (siehe Modul-Dokumentation in `lib.rs`).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct LedgerState {
    /// Aktuelle Epoche.
    pub epoch: EpochId,
    /// Credit-Preis: MYL-Kleinstbeträge je vTFE-Einheit, mindestens 1;
    /// der gerundete Wert von [`LedgerState::credit_preis_fein`], am
    /// Epochenwechsel nach der Auslastung fortgeschrieben (Startwert zu
    /// Genesis).
    pub credit_price: u64,
    /// Konten, deterministisch nach Adresse geordnet.
    pub accounts: crate::sammlung::Sammlung<Address, AccountState>,
    /// Offene Agenten-Sessions, deterministisch nach Kontraktadresse
    /// geordnet (Whitepaper Kap. 8.2).
    pub sitzungen: crate::sammlung::Sammlung<SitzungId, Sitzung>,
    /// Gegenstände, deren Manifest **unmittelbar** im Zustand steht,
    /// geordnet nach ihrer Wurzel.
    ///
    /// # ⚑ Warum nur ein Teil der Gegenstände hier steht (D7, 2026-08-31)
    ///
    /// [`Self::commitment`] serialisiert den **ganzen** Zustand und
    /// hasht ihn; es gibt keinen Baum mit Teilbeweisen. Jede
    /// Zustandsänderung kostet damit O(Zustandsgröße), und zwar je
    /// Block. **Eine unbegrenzt wachsende Menge darf deshalb nicht
    /// einzeln hier stehen**, sonst serialisiert jeder Block die ganze
    /// Wissensdatenbank.
    ///
    /// Hier steht die **Infrastruktur**: Shardgewichte, Skalenpakete,
    /// Tabellen. Sie wächst nur durch Governance-Akte und ist damit
    /// begrenzt, und ein beitretender Miner muss sie finden können,
    /// **bevor** er irgendetwas beweisen kann.
    ///
    /// Die Wissensdatenbank läuft über eine Wurzel (κ_v). Der Weg
    /// dorthin ist nicht gebaut; [`crate::transitions::speicher_aufnehmen`]
    /// weist die Wissensklassen deshalb mit benanntem Grund ab, statt
    /// sie stillschweigend aufzunehmen.
    ///
    /// **Angehängt und nicht eingefügt.** Ein neues Feld ändert den
    /// Zustandshash ohnehin; die Reihenfolge bleibt trotzdem stabil,
    /// damit ein Leser zweier Fassungen sieht, was dazukam.
    pub speicher: crate::sammlung::Sammlung<MerkleRoot, Manifest>,
    /// Was in der **laufenden** Epoche verbrannt wurde, `B_e`.
    ///
    /// ⚑ **Ohne diese Zahl gibt es keine Prägung.** Kap. 5.2 leitet
    /// `m_e` aus dem geglätteten Burn ab, der geglättete aus dem
    /// Burn je Epoche. `burn_to_credits` zerstörte die Münzen bis zum
    /// 2026-08-31 und vergaß sofort, wie viele es waren; die
    /// Prägungsformel hatte damit keine Eingabe im Zustand.
    pub burn_epoche: u64,
    /// Der geglättete Burn `B̄_e` über die bisherigen Epochen.
    pub burn_ema: u64,
    /// Die in der **laufenden** Epoche nachgefragte Rechenleistung in
    /// vTFE.
    ///
    /// ⚑ **Ohne diese Zahl gibt es keine Auslastung** (Fund 184,
    /// 2026-09-05). `myl_tokenomics::utilization::calculate_utilization`
    /// stand seit Monaten da und hatte ausserhalb der eigenen Tests
    /// keinen Aufrufer, weil ihre Eingangsgrösse im Zustand nicht
    /// existierte. Kap. 7.1 macht die Trainingsmenge von der Auslastung
    /// abhängig; die Regel liess sich nicht auswerten.
    ///
    /// Gezählt wird in [`crate::transitions::credit_spend`], dem einen
    /// Engpass, durch den jede bezahlte Anfrage geht.
    pub vtfe_epoche: u64,
    /// Was in der **abgeschlossenen** Vorepoche nachgefragt wurde.
    ///
    /// # ⚑ Warum die Vorepoche und nicht die laufende
    ///
    /// Die Pod-Zuteilung einer Epoche steht **fest, bevor die Epoche
    /// läuft**. Wer sie aus der Nachfrage derselben Epoche ableitete,
    /// bräuchte eine Zahl, die es noch nicht gibt.
    ///
    /// ⚑ **Dasselbe Kausalitätsargument wie beim Lastausgleich des
    /// Routers**, der die vorige Charge benutzt und nicht die laufende.
    /// Es ist kein Kompromiss, sondern die einzige Reihenfolge, in der
    /// die Frage überhaupt beantwortbar ist.
    pub vtfe_vorepoche: u64,
    /// Die Trainingssegmente der laufenden Epoche, je Pod eines.
    ///
    /// ⚑ **Wie die Bündel: begrenzt und je Epoche geleert.** Eine
    /// wachsende Menge im Zustand bräche Entscheidung D7, weil
    /// `commitment` den ganzen Zustand serialisiert. Die Historie steht
    /// in den Blöcken.
    pub trainingssegmente: crate::sammlung::Sammlung<PodId, myl_types::trainingssegment::Trainingssegment>,

    /// Die Fassung des Modells, an der gerechnet wird.
    ///
    /// ⚑ **Sie steigt nur durch bestätigtes Training** (Kap. 7.2), also
    /// dann, wenn beide Pods eines Redundanzpaars zum selben Δ gekommen
    /// sind. Ein Ergebnis, das nur einer gerechnet hat, bewegt das
    /// Modell nicht: **Es liesse sich mit nichts vergleichen.**
    ///
    /// ⚑ **Die Kette rechnet die Summe nicht**, sie hält das Rezept:
    /// alte Fassung plus geordnete Liste der bestätigten Δ-Commitments,
    /// siehe [`myl_types::modellversion`]. Wer die Gewichte hat, wendet
    /// sie an und rechnet die Wurzel nach.
    pub modell_version: MerkleRoot,

    /// Der verankerte Trainingskorpus, falls das Netz einen hat.
    ///
    /// ⚑ **Nur im Genesis zu setzen**, siehe
    /// [`myl_types::korpusanker::Korpusanker`]: Eine Anweisung, die
    /// jeder einreichen kann, uebergaebe die schwerste Entscheidung
    /// dieses Systems an jeden, und einen Governance-Weg in den Zustand
    /// gibt es noch nicht.
    ///
    /// `None` heisst: Dieses Netz trainiert nicht, auch wenn es
    /// Trainingspods bildet. Der Plan weist dann kein Buendel zu, und
    /// **das steht im Plan** statt in einem Protokoll.
    pub korpus: Option<myl_types::korpusanker::Korpusanker>,
    /// Wann jeder Miner zuletzt zum Training herangezogen wurde.
    ///
    /// # ⚑ Am Miner und nicht am Pod
    ///
    /// „Ein Pod, der zuletzt trainiert hat, kommt nicht sofort wieder
    /// dran" ist die Absicht, aber **Pods werden jede Epoche neu
    /// gebildet**: Pod 5 der Epoche 100 und Pod 5 der Epoche 101 sind
    /// verschiedene Leute. Was über Epochen hinweg besteht, ist der
    /// Miner.
    ///
    /// ⚑ **Vermerkt wird die Zuweisung, nicht die erbrachte Leistung.**
    /// Ein Pod, der zugewiesen war und scheiterte, soll nicht sofort
    /// wieder vorn stehen; sonst wäre Scheitern ein Weg, immer wieder
    /// gewählt zu werden, und bei gedeckelter Trainingsvergütung will
    /// das niemand. Ob er geliefert hat, entscheidet die Vergütung.
    ///
    /// ⚑ **Er wächst nicht unbegrenzt.** Die Menge ist durch das
    /// Minerregister beschränkt, das ohnehin im Zustand steht; ein
    /// abgemeldeter Miner verschwindet mit dem nächsten Aufräumen.
    pub trainingsstand: crate::sammlung::Sammlung<MinerId, u64>,
    /// Bis einschließlich welcher Epoche der geglättete Burn fortgeschrieben ist.
    ///
    /// ⚑ **Gegen die doppelte Fortschreibung.** Die Glättung darf je
    /// Epoche genau einmal laufen; zweimal gerufen, verschiebt sie den
    /// Durchschnitt in Richtung der letzten Beobachtung, und niemand
    /// sähe es der Zahl an.
    pub burn_ema_bis: EpochId,
    /// Wohin ein Miner bezahlt wird.
    ///
    /// # ⚑ Warum die Auszahlung nicht an der Miner-Kennung hängt
    ///
    /// Die Kennung ist `SHA-256` über den **Konsensschlüssel**, und der
    /// liegt heiß: Er unterschreibt jeden Vote, jeden Commit, jeden
    /// Übergang, jede Kapazitätszusage und jede Speicherquittung. Ihn
    /// zugleich zum Konto zu machen, auf dem sich der Ertrag sammelt,
    /// ist ein bekannter Fehler: Andere Netze haben ihn gemacht und
    /// später mit einer ökosystemweiten Migration korrigiert, andere
    /// trennen von Anfang an. **Der Schnitt heisst überall dasselbe:**
    /// ein Schlüssel, der arbeitet, und ein Konto, das hält.
    ///
    /// # ⚑ Und die Änderung gehört dem kalten Konto
    ///
    /// Die **erste** Eintragung unterschreibt der Miner selbst, er hat
    /// nichts zu verlieren. **Jede weitere unterschreibt das
    /// eingetragene Konto.** Damit kann ein gestohlener heißer Schlüssel
    /// den Ertrag nicht umleiten, und es braucht dafür keine Wartefrist,
    /// über die jemand streiten könnte.
    ///
    /// # Ohne Eintrag kein Anteil
    ///
    /// Festlegung des Projektinhabers, 2026-08-31: Wer nichts eingetragen
    /// hat, wird bei der Verteilung übergangen, sein Gewicht zählt nicht.
    /// **So sammelt sich nie ein Ertrag unter einem heißen Schlüssel an**,
    /// und der Fehler fällt sofort auf, weil nichts ankommt.
    pub auszahlung: crate::sammlung::Sammlung<MinerId, Address>,

    /// Wer sich als Miner angemeldet hat (Punkt 40, Glied 3a).
    ///
    /// # ⚑ Der Doc-Kommentar sagte es seit Monaten, und es stimmte nicht
    ///
    /// `MinerRegistration` trug schon immer den Satz „wird bei der
    /// Miner-Registrierung erstellt und **im Ledger gespeichert**".
    /// **Der Ledger kannte sie nicht.** Der Scheduler bekam seine Liste
    /// vom Aufrufer, und wer sie liefert, entscheidet über die
    /// Pod-Bildung: Zwei Knoten mit verschiedenen Listen kommen zu
    /// verschiedenen Pods.
    ///
    /// # Warum das in den Zustand darf und die Wissensdatenbank nicht
    ///
    /// Entscheidung D7 hält unbegrenzt wachsende Mengen aus dem
    /// Zustand heraus, weil [`LedgerState::commitment`] den **ganzen**
    /// Zustand je Block serialisiert. **Das Register wächst nicht
    /// unbegrenzt**, sondern mit der Zahl der Miner, und die ist die
    /// Größe des Netzes selbst: Ein Netz, dessen Teilnehmerliste den
    /// Zustand sprengt, hat ein anderes Problem.
    ///
    /// ⚑ **Latenz-Atteste gehören aus demselben Grund nicht hierher.**
    /// Sie wären bei tausend Minern gut vier Megabyte je Epoche; sie
    /// sind Blockdaten, und in den Zustand kommt nur, was aus ihnen
    /// folgt.
    pub miner: crate::sammlung::Sammlung<MinerId, MinerRegistration>,

    /// ⚑ **Die Kapazitätszusagen der Miner** (Stufe 4, 2026-10-21), je
    /// Miner nach `ab_epoche` aufsteigend: höchstens die geltende und die
    /// kommenden. **Beide, nicht nur die neueste**: Die Zuteilung der
    /// laufenden Epoche wird jederzeit aus dem Zustand abgeleitet, und eine
    /// Zusage für die nächste darf sie nicht mitten in der Epoche ändern.
    pub zusagen: crate::sammlung::Sammlung<MinerId, Vec<myl_types::zusage::Kapazitaetszusage>>,

    /// ⚑ **Das Podmodell des Netzes** (Stufe 4, 2026-10-22): welches
    /// Artefakt die Pods rechnen, mit Ebenenzahl und Kopf. Gesetzt beim
    /// Beginn der Kette, wie die erste Saat; der Ledger kennt sein Netz
    /// nicht. **`None` heißt: keine Modellzusage gilt**, also auch kein Pod.
    pub podmodell: Option<myl_types::podmodell::Podmodell>,

    /// ⚑ **Fristsperren** (2026-10-22): In der genannten Epoche gilt die
    /// Zusage des Miners nicht, weil er in der Epoche davor seine Frist
    /// überschritten hat (`fristverstoss_einreichen`). Höchstens ein
    /// Eintrag je Miner; vergangene fallen beim nächsten Vermerk weg.
    pub fristsperren: crate::sammlung::Sammlung<MinerId, EpochId>,

    /// Die PoI-Bündel der **laufenden** Epoche, je Pod eines
    /// (Punkt 40, Glied 1).
    ///
    /// # ⚑ Warum das den Zustand nicht sprengt
    ///
    /// Ein Bündel je Pod je Epoche, und **beim Epochenabschluss wird
    /// geleert**. Bei tausend Minern und Pods aus `k+2` Mitgliedern sind
    /// das rund 170 Einträge zu je gut 200 Byte. Entscheidung D7 hält
    /// unbegrenzt wachsende Mengen aus dem Zustand heraus, weil
    /// [`LedgerState::commitment`] ihn je Block ganz serialisiert;
    /// **begrenzt ist diese Menge, weil sie geleert wird**, nicht weil
    /// sie klein anfängt.
    ///
    /// Wer hier eine Aufbewahrung über Epochen hinweg einbaut, macht aus
    /// einer begrenzten Menge eine wachsende und bricht D7. Die Historie
    /// steht in den Blöcken.
    pub buendel: crate::sammlung::Sammlung<PodId, PoIBundle>,

    /// Wie sich die Arbeit eines Pods auf seine Positionen verteilt
    /// (Punkt 40, letztes Glied).
    ///
    /// # ⚑ Warum Gewichte und nicht das Modellprofil
    ///
    /// Die Zuschreibung je Miner folgt aus den
    /// Multiplikations-Additionen seines Zuschnitts. Dafür bräuchte der
    /// Zustand das **Modellprofil** und den **Zuschnitt je Position**;
    /// beides wäre möglich und die falsche Wahl. **Ein Profil im Zustand
    /// ist genauso eine Erklärung wie ein Gewicht, nur mit zehnfacher
    /// Fläche**, und es zöge die Modellinnereien in einen Konsenstyp:
    /// Eine neue Architektur änderte die **Form des Zustands** und
    /// verlangte eine harte Gabelung. Mit Gewichten ändert sie die
    /// Zahlen.
    ///
    /// **`None` heißt: es wird nichts zugeschrieben.** Ohne Verteilung
    /// bleibt der Shard-Miner-Anteil ungeprägt, und das ist die sichere
    /// Richtung.

    /// Die Saat, aus der die Pod-Zuteilung **dieser** Epoche folgt.
    ///
    /// # ⚑ Fund 143: Die Saat kam vom Ende derselben Epoche
    ///
    /// Bis zum 2026-09-02 reichte der Knoten `self.letzter_hash` durch,
    /// und beim Epochenabschluss ist das der letzte Block der Epoche,
    /// die gerade abgerechnet wird. **Damit stand die Zuteilung erst
    /// fest, wenn die Epoche vorbei war**, während ein Bündel
    /// **während** ihr eingereicht sein muss. Kein Pod konnte während
    /// seiner Epoche wissen, dass er einer ist.
    ///
    /// **Jetzt steht sie im Zustand und gilt die ganze Epoche.** Sie
    /// ist der letzte Blockhash der Epoche `e−2`; siehe
    /// [`Self::epochensaat_naechste`] für den Umlauf.
    ///
    /// ⚑ **Zwei Epochen Vorlauf, und die Zahl ist nicht gegriffen.**
    /// Der Registrierungsschluss in Anhang A.2 ist `e−2`: Wer sich
    /// später anmeldet, zählt nicht mit. Käme die Saat aus `e−1`, wäre
    /// der Schluss weicher als die Saat, und ein Miner könnte sich
    /// anmelden, nachdem er sie kennt. Zwei Epochen Vorlauf sind
    /// dabei kein eigener Einfall: Die Saat für Epoche `N` am Ende von
    /// `N−2` ist Stand der Technik.
    pub epochensaat: Hash,

    /// Die Saat, die zur **nächsten** Epoche in Kraft tritt.
    ///
    /// Der Umlauf am Epochenwechsel mit dem letzten Blockhash `h`:
    ///
    /// ```text
    /// epochensaat          <- epochensaat_naechste
    /// epochensaat_naechste <- h
    /// ```
    ///
    /// ⚑ **Der Umlauf steht hinter dem Abschluss, nicht davor.** Die
    /// abzurechnende Epoche muss mit der Saat abgerechnet werden, die
    /// **während** ihr galt; drehte man zuerst, rechnete der Abschluss
    /// gegen eine Zuteilung, die es in dieser Epoche nie gab.
    pub epochensaat_naechste: Hash,

    /// ⚑ **Die Historienwurzel** (Archivknoten, Stufe A2, 2026-10-23): ein
    /// Merkle Mountain Range über die Hashes aller Blöcke dieser Kette, Blatt
    /// `i` ist der Hash von Block `i` (Blatt 0 der Startwert). Fortgeschrieben
    /// beim Anwenden jedes Blocks mit dem Hash seines **Vorgängers**: Den
    /// eigenen kann der Zustand nicht tragen, denn der Blockhash deckt die
    /// Zustandswurzel, in der die Historie steht.
    ///
    /// Damit lässt sich jeder alte Block mit einem kurzen Beweis gegen den
    /// heutigen Zustand prüfen, ohne die Kette ab Genesis nachzurechnen; das
    /// tragen das Nachladen der Archivknoten und ihre Stichproben.
    pub historie: myl_types::historie::Historie,

    /// ⚑ **Die Archivpflicht** (Archivknoten, Stufen A5 und A6,
    /// 2026-10-23): Kopien und Splitter der laufenden Periode, Wurzel der
    /// Grenze, Belege, verfehlte Epochen, Halter bis zur Übergabe. Siehe
    /// [`crate::archiv`].
    pub archiv: crate::archiv::Archivstand,

    /// ⚑ **Bis zu welchem Prüfpunkt die Kette bestätigt ist** (2026-10-27,
    /// Festlegung des Projektinhabers: Prüfpunkte alle 150 Blöcke): die Höhe
    /// des jüngsten Prüfpunkts, dessen Zertifikat ein Block getragen hat,
    /// null vor dem ersten. **Im Zustand und nicht in der Kette**, damit
    /// jeder Knoten, auch einer, der aus einem Abzug begann, dasselbe
    /// nächste Zertifikat annimmt.
    pub pruefpunkt_bestaetigt: u64,

    /// ⚑ **Ziehungen, die auf ihr Zertifikat warten** (2026-10-27): je
    /// abgelaufene Epoche die Anteile ihrer bezeugten Bündel. Gezogen wird,
    /// sobald ein Block das Zertifikat eines Prüfpunkts an oder nach dem
    /// Wechsel trägt; aus ihm kommt die Saat, die beim Einreichen der Bündel
    /// niemand kennen konnte. **Begrenzt**: ein Eintrag je Epoche, und er
    /// fällt mit der Ziehung weg; nur eine Abstimmung, die über Epochen
    /// hängt, lässt die Liste wachsen.
    pub stichproben_offen: Vec<OffeneStichprobe>,

    /// ⚑ **Gebuchte Doppelbelege** (2026-10-27, CONSENSUS 7.6): je
    /// (Schuldiger, Art, Höhe) die Epoche der Buchung, damit derselbe
    /// Widerspruch nur einmal schlachtet. **Begrenzt**: nach der Sperrfrist
    /// fällt ein Eintrag weg, und ein älterer Widerspruch ist ohnehin nicht
    /// mehr schlachtbar.
    pub doppelbelege: crate::sammlung::Sammlung<(Address, u8, u64), u64>,

    /// ⚑ **Der Grundpreis je Byte** (2026-10-27, Festlegung des
    /// Projektinhabers, G1): wandert je Block um höchstens ein Achtel um das
    /// Ziel und fällt nie unter den Boden des Netzes. Die Kette setzt ihn.
    pub grundpreis_je_byte: u64,

    /// Die Gebühren der laufenden Epoche; beim Wechsel geteilt.
    pub gebuehren_epoche: u64,

    /// ⚑ **Der Anteil der Validatoren an den Gebühren**, zehn Dreizehntel,
    /// gesammelt, bis es eine Gewichtsquelle für Validatoren gibt. Die
    /// Prägung zahlt ihren Validatorenanteil aus demselben Grund nicht aus
    /// (`Auslassungsgrund::KeineGewichtsquelle`); erfunden wird hier keine.
    pub gebuehren_validatoren: u64,

    /// ⚑ **Unterschriften je Validator in den Prüfpunktzertifikaten der
    /// laufenden Epoche** (2026-10-28, Festlegung des Projektinhabers): Am
    /// Epochenwechsel geht der Topf der Validatoren nach Stake mal dieser
    /// Zahl an die Mitglieder des Stimmsatzes; danach leer.
    pub pruefpunkt_stimmen: crate::sammlung::Sammlung<MinerId, u64>,

    /// ⚑ **Hinterlegungen für neuen Zustand** (G1): je (Art, Kennung) wer
    /// hinterlegt hat und wie viel. Zurück beim Abmelden, beim Aufräumen der
    /// Sitzung, beim Löschen des Manifests. Art 0 Anmeldung (Kennung des
    /// Miners), 1 Sitzung, 2 Speichermanifest.
    pub hinterlegungen: crate::sammlung::Sammlung<(u8, [u8; 32]), (Address, u64)>,

    /// ⚑ **Der Credit-Preis in Feinauflösung** (Whitepaper 5.4, Festlegung
    /// des Projektinhabers vom 2026-10-28): Festkomma mit 32
    /// Nachkommabits, Kleinstbeträge je vTFE-Einheit. Die Kette schreibt
    /// ihn je Epoche fort; [`LedgerState::credit_price`] ist sein gerundeter
    /// Wert. 📌 Ohne die Nachkommastellen bliebe ein kleiner Preis stehen:
    /// Ein Schritt von 3,4 % rundet unter rund 15 Kleinstbeträgen auf null.
    pub credit_preis_fein: i64,

    /// ⚑ **Stufe 1, gezählt** (2026-10-28): wie viele Zwillingspaare seit
    /// Genesis an einem Epochenwechsel einig waren (beide vergütet) und wie
    /// viele uneinig oder unvollständig (keiner vergütet).
    pub zwillinge_einig: u64,
    pub zwillinge_uneinig: u64,

    /// ⚑ **Die Wurzeln der zuletzt gezogenen Stichprobe** (Stufe 2,
    /// 2026-10-28): Pod, `segments_root` und Segmentzahl je Bündel der
    /// Epoche, aus der zuletzt gezogen wurde. Gegen sie prüft ein Knoten eine
    /// gelieferte Spur. Angehängt.
    pub stichprobe_wurzeln: Vec<(PodId, MerkleRoot, u32)>,

    /// ⚑ **Zurückgehaltene Gutschriften** (Stufe 1, Anlaufregel,
    /// 2026-10-28): je Miner die vTFE eines Pods, der allein rechnete (ohne
    /// Zwilling), mit der Epoche, ab der sie ausgezahlt werden (nach der
    /// Streitfrist). Angehängt.
    pub zurueckgehalten: Vec<(u64, MinerId, u64)>,

    /// ⚑ **Die offenen Prüfaufträge** (Stufe 2, 2026-10-29): je gezogenem
    /// Segment seine Checker, Festlegungen und Aufdeckungen
    /// ([`crate::pruefung`]). Angehängt.
    pub pruefauftraege: Vec<crate::pruefung::Pruefauftrag>,

    /// ⚑ **Das geprüfte Volumen der laufenden Epoche** je Checker
    /// (Anhang B.3: „proportional zu geprüftem Volumen“): nachgerechnete
    /// Positionen aus einig abgeschlossenen Aufträgen. Der Epochenwechsel
    /// zahlt daraus den Checker-Pool aus und leert es. Angehängt.
    pub pruefvolumen: crate::sammlung::Sammlung<MinerId, u64>,

    /// ⚑ **Die offenen Streitfälle** (T6d, 2026-10-29): je aufgedeckter
    /// Abweichung bis zur Entscheidung des Komitees oder zum Verfall
    /// ([`crate::pruefung::Streitfall`]). Angehängt.
    pub streitfaelle: Vec<crate::pruefung::Streitfall>,

    /// ⚑ **Die uneinigen Zwillingsgruppen** (2026-10-29), bis ihre Frist
    /// für einen Differenzbeweis abläuft ([`crate::pruefung::Uneinigkeit`]).
    /// Angehängt.
    pub uneinigkeiten: Vec<crate::pruefung::Uneinigkeit>,

    /// ⚑ **Die Vorbehalte uneiniger Gruppen** (Whitepaper 6.4, Rückbuchung,
    /// 2026-10-29; [`crate::pruefung::Vorbehalt`]) und wie viele seit
    /// Genesis ausgezahlt wurden. Angehängt.
    pub vorbehalte: Vec<crate::pruefung::Vorbehalt>,
    pub rueckbuchungen: u64,

    /// ⚑ **Ablauffächer der Sitzungen** (Skalierung, 2026-10-29): je
    /// Epoche, in der aufgeräumt wird, die Sitzungen, die dann ablaufen.
    /// Das Aufräumen am Epochenwechsel liest nur die fälligen Fächer, statt
    /// über alle Sitzungen und alle Hinterlegungen zu laufen; bei einer
    /// Milliarde Teilnehmern wäre das je Epoche ein Durchlauf über den
    /// ganzen Bestand, und ein Zeuge könnte ihn nicht belegen.
    pub sitzungsablauf: crate::sammlung::Sammlung<u64, Vec<SitzungId>>,
    /// Bis zu welcher Epoche die Fächer abgearbeitet sind.
    pub sitzungsablauf_bis: u64,

    /// ⚑ **Vorgemerkte Auszahlungen** (verteilte Ausschüttung, 2026-10-30,
    /// nach dem Vorbild von Solanas „Partitioned Epoch Rewards“, SIMD-0118):
    /// je Konto, was ihm aus Ausschüttungen zusteht und noch nicht gebucht
    /// ist. Die Ausschüttung am Epochenwechsel merkt nur vor; jeder Block
    /// bucht höchstens `AUSZAHLUNGEN_JE_BLOCK` davon, nach Adresse. ⚑ **Im
    /// kleinen Baum**: Der Plan wächst mit den Minern wie ihr Register, und
    /// so berührt der Wechselblock keinen Zeugen; bei sehr vielen Minern
    /// wäre er sonst für zustandslose Knoten zu gross geworden.
    pub auszahlungen_offen: crate::sammlung::Sammlung<Address, u64>,

    /// ⚑ **Der Zustandsbaum der Sammlungen** (Skalierung, 2026-10-29), nicht
    /// kodiert: aus den Sammlungen gebaut und je Block mit dem Geänderten
    /// fortgeschrieben ([`LedgerState::wurzel`]).
    #[borsh(skip)]
    pub baum: Baumcache,
}

/// Die Zustandsbäume samt der Angabe, ob sie gebaut sind. Gleich sind zwei
/// immer: Sie sind aus den Sammlungen abgeleitet.
///
/// ⚑ **Zwei Bäume** (zustandslose Validierung, 2026-10-29): Der **kleine**
/// trägt die Register, die mit den Minern wachsen, und jeder Knoten hält ihn
/// ganz. Der **grosse** trägt, was mit den Teilnehmern wächst
/// ([`GROSSE_SAMMLUNGEN`]); ein prüfender Knoten hält von ihm nur die Wurzel
/// und bekommt je Block einen Zeugen.
#[derive(Debug, Default)]
pub struct Baumcache {
    klein: myl_types::zustandsbaum::Zustandsbaum,
    gross: Grossbaum,
    gebaut: bool,
    /// Beim Prüfer: Eine Änderung lag ausserhalb des Zeugen.
    verletzt: bool,
    /// Beim Prüfer: die Wurzel des grossen Baums vor dem Block.
    vorwurzel: [u8; 32],
    /// ⚑ Wohin der grosse Baum ausgelagert wird (2026-10-30), wenn er es
    /// wird; ein Neubau lagert wieder dorthin aus.
    ablage: Option<std::path::PathBuf>,
}

/// Der grosse Baum, so weit er gehalten wird.
#[derive(Debug, Clone)]
enum Grossbaum {
    /// Ganz (ein voller Knoten).
    Voll(myl_types::zustandsbaum::Zustandsbaum),
    /// Der Zeuge des laufenden Blocks (ein prüfender Knoten).
    Zeuge(myl_types::zustandsbaum::Teilbaum),
    /// Nur die Wurzel (ein prüfender Knoten zwischen den Blöcken).
    Wurzel([u8; 32]),
}

impl Default for Grossbaum {
    fn default() -> Self {
        Grossbaum::Voll(Default::default())
    }
}

/// ⚑ **Die Sammlungen, die mit den Teilnehmern wachsen** (Nummern im
/// Zustandsbaum): Konten, Sitzungen, Speicher, Hinterlegungen,
/// Ablauffächer. Die übrigen wachsen mit den Minern.
pub const GROSSE_SAMMLUNGEN: [u8; 5] = [0, 1, 2, 12, 14];

/// Die beiden Bäume als Ziel des Einspeisens, nach der Nummer getrennt.
struct Baeume<'a> {
    klein: &'a mut myl_types::zustandsbaum::Zustandsbaum,
    gross: &'a mut Grossbaum,
    verletzt: &'a mut bool,
}

impl Baeume<'_> {
    fn setzen(&mut self, nummer: u8, schluessel: [u8; 32], wert: Option<[u8; 32]>) {
        if !GROSSE_SAMMLUNGEN.contains(&nummer) {
            self.klein.setzen(schluessel, wert);
            return;
        }
        match self.gross {
            Grossbaum::Voll(b) => b.setzen(schluessel, wert),
            Grossbaum::Zeuge(t) => {
                if t.setzen(schluessel, wert).is_err() {
                    *self.verletzt = true;
                }
            }
            Grossbaum::Wurzel(_) => *self.verletzt = true,
        }
    }
}

/// ⚑ **Der Zeuge eines Blocks** (zustandslose Validierung, 2026-10-29): je
/// grosser Sammlung die Einträge, die der Block berührt, mit ihrem Wert
/// davor (`None`: es gab ihn nicht), und der Teilbaum des grossen Baums
/// über ihre Pfade. Wer nur die Wurzel des grossen Baums hält, rechnet
/// damit den Block nach. Er geht nicht in den Blockhash ein: Er belegt sich
/// selbst über die Wurzel vor dem Block.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Zeuge {
    pub baum: Vec<u8>,
    pub accounts: Vec<(Address, Option<AccountState>)>,
    pub sitzungen: Vec<(SitzungId, Option<Sitzung>)>,
    pub speicher: Vec<(MerkleRoot, Option<Manifest>)>,
    pub hinterlegungen: Vec<(Hinterlegungsschluessel, Option<(Address, u64)>)>,
    pub sitzungsablauf: Vec<(u64, Option<Vec<SitzungId>>)>,
}

/// Ein Konto (oder sein Fehlen) mit dem Beweis gegen den grossen Baum.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kontobeweis {
    pub adresse: Address,
    pub konto: Option<AccountState>,
    pub beweis: myl_types::zustandsbaum::Baumbeweis,
}

/// Der Schlüssel einer Hinterlegung: (Art, Kennung).
pub type Hinterlegungsschluessel = (u8, [u8; 32]);

/// Warum ein Zeuge nicht trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zeugenfehler {
    /// Die Bytes sind kein Teilbaum.
    Unlesbar,
    /// Seine Wurzel ist nicht die vor dem Block.
    FremdeWurzel,
    /// Ein Eintrag passt nicht zum Teilbaum.
    EintragFalsch,
    /// Der Block griff auf etwas zu, das der Zeuge nicht enthält (oder auf
    /// eine ganze grosse Sammlung).
    Unvollstaendig,
    /// Der Zustand hält den grossen Baum nicht ganz; nur ein voller Knoten
    /// zeichnet auf.
    NichtVoll,
}

/// ⛔️ **Eine Kopie vergisst die Ablage** (2026-10-30): Sie liegt ganz im
/// Speicher (siehe `Zustandsbaum::clone`); öffnete ein Neubau der Kopie
/// dieselbe Datei, leerte er die Seiten des Originals.
impl Clone for Baumcache {
    fn clone(&self) -> Self {
        Self {
            klein: self.klein.clone(),
            gross: self.gross.clone(),
            gebaut: self.gebaut,
            verletzt: self.verletzt,
            vorwurzel: self.vorwurzel,
            ablage: None,
        }
    }
}

impl PartialEq for Baumcache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Baumcache {}

/// Was beim Anwenden eines fremden Blocks gesichert wird, statt den ganzen
/// Zustand zu kopieren: der Rest ausser den Sammlungen und dem Baum; die
/// Sammlungen führen ein Journal.
pub struct Sicherung(Box<LedgerState>);

/// Eine Ziehung, die auf das Zertifikat ihres Epochenwechsels wartet.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct OffeneStichprobe {
    /// Die abgelaufene Epoche.
    pub epoche: u64,
    /// Je bezeugtem Bündel der Pod und seine Zahl der Segmente.
    pub anteile: Vec<(PodId, u32)>,
    /// ⚑ Je bezeugtem Bündel Pod, `segments_root` und Segmentzahl
    /// (2026-10-28): damit prüft, wer eine Spur bekommt, ob sie zum Bündel
    /// gehört. Angehängt.
    pub wurzeln: Vec<(PodId, MerkleRoot, u32)>,
    /// ⚑ Die Pods, die allein rechneten (Anlaufregel, 2026-10-28): Jedes
    /// ihrer Segmente wird gezogen. Angehängt.
    pub voll: Vec<PodId>,
    /// ⚑ Wer die Ziehung prüfen darf (2026-10-29): die Miner ohne Pod und
    /// die des Pods in Bereitschaft der abgelaufenen Epoche. Angehängt.
    pub pruefkandidaten: Vec<MinerId>,
    /// ⚑ Je bezeugtem Pod die Mitglieder der übrigen Zwillingsgruppen, als
    /// Ersatz, wenn es keine Kandidaten gibt (2026-10-29). Angehängt.
    pub ersatz: Vec<(PodId, Vec<MinerId>)>,
    /// ⚑ Je bezeugtem Pod die Besetzung der abgelaufenen Epoche: je
    /// Position Miner und Layerbereich `[von, bis)` (2026-10-29). Daraus
    /// folgt im Streitfall, wen eine Abweichung trifft. Angehängt.
    pub besetzung: Vec<(PodId, crate::pruefung::Besetzung)>,
}

impl LedgerState {
    /// Genesis-Leerzustand mit gegebenem Credit-Preis.
    pub fn genesis(credit_price: u64) -> Self {
        Self {
            epoch: EpochId(0),
            credit_price,
            accounts: Default::default(),
            sitzungen: Default::default(),
            speicher: Default::default(),
            burn_epoche: 0,
            vtfe_epoche: 0,
            vtfe_vorepoche: 0,
            trainingsstand: Default::default(),
            korpus: None,
            modell_version: MerkleRoot::new([0u8; 32]),
            trainingssegmente: Default::default(),
            burn_ema: 0,
            burn_ema_bis: EpochId(0),
            auszahlung: Default::default(),
            miner: Default::default(),
            zusagen: Default::default(),
            podmodell: None,
            fristsperren: Default::default(),
            buendel: Default::default(),
            // ⚑ **Null, und die Kette setzt sie.** Vor dem ersten
            // Block gibt es keinen Blockhash, und der Ledger kennt den
            // Startwert seiner Kette nicht: Er ist eine Eigenschaft des
            // Netzes, nicht des Zustands. `Kette::probestand` trägt ihn
            // ein, damit die Saat der ersten beiden Epochen an ihre
            // Kette gebunden ist statt an eine Null, die überall
            // dieselbe wäre.
            epochensaat: Hash::from_bytes([0u8; 32]),
            epochensaat_naechste: Hash::from_bytes([0u8; 32]),
            historie: myl_types::historie::Historie::default(),
            archiv: crate::archiv::Archivstand::default(),
            pruefpunkt_bestaetigt: 0,
            stichproben_offen: Vec::new(),
            doppelbelege: Default::default(),
            grundpreis_je_byte: 0,
            gebuehren_epoche: 0,
            gebuehren_validatoren: 0,
            pruefpunkt_stimmen: Default::default(),
            hinterlegungen: Default::default(),
            credit_preis_fein: i64::try_from(credit_price).ok().and_then(|p| p.checked_mul(1 << 32)).unwrap_or(i64::MAX),
            zwillinge_einig: 0,
            zwillinge_uneinig: 0,
            stichprobe_wurzeln: Vec::new(),
            zurueckgehalten: Vec::new(),
            pruefauftraege: Vec::new(),
            pruefvolumen: Default::default(),
            streitfaelle: Vec::new(),
            uneinigkeiten: Vec::new(),
            vorbehalte: Vec::new(),
            rueckbuchungen: 0,
            sitzungsablauf: Default::default(),
            sitzungsablauf_bis: 0,
            auszahlungen_offen: Default::default(),
            baum: Baumcache::default(),
        }
    }

    /// Konto lesen oder das leere Konto liefern (liest nie zustands-
    /// verändernd; `account_mut` für Übergänge).
    pub fn account(&self, addr: &Address) -> AccountState {
        self.accounts.get(addr).cloned().unwrap_or_else(|| AccountState::neu(self.epoch))
    }

    /// Eine Session lesen, ohne sie anzulegen.
    pub fn sitzung(&self, adresse: &SitzungId) -> Option<&Sitzung> {
        self.sitzungen.get(adresse)
    }

    /// Konto zur Veränderung lesen bzw. anlegen.
    pub fn account_mut(&mut self, addr: &Address) -> &mut AccountState {
        let epoche = self.epoch;
        self.accounts.entry(*addr).or_insert_with(|| AccountState::neu(epoche))
    }

    /// ⚑ **Leere Konten fallen weg** (Skalierung, 2026-10-29): je Block
    /// geprüft, nur unter den in ihm geänderten, also O(geändert). Ein
    /// Staubkonto bläht den Zustand so nicht auf; der Nonce-Boden
    /// ([`AccountState::neu`]) schützt vor Wiederholung, wenn an derselben
    /// Adresse wieder eines entsteht.
    pub fn leere_konten_entfernen(&mut self) -> usize {
        let leer: Vec<Address> = self
            .accounts
            .geaenderte()
            .iter()
            .filter(|a| self.accounts.get(*a).is_some_and(AccountState::ist_leer))
            .copied()
            .collect();
        for a in &leer {
            self.accounts.remove(a);
        }
        leer.len()
    }

    /// Wie oft dieses Konto innerhalb der letzten `fenster` Epochen
    /// geschlachtet wurde, die laufende eingeschlossen.
    ///
    /// **Rein lesend und ohne Kürzung.** Der Aufruf verändert nichts,
    /// auch nicht die Historie: Würde er dabei alte Einträge wegräumen,
    /// hinge der Zustand daran, **wer wann gelesen hat**, und zwei
    /// Knoten mit unterschiedlicher Lesereihenfolge kämen zu
    /// verschiedenen Verpflichtungen. Gekürzt wird ausschließlich beim
    /// Vermerken, also in einem Übergang. Dasselbe Muster wie bei den
    /// verfallenen Credits in `credit_spend`.
    ///
    /// `fenster = 0` ergibt null: kein Fenster, keine Vorgeschichte.
    /// Ein Fenster, das über die Epoche 0 hinausreicht, wird bei 0
    /// abgeschnitten statt umzulaufen.
    pub fn verstoesse_im_fenster(&self, addr: &Address, fenster: u64) -> u64 {
        if fenster == 0 {
            return 0;
        }
        let ab = EpochId(self.epoch.0.saturating_sub(fenster.saturating_sub(1)));
        self.accounts
            .get(addr)
            .map(|k| {
                k.verstoesse
                    .iter()
                    .filter(|v| v.epoche >= ab && v.epoche <= self.epoch)
                    .fold(0u64, |summe, v| summe.saturating_add(v.anzahl as u64))
            })
            .unwrap_or(0)
    }

    /// Vermerkt einen Verstoß in der laufenden Epoche und kürzt die
    /// Historie auf [`VERSTOSS_FENSTER`].
    ///
    /// **Nicht öffentlich, und das ist die Zusage.** Der einzige Weg,
    /// einen Verstoß in den Zustand zu bekommen, führt über ein
    /// gebuchtes Urteil ([`crate::transitions::apply_verdict`]). Wäre
    /// das Vermerken von außen aufrufbar, gäbe es zwei Wege zu
    /// derselben Tatsache — einen, der schlachtet und zählt, und einen,
    /// der nur zählt. Genau daraus entstehen Zähler, die von dem
    /// abweichen, was tatsächlich geschehen ist.
    ///
    /// **Gekürzt wird vor dem Zählen**, damit die Länge der Historie
    /// nach jedem Vermerk höchstens [`VERSTOSS_FENSTER`] Einträge
    /// beträgt, unabhängig davon, wie lange das Konto ruhig war.
    pub(crate) fn verstoss_vermerken(&mut self, addr: &Address) {
        let jetzt = self.epoch;
        let ab = EpochId(jetzt.0.saturating_sub(VERSTOSS_FENSTER.saturating_sub(1)));
        let konto = self.account_mut(addr);
        konto.verstoesse.retain(|v| v.epoche >= ab && v.epoche <= jetzt);
        match konto.verstoesse.iter_mut().find(|v| v.epoche == jetzt) {
            Some(v) => v.anzahl = v.anzahl.saturating_add(1),
            None => {
                konto.verstoesse.push(Verstoss { epoche: jetzt, anzahl: 1 });
                // Aufsteigend nach Epoche: Die Ordnung ist
                // Konsens-Eigenschaft wie die der Konten, denn sie geht
                // in die Serialisierung und damit in das Commitment ein.
                konto.verstoesse.sort_by_key(|v| v.epoche.0);
            }
        }
    }

    /// ⚑ **Die Zustandswurzel** (Skalierung, Festlegung des Projektinhabers
    /// vom 2026-10-29): `SHA-256("myelith-zustand-v2" ‖ Kopf ‖ Baumwurzel)`.
    /// Der **Kopf** ist die Borsh-Kodierung der Einzelwerte (ein paar KB);
    /// die **Sammlungen** stehen im Zustandsbaum, je Eintrag mit dem
    /// Schlüssel `SHA-256(Nummer der Sammlung ‖ Borsh(Schlüssel))` und dem
    /// Werthash `SHA-256(Borsh(Wert))`.
    ///
    /// 📌 Bis hierher: SHA-256 über den ganzen Zustand, je Block; bei einer
    /// Million Konten 127 ms, länger als ein Blocktakt. Über den Baum mit 2000
    /// Änderungen je Block rund 11 ms.
    ///
    /// Diese Fassung rechnet alles neu (für Tests und seltene Aufrufer);
    /// je Block gilt [`Self::wurzel`], die dieselbe Wurzel fortschreibt.
    pub fn commitment(&self) -> Hash {
        let mut klein = myl_types::zustandsbaum::Zustandsbaum::default();
        let mut gross = Grossbaum::default();
        let mut verletzt = false;
        self.alle_einspeisen(&mut Baeume { klein: &mut klein, gross: &mut gross, verletzt: &mut verletzt });
        // Wer den grossen Baum nicht ganz hält, hat keine Gegenprobe für
        // ihn: Es gilt die gehaltene Wurzel.
        let g = match (&self.baum.gross, gross) {
            (Grossbaum::Voll(_), Grossbaum::Voll(mut g)) => g.wurzel(),
            (gehalten, _) => Baumcache { gross: gehalten.clone(), ..Default::default() }.grosswurzel(),
        };
        Self::zusammen(&self.kopf_bytes(), klein.wurzel(), g)
    }

    /// ⚑ **Die Zustandswurzel, fortgeschrieben**: nur die seit dem letzten
    /// Mal geänderten Einträge gehen in den Baum. Gleich [`Self::commitment`].
    pub fn wurzel(&mut self) -> Hash {
        let mut baum = std::mem::take(&mut self.baum);
        if !baum.gebaut {
            baum.klein = Default::default();
            baum.gross = baum.grossbaum_neu();
            self.alle_einspeisen(&mut Baeume { klein: &mut baum.klein, gross: &mut baum.gross, verletzt: &mut baum.verletzt });
            baum.gebaut = true;
        }
        // Nach dem ersten Bau auch: Die Merkzettel leeren (der Eintrag steht
        // dann schon im Baum, ein zweites Setzen ändert nichts).
        self.geaenderte_einspeisen(&mut Baeume { klein: &mut baum.klein, gross: &mut baum.gross, verletzt: &mut baum.verletzt });
        // ⚑ **Das Sicherheitsnetz**: Stimmt die Zahl der Einträge in einem
        //   Baum nicht mit den Sammlungen überein, ist eine Änderung am
        //   Merken vorbeigegangen (📌 beim ersten Einbau ein
        //   `std::mem::take` auf eine Sammlung am Epochenwechsel). Dann wird
        //   neu gebaut, die Wurzel bleibt richtig; in Tests schlägt es laut
        //   an. Den grossen Baum prüft nur, wer ihn ganz hält.
        let (klein, gross) = self.eintraege_der_sammlungen();
        let gross_falsch = matches!(&baum.gross, Grossbaum::Voll(g) if g.len() != gross);
        if baum.klein.len() != klein || gross_falsch {
            debug_assert!(false, "eine Änderung ging am Merken vorbei: {} und {} in den Bäumen, {klein} und {gross} in den Sammlungen", baum.klein.len(), match &baum.gross { Grossbaum::Voll(g) => g.len(), _ => 0 });
            let mut nur_klein = Grossbaum::Wurzel([0; 32]);
            let mut egal = false;
            if gross_falsch {
                baum.gross = baum.grossbaum_neu();
            }
            let ziel_gross = if gross_falsch { &mut baum.gross } else { &mut nur_klein };
            baum.klein = Default::default();
            let mut b = Baeume { klein: &mut baum.klein, gross: ziel_gross, verletzt: &mut egal };
            self.alle_einspeisen(&mut b);
        }
        let k = baum.klein.wurzel();
        let g = baum.grosswurzel();
        self.baum = baum;
        Self::zusammen(&self.kopf_bytes(), k, g)
    }

    /// Die Einträge der kleinen und der grossen Sammlungen.
    fn eintraege_der_sammlungen(&self) -> (usize, usize) {
        let klein = self.trainingssegmente.anzahl() + self.trainingsstand.anzahl() + self.auszahlung.anzahl() + self.miner.anzahl() + self.zusagen.anzahl() + self.fristsperren.anzahl() + self.buendel.anzahl() + self.doppelbelege.anzahl() + self.pruefpunkt_stimmen.anzahl() + self.pruefvolumen.anzahl() + self.auszahlungen_offen.anzahl();
        let gross = self.accounts.anzahl() + self.sitzungen.anzahl() + self.speicher.anzahl() + self.hinterlegungen.anzahl() + self.sitzungsablauf.anzahl();
        (klein, gross)
    }

    /// ⚑ `SHA-256("myelith-zustand-v3" ‖ SHA-256(Kopf) ‖ kleine Wurzel ‖
    /// grosse Wurzel)`; v3 seit den zwei Bäumen (2026-10-29).
    fn zusammen(kopf: &[u8], klein: [u8; 32], gross: [u8; 32]) -> Hash {
        let mut h = Sha256::new();
        h.update(b"myelith-zustand-v3");
        h.update(Sha256::digest(kopf));
        h.update(klein);
        h.update(gross);
        let mut out = [0u8; 32];
        out.copy_from_slice(&h.finalize());
        Hash::from_bytes(out)
    }

    /// ⚑ **Der Erzeuger beginnt die Aufzeichnung** (zustandslose
    /// Validierung, 2026-10-29), vor dem Anwenden eines Blocks: Die Bäume
    /// stehen danach auf dem Stand davor, und die grossen Sammlungen
    /// merken sich jeden Zugriff mit dem Wert davor.
    pub fn aufzeichnen_beginnen(&mut self) {
        self.wurzel();
        self.accounts.aufzeichnen_beginnen();
        self.sitzungen.aufzeichnen_beginnen();
        self.speicher.aufzeichnen_beginnen();
        self.hinterlegungen.aufzeichnen_beginnen();
        self.sitzungsablauf.aufzeichnen_beginnen();
    }

    /// ⚑ **Der Zeuge des eben angewandten Blocks**, nach dem Anwenden und
    /// vor [`Self::wurzel`] (der grosse Baum steht dann noch auf dem Stand
    /// davor). Beendet die Aufzeichnung. Ein Zugriff über eine ganze grosse
    /// Sammlung lässt sich nicht bezeugen: dann `Unvollstaendig`.
    pub fn zeuge_nehmen(&mut self) -> Result<Zeuge, Zeugenfehler> {
        let fehlgriff = self.accounts.fehlgriff()
            || self.sitzungen.fehlgriff()
            || self.speicher.fehlgriff()
            || self.hinterlegungen.fehlgriff()
            || self.sitzungsablauf.fehlgriff();
        let accounts: Vec<_> = self.accounts.aufzeichnung_nehmen().into_iter().collect();
        let sitzungen: Vec<_> = self.sitzungen.aufzeichnung_nehmen().into_iter().collect();
        let speicher: Vec<_> = self.speicher.aufzeichnung_nehmen().into_iter().collect();
        let hinterlegungen: Vec<_> = self.hinterlegungen.aufzeichnung_nehmen().into_iter().collect();
        let sitzungsablauf: Vec<_> = self.sitzungsablauf.aufzeichnung_nehmen().into_iter().collect();
        if fehlgriff {
            return Err(Zeugenfehler::Unvollstaendig);
        }
        let mut schluessel = Vec::new();
        let mut entfernt = Vec::new();
        pfade(&mut schluessel, &mut entfernt, 0, &accounts, &self.accounts);
        pfade(&mut schluessel, &mut entfernt, 1, &sitzungen, &self.sitzungen);
        pfade(&mut schluessel, &mut entfernt, 2, &speicher, &self.speicher);
        pfade(&mut schluessel, &mut entfernt, 12, &hinterlegungen, &self.hinterlegungen);
        pfade(&mut schluessel, &mut entfernt, 14, &sitzungsablauf, &self.sitzungsablauf);
        let Grossbaum::Voll(g) = &mut self.baum.gross else {
            return Err(Zeugenfehler::NichtVoll);
        };
        let baum = g.teilbaum(&schluessel, &entfernt).als_bytes();
        Ok(Zeuge { baum, accounts, sitzungen, speicher, hinterlegungen, sitzungsablauf })
    }

    /// ⚑ **Der Prüfer setzt den Zeugen eines Blocks ein**, vor dem
    /// Anwenden: Der Teilbaum muss die Wurzel des grossen Baums vor dem
    /// Block haben, und jeder Eintrag muss zu ihm passen; dann hält jede
    /// grosse Sammlung genau die Einträge des Zeugen.
    pub fn zeuge_einsetzen(&mut self, zeuge: &Zeuge) -> Result<(), Zeugenfehler> {
        self.wurzel();
        let vorher = self.baum.grosswurzel();
        let mut t = myl_types::zustandsbaum::Teilbaum::aus_bytes(&zeuge.baum).ok_or(Zeugenfehler::Unlesbar)?;
        if t.wurzel() != vorher {
            return Err(Zeugenfehler::FremdeWurzel);
        }
        passt(&t, 0, &zeuge.accounts)?;
        passt(&t, 1, &zeuge.sitzungen)?;
        passt(&t, 2, &zeuge.speicher)?;
        passt(&t, 12, &zeuge.hinterlegungen)?;
        passt(&t, 14, &zeuge.sitzungsablauf)?;
        self.accounts.teilweise_setzen(zeuge.accounts.clone());
        self.sitzungen.teilweise_setzen(zeuge.sitzungen.clone());
        self.speicher.teilweise_setzen(zeuge.speicher.clone());
        self.hinterlegungen.teilweise_setzen(zeuge.hinterlegungen.clone());
        self.sitzungsablauf.teilweise_setzen(zeuge.sitzungsablauf.clone());
        self.baum.gross = Grossbaum::Zeuge(t);
        self.baum.verletzt = false;
        self.baum.vorwurzel = vorher;
        Ok(())
    }

    /// Ob der Block, seit der Zeuge eingesetzt ist, nur Bezeugtes las und
    /// änderte. Nach [`Self::wurzel`] zu fragen.
    pub fn zeuge_eingehalten(&self) -> bool {
        !(self.baum.verletzt
            || self.accounts.fehlgriff()
            || self.sitzungen.fehlgriff()
            || self.speicher.fehlgriff()
            || self.hinterlegungen.fehlgriff()
            || self.sitzungsablauf.fehlgriff())
    }

    /// Nach dem Block: Die grossen Sammlungen werden wieder leer, vom
    /// grossen Baum bleibt die Wurzel; `gilt = false` (der Block wurde
    /// abgewiesen) die von vorher.
    pub fn zeuge_abschliessen(&mut self, gilt: bool) {
        let w = if gilt { self.baum.grosswurzel() } else { self.baum.vorwurzel };
        self.accounts.teilweise_beenden();
        self.sitzungen.teilweise_beenden();
        self.speicher.teilweise_beenden();
        self.hinterlegungen.teilweise_beenden();
        self.sitzungsablauf.teilweise_beenden();
        self.baum.gross = Grossbaum::Wurzel(w);
        self.baum.verletzt = false;
    }

    /// ⚑ **Die leichte Form** (zustandslose Validierung, 2026-10-29): der
    /// Zustand ohne die grossen Sammlungen in Borsh, dahinter die Wurzel des
    /// grossen Baums (32 Bytes). Daraus wird ein zustandsloser Knoten (für
    /// den leichten Abzug); dieselbe Zustandswurzel.
    pub fn leicht_bytes(&mut self) -> Vec<u8> {
        let mut leicht = self.zustandslos();
        let gross = leicht.baum.grosswurzel();
        let mut b = borsh::to_vec(&leicht).expect("ein Zustand ist stets kodierbar");
        b.extend_from_slice(&gross);
        b
    }

    /// ⚑ **Liest die leichte Form.** `None`, wenn die Bytes keine sind,
    /// nicht kanonisch sind oder ⛔️ **in einer grossen Sammlung etwas
    /// steht**: Vom grossen Baum gilt nur die Wurzel, Einträge daneben deckte
    /// sie nicht, und so kämen ungeprüfte Konten in den Zustand. Ob die
    /// Zustandswurzel stimmt, prüft der Aufrufer (gegen den Block).
    pub fn aus_leicht(bytes: &[u8]) -> Option<LedgerState> {
        let (zustand, gross) = bytes.split_at_checked(bytes.len().checked_sub(32)?)?;
        let mut z: LedgerState = borsh::from_slice(zustand).ok()?;
        if z.accounts.anzahl() + z.sitzungen.anzahl() + z.speicher.anzahl() + z.hinterlegungen.anzahl() + z.sitzungsablauf.anzahl() != 0 {
            return None;
        }
        if borsh::to_vec(&z).ok()?.as_slice() != zustand {
            return None;
        }
        let gross: [u8; 32] = gross.try_into().ok()?;
        // Den kleinen Baum bauen, der grosse ist die Wurzel.
        z.baum = Baumcache { gross: Grossbaum::Wurzel(gross), gebaut: true, ..Default::default() };
        let mut verletzt = false;
        let mut klein = myl_types::zustandsbaum::Zustandsbaum::default();
        let mut nur = Grossbaum::Wurzel(gross);
        z.alle_einspeisen(&mut Baeume { klein: &mut klein, gross: &mut nur, verletzt: &mut verletzt });
        z.baum.klein = klein;
        z.trainingssegmente.geaenderte_nehmen();
        z.trainingsstand.geaenderte_nehmen();
        z.auszahlung.geaenderte_nehmen();
        z.miner.geaenderte_nehmen();
        z.zusagen.geaenderte_nehmen();
        z.fristsperren.geaenderte_nehmen();
        z.buendel.geaenderte_nehmen();
        z.doppelbelege.geaenderte_nehmen();
        z.pruefpunkt_stimmen.geaenderte_nehmen();
        z.pruefvolumen.geaenderte_nehmen();
        z.auszahlungen_offen.geaenderte_nehmen();
        Some(z)
    }

    /// ⚑ **Kontobeweise** (2026-10-29): je Adresse das Konto (oder sein
    /// Fehlen) und der Beweis gegen den grossen Baum, auf dem jetzigen
    /// Stand. Nur ein voller Knoten kann sie ausstellen.
    pub fn kontobeweise(&mut self, adressen: &[Address]) -> Option<Vec<Kontobeweis>> {
        self.wurzel();
        let Grossbaum::Voll(g) = &mut self.baum.gross else {
            return None;
        };
        Some(
            adressen
                .iter()
                .map(|a| Kontobeweis {
                    adresse: *a,
                    konto: self.accounts.wert_still(a).cloned(),
                    beweis: g.beweis(&baumschluessel(0, a)),
                })
                .collect(),
        )
    }

    /// Ob ein Kontobeweis gegen die Wurzel des grossen Baums trägt, die
    /// dieser Zustand hält.
    pub fn kontobeweis_traegt(&mut self, b: &Kontobeweis) -> bool {
        self.wurzel();
        let w = self.baum.grosswurzel();
        b.beweis.pruefen(&w, &baumschluessel(0, &b.adresse), b.konto.as_ref().map(werthash).as_ref())
    }

    /// ⚑ **Lagert den grossen Baum auf die Platte aus** (2026-10-30, nach dem
    /// Vorbild von NOMT): unter `pfad`, als Zwischenspeicher (siehe
    /// `myl_types::zustandsbaum`). Dieselbe Wurzel; im Speicher bleibt der
    /// obere Baum. Der nächste Aufruf von [`Self::wurzel`] baut neu.
    pub fn baum_auslagern(&mut self, pfad: &std::path::Path) -> std::io::Result<()> {
        // Gleich öffnen, damit ein Fehler hier auffällt und nicht erst im
        // nächsten Block.
        myl_types::zustandsbaum::Zustandsbaum::mit_ablage(pfad)?;
        self.baum.ablage = Some(pfad.to_path_buf());
        self.baum.gebaut = false;
        Ok(())
    }

    /// Wie viele Knoten des grossen Baums im Speicher stehen und wie viele
    /// Bytes ausgelagert sind (für Angaben).
    pub fn baum_groessen(&self) -> (u64, u64) {
        match &self.baum.gross {
            Grossbaum::Voll(g) => (g.knoten_im_speicher() as u64, g.ablage().map_or(0, |a| a.belegt())),
            _ => (0, 0),
        }
    }

    /// Die Konten zu `adressen`, ohne als Zugriff zu zählen (`None`: es gibt
    /// es nicht). ⚠️ **Nicht im Blockpfad**: für einen zustandslosen Knoten,
    /// der nach einem geprüften Block seine eigenen Konten fortschreibt.
    pub fn konten_still(&self, adressen: &[Address]) -> Vec<(Address, Option<AccountState>)> {
        adressen.iter().map(|a| (*a, self.accounts.wert_still(a).cloned())).collect()
    }

    /// ⚑ **Der Stand eines prüfenden Knotens**: dieser Zustand ohne die
    /// grossen Sammlungen, vom grossen Baum nur die Wurzel. Dieselbe
    /// Zustandswurzel.
    pub fn zustandslos(&mut self) -> LedgerState {
        self.wurzel();
        let s = self.sammlungen_nehmen();
        let baum = std::mem::take(&mut self.baum);
        let mut leicht = self.clone();
        self.sammlungen_legen(s);
        leicht.trainingssegmente = self.trainingssegmente.clone();
        leicht.trainingsstand = self.trainingsstand.clone();
        leicht.auszahlung = self.auszahlung.clone();
        leicht.miner = self.miner.clone();
        leicht.zusagen = self.zusagen.clone();
        leicht.fristsperren = self.fristsperren.clone();
        leicht.buendel = self.buendel.clone();
        leicht.doppelbelege = self.doppelbelege.clone();
        leicht.pruefpunkt_stimmen = self.pruefpunkt_stimmen.clone();
        leicht.pruefvolumen = self.pruefvolumen.clone();
        leicht.auszahlungen_offen = self.auszahlungen_offen.clone();
        // Nach `wurzel` ist nichts mehr gemerkt; der kleine Baum steht.
        leicht.baum = Baumcache { klein: baum.klein.clone(), gross: Grossbaum::Wurzel(baum.grosswurzel_fest()), gebaut: true, verletzt: false, vorwurzel: [0; 32], ablage: None };
        self.baum = baum;
        leicht
    }

    /// Die Einzelwerte, in der Folge ihrer Felder. ⚑ **Erschöpfend
    /// zerlegt**: Ein neues Feld baut nicht, bis es hier oder bei den
    /// Sammlungen steht; so fällt keines aus der Wurzel.
    fn kopf_bytes(&self) -> Vec<u8> {
        let LedgerState {
            epoch,
            credit_price,
            burn_epoche,
            burn_ema,
            vtfe_epoche,
            vtfe_vorepoche,
            modell_version,
            korpus,
            burn_ema_bis,
            podmodell,
            epochensaat,
            epochensaat_naechste,
            historie,
            archiv,
            pruefpunkt_bestaetigt,
            stichproben_offen,
            grundpreis_je_byte,
            gebuehren_epoche,
            gebuehren_validatoren,
            credit_preis_fein,
            zwillinge_einig,
            zwillinge_uneinig,
            stichprobe_wurzeln,
            zurueckgehalten,
            pruefauftraege,
            streitfaelle,
            uneinigkeiten,
            vorbehalte,
            rueckbuchungen,
            sitzungsablauf_bis,
            accounts: _,
            sitzungen: _,
            speicher: _,
            trainingssegmente: _,
            trainingsstand: _,
            auszahlung: _,
            miner: _,
            zusagen: _,
            fristsperren: _,
            buendel: _,
            doppelbelege: _,
            pruefpunkt_stimmen: _,
            hinterlegungen: _,
            pruefvolumen: _,
            sitzungsablauf: _,
            auszahlungen_offen: _,
            baum: _
        } = self;
        let mut aus = Vec::new();
        epoch.serialize(&mut aus).expect("in einen Vec");
        credit_price.serialize(&mut aus).expect("in einen Vec");
        burn_epoche.serialize(&mut aus).expect("in einen Vec");
        burn_ema.serialize(&mut aus).expect("in einen Vec");
        vtfe_epoche.serialize(&mut aus).expect("in einen Vec");
        vtfe_vorepoche.serialize(&mut aus).expect("in einen Vec");
        modell_version.serialize(&mut aus).expect("in einen Vec");
        korpus.serialize(&mut aus).expect("in einen Vec");
        burn_ema_bis.serialize(&mut aus).expect("in einen Vec");
        podmodell.serialize(&mut aus).expect("in einen Vec");
        epochensaat.serialize(&mut aus).expect("in einen Vec");
        epochensaat_naechste.serialize(&mut aus).expect("in einen Vec");
        historie.serialize(&mut aus).expect("in einen Vec");
        archiv.serialize(&mut aus).expect("in einen Vec");
        pruefpunkt_bestaetigt.serialize(&mut aus).expect("in einen Vec");
        stichproben_offen.serialize(&mut aus).expect("in einen Vec");
        grundpreis_je_byte.serialize(&mut aus).expect("in einen Vec");
        gebuehren_epoche.serialize(&mut aus).expect("in einen Vec");
        gebuehren_validatoren.serialize(&mut aus).expect("in einen Vec");
        credit_preis_fein.serialize(&mut aus).expect("in einen Vec");
        zwillinge_einig.serialize(&mut aus).expect("in einen Vec");
        zwillinge_uneinig.serialize(&mut aus).expect("in einen Vec");
        stichprobe_wurzeln.serialize(&mut aus).expect("in einen Vec");
        zurueckgehalten.serialize(&mut aus).expect("in einen Vec");
        pruefauftraege.serialize(&mut aus).expect("in einen Vec");
        streitfaelle.serialize(&mut aus).expect("in einen Vec");
        uneinigkeiten.serialize(&mut aus).expect("in einen Vec");
        vorbehalte.serialize(&mut aus).expect("in einen Vec");
        rueckbuchungen.serialize(&mut aus).expect("in einen Vec");
        sitzungsablauf_bis.serialize(&mut aus).expect("in einen Vec");
        aus
    }

    fn alle_einspeisen(&self, baum: &mut Baeume<'_>) {
        einspeisen(baum, 0, &self.accounts);
        einspeisen(baum, 1, &self.sitzungen);
        einspeisen(baum, 2, &self.speicher);
        einspeisen(baum, 3, &self.trainingssegmente);
        einspeisen(baum, 4, &self.trainingsstand);
        einspeisen(baum, 5, &self.auszahlung);
        einspeisen(baum, 6, &self.miner);
        einspeisen(baum, 7, &self.zusagen);
        einspeisen(baum, 8, &self.fristsperren);
        einspeisen(baum, 9, &self.buendel);
        einspeisen(baum, 10, &self.doppelbelege);
        einspeisen(baum, 11, &self.pruefpunkt_stimmen);
        einspeisen(baum, 12, &self.hinterlegungen);
        einspeisen(baum, 13, &self.pruefvolumen);
        einspeisen(baum, 14, &self.sitzungsablauf);
        einspeisen(baum, 15, &self.auszahlungen_offen);
    }

    fn geaenderte_einspeisen(&mut self, baum: &mut Baeume<'_>) {
        // Erst die geänderten Schlüssel nehmen, dann lesen.
        let g = Geaendert {
            accounts: self.accounts.geaenderte_nehmen(),
            sitzungen: self.sitzungen.geaenderte_nehmen(),
            speicher: self.speicher.geaenderte_nehmen(),
            trainingssegmente: self.trainingssegmente.geaenderte_nehmen(),
            trainingsstand: self.trainingsstand.geaenderte_nehmen(),
            auszahlung: self.auszahlung.geaenderte_nehmen(),
            miner: self.miner.geaenderte_nehmen(),
            zusagen: self.zusagen.geaenderte_nehmen(),
            fristsperren: self.fristsperren.geaenderte_nehmen(),
            buendel: self.buendel.geaenderte_nehmen(),
            doppelbelege: self.doppelbelege.geaenderte_nehmen(),
            pruefpunkt_stimmen: self.pruefpunkt_stimmen.geaenderte_nehmen(),
            hinterlegungen: self.hinterlegungen.geaenderte_nehmen(),
            pruefvolumen: self.pruefvolumen.geaenderte_nehmen(),
            sitzungsablauf: self.sitzungsablauf.geaenderte_nehmen(),
            auszahlungen_offen: self.auszahlungen_offen.geaenderte_nehmen(),
        };
        g.einspeisen(self, baum);
    }

    /// **Ein Blick unter anderer Epoche und Saat** (Skalierung, 2026-10-29):
    /// für Ableitungen wie die Zuteilung der nächsten Epoche, ohne den
    /// Zustand zu kopieren (bis hierher zwei volle Kopien je Durchlauf der
    /// Miner-Schleife). Danach steht alles wie vorher.
    pub fn als_epoche<R>(&mut self, epoche: EpochId, saat: Hash, f: impl FnOnce(&LedgerState) -> R) -> R {
        let alt = (self.epoch, self.epochensaat);
        self.epoch = epoche;
        self.epochensaat = saat;
        let r = f(self);
        (self.epoch, self.epochensaat) = alt;
        r
    }

    /// ⚑ **Sichern, bevor ein fremder Block angewandt wird** (statt einer
    /// Kopie des ganzen Zustands, bei einer Million Konten 30 ms je Block):
    /// Die Sammlungen beginnen ein Journal, kopiert wird nur der Rest.
    pub fn sichern(&mut self) -> Sicherung {
        let s = self.sammlungen_nehmen();
        let baum = std::mem::take(&mut self.baum);
        let rest = self.clone();
        self.sammlungen_legen(s);
        self.baum = baum;
        self.accounts.journal_beginnen();
        self.sitzungen.journal_beginnen();
        self.speicher.journal_beginnen();
        self.trainingssegmente.journal_beginnen();
        self.trainingsstand.journal_beginnen();
        self.auszahlung.journal_beginnen();
        self.miner.journal_beginnen();
        self.zusagen.journal_beginnen();
        self.fristsperren.journal_beginnen();
        self.buendel.journal_beginnen();
        self.doppelbelege.journal_beginnen();
        self.pruefpunkt_stimmen.journal_beginnen();
        self.hinterlegungen.journal_beginnen();
        self.pruefvolumen.journal_beginnen();
        self.sitzungsablauf.journal_beginnen();
        self.auszahlungen_offen.journal_beginnen();
        Sicherung(Box::new(rest))
    }

    /// Der Block gilt: Die Änderungen bleiben.
    pub fn sicherung_verwerfen(&mut self) {
        self.accounts.journal_abschliessen();
        self.sitzungen.journal_abschliessen();
        self.speicher.journal_abschliessen();
        self.trainingssegmente.journal_abschliessen();
        self.trainingsstand.journal_abschliessen();
        self.auszahlung.journal_abschliessen();
        self.miner.journal_abschliessen();
        self.zusagen.journal_abschliessen();
        self.fristsperren.journal_abschliessen();
        self.buendel.journal_abschliessen();
        self.doppelbelege.journal_abschliessen();
        self.pruefpunkt_stimmen.journal_abschliessen();
        self.hinterlegungen.journal_abschliessen();
        self.pruefvolumen.journal_abschliessen();
        self.sitzungsablauf.journal_abschliessen();
        self.auszahlungen_offen.journal_abschliessen();
    }

    /// Der Block gilt nicht: zurück auf die Sicherung.
    pub fn zuruecksetzen(&mut self, sicherung: Sicherung) {
        self.accounts.journal_zurueckspielen();
        self.sitzungen.journal_zurueckspielen();
        self.speicher.journal_zurueckspielen();
        self.trainingssegmente.journal_zurueckspielen();
        self.trainingsstand.journal_zurueckspielen();
        self.auszahlung.journal_zurueckspielen();
        self.miner.journal_zurueckspielen();
        self.zusagen.journal_zurueckspielen();
        self.fristsperren.journal_zurueckspielen();
        self.buendel.journal_zurueckspielen();
        self.doppelbelege.journal_zurueckspielen();
        self.pruefpunkt_stimmen.journal_zurueckspielen();
        self.hinterlegungen.journal_zurueckspielen();
        self.pruefvolumen.journal_zurueckspielen();
        self.sitzungsablauf.journal_zurueckspielen();
        self.auszahlungen_offen.journal_zurueckspielen();
        let s = self.sammlungen_nehmen();
        let baum = std::mem::take(&mut self.baum);
        *self = *sicherung.0;
        self.sammlungen_legen(s);
        self.baum = baum;
    }

    fn sammlungen_nehmen(&mut self) -> SammlungenAllein {
        SammlungenAllein {
            accounts: std::mem::take(&mut self.accounts),
            sitzungen: std::mem::take(&mut self.sitzungen),
            speicher: std::mem::take(&mut self.speicher),
            trainingssegmente: std::mem::take(&mut self.trainingssegmente),
            trainingsstand: std::mem::take(&mut self.trainingsstand),
            auszahlung: std::mem::take(&mut self.auszahlung),
            miner: std::mem::take(&mut self.miner),
            zusagen: std::mem::take(&mut self.zusagen),
            fristsperren: std::mem::take(&mut self.fristsperren),
            buendel: std::mem::take(&mut self.buendel),
            doppelbelege: std::mem::take(&mut self.doppelbelege),
            pruefpunkt_stimmen: std::mem::take(&mut self.pruefpunkt_stimmen),
            hinterlegungen: std::mem::take(&mut self.hinterlegungen),
            pruefvolumen: std::mem::take(&mut self.pruefvolumen),
            sitzungsablauf: std::mem::take(&mut self.sitzungsablauf),
            auszahlungen_offen: std::mem::take(&mut self.auszahlungen_offen),
        }
    }

    fn sammlungen_legen(&mut self, s: SammlungenAllein) {
        self.accounts = s.accounts;
        self.sitzungen = s.sitzungen;
        self.speicher = s.speicher;
        self.trainingssegmente = s.trainingssegmente;
        self.trainingsstand = s.trainingsstand;
        self.auszahlung = s.auszahlung;
        self.miner = s.miner;
        self.zusagen = s.zusagen;
        self.fristsperren = s.fristsperren;
        self.buendel = s.buendel;
        self.doppelbelege = s.doppelbelege;
        self.pruefpunkt_stimmen = s.pruefpunkt_stimmen;
        self.hinterlegungen = s.hinterlegungen;
        self.pruefvolumen = s.pruefvolumen;
        self.sitzungsablauf = s.sitzungsablauf;
        self.auszahlungen_offen = s.auszahlungen_offen;
    }
}

/// Die Sammlungen, für einen Augenblick aus dem Zustand genommen.
struct SammlungenAllein {
    accounts: crate::sammlung::Sammlung<Address, AccountState>,
    sitzungen: crate::sammlung::Sammlung<SitzungId, Sitzung>,
    speicher: crate::sammlung::Sammlung<MerkleRoot, Manifest>,
    trainingssegmente: crate::sammlung::Sammlung<PodId, myl_types::trainingssegment::Trainingssegment>,
    trainingsstand: crate::sammlung::Sammlung<MinerId, u64>,
    auszahlung: crate::sammlung::Sammlung<MinerId, Address>,
    miner: crate::sammlung::Sammlung<MinerId, MinerRegistration>,
    zusagen: crate::sammlung::Sammlung<MinerId, Vec<myl_types::zusage::Kapazitaetszusage>>,
    fristsperren: crate::sammlung::Sammlung<MinerId, EpochId>,
    buendel: crate::sammlung::Sammlung<PodId, PoIBundle>,
    doppelbelege: crate::sammlung::Sammlung<(Address, u8, u64), u64>,
    pruefpunkt_stimmen: crate::sammlung::Sammlung<MinerId, u64>,
    hinterlegungen: crate::sammlung::Sammlung<(u8, [u8; 32]), (Address, u64)>,
    pruefvolumen: crate::sammlung::Sammlung<MinerId, u64>,
    sitzungsablauf: crate::sammlung::Sammlung<u64, Vec<SitzungId>>,
    auszahlungen_offen: crate::sammlung::Sammlung<Address, u64>,
}

impl Baumcache {
    /// Ein neuer, leerer grosser Baum: ausgelagert, wenn es eine Ablage
    /// gibt. 📌 Eine Ablage, die sich nicht öffnen lässt, hält den Knoten
    /// an: Ein Zwischenspeicher, der still in den Speicher ausweicht, hätte
    /// den Speicherbedarf verschleiert.
    fn grossbaum_neu(&self) -> Grossbaum {
        match &self.ablage {
            Some(p) => Grossbaum::Voll(
                myl_types::zustandsbaum::Zustandsbaum::mit_ablage(p).expect("die Ablage des Zustandsbaums lässt sich nicht öffnen"),
            ),
            None => Grossbaum::default(),
        }
    }

    /// Die Wurzel des grossen Baums, wie er gehalten wird.
    fn grosswurzel(&mut self) -> [u8; 32] {
        match &mut self.gross {
            Grossbaum::Voll(g) => g.wurzel(),
            Grossbaum::Zeuge(t) => t.wurzel(),
            Grossbaum::Wurzel(w) => *w,
        }
    }

    /// Dasselbe ohne `&mut` (nach [`LedgerState::wurzel`] ist gehasht).
    /// 📌 Ohne Kopie: Eine Kopie eines ausgelagerten Baums lädt ihn ganz
    /// (und das an jedem Prüfpunkt, für den leichten Abzug).
    fn grosswurzel_fest(&self) -> [u8; 32] {
        match &self.gross {
            Grossbaum::Voll(g) => g.wurzel_gerechnet().unwrap_or_else(|| g.clone().wurzel()),
            Grossbaum::Zeuge(t) => t.clone().wurzel(),
            Grossbaum::Wurzel(w) => *w,
        }
    }
}

/// Speist alle Einträge einer Sammlung in den Baum.
fn einspeisen<K, V>(baum: &mut Baeume<'_>, nummer: u8, s: &crate::sammlung::Sammlung<K, V>)
where
    K: Ord + Clone + BorshSerialize,
    V: Clone + BorshSerialize,
{
    for (k, v) in s.alle() {
        baum.setzen(nummer, baumschluessel(nummer, k), Some(werthash(v)));
    }
}

/// Die Pfade einer aufgezeichneten Sammlung für den Teilbaum, und darunter
/// die, deren Eintrag der Block entfernt hat.
fn pfade<K, V>(
    schluessel: &mut Vec<[u8; 32]>,
    entfernt: &mut Vec<[u8; 32]>,
    nummer: u8,
    aufgezeichnet: &[(K, Option<V>)],
    jetzt: &crate::sammlung::Sammlung<K, V>,
) where
    K: Ord + Clone + BorshSerialize,
    V: Clone + BorshSerialize,
{
    for (k, vorher) in aufgezeichnet {
        let b = baumschluessel(nummer, k);
        schluessel.push(b);
        if vorher.is_some() && !jetzt.enthaelt_still(k) {
            entfernt.push(b);
        }
    }
}

/// Ob die Einträge eines Zeugen zu seinem Teilbaum passen.
fn passt<K, V>(t: &myl_types::zustandsbaum::Teilbaum, nummer: u8, eintraege: &[(K, Option<V>)]) -> Result<(), Zeugenfehler>
where
    K: Ord + Clone + BorshSerialize,
    V: Clone + BorshSerialize,
{
    for (k, v) in eintraege {
        let im_baum = t.lesen(&baumschluessel(nummer, k)).map_err(|_| Zeugenfehler::EintragFalsch)?;
        if im_baum != v.as_ref().map(werthash) {
            return Err(Zeugenfehler::EintragFalsch);
        }
    }
    Ok(())
}

fn baumschluessel<K: BorshSerialize>(nummer: u8, k: &K) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([nummer]);
    h.update(borsh::to_vec(k).expect("ein Schlüssel ist stets kodierbar"));
    h.finalize().into()
}

fn werthash<V: BorshSerialize>(v: &V) -> [u8; 32] {
    Sha256::digest(borsh::to_vec(v).expect("ein Wert ist stets kodierbar")).into()
}

/// Die geänderten Schlüssel je Sammlung.
struct Geaendert {
    accounts: std::collections::BTreeSet<Address>,
    sitzungen: std::collections::BTreeSet<SitzungId>,
    speicher: std::collections::BTreeSet<MerkleRoot>,
    trainingssegmente: std::collections::BTreeSet<PodId>,
    trainingsstand: std::collections::BTreeSet<MinerId>,
    auszahlung: std::collections::BTreeSet<MinerId>,
    miner: std::collections::BTreeSet<MinerId>,
    zusagen: std::collections::BTreeSet<MinerId>,
    fristsperren: std::collections::BTreeSet<MinerId>,
    buendel: std::collections::BTreeSet<PodId>,
    doppelbelege: std::collections::BTreeSet<(Address, u8, u64)>,
    pruefpunkt_stimmen: std::collections::BTreeSet<MinerId>,
    hinterlegungen: std::collections::BTreeSet<(u8, [u8; 32])>,
    pruefvolumen: std::collections::BTreeSet<MinerId>,
    sitzungsablauf: std::collections::BTreeSet<u64>,
    auszahlungen_offen: std::collections::BTreeSet<Address>,
}

impl Geaendert {
    fn einspeisen(self, z: &LedgerState, baum: &mut Baeume<'_>) {
        nachfuehren(baum, 0, &z.accounts, self.accounts);
        nachfuehren(baum, 1, &z.sitzungen, self.sitzungen);
        nachfuehren(baum, 2, &z.speicher, self.speicher);
        nachfuehren(baum, 3, &z.trainingssegmente, self.trainingssegmente);
        nachfuehren(baum, 4, &z.trainingsstand, self.trainingsstand);
        nachfuehren(baum, 5, &z.auszahlung, self.auszahlung);
        nachfuehren(baum, 6, &z.miner, self.miner);
        nachfuehren(baum, 7, &z.zusagen, self.zusagen);
        nachfuehren(baum, 8, &z.fristsperren, self.fristsperren);
        nachfuehren(baum, 9, &z.buendel, self.buendel);
        nachfuehren(baum, 10, &z.doppelbelege, self.doppelbelege);
        nachfuehren(baum, 11, &z.pruefpunkt_stimmen, self.pruefpunkt_stimmen);
        nachfuehren(baum, 12, &z.hinterlegungen, self.hinterlegungen);
        nachfuehren(baum, 13, &z.pruefvolumen, self.pruefvolumen);
        nachfuehren(baum, 14, &z.sitzungsablauf, self.sitzungsablauf);
        nachfuehren(baum, 15, &z.auszahlungen_offen, self.auszahlungen_offen);
    }
}

fn nachfuehren<K, V>(baum: &mut Baeume<'_>, nummer: u8, s: &crate::sammlung::Sammlung<K, V>, keys: std::collections::BTreeSet<K>)
where
    K: Ord + Clone + BorshSerialize,
    V: Clone + BorshSerialize,
{
    for k in keys {
        baum.setzen(nummer, baumschluessel(nummer, &k), s.wert_still(&k).map(werthash));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    #[test]
    fn genesis_zustand_ist_leer_und_bekommt_konten() {
        let mut state = LedgerState::genesis(100);
        assert_eq!(state.epoch, EpochId(0));
        assert_eq!(state.accounts.len(), 0);
        assert_eq!(state.account(&adresse(1)), AccountState::empty());
        state.account_mut(&adresse(1)).balance = 500;
        assert_eq!(state.account(&adresse(1)).balance, 500);
        assert_eq!(state.accounts.len(), 1);
    }

    #[test]
    fn commitment_ist_deterministisch_und_unterscheidet_zustaende() {
        let mut a = LedgerState::genesis(100);
        let mut b = LedgerState::genesis(100);
        assert_eq!(a.commitment(), b.commitment());

        a.account_mut(&adresse(1)).balance = 10;
        b.account_mut(&adresse(1)).balance = 10;
        // Gleiche Änderungen, unabhängig voneinander ausgeführt:
        assert_eq!(a.commitment(), b.commitment());

        b.account_mut(&adresse(1)).balance = 11;
        assert_ne!(a.commitment(), b.commitment());
    }

    // --- Verstoßhistorie ---------------------------------------------

    /// Gezählt wird, was im Fenster liegt, und nur das.
    #[test]
    fn nur_verstoesse_im_fenster_zaehlen() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);

        state.epoch = EpochId(1);
        state.verstoss_vermerken(&a);
        state.epoch = EpochId(5);
        state.verstoss_vermerken(&a);
        state.verstoss_vermerken(&a);

        // Fenster 10 ab Epoche 5 reicht bis Epoche 0: alle drei zaehlen.
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 3);
        // Fenster 5 ab Epoche 5 reicht bis Epoche 1: ebenfalls alle drei.
        assert_eq!(state.verstoesse_im_fenster(&a, 5), 3);
        // Fenster 2 ab Epoche 5 reicht bis Epoche 4: nur die beiden aus 5.
        assert_eq!(state.verstoesse_im_fenster(&a, 2), 2);
        // Kein Fenster, keine Vorgeschichte.
        assert_eq!(state.verstoesse_im_fenster(&a, 0), 0);
        // Ein unbeteiligtes Konto hat keine.
        assert_eq!(state.verstoesse_im_fenster(&adresse(2), VERSTOSS_FENSTER), 0);
    }

    /// **Ein Fenster ueber die Epoche 0 hinaus laeuft nicht um.**
    ///
    /// Ohne die saettigende Subtraktion ergaebe `0 - 9` die Untergrenze
    /// `u64::MAX`, und die Bedingung `epoche >= ab` waere fuer **keinen**
    /// Eintrag erfuellt: Die Vorgeschichte waere in den ersten Epochen des
    /// Netzes leer, und die Staffelung damit abgeschaltet, genau in der
    /// Zeit, in der sie am ehesten gebraucht wird.
    #[test]
    fn ein_fenster_vor_der_ersten_epoche_laeuft_nicht_um() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        state.epoch = EpochId(0);
        state.verstoss_vermerken(&a);
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 1);
        assert_eq!(state.verstoesse_im_fenster(&a, u64::MAX), 1);
    }

    /// **Die Historie waechst nicht ueber das Fenster hinaus.**
    ///
    /// Sonst haenge die Groesse des Konsenszustands daran, wie oft jemand
    /// auffaellt, und das ist eine Groesse, die ein Angreifer selbst
    /// bestimmt.
    #[test]
    fn die_historie_bleibt_auf_das_fenster_begrenzt() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        for e in 0..200u64 {
            state.epoch = EpochId(e);
            // Mehrfach je Epoche: Der zweite Vermerk darf keinen zweiten
            // Eintrag erzeugen.
            state.verstoss_vermerken(&a);
            state.verstoss_vermerken(&a);
            assert!(
                state.account(&a).verstoesse.len() as u64 <= VERSTOSS_FENSTER,
                "nach Epoche {e} stehen {} Eintraege",
                state.account(&a).verstoesse.len()
            );
        }
        // Und gezaehlt wird trotzdem richtig: zwei je Epoche ueber das Fenster.
        assert_eq!(
            state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER),
            2 * VERSTOSS_FENSTER
        );
    }

    /// **Lesen veraendert nichts, auch nicht die Historie.**
    ///
    /// Raeumte das Lesen alte Eintraege weg, hinge der Zustand daran, wer
    /// wann gelesen hat, und zwei Knoten mit verschiedener
    /// Lesereihenfolge kaemen zu verschiedenen Verpflichtungen. Das ist
    /// der Grund, warum ausschliesslich der Uebergang kuerzt.
    #[test]
    fn lesen_veraendert_die_verpflichtung_nicht() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        state.epoch = EpochId(3);
        state.verstoss_vermerken(&a);
        state.epoch = EpochId(80); // weit ausserhalb des Fensters

        let vorher = state.commitment();
        for fenster in [0u64, 1, VERSTOSS_FENSTER, u64::MAX] {
            let _ = state.verstoesse_im_fenster(&a, fenster);
        }
        assert_eq!(vorher, state.commitment(), "das Lesen hat den Zustand bewegt");
        // Der alte Eintrag liegt noch da und zaehlt trotzdem nicht mehr.
        assert_eq!(state.account(&a).verstoesse.len(), 1);
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 0);
    }

    /// **Die Historie ist Konsensfeld, nicht Beiwerk.**
    ///
    /// Zwei Zustaende, die sich nur in ihr unterscheiden, muessen
    /// verschiedene Verpflichtungen tragen — sonst koennten zwei Knoten
    /// mit verschiedenen Vorgeschichten denselben Block bestaetigen und
    /// beim naechsten Urteil verschieden hoch schlachten.
    #[test]
    fn die_historie_geht_in_die_verpflichtung_ein() {
        let mut ohne = LedgerState::genesis(100);
        ohne.account_mut(&adresse(1)).staked = 10;
        let mut mit = ohne.clone();
        assert_eq!(ohne.commitment(), mit.commitment());
        mit.verstoss_vermerken(&adresse(1));
        assert_ne!(
            ohne.commitment(),
            mit.commitment(),
            "ein vermerkter Verstoss blieb ohne Wirkung auf die Verpflichtung"
        );
    }

    /// Die Reihenfolge der Eintraege haengt nicht an der Reihenfolge der
    /// Epochen, in denen vermerkt wurde.
    #[test]
    fn die_historie_ist_nach_epoche_geordnet() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        for e in [5u64, 2, 7, 1] {
            state.epoch = EpochId(e);
            state.verstoss_vermerken(&a);
        }
        let epochen: Vec<u64> =
            state.account(&a).verstoesse.iter().map(|v| v.epoche.0).collect();
        let mut sortiert = epochen.clone();
        sortiert.sort_unstable();
        assert_eq!(epochen, sortiert, "die Historie steht nicht aufsteigend");
    }

    #[test]
    fn kontenordnung_ist_unabhaengig_von_einfuegereihenfolge() {
        // BTreeMap: dieselbe Zielmenge ergibt dieselbe Serialisierung,
        // egal in welcher Reihenfolge die Konten angelegt wurden.
        let mut aufsteigend = LedgerState::genesis(100);
        for b in [1u8, 2, 3] {
            aufsteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        let mut absteigend = LedgerState::genesis(100);
        for b in [3u8, 2, 1] {
            absteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        assert_eq!(aufsteigend.commitment(), absteigend.commitment());
    }

    /// ⚑ **Die fortgeschriebene Wurzel ist die neu gerechnete** (Skalierung,
    /// 2026-10-29), über Konten, eine zweite Sammlung, einen Kopfwert und
    /// Entfernungen hinweg, Block für Block; und zwei Zustände, die
    /// denselben Inhalt auf verschiedenen Wegen erreichen, haben dieselbe.
    #[test]
    fn die_fortgeschriebene_wurzel_ist_die_neu_gerechnete() {
        let mut a = LedgerState::genesis(100);
        let mut b = LedgerState::genesis(100);
        for runde in 0..5u8 {
            for i in 0..50u8 {
                a.account_mut(&adresse(i)).balance = u64::from(runde) * 1000 + u64::from(i);
            }
            a.pruefvolumen.insert(MinerId::new([runde; 32]), u64::from(runde));
            a.grundpreis_je_byte = u64::from(runde) + 1;
            if runde == 3 {
                a.accounts.remove(&adresse(7));
            }
            assert_eq!(a.wurzel(), a.commitment(), "Runde {runde}");
        }
        // Derselbe Inhalt, anders erreicht (Konto 7 ist in Runde 4 wieder da;
        // hier entsteht, verschwindet und entsteht es neu).
        b.account_mut(&adresse(7)).balance = 1;
        b.accounts.remove(&adresse(7));
        for i in (0..50u8).rev() {
            b.account_mut(&adresse(i)).balance = 4000 + u64::from(i);
        }
        for r in (0..5u8).rev() {
            b.pruefvolumen.insert(MinerId::new([r; 32]), u64::from(r));
        }
        b.grundpreis_je_byte = 5;
        assert_eq!(b.wurzel(), a.wurzel());
        // Ein anderer Kopfwert ist eine andere Wurzel.
        b.zwillinge_einig += 1;
        assert_ne!(b.wurzel(), a.wurzel());
    }

    /// ⚑ **Sichern und Zurücksetzen** statt einer Kopie des ganzen Zustands:
    /// Nach einem verworfenen Block sind Inhalt und Wurzel wie vorher, in den
    /// Sammlungen wie im Kopf; nach einem angenommenen bleiben die Änderungen.
    #[test]
    fn zuruecksetzen_stellt_inhalt_und_wurzel_wieder_her() {
        let mut s = LedgerState::genesis(100);
        for i in 0..20u8 {
            s.account_mut(&adresse(i)).balance = u64::from(i);
        }
        let vorher = s.clone();
        let w = s.wurzel();
        let sicherung = s.sichern();
        s.account_mut(&adresse(1)).balance = 999;
        s.account_mut(&adresse(200)).balance = 5;
        s.accounts.remove(&adresse(3));
        s.epoch = EpochId(9);
        s.zurueckgehalten.push((1, MinerId::new([1; 32]), 2));
        assert_ne!(s.wurzel(), w);
        s.zuruecksetzen(sicherung);
        assert_eq!(s, vorher);
        assert_eq!(s.wurzel(), w);
        assert_eq!(s.wurzel(), s.commitment());
        let sicherung = s.sichern();
        s.account_mut(&adresse(1)).balance = 7;
        s.sicherung_verwerfen();
        drop(sicherung);
        assert_eq!(s.account(&adresse(1)).balance, 7);
        assert_eq!(s.wurzel(), s.commitment());
    }

    /// ⚑ **Leere Konten fallen weg, und der Nonce-Boden schützt** (2026-10-29):
    /// Ein geleertes Konto verschwindet am Ende des Blocks; entsteht an
    /// derselben Adresse in einer späteren Epoche wieder eines, beginnt seine
    /// Nonce über allen früheren. Ein Konto mit Einsatz, Credits oder
    /// Verstössen bleibt; geprüft werden nur die geänderten.
    #[test]
    fn leere_konten_fallen_weg_und_der_nonce_boden_schuetzt() {
        let mut s = LedgerState::genesis(100);
        s.account_mut(&adresse(1)).balance = 10;
        s.account_mut(&adresse(1)).nonce = 7;
        s.account_mut(&adresse(2)).staked = 5;
        s.wurzel();
        assert_eq!(s.leere_konten_entfernen(), 0);
        s.account_mut(&adresse(1)).balance = 0;
        s.account_mut(&adresse(2)).balance = 0;
        assert_eq!(s.leere_konten_entfernen(), 1, "nur das wirklich leere");
        assert!(!s.accounts.contains_key(&adresse(1)));
        assert!(s.accounts.contains_key(&adresse(2)), "mit Einsatz bleibt es");
        assert_eq!(s.wurzel(), s.commitment());
        // Wieder angelegt in Epoche 3: die Nonce über allen früheren.
        s.epoch = EpochId(3);
        assert_eq!(s.account(&adresse(1)).nonce, 3 << 32, "auch beim Lesen");
        assert_eq!(s.account_mut(&adresse(1)).nonce, 3 << 32);
        assert!(s.account(&adresse(1)).nonce > 7);
        // In Epoche null ist der Boden null.
        assert_eq!(AccountState::neu(EpochId(0)).nonce, 0);
    }
}
