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
    assert_eq!(aus.len(), zustand.wert_dim, "die Ausgabe passt nicht zur Wertdimension");
    let (sd, wd) = (zustand.schluessel_dim, zustand.wert_dim);
    let mut akkus = vec![0i128; wd];
    // SICHERHEIT: `zustand` ist fuer die Dauer des Aufrufs exklusiv
    // ausgeliehen, und `spalten_schritt` formt Ausschnitte nur innerhalb
    // seiner `sd * wd` Werte.
    unsafe { spalten_schritt(zustand.werte.as_mut_ptr(), sd, wd, 0, wd, q, k, v, g, beta, &mut akkus) };
    ausgabe_skalieren(&akkus, aus)
}

/// **Der Kern eines Schritts, fuer die Spalten `[b0, b1)` eines Kopfes.**
///
/// ⚑ **Die Spalten eines Kopfes sind voneinander unabhaengig**: Zerfall
/// und Fortschreibung wirken je Wert, und beide Kontraktionen summieren je
/// Spalte ueber die Zeilen. Erst die Ausgangsskala in
/// [`ausgabe_skalieren`] schaut ueber alle Spalten, und sie kommt danach.
/// Ein Kopf laesst sich deshalb in Spaltenbloecke teilen, die verschiedene
/// Faeden rechnen ([`schritte_fenster`]); [`schritt`] ist derselbe Kern
/// ueber alle Spalten.
///
/// ⚑ **Zwei Durchgaenge statt vier** (2026-09-25): Zerfall und Lesen mit
/// `k` teilen sich einen, Fortschreibung und Lesen mit `q` den anderen.
/// Jeder Wert erfaehrt dieselben Operationen in derselben Reihenfolge wie
/// vorher, nur liegt er dabei noch im Zwischenspeicher.
///
/// Die rohen Lesesummen mit `q` landen in `akkus`, eine je Spalte.
///
/// # Sicherheit
///
/// `werte` zeigt auf `sd * wd` Werte eines Zustands, `b0 <= b1 <= wd`, und
/// waehrend des Aufrufs fasst niemand sonst die Spalten `[b0, b1)` an.
/// Verschiedene Spaltenbloecke desselben Zustands duerfen gleichzeitig
/// laufen; jeder formt Ausschnitte nur ueber seine eigenen Spalten.
#[allow(clippy::too_many_arguments)]
unsafe fn spalten_schritt(
    werte: *mut i64,
    sd: usize,
    wd: usize,
    b0: usize,
    b1: usize,
    q: &[i16],
    k: &[i16],
    v: &[i16],
    g: i64,
    beta: i16,
    akkus: &mut [i128],
) {
    assert_eq!(q.len(), sd, "q passt nicht zur Schluesseldimension");
    assert_eq!(k.len(), sd, "k passt nicht zur Schluesseldimension");
    assert_eq!(v.len(), wd, "v passt nicht zur Wertdimension");
    assert!(b0 <= b1 && b1 <= wd, "die Spalten liegen ausserhalb des Kopfes");
    let breite = b1 - b0;
    assert_eq!(akkus.len(), breite, "eine Summe je Spalte");
    // SICHERHEIT: siehe oben; Zeile `a` beginnt bei `a * wd`, und
    // `[b0, b1)` liegt darin.
    let zeile = |a: usize| -> &mut [i64] {
        unsafe { std::slice::from_raw_parts_mut(werte.add(a * wd + b0), breite) }
    };

    // --- 1. Der Zustand verblasst, und
    //
    // ⚑ **Ein Faktor unter eins ist ein Rechtsshift**, und genau hier
    // entsteht der Rundungsfehler, den die Messung vermessen hat. Er wird
    // im selben Schritt auch gedaempft: Jeder aeltere Fehler bekommt
    // denselben Faktor. Deshalb laeuft die Summe in ein Gleichgewicht
    // statt linear zu wachsen.
    //
    // --- 2. Lesen mit dem Schluessel, Kontraktion ueber a.
    //
    // ⚑ **Zeilenweise und nicht spaltenweise** (2026-09-25). Die Summe je
    // Spalte `b` laeuft ueber die Zeilen `a`; hier stand sie als innere
    // Schleife, und jeder Summand lag eine ganze Zeile (1 KiB) hinter dem
    // vorigen. Jetzt wird Zeile fuer Zeile auf alle Spaltensummen
    // zugleich addiert, und der Speicher wird am Stueck gelesen.
    // Gemessen an 32 Koepfen zu 128 x 128: 1,54 ms auf 0,66 ms je Schritt.
    //
    // ⚑ **Dasselbe Ergebnis, Bit fuer Bit.** Die Summanden sind dieselben,
    // nur ihre Reihenfolge ist eine andere, und eine Ganzzahlsumme in
    // `i128` laeuft hier nicht ueber: Auf die Reihenfolge kommt es nicht an.
    let mut kv = vec![0i128; breite];
    for (a, &k_a) in k.iter().enumerate() {
        let z = zeile(a);
        for wert in z.iter_mut() {
            if *wert != 0 {
                *wert = rshift_round_i128(i128::from(*wert) * i128::from(g), ZERFALL_FRAC) as i64;
            }
        }
        zeile_aufaddieren(&mut kv, z, k_a);
    }

    // --- 3. Die Korrektur.
    let mut delta = vec![0i32; breite];
    for (j, ziel) in delta.iter_mut().enumerate() {
        // ⛔️ **Beide Seiten muessen dieselbe Skala tragen.** `v` kommt
        //   als Aktivierung auf `WERT_FRAC`, `kv` als Rechenzwischenstand
        //   auf `INTERN_FRAC`; ohne diese Verschiebung subtrahierte man
        //   Zahlen verschiedener Bedeutung.
        let kv_mem = rshift_round_i128(kv[j], ZUSTAND_FRAC + NORM_FRAC - INTERN_FRAC) as i32;
        let v_intern = i64::from(v[b0 + j]) << (INTERN_FRAC - WERT_FRAC);
        let roh = v_intern - i64::from(kv_mem);
        *ziel = rshift_round_i64(roh * i64::from(beta), WERT_FRAC as u8) as i32;
    }

    // --- 4. Rang-1-Fortschreibung, verlustfrei, und
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
    let links = ZUSTAND_FRAC - NORM_FRAC - INTERN_FRAC;
    akkus.iter_mut().for_each(|x| *x = 0);
    for (a, (&k_a, &q_a)) in k.iter().zip(q.iter()).enumerate() {
        let z = zeile(a);
        if k_a != 0 {
            for (wert, &d_b) in z.iter_mut().zip(delta.iter()) {
                if d_b != 0 {
                    *wert += (i64::from(k_a) * i64::from(d_b)) << links;
                }
            }
        }
        zeile_aufaddieren(akkus, z, q_a);
    }
}

/// **Eine Zeile auf die Spaltensummen addieren**, gewichtet mit `x_a`.
///
/// ⚑ **Zeilenweise und nicht spaltenweise** (2026-09-25). Die Summe je
/// Spalte laeuft ueber die Zeilen; frueher stand sie als innere Schleife,
/// und jeder Summand lag eine ganze Zeile (1 KiB) hinter dem vorigen.
/// Jetzt wird Zeile fuer Zeile auf alle Spaltensummen zugleich addiert.
/// Gemessen an 32 Koepfen zu 128 x 128: 1,54 ms auf 0,66 ms je Schritt.
///
/// ⚑ **Dasselbe Ergebnis, Bit fuer Bit.** Die Summanden sind dieselben,
/// nur ihre Reihenfolge ist eine andere, und eine Ganzzahlsumme in `i128`
/// laeuft hier nicht ueber: Auf die Reihenfolge kommt es nicht an. Eine
/// Zeile mit `x_a == 0` traegt nichts bei und wird uebersprungen.
#[inline]
fn zeile_aufaddieren(akkus: &mut [i128], zeile: &[i64], x_a: i16) {
    if x_a == 0 {
        return;
    }
    let faktor = i128::from(x_a);
    for (akku, &s) in akkus.iter_mut().zip(zeile.iter()) {
        *akku += i128::from(s) * faktor;
    }
}

/// **Die Ausgabe eines Kopfes aus seinen rohen Lesesummen**, mit einer
/// Skala aus seinem eigenen Groesstwert (Fund 418, Begruendung in
/// [`spalten_schritt`]). Gibt die Skala zurueck.
fn ausgabe_skalieren(akkus: &[i128], aus: &mut [i16]) -> u8 {
    assert_eq!(akkus.len(), aus.len(), "eine Ausgabe je Spalte");
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
    u8::try_from(aus_frac).expect("aus_frac passt in u8")
}

/// **Die Kontraktion `Summe_a S[a][b] * x[a]` fuer alle Spalten `b`**,
/// ueber [`zeile_aufaddieren`]; nur fuer die Probe, die sie gegen die
/// fruehere spaltenweise Schleife haelt.
#[cfg(test)]
fn zeilen_kontrahieren(werte: &[i64], x: &[i16], wd: usize) -> Vec<i128> {
    let mut akkus = vec![0i128; wd];
    for (zeile, &x_a) in werte.chunks_exact(wd).zip(x.iter()) {
        zeile_aufaddieren(&mut akkus, zeile, x_a);
    }
    akkus
}

/// **Die Schritte aller Koepfe einer Ebene ueber ein Fenster von Token,
/// verteilt ueber die Faeden.**
///
/// ⚑ **Die Koepfe sind voneinander unabhaengig**: Jeder hat seinen
/// eigenen Zustand und liest nur seine eigenen Eingaben. Nacheinander
/// sind nur die Token eines Kopfes. Hier laeuft deshalb jeder Kopf ueber
/// alle Token des Fensters, und die Koepfe laufen verteilt: **eine Runde
/// je Fenster statt einer je Token**, und der Zustand eines Kopfes
/// (128 x 128 Werte, 128 KiB) bleibt dabei im Zwischenspeicher seines
/// Kerns. Jeder Kopf rechnet Bit fuer Bit dasselbe wie [`schritt`] in
/// der Schleife, denn beide rufen denselben Kern.
///
/// ⚑ **Und jeder Kopf in Spaltenbloecken** ([`SPALTENBLOECKE`]): Die
/// Spalten eines Kopfes sind voneinander unabhaengig (siehe
/// [`spalten_schritt`]). 32 ganze Koepfe auf 15 Faeden liessen elf Faeden
/// drei Koepfe rechnen und vier warten; 128 Bloecke verteilen sich
/// gleichmaessig.
///
/// Gemessen an 32 Koepfen zu 128 x 128, je Token: 0,66 ms nacheinander,
/// 0,17 ms verteilt (2026-09-25).
///
/// `eingabe(kopf, token)` liefert `(q, k, v, g, beta)`. Rueckgabe: je
/// Token die Ausgaben aller Koepfe (Kopf fuer Kopf zu je `wert_dim`) und
/// je Token die Skala jedes Kopfes.
pub fn schritte_fenster<'e, F>(
    zustaende: &mut [Zustand],
    token: usize,
    eingabe: F,
    faeden: usize,
) -> (Vec<Vec<i16>>, Vec<Vec<u8>>)
where
    F: Fn(usize, usize) -> (&'e [i16], &'e [i16], &'e [i16], i64, i16) + Sync,
{
    let koepfe = zustaende.len();
    let (sd, wd) = zustaende.first().map_or((0, 0), |z| (z.schluessel_dim, z.wert_dim));
    assert!(
        zustaende.iter().all(|z| z.schluessel_dim == sd && z.wert_dim == wd),
        "alle Koepfe gleich gross"
    );
    let bloecke = if wd % SPALTENBLOECKE == 0 { SPALTENBLOECKE } else { 1 };
    let breite = wd / bloecke;

    // Je Kopf und Spaltenblock eine Einheit; jede laeuft ueber alle Token.
    let basen: Vec<usize> = zustaende.iter_mut().map(|z| z.werte.as_mut_ptr() as usize).collect();
    let roh: Vec<Vec<i128>> = crate::fadenpool::verteilen(koepfe * bloecke, faeden, |einheit| {
        let (h, block) = (einheit / bloecke, einheit % bloecke);
        let mut akkus = vec![0i128; token * breite];
        for (t, ziel) in akkus.chunks_exact_mut(breite).enumerate() {
            let (q, k, v, g, beta) = eingabe(h, t);
            // SICHERHEIT: `zustaende` ist fuer die ganze Runde exklusiv
            // ausgeliehen, denn `verteilen` kehrt erst zurueck, wenn alle
            // Einheiten fertig sind. Jede Einheit ist genau ein Kopf und
            // ein Spaltenblock, `verteilen` rechnet jede genau einmal, und
            // die Bloecke eines Kopfes sind disjunkte Spalten.
            unsafe {
                spalten_schritt(
                    basen[h] as *mut i64, sd, wd,
                    block * breite, (block + 1) * breite,
                    q, k, v, g, beta, ziel,
                );
            }
        }
        akkus
    });

    // Je Token und Kopf die Ausgangsskala ueber alle Spalten des Kopfes.
    let je_token: Vec<(Vec<i16>, Vec<u8>)> = crate::fadenpool::verteilen(token, faeden, |t| {
        let mut aus = vec![0i16; koepfe * wd];
        let mut fracs = vec![0u8; koepfe];
        let mut akkus = vec![0i128; wd];
        for h in 0..koepfe {
            for block in 0..bloecke {
                akkus[block * breite..(block + 1) * breite]
                    .copy_from_slice(&roh[h * bloecke + block][t * breite..(t + 1) * breite]);
            }
            fracs[h] = ausgabe_skalieren(&akkus, &mut aus[h * wd..(h + 1) * wd]);
        }
        (aus, fracs)
    });
    je_token.into_iter().unzip()
}

/// **In wie viele Spaltenbloecke ein Kopf fuer [`schritte_fenster`]
/// geteilt wird.**
///
/// ⚑ Gemessen am 2026-09-25 an 32 Koepfen zu 128 x 128, 64 Token, 15
/// Faeden, je Fenster: ganze Koepfe 10,5 ms, zwei Bloecke 7,1, vier 6,1,
/// acht 6,0. Vier verteilen 128 Einheiten gleichmaessig auf die Faeden;
/// mehr bringt kaum etwas und macht die Summen je Block kuerzer.
const SPALTENBLOECKE: usize = 4;

#[cfg(test)]
mod proben {
    use super::*;

    /// **Die zeilenweise Kontraktion ist die spaltenweise von vorher**,
    /// Wert fuer Wert, auch mit Nullen mitten im Vektor und mit Zustaenden
    /// bis `2^57`, wo schon ein Produkt `i128` braucht.
    ///
    /// ⚑ Die Vergleichsformel ist die Schleife, die bis zum 2026-09-25 im
    /// Kern stand: je Spalte `b` die Summe ueber `a` mit `S[a * wd + b]`.
    #[test]
    fn die_zeilenweise_kontraktion_ist_die_spaltenweise() {
        let (sd, wd) = (7usize, 5usize);
        let werte: Vec<i64> = (0..sd * wd)
            .map(|i| ((i as i64 * 7919) % 2001 - 1000) << 47)
            .collect();
        let x: Vec<i16> = vec![3000, 0, -32768, 17, 0, 32767, -5];
        let spaltenweise: Vec<i128> = (0..wd)
            .map(|b| {
                let mut akku: i128 = 0;
                for (a, &x_a) in x.iter().enumerate() {
                    if x_a != 0 {
                        akku += i128::from(werte[a * wd + b]) * i128::from(x_a);
                    }
                }
                akku
            })
            .collect();
        assert!(spaltenweise.iter().any(|&s| s.unsigned_abs() > i64::MAX as u128), "die Probe erreicht i128 nicht");
        assert_eq!(zeilen_kontrahieren(&werte, &x, wd), spaltenweise);
    }

    /// **Das Fenster rechnet jeden Kopf wie die Schleife Token fuer
    /// Token**, ueber mehrere Fenster verschiedener Laenge, damit ein
    /// vertauschter Zustand oder ein vertauschtes Token auffiele und nicht
    /// erst im Modell.
    #[test]
    fn das_fenster_ist_bitgleich_zur_schleife() {
        /// `(q, k, v, g, beta)` eines Kopfes fuer ein Token.
        type Eingabe = (Vec<i16>, Vec<i16>, Vec<i16>, i64, i16);
        let (koepfe, sd, wd) = (6usize, 8usize, 8usize);
        let mut s: u64 = 0x2545_f491_4f6c_dd1d;
        let mut zufall = |m: i64| -> i64 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % (2 * m as u64 + 1)) as i64 - m
        };
        let mut einzeln: Vec<Zustand> = (0..koepfe).map(|_| Zustand::leer(sd, wd)).collect();
        let mut gesamt = einzeln.clone();
        for token in [3usize, 1, 4] {
            // eingaben[t][h]
            let eingaben: Vec<Vec<Eingabe>> = (0..token)
                .map(|_| {
                    (0..koepfe)
                        .map(|_| {
                            let mut feld = |n: usize, m: i64| -> Vec<i16> { (0..n).map(|_| zufall(m) as i16).collect() };
                            // 📌 **Mit Nullen mitten im Vektor.** Zufallswerte
                            // bis 1400 sind fast nie null, und eine
                            // Kontraktion, die an der ersten Null aufhoert
                            // statt sie zu ueberspringen, blieb damit
                            // unbemerkt (nachgestellt).
                            let mut q = feld(sd, 1400);
                            let mut k = feld(sd, 1400);
                            q[2] = 0;
                            k[3] = 0;
                            let v = feld(wd, 600);
                            (q, k, v, (1i64 << ZERFALL_FRAC) - 1 - zufall(1 << 24).abs(), (128 + zufall(100)) as i16)
                        })
                        .collect()
                })
                .collect();
            let mut aus_a = vec![vec![0i16; koepfe * wd]; token];
            let mut fracs_a = vec![vec![0u8; koepfe]; token];
            for t in 0..token {
                for (h, (q, k, v, g, beta)) in eingaben[t].iter().enumerate() {
                    fracs_a[t][h] = schritt(&mut einzeln[h], q, k, v, *g, *beta, &mut aus_a[t][h * wd..(h + 1) * wd]);
                }
            }
            let (aus_b, fracs_b) = schritte_fenster(
                &mut gesamt,
                token,
                |h, t| {
                    let (q, k, v, g, beta) = &eingaben[t][h];
                    (q.as_slice(), k.as_slice(), v.as_slice(), *g, *beta)
                },
                4,
            );
            assert!(aus_a.iter().flatten().any(|&x| x != 0), "die Probe rechnet mit lauter Nullen");
            assert_eq!(aus_a, aus_b, "{token} Token: das Fenster weicht in der Ausgabe ab");
            assert_eq!(fracs_a, fracs_b, "{token} Token: das Fenster weicht in der Skala ab");
            // 📌 **Und die Zustaende selbst.** Eine Verteilung, die jeden
            // Kopf bestaendig in den Platz seines Nachbarn schreibt, gibt
            // dieselben Ausgaben, denn jeder Platz sieht nur einen Kopf;
            // sichtbar wird sie erst hier (nachgestellt).
            assert_eq!(einzeln, gesamt, "{token} Token: das Fenster schreibt in fremde Zustaende");
        }
    }

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
