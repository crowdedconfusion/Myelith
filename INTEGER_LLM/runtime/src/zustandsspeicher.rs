//! **Der Speicher der rekurrenten Ebenen**: Zustand und Faltungsfenster.
//!
//! # ⚑ Der Unterschied zum KV-Speicher, und er ist der Sinn der Sache
//!
//! Der KV-Speicher **waechst mit der Folge**: je Position ein Schluessel
//! und ein Wert, je Ebene und Kopf. Dieser Speicher hier **waechst
//! nicht**. Der Zustand einer rekurrenten Ebene ist eine Matrix fester
//! Groesse, `schluessel_dim` mal `wert_dim`, ganz gleich ob zehn oder
//! zweihunderttausend Token gelaufen sind.
//!
//! ⚑ **Das ist der Grund, warum diese Bauart lange Folgen traegt.** Beim
//! grossen Modell sind 30 von 40 Ebenen rekurrent; nur die uebrigen zehn
//! legen ueberhaupt etwas ab, das mit der Laenge waechst.
//!
//! # ⚑ Warum nur die rekurrenten Ebenen belegt werden
//!
//! Ein Zustand je Kopf sind `128 * 128 * 8` Bytes, also 128 KiB; bei 32
//! Koepfen 4 MiB je Ebene. **Fuer alle 40 Ebenen zu belegen kostete 40
//! MiB fuer nichts.** Belegt werden deshalb nur die rekurrenten, und ein
//! Platztisch bildet den absoluten Ebenenindex darauf ab.
//!
//! ⚠️ **Der Zugriff auf eine achtsame Ebene bricht ab** und liefert
//! keinen Ersatzzustand. Wer hier landet, hat die Ebenenart nicht
//! geprueft, und ein leerer Zustand waere eine falsche Zahl ohne Meldung.

use integer_llm_kernels::faltung::{Faltungsfenster, KERN};
use integer_llm_kernels::zustandsschicht::Zustand;

pub struct Zustandsspeicher {
    /// Je Ebene der Platz im Feld, oder `usize::MAX` fuer „achtsam".
    plaetze: Vec<usize>,
    koepfe: usize,
    /// `zustaende[platz * koepfe + kopf]`.
    zustaende: Vec<Zustand>,
    /// Ein Fenster je rekurrenter Ebene; die Faltung laeuft ueber alle
    /// Kanaele auf einmal und nicht je Kopf.
    fenster: Vec<Faltungsfenster>,
    laenge: usize,
}

impl Zustandsspeicher {
    /// **Legt den Speicher an**, nach der Ebenenliste.
    ///
    /// `rekurrent[i]` sagt, ob Ebene `i` rekurrent mischt.
    pub fn neu(
        rekurrent: &[bool],
        koepfe: usize,
        schluessel_dim: usize,
        wert_dim: usize,
        kanaele: usize,
    ) -> Self {
        assert!(koepfe > 0, "ohne Koepfe gibt es nichts abzulegen");
        let mut plaetze = vec![usize::MAX; rekurrent.len()];
        let mut zustaende = Vec::new();
        let mut fenster = Vec::new();
        for (i, &ist) in rekurrent.iter().enumerate() {
            if !ist {
                continue;
            }
            plaetze[i] = fenster.len();
            for _ in 0..koepfe {
                zustaende.push(Zustand::leer(schluessel_dim, wert_dim));
            }
            fenster.push(Faltungsfenster::leer(kanaele, KERN));
        }
        Self { plaetze, koepfe, zustaende, fenster, laenge: 0 }
    }

    fn platz(&self, ebene: usize) -> usize {
        let p = *self.plaetze.get(ebene).unwrap_or(&usize::MAX);
        assert_ne!(
            p,
            usize::MAX,
            "Ebene {ebene} mischt nicht rekurrent und hat keinen Zustand; \
             der Aufrufer prueft die Ebenenart nicht"
        );
        p
    }

    /// Der Zustand einer Ebene und eines Kopfes, zum Fortschreiben.
    pub fn zustand_mut(&mut self, ebene: usize, kopf: usize) -> &mut Zustand {
        let p = self.platz(ebene);
        assert!(kopf < self.koepfe, "Kopf {kopf} von {}", self.koepfe);
        &mut self.zustaende[p * self.koepfe + kopf]
    }

    /// Dasselbe, nur lesend.
    pub fn zustand(&self, ebene: usize, kopf: usize) -> &Zustand {
        let p = self.platz(ebene);
        assert!(kopf < self.koepfe, "Kopf {kopf} von {}", self.koepfe);
        &self.zustaende[p * self.koepfe + kopf]
    }

    /// Das Faltungsfenster einer Ebene, zum Fortschreiben.
    pub fn fenster_mut(&mut self, ebene: usize) -> &mut Faltungsfenster {
        let p = self.platz(ebene);
        &mut self.fenster[p]
    }

    /// **Wie viele Token durchgelaufen sind.**
    ///
    /// ⚠️ **Nicht die Groesse des Speichers**, denn die aendert sich
    /// nicht. Sie steht hier, weil ein Aufrufer wissen muss, ob der
    /// Zustand zum Kontext passt, den er meint.
    pub fn laenge(&self) -> usize {
        self.laenge
    }

    /// Ein Token ist durch.
    pub fn vorruecken(&mut self) {
        self.laenge += 1;
    }

    /// **Alles auf null, wie am Anfang einer Folge.**
    ///
    /// ⚑ **Ohne neu zu belegen.** Eine neue Folge soll keinen
    /// Speicherbedarf erzeugen, den die vorige schon hatte.
    pub fn leeren(&mut self) {
        for z in self.zustaende.iter_mut() {
            z.leeren();
        }
        for f in self.fenster.iter_mut() {
            f.leeren();
        }
        self.laenge = 0;
    }

    /// Wie viele Bytes belegt sind.
    pub fn belegte_bytes(&self) -> usize {
        let z: usize = self
            .zustaende
            .iter()
            .map(|z| z.schluessel_dim * z.wert_dim * std::mem::size_of::<i64>())
            .sum();
        let f: usize = self
            .fenster
            .iter()
            .map(|f| f.kanaele * (KERN - 1) * std::mem::size_of::<i16>())
            .sum();
        z + f
    }

    /// Wie viele Ebenen rekurrent mischen.
    pub fn rekurrente_ebenen(&self) -> usize {
        self.fenster.len()
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    fn muster() -> Vec<bool> {
        // Drei rekurrente, eine achtsame, wie beim grossen Modell.
        (0..8).map(|i| (i + 1) % 4 != 0).collect()
    }

    /// **Nur die rekurrenten Ebenen belegen Speicher.**
    #[test]
    fn achtsame_ebenen_belegen_nichts() {
        let s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        assert_eq!(s.rekurrente_ebenen(), 6, "von acht Ebenen sind sechs rekurrent");
        // 6 Ebenen * 2 Koepfe * 4*4 * 8 Byte + 6 * 16 * 3 * 2 Byte
        assert_eq!(s.belegte_bytes(), 6 * 2 * 16 * 8 + 6 * 16 * 3 * 2);
    }

    /// ⛔️ **Der Zugriff auf eine achtsame Ebene bricht ab.**
    ///
    /// 📌 Ein leerer Ersatzzustand waere eine falsche Zahl ohne Meldung,
    /// und die faellt erst viel spaeter als schlechte Ausgabe auf.
    #[test]
    #[should_panic(expected = "mischt nicht rekurrent")]
    fn eine_achtsame_ebene_hat_keinen_zustand() {
        let mut s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        let _ = s.zustand_mut(3, 0); // Ebene 3 ist achtsam
    }

    /// **Der Speicher waechst nicht mit der Folge.**
    ///
    /// ⚑ **Das ist die Zusage, um die es bei dieser Bauart geht**, und
    /// sie steht hier als Probe und nicht nur als Satz im Kopf.
    #[test]
    fn der_speicher_waechst_nicht_mit_der_folge() {
        let mut s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        let anfang = s.belegte_bytes();
        for _ in 0..1000 {
            s.zustand_mut(0, 0).set(0, 0, 42);
            s.vorruecken();
        }
        assert_eq!(s.laenge(), 1000);
        assert_eq!(s.belegte_bytes(), anfang, "der Speicher ist gewachsen");
    }

    /// **Leeren setzt zurueck, ohne neu zu belegen.**
    #[test]
    fn leeren_setzt_zurueck() {
        let mut s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        s.zustand_mut(0, 0).set(1, 1, 7);
        s.fenster_mut(0).set(0, 0, 9);
        s.vorruecken();
        let bytes = s.belegte_bytes();
        s.leeren();
        assert_eq!(s.laenge(), 0);
        assert_eq!(s.zustand(0, 0).get(1, 1), 0);
        assert_eq!(s.belegte_bytes(), bytes, "Leeren hat neu belegt");
    }

    /// ⚠️ **Zwei Koepfe teilen sich keinen Zustand.**
    #[test]
    fn koepfe_bleiben_getrennt() {
        let mut s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        s.zustand_mut(0, 0).set(2, 3, 11);
        assert_eq!(s.zustand(0, 1).get(2, 3), 0, "der zweite Kopf sieht den ersten");
    }

    /// ⚠️ **Zwei Ebenen teilen sich keinen Zustand.**
    #[test]
    fn ebenen_bleiben_getrennt() {
        let mut s = Zustandsspeicher::neu(&muster(), 2, 4, 4, 16);
        s.zustand_mut(0, 0).set(2, 3, 11);
        assert_eq!(s.zustand(1, 0).get(2, 3), 0, "die zweite Ebene sieht die erste");
    }
}
