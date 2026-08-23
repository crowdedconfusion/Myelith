//! Probe: Wie viel Durchsatz liegt in der Parallelisierung, und was
//! kostet der Thread-Start?
//!
//! Zeilen einer Matrix sind voneinander unabhaengig: Jede ist ein
//! eigenes Skalarprodukt, das in sein eigenes Ausgabefeld schreibt. Eine
//! Aufteilung ueber Threads ist deshalb **bitgleich per Konstruktion**,
//! unabhaengig von der Threadzahl und der Reihenfolge.
//!
//! Gemessen wird an der groessten Matrix des Projekts (7B: 18944 x 3584).

use std::time::Instant;

fn dot(row: &[i8], x: &[i16]) -> i64 {
    row.iter().zip(x).map(|(&w, &v)| w as i64 * v as i64).sum()
}

fn einkernig(w: &[i8], x: &[i16], zeilen: usize, spalten: usize) -> Vec<i64> {
    w.chunks_exact(spalten).take(zeilen).map(|r| dot(r, x)).collect()
}

fn mehrkernig(w: &[i8], x: &[i16], zeilen: usize, spalten: usize, threads: usize) -> Vec<i64> {
    let mut out = vec![0i64; zeilen];
    let je = zeilen.div_ceil(threads);
    std::thread::scope(|s| {
        for (t, teil) in out.chunks_mut(je).enumerate() {
            let w = &w;
            s.spawn(move || {
                let start = t * je;
                for (i, ziel) in teil.iter_mut().enumerate() {
                    let z = start + i;
                    *ziel = dot(&w[z * spalten..(z + 1) * spalten], x);
                }
            });
        }
    });
    out
}

fn main() {
    let (zeilen, spalten) = (18944usize, 3584usize);
    println!("Matrix {zeilen} x {spalten} (groesste des 7B-Modells)\n");

    let w: Vec<i8> = (0..zeilen * spalten).map(|i| ((i * 31) % 255) as i8).collect();
    let x: Vec<i16> = (0..spalten).map(|i| ((i * 17) % 4096) as i16 - 2048).collect();

    let t0 = Instant::now();
    let a = einkernig(&w, &x, zeilen, spalten);
    let ein = t0.elapsed();
    println!("  1 Thread   {:>8.1} ms", ein.as_secs_f64() * 1000.0);

    for threads in [2usize, 4, 5, 8, 15] {
        let t0 = Instant::now();
        let b = mehrkernig(&w, &x, zeilen, spalten, threads);
        let d = t0.elapsed();
        assert_eq!(a, b, "{threads} Threads liefern ein anderes Ergebnis");
        println!("  {threads:>2} Threads  {:>8.1} ms   {:>5.2}x   bitgleich: ja",
                 d.as_secs_f64() * 1000.0, ein.as_secs_f64() / d.as_secs_f64());
    }

    // Was kostet der Start allein, je Threadzahl?
    println!("\n  Kosten des Thread-Starts:");
    for n in [2usize, 4, 5, 8, 15] {
        let t0 = Instant::now();
        for _ in 0..200 {
            std::thread::scope(|s| { for _ in 0..n { s.spawn(|| {}); } });
        }
        println!("    {n:>2} Threads: {:>6.1} us je Aufruf",
                 t0.elapsed().as_secs_f64() * 1e6 / 200.0);
    }

    // Und an den echten Matrixgroessen des 0,5B-Modells.
    println!("\n  Die Matrizen von 0,5B, je Aufruf:");
    for (name, zeilen, spalten) in [
        ("q/o_proj", 896usize, 896usize),
        ("gate/up_proj", 4864, 896),
        ("down_proj", 896, 4864),
    ] {
        let w: Vec<i8> = (0..zeilen * spalten).map(|i| ((i * 31) % 255) as i8).collect();
        let x: Vec<i16> = (0..spalten).map(|i| ((i * 17) % 4096) as i16 - 2048).collect();
        let t0 = Instant::now();
        for _ in 0..20 { std::hint::black_box(einkernig(&w, &x, zeilen, spalten)); }
        let ein = t0.elapsed().as_secs_f64() * 1e6 / 20.0;
        print!("    {name:<14} {zeilen:>5}x{spalten:<5} einkernig {ein:>7.1} us");
        for n in [4usize, 8, 15] {
            let t0 = Instant::now();
            for _ in 0..20 { std::hint::black_box(mehrkernig(&w, &x, zeilen, spalten, n)); }
            let d = t0.elapsed().as_secs_f64() * 1e6 / 20.0;
            print!("   {n}T {d:>6.1} us ({:.2}x)", ein / d);
        }
        println!();
    }
}
