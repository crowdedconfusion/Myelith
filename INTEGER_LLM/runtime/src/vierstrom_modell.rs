//! **Der Rechenweg der Bauart mit vier Stroemen** (Qwen3.8-Flash-Next, seit
//! dem 2026-10-19).
//!
//! # Was anders ist als bei jedem Modell davor
//!
//! - **Vier Stroeme statt eines Residualstroms.** Der Zustand eines Tokens
//!   ist `S * H` breit (`S` Stroeme zu `H`). Vor dem Mischer und vor dem
//!   Gemisch jeder Ebene steht ein [`Torblock`]: Er normiert jeden Strom,
//!   mischt sie ueber eine Niedrigrangprojektion zu einem Eingang der Breite
//!   `H` und gibt je Strom einen Faktor, mit dem der Ausgang in diesen Strom
//!   zurueckfliesst. **Eine Norm vor Mischer oder Gemisch gibt es nicht**,
//!   und am Ende keine Schlussnorm, sondern einen letzten Torblock.
//! - **Eine N-Gramm-Einbettung** ([`Ngrammeinbettung`]) in einer Ebene: 16
//!   Tabellenzeilen je Token aus Bi- und Trigrammen, ein Tor je Strom und eine
//!   Faltung, die jede dritte Position liest.
//! - **Eine Blockauswahl** ([`Blockauswahl`]) in den Aufmerksamkeitsebenen:
//!   Ab einer bestimmten Laenge sieht jede Abfrage nur die besten Viererbloecke
//!   und den Rest am Ende.
//!
//! ⚑ **Mischer und Gemisch sind dieselben wie beim 35B**, Zeile fuer Zeile:
//! Die Achtsamkeit, die Zustandsschicht und das Expertengemisch bekommen hier
//! ihren Eingang auf einer kalibrierten Skala und liefern ihren Ausgang auf
//! einer Skala je Kanal. Neu ist, was davor und dahinter steht.
//!
//! ⚑ **Ganzzahlig von vorn bis hinten.** Die Vorlage rechnet die Mittel ueber
//! die Stroeme, die Wurzel des N-Gramm-Tors und die Blockwertung in float32;
//! hier ist jede davon eine ganze Zahl mit festgelegter Rundung.

use super::*;
use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64, rshift_round_i64};
use integer_llm_kernels::integer_math::{sigmoid_nachschlagen, silu_zerlegt, torfaktor};
use integer_llm_kernels::rmsnorm::{inv_n_q20, rmsnorm_i16_mit_eps};
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_kernels::vierstrom::{blockauswahl, blockmittel, blockwertung, vorzeichenwurzel, Abstandsfaltung, Ngrammmasse};

/// **Ein Torblock**: Norm je Strom, Mischen ueber die Stroeme, und (vor
/// Mischer und Gemisch) die Faktoren, mit denen der Ausgang zurueckfliesst.
pub struct Torblock {
    /// Gamma der Norm, `S * H` lang (der Versatz `1 + w` steckt schon drin).
    pub norm: QTensor,
    /// `[R, S*H]` und `[S*H, R]`: die Niedrigrangprojektion der Mischgewichte.
    pub ab: QTensor,
    pub auf: QTensor,
    /// `[S, S*H]`: die Faktoren je Strom. `None` beim letzten Block.
    pub einspeisung: Option<QTensor>,
    /// Die Skala des Zustands vor diesem Block, je Kanal (`S * H`).
    pub ein_fracs: Vec<u8>,
    pub norm_frac: u8,
    pub ab_frac: u8,
    /// Eingang von `auf`, also `silu(ab / S)`.
    pub auf_ein_frac: u8,
    pub auf_frac: u8,
    pub einspeisung_frac: u8,
    /// Der gemischte Eingang von Mischer bzw. Gemisch bzw. Kopf.
    pub aus_frac: u8,
}

/// **Die N-Gramm-Einbettung einer Ebene.**
pub struct Ngrammeinbettung {
    pub masse: Ngrammmasse,
    pub eos: u64,
    /// `[zeilen, kopfbreite]`, int8 mit einer Skala je Zeile; eingeblendet.
    pub tabelle: QTensor,
    pub kopfbreite: usize,
    /// `[S*H, E]` und `[H, E]`.
    pub schluessel: QTensor,
    pub wert: QTensor,
    /// Die drei Normen, je `S * H` lang.
    pub norm_schluessel: QTensor,
    pub norm_abfrage: QTensor,
    pub norm_faltung: QTensor,
    /// `[S*H, kern]`, tiefenweise.
    pub faltung: QTensor,
    pub kern: usize,
    pub abstand: usize,
    /// `round(2^16 / H^(1/4))`, fuer das Tor (siehe [`vorzeichenwurzel`]).
    pub c_q16: u64,
    /// Skala des Zustands vor der Einbettung, je Kanal (`S * H`).
    pub ein_fracs: Vec<u8>,
    pub e_frac: u8,
    pub schluessel_frac: u8,
    pub schluessel_norm_frac: u8,
    pub wert_frac: u8,
    pub abfrage_norm_frac: u8,
    pub faltung_norm_frac: u8,
    pub faltung_frac: u8,
}

/// **Die Blockauswahl einer Aufmerksamkeitsebene.**
pub struct Blockauswahl {
    /// `[(koepfe + 1) * kopf_dim, H]`: erst die Abfragekoepfe, dann ein
    /// Schluessel.
    pub qk: QTensor,
    pub q_norm: QTensor,
    pub k_norm: QTensor,
    pub koepfe: usize,
    pub kopf_dim: usize,
    /// Wie viele Bloecke eine Abfrage sieht (`budget / verdichtung`).
    pub bloecke: usize,
    /// Token je Block.
    pub verdichtung: usize,
    pub q_frac: u8,
    pub k_frac: u8,
    pub q_norm_frac: u8,
    pub k_norm_frac: u8,
}

/// Was eine Ebene dieser Bauart mehr hat als die Ebene selbst.
pub struct Vierstromebene {
    pub ngramm: Option<Ngrammeinbettung>,
    pub vor_mischer: Torblock,
    /// Skala des Mischerausgangs je Kanal (`H`).
    pub mischer_aus_fracs: Vec<u8>,
    pub blockauswahl: Option<Blockauswahl>,
    pub vor_gemisch: Torblock,
    pub gemisch_aus_fracs: Vec<u8>,
}

/// **Alles, was ein Modell dieser Bauart mehr hat**, einmal am Modell.
pub struct Vierstrom {
    pub stroeme: usize,
    pub ebenen: Vec<Vierstromebene>,
    pub schluss: Torblock,
    /// `2^20 / H`, fuer die Norm je Strom.
    pub inv_n_strom: i64,
}

impl Vierstrom {
    /// Die Skala des Zustands am Eingang von Ebene `i` (je Kanal), oder am
    /// Schlussblock fuer `i == Ebenen`.
    pub fn eingang(&self, i: usize) -> &[u8] {
        match self.ebenen.get(i) {
            Some(e) => match &e.ngramm {
                Some(n) => &n.ein_fracs,
                None => &e.vor_mischer.ein_fracs,
            },
            None => &self.schluss.ein_fracs,
        }
    }

    fn log2_stroeme(&self) -> u8 {
        self.stroeme.trailing_zeros() as u8
    }
}

/// Der Zustand der N-Gramm-Einbettung ueber die Folge: die Vorgaenger des
/// naechsten Tokens und das Fenster der Faltung.
#[derive(Clone, Debug)]
pub struct Ngrammzustand {
    pub kontext: Vec<u64>,
    pub faltung: Abstandsfaltung,
}

impl IntegerModel {
    fn vierstrom_teile(&self) -> &Vierstrom {
        self.vierstrom.as_deref().expect("dieses Modell hat keine vier Stroeme")
    }

    /// **Norm je Strom**: jeder der `S` Abschnitte fuer sich, mit seinem Teil
    /// der Skalen und des Gammas.
    ///
    /// ⛔️ **Mit Epsilon, wie jede Norm dieser Bauart.** Am Zufallsmodell lag
    /// `sigmoid(g) * v` vor der Faltungsnorm bei 0,005; das mittlere Quadrat
    /// war damit von der Groessenordnung `eps = 1e-6`, und ohne es wich die
    /// Norm um 30 % ab (2026-10-19). 📌 Dieselbe Lehre wie Fund 419: Eine
    /// Annahme ueber Groessenordnungen gilt fuer den Zweig, fuer den sie
    /// geprueft wurde.
    fn norm_je_strom(&self, x: &[i16], x_fracs: &[u8], gamma: &QTensor, aus_frac: u8) -> Vec<i16> {
        let vs = self.vierstrom_teile();
        let h = self.hidden_size;
        let cfg = &self.config;
        assert_eq!(x.len(), vs.stroeme * h, "Norm je Strom: Breite passt nicht");
        let mut aus = Vec::with_capacity(x.len());
        for s in 0..vs.stroeme {
            let r = s * h..(s + 1) * h;
            aus.extend(rmsnorm_i16_mit_eps(
                &x[r.clone()],
                &x_fracs[r.clone()],
                &gamma.data[r.clone()],
                &gamma.shifts[r],
                &self.rsqrt_lut,
                cfg.rsqrt_input_shift,
                cfg.rsqrt_output_frac,
                vs.inv_n_strom,
                aus_frac,
                self.norm_eps_q40,
            ));
        }
        aus
    }

    /// **Ein Torblock**: der gemischte Eingang auf `aus_frac`, und die Faktoren
    /// je Strom auf `sigmoid_aus_frac - 1` (der Wert ist `2 * sigmoid`).
    pub(super) fn torblock(&self, tb: &Torblock, x: &[i16]) -> (Vec<i16>, Option<Vec<i64>>) {
        let vs = self.vierstrom_teile();
        let (h, s_n, ls) = (self.hidden_size, vs.stroeme, vs.log2_stroeme());
        let xn = self.norm_je_strom(x, &tb.ein_fracs, &tb.norm, tb.norm_frac);

        // `silu(ab(xn) / S)`: die Teilung ist eine Skala, kein Rechenschritt.
        // ⛔️ **`silu_zerlegt` und nicht die Silu-Tabelle** (Fund 417): Die
        //   Tabelle rastert ihren Eingang auf 1/64, und hier kommen Werte um
        //   0,05 an. Am Zufallsmodell wich schon der erste gemischte Eingang
        //   um 1,3 % ab, mit identischen Gewichten.
        let d = projektion_fuer_alle(&[&xn], &tb.ab, tb.norm_frac, tb.ab_frac).pop().expect("eine Eingabe");
        let s_val: Vec<i16> = d
            .iter()
            .map(|&v| {
                clamp_i16_from_i64(silu_zerlegt(
                    i64::from(v), tb.ab_frac + ls, &self.sigmoid_lut, self.sigmoid_versatz,
                    self.sigmoid_ein_frac, self.sigmoid_aus_frac, tb.auf_ein_frac,
                ))
            })
            .collect();
        let u = projektion_fuer_alle(&[&s_val], &tb.auf, tb.auf_ein_frac, tb.auf_frac).pop().expect("eine Eingabe");

        // Mittel ueber die Stroeme von `sigmoid(u) * xn`, in i64.
        let mut ein = Vec::with_capacity(h);
        for j in 0..h {
            let mut summe = 0i64;
            for s in 0..s_n {
                let c = s * h + j;
                let g = torfaktor(u[c], tb.auf_frac, &self.sigmoid_lut, self.sigmoid_versatz, self.sigmoid_ein_frac, self.sigmoid_aus_frac);
                summe += g * i64::from(xn[c]);
            }
            ein.push(clamp_i16_from_i64(rescale_i64(summe, self.sigmoid_aus_frac + tb.norm_frac + ls, tb.aus_frac)));
        }

        let faktoren = tb.einspeisung.as_ref().map(|einsp| {
            let roh = projektion_fuer_alle(&[&xn], einsp, tb.norm_frac, tb.einspeisung_frac).pop().expect("eine Eingabe");
            roh.iter()
                .map(|&v| {
                    let dom = rescale(i32::from(v), tb.einspeisung_frac + ls, self.sigmoid_ein_frac);
                    // `2 * sigmoid`: derselbe Tabellenwert, eine Stelle groeber gelesen.
                    sigmoid_nachschlagen(dom, &self.sigmoid_lut, self.sigmoid_versatz, self.sigmoid_ein_frac, self.sigmoid_aus_frac)
                })
                .collect()
        });
        (ein, faktoren)
    }

    /// **`X[s] + t[s] * aus`, in alle Stroeme**, mit einer Rundung je Kanal.
    ///
    /// ⚑ Beide Summanden werden exakt auf die feinste der beteiligten Skalen
    /// gehoben (nur Linksshifts), addiert und **einmal** auf die Zielskala
    /// gebracht (Fund 31: nie einzeln klemmen, dann addieren).
    pub(super) fn einspeisen(&self, x: &[i16], x_fracs: &[u8], t: &[i64], aus: &[i16], aus_fracs: &[u8], ziel: &[u8]) -> Vec<i16> {
        let vs = self.vierstrom_teile();
        let h = self.hidden_size;
        let t_frac = self.sigmoid_aus_frac - 1;
        let mut neu = Vec::with_capacity(x.len());
        for s in 0..vs.stroeme {
            for j in 0..h {
                let c = s * h + j;
                let a_frac = t_frac + aus_fracs[j];
                let f = x_fracs[c].max(a_frac).max(ziel[c]);
                let a = i64::from(x[c]) << (f - x_fracs[c]);
                let b = (t[s] * i64::from(aus[j])) << (f - a_frac);
                neu.push(clamp_i16_from_i64(rescale_i64(a + b, f, ziel[c])));
            }
        }
        neu
    }

    /// **Der Beitrag der N-Gramm-Einbettung, schon addiert**: `X + gv +
    /// silu(faltung(norm(gv)))`, auf der Skala des folgenden Torblocks.
    fn ngramm_addieren(&self, ne: &Ngrammeinbettung, ebene: usize, x: &[i16], token: usize, cache: &mut KVCache, ziel: &[u8]) -> Vec<i16> {
        let vs = self.vierstrom_teile();
        let (h, s_n) = (self.hidden_size, vs.stroeme);
        let breite = s_n * h;
        let zustand = self.ngrammzustand(cache, ebene, ne);
        let kennungen = ne.masse.kennungen(token as u64, &zustand.kontext);

        // Die Zeilen der Tabelle, aneinander, auf `e_frac`.
        let mut e = Vec::with_capacity(kennungen.len() * ne.kopfbreite);
        for &k in &kennungen {
            let zeile = k as usize;
            let w = ne.tabelle.row(zeile);
            let shift = ne.tabelle.shifts[zeile];
            e.extend(w.iter().map(|&v| clamp_i16_from_i64(rescale_i64(i64::from(v), shift, ne.e_frac))));
        }
        let spos = zustand_laenge(cache);
        spur(spos, ebene, "ple_e", &e, &[ne.e_frac]);
        let k_roh = projektion_fuer_alle(&[&e], &ne.schluessel, ne.e_frac, ne.schluessel_frac).pop().expect("eine Eingabe");
        let kn = self.norm_je_strom(&k_roh, &vec![ne.schluessel_frac; breite], &ne.norm_schluessel, ne.schluessel_norm_frac);
        let v = projektion_fuer_alle(&[&e], &ne.wert, ne.e_frac, ne.wert_frac).pop().expect("eine Eingabe");
        let qn = self.norm_je_strom(x, &ne.ein_fracs, &ne.norm_abfrage, ne.abfrage_norm_frac);
        spur(spos, ebene, "ple_kn", &kn, &[ne.schluessel_norm_frac]);
        spur(spos, ebene, "ple_v", &v, &[ne.wert_frac]);
        spur(spos, ebene, "ple_qn", &qn, &[ne.abfrage_norm_frac]);

        // Das Tor je Strom: sigmoid(sign(g) sqrt(|g| / sqrt(H))).
        let g_frac = u32::from(ne.schluessel_norm_frac) + u32::from(ne.abfrage_norm_frac);
        let mut gv = Vec::with_capacity(breite);
        for s in 0..s_n {
            let r = s * h..(s + 1) * h;
            let g: i128 = kn[r.clone()].iter().zip(&qn[r]).map(|(&a, &b)| i128::from(a) * i128::from(b)).sum();
            let dom = vorzeichenwurzel(g, g_frac, ne.c_q16, u32::from(self.sigmoid_ein_frac));
            let sig = sigmoid_nachschlagen(dom, &self.sigmoid_lut, self.sigmoid_versatz, self.sigmoid_ein_frac, self.sigmoid_aus_frac);
            gv.extend(v.iter().map(|&w| clamp_i16_from_i64(rshift_round_i64(sig * i64::from(w), self.sigmoid_aus_frac))));
        }
        let n = self.norm_je_strom(&gv, &vec![ne.wert_frac; breite], &ne.norm_faltung, ne.faltung_norm_frac);
        spur(spos, ebene, "ple_gv", &gv, &[ne.wert_frac]);
        spur(spos, ebene, "ple_n", &n, &[ne.faltung_norm_frac]);

        // Faltung und Kontext schreiten fort; der Zustand gehoert der Folge.
        let zustand = self.ngrammzustand(cache, ebene, ne);
        let gefaltet = zustand.faltung.schritt(&n, ne.faltung_norm_frac, &ne.faltung.data, &ne.faltung.shifts, ne.faltung_frac);
        zustand.kontext = Ngrammmasse::weiter(&zustand.kontext, token as u64, ne.eos);
        let g16: Vec<i16> = gefaltet.iter().map(|&v| clamp_i16_from_i64(v)).collect();
        spur(spos, ebene, "ple_faltung", &g16, &[ne.faltung_frac]);

        // X + gv + silu(gefaltet), exakt auf der feinsten Skala, einmal gerundet.
        // ⚑ Silu wieder zerlegt (Fund 417), und gleich auf die Summenskala.
        let mut neu = Vec::with_capacity(breite);
        for c in 0..breite {
            let f = ne.ein_fracs[c].max(ne.wert_frac).max(ne.faltung_frac).max(ziel[c]);
            let a = silu_zerlegt(
                gefaltet[c], ne.faltung_frac, &self.sigmoid_lut, self.sigmoid_versatz,
                self.sigmoid_ein_frac, self.sigmoid_aus_frac, f,
            );
            let summe = (i64::from(x[c]) << (f - ne.ein_fracs[c])) + (i64::from(gv[c]) << (f - ne.wert_frac)) + a;
            neu.push(clamp_i16_from_i64(rescale_i64(summe, f, ziel[c])));
        }
        neu
    }

    /// Der Zustand der N-Gramm-Einbettung einer Ebene, bei Bedarf angelegt.
    fn ngrammzustand<'c>(&self, cache: &'c mut KVCache, ebene: usize, ne: &Ngrammeinbettung) -> &'c mut Ngrammzustand {
        let masse = self.zustandsmasse.as_ref().expect("die Bauart mit vier Stroemen hat rekurrente Ebenen");
        let rekurrent: Vec<bool> = self.layers.iter().map(|l| l.ist_rekurrent()).collect();
        let speicher = cache.zustand_bereit(&rekurrent, masse.wert_koepfe, masse.schluessel_dim, masse.wert_dim, masse.kanaele);
        let breite = self.vierstrom_teile().stroeme * self.hidden_size;
        speicher.ngramm_bereit(ebene, || Ngrammzustand {
            kontext: vec![ne.eos; ne.masse.ngramm - 1],
            faltung: Abstandsfaltung::leer(breite, ne.kern, ne.abstand),
        })
    }

    /// **Welche Positionen die Abfrage sieht**, oder `None` fuer alle.
    ///
    /// Schreibt den rohen Indexschluessel dieser Position und, wenn damit ein
    /// Block voll wird, dessen Schluessel.
    fn blockauswahl_rechnen(&self, ba: &Blockauswahl, ebene: usize, ein: &[i16], ein_frac: u8, pos: usize, cache: &mut KVCache) -> Option<Vec<usize>> {
        let cfg = &self.config;
        let (d, nq, r) = (ba.kopf_dim, ba.koepfe * ba.kopf_dim, ba.verdichtung);
        let mut aus_fracs = vec![ba.q_frac; nq];
        aus_fracs.extend(std::iter::repeat_n(ba.k_frac, d));
        let qk = projektion_pc_fuer_alle(&[ein], &ba.qk, ein_frac, &aus_fracs).pop().expect("eine Eingabe");
        cache.index_schreiben(ebene, pos, &qk[nq..]);

        let half = self.drehbreite / 2;
        let inv_n = inv_n_q20(d);
        let n = pos + 1;
        let voll = n / r;
        if voll > cache.index_bloecke(ebene) {
            let b = voll - 1;
            let roh: Vec<&[i16]> = (b * r..(b + 1) * r).map(|p| cache.index_roh(ebene, p)).collect();
            let mittel = blockmittel(&roh);
            let normiert = rmsnorm_i16_mit_eps(&mittel, &vec![ba.k_frac; d], &ba.k_norm.data, &ba.k_norm.shifts, &self.rsqrt_lut, cfg.rsqrt_input_shift, cfg.rsqrt_output_frac, inv_n, ba.k_norm_frac, self.norm_eps_q40);
            let start = b * r;
            let gedreht = rotate_half_split_i16(&normiert, &self.cos_lut[start * half..(start + 1) * half], &self.sin_lut[start * half..(start + 1) * half], cfg.rope_frac_bits);
            cache.index_block_schreiben(ebene, b, &gedreht);
        }
        if voll <= ba.bloecke {
            return None;
        }
        let mut q = Vec::with_capacity(nq);
        for k in 0..ba.koepfe {
            let normiert = rmsnorm_i16_mit_eps(&qk[k * d..(k + 1) * d], &vec![ba.q_frac; d], &ba.q_norm.data, &ba.q_norm.shifts, &self.rsqrt_lut, cfg.rsqrt_input_shift, cfg.rsqrt_output_frac, inv_n, ba.q_norm_frac, self.norm_eps_q40);
            q.extend(rotate_half_split_i16(&normiert, &self.cos_lut[pos * half..(pos + 1) * half], &self.sin_lut[pos * half..(pos + 1) * half], cfg.rope_frac_bits));
        }
        let wertungen: Vec<i64> = (0..voll).map(|b| blockwertung(&q, d, cache.index_block(ebene, b))).collect();
        let mut positionen = Vec::with_capacity(ba.bloecke * r + r);
        for b in blockauswahl(&wertungen, ba.bloecke) {
            positionen.extend(b * r..(b + 1) * r);
        }
        positionen.extend(voll * r..n);
        if std::env::var_os("MYL_VIERSTROM_SPUR").is_some() {
            eprintln!("{{\"auswahl\":{positionen:?},\"pos\":{pos},\"ebene\":{ebene}}}");
        }
        Some(positionen)
    }

    /// **Der Mischer einer Ebene dieser Bauart**: Achtsamkeit (mit
    /// Blockauswahl) oder Zustandsschicht, Ausgang auf `mischer_aus_fracs`.
    fn vierstrom_mischer(&self, layer: &TransformerLayer, ve: &Vierstromebene, ein: &[i16], pos: usize, cache: &mut KVCache) -> Vec<i16> {
        match &layer.mischer {
            Mischer::Zustand(zs) => self
                .zustandsmischer(layer, zs, &[ein], cache, &ve.mischer_aus_fracs)
                .pop()
                .expect("ein Token ergibt genau eine Ausgabe"),
            Mischer::Achtsamkeit(_) => {
                let [q_roh, k_flat, v_flat] =
                    self.projektionen_qkv(layer, &[ein]).pop().expect("eine Eingabe ergibt genau eine Projektion");
                let (q_flat, tor) = if self.achtsamkeit_mit_tor {
                    let (q, g) = abfrage_und_tor_trennen(&q_roh, self.num_heads, self.head_dim);
                    (q, Some(g))
                } else {
                    (q_roh, None)
                };
                let (q_heads, k_heads, v_heads) = self.koepfe_drehen(layer, &q_flat, &k_flat, &v_flat, pos);
                self.kv_schreiben(layer, &k_heads, &v_heads, pos, cache);
                let auswahl = ve
                    .blockauswahl
                    .as_ref()
                    .and_then(|ba| self.blockauswahl_rechnen(ba, layer.layer_idx, ein, layer.scales.norm_attn_frac, pos, cache));
                let attn = self.aufmerksamkeit_lesen_auswahl(layer, &q_heads, pos, cache, auswahl.as_deref());
                let attn = match tor {
                    Some(g) => self.tor_anwenden(&attn, &g, layer),
                    None => attn,
                };
                self.projektion_o(layer, &[attn.as_slice()], &ve.mischer_aus_fracs)
                    .pop()
                    .expect("eine Eingabe ergibt genau eine Projektion")
            }
        }
    }

    /// **Eine Ebene dieser Bauart fuer ein Token.** Ausgang auf `ziel`.
    #[allow(clippy::too_many_arguments)]
    fn vierstrom_ebene(
        &self,
        layer: &TransformerLayer,
        ve: &Vierstromebene,
        x: Vec<i16>,
        token: usize,
        pos: usize,
        cache: &mut KVCache,
        ziel: &[u8],
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> Vec<i16> {
        let i = layer.layer_idx;
        let x = match &ve.ngramm {
            Some(ne) => {
                let x = self.ngramm_addieren(ne, i, &x, token, cache, &ve.vor_mischer.ein_fracs);
                spur(pos, i, "nach_ngramm", &x, &ve.vor_mischer.ein_fracs);
                x
            }
            None => x,
        };
        let (ein, t) = self.torblock(&ve.vor_mischer, &x);
        spur(pos, i, "ein_mischer", &ein, &[ve.vor_mischer.aus_frac]);
        let o = self.vierstrom_mischer(layer, ve, &ein, pos, cache);
        spur(pos, i, "mischer", &o, &ve.mischer_aus_fracs);
        let x_mitte = self.einspeisen(&x, &ve.vor_mischer.ein_fracs, &t.expect("vor dem Mischer gibt es Faktoren"), &o, &ve.mischer_aus_fracs, &ve.vor_gemisch.ein_fracs);
        spur(pos, i, "mitte", &x_mitte, &ve.vor_gemisch.ein_fracs);

        let (ein2, t2) = self.torblock(&ve.vor_gemisch, &x_mitte);
        spur(pos, i, "ein_gemisch", &ein2, &[ve.vor_gemisch.aus_frac]);
        // ⚑ **Der Mitschnitt des Gemischs**, im Format des Trainings: der
        //   Eingang des Gemischs steht dort, wo bei einem Residualstrom die
        //   Norm vor dem MLP steht (`norm_mitte`). `expertenmitschnitt` und
        //   damit der Bau in vier Bit lesen genau das.
        if let Some(a) = auf.as_mut() {
            a.norm_mitte = ein2.clone();
        }
        let f = match &layer.ffn {
            Feedforward::Moe(moe) => {
                let mut spur_moe = auf.as_ref().map(|_| crate::mitschnitt::Moespur::default());
                let f = self.moe_vorwaerts(moe, &ein2, &layer.scales, &self.config, &ve.gemisch_aus_fracs, None, layer.layer_idx, spur_moe.as_mut());
                if let (Some(a), Some(sp)) = (auf.as_mut(), spur_moe) {
                    a.mlp = crate::mitschnitt::Mlpteil::Expertengemisch {
                        experten: sp.experten,
                        gewichte: sp.gewichte,
                        ausgaben: sp.ausgaben,
                        teile: sp.teile,
                        logits: sp.logits,
                    };
                }
                f
            }
            Feedforward::Dense(mlp) => self.mlp_vorwaerts(mlp, &ein2, &layer.scales, &self.config, &ve.gemisch_aus_fracs, None),
        };
        spur(pos, i, "gemisch", &f, &ve.gemisch_aus_fracs);
        let aus = self.einspeisen(&x_mitte, &ve.vor_gemisch.ein_fracs, &t2.expect("vor dem Gemisch gibt es Faktoren"), &f, &ve.gemisch_aus_fracs, ziel);
        spur(pos, i, "aus", &aus, ziel);
        aus
    }

    /// **Ein Token durch alle Ebenen und den Schlussblock**: der gemischte
    /// Eingang des Kopfes, auf `final_norm_frac`.
    pub(super) fn vierstrom_durch(&self, token: usize, pos: usize, cache: &mut KVCache) -> Vec<i16> {
        self.vierstrom_durch_mit(token, pos, cache, None)
    }

    /// **Die Logits eines Tokens samt Mitschnitt** des Gemischs je Ebene
    /// (`norm_mitte` und `mlp`), fuer `expertenmitschnitt` und den Bau in vier
    /// Bit. ⚑ Derselbe Weg wie [`IntegerModel::forward_token`], nur mit
    /// aufzeichnendem Gemisch.
    pub fn vierstrom_mit_mitschnitt(&self, token: usize, pos: usize, cache: &mut KVCache, auf: &mut crate::mitschnitt::Zwischenwerte) -> Vec<i32> {
        let ein = self.vierstrom_durch_mit(token, pos, cache, Some(auf));
        self.logits_aus_normiertem(&ein, self.config.logit_frac_bits)
    }

    fn vierstrom_durch_mit(&self, token: usize, pos: usize, cache: &mut KVCache, mut auf: Option<&mut crate::mitschnitt::Zwischenwerte>) -> Vec<i16> {
        let vs = self.vierstrom_teile();
        let h = self.hidden_size;
        let erste = vs.eingang(0);
        let emb = self.embedding_table.row(token);
        let emb_shift = self.embedding_table.shifts[token];
        let mut x: Vec<i16> = (0..vs.stroeme * h)
            .map(|c| clamp_i16(rescale(i32::from(emb[c % h]), emb_shift, erste[c])))
            .collect();
        for (i, layer) in self.layers.iter().enumerate() {
            let mut ebene_auf = auf.is_some().then(crate::mitschnitt::Ebenenmitschnitt::default);
            x = self.vierstrom_ebene(layer, &vs.ebenen[i], x, token, pos, cache, vs.eingang(i + 1), ebene_auf.as_mut());
            if let (Some(z), Some(e)) = (auf.as_deref_mut(), ebene_auf) {
                z.anhaengen(e);
            }
        }
        self.torblock(&vs.schluss, &x).0
    }

    /// **Die Logits eines Tokens** bei dieser Bauart.
    pub(super) fn vierstrom_token(&self, token: usize, pos: usize, cache: &mut KVCache) -> Vec<i32> {
        let ein = self.vierstrom_durch(token, pos, cache);
        self.logits_aus_normiertem(&ein, self.config.logit_frac_bits)
    }
}

/// **Die Spur zur Fehlersuche** (`MYL_VIERSTROM_SPUR`): eine JSON-Zeile je
/// Stufe mit den rohen Werten und ihren Skalen (eine je Kanal, oder eine fuer
/// alle). ⚑ Ohne die Variable kostet sie einen Blick in die Umgebung.
fn spur(pos: usize, ebene: usize, teil: &str, werte: &[i16], fracs: &[u8]) {
    if std::env::var_os("MYL_VIERSTROM_SPUR").is_some() {
        eprintln!("{{\"pos\":{pos},\"ebene\":{ebene},\"teil\":\"{teil}\",\"x\":{werte:?},\"fracs\":{fracs:?}}}");
    }
}

/// Die Position fuer die Spur: wie viele Token der Zustandsspeicher schon
/// gesehen hat (nur zur Fehlersuche).
fn zustand_laenge(cache: &mut KVCache) -> usize {
    cache.zustand_mut().map(|z| z.laenge()).unwrap_or(0)
}
