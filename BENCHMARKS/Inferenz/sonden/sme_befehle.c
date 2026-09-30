// Mikroprobe: was kosten einfache Vektorbefehle im Streaming-Modus, und die ZA-Punktprodukte (SME2)?
#include <arm_sme.h>
#include <stdio.h>
#include <stdint.h>
#include <time.h>
static double jetzt(void){ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec + t.tv_nsec*1e-9; }
static uint8_t A[4096], B[4096];
#define N 20000000L

__arm_locally_streaming static uint64_t l_and(void){ svbool_t p=svptrue_b8(); svuint8_t a=svld1_u8(p,A), s0=svdup_u8(0),s1=s0,s2=s0,s3=s0;
  for(long i=0;i<N;i++){ s0=svadd_u8_x(p,s0,svand_u8_x(p,a,s1)); s1=svadd_u8_x(p,s1,svlsr_n_u8_x(p,a,2)); s2=svadd_u8_x(p,s2,svand_u8_x(p,a,s3)); s3=svadd_u8_x(p,s3,svlsr_n_u8_x(p,a,4)); }
  return svaddv_u8(p,svadd_u8_x(p,svadd_u8_x(p,s0,s1),svadd_u8_x(p,s2,s3))); }
__arm_locally_streaming static uint64_t l_load(void){ svbool_t p=svptrue_b8(); svuint8_t s0=svdup_u8(0),s1=s0,s2=s0,s3=s0;
  for(long i=0;i<N;i++){ long o=(i*64)&2047; s0=sveor_u8_x(p,s0,svld1_u8(p,A+o)); s1=sveor_u8_x(p,s1,svld1_u8(p,B+o)); s2=sveor_u8_x(p,s2,svld1_u8(p,A+o+64)); s3=sveor_u8_x(p,s3,svld1_u8(p,B+o+64)); }
  return svaddv_u8(p,svadd_u8_x(p,svadd_u8_x(p,s0,s1),svadd_u8_x(p,s2,s3))); }
__arm_locally_streaming static uint64_t l_dot32(void){ svbool_t p=svptrue_b8(); svint8_t a=svld1_s8(p,(int8_t*)A),b=svld1_s8(p,(int8_t*)B); svint32_t s0=svdup_s32(0),s1=s0,s2=s0,s3=s0;
  for(long i=0;i<N;i++){ s0=svdot_s32(s0,a,b); s1=svdot_s32(s1,b,a); s2=svdot_s32(s2,a,a); s3=svdot_s32(s3,b,b); }
  return svaddv_s32(svptrue_b32(),svadd_s32_x(svptrue_b32(),svadd_s32_x(svptrue_b32(),s0,s1),svadd_s32_x(svptrue_b32(),s2,s3))); }
// SME2: vier Vektoren mal einer nach ZA (vgx4), i8 -> i32
__arm_locally_streaming __arm_new("za") static uint64_t l_zadot(void){ svbool_t p=svptrue_b8(); svint8_t a=svld1_s8(p,(int8_t*)A),b=svld1_s8(p,(int8_t*)B); svzero_za();
  svint8x4_t v=svcreate4_s8(a,b,a,b);
  for(long i=0;i<N;i++){ svdot_single_za32_s8_vg1x4(0,v,a); svdot_single_za32_s8_vg1x4(4,v,b); svdot_single_za32_s8_vg1x4(0,v,b); svdot_single_za32_s8_vg1x4(4,v,a); }
  return svaddv_s32(svptrue_b32(), svread_hor_za32_s32_m(svdup_s32(0), svptrue_b32(), 0, 0)); }
__arm_locally_streaming __arm_new("za") static uint64_t l_mopa8(void){ svbool_t p=svptrue_b8(); svint8_t a=svld1_s8(p,(int8_t*)A),b=svld1_s8(p,(int8_t*)B); svzero_za();
  for(long i=0;i<N;i++){ svmopa_za32_s8_m(0,p,p,a,b); svmopa_za32_s8_m(1,p,p,b,a); svmopa_za32_s8_m(2,p,p,a,a); svmopa_za32_s8_m(3,p,p,b,b); }
  return svaddv_s32(svptrue_b32(), svread_hor_za32_s32_m(svdup_s32(0), svptrue_b32(), 0, 0)); }
__arm_locally_streaming __arm_new("za") static uint64_t l_lesen(void){ svbool_t p=svptrue_b32(); svzero_za(); svint32_t s=svdup_s32(0);
  for(long i=0;i<N;i++){ s=svadd_s32_x(p,s,svread_hor_za32_s32_m(s,p,0,i&15)); s=svadd_s32_x(p,s,svread_hor_za32_s32_m(s,p,1,i&15)); s=svadd_s32_x(p,s,svread_hor_za32_s32_m(s,p,2,i&15)); s=svadd_s32_x(p,s,svread_hor_za32_s32_m(s,p,3,i&15)); }
  return svaddv_s32(p,s); }
static volatile uint64_t senke;
static void leer(void){}
__arm_locally_streaming static void l_modus(void){ }
int main(void){
  for(int i=0;i<4096;i++){A[i]=i*37+1;B[i]=i*91+3;}
  struct { const char*n; uint64_t(*f)(void); double prod; } t[] = {
    {"and/lsr/add.b (8 je Durchgang)", l_and, 0}, {"ld1b+eor (4 Paare)", l_load, 0}, {"sdot z.s,z.b (4)", l_dot32, 64},
    {"ZA sdot vgx4 i8 (4)", l_zadot, 256}, {"smopa i8 (4)", l_mopa8, 1024}, {"ZA lesen+add (4 Paare)", l_lesen, 0} };
  for (int k=0;k<6;k++){ double t0=jetzt(); senke=t[k].f(); double s=jetzt()-t0; double je = s*1e9/(4.0*N);
    printf("%-32s %6.3f ns je Befehl(spaar)", t[k].n, je); if (t[k].prod>0) printf("  %8.1f G Byteprodukte/s", t[k].prod/je); printf("\n"); }
  double t0=jetzt(); for(long i=0;i<2000000;i++){ l_modus(); } printf("smstart+smstop: %.1f ns je Paar\n", (jetzt()-t0)*1e9/2e6);
  return 0; }
