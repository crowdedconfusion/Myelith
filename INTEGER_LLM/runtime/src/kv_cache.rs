//! Integer-KV-Cache
//!
//! Jeder Layer und jeder Head haelt Keys und Values als INT16-Fixed-Point.
//!
//! # 📌 Warum zusammenhaengend und nicht als Karte (2026-09-14)
//!
//! Bis hierher stand jede Position als eigener `Vec<i16>` in einem
//! `BTreeMap`. Die Aufmerksamkeit liest je Token und je Kopf **alle**
//! Positionen, und jede war eine eigene Belegung irgendwo im Speicher.
//! **Gemessen am 4B bei 1 907 Token Kontext:** 63 % eines Decode-Schritts
//! lagen in der Aufmerksamkeit, davon ein Viertel allein im Einsammeln der
//! Positionen aus der Karte; der Decode fiel von 44 auf 108 ms je Token.
//!
//! ⚑ **Jetzt liegt je Ebene und Kopf ein Feld, Position hinter Position.**
//! Das Lesen ist ein Ausschnitt ohne Belegung, und das Punktprodukt laeuft
//! ueber zusammenhaengenden Speicher. **An keiner Zahl aendert das etwas:**
//! gelesen werden dieselben Werte in derselben Reihenfolge.
//!
//! ⚠️ **Eine Luecke gibt es nicht mehr.** Die Karte nahm eine Position
//! hinter dem Ende an und liess die dazwischen einfach aus; die
//! Aufmerksamkeit sah dann weniger Positionen, ohne dass es jemand merkte.
//! Hier ist das ein Abbruch, und wer Positionen von aussen annimmt, prueft
//! vorher mit [`KVCache::laenge`].

pub struct KVCache {
    erste_ebene: usize,
    ebenen: usize,
    koepfe: usize,
    /// Werte je Position in Keys und Values, beim ersten Schreiben
    /// festgelegt; null, solange nichts geschrieben ist.
    breite_k: usize,
    breite_v: usize,
    /// Je Ebene und Kopf ein Feld: `k[(ebene - erste_ebene) * koepfe + kopf]`.
    k: Vec<Vec<i16>>,
    v: Vec<Vec<i16>>,
    /// **Der Zustand der rekurrenten Ebenen**, falls das Modell welche
    /// hat.
    ///
    /// ⚑ **Er wohnt hier und nicht daneben**, weil beide dasselbe sind:
    /// was die Folge mit sich traegt. Zwei Felder nebeneinander liessen
    /// zu, dass jemand das eine kuerzt und das andere vergisst, und
    /// **das waere plausibler, aber falscher Text**.
    zustand: Option<crate::zustandsspeicher::Zustandsspeicher>,
}

impl KVCache {
    pub fn new(num_layers: usize, num_heads: usize) -> Self {
        Self::for_range(0, num_layers, num_heads)
    }

    /// KV-Cache für einen Layer-Bereich `[layer_start, layer_end)`,
    /// für Pipeline-Stages, die nur ihre eigenen Layer halten
    /// (indiziert wird mit den absoluten Layer-Indizes, siehe
    /// `TransformerLayer.layer_idx`).
    pub fn for_range(layer_start: usize, layer_end: usize, num_heads: usize) -> Self {
        let ebenen = layer_end.saturating_sub(layer_start);
        let faecher = ebenen * num_heads;
        KVCache {
            zustand: None,
            erste_ebene: layer_start,
            ebenen,
            koepfe: num_heads,
            breite_k: 0,
            breite_v: 0,
            k: vec![Vec::new(); faecher],
            v: vec![Vec::new(); faecher],
        }
    }

    /// Das Fach einer Ebene und eines Kopfes; ausserhalb des Bereichs ein
    /// Abbruch, wie vorher beim `unwrap` auf der Karte.
    fn fach(&self, layer: usize, head: usize) -> usize {
        assert!(
            layer >= self.erste_ebene && layer < self.erste_ebene + self.ebenen && head < self.koepfe,
            "KV-Speicher: Ebene {layer} Kopf {head} liegt nicht in Ebenen [{}, {}) mit {} Koepfen",
            self.erste_ebene,
            self.erste_ebene + self.ebenen,
            self.koepfe
        );
        (layer - self.erste_ebene) * self.koepfe + head
    }

    /// Wie viele Positionen in einem Fach stehen.
    fn positionen(&self, fach: usize) -> usize {
        self.k[fach].len().checked_div(self.breite_k).unwrap_or(0)
    }

    /// **Schreibt eine Position**: hinter das Ende oder ueber eine schon
    /// geschriebene.
    pub fn write(&mut self, layer: usize, head: usize, pos: usize, key: &[i16], value: &[i16]) {
        let fach = self.fach(layer, head);
        if self.breite_k == 0 && self.breite_v == 0 {
            self.breite_k = key.len();
            self.breite_v = value.len();
        }
        assert!(
            key.len() == self.breite_k && value.len() == self.breite_v && self.breite_k > 0,
            "KV-Speicher: Breite {}/{} statt {}/{}",
            key.len(),
            value.len(),
            self.breite_k,
            self.breite_v
        );
        let n = self.positionen(fach);
        assert!(pos <= n, "KV-Speicher: Position {pos} hinter dem Ende {n} (Ebene {layer}, Kopf {head})");
        let (bk, bv) = (self.breite_k, self.breite_v);
        if pos == n {
            self.k[fach].extend_from_slice(key);
            self.v[fach].extend_from_slice(value);
        } else {
            self.k[fach][pos * bk..(pos + 1) * bk].copy_from_slice(key);
            self.v[fach][pos * bv..(pos + 1) * bv].copy_from_slice(value);
        }
    }

    /// **Die Positionen bis einschliesslich `upto`, als zwei
    /// zusammenhaengende Ausschnitte** (Keys, Values), Position hinter
    /// Position.
    pub fn lesen(&self, layer: usize, head: usize, upto: usize) -> (&[i16], &[i16]) {
        let fach = self.fach(layer, head);
        let n = self.positionen(fach).min(upto.saturating_add(1));
        (&self.k[fach][..n * self.breite_k], &self.v[fach][..n * self.breite_v])
    }

    /// Dasselbe wie [`KVCache::lesen`], je Position ein Ausschnitt.
    pub fn read_scheiben(
        &self,
        layer: usize,
        head: usize,
        upto: usize,
    ) -> (Vec<&[i16]>, Vec<&[i16]>) {
        let (k, v) = self.lesen(layer, head, upto);
        if self.breite_k == 0 {
            return (Vec::new(), Vec::new());
        }
        (k.chunks_exact(self.breite_k).collect(), v.chunks_exact(self.breite_v).collect())
    }

    pub fn read(&self, layer: usize, head: usize, upto: usize) -> (Vec<Vec<i16>>, Vec<Vec<i16>>) {
        let (k, v) = self.read_scheiben(layer, head, upto);
        (k.into_iter().map(<[i16]>::to_vec).collect(), v.into_iter().map(<[i16]>::to_vec).collect())
    }

    /// **Behaelt nur die Positionen unter `laenge`**, in allen Ebenen und
    /// Koepfen. Fuer die Fortsetzung eines Gespraechs, dessen Anfang gleich
    /// geblieben ist (siehe `generate::Fortsetzung`).
    /// **Kuerzt auf `laenge` und gibt zurueck, wie weit es wirklich ging.**
    ///
    /// # ⛔️ Ein rekurrenter Zustand laesst sich nicht kuerzen
    ///
    /// Ein KV-Eintrag je Position ist unabhaengig und laesst sich
    /// wegwerfen. Ein Zustand `S_t` ist das Ergebnis von `t`
    /// Fortschreibungen; es gibt **keinen Weg zurueck** auf Position
    /// `laenge`, ohne von vorn zu rechnen.
    ///
    /// ⚑ **Deshalb der Rueckgabewert.** Hat das Modell rekurrente
    /// Ebenen, wird auf **null** gekuerzt und alles geleert; der
    /// Aufrufer erfaehrt es und setzt dort auf, wo wirklich gekuerzt
    /// wurde.
    ///
    /// 📌 **Ein Wert, den der Aufrufer benutzen MUSS, ist sicherer als
    /// ein Kommentar, den er lesen KANN.** Ohne ihn kuerzte jemand den
    /// KV-Speicher, liesse den Zustand stehen und bekaeme plausiblen,
    /// aber falschen Text; niemand saehe es, weil die Ausgabe gut
    /// aussieht.
    #[must_use]
    pub fn kuerzen(&mut self, laenge: usize) -> usize {
        let wirklich = if self.zustand.is_some() && laenge > 0 {
            0
        } else {
            laenge
        };
        if let Some(z) = self.zustand.as_mut() {
            if wirklich == 0 {
                z.leeren();
            }
        }
        let (bk, bv) = (self.breite_k, self.breite_v);
        for f in self.k.iter_mut() {
            f.truncate(wirklich.saturating_mul(bk));
        }
        for f in self.v.iter_mut() {
            f.truncate(wirklich.saturating_mul(bv));
        }
        wirklich
    }

    /// **Der Zustandsspeicher, bei Bedarf angelegt.**
    ///
    /// ⚑ **Traege und nicht im Konstruktor**, weil erst das Modell die
    /// Masse kennt und weil ein rein achtsames Modell nichts belegen
    /// soll.
    pub fn zustand_bereit(
        &mut self,
        rekurrent: &[bool],
        koepfe: usize,
        schluessel_dim: usize,
        wert_dim: usize,
        kanaele: usize,
    ) -> &mut crate::zustandsspeicher::Zustandsspeicher {
        self.zustand.get_or_insert_with(|| {
            crate::zustandsspeicher::Zustandsspeicher::neu(
                rekurrent, koepfe, schluessel_dim, wert_dim, kanaele,
            )
        })
    }

    /// Der Zustandsspeicher, falls angelegt.
    pub fn zustand_mut(&mut self) -> Option<&mut crate::zustandsspeicher::Zustandsspeicher> {
        self.zustand.as_mut()
    }

    /// Wie viele Positionen in der ersten Ebene und im ersten Kopf stehen.
    pub fn laenge(&self) -> usize {
        if self.k.is_empty() { 0 } else { self.positionen(0) }
    }

    /// Wie viele Bytes Keys und Values belegen.
    pub fn belegte_bytes(&self) -> usize {
        self.k.iter().chain(self.v.iter()).map(|f| f.len() * 2).sum()
    }

    pub fn truncate(&mut self, layer: usize, head: usize, max_len: usize) {
        let fach = self.fach(layer, head);
        let (bk, bv) = (self.breite_k, self.breite_v);
        self.k[fach].truncate(max_len.saturating_mul(bk));
        self.v[fach].truncate(max_len.saturating_mul(bv));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`kuerzen` laesst genau die Positionen unter der Laenge stehen**,
    /// in jeder Ebene und jedem Kopf, Keys wie Values.
    ///
    /// Die Fortsetzungspruefung im Lader faengt einen Eintrag zu viel nicht:
    /// Die Vorbereitung ueberschreibt die Position hinter dem gemeinsamen
    /// Anfang ohnehin. Wer den Speicher anders kuerzt, etwa beim
    /// Verdichten eines Gespraechs, verlaesst sich aber auf die Grenze.
    #[test]
    fn kuerzen_laesst_genau_die_vorderen_positionen_stehen() {
        let mut cache = KVCache::for_range(2, 4, 3);
        for ebene in 2..4 {
            for kopf in 0..3 {
                for pos in 0..6 {
                    cache.write(ebene, kopf, pos, &[pos as i16], &[-(pos as i16)]);
                }
            }
        }
        assert_eq!(cache.laenge(), 6);
        cache.kuerzen(4);
        assert_eq!(cache.laenge(), 4);
        for ebene in 2..4 {
            for kopf in 0..3 {
                // `read_scheiben` liest Keys und Values je fuer sich; `read`
                // naehme die Positionen der Keys auch fuer die Values.
                let (k, v) = cache.read_scheiben(ebene, kopf, usize::MAX);
                let erwartet_k: Vec<Vec<i16>> = (0..4).map(|p| vec![p as i16]).collect();
                let erwartet_v: Vec<Vec<i16>> = (0..4).map(|p| vec![-(p as i16)]).collect();
                assert_eq!(k, erwartet_k.iter().map(Vec::as_slice).collect::<Vec<_>>(), "Ebene {ebene} Kopf {kopf}");
                assert_eq!(v, erwartet_v.iter().map(Vec::as_slice).collect::<Vec<_>>(), "Ebene {ebene} Kopf {kopf}");
            }
        }
        cache.kuerzen(9);
        assert_eq!(cache.laenge(), 4, "laenger kuerzen aendert nichts");
        cache.kuerzen(0);
        assert_eq!(cache.laenge(), 0);
        assert_eq!(KVCache::new(0, 0).laenge(), 0, "ohne Ebenen ist der Speicher leer");
    }

    /// **Ueberschreiben ersetzt, Anhaengen verlaengert, und eine Luecke
    /// bricht ab.** Die Karte davor liess eine Luecke stillschweigend zu.
    #[test]
    fn schreiben_ueberschreibt_und_haengt_an() {
        let mut cache = KVCache::for_range(1, 2, 2);
        cache.write(1, 1, 0, &[1, 2], &[3]);
        cache.write(1, 1, 1, &[4, 5], &[6]);
        cache.write(1, 1, 0, &[7, 8], &[9]);
        let (k, v) = cache.lesen(1, 1, 5);
        assert_eq!((k, v), (&[7i16, 8, 4, 5][..], &[9i16, 6][..]));
        assert_eq!(cache.lesen(1, 1, 0), (&[7i16, 8][..], &[9i16][..]), "bis einschliesslich upto");
        assert_eq!(cache.lesen(1, 0, 9), (&[][..], &[][..]), "der andere Kopf ist leer");
        assert_eq!(cache.belegte_bytes(), 12);
        let luecke = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut c = KVCache::for_range(1, 2, 2);
            c.write(1, 0, 1, &[1, 2], &[3]);
        }));
        assert!(luecke.is_err(), "eine Position hinter dem Ende muss abbrechen");
    }
}
