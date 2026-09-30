#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

struct Form { int32_t zeilen; int32_t spalten; int32_t eingaben; };

// Vergleichsmass: der vorhandene Weg, int8 mal int8 ueber matmul2d.
kernel void produkt(device int8_t *w [[buffer(0)]], device int8_t *x [[buffer(1)]], device int32_t *c [[buffer(2)]],
                    constant Form &f [[buffer(3)]], uint2 gruppe [[threadgroup_position_in_grid]]) {
    auto X = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(x, dextents<int32_t, 2>(f.spalten, f.eingaben));
    auto W = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(w, dextents<int32_t, 2>(f.spalten, f.zeilen));
    auto C = tensor<device int32_t, dextents<int32_t, 2>, tensor_inline>(c, dextents<int32_t, 2>(f.zeilen, f.eingaben));
    constexpr auto d = matmul2d_descriptor(16, 64, static_cast<int>(dynamic_extent), false, true, false);
    matmul2d<d, execution_simdgroups<4>> op;
    auto teil_x = X.slice(0, int(gruppe.y) * 16);
    auto teil_w = W.slice(0, int(gruppe.x) * 64);
    auto teil_c = C.slice(int(gruppe.x) * 64, int(gruppe.y) * 16);
    op.run(teil_x, teil_w, teil_c);
}

// Eigener Kern: ein Faden je (Zeile, Eingabe), Codes aus dem 2-Bit-Muster.
kernel void ternaer(device uint *muster [[buffer(0)]], device int16_t *betraege [[buffer(1)]],
                    device int16_t *x [[buffer(2)]], device int64_t *aus [[buffer(3)]],
                    constant Form &f [[buffer(4)]], uint2 ort [[thread_position_in_grid]]) {
    if (int(ort.x) >= f.zeilen || int(ort.y) >= f.eingaben) { return; }
    int gruppen = f.spalten / 128;
    long z = long(ort.x), b = long(ort.y);
    const device uint *m = muster + z * gruppen * 8;
    const device int16_t *xs = x + b * f.spalten;
    const device int16_t *bt = betraege + z * gruppen;
    int64_t acc = 0;
    for (int g = 0; g < gruppen; g++) {
        int s = 0;
        for (int blk = 0; blk < 2; blk++) {
            const device int16_t *xb = xs + g * 128 + blk * 64;
            for (int q = 0; q < 4; q++) {
                uint wort = m[g * 8 + blk * 4 + q];
                for (int k = 0; k < 4; k++) {
                    uint B = (wort >> (8 * k)) & 255u;
                    int j = q * 4 + k;
                    s += (int(B & 3u) - 1) * int(xb[j]) + (int((B >> 2) & 3u) - 1) * int(xb[16 + j])
                       + (int((B >> 4) & 3u) - 1) * int(xb[32 + j]) + (int(B >> 6) - 1) * int(xb[48 + j]);
                }
            }
        }
        acc += int64_t(s) * int64_t(bt[g]);
    }
    aus[b * long(f.zeilen) + z] = acc;
}

// Variante: ein Faden je (Zeile, VIER Eingaben). Die Codes eines Bytes
// entstehen einmal und dienen vier Eingaben; x liegt je Spalte als short4.
// Gerechnet wird mit c in 0..3 statt t = c - 1; die Summe der Eingabe je
// Gruppe (summen) wird danach abgezogen.
kernel void ternaer4(device uint *muster [[buffer(0)]], device int16_t *betraege [[buffer(1)]],
                     device packed_short4 *x [[buffer(2)]], device int64_t *aus [[buffer(3)]],
                     constant Form &f [[buffer(4)]], device int4 *summen [[buffer(5)]],
                     uint2 ort [[thread_position_in_grid]]) {
    int viertel = (f.eingaben + 3) / 4;
    if (int(ort.x) >= f.zeilen || int(ort.y) >= viertel) { return; }
    int gruppen = f.spalten / 128;
    long z = long(ort.x), v = long(ort.y);
    const device uint *m = muster + z * gruppen * 8;
    const device packed_short4 *xs = x + v * f.spalten;
    const device int16_t *bt = betraege + z * gruppen;
    const device int4 *su = summen + v * gruppen;
    long4 acc = long4(0);
    for (int g = 0; g < gruppen; g++) {
        int4 s = int4(0);
        for (int blk = 0; blk < 2; blk++) {
            const device packed_short4 *xb = xs + g * 128 + blk * 64;
            for (int q = 0; q < 4; q++) {
                uint wort = m[g * 8 + blk * 4 + q];
                for (int k = 0; k < 4; k++) {
                    uint B = wort >> (8 * k);
                    int j = q * 4 + k;
                    s += int(B & 3u) * int4(short4(xb[j])) + int((B >> 2) & 3u) * int4(short4(xb[16 + j]))
                       + int((B >> 4) & 3u) * int4(short4(xb[32 + j])) + int((B >> 6) & 3u) * int4(short4(xb[48 + j]));
                }
            }
        }
        acc += long4(s - su[g]) * long(bt[g]);
    }
    for (int k = 0; k < 4; k++) {
        long b = v * 4 + k;
        if (b < long(f.eingaben)) { aus[b * long(f.zeilen) + z] = acc[k]; }
    }
}

// Versuch: matmul2d je 128er-Gruppe (K = 128), das Teilprodukt einer Kachel
// in einen eigenen Zwischenbereich, danach mit dem Betrag der Gruppe
// gewichtet in int64 gesammelt.
struct Gruppenform { int32_t zeilen; int32_t spalten; int32_t eingaben; int32_t kacheln_x; int32_t breite; };
kernel void gruppenweise(device int8_t *w [[buffer(0)]], device int8_t *x [[buffer(1)]], device int32_t *zw [[buffer(2)]],
                         constant Gruppenform &f [[buffer(3)]], device int16_t *betraege [[buffer(4)]],
                         device int64_t *acc [[buffer(5)]],
                         uint2 gruppe [[threadgroup_position_in_grid]], uint faden [[thread_index_in_threadgroup]]) {
    auto X = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(x, dextents<int32_t, 2>(f.spalten, f.eingaben));
    auto W = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(w, dextents<int32_t, 2>(f.spalten, f.zeilen));
    long kachel = long(gruppe.y) * f.kacheln_x + long(gruppe.x);
    auto C = tensor<device int32_t, dextents<int32_t, 2>, tensor_inline>(zw + kachel * 1024, dextents<int32_t, 2>(64, 16));
    constexpr auto d = matmul2d_descriptor(16, 64, 128, false, true, false);
    matmul2d<d, execution_simdgroups<4>> op;
    int gruppen = f.spalten / 128;
    for (int g = 0; g < gruppen; g++) {
        auto teil_x = X.slice(g * 128, int(gruppe.y) * 16);
        auto teil_w = W.slice(g * 128, int(gruppe.x) * 64);
        op.run(teil_x, teil_w, C);
        threadgroup_barrier(mem_flags::mem_device);
        for (int i = int(faden); i < 1024; i += f.breite) {
            int zl = i % 64, e = i / 64;
            long z = long(gruppe.x) * 64 + zl, b = long(gruppe.y) * 16 + e;
            if (z < long(f.zeilen) && b < long(f.eingaben)) {
                acc[b * long(f.zeilen) + z] += int64_t(zw[kachel * 1024 + i]) * int64_t(betraege[z * gruppen + g]);
            }
        }
        threadgroup_barrier(mem_flags::mem_device);
    }
}
