// Mikroprobe: Durchsatz der SME-Einheit fuer i16 x i16 (sdot 512 Bit, smopa nach ZA.D).
#include <arm_sme.h>
#include <stdio.h>
#include <stdint.h>
#include <time.h>
#include <pthread.h>
#include <stdlib.h>

static double jetzt(void){ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec + t.tv_nsec*1e-9; }

// sdot: 32 i16-Produkte je Befehl (8 Lanes i64 zu je 4 Produkten)
__arm_locally_streaming static int64_t lauf_sdot(const int16_t *a, const int16_t *b, long n) {
    svbool_t pg = svptrue_b16();
    svint64_t s0 = svdup_s64(0), s1 = svdup_s64(0), s2 = svdup_s64(0), s3 = svdup_s64(0);
    for (long i = 0; i < n; i++) {
        svint16_t va = svld1_s16(pg, a + ((i*32) & 1023)), vb = svld1_s16(pg, b + ((i*32) & 1023));
        s0 = svdot_s64(s0, va, vb); s1 = svdot_s64(s1, vb, va);
        s2 = svdot_s64(s2, va, va); s3 = svdot_s64(s3, vb, vb);
    }
    return svaddv_s64(svptrue_b64(), svadd_s64_x(svptrue_b64(), svadd_s64_x(svptrue_b64(), s0, s1), svadd_s64_x(svptrue_b64(), s2, s3)));
}

// smopa: 8x8 Kachel i64, je Element 4 Produkte: 256 Produkte je Befehl
__arm_locally_streaming __arm_new("za") static int64_t lauf_smopa(const int16_t *a, const int16_t *b, long n) {
    svbool_t pg = svptrue_b16();
    svzero_za();
    for (long i = 0; i < n; i++) {
        svint16_t va = svld1_s16(pg, a + ((i*32) & 1023)), vb = svld1_s16(pg, b + ((i*32) & 1023));
        svmopa_za64_s16_m(0, pg, pg, va, vb);
        svmopa_za64_s16_m(1, pg, pg, vb, va);
        svmopa_za64_s16_m(2, pg, pg, va, va);
        svmopa_za64_s16_m(3, pg, pg, vb, vb);
    }
    svint64_t z = svread_hor_za64_s64_m(svdup_s64(0), svptrue_b64(), 0, 0);
    return svaddv_s64(svptrue_b64(), z);
}

static int16_t A[2048], B[2048];
static long N = 20000000;
static void *faden(void *arg) {
    long art = (long)arg; volatile int64_t r;
    if (art == 0) r = lauf_sdot(A, B, N); else r = lauf_smopa(A, B, N);
    (void)r; return NULL;
}

int main(int argc, char **argv) {
    for (int i = 0; i < 2048; i++) { A[i] = (int16_t)(i*37 - 3000); B[i] = (int16_t)(i*91 - 5000); }
    int maxf = argc > 1 ? atoi(argv[1]) : 8;
    for (int art = 0; art < 2; art++) {
        for (int f = 1; f <= maxf; f = f < 4 ? f + 1 : f * 2) {
            pthread_t t[64]; double t0 = jetzt();
            for (int k = 0; k < f; k++) pthread_create(&t[k], NULL, faden, (void*)(long)art);
            for (int k = 0; k < f; k++) pthread_join(t[k], NULL);
            double s = jetzt() - t0;
            double befehle = 4.0 * N * f, prod = befehle * (art == 0 ? 32 : 256);
            printf("%s %2d Faeden: %6.2f ns je Befehl, %8.1f G Produkte/s gesamt\n", art == 0 ? "sdot.h 512" : "smopa i16 ", f, s * 1e9 / (4.0 * N), prod / s / 1e9);
        }
    }
    return 0;
}
