//! **Was eine Zustandsebene beitraegt**, als Zahlen zum Nachrechnen.
//!
//! # ⛔️ Wozu
//!
//! Am 2026-09-22 stimmte der Entwurf des Rechenwegs nachweislich (jede
//! Stufe der Ebene 0 gegen die Fremdimplementierung, groesster relativer
//! Fehler 0,027), und das Modell erzeugte trotzdem Kauderwelsch. Damit
//! ist der wahrscheinlichste Verdaechtige die Abweichung des **Kerns**
//! von diesem Entwurf.
//!
//! ⚑ **Dieses Werkzeug gibt den Eingang und den Beitrag des Mischers
//! aus**, damit `tests/diag/zustandsebene_stufenvergleich.py` beides
//! gegen seine eigene Rechnung halten kann. Der Mitschnitt liefert
//! `norm_ein` und `residual_mitte`; die Differenz zum Residualstrom ist
//! der Beitrag.
//!
//! Aufruf:
//!     zustandsprobe <artefakt> <token-id> [ebene]

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::Zwischenwerte;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: zustandsprobe <artefakt> <token-id> [ebene]");
        std::process::exit(2);
    }
    // ⚑ **Mehrere Token, durch Komma getrennt.**
    //
    // ⛔️ Bis zum 2026-09-22 nahm dieses Werkzeug genau ein Token, und
    // damit war es fuer die Zustandsschicht **blind**: Bei leerem Zustand
    // und Position 0 hat sie nichts zu lesen, ihr Ausgang ist
    // `(k . q) * v * beta` mit einem Median von 2e-6, und die Referenz
    // liefert dort dasselbe Nichts.
    //
    // 📌 **Eine Messung am trivialen Fall prueft den trivialen Fall.**
    // Fuenf Laeufe an Position 0 haben nichts ueber die Rekurrenz
    // gesagt, und ich habe daraus trotzdem geschlossen.
    let token: Vec<usize> = args[2]
        .split(',')
        .map(|s| s.trim().parse().expect("token-id"))
        .collect();
    let ebene: usize = args.get(3).map(|s| s.parse().expect("ebene")).unwrap_or(0);

    let m = load_model(std::path::Path::new(&args[1])).expect("Modell laedt");
    let mut cache = KVCache::new(m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::default();
    // ⚠️ **Token fuer Token, damit der Zustand mitwaechst.** Die letzte
    // Aufzeichnung gehoert zum letzten Token, und nur die ist
    // aussagekraeftig.
    for (pos, &tk) in token.iter().enumerate() {
        auf.leeren();
        let hidden = m.embed_token(tk);
        let _ = m.run_layers_mit_mitschnitt(hidden, pos, &mut cache, 0, ebene + 1, &mut auf);
    }

    // ⚑ **Ein Lauf, alle Ebenen.** Der Mitschnitt haelt jede Ebene bis
    //   zur Grenze; eine Zeile je Ebene sagt, wo der Residualstrom
    //   seine Groesse verliert, ohne vierzig Modellladungen.
    if std::env::var_os("MYL_ALLE_EBENEN").is_some() {
        println!("{{ \"ebenen\": [");
        let n = auf.ebenen().len();
        for (i, e) in auf.ebenen().iter().enumerate() {
            let feld = |v: &[i16]| -> String {
                v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
            };
            println!(
                "  {{ \"ebene\": {i}, \"residual_ein\": [{}], \"residual_mitte\": [{}], \"norm_ein\": [{}] }}{}",
                feld(&e.residual_ein),
                feld(&e.residual_mitte),
                feld(&e.norm_ein),
                if i + 1 < n { "," } else { "" }
            );
        }
        println!("] }}");
        return;
    }

    let e = auf
        .ebenen()
        .get(ebene)
        .expect("die Ebene wurde aufgezeichnet");
    // ⚑ Als JSON, damit die Python-Seite es ohne Parser liest.
    println!("{{");
    println!("  \"ebene\": {ebene}, \"token\": {:?}, \"position\": {},", token, token.len() - 1);
    // ⚑ **Auf einer Achtsamkeitsebene ist `attn_aus` das, worauf es
    //   ankommt**: der Zweigausgang vor `o_proj`. Mit dem `norm_ein`
    //   derselben Aufzeichnung als Eingang laesst sich der Block
    //   gegen die Referenz halten, ohne die Ebenen davor zu rechnen.
    println!("  \"koepfe_q\": {}, \"kopfbreite_q\": {},",
        e.q.len(), e.q.first().map(|h| h.len()).unwrap_or(0));
    println!("  \"koepfe_k\": {}, \"kopfbreite_k\": {},",
        e.k.len(), e.k.first().map(|h| h.len()).unwrap_or(0));
    for (name, feld) in [
        ("residual_ein", &e.residual_ein),
        ("norm_ein", &e.norm_ein),
        ("attn_aus", &e.attn_aus),
        ("residual_mitte", &e.residual_mitte),
    ] {
        let zahlen: Vec<String> = feld.iter().map(|v| v.to_string()).collect();
        println!("  \"{name}\": [{}],", zahlen.join(","));
    }
    println!("  \"ende\": true");
    println!("}}");
}
