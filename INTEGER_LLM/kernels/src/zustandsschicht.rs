//! **Die rekurrente Zustandsschicht**, ganzzahlig (Arbeitstitel).
//!
//! # ⚑ Was hier neu ist, und warum es eine eigene Sorte Rechnung ist
//!
//! Alle bisherigen Kernel dieses Projekts rechnen **Reduktionen**: Eine
//! Summe ueber viele Produkte, und die darf man umordnen, weil
//! Ganzzahladdition assoziativ ist. **Genau darauf steht die Kernthese.**
//!
//! Diese Schicht rechnet eine **Rekurrenz**. Der Zustand bei Token `i`
//! haengt am Zustand bei `i-1`, es gibt nichts umzuordnen, und die These
//! hilft hier nicht.
//!
//! ⚑ **Sie wird aber auch nicht verletzt, und der Gewinn liegt
//! woanders.** Bei gleichem Ganzzahlzustand und gleicher Eingabe rechnet
//! jeder Knoten denselben Folgezustand, Bit fuer Bit. **Die
//! Gleitkomma-Referenz braucht `float32` gerade deshalb, weil eine
//! Gleitkomma-Akkumulation ueber hunderte Schritte von der Umsetzung
//! abhaengt.** Ein Ganzzahlzustand mit festem Shiftplan ist
//! reproduzierbarer als sie, nicht weniger.
//!
//! # ⚑ Die Rekurrenz, je Token und je Kopf
//!
//! ```text
//! S      = S * g                     der Zustand verblasst
//! kv_mem = Summe_a(S[a][b] * k[a])    Lesen mit dem Schluessel
//! delta  = (v - kv_mem) * beta        Korrektur
//! S     += k (x) delta                Rang-1-Fortschreibung
//! out    = Summe_a(S[a][b] * q[a])    Lesen mit der Abfrage
//! ```
//!
//! # ⛔️ Die Breiten sind gemessen und nicht gewaehlt
//!
//! Sie stammen aus einer Messung vom 2026-09-21, die dieselbe Rekurrenz
//! zweimal ganzzahlig rechnet: einmal mit der zu pruefenden Breite und
//! einmal mit 48 Bit. Gemessen wurde gegen die **echten** Zerfaelle des
//! Zielmodells (`min = 0`, `mittel = 0,649`, `max = 0,999999998980`),
//! und zwar ueber **alle 30** Zustandsebenen.
//!
//! ⚑ **Die Spanne von `a` ist gemessen und nicht angenommen.** Sie kommt
//! zur Laufzeit aus einer Projektion, war also ohne Vorwaertspass nicht
//! bekannt; bis zum 2026-09-21 rechnete die Messung deshalb mit `a = 0`
//! und einer einzigen Ebene. Die Aktivierungsstatistik des gebauten
//! Artefakts haelt den beobachteten Betragsgrosstwert des
//! `in_proj_a`-Ausgangs je Ebene, gesammelt ueber den ganzen
//! Kalibrierkorpus: **6,2 bis 14,4**, je nach Ebene.
//!
//! | | Bruchbits | Grund |
//! |---|---|---|
//! | Zustand | [`ZUSTAND_FRAC`] = 28 | gemessen reichen 24; in `i64` kosten vier Stellen Reserve nichts |
//! | Zerfall | [`ZERFALL_FRAC`] = 28 | `log2(N) + WERT_FRAC - 1` fuer das volle Fenster sind 25, gemessen 26 bis 28 |
//! | Werte | [`WERT_FRAC`] = 8 | wie im uebrigen Rechenpfad |
//! | normiertes `q`, `k` | [`NORM_FRAC`] = 12 | Normieren schrumpft jede Komponente um `sqrt(kopf_dim)`; unter 12 dominiert diese Rundung alles andere |
//!
//! ⛔️ **Hier stand bis zum 2026-09-21 ein Zerfall von 35 Bruchbits, und
//! die Begruendung dafuer war falsch.** Sie lautete: Der Fehler im
//! Produkt waechst mit `N * eps / (1-g)`, also `log2(N/(1-g))` Bit.
//!
//! ⚑ **Richtig ist das Minimum und nicht das Produkt.** Ein Fehler `eps`
//! in `g` wirkt sich auf das Produkt ueber die Folge mit etwa
//! `eps * min(N, 1/(1-g))` aus: Eine Folge kann nur so weit
//! zurueckwirken, wie sie **lang** ist ODER wie weit das Gedaechtnis
//! reicht, **nicht beides multipliziert**. Wer beides multipliziert,
//! bezahlt Stellen fuer einen Fall, den es nicht gibt.
//!
//! ⚑ **Und der haerteste Pruefpunkt liegt nicht am Rand, sondern in der
//! Mitte.** Der langsamste im Modell gemessene Zerfall ist
//! `g = 0,999999998980`; er sieht am schaerfsten aus und ist der
//! harmloseste, denn ueber das ganze Fenster zerfaellt dabei nichts
//! Messbares, und jede Darstellung rundet ihn auf glatt eins. Eng wird
//! es bei `g = 1 - 1/N`, wo beide Faktoren gleich gross sind.
//!
//! 📌 **Ein Extremwert ist nicht automatisch der schlimmste Fall.**
//! Welcher es ist, sagt die Fehlerformel, nicht die Anschauung.
//!
//! 📌 **Und eine naheliegende Abkuerzung ist gemessen worden und falsch.**
//! Den Zerfall als Komplement `1-g` darzustellen bringt **nichts**: Beide
//! Darstellungen runden auf dieselbe absolute Schrittweite, und in `g^N`
//! geht der absolute Fehler ein, nicht der relative. **Eine
//! Umparametrisierung verschiebt Genauigkeit nur, wenn sie die Skala
//! mitverschiebt.**
//!
//! # ⛔️ Was VOR diesem Kernel geschehen muss, und im Entwurf fehlte
//!
//! Die Rekurrenz oben ist vollstaendig, **der Weg zu ihren Eingaengen
//! war es nicht**. Die Referenz tut vor dem ersten Schritt zweierlei,
//! und beides stand bis zum 2026-09-21 in keiner Zeile dieses Moduls:
//!
//! 1. ⛔️ **`q` und `k` werden auf Einheitslaenge gebracht**
//!    (`l2norm(..., eps=1e-6)`), je Kopf und je Token.
//! 2. **`q` wird mit `1/sqrt(kopf_dim)` skaliert**, wie bei der
//!    Aufmerksamkeit.
//!
//! ⚑ **Das ist keine Feinheit, sondern aendert den Wertebereich.** Mit
//! normiertem `k` ist `Summe(S * k)` eine Projektion auf einen
//! Einheitsvektor; ohne die Normierung haengt sie an der Laenge von `k`,
//! und die Breitenmessung haette einen anderen Gegenstand gemessen.
//!
//! ⚑ **Der Baustein dafuer ist da und nur nicht angeschlossen:** Die
//! RMSNorm rechnet ihre Kehrwurzel bereits ganzzahlig
//! (`fixed_point::inv_sqrt_q15`, `rsqrt_lut`).
//!
//! 📌 **Eine Formel aus einem Aufsatz ist nicht die Schicht.** Die
//! Rekurrenz stand richtig da, seit es dieses Modul gibt; was fehlte,
//! war das, was die Referenz **davor** tut. Wer eine fremde Schicht
//! nachbaut, liest ihren ganzen Vorwaertspass und nicht nur die
//! Gleichung, die den Namen traegt.
//!
//! # ⚠️ Was hier NICHT steht
//!
//! **Die Nichtlinearitaeten, aus denen `g` und `beta` entstehen**
//! (`sigmoid` und `softplus`), und die Formaterweiterung in θ_v. Beides
//! gehoert in denselben Zug wie eine Versionsanhebung, denn der Lader
//! vergleicht die θ_v-Version eines Artefakts zeichengenau mit der
//! eingebetteten Spezifikation und lehnt bei Abweichung ab. **Ein
//! Abschnitt, den noch kein Kernel liest, entwertet alle vorhandenen
//! Artefakte fuer nichts.**
//!
//! Dieser Kernel nimmt `g` und `beta` deshalb **fertig entgegen**.

use crate::fixed_point::{rshift_round_i128, rshift_round_i64};

/// Bruchbits des Zustands. Siehe Modulkopf: gemessen, nicht gewaehlt.
pub const ZUSTAND_FRAC: u32 = 28;
/// Bruchbits des Zerfalls `g`. Siehe Modulkopf: gemessen, nicht gewaehlt.
pub const ZERFALL_FRAC: u32 = 28;
/// Bits der groessten Kontextlaenge, die dieser Zerfall tragen muss.
///
/// ⚑ `2^18 = 262 144`, das volle Fenster des grossen Modells. **Die
/// Zahl steht hier und nicht in einem Kommentar**, weil die
/// Bauzusicherung darunter mit ihr rechnet: Wer das Fenster vergroessert,
/// bekommt einen Uebersetzungsfehler statt einer stillen Drift.
pub const KONTEXT_BITS: u32 = 18;
/// Bruchbits von `v` und `beta`, wie im uebrigen Rechenpfad.
pub const WERT_FRAC: u32 = 8;
/// Bruchbits des **normierten** `q` und `k`. Groesser als [`WERT_FRAC`].
///
/// ⛔️ **Normieren schrumpft jede Komponente um `sqrt(kopf_dim)`**, und
/// damit reicht die uebliche Wertauflösung nicht mehr. Ein Einheits-
/// vektor ueber 128 Stellen hat typische Komponenten um 0,088; bei
/// `2^-8` sind das 22 Einheiten, also rund 4 Prozent Auflösung je
/// Komponente.
///
/// ⚑ **Gemessen am 2026-09-21**, gegen die Rekurrenz in doppelter
/// Breite, bei 32 Schritten:
///
/// | Bruchbits | groesster Abstand |
/// |---|---|
/// | 8 | 1,90 |
/// | 10 | 0,76 |
/// | **12** | **0,55** |
/// | 14, 16 | 0,55 |
///
/// Ab zwoelf saettigt der Abstand; was dann bleibt, kommt aus Zustand
/// und Zerfall. 📌 **Die Zahl deckt sich mit dem Grund:**
/// `log2(sqrt(128))` sind 3,5 Stellen, also `8 + 3,5` aufgerundet.
pub const NORM_FRAC: u32 = 12;
/// Bruchbits der **Ausgabe** der Rekurrenz.
///
/// ⛔️ **Hier stand bis zum 2026-09-22 `WERT_FRAC`, also 8, und das war
/// eine gesetzte statt einer gerechneten Zahl.** Gemessen am echten
/// Modell lieferte der Schritt dann `max 1` bei **4088 Nullen von
/// 4096**: Der Ausgang trug praktisch keine Information mehr, und die
/// torgesteuerte Norm dahinter normierte Rauschen.
///
/// ⚑ **Warum er so klein ist, folgt aus der Bauart.** Der Ausgang ist
/// `(k . q) * v * beta`, und `q` wurde zuvor durch `sqrt(kopf_dim)`
/// geteilt; bei 128 Stellen ist `k . q` damit hoechstens `0,088`. Mal
/// `v` (rund 0,43) und `beta` (rund 0,5) bleiben **etwa 0,02**, also
/// fuenf Einheiten von `2^-8`, und die meisten Koepfe darunter.
///
/// 📌 **Eine Ausgabeskala folgt aus dem Wertebereich, nicht aus der
/// Nachbarschaft.** Die Breiten von Zustand und Zerfall sind gemessen;
/// diese war gesetzt, weil `v` und `beta` daneben `WERT_FRAC` tragen.
///
/// ⚑ **14 statt 8**, mit Reserve: Bei `0,02` sind das 328 Einheiten
/// statt fuenf, und selbst ein Ausgang von 1,0 bliebe mit 16 384 weit
/// innerhalb von `i16`.
pub const REKURRENZ_AUS_FRAC: u32 = 18;
/// Bruchbits **innerhalb** der Rekurrenz, fuer `kv` und `delta`.
///
/// ⛔️ **Sie lagen bis zum 2026-09-22 auf [`WERT_FRAC`], und das war die
/// groessere Haelfte des Fehlers.** `kv = Summe(S * k)` hat dieselbe
/// Kleinheit wie der Ausgang; auf acht Bruchbits gerundet verliert
/// `delta = (v - kv) * beta` seinen Vorhersagefehler, und **genau der
/// ist die Delta-Regel**. Ohne ihn wird aus der Rekurrenz ein
/// gleitender Mittelwert.
///
/// ⚑ **Gemessen am echten Modell ueber vier Token:** Mit acht
/// Bruchbits lag der relative Fehler des Rekurrenzausgangs bei 0,344,
/// waehrend die reine Ausgabequantisierung nur 0,012 erklaerte. Die
/// Differenz sass hier.
///
/// 📌 **Eine Zwischengroesse braucht ihre eigene Aufloesung.** `v` und
/// `beta` tragen `WERT_FRAC`, weil sie Aktivierungen sind; `kv` und
/// `delta` sind es nicht, sie sind Rechenzwischenstaende.
pub const INTERN_FRAC: u32 = 16;

// ⛔️ **Die Breiten passen zueinander, und das entscheidet der Uebersetzer
// und nicht eine Probe.**
//
// ⚑ **Warum als Bauzusicherung und nicht als Test** (2026-09-21): Ein
// Test faellt, wenn ihn jemand laufen laesst; eine falsche Breite soll
// aber gar nicht erst uebersetzen. Wer `ZUSTAND_FRAC` senkt, bekommt
// hier einen Fehler und nicht spaeter ein Artefakt mit stiller
// Rundung in der Fortschreibung.
const _: () = assert!(
    ZUSTAND_FRAC >= NORM_FRAC + INTERN_FRAC,
    "ZUSTAND_FRAC unter NORM_FRAC+INTERN_FRAC: Schritt 4 wuerde runden statt links zu schieben",
);
// ⚑ Und die Ausgabe darf nicht feiner sein, als der Zustand hergibt.
const _: () = assert!(
    ZUSTAND_FRAC + NORM_FRAC >= REKURRENZ_AUS_FRAC,
    "REKURRENZ_AUS_FRAC groesser als der Zustand hergibt",
);
// ⛔️ **Hier stand bis zum 2026-09-21 `ZERFALL_FRAC > ZUSTAND_FRAC`**,
// begruendet damit, der Zerfall gehoere breiter als der Zustand, den er
// bewegt. Das ist keine Schranke, sondern ein Vergleich zweier Groessen,
// die nichts miteinander zu tun haben.
//
// ⚑ **Die Schranke, die wirklich gilt**, kommt aus der Fehlerformel:
// Der Fehler des Zerfalls geht mit `eps * min(N, 1/(1-g))` in das
// Produkt ueber die Folge ein, und im engsten Punkt ist dieser Faktor
// `N`. Damit er unter einer Wertstelle bleibt, braucht `g` mindestens
// `log2(N) + WERT_FRAC - 1` Bruchbits.
const _: () = assert!(
    ZERFALL_FRAC >= KONTEXT_BITS + WERT_FRAC - 1,
    "ZERFALL_FRAC zu knapp fuer das volle Kontextfenster: der Fehler in g^N \
     ueberschreitet eine Wertstelle",
);

/// **Der Zustand eines Kopfes**, `schluessel_dim` mal `wert_dim`.
///
/// ⚑ **Zeilenweise abgelegt** (`a` aussen, `b` innen), weil beide
/// Kontraktionen ueber `a` laufen und der Rang-1-Zuschlag eine ganze
/// Zeile auf einmal anfasst.
///
/// ⚠️ **`i64` und nicht `i32`.** Bei 28 Bruchbits blieben in `i32` nur
/// drei Bit Vorkomma, und der Zustand laeuft im Gleichgewicht auf rund
/// `1/(1-g)` mal die Werteskala, bei den gemessenen Zerfaellen also auf
/// etwa 3650. Das braucht zwoelf Bit Vorkomma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zustand {
    pub schluessel_dim: usize,
    pub wert_dim: usize,
    werte: Vec<i64>,
}

impl Zustand {
    /// Ein leerer Zustand, wie er am Anfang einer Folge steht.
    ///
    /// ⚑ **Null und nicht etwas Kleines.** Ein Anfangswert ungleich null
    /// waere eine Annahme ueber den Kontext, den es noch nicht gibt.
    pub fn leer(schluessel_dim: usize, wert_dim: usize) -> Self {
        Self { schluessel_dim, wert_dim, werte: vec![0; schluessel_dim * wert_dim] }
    }

    #[inline]
    fn index(&self, a: usize, b: usize) -> usize {
        a * self.wert_dim + b
    }

    /// Ein einzelner Eintrag, in Einheiten von `2^-ZUSTAND_FRAC`.
    #[inline]
    pub fn get(&self, a: usize, b: usize) -> i64 {
        self.werte[self.index(a, b)]
    }

    /// Setzt einen Eintrag. Fuer Proben und fuer das Wiederherstellen
    /// eines Zustands nach einer Uebernahme.
    #[inline]
    /// **Alles auf null, wie am Anfang einer Folge.**
    ///
    /// ⚑ **Ohne neu zu belegen.** Eine neue Folge soll keinen
    /// Speicherbedarf erzeugen, den die vorige schon hatte; bei 128 KiB
    /// je Kopf und 32 Koepfen je Ebene ist das kein Nebenposten.
    pub fn leeren(&mut self) {
        self.werte.iter_mut().for_each(|w| *w = 0);
    }

    pub fn set(&mut self, a: usize, b: usize, wert: i64) {
        let i = self.index(a, b);
        self.werte[i] = wert;
    }

    /// Die Zahl der Werte, fuer Speicherrechnungen.
    pub fn len(&self) -> usize {
        self.werte.len()
    }

    /// Ob der Zustand leer ist (Laenge null, nicht: alle Werte null).
    pub fn is_empty(&self) -> bool {
        self.werte.is_empty()
    }
}

/// **Ein Schritt der Rekurrenz**, ein Token, ein Kopf.
///
/// ⚑ **`q` und `k` kommen NORMIERT und in `2^-NORM_FRAC`**, `v` und
/// `beta` in `2^-WERT_FRAC`, `g` in `2^-ZERFALL_FRAC`. Die Normierung
/// geschieht vor diesem Kernel, denn sie laeuft ueber den ganzen Kopf
/// und nicht ueber einen Schritt. Die Ausgabe kommt in `2^-WERT_FRAC`
/// zurueck, also in derselben Skala wie die Eingabe.
///
/// # ⚑ Die Schiebeweiten, und warum eine davon nach links geht
///
/// ```text
/// 1. Zerfall:   S * g           >> ZERFALL_FRAC
/// 2. Lesen:     Summe S * k     >> ZUSTAND_FRAC + NORM_FRAC - WERT_FRAC
/// 3. Korrektur: (v - kv) * beta >> WERT_FRAC
/// 4. Update:    k * delta       << ZUSTAND_FRAC - NORM_FRAC - WERT_FRAC
/// 5. Ausgabe:   Summe S * q     >> ZUSTAND_FRAC + NORM_FRAC - REKURRENZ_AUS_FRAC
/// ```
///
/// ⛔️ **Schritt 4 ist ein Linksshift und damit verlustfrei**, weil der
/// Zustand breiter ist als das Produkt zweier Werte. **Das ist der Grund,
/// warum die Fortschreibung nichts kostet**, und es ist die tragende
/// Erkenntnis der Breitenmessung: Gerundet wird nur am Zerfall und an den
/// zwei Kontraktionen, nicht bei jedem Zuschlag.
///
/// ⚠️ **`i128` in den Kontraktionen ist keine Vorsicht, sondern noetig.**
/// Der Zustand erreicht rund `2^41`, `k` rund `2^15`, und ueber 128
/// Summanden sind das `2^63`. In `i64` waere das der Rand.
///
/// # Panics
///
/// Wenn `q`, `k` oder `v` nicht zu den Massen des Zustands passen.
pub fn schritt(
    zustand: &mut Zustand,
    q: &[i16],
    k: &[i16],
    v: &[i16],
    g: i64,
    beta: i16,
    aus: &mut [i16],
) -> u8 {
    assert_eq!(q.len(), zustand.schluessel_dim, "q passt nicht zur Schluesseldimension");
    assert_eq!(k.len(), zustand.schluessel_dim, "k passt nicht zur Schluesseldimension");
    assert_eq!(v.len(), zustand.wert_dim, "v passt nicht zur Wertdimension");
    assert_eq!(aus.len(), zustand.wert_dim, "die Ausgabe passt nicht zur Wertdimension");

    let sd = zustand.schluessel_dim;
    let wd = zustand.wert_dim;

    // --- 1. Der Zustand verblasst.
    //
    // ⚑ **Ein Faktor unter eins ist ein Rechtsshift**, und genau hier
    // entsteht der Rundungsfehler, den die Messung vermessen hat. Er wird
    // im selben Schritt auch gedaempft: Jeder aeltere Fehler bekommt
    // denselben Faktor. Deshalb laeuft die Summe in ein Gleichgewicht
    // statt linear zu wachsen.
    for wert in zustand.werte.iter_mut() {
        if *wert != 0 {
            *wert = rshift_round_i128(i128::from(*wert) * i128::from(g), ZERFALL_FRAC) as i64;
        }
    }

    // --- 2. Lesen mit dem Schluessel, Kontraktion ueber a.
    let mut kv_mem = vec![0i32; wd];
    for (b, ziel) in kv_mem.iter_mut().enumerate() {
        let mut akku: i128 = 0;
        for (a, &k_a) in k.iter().enumerate() {
            if k_a != 0 {
                akku += i128::from(zustand.werte[a * wd + b]) * i128::from(k_a);
            }
        }
        *ziel = rshift_round_i128(akku, ZUSTAND_FRAC + NORM_FRAC - INTERN_FRAC) as i32;
    }

    // --- 3. Die Korrektur.
    let mut delta = vec![0i32; wd];
    for (b, ziel) in delta.iter_mut().enumerate() {
        // ⛔️ **Beide Seiten muessen dieselbe Skala tragen.** `v` kommt
        //   als Aktivierung auf `WERT_FRAC`, `kv` als Rechenzwischenstand
        //   auf `INTERN_FRAC`; ohne diese Verschiebung subtrahierte man
        //   Zahlen verschiedener Bedeutung.
        let v_intern = i64::from(v[b]) << (INTERN_FRAC - WERT_FRAC);
        let roh = v_intern - i64::from(kv_mem[b]);
        *ziel = rshift_round_i64(roh * i64::from(beta), WERT_FRAC as u8) as i32;
    }

    // --- 4. Rang-1-Fortschreibung, verlustfrei.
    let links = ZUSTAND_FRAC - NORM_FRAC - INTERN_FRAC;
    for (a, &k_a) in k.iter().enumerate() {
        if k_a == 0 {
            continue;
        }
        let basis = a * wd;
        for (b, &d_b) in delta.iter().enumerate() {
            if d_b == 0 {
                continue;
            }
            zustand.werte[basis + b] += (i64::from(k_a) * i64::from(d_b)) << links;
        }
    }

    // --- 5. Lesen mit der Abfrage, mit einer Skala JE KOPF.
    //
    // # ⛔️ Fund 418: eine Skala fuer 32 Koepfe, die um 1050 auseinander
    //   liegen
    //
    // Bis zum 2026-09-22 stand hier eine feste Verschiebung auf
    // `REKURRENZ_AUS_FRAC`, dieselbe fuer jeden Kopf. Gemessen am
    // Qwen3.6-35B-A3B, Ebene 0, Token 3: Der lauteste Kopf hatte einen
    // Effektivwert von 523 Zaehlern, der leiseste **0,5** - ein
    // Verhaeltnis von 1050, also mehr als zehn Bit. In fuenfzehn Bit
    // passen beide nicht: Wer den lauten Kopf ohne Saettigung abbilden
    // will, laesst dem leisen weniger als einen Zaehler.
    //
    // ⛔️ **Und dahinter steht eine Normierung.** `torgesteuerte_norm`
    // normiert **je Kopf** auf den Effektivwert eins. Sie loescht damit
    // genau den Groessenunterschied, der die Skala gerechtfertigt hat,
    // und hebt den leisen Kopf wieder auf volle Hoehe - mitsamt seinem
    // Rundungsfehler. Gemessen: Die sechs Koepfe unter 32 Zaehlern kamen
    // hinter der Norm auf einen mittleren Fehler von **24,5**, die
    // sechsundzwanzig darueber auf 0,93. Die Verstaerkung erreichte das
    // 167-fache.
    //
    // 📌 **Eine Normierung hinter einer geteilten Skala ist ein
    // Rauschverstaerker.** Die Skala richtet sich nach dem lautesten
    // Teilnehmer, die Normierung blaest den leisesten wieder auf, und
    // was sie aufblaest, ist Rundungsfehler. Wer beides hintereinander
    // baut, muss die Skala **so fein teilen wie die Normierung**.
    //
    // ⚑ **Die Skala kommt aus dem Groesstwert des Kopfes selbst**, als
    // gerade Zweierpotenz, und wird zurueckgegeben. Das ist dasselbe
    // Verfahren, das `rmsnorm_i16` fuer den Index der Wurzeltabelle
    // benutzt (`dynamic_even_shift`), und es ist rein ganzzahlig und
    // damit auf jedem Knoten gleich.
    let mut akkus = vec![0i128; wd];
    for (b, ziel) in akkus.iter_mut().enumerate() {
        let mut akku: i128 = 0;
        for (a, &q_a) in q.iter().enumerate() {
            if q_a != 0 {
                akku += i128::from(zustand.werte[a * wd + b]) * i128::from(q_a);
            }
        }
        *ziel = akku;
    }

    // Die Verschiebung, die den Groesstwert gerade noch in i16 legt.
    let groesst = akkus.iter().map(|a| a.unsigned_abs()).max().unwrap_or(0);
    let bits = 128 - groesst.leading_zeros();
    // ⚠️ Nach oben begrenzt, damit `aus_frac` nicht unter null faellt;
    //    darueber bleibt es bei der ausdruecklichen Saettigung.
    let mut schiebung = bits.saturating_sub(15).min(ZUSTAND_FRAC + NORM_FRAC);
    // ⚠️ **Die Rundung kann den Groesstwert noch ueber die Grenze
    //    heben**: `groesst >> schiebung` bleibt unter 2^15, aber ein
    //    Rest ab der Haelfte rundet auf 32768 auf. Dann ein Bit mehr,
    //    statt ausgerechnet den lautesten Wert zu saettigen.
    if schiebung < ZUSTAND_FRAC + NORM_FRAC
        && rshift_round_i128(groesst as i128, schiebung) > i128::from(i16::MAX)
    {
        schiebung += 1;
    }
    let aus_frac = ZUSTAND_FRAC + NORM_FRAC - schiebung;

    for (ziel, &akku) in aus.iter_mut().zip(akkus.iter()) {
        let wert = rshift_round_i128(akku, schiebung);
        // ⚠️ **Gesaettigt und nicht abgeschnitten.** Ein Abschneiden
        // dreht das Vorzeichen, und ein Vorzeichenwechsel im
        // Rechenpfad ist ein Konsensbruch ohne Meldung.
        *ziel = wert.clamp(i128::from(i16::MIN), i128::from(i16::MAX)) as i16;
    }
    let _ = sd;
    u8::try_from(aus_frac).expect("aus_frac passt in u8")
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Ein Zustand, der nie beschrieben wurde, liefert null.
    #[test]
    fn ein_leerer_zustand_antwortet_mit_null() {
        let mut z = Zustand::leer(4, 4);
        let mut aus = [0i16; 4];
        let eins_g = 1i64 << ZERFALL_FRAC;
        schritt(&mut z, &[1, 2, 3, 4], &[0, 0, 0, 0], &[0, 0, 0, 0], eins_g, 0, &mut aus);
        assert_eq!(aus, [0, 0, 0, 0]);
    }

    /// **Der erste Schritt schreibt v in den Zustand, wenn beta voll ist.**
    ///
    /// ⚑ Mit leerem Zustand ist `kv_mem` null, also `delta = v * beta`.
    /// Bei `beta = 1` und `k` als Einheitsvektor steht danach `v` in der
    /// Zeile von `k`, und eine Abfrage mit demselben Einheitsvektor gibt
    /// es zurueck. **Das ist die Rekurrenz an ihrem einfachsten Fall**,
    /// und wenn der nicht stimmt, stimmt nichts.
    #[test]
    fn ein_schritt_legt_den_wert_ab_und_gibt_ihn_zurueck() {
        let mut z = Zustand::leer(4, 4);
        let mut aus = [0i16; 4];
        let eins_w = 1i16 << WERT_FRAC;
        let eins_g = 1i64 << ZERFALL_FRAC;
        // ⚑ **Der Einheitsschluessel liegt in NORM_FRAC**, denn `q` und
        //   `k` kommen normiert herein und nicht in der Wertauflösung.
        let eins_n = 1i16 << NORM_FRAC;
        let k = [eins_n, 0, 0, 0];
        let v = [10, 20, 30, 40];
        let aus_frac = schritt(&mut z, &k, &k, &v, eins_g, eins_w, &mut aus);
        // ⚑ **Die Ausgabe traegt seit Fund 418 eine Skala je Kopf**, und
        //   `schritt` gibt sie zurueck. Verglichen wird deshalb der
        //   **Wert**, umgerechnet auf die gemeldete Skala, und nicht die
        //   Zahl. 📌 Eine Probe, die eine feste Skala annimmt, prueft
        //   die Skala und nicht die Rechnung.
        let erwartet: Vec<i16> = v
            .iter()
            .map(|&x| {
                let f = i64::from(x) << (aus_frac - WERT_FRAC as u8);
                f.clamp(-32768, 32767) as i16
            })
            .collect();
        assert_eq!(aus.to_vec(), erwartet, "was hineingeschrieben wurde, kommt zurueck");
        // ⚑ **Und die Skala fuellt i16 aus.** Ohne diese Zeile bliebe die
        //   Probe auch dann gruen, wenn `schritt` eine viel zu grobe
        //   Skala waehlte, denn der Vergleich rechnet sie ja mit.
        let groesst = aus.iter().map(|x| x.unsigned_abs()).max().unwrap();
        assert!(groesst > 1 << 13, "die Skala laesst i16 halb leer: {groesst}");
    }

    /// ⛔️ **Der Zerfall wirkt, und zwar messbar.**
    ///
    /// Gegenprobe zur Probe darueber: Mit halbem Zerfall muss die zweite
    /// Abfrage ungefaehr die Haelfte liefern. Ohne diese Probe bliebe die
    /// Rekurrenz auch dann gruen, wenn Schritt 1 fehlte.
    #[test]
    fn der_zerfall_halbiert_den_zustand() {
        let mut z = Zustand::leer(2, 2);
        let mut aus = [0i16; 2];
        let eins_w = 1i16 << WERT_FRAC;
        let eins_g = 1i64 << ZERFALL_FRAC;
        let halb_g = eins_g / 2;
        let k = [1i16 << NORM_FRAC, 0];
        // Ablegen, ohne Zerfall.
        // ⚑ Kleine Werte, damit `v * 2^10` in i16 bleibt.
        let kl = |x: i64, frac: u8| (x << (frac - WERT_FRAC as u8)).clamp(-32768, 32767) as i16;
        let f1 = schritt(&mut z, &k, &k, &[10, 20], eins_g, eins_w, &mut aus);
        assert_eq!(aus, [kl(10, f1), kl(20, f1)]);
        // Nur zerfallen lassen: k = 0, also kein neuer Zuschlag.
        let f2 = schritt(&mut z, &k, &[0, 0], &[0, 0], halb_g, 0, &mut aus);
        assert_eq!(aus, [kl(5, f2), kl(10, f2)], "der Zustand ist halbiert");
        // ⛔️ **Die Halbierung muss in der SKALA sichtbar sein, nicht nur
        //    in der Zahl.** Weil `schritt` je Kopf neu skaliert, liefert
        //    ein halbierter Zustand dieselben Zahlen mit einer um eins
        //    feineren Skala. Ohne diese Zeile bliebe die Probe gruen,
        //    wenn der Zerfall gar nicht wirkte.
        assert_eq!(f2, f1 + 1, "der halbierte Zustand braucht ein Bit mehr Skala");
    }

    /// **Zweimal derselbe Lauf ergibt denselben Zustand, Bit fuer Bit.**
    ///
    /// ⚑ **Das ist die Zusage, um die es geht.** Sie ist hier trivial zu
    /// erfuellen und steht trotzdem da: Sie faellt, sobald jemand eine
    /// Gleitkommazahl oder eine ungeordnete Summe einfuehrt.
    #[test]
    fn zwei_laeufe_ergeben_denselben_zustand() {
        let bauen = || {
            let mut z = Zustand::leer(8, 8);
            let mut aus = [0i16; 8];
            let g = (1i64 << ZERFALL_FRAC) - (1i64 << (ZERFALL_FRAC - 10));
            for t in 0..64i16 {
                let q: Vec<i16> = (0..8).map(|i| (t * 3 + i * 7) % 251 - 125).collect();
                let k: Vec<i16> = (0..8).map(|i| (t * 5 + i * 11) % 251 - 125).collect();
                let v: Vec<i16> = (0..8).map(|i| (t * 7 + i * 13) % 251 - 125).collect();
                schritt(&mut z, &q, &k, &v, g, 200, &mut aus);
            }
            z
        };
        assert_eq!(bauen(), bauen());
    }

    /// ⛔️ **Die Rang-1-Fortschreibung ist verlustfrei**, und das ist
    /// nachzurechnen statt zu glauben.
    ///
    /// Bei leerem Zustand, `beta = 1` und einem Einheitsschluessel ist
    /// der Zuschlag genau `v`, in Zustandsskala. Steht dort ein anderer
    /// Wert, hat Schritt 4 gerundet, und dann ist die Breite falsch
    /// gewaehlt.
    #[test]
    fn die_fortschreibung_rundet_nicht() {
        let mut z = Zustand::leer(2, 2);
        let mut aus = [0i16; 2];
        let eins_w = 1i16 << WERT_FRAC;
        let eins_g = 1i64 << ZERFALL_FRAC;
        // v = 1 in der letzten Stelle, also der kleinste darstellbare Wert.
        let eins_n = 1i16 << NORM_FRAC;
        schritt(&mut z, &[eins_n, 0], &[eins_n, 0], &[1, 0], eins_g, eins_w, &mut aus);
        // Erwartung gebaut und nicht getippt: Der Zuschlag ist
        // `k * delta << (ZUSTAND_FRAC - NORM_FRAC - WERT_FRAC)` mit
        // `k = 2^NORM_FRAC` und `delta = 1`, also bleibt es bei
        // `2^(ZUSTAND_FRAC - WERT_FRAC)`: Die Normierungsstellen kuerzen
        // sich gegen die Linksschiebung heraus.
        let soll = 1i64 << (ZUSTAND_FRAC - WERT_FRAC);
        assert_eq!(z.get(0, 0), soll, "die kleinste Stelle ist verlorengegangen");
    }

    /// ⚠️ **Eine Ausgabe ausserhalb von `i16` saettigt, sie kippt nicht.**
    ///
    /// 📌 Ein Abschneiden dreht das Vorzeichen, und ein Vorzeichenwechsel
    /// im Rechenpfad ist ein Konsensbruch ohne Meldung.
    #[test]
    fn die_ausgabe_saettigt_statt_zu_kippen() {
        let mut z = Zustand::leer(1, 1);
        // Den Zustand von Hand so gross setzen, dass die Abfrage ueberlaeuft.
        z.set(0, 0, i64::from(i16::MAX) << (ZUSTAND_FRAC + 4));
        let mut aus = [0i16; 1];
        let eins_w = 1i16 << WERT_FRAC;
        let eins_g = 1i64 << ZERFALL_FRAC;
        schritt(&mut z, &[eins_w], &[0], &[0], eins_g, 0, &mut aus);
        assert_eq!(aus[0], i16::MAX, "gesaettigt");
    }

}
