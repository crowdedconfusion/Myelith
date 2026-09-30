// Mikroprobe: wie viele sdot/udot/and/shift je Takt schafft ein Kern?
use std::arch::aarch64::*;
use std::arch::asm;
use std::time::Instant;

#[inline(always)]
unsafe fn sdot(acc: int32x4_t, a: int8x16_t, b: int8x16_t) -> int32x4_t {
    let mut r = acc;
    asm!("sdot {r:v}.4s, {a:v}.16b, {b:v}.16b", r = inout(vreg) r, a = in(vreg) a, b = in(vreg) b, options(pure, nomem, nostack, preserves_flags));
    r
}

#[target_feature(enable = "dotprod")]
unsafe fn dots(n: usize, ketten: usize, a: &[i8; 64], b: &[i8; 64]) -> i64 {
    let mut acc = [vdupq_n_s32(0); 16];
    let va = [vld1q_s8(a.as_ptr()), vld1q_s8(a.as_ptr().add(16)), vld1q_s8(a.as_ptr().add(32)), vld1q_s8(a.as_ptr().add(48))];
    let vb = [vld1q_s8(b.as_ptr()), vld1q_s8(b.as_ptr().add(16)), vld1q_s8(b.as_ptr().add(32)), vld1q_s8(b.as_ptr().add(48))];
    for _ in 0..n {
        for k in 0..ketten {
            acc[k] = sdot(acc[k], va[k & 3], vb[k & 3]);
        }
    }
    let mut s = 0i64;
    for k in 0..ketten { s += i64::from(vaddvq_s32(acc[k])); }
    s
}

unsafe fn ands(n: usize, ketten: usize, a: &[i8; 64]) -> i64 {
    let mut acc = [vdupq_n_u8(0xFF); 16];
    let va = [vld1q_u8(a.as_ptr() as *const u8), vld1q_u8((a.as_ptr() as *const u8).add(16))];
    for _ in 0..n {
        for k in 0..ketten {
            acc[k] = vaddq_u8(acc[k], va[k & 1]);
        }
    }
    let mut s = 0i64;
    for k in 0..ketten { s += i64::from(vaddvq_u8(acc[k])); }
    s
}

#[inline(never)]
fn kette(n: usize, mut x: u64) -> u64 {
    for _ in 0..n { unsafe { asm!("add {x}, {x}, #1", "add {x}, {x}, #3", x = inout(reg) x, options(pure, nomem, nostack)); } }
    x
}

fn main() {
    let a: [i8; 64] = std::array::from_fn(|i| (i as i8).wrapping_mul(7));
    let b: [i8; 64] = std::array::from_fn(|i| (i as i8).wrapping_mul(13));
    let n = 200_000_000usize;
    let t = Instant::now(); let x = kette(n, 1); let ns_takt = t.elapsed().as_secs_f64() * 1e9 / (2.0 * n as f64);
    println!("Takt: {:.4} ns ({:.2} GHz), x={x}", ns_takt, 1.0 / ns_takt);
    for ketten in [1usize, 2, 4, 8, 12, 16] {
        let n = 50_000_000usize;
        let t = Instant::now(); let s = unsafe { dots(n, ketten, &a, &b) };
        let ns = t.elapsed().as_secs_f64() * 1e9 / (n * ketten) as f64;
        let t = Instant::now(); let s2 = unsafe { ands(n, ketten, &a) };
        let ns2 = t.elapsed().as_secs_f64() * 1e9 / (n * ketten) as f64;
        println!("{ketten:2} Ketten: sdot {:.3} Takte je Befehl ({:.2} je Takt) | add.16b {:.3} Takte ({:.2} je Takt)  [{s} {s2}]", ns / ns_takt, ns_takt / ns, ns2 / ns_takt, ns_takt / ns2);
    }
}
