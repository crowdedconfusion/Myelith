//! Der Trainingsschritt: Gewichte fortschreiben, ganzzahlig und
//! ordnungsfrei (Whitepaper Kap. 7, Anhang B.6.2).
//!
//! ## ⚑ Warum stochastisch gerundet wird, und warum das gemessen ist
//!
//! Ein SGD-Schritt bewegt ein Gewicht im Median um **6,4e-6 einer
//! Rasterstufe**. Wer zur nächsten Stufe rundet, bekommt dann entweder
//! **nichts** oder einen **ganzen Sprung**, und beides ist falsch: Die
//! kleinen Bewegungen, aus denen Lernen besteht, verschwinden.
//!
//! Gemessen an Qwen2.5-0,5B über WikiText-2:
//!
//! | Variante | zurückgehaltener Text | Abstand |
//! |---|---|---|
//! | Gleitkomma-Referenz | 3,0472 → **2,9795** | |
//! | Rundung zur nächsten Stufe | 3,0689 → 3,8713 | **+29,9 %** |
//! | **stochastisches Runden** | 3,0770 → **2,9994** | **+0,67 %** |
//!
//! **Eine einzige geänderte Zeile dreht das Ergebnis.** Deshalb steht
//! sie hier mit dieser Begründung und nicht als Einzeiler.
//!
//! ## ⚑ Der Würfel ist eine Funktion, kein Zustand
//!
//! Naheliegend wäre ein PRNG, der über die Gewichte läuft. **Das wäre
//! hier falsch, und zwar aus zwei Gründen, die beide das Protokoll
//! betreffen:**
//!
//! 1. **Ein Zustand hängt an der Reihenfolge.** Wer die Gewichte in
//!    anderer Reihenfolge durchläuft, bekommt andere Zufallszahlen und
//!    damit andere Gewichte. Zwei ehrliche Miner, die dieselbe Rechnung
//!    verschieden aufteilen, kämen zu verschiedenen Ergebnissen — und
//!    der Redundanzvergleich meldete beide als fehlerhaft.
//! 2. **Ein Zustand müsste übertragen werden.** Er wäre Teil des
//!    Trainingssegments, also des Konsensvertrags.
//!
//! Der Würfel ist deshalb eine reine Funktion aus **(Ebene, Schritt,
//! Index)**. Zwei Miner brauchen sich über nichts zu einigen außer über
//! diese drei Zahlen, die ohnehin feststehen.
//!
//! **Das ist derselbe Gedanke wie die Assoziativität der
//! Ganzzahladdition**, auf die das ganze Projekt gebaut ist: Kein
//! Ergebnis darf von der Reihenfolge abhängen.

use crate::prng::splitmix64;

/// Ein Gradient, wie ihn [`crate::backward`] liefert.
pub type Grad = i32;

/// Ein Mastergewicht.
///
/// Breiter als die Übertragungsform: Aus ihm wird int8, aber gerechnet
/// wird auf ihm. Die kleinen Bewegungen, die ein SGD-Schritt erzeugt,
/// hätten in int8 keinen Platz.
pub type Master = i32;

/// Zusätzliche Bruchstellen unterhalb der Master-Rasterstufe, in denen
/// die Bewegung eines Schrittes ausgerechnet wird.
///
/// ⚑ **Ohne sie gäbe es nichts zu runden.** Der Schritt ist im Median
/// 6,4e-6 einer Stufe groß; wer ihn in Stufen rechnet, rechnet mit null.
/// Zwanzig Bit reichen für Bewegungen bis herunter zu etwa 1e-6 einer
/// Stufe, also für den gemessenen Median.
pub const FEIN_BITS: u32 = 20;

/// Die Bruchstellen eines Mastergewichts, wie das Protokoll sie
/// festlegt.
///
/// # ⚑ Warum das eine Festlegung ist und keine Wahl des Aufrufers
///
/// Aus `master_frac` und der Zeilenverschiebung folgt der reale Wert
/// eines Gewichts (siehe `trainingsschritt::gewicht_aus_master`).
/// **Zwei ehrliche Miner, die denselben Trainingsschritt mit
/// verschiedenen Werten rechnen, bekommen verschiedene Gewichte**, und
/// der Redundanzvergleich meldete beide als fehlerhaft, ohne dass einer
/// gelogen hätte.
///
/// ⚑ **Dieselbe Lage wie beim Würfel**, und dort ist die Antwort
/// ausgeschrieben: Was in die Bitgleichheit eingeht, darf kein Aufrufer
/// wählen.
///
/// # Woher die Zwanzig kommt
///
/// Die Zeilenverschiebungen eines quantisierten Gewichts liegen zwischen
/// null und zwanzig. Der Master muss mindestens so viele Bruchstellen
/// tragen, damit `master_frac − s` nicht negativ wird; bei zwanzig passt
/// der grösste Master (`127 · 2^20`) mit Abstand in `i32`, und der Weg
/// vom Artefakt zum Master und zurück ist exakt.
///
/// ⚑ **Offen bleibt der Ort.** Als Eigenschaft des Zahlenformats gehört
/// die Zahl in die Formatfestlegung neben die übrigen Bruchstellen;
/// heute steht sie hier, weil eine Formatänderung jeden Golden Vector
/// neu erzeugen liesse. **Solange sie hier steht, gilt: ein Aufrufer,
/// der sie überschreibt, verlässt das Protokoll.**
pub const MASTER_FRAC: u8 = 20;

/// Der grösste Master, den die Übertragungsform noch ausdrücken kann.
///
/// # ⚑ Die naheliegende Zahl ist falsch, und zwar um eine Oktave
///
/// Naheliegend wäre `127 << MASTER_FRAC`, denn 127 ist der grösste
/// Betrag eines `i8`, und so prüft es auch der 30B-Lauf. **Die
/// Übertragungsform ist grosszügiger**, und eine Gegenprobe am
/// 2026-09-05 hat es gezeigt:
/// [`crate::trainingsschritt::gewicht_aus_master`] sucht die kleinste
/// Verschiebung `s` mit `betrag >> s <= 127` und lässt `s` bis
/// `master_frac` zu. Bei `s = master_frac` passt alles bis
/// `(128 << master_frac) − 1` hinein, weil das Schieben **abrundet**.
///
/// Der Unterschied ist keine Spitzfindigkeit. Eine Prüfung, die
/// **strenger** ist als die Form, meldet ein Segment als gescheitert,
/// das gerechnet hätte werden können; eine, die **lockerer** ist, lässt
/// den Absturz am fernen Ende passieren, den sie verhindern sollte.
/// Deshalb ist sie hier exakt, und ein Test hält sie gegen die Form.
///
/// ⚑ **Was diese Zahl nicht sagt:** ob die Zeile noch etwas taugt. Ein
/// Master in der letzten Oktave braucht `s = master_frac`, hat also
/// **null Nachkommastellen**: darstellbar, aber ohne jede Auflösung. Der
/// 30B-Lauf schlägt bei `127 << MASTER_FRAC` deshalb bewusst früher an.
pub const MASTER_GRENZE: i64 = (128i64 << MASTER_FRAC) - 1;

/// Verlässt eine dieser Matrizen die Übertragungsform?
///
/// Gibt den Index des ersten Gewichts zurück, dessen Betrag
/// [`MASTER_GRENZE`] überschreitet, sonst `None`.
///
/// # ⚑ Warum es diese Funktion gibt, und warum die Panik nicht reicht
///
/// [`crate::trainingsschritt::gewicht_aus_master`] **paniked** in genau
/// diesem Fall, und das ist dort richtig: Ein still gekapptes Gewicht
/// fiele erst an der Verlustkurve auf. Aber diese Panik kommt **am
/// fernen Ende**, beim Zurückschreiben ins Artefakt, nachdem ein Lauf
/// vielleicht tausend Schritte gerechnet hat.
///
/// ⚑ **Gemessen am 2026-09-05 auf dem echten 30B:** Bei Lernrate 2⁻¹⁰
/// verlässt der Router den Bereich **bei Schritt 67**. Der Testaufbau
/// fand das nur, weil er von Hand danach sah; `schritt` selbst prüft
/// nichts, und die Trainingsschleife merkte es nicht.
///
/// ⚑ **Für geshardetes Training ist eine Panik keine Antwort.** Ein Pod,
/// dessen Segment aus der Form läuft, muss das **melden** können, damit
/// die Kette das Segment als gescheitert verbucht statt auf ein Ergebnis
/// zu warten, das nie kommt. Ein Absturz ist von einem ausgefallenen
/// Miner nicht zu unterscheiden.
pub fn ausserhalb_der_form(master: &[Master]) -> Option<usize> {
    // ⚑ `unsigned_abs` und nicht `abs`: `Master::MIN.abs()` läuft über,
    // und `Master::MIN` ist erreichbar, weil `schritt` genau dorthin
    // klemmt. Ein Prüfer, der am Extremwert paniked, prüft das
    // Interessanteste nicht.
    let grenze = MASTER_GRENZE as u64;
    master.iter().position(|v| u64::from(v.unsigned_abs()) > grenze)
}

/// Der Würfel: eine reine Funktion aus Ebene, Schritt und Index.
///
/// ⚑ **Kein Zustand, keine Reihenfolge, nichts zu übertragen.** Wer
/// dieselben drei Zahlen einsetzt, bekommt dieselbe Zahl heraus, auf
/// jeder Maschine und in jeder Aufrufreihenfolge.
///
/// Die drei Zahlen werden nacheinander eingemischt statt addiert:
/// Addition brächte `(1, 2)` und `(2, 1)` auf denselben Wert, und
/// benachbarte Gewichte bekämen benachbarte Würfe.
#[inline]
pub fn wuerfel(ebene: u32, schritt: u64, index: u64) -> u64 {
    let (s, _) = splitmix64(ebene as u64);
    let (s, _) = splitmix64(s ^ schritt.rotate_left(17));
    let (_, z) = splitmix64(s ^ index.rotate_left(41));
    z
}

/// Rundet einen feinen Wert auf die Master-Rasterstufe, stochastisch.
///
/// `fein` ist in Einheiten von `2^-FEIN_BITS` Rasterstufen. Der ganze
/// Anteil wird übernommen; der Rest entscheidet **mit seiner eigenen
/// Wahrscheinlichkeit**, ob eine Stufe dazukommt.
///
/// ⚑ **Und zwar in Richtung des Vorzeichens.** Bei negativen Werten
/// rundet die ganzzahlige Division in Rust zur Null hin; wer den Rest
/// dann positiv behandelt, bekommt einen systematischen Drift nach oben.
/// Genau dieser Fehler wäre in einem Trainingslauf erst nach Tausenden
/// Schritten sichtbar, und dann als „das Modell lernt nicht".
#[inline]
pub fn runde_stochastisch(fein: i64, wurf: u64) -> i64 {
    let stufe = 1i64 << FEIN_BITS;
    let ganz = fein.div_euclid(stufe);
    let rest = fein.rem_euclid(stufe); // stets 0..stufe, auch negativ
    // Der Wurf entscheidet über die eine zusätzliche Stufe.
    let schwelle = (wurf & ((1u64 << FEIN_BITS) - 1)) as i64;
    if schwelle < rest {
        ganz + 1
    } else {
        ganz
    }
}

/// Wo ein Stück Gewichte im Trainingslauf steht.
///
/// ⚑ **Der Versatz ist nicht Bequemlichkeit, sondern Voraussetzung.**
/// Der Würfel hängt am Index innerhalb der Ebene. Leitete `schritt` ihn
/// aus der Position im übergebenen Stück ab, dann bekäme dasselbe
/// Gewicht je nach Zuschnitt einen anderen Wurf — **und ein Netz, das
/// Arbeit aufteilt, teilt Ebenen auf.** Zwei Miner mit verschiedenem
/// Zuschnitt kämen zu verschiedenen Gewichten, ohne dass einer gelogen
/// hätte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schrittkennung {
    /// Welche Ebene.
    pub ebene: u32,
    /// Der wievielte Trainingsschritt.
    pub schritt: u64,
    /// Der Index des **ersten** übergebenen Gewichts innerhalb der Ebene.
    pub index_versatz: u64,
}

/// Ein Trainingsschritt auf einer Gewichtsscheibe.
///
/// `lr_zaehler / lr_nenner` ist die Lernrate als Bruch zweier
/// Ganzzahlen. ⚑ **Kein Gleitkomma, auch nicht „nur für die Lernrate":**
/// Eine Gleitkommazahl im Konsenspfad ist eine Gleitkommazahl im
/// Konsenspfad, gleich wie klein ihre Rolle scheint.
///
/// Der Schritt ist **ordnungsfrei**: Wer die Indizes in anderer
/// Reihenfolge durchläuft, bekommt dieselben Gewichte.
///
/// **Panik statt stiller Kürzung**, wenn die Längen nicht passen: Zwei
/// Scheiben verschiedener Länge sind ein Aufrufer-Fehler, und ein
/// stillschweigend gekürzter Schritt fiele erst an der Verlustkurve auf.
pub fn schritt(
    master: &mut [Master],
    grad: &[Grad],
    kennung: Schrittkennung,
    lr_zaehler: i64,
    lr_nenner: i64,
) {
    assert_eq!(master.len(), grad.len(), "schritt: Laengen passen nicht");
    let mut summe = vec![0i64; grad.len()];
    sammle(&mut summe, grad, lr_zaehler, lr_nenner);
    schritt_aus_summe(master, &summe, kennung);
}

/// Rechnet einen Gradienten in feine Einheiten um und **addiert** ihn
/// auf, statt ihn anzuwenden.
///
/// # ⚑ Wozu, und warum die Summe und nicht der Mittelwert
///
/// Bei kleiner Lernrate ist die Bewegung eines einzelnen Schrittes fast
/// immer **kleiner als eine Master-Stufe**. Das stochastische Runden
/// entscheidet dann über jedes Gewicht mit einem Münzwurf: im Mittel
/// richtig, aber mit einer Streuung, die über viele Ebenen das Signal
/// überdeckt. **Gemessen am 2026-09-06 waren es über 24 Ebenen drei
/// Grössenordnungen** (Fund 189).
///
/// Wer `n` Gradienten erst summiert und dann **einmal** rundet, bekommt
/// **denselben Erwartungswert** und **einen** Münzwurf statt `n`. Die
/// Lernrate bleibt dabei dieselbe: Summiert werden die feinen
/// Einheiten, nicht die Schritte.
///
/// ⚑ **Der Mittelwert wäre falsch.** Er teilte die Bewegung durch `n`
/// und wäre damit eine `n`-fach kleinere Lernrate, also genau die
/// Grösse, die das Problem erzeugt.
///
/// ⚑ **`i64` und nicht `i32`.** Ein einzelnes `fein` passt in `i32`
/// nicht sicher, und die Summe über tausend Folgen erst recht nicht.
pub fn sammle(ziel: &mut [i64], grad: &[Grad], lr_zaehler: i64, lr_nenner: i64) {
    assert_eq!(ziel.len(), grad.len(), "sammle: Laengen passen nicht");
    assert!(lr_nenner > 0, "sammle: Lernraten-Nenner muss > 0 sein");
    for (z, g) in ziel.iter_mut().zip(grad) {
        let fein = -(*g as i64) * lr_zaehler * (1i64 << FEIN_BITS) / lr_nenner;
        *z = z.saturating_add(fein);
    }
}

/// Wendet eine aufgelaufene Summe feiner Einheiten an, mit **einem**
/// Wurf je Gewicht.
///
/// ⚑ **Sättigend, und aus demselben Grund wie zuvor:** Ein Gewicht, das
/// den Bereich verlässt, bleibt am Rand stehen. Ein umlaufendes Gewicht
/// wäre ein Vorzeichenwechsel aus dem Nichts.
pub fn schritt_aus_summe(master: &mut [Master], summe: &[i64], kennung: Schrittkennung) {
    assert_eq!(master.len(), summe.len(), "schritt_aus_summe: Laengen passen nicht");
    for (i, (w, f)) in master.iter_mut().zip(summe).enumerate() {
        let index = kennung.index_versatz + i as u64;
        let stufen = runde_stochastisch(*f, wuerfel(kennung.ebene, kennung.schritt, index));
        *w = (*w as i64).saturating_add(stufen).clamp(Master::MIN as i64, Master::MAX as i64)
            as Master;
    }
}

/// Sammelt den **rohen** Gradienten, ohne Lernrate.
///
/// ⚑ **Die Rate kommt erst nach der Normierung.** Wer sie vorher
/// anwendete, teilte durch eine Zahl und normierte danach wieder weg,
/// was er geteilt hat; die Rate wäre wirkungslos.
pub fn sammle_roh(ziel: &mut [i64], grad: &[Grad]) {
    assert_eq!(ziel.len(), grad.len(), "sammle_roh: Laengen passen nicht");
    for (z, g) in ziel.iter_mut().zip(grad) {
        *z = z.saturating_sub(*g as i64);
    }
}

/// Auf wie viele Bits das Betragsmaximum einer Matrix gebracht wird,
/// bevor die Lernrate greift.
///
/// # ⚑ Warum genau `MASTER_FRAC + FEIN_BITS`
///
/// Eine Rasterstufe sind `2^MASTER_FRAC` Master-Stufen, und eine
/// Master-Stufe sind `2^FEIN_BITS` feine Einheiten. Ein Betragsmaximum
/// von `2^(MASTER_FRAC + FEIN_BITS)` feinen Einheiten heisst deshalb
/// genau: **das grösste Gewicht der Matrix bewegt sich um eine ganze
/// Rasterstufe.**
///
/// Damit bekommt der Nenner der Lernrate eine Bedeutung, die man
/// aussprechen kann: `lr_nenner` ist die Zahl der Aktualisierungen, die
/// das grösste Gewicht einer Matrix braucht, um **eine Rasterstufe**
/// zu wandern.
pub const NORMBITS: u32 = MASTER_FRAC as u32 + FEIN_BITS;

/// Bringt eine gesammelte Bewegung auf die Normskala und gibt die
/// angewandte Verschiebung zurück.
///
/// # ⚑ Wozu, und was ohne sie geschieht
///
/// Ein Gradient trägt `dL/dZ` auf **einer Skala, die der Rechenweg
/// wählt**. Gemessen am 2026-09-06 fällt sie je Modell um
/// Grössenordnungen verschieden aus: Qwen2.5-0,5B lernt bei 2⁻²⁶ über
/// vierundzwanzig Ebenen, Qwen3-4B bewegt bei 2⁻¹² **kein einziges
/// Gewicht** und erst bei 2⁻⁸ (Fund 194).
///
/// **Damit war die Lernrate keine Protokollgrösse**, sondern eine Zahl,
/// die je Modell neu zu kalibrieren gewesen wäre. Für ein Netz, das
/// sein Modell wachsen lässt, ist das nicht haltbar: Nach jedem
/// Wachstumsschritt stünde die Kalibrierung wieder aus.
///
/// ⚑ **Eine Division und keine Zweierpotenz-Verschiebung, und das
/// haben zwei Tests entschieden.** Der erste Entwurf verschob um
/// `NORMBITS − bits(max)`. Das bringt das Betragsmaximum in ein **Band**
/// `[2^(NORMBITS−1), 2^NORMBITS)` und nicht auf einen Wert: Zwei
/// Modelle, deren Gradienten sich um den Faktor tausend unterscheiden,
/// lagen danach noch um den Faktor zwei auseinander.
///
/// **Zwei ist viel weniger als 256, und trotzdem nicht null.** Der Sinn
/// dieser Funktion ist, dass eine Lernrate überall dasselbe bedeutet;
/// ein Rest von einer Oktave hätte genau das nicht geleistet. Die
/// Division ist ganzzahlig, reihenfolgeunabhängig und einmalig beim
/// Anwenden, führt also keine zweite Art von **gespeicherter** Skala
/// ein.
///
/// ⚑ **Und sie wirkt je Matrix.** Zwei Matrizen einer Ebene tragen
/// verschieden grosse Gradienten, und mit dieser Normierung bewegen
/// sich beide gleich weit. Das ist eine Entscheidung und keine
/// Nebenwirkung: Sie entspricht der schichtweisen Ratenanpassung, die
/// die Literatur für grosse Chargen kennt.
///
/// Gibt das ursprüngliche Betragsmaximum zurück, oder `None`, wenn die
/// Matrix sich gar nicht bewegt hat; dann gibt es nichts zu normieren.
///
/// ⚑ **Das Maximum ist der Rückgabewert und nicht die Verschiebung**,
/// weil es die Grösse ist, die etwas aussagt: Sie sagt, wie weit dieses
/// Modell überhaupt zieht, und genau diese Zahl unterscheidet sich je
/// Modell um Grössenordnungen.
pub fn normiere(summe: &mut [i64]) -> Option<u64> {
    let groesster = summe.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    if groesster == 0 {
        return None;
    }
    // ⚑ **`i128` für das Zwischenergebnis.** `summe_i · 2^40` verlässt
    // `i64` schon bei mittelgrossen Summen; wer hier sättigte, machte
    // aus der Normierung eine Klemmung.
    let ziel = 1i128 << NORMBITS;
    let teiler = groesster as i128;
    for z in summe.iter_mut() {
        *z = ((*z as i128) * ziel / teiler) as i64;
    }
    Some(groesster)
}

/// Normiert eine gesammelte Bewegung und wendet sie mit **einem** Wurf
/// je Gewicht an.
///
/// `lr_nenner` ist danach der Kehrwert des Anteils, um den sich das
/// **grösste Gewicht dieser Matrix je Aktualisierung bezogen auf sich
/// selbst** bewegt: Bei `lr_nenner = 1000` ändert es sich um ein
/// Tausendstel seines eigenen Betrags. Diese Bedeutung hängt weder an
/// der Skala des Gradienten noch an der der Gewichte.
///
/// Gibt das Betragsmaximum vor der Normierung zurück, oder `None`, wenn
/// sich nichts bewegt hat.
pub fn schritt_normiert(
    master: &mut [Master],
    summe: &mut [i64],
    kennung: Schrittkennung,
    lr_nenner: i64,
) -> Option<u64> {
    assert_eq!(master.len(), summe.len(), "schritt_normiert: Laengen passen nicht");
    assert!(lr_nenner > 0, "schritt_normiert: Lernraten-Nenner muss > 0 sein");
    let g_max = summe.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    if g_max == 0 {
        return None;
    }
    // ⚑ **Das Ziel ist die Matrix selbst und keine feste Rasterstufe.**
    // Der erste Entwurf normierte auf eine ganze Rasterstufe, also auf
    // eine **absolute** Grösse. Gemessen zerstörte das ein Netz aus
    // vierundzwanzig Ebenen bei Nenner vier vollständig, und der Grund
    // ist einleuchtend, sobald man ihn sieht: Eine Zeile mit kleinen
    // Gewichten bekam dieselbe absolute Bewegung wie eine mit grossen,
    // also eine **relative** Änderung von hundert Prozent.
    //
    // Bezogen wird die Bewegung deshalb auf das Betragsmaximum der
    // Matrix: `lr_nenner` sagt, um welchen **Bruchteil ihres eigenen
    // grössten Gewichts** sich eine Matrix je Aktualisierung bewegt.
    // Das ist scale-frei in beiden Richtungen, gegen die Skala des
    // Gradienten und gegen die der Gewichte, und es ist dieselbe
    // Grösse, die die Literatur zur schichtweisen Ratenanpassung als
    // Vertrauensverhältnis führt.
    let w_max = master.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    if w_max == 0 {
        // Eine Matrix aus lauter Nullen hat keine eigene Skala; sie zu
        // bewegen hiesse, eine zu erfinden.
        return None;
    }
    let ziel = (w_max as i128) << FEIN_BITS;
    let teiler = (g_max as i128) * (lr_nenner as i128);
    for (i, (w, f)) in master.iter_mut().zip(summe.iter()).enumerate() {
        let index = kennung.index_versatz + i as u64;
        let fein = ((*f as i128) * ziel / teiler) as i64;
        let stufen = runde_stochastisch(fein, wuerfel(kennung.ebene, kennung.schritt, index));
        *w = (*w as i64).saturating_add(stufen).clamp(Master::MIN as i64, Master::MAX as i64)
            as Master;
    }
    Some(g_max)
}

/// Wie [`schritt_normiert`], bezieht die Bewegung aber auf das
/// Betragsmaximum **der Zeile** statt der ganzen Matrix.
///
/// # ⛑ Warum es das braucht: Fund 194, eine Ebene tiefer
///
/// [`schritt_normiert`] traegt in seinem eigenen Kommentar die
/// Begruendung, die hier weitergeht:
///
/// > Eine Zeile mit kleinen Gewichten bekam dieselbe absolute Bewegung
/// > wie eine mit grossen, also eine **relative** Aenderung von hundert
/// > Prozent.
///
/// Damals ging es von **je Tensor absolut** auf **je Matrix relativ**.
/// ⚑ **Innerhalb der Matrix besteht dasselbe Problem unveraendert
/// fort:** Das Gewicht mit dem groessten Gradienten bewegt sich um
/// `w_max / lr_nenner`, einen Bruchteil des groessten Gewichts **der
/// Matrix**.
///
/// | Gewicht | Anteil an `w_max` | Bewegung bei Nenner 64 | in Prozent seiner selbst |
/// |---|---|---|---|
/// | das groesste | 100 % | 1,6 % von `w_max` | 1,6 % |
/// | ein mittleres | 10 % | 1,6 % von `w_max` | **16 %** |
/// | ein kleines | 1 % | 1,6 % von `w_max` | ⛑ **156 %**, es kippt |
///
/// Gemessen am 2026-09-08: Nenner 64 auf den Ebenen zerstoerte das
/// Modell, Paris fiel von Rang 94 auf 51 670.
///
/// # ⚑ Warum die Zeile die richtige Einheit ist, und keine erfundene
///
/// Die Uebertragungsform quantisiert **zeilenweise**: Jede Zeile traegt
/// ihren eigenen Versatz (`w_shifts`), und die Matrix liegt als
/// `[aus, ein]` zeilenweise. Das Betragsmaximum einer Zeile ist damit
/// dieselbe Groesse, an der auch die Quantisierung haengt.
///
/// # ⚑ Und warum `g_max` trotzdem ueber die ganze Matrix geht
///
/// Naeme man auch ihn je Zeile, bewegte sich **jede** Zeile um
/// `1/lr_nenner` ihres eigenen Maximums, auch eine mit verschwindendem
/// Gradienten: Aus der Wichtigkeit einer Zeile wuerde ihre blosse
/// Anwesenheit. `lr_nenner` sagt hier: **Die Zeile mit dem groessten
/// Gradienten der Matrix bewegt sich um diesen Bruchteil ihres eigenen
/// Maximums**, alle anderen anteilig weniger.
///
/// `zeilenbreite` ist `in_features`. Ist sie 0 oder passt sie nicht,
/// faellt die Funktion auf [`schritt_normiert`] zurueck, statt still
/// etwas anderes zu rechnen.
pub fn schritt_normiert_je_zeile(
    master: &mut [Master],
    summe: &mut [i64],
    zeilenbreite: usize,
    kennung: Schrittkennung,
    lr_nenner: i64,
) -> Option<u64> {
    assert_eq!(master.len(), summe.len(), "schritt_normiert_je_zeile: Laengen passen nicht");
    assert!(lr_nenner > 0, "schritt_normiert_je_zeile: Lernraten-Nenner muss > 0 sein");
    if zeilenbreite == 0 || master.len() % zeilenbreite != 0 {
        return schritt_normiert(master, summe, kennung, lr_nenner);
    }
    let g_max = summe.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    if g_max == 0 {
        return None;
    }
    let teiler = (g_max as i128) * (lr_nenner as i128);
    for (z, (mzeile, szeile)) in master
        .chunks_mut(zeilenbreite)
        .zip(summe.chunks(zeilenbreite))
        .enumerate()
    {
        let w_max = mzeile.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
        if w_max == 0 {
            // Eine Zeile aus lauter Nullen hat keine eigene Skala; sie
            // zu bewegen hiesse, eine zu erfinden. Dieselbe Ueberlegung
            // wie fuer die Matrix in `schritt_normiert`.
            continue;
        }
        let ziel = (w_max as i128) << FEIN_BITS;
        let ab = z * zeilenbreite;
        for (i, (w, f)) in mzeile.iter_mut().zip(szeile.iter()).enumerate() {
            let index = kennung.index_versatz + (ab + i) as u64;
            let fein = ((*f as i128) * ziel / teiler) as i64;
            let stufen = runde_stochastisch(fein, wuerfel(kennung.ebene, kennung.schritt, index));
            *w = (*w as i64).saturating_add(stufen).clamp(Master::MIN as i64, Master::MAX as i64)
                as Master;
        }
    }
    Some(g_max)
}

#[cfg(test)]
mod zeilennormierung {
    use super::*;

    /// ⚑ **Der Befund, um den es geht.**
    ///
    /// Zwei Zeilen, dieselbe Gradientenlage, hundertfach verschiedene
    /// Gewichte. Bei Matrixnormierung bekommt die kleine Zeile die
    /// **absolute** Bewegung der grossen und wird zerrissen; bei
    /// Zeilennormierung bewegen sich beide um denselben **Anteil ihrer
    /// selbst**.
    #[test]
    fn die_kleine_zeile_wird_nicht_von_der_grossen_zerrissen() {
        let breite: usize = 8;
        let gross: Vec<Master> = (0..breite).map(|i| 100_000 + i as Master * 1000).collect();
        let klein: Vec<Master> = (0..breite).map(|i| 1_000 + i as Master * 10).collect();
        let start: Vec<Master> = gross.iter().chain(klein.iter()).copied().collect();
        let grad: Vec<i32> =
            (0..2 * breite).map(|i| if i % 2 == 0 { 1000i32 } else { -1000 }).collect();
        let kn = Schrittkennung { ebene: 1, schritt: 0, index_versatz: 0 };

        let mut a = start.clone();
        let mut sa = vec![0i64; 2 * breite];
        sammle_roh(&mut sa, &grad);
        schritt_normiert(&mut a, &mut sa, kn, 64);

        let mut b = start.clone();
        let mut sb = vec![0i64; 2 * breite];
        sammle_roh(&mut sb, &grad);
        schritt_normiert_je_zeile(&mut b, &mut sb, breite, kn, 64);

        // Die kleine Zeile: relative Bewegung je Verfahren.
        let rel = |neu: &[Master]| -> f64 {
            let d: i64 = neu[breite..]
                .iter()
                .zip(&start[breite..])
                .map(|(n, s)| (*n as i64 - *s as i64).abs())
                .sum();
            let betrag: i64 = start[breite..].iter().map(|w| w.unsigned_abs() as i64).sum();
            d as f64 / betrag as f64
        };
        let matrix = rel(&a);
        let zeile = rel(&b);
        assert!(
            matrix > 0.5,
            "die Matrixnormierung zerreisst die kleine Zeile nicht mehr ({matrix:.2});              dann ist dieser Test veraltet"
        );
        assert!(
            zeile < 0.05,
            "die Zeilennormierung bewegt die kleine Zeile zu weit ({zeile:.3})"
        );
    }

    /// Die grosse Zeile bewegt sich bei beiden Verfahren gleich weit:
    /// Ihr Maximum **ist** das Maximum der Matrix.
    #[test]
    fn die_groesste_zeile_bewegt_sich_wie_zuvor() {
        let breite: usize = 8;
        let start: Vec<Master> = (0..2 * breite).map(|i| 50_000 - (i as Master) * 100).collect();
        let grad: Vec<i32> = (0..2 * breite).map(|i| (i as i32 % 5) - 2).collect();
        let kn = Schrittkennung { ebene: 3, schritt: 7, index_versatz: 0 };

        let mut a = start.clone();
        let mut sa = vec![0i64; 2 * breite];
        sammle_roh(&mut sa, &grad);
        schritt_normiert(&mut a, &mut sa, kn, 128);

        let mut b = start.clone();
        let mut sb = vec![0i64; 2 * breite];
        sammle_roh(&mut sb, &grad);
        schritt_normiert_je_zeile(&mut b, &mut sb, breite, kn, 128);

        // Erste Zeile: bei ihr sind Zeilen- und Matrixmaximum dasselbe.
        assert_eq!(&a[..breite], &b[..breite]);
    }

    /// ⚑ Skalenfrei wie das Vorbild: derselbe Gradient mal tausend
    /// bewegt gleich weit.
    #[test]
    fn die_skala_des_gradienten_faellt_auch_hier_heraus() {
        let breite: usize = 4;
        let start: Vec<Master> = (0..3 * breite).map(|i| 2000 + i as Master * 300).collect();
        let klein: Vec<i32> = (0..3 * breite).map(|i| i as i32 - 6).collect();
        let gross: Vec<i32> = klein.iter().map(|g| g * 1000).collect();
        let kn = Schrittkennung { ebene: 2, schritt: 1, index_versatz: 0 };

        let mut a = start.clone();
        let mut sa = vec![0i64; 3 * breite];
        sammle_roh(&mut sa, &klein);
        schritt_normiert_je_zeile(&mut a, &mut sa, breite, kn, 16);

        let mut b = start.clone();
        let mut sb = vec![0i64; 3 * breite];
        sammle_roh(&mut sb, &gross);
        schritt_normiert_je_zeile(&mut b, &mut sb, breite, kn, 16);

        assert_eq!(a, b, "die Gradientenskala faellt nicht heraus");
        assert_ne!(a, start, "es hat sich gar nichts bewegt");
    }

    /// ⚑ **Kein stiller Rueckfall in etwas Drittes.** Passt die Breite
    /// nicht, wird die Matrixnormierung gerechnet, und zwar genau sie.
    #[test]
    fn eine_unpassende_breite_faellt_sauber_zurueck() {
        let start: Vec<Master> = (0..10).map(|i| 700 + i as Master * 50).collect();
        let grad: Vec<i32> = (0..10).map(|i| i - 5).collect();
        let kn = Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 };

        let mut a = start.clone();
        let mut sa = vec![0i64; 10];
        sammle_roh(&mut sa, &grad);
        schritt_normiert(&mut a, &mut sa, kn, 32);

        for breite in [0usize, 3, 4, 7] {
            let mut b = start.clone();
            let mut sb = vec![0i64; 10];
            sammle_roh(&mut sb, &grad);
            schritt_normiert_je_zeile(&mut b, &mut sb, breite, kn, 32);
            assert_eq!(a, b, "Breite {breite} faellt nicht auf die Matrixnormierung zurueck");
        }
    }

    /// Eine Zeile aus lauter Nullen bekommt keine erfundene Skala.
    #[test]
    fn eine_nullzeile_bleibt_null() {
        let breite: usize = 4;
        let mut m: Vec<Master> = vec![0, 0, 0, 0, 900, 800, 700, 600];
        let grad: Vec<i32> = vec![5, -5, 5, -5, 5, -5, 5, -5];
        let mut s = vec![0i64; 8];
        sammle_roh(&mut s, &grad);
        let kn = Schrittkennung { ebene: 4, schritt: 2, index_versatz: 0 };
        schritt_normiert_je_zeile(&mut m, &mut s, breite, kn, 8);
        assert_eq!(&m[..breite], &[0, 0, 0, 0], "die Nullzeile hat eine Skala bekommen");
        assert_ne!(&m[breite..], &[900, 800, 700, 600], "die zweite Zeile hat sich nicht bewegt");
    }
}

#[cfg(test)]
mod normierung {
    use super::*;

    /// ⚑ **Zwei Modelle, dieselbe Bewegung.**
    ///
    /// Derselbe Gradient, einmal mit dem Tausendfachen multipliziert.
    /// Ohne Normierung bewegte der eine Lauf tausendmal so weit; mit
    /// ihr bewegen beide **gleich weit**. Genau das ist Fund 194.
    #[test]
    fn die_skala_des_gradienten_faellt_heraus() {
        let klein: Vec<i32> = (0..64).map(|i| i - 32).collect();
        let gross: Vec<i32> = klein.iter().map(|g| g * 1000).collect();
        let kn = Schrittkennung { ebene: 2, schritt: 0, index_versatz: 0 };

        // ⚑ **Ein Startstand, der nicht null ist.** Die Bewegung ist
        // seit dem 2026-09-06 ein Anteil des **eigenen** Betragsmaximums
        // der Matrix; eine Matrix aus lauter Nullen hat keines, und sie
        // zu bewegen hiesse, eine Skala zu erfinden.
        let start: Vec<Master> = (0..64).map(|i| (i as Master) * 1000 + 5000).collect();

        let mut a = start.clone();
        let mut sa = vec![0i64; 64];
        sammle_roh(&mut sa, &klein);
        schritt_normiert(&mut a, &mut sa, kn, 1 << 4);

        let mut b = start.clone();
        let mut sb = vec![0i64; 64];
        sammle_roh(&mut sb, &gross);
        schritt_normiert(&mut b, &mut sb, kn, 1 << 4);

        assert_eq!(a, b, "die Gradientenskala faellt nicht heraus");
        assert!(a != start, "es hat sich gar nichts bewegt");
    }

    /// ⚑ **Der Nenner sagt, was er verspricht.** Bei `lr_nenner = n`
    /// bewegt sich das groesste Gewicht einer Matrix um ein `n`-tel
    /// **seines eigenen Betrags**.
    ///
    /// ⚑ **Das ist die zweite Fassung dieses Tests, und die erste war
    /// falsch.** Sie erwartete ein `n`-tel einer **Rasterstufe**, also
    /// eine absolute Groesse. Gemessen zerstoerte diese Bedeutung ein
    /// Netz aus vierundzwanzig Ebenen bei `n = 4` vollstaendig: Eine
    /// Zeile mit kleinen Gewichten bekam dieselbe absolute Bewegung wie
    /// eine mit grossen.
    #[test]
    fn der_nenner_bedeutet_ein_n_tel_des_eigenen_betrags() {
        for n in [1i64, 2, 16, 256] {
            let grad: Vec<i32> = vec![-1000, 500, -250, 125];
            // Ein Betragsmaximum, das gross genug ist, dass ein
            // Zweihundertsechsundfuenfzigstel davon noch sichtbar ist.
            let start: Vec<Master> = vec![1 << 20, 1 << 18, -(1 << 17), 1 << 16];
            let w_max = (1i64 << 20) as f64;
            let mut m = start.clone();
            let mut s = vec![0i64; grad.len()];
            sammle_roh(&mut s, &grad);
            schritt_normiert(
                &mut m,
                &mut s,
                Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 },
                n,
            );
            // Das groesste Element des Gradienten ist -1000, es sitzt
            // auf Position 0; dort ist die Bewegung am groessten.
            let bewegung = (m[0] - start[0]).unsigned_abs() as f64;
            let erwartet = w_max / n as f64;
            assert!(
                (bewegung - erwartet).abs() <= 2.0,
                "bei n={n} erwartet {erwartet}, gefunden {bewegung}"
            );
        }
    }

    /// Eine Matrix ohne Bewegung wird nicht normiert.
    ///
    /// ⚑ **Sonst teilte man durch null.** Und der Fall ist nicht
    /// theoretisch: Ein Experte, der in keiner Position gewaehlt wurde,
    /// hat genau diesen Gradienten.
    #[test]
    fn eine_unbewegte_matrix_wird_nicht_normiert() {
        let mut s = vec![0i64; 8];
        assert_eq!(normiere(&mut s), None);
        let mut m = vec![7i32; 8];
        let vorher = m.clone();
        assert_eq!(
            schritt_normiert(
                &mut m,
                &mut s,
                Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 },
                4
            ),
            None
        );
        assert_eq!(m, vorher, "eine unbewegte Matrix darf sich nicht bewegen");
    }

    /// Eine Matrix aus lauter Nullen wird nicht bewegt.
    ///
    /// ⚑ **Sie hat keine eigene Skala**, und eine zu erfinden hiesse,
    /// ihr eine Bedeutung zu geben, die sie nicht hat.
    #[test]
    fn eine_nullmatrix_bekommt_keine_skala() {
        let mut m = vec![0i32; 4];
        let mut s = vec![0i64; 4];
        sammle_roh(&mut s, &[-1000, 500, -250, 125]);
        assert_eq!(
            schritt_normiert(
                &mut m,
                &mut s,
                Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 },
                8
            ),
            None
        );
        assert_eq!(m, vec![0i32; 4]);
    }

    /// Die Normierung trifft ihr Ziel **genau**, nach oben wie nach
    /// unten.
    ///
    /// ⚑ **`assert_eq` und nicht „im Band".** Der erste Entwurf
    /// verschob um Zweierpotenzen und traf ein Band; der Test darauf
    /// waere gruen gewesen und haette den Faktor zwei durchgelassen,
    /// den der Test darueber dann fand.
    #[test]
    fn nach_der_normierung_liegt_das_maximum_im_band() {
        for start in [1i64, 1 << 10, 1 << 40, 1 << 55] {
            let mut s = vec![start, -start / 3, start / 7];
            normiere(&mut s).expect("bewegt");
            let groesster = s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
            assert_eq!(
                groesster,
                1u64 << NORMBITS,
                "bei Start {start} traf die Normierung ihr Ziel nicht"
            );
        }
    }
}

#[cfg(test)]
mod sammlung {
    use super::*;

    /// ⚑ **Der Aufbau ist so gewaehlt, dass der Wuerfel maximal
    /// entscheidet.** Mit `g = -1` und `lr_nenner = 2` ist die Bewegung
    /// eines Schrittes **exakt eine halbe Master-Stufe**: Der ganze
    /// Anteil ist null, der Rest ist die Haelfte, und das Runden ist
    /// ein fairer Muenzwurf. Ein Aufbau, bei dem der Rest klein waere,
    /// prueft nichts.
    const N: usize = 16;

    fn leer() -> Vec<Master> {
        vec![0; 64]
    }

    /// Ein Sammelschritt ueber `N` Gradienten bewegt **jedes** Gewicht
    /// um genau `N/2` Stufen, ohne jede Streuung.
    ///
    /// ⚑ **Das ist der Kern von Punkt 4.14.** Die Summe ist
    /// `N * stufe/2 = (N/2) * stufe`, also ein glattes Vielfaches: Es
    /// bleibt **kein Rest**, ueber den der Wuerfel entscheiden koennte.
    #[test]
    fn gesammelt_bewegt_sich_jedes_gewicht_gleich_weit() {
        let mut master = leer();
        let grad = vec![-1i32; master.len()];
        let mut summe = vec![0i64; master.len()];
        for _ in 0..N {
            sammle(&mut summe, &grad, 1, 2);
        }
        schritt_aus_summe(
            &mut master,
            &summe,
            Schrittkennung { ebene: 3, schritt: 0, index_versatz: 0 },
        );
        assert!(
            master.iter().all(|w| *w == (N / 2) as Master),
            "gesammelt darf nicht streuen, gefunden: {:?}",
            &master[..8]
        );
    }

    /// **Die Gegenprobe:** Dieselben `N` Gradienten einzeln angewandt
    /// streuen, und zwar breit.
    ///
    /// ⚑ **Der Erwartungswert ist derselbe**, die Streuung ist es
    /// nicht. Genau das ist Fund 189 in seiner kleinsten Form.
    #[test]
    fn einzeln_angewandt_streut_dasselbe_ergebnis() {
        let mut master = leer();
        let grad = vec![-1i32; master.len()];
        for s in 0..N as u64 {
            schritt(
                &mut master,
                &grad,
                Schrittkennung { ebene: 3, schritt: s, index_versatz: 0 },
                1,
                2,
            );
        }
        let kleinster = *master.iter().min().expect("nicht leer");
        let groesster = *master.iter().max().expect("nicht leer");
        assert!(
            groesster > kleinster,
            "einzeln angewandt muesste streuen, alle stehen auf {kleinster}"
        );
        // Und die Streuung ist keine Kleinigkeit: ueber sechzehn Wuerfe
        // liegen die Gewichte mehrere Stufen auseinander.
        assert!(
            groesster - kleinster >= 4,
            "erwartet wurde eine breite Streuung, gefunden {kleinster} bis {groesster}"
        );
        // Der Mittelwert trifft trotzdem N/2: der Wuerfel ist fair.
        let summe: i64 = master.iter().map(|w| *w as i64).sum();
        let mittel = summe as f64 / master.len() as f64;
        assert!(
            (mittel - (N as f64 / 2.0)).abs() < 2.0,
            "der Erwartungswert stimmt nicht, Mittel {mittel}"
        );
    }

    /// Ein einzelner gesammelter Gradient ist **bitgleich** mit einem
    /// gewoehnlichen Schritt.
    ///
    /// ⚑ **Sonst waere die Sammlung ein zweiter Rechenweg** und damit
    /// eine zweite Quelle fuer Abweichungen zwischen zwei ehrlichen
    /// Minern.
    #[test]
    fn eins_gesammelt_ist_ein_gewoehnlicher_schritt() {
        let grad: Vec<i32> = (0..64).map(|i| i * 37 - 900).collect();
        let kn = Schrittkennung { ebene: 5, schritt: 11, index_versatz: 7 };

        let mut a = leer();
        schritt(&mut a, &grad, kn, 1, 1 << 12);

        let mut b = leer();
        let mut summe = vec![0i64; grad.len()];
        sammle(&mut summe, &grad, 1, 1 << 12);
        schritt_aus_summe(&mut b, &summe, kn);

        assert_eq!(a, b, "Sammlung mit n=1 weicht vom gewoehnlichen Schritt ab");
    }
}

#[cfg(test)]
mod shardtransparenz {
    use super::*;

    /// ⚑ **Der Nenner und die Gradienten muessen einen Rest lassen.**
    ///
    /// Der erste Entwurf dieser Tests nahm Gradienten, die glatt durch
    /// den Nenner teilbar waren. Dann ist die Schrittweite exakt, das
    /// stochastische Runden hat nichts zu entscheiden, **und der Wuerfel
    /// kommt gar nicht vor**. Beide Tests waren gruen und prueften
    /// nichts; die Gegenprobe hat es am 2026-09-05 aufgedeckt.
    ///
    /// Hier ist `grad / NENNER` durchweg null mit grossem Rest, also
    /// entscheidet allein der Wuerfel zwischen null und eins.
    const NENNER: i64 = 1 << 12;

    fn aufbau() -> (Vec<Master>, Vec<i32>, Schrittkennung) {
        let n = 64usize;
        let anfang: Vec<Master> = (0..n).map(|i| (i as Master) * 977 - 20_000).collect();
        let grad: Vec<i32> = (0..n).map(|i| (i as i32) * 37 + 401).collect();
        (anfang, grad, Schrittkennung { ebene: 7, schritt: 3, index_versatz: 0 })
    }

    /// ⚑ **Die Eigenschaft, an der geshardetes Training haengt.**
    ///
    /// Der Wuerfel ist eine reine Funktion aus `(ebene, schritt, index)`,
    /// wobei `index` **innerhalb der Ebene** zaehlt. Wenn das stimmt,
    /// ist es gleichgueltig, ob eine Ebene in einem Stueck oder in
    /// Scheiben fortgeschrieben wird: Zwei Shards, die sich eine Ebene
    /// teilen, kommen zum selben Ergebnis wie ein einzelner Rechner.
    ///
    /// **Ohne diese Eigenschaft waere geshardetes Training nicht gegen
    /// einen Einzelknoten nachrechenbar**, und damit fiele die
    /// Redundanzpruefung aus, auf der die ganze Verifikation ruht.
    ///
    /// ⚑ **Und sie stellt die eigentliche Anforderung an den Shard:**
    /// Er muss seine **globale** Ebenennummer und seinen **globalen**
    /// Versatz innerhalb der Ebene kennen. Ein Shard, der seine Ebenen
    /// bei null durchnummerierte, wuerfelte anders, und niemand saehe es
    /// dem Ergebnis an: Es waere ein plausibles Delta, nur ein anderes.
    #[test]
    fn eine_ebene_in_scheiben_ergibt_dasselbe_wie_am_stueck() {
        let (anfang, grad, kn) = aufbau();
        let n = anfang.len();

        // Am Stueck.
        let mut ganz = anfang.clone();
        schritt(&mut ganz, &grad, kn, 1, NENNER);
        assert_ne!(ganz, anfang, "der Schritt hat gar nichts bewegt");

        // In drei Scheiben, mit den richtigen Versaetzen.
        let mut geteilt = anfang.clone();
        let schnitte = [0usize, 17, 40, n];
        for paar in schnitte.windows(2) {
            let (a, b) = (paar[0], paar[1]);
            schritt(
                &mut geteilt[a..b],
                &grad[a..b],
                Schrittkennung { index_versatz: a as u64, ..kn },
                1,
                NENNER,
            );
        }
        assert_eq!(ganz, geteilt, "die Aufteilung hat das Ergebnis veraendert");
    }

    /// ⚑ **Die Gegenprobe: der falsche Versatz faellt auf.** Ein Shard,
    /// der seinen Versatz bei null anfangen liesse, bekaeme ein anderes
    /// Ergebnis. Der Test haelt fest, dass die Zusicherung oben etwas
    /// zusichert.
    #[test]
    fn ein_falscher_versatz_ergibt_ein_anderes_delta() {
        let (anfang, grad, kn) = aufbau();
        let n = anfang.len();

        let mut ganz = anfang.clone();
        schritt(&mut ganz, &grad, kn, 1, NENNER);

        let mut falsch = anfang.clone();
        let schnitte = [0usize, 17, 40, n];
        for paar in schnitte.windows(2) {
            let (a, b) = (paar[0], paar[1]);
            // Jede Scheibe faengt faelschlich bei null an.
            schritt(&mut falsch[a..b], &grad[a..b], kn, 1, NENNER);
        }
        assert_ne!(
            ganz, falsch,
            "der falsche Versatz aendert nichts; dann prueft der Test darueber nichts"
        );
    }

    /// Und die Ebenennummer wirkt ebenso.
    #[test]
    fn eine_andere_ebenennummer_wuerfelt_anders() {
        let n = 48usize;
        let anfang: Vec<Master> = (0..n).map(|i| (i as Master) * 131).collect();
        let grad: Vec<i32> = vec![3_000; n];
        let mut a = anfang.clone();
        let mut b = anfang.clone();
        schritt(&mut a, &grad, Schrittkennung { ebene: 0, schritt: 1, index_versatz: 0 }, 1, 1 << 14);
        schritt(&mut b, &grad, Schrittkennung { ebene: 1, schritt: 1, index_versatz: 0 }, 1, 1 << 14);
        assert_ne!(a, b, "die Ebenennummer geht nicht in den Wuerfel ein");
    }
}

#[cfg(test)]
mod grenze_tests {
    use super::*;

    /// Innerhalb der Form meldet die Prüfung nichts.
    #[test]
    fn was_hineinpasst_wird_nicht_gemeldet() {
        let m = vec![0, 1, -1, MASTER_GRENZE as Master, -(MASTER_GRENZE as Master)];
        assert_eq!(ausserhalb_der_form(&m), None);
    }

    /// ⚑ **Die Oktave, die der naheliegenden Zahl fehlt.** `127 << 20`
    /// ist nicht die Grenze der Form, sondern der Anfang ihrer letzten
    /// Oktave. Diese Werte gehen alle noch durch.
    #[test]
    fn die_letzte_oktave_gehoert_noch_dazu() {
        for v in [127i64 << MASTER_FRAC, (127i64 << MASTER_FRAC) + 1, MASTER_GRENZE] {
            assert_eq!(
                ausserhalb_der_form(&[v as Master]),
                None,
                "{v} liegt in der letzten Oktave und ist darstellbar"
            );
        }
    }

    /// Eins darüber wird gemeldet, mit dem Index.
    #[test]
    fn eins_darueber_wird_gemeldet() {
        let ueber = (MASTER_GRENZE + 1) as Master;
        assert_eq!(ausserhalb_der_form(&[0, 0, ueber, 0]), Some(2));
        assert_eq!(ausserhalb_der_form(&[0, -ueber]), Some(1));
    }

    /// ⚑ **Der Extremwert, an dem `abs` überliefe.** `schritt` klemmt
    /// auf `Master::MIN`, also ist dieser Wert erreichbar, und eine
    /// Prüfung, die dort paniked, ist an der einen Stelle blind, an der
    /// sie gebraucht wird.
    #[test]
    fn der_kleinste_master_laesst_die_pruefung_nicht_ueberlaufen() {
        assert_eq!(ausserhalb_der_form(&[Master::MIN]), Some(0));
        assert_eq!(ausserhalb_der_form(&[Master::MAX]), Some(0));
    }

    /// ⚑ **Die Schranke ist genau die, an der `gewicht_aus_master`
    /// aufgibt**, und das ist der Sinn der Sache: Was diese Prüfung
    /// durchlässt, muss dort ankommen.
    #[test]
    fn die_schranke_deckt_sich_mit_der_uebertragungsform() {
        let n = 8usize;
        let gerade_noch = vec![MASTER_GRENZE as Master; n];
        assert_eq!(ausserhalb_der_form(&gerade_noch), None);
        let (w, shifts) = crate::trainingsschritt::gewicht_aus_master(&gerade_noch, n, MASTER_FRAC);
        assert_eq!(w.len(), n, "die Form traegt den Grenzwert");
        assert_eq!(shifts, vec![0u8]);

        // Und eins darüber gibt `gewicht_aus_master` auf.
        let zu_gross = vec![(MASTER_GRENZE + 1) as Master; n];
        assert!(ausserhalb_der_form(&zu_gross).is_some());
        assert!(
            std::panic::catch_unwind(|| {
                crate::trainingsschritt::gewicht_aus_master(&zu_gross, n, MASTER_FRAC)
            })
            .is_err(),
            "was diese Pruefung meldet, muss die Uebertragungsform ablehnen"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Die tragende Eigenschaft, und sie ist der Grund, warum
    /// stochastisches Runden überhaupt hilft:** Im Mittel trifft es den
    /// wahren Wert, während Rundung zur nächsten Stufe ihn systematisch
    /// verfehlt.
    ///
    /// Geprüft an einer Bewegung von einem Viertel einer Rasterstufe.
    /// Zur nächsten Stufe gerundet wäre sie **immer null**; stochastisch
    /// gerundet ist sie in einem Viertel der Fälle eins.
    #[test]
    fn stochastisches_runden_trifft_im_mittel_den_wahren_wert() {
        let stufe = 1i64 << FEIN_BITS;
        let viertel = stufe / 4;

        let mut summe = 0i64;
        let n = 20_000u64;
        for i in 0..n {
            summe += runde_stochastisch(viertel, wuerfel(0, 0, i));
        }
        // Erwartung: 0,25 · n. Zugelassen sind zwei Prozent Abweichung.
        let erwartet = (n / 4) as i64;
        let abstand = (summe - erwartet).abs();
        assert!(
            abstand * 50 < erwartet,
            "Summe {summe}, erwartet {erwartet}, Abstand {abstand}"
        );

        // Die Gegenprobe: Rundung zur nächsten Stufe ergibt hier
        // durchgehend null, und genau das ist der gemessene Schaden.
        let zur_naechsten = |fein: i64| (fein + (stufe / 2)) / stufe;
        assert_eq!(zur_naechsten(viertel), 0);
    }

    /// ⚑ Und dasselbe nach unten. Bei negativen Werten rundet die
    /// ganzzahlige Division in Rust zur Null hin; wer den Rest dann
    /// positiv behandelt, bekommt einen **systematischen Drift nach
    /// oben**, der erst nach Tausenden Schritten als „das Modell lernt
    /// nicht" auffiele.
    #[test]
    fn auch_nach_unten_wird_unverzerrt_gerundet() {
        let stufe = 1i64 << FEIN_BITS;
        let minus_viertel = -stufe / 4;

        let mut summe = 0i64;
        let n = 20_000u64;
        for i in 0..n {
            summe += runde_stochastisch(minus_viertel, wuerfel(7, 3, i));
        }
        let erwartet = -((n / 4) as i64);
        let abstand = (summe - erwartet).abs();
        assert!(
            abstand * 50 < erwartet.abs(),
            "Summe {summe}, erwartet {erwartet}, Abstand {abstand}"
        );
    }

    /// Ganze Stufen gehen unverändert durch, mit und ohne Rest.
    #[test]
    fn ganze_stufen_bleiben_ganze_stufen() {
        let stufe = 1i64 << FEIN_BITS;
        for wurf in [0u64, u64::MAX, 12345] {
            assert_eq!(runde_stochastisch(3 * stufe, wurf), 3);
            assert_eq!(runde_stochastisch(-3 * stufe, wurf), -3);
            assert_eq!(runde_stochastisch(0, wurf), 0);
        }
    }

    /// ⚑ **Der Kern der Sache: Der Schritt ist ordnungsfrei.** Wer die
    /// Indizes in anderer Reihenfolge durchläuft, bekommt dieselben
    /// Gewichte. Mit einem PRNG-Zustand wäre das falsch, und zwei
    /// ehrliche Miner mit verschiedener Aufteilung kämen zu
    /// verschiedenen Ergebnissen.
    #[test]
    fn der_schritt_haengt_nicht_am_zuschnitt() {
        let grad: Vec<Grad> = (0..64).map(|i| (i * 37 % 101) - 50).collect();
        let start: Vec<Master> = (0..64).map(|i| i * 3).collect();
        let k = |versatz| Schrittkennung { ebene: 5, schritt: 42, index_versatz: versatz };

        // In einem Zug.
        let mut ganz = start.clone();
        schritt(&mut ganz, &grad, k(0), 1, 3);

        // In vier Stücken, in umgekehrter Reihenfolge, über **dieselbe
        // Schnittstelle**. Nur der Versatz sagt, wo das Stück steht.
        let mut stueckweise = start.clone();
        for anfang in (0..64).step_by(16).rev() {
            schritt(
                &mut stueckweise[anfang..anfang + 16],
                &grad[anfang..anfang + 16],
                k(anfang as u64),
                1,
                3,
            );
        }
        assert_eq!(ganz, stueckweise);

        // ⚑ Und die Gegenprobe: Ohne den Versatz wäre es falsch. Jedes
        // Stück bekäme die Würfe des ersten, und das Ergebnis wiche ab.
        let mut ohne_versatz = start.clone();
        for anfang in (0..64).step_by(16) {
            schritt(
                &mut ohne_versatz[anfang..anfang + 16],
                &grad[anfang..anfang + 16],
                k(0),
                1,
                3,
            );
        }
        assert_ne!(ganz, ohne_versatz, "ohne Versatz muesste es abweichen");
    }

    /// Zwei Läufe mit denselben Zahlen sind bitgleich, und drei
    /// verschiedene Ebenen oder Schritte sind es nicht.
    #[test]
    fn derselbe_wurf_bei_denselben_zahlen_und_sonst_nicht() {
        assert_eq!(wuerfel(3, 9, 17), wuerfel(3, 9, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(4, 9, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(3, 10, 17));
        assert_ne!(wuerfel(3, 9, 17), wuerfel(3, 9, 18));
        // ⚑ Und die Vertauschung ergibt nicht denselben Wurf: Addition
        // hätte (1,2) und (2,1) zusammenfallen lassen.
        assert_ne!(wuerfel(1, 2, 3), wuerfel(2, 1, 3));
        assert_ne!(wuerfel(1, 2, 3), wuerfel(3, 2, 1));
    }

    /// ⚑ Ein Gewicht am Rand läuft nicht um. Ein umlaufendes Gewicht
    /// wäre ein Vorzeichenwechsel aus dem Nichts.
    #[test]
    fn am_rand_wird_gesaettigt_und_nicht_umgelaufen() {
        let mut oben = vec![Master::MAX];
        schritt(&mut oben, &[-1_000_000], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1_000, 1);
        assert_eq!(oben[0], Master::MAX);

        let mut unten = vec![Master::MIN];
        schritt(&mut unten, &[1_000_000], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1_000, 1);
        assert_eq!(unten[0], Master::MIN);
    }

    /// Ein Gradient von null bewegt nichts, gleich wie der Würfel fällt.
    #[test]
    fn ohne_gradient_keine_bewegung() {
        let mut w = vec![7, -7, 0, 12345];
        let vorher = w.clone();
        schritt(&mut w, &[0, 0, 0, 0], Schrittkennung { ebene: 2, schritt: 99, index_versatz: 0 }, 5, 1);
        assert_eq!(w, vorher);
    }

    /// Die Bewegung zeigt der Steigung entgegen, wie ein Abstieg es
    /// verlangt.
    #[test]
    fn der_schritt_geht_bergab() {
        let stufe = 1i64 << FEIN_BITS;
        // Ein Gradient, der ganze Stufen ergibt: kein Würfel im Spiel.
        let g = (stufe / (1i64 << FEIN_BITS)) as Grad * 4;
        let mut w = vec![100 as Master];
        schritt(&mut w, &[g], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1, 1);
        assert!(w[0] < 100, "positiver Gradient muss das Gewicht senken");

        let mut w2 = vec![100 as Master];
        schritt(&mut w2, &[-g], Schrittkennung { ebene: 0, schritt: 0, index_versatz: 0 }, 1, 1);
        assert!(w2[0] > 100, "negativer Gradient muss es heben");
    }
}

/// Der **Trainingsabdruck**: der Wert, den zwei Maschinen vergleichen.
///
/// # ⚑ Wozu es ihn gibt
///
/// Der Konformitätslauf belegt Bitgleichheit gegen **feste**
/// Sollvektoren: Zwei Maschinen, die dasselbe Soll treffen, rechnen
/// gleich. Für einen Trainingsschritt auf einem echten Modell gibt es
/// kein solches Soll, und es kann keins geben, denn die Sollwerte wären
/// die Antwort auf genau die Frage, die geprüft werden soll.
///
/// Also vergleichen die Maschinen **ihre Ergebnisse miteinander**. Der
/// Abdruck ist die Zahl, die dabei nebeneinandergelegt wird.
///
/// # ⚑ Warum die Länge vor jeder Matrix steht
///
/// Ohne sie wären die Bytes zweier Matrizen von 100 und 200 Gewichten
/// nicht von denen dreier Matrizen mit 100, 100 und 100 zu
/// unterscheiden: dieselbe Bytefolge, derselbe Abdruck. Dieselbe
/// Überlegung wie bei den Trennzeichen in `digest_ueber` des
/// Testclients; hier steht sie als Längenangabe da, weil Zahlen jeden
/// Trenner enthalten können.
///
/// ⚑ **Und die Reihenfolge der Matrizen geht ein.** Sie ist keine
/// Eigenschaft des Dateisystems, sondern der Rechenvorschrift: Q, K, V,
/// O, Gate, Up, Down. Wer sie anders reiht, rechnet etwas anderes.
///
/// # ⚑ Was der Abdruck nicht ist
///
/// **Kein Beleg für Richtigkeit.** Zwei Maschinen, die denselben Fehler
/// machen, bekommen denselben Abdruck. Er belegt Übereinstimmung, und
/// das ist genau die Frage, an der die Kernthese hängt: Wer verifizieren
/// will, muss nachrechnen können, und nachrechnen heisst dasselbe
/// herausbekommen.
pub fn trainingsabdruck(matrizen: &[&[Master]]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for m in matrizen {
        h.update((m.len() as u64).to_le_bytes());
        for w in *m {
            h.update(w.to_le_bytes());
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod abdruck_tests {
    use super::*;

    #[test]
    fn derselbe_zustand_ergibt_denselben_abdruck() {
        let a = vec![1i32, -2, 3];
        let b = vec![4i32, 5];
        assert_eq!(
            trainingsabdruck(&[&a, &b]),
            trainingsabdruck(&[&a, &b])
        );
    }

    /// ⚑ **Ein einziges Gewicht reicht.** Ein Abdruck, der eine
    /// Abweichung in der letzten Stelle verschluckte, belegte nichts.
    #[test]
    fn ein_einziges_gewicht_aendert_den_abdruck() {
        let a = vec![1i32, -2, 3];
        let mut c = a.clone();
        c[2] += 1;
        assert_ne!(trainingsabdruck(&[&a]), trainingsabdruck(&[&c]));
    }

    /// ⚑ **Die Gegenprobe zur Längenangabe.** Ohne sie hätten diese
    /// beiden Aufteilungen denselben Abdruck.
    #[test]
    fn dieselben_zahlen_anders_aufgeteilt_sind_ein_anderer_abdruck() {
        let alle = [1i32, 2, 3, 4, 5, 6];
        let links = trainingsabdruck(&[&alle[..2], &alle[2..]]);
        let rechts = trainingsabdruck(&[&alle[..4], &alle[4..]]);
        assert_ne!(links, rechts);
    }

    /// ⚑ **Und die Gegenprobe zur Reihenfolge.**
    #[test]
    fn die_reihenfolge_der_matrizen_geht_ein() {
        let a = vec![1i32, 2];
        let b = vec![3i32, 4];
        assert_ne!(trainingsabdruck(&[&a, &b]), trainingsabdruck(&[&b, &a]));
    }

    #[test]
    fn der_abdruck_ist_ein_voller_sha256() {
        let a = vec![0i32];
        let d = trainingsabdruck(&[&a]);
        assert_eq!(d.len(), 64, "{d}");
        assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Die **Aggregation vieler Miner** (TRAINING 2.2):
///
/// ```text
/// m_{v+1} = klemmen(m_v + Σ Δm_i)
/// ```
///
/// # ⚑ Ordnungsfrei, und das ist kein Nebeneffekt
///
/// Viele Miner rechnen verschiedene Chargen gegen **denselben**
/// Ausgangszustand, und ihre Ergebnisse addieren sich. In welcher
/// Reihenfolge die Beiträge eintreffen, entscheidet das Netz, also
/// niemand: Pakete überholen sich, ein Knoten sammelt anders als der
/// nächste. **Hinge das Ergebnis daran, hinge der Kettenzustand am
/// Zufall der Zustellung.**
///
/// Eine Summe ganzer Zahlen ist von Natur aus ordnungsfrei. Was sie
/// verlieren kann, ist genau eines: die **Sättigung**.
///
/// # ⚑ Geklemmt wird genau einmal, ganz am Ende
///
/// Wer je Summand klemmt, bekommt ein Ergebnis, das von der Reihenfolge
/// abhängt. Der Fall ist keine Spitzfindigkeit, er ist einzeilig
/// hinzuschreiben: Mit Obergrenze 100 und Beiträgen `+80, +80, −80`
/// ergibt
///
/// | Reihenfolge | je Summand geklemmt | einmal am Ende |
/// |---|---|---|
/// | `+80, +80, −80` | 100 dann 20 | **80** |
/// | `−80, +80, +80` | −80 dann 0 dann 80 | **80** |
///
/// links zweimal etwas anderes, rechts zweimal dasselbe.
///
/// ⚑ **Dieselbe Regel wie in `mische_experten` und in
/// `linear_backward`** (Fund 24): in `i64` summieren, **einmal**
/// schieben, **einmal** klemmen. Sie steht in diesem Projekt zum
/// dritten Mal, und zum dritten Mal aus demselben Grund.
///
/// # Argumente
///
/// - `master`: der Ausgangszustand `m_v`, wird an Ort und Stelle
///   fortgeschrieben
/// - `deltas`: die Beiträge, je einer je Miner, alle über derselben
///   Gewichtsscheibe
///
/// # ⚑ Panik statt stiller Kürzung
///
/// Ein Delta anderer Länge ist ein Aufrufer-Fehler, kein Randfall. Wer
/// es stillschweigend anschnitte, aggregierte einen Teil und zahlte für
/// das Ganze.
pub fn aggregiere(master: &mut [Master], deltas: &[&[Master]]) {
    for (n, d) in deltas.iter().enumerate() {
        assert_eq!(
            d.len(),
            master.len(),
            "aggregiere: Beitrag {n} hat {} Gewichte, der Master {}",
            d.len(),
            master.len()
        );
    }
    for (i, m) in master.iter_mut().enumerate() {
        // ⚑ **Die Summe in i64.** Bei 10 Millionen Beiträgen zu je
        // 2^31 wäre ein i32-Akkumulator nach dem zweiten voll; die
        // Bitbudget-Rechnung vom 2026-08-22 sagt dasselbe für den
        // Aggregationsakkumulator wie für den Master.
        let mut summe = i64::from(*m);
        for d in deltas {
            summe += i64::from(d[i]);
        }
        *m = summe.clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master;
    }
}

/// Das **Δm** eines Segments: was ein Schritt an den Gewichten geändert
/// hat.
///
/// ⚑ **Die Differenz und nicht der Endzustand.** Ein Endzustand addiert
/// sich nicht: Zwei Miner, die verschiedene Chargen gegen denselben
/// Ausgangszustand rechnen, haben zwei verschiedene Endzustände, und
/// keiner davon ist der, den die Kette am Ende trägt. Siehe
/// [`aggregiere`] und `myl_types::trainingssegment`.
///
/// **Panik statt stiller Kürzung**, aus demselben Grund wie dort.
pub fn delta(vorher: &[Master], nachher: &[Master]) -> Vec<Master> {
    assert_eq!(
        vorher.len(),
        nachher.len(),
        "delta: {} gegen {} Gewichte",
        vorher.len(),
        nachher.len()
    );
    vorher
        .iter()
        .zip(nachher.iter())
        .map(|(a, b)| {
            // ⚑ **Auch hier geklemmt und nicht umlaufend.** Die
            // Differenz zweier i32 passt nicht immer in i32; ein Umlauf
            // machte aus einem grossen Schritt einen grossen Schritt in
            // die Gegenrichtung.
            (i64::from(*b) - i64::from(*a))
                .clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master
        })
        .collect()
}

/// Eine **dünn besetzte** Scheibe eines Beitrags: wo sie im Master
/// beginnt, und was sie ändert.
pub type Scheibe<'a> = (usize, &'a [Master]);

/// Die Aggregation über **dünn besetzte** Beiträge (TRAINING Phase 5).
///
/// # ⚑ Warum ein Expertengemisch einen eigenen Weg braucht
///
/// Bei einem Gemisch berührt ein Miner nur die Experten, die sein Router
/// gewählt hat. **Gemessen am 2026-09-05 an Qwen3-30B-A3B**, 24 Schritte
/// auf einer Ebene: **18 von 128 Experten**, mit Lastausgleich, und 10
/// ohne. Ein dichtes Δm über alle wäre zu 86 Prozent null.
///
/// Bei 128 Experten je Ebene und 4,7 Millionen Gewichten je Experte sind
/// das **560 Millionen Nullen** je Ebene und Beitrag, übertragen und
/// summiert. ⚑ **Das ist nicht Sparsamkeit, sondern der Kern der
/// Sache:** Ein Expertengemisch ist genau deshalb billig, weil ein Token
/// nur k Experten berührt; eine Aggregation, die alle anfasst, hat den
/// Vorteil weggeworfen.
///
/// # ⚑ Ordnungsfrei, und geklemmt genau einmal
///
/// Dieselbe Regel wie in [`aggregiere`], mit einem Zusatz: Zwei Beiträge
/// dürfen **dieselbe** Stelle treffen, denn zwei Miner können denselben
/// Experten gewählt haben. Deshalb wird erst über alle Beiträge
/// aufsummiert und **danach** geklemmt, nicht je Scheibe.
///
/// ⚑ **`BTreeMap` und keine Hashtabelle.** Der Sammelplatz wird geordnet
/// durchlaufen; eine Hashtabelle gäbe dabei eine Reihenfolge, die an
/// Adressen hängt. Auf das Ergebnis wirkt sich das hier nicht aus, weil
/// jede Stelle einmal geschrieben wird, **aber es wäre eine Reihenfolge
/// im Konsenspfad, die niemand festgelegt hat**, und die nächste
/// Änderung machte sie sichtbar.
pub fn aggregiere_duenn(master: &mut [Master], beitraege: &[&[Scheibe<'_>]]) {
    use std::collections::BTreeMap;
    let mut summen: BTreeMap<usize, i64> = BTreeMap::new();
    for (n, beitrag) in beitraege.iter().enumerate() {
        for (versatz, scheibe) in beitrag.iter() {
            assert!(
                versatz + scheibe.len() <= master.len(),
                "aggregiere_duenn: Beitrag {n} reicht von {versatz} ueber {} hinaus,                  der Master hat {}",
                scheibe.len(),
                master.len()
            );
            for (i, d) in scheibe.iter().enumerate() {
                *summen.entry(versatz + i).or_insert(0) += i64::from(*d);
            }
        }
    }
    for (i, s) in summen {
        let summe = i64::from(master[i]) + s;
        master[i] = summe.clamp(i64::from(Master::MIN), i64::from(Master::MAX)) as Master;
    }
}

#[cfg(test)]
mod aggregation_tests {
    use super::*;

    /// ⚑ **Die Aussage, um die es geht: die Reihenfolge ändert nichts.**
    #[test]
    fn die_reihenfolge_der_beitraege_aendert_das_ergebnis_nicht() {
        let start = vec![100i32, -50, 0, 7];
        let a = vec![10i32, 20, -30, 1];
        let b = vec![-5i32, 5, 5, -1];
        let c = vec![1000i32, -1000, 1, 0];

        let mut links = start.clone();
        aggregiere(&mut links, &[&a, &b, &c]);
        let mut rechts = start.clone();
        aggregiere(&mut rechts, &[&c, &a, &b]);
        let mut mitte = start.clone();
        aggregiere(&mut mitte, &[&b, &c, &a]);

        assert_eq!(links, rechts);
        assert_eq!(links, mitte);
        assert_eq!(links, vec![1105, -1025, -24, 7]);
    }

    /// ⚑ **Auch an der Sättigung**, und das ist der einzige Ort, wo eine
    /// ganzzahlige Summe die Ordnungsfreiheit verlieren kann.
    #[test]
    fn die_reihenfolge_aendert_auch_an_der_saettigung_nichts() {
        let gross = Master::MAX - 10;
        let start = vec![gross];
        let hoch = vec![100i32];
        let runter = vec![-100i32];

        let mut a = start.clone();
        aggregiere(&mut a, &[&hoch, &runter]);
        let mut b = start.clone();
        aggregiere(&mut b, &[&runter, &hoch]);

        assert_eq!(a, b, "die Reihenfolge hat an der Saettigung entschieden");
        assert_eq!(a, vec![gross], "die Summe ist null, der Wert bleibt");
    }

    /// ⛑ **Die Gegenprobe: so sähe es aus, wenn je Summand geklemmt
    /// würde.**
    ///
    /// Ohne diesen Test bliebe offen, ob der obige überhaupt etwas
    /// auswählt: Vielleicht klemmt gar nichts, und dann prüft er die
    /// Reihenfolge einer Summe, die ohnehin nie an die Grenze kommt.
    #[test]
    fn je_summand_geklemmt_haenge_das_ergebnis_an_der_reihenfolge() {
        let gross = i64::from(Master::MAX) - 10;
        let klemmen = |v: i64| v.clamp(i64::from(Master::MIN), i64::from(Master::MAX));
        let je_summand = |folge: &[i64]| -> i64 {
            let mut m = gross;
            for d in folge {
                m = klemmen(m + d);
            }
            m
        };
        assert_ne!(
            je_summand(&[100, -100]),
            je_summand(&[-100, 100]),
            "der Aufbau erreicht die Saettigung gar nicht, dann prueft der Test daneben nichts"
        );
    }

    /// ⚑ **Ohne Beiträge bleibt der Master, wie er war.**
    #[test]
    fn ohne_beitraege_aendert_sich_nichts() {
        let mut m = vec![1i32, -2, 3];
        aggregiere(&mut m, &[]);
        assert_eq!(m, vec![1, -2, 3]);
    }

    /// ⚑ **Ein Beitrag falscher Länge bricht ab.**
    #[test]
    #[should_panic(expected = "Beitrag 1 hat")]
    fn ein_beitrag_falscher_laenge_bricht_ab() {
        let mut m = vec![1i32, 2, 3];
        let gut = vec![1i32, 1, 1];
        let kurz = vec![1i32];
        aggregiere(&mut m, &[&gut, &kurz]);
    }

    /// ⚑ **Δm ist die Differenz, und sie führt zurück.**
    #[test]
    fn das_delta_fuehrt_zum_endzustand_zurueck() {
        let vorher = vec![100i32, -50, 0];
        let nachher = vec![103i32, -49, -7];
        let d = delta(&vorher, &nachher);
        assert_eq!(d, vec![3, 1, -7]);
        let mut zurueck = vorher.clone();
        aggregiere(&mut zurueck, &[&d]);
        assert_eq!(zurueck, nachher);
    }

    /// ⚑ **Zwei Deltas gegen denselben Ausgangszustand addieren sich.**
    ///
    /// Das ist die Eigenschaft, wegen der das Commitment über Δm geht
    /// und nicht über den Endzustand: Endzustände addieren sich nicht.
    #[test]
    fn zwei_deltas_gegen_denselben_start_addieren_sich() {
        let start = vec![100i32, 100];
        let ende_a = vec![110i32, 90];
        let ende_b = vec![95i32, 120];
        let da = delta(&start, &ende_a);
        let db = delta(&start, &ende_b);

        let mut zusammen = start.clone();
        aggregiere(&mut zusammen, &[&da, &db]);
        assert_eq!(zusammen, vec![105, 110]);

        // ⛑ Und die Gegenprobe: die Endzustaende zu addieren gaebe
        // etwas anderes, naemlich den doppelten Ausgangszustand mit.
        let mut falsch = vec![0i32, 0];
        aggregiere(&mut falsch, &[&ende_a, &ende_b]);
        assert_ne!(falsch, zusammen, "Endzustaende addieren sich doch?");
    }

    /// ⚑ **Die Differenz klemmt, statt umzulaufen.**
    #[test]
    fn eine_grosse_differenz_klemmt() {
        let d = delta(&[Master::MIN], &[Master::MAX]);
        assert_eq!(d, vec![Master::MAX], "die Differenz ist umgelaufen");
    }
}

#[cfg(test)]
mod duenne_aggregation_tests {
    use super::*;

    /// ⚑ **Dünn und dicht liefern dasselbe.**
    ///
    /// Die Aussage, an der alles hängt: Der dünne Weg ist eine
    /// Abkürzung, keine zweite Rechnung.
    #[test]
    fn duenn_und_dicht_liefern_dasselbe() {
        let start = vec![10i32; 12];
        // Zwei Beitraege, die je zwei Scheiben treffen und sich an einer
        // Stelle ueberlappen.
        let a1 = [1i32, 2];
        let a2 = [5i32, 5, 5];
        let b1 = [-3i32];
        let b2 = [7i32, 7];

        let mut duenn = start.clone();
        aggregiere_duenn(
            &mut duenn,
            &[&[(0usize, &a1[..]), (4, &a2[..])], &[(1usize, &b1[..]), (4, &b2[..])]],
        );

        // Dasselbe von Hand als dichte Beitraege.
        let mut da = vec![0i32; 12];
        da[0] = 1;
        da[1] = 2;
        da[4] = 5;
        da[5] = 5;
        da[6] = 5;
        let mut db = vec![0i32; 12];
        db[1] = -3;
        db[4] = 7;
        db[5] = 7;
        let mut dicht = start.clone();
        aggregiere(&mut dicht, &[&da, &db]);

        assert_eq!(duenn, dicht);
    }

    /// ⚑ **Die Reihenfolge der Beiträge ändert nichts**, auch wenn sie
    /// sich überlappen.
    #[test]
    fn die_reihenfolge_der_duennen_beitraege_aendert_nichts() {
        let start = vec![100i32; 6];
        let a = [10i32, 20];
        let b = [-5i32, -5];
        let mut links = start.clone();
        aggregiere_duenn(&mut links, &[&[(2usize, &a[..])], &[(2usize, &b[..])]]);
        let mut rechts = start.clone();
        aggregiere_duenn(&mut rechts, &[&[(2usize, &b[..])], &[(2usize, &a[..])]]);
        assert_eq!(links, rechts);
        assert_eq!(links[2], 105);
    }

    /// ⚑ **Zwei Beiträge auf dieselbe Stelle addieren sich**, statt dass
    /// einer den anderen überschreibt.
    ///
    /// Zwei Miner können denselben Experten gewählt haben; wer hier
    /// überschriebe, verwürfe die halbe Arbeit.
    #[test]
    fn zwei_beitraege_auf_dieselbe_stelle_addieren_sich() {
        let mut m = vec![0i32; 3];
        let a = [7i32];
        let b = [5i32];
        aggregiere_duenn(&mut m, &[&[(1usize, &a[..])], &[(1usize, &b[..])]]);
        assert_eq!(m, vec![0, 12, 0], "einer hat den anderen ueberschrieben");
    }

    /// ⚑ **Geklemmt genau einmal**, auch dünn.
    #[test]
    fn auch_duenn_wird_genau_einmal_geklemmt() {
        let gross = Master::MAX - 10;
        let hoch = [100i32];
        let runter = [-100i32];
        let mut a = vec![gross];
        aggregiere_duenn(&mut a, &[&[(0usize, &hoch[..])], &[(0usize, &runter[..])]]);
        let mut b = vec![gross];
        aggregiere_duenn(&mut b, &[&[(0usize, &runter[..])], &[(0usize, &hoch[..])]]);
        assert_eq!(a, b);
        assert_eq!(a, vec![gross]);
    }

    /// ⚑ **Unberührte Stellen bleiben unberührt.**
    #[test]
    fn was_niemand_beruehrt_bleibt_stehen() {
        let mut m = vec![1i32, 2, 3, 4, 5];
        let d = [10i32];
        aggregiere_duenn(&mut m, &[&[(2usize, &d[..])]]);
        assert_eq!(m, vec![1, 2, 13, 4, 5]);
    }

    /// ⚑ **Eine Scheibe über den Rand hinaus bricht ab.**
    #[test]
    #[should_panic(expected = "reicht von 3 ueber 4 hinaus")]
    fn eine_scheibe_ueber_den_rand_bricht_ab() {
        let mut m = vec![0i32; 5];
        let d = [1i32, 1, 1, 1];
        aggregiere_duenn(&mut m, &[&[(3usize, &d[..])]]);
    }
}

#[cfg(test)]
mod vektorbau {
    use super::*;

    /// Erzeugt die Zahlen für den Konformitätsvektor des normierten
    /// Schritts.
    ///
    /// ⚑ **Ein Test und kein Binary**, damit er mit dem Code wandert und
    /// nicht daneben veraltet. Er läuft nur mit `--nocapture` sichtbar
    /// und behauptet nichts über Richtigkeit: Er **liest ab**, was diese
    /// Umsetzung tut.
    #[test]
    fn zahlen_fuer_den_konformitaetsvektor() {
        let master: Vec<Master> = vec![1280, -640, 320, 16, -1280, 96, -32, 4096];
        let grad: Vec<Grad> = vec![1500, -700, 40, 900, -3, 1000000, 7, -250000];
        let kn = Schrittkennung { ebene: 3, schritt: 17, index_versatz: 64 };
        let nenner = 256i64;

        let mut summe = vec![0i64; grad.len()];
        sammle_roh(&mut summe, &grad);
        let mut m = master.clone();
        let vorher = schritt_normiert(&mut m, &mut summe, kn, nenner).expect("bewegt");

        let wuerfe: Vec<u64> = (0..master.len())
            .map(|i| wuerfel(kn.ebene, kn.schritt, kn.index_versatz + i as u64))
            .collect();
        eprintln!("VEKTOR master     {master:?}");
        eprintln!("VEKTOR grad       {grad:?}");
        eprintln!("VEKTOR summe_roh  {:?}", {
            let mut s = vec![0i64; grad.len()];
            sammle_roh(&mut s, &grad);
            s
        });
        eprintln!("VEKTOR wuerfe     {wuerfe:?}");
        eprintln!("VEKTOR g_max      {vorher}");
        eprintln!("VEKTOR master_neu {m:?}");
        assert_ne!(m, master, "der Vektor muesste etwas bewegen");
    }
}
